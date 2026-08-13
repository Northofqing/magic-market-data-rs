use std::env;
use std::io::Read;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const BRIDGE_STEM: &str = "magic-tdx-native-bridge";
const DISCOVER_ARGUMENT: &str = "--discover";
const DISCOVERY_SCHEMA_VERSION: u8 = 1;
const CHILD_WAIT_QUANTUM: Duration = Duration::from_millis(10);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct CandidateEvidence {
    pub(crate) discovery_schema_version: u8,
    pub(crate) process_id: u32,
    pub(crate) session_id: u32,
    pub(crate) process_creation_time_100ns_since_1601: u64,
    pub(crate) executable_architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) executable_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) executable_file_version: Option<NumericExecutableVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) executable_product_version: Option<NumericExecutableVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) executable_version_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) executable_version_failure: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DiscoveryOutcome {
    None { reason: String },
    Candidate(CandidateEvidence),
    Rejected { reason: String },
}

pub(crate) trait DiscoverTerminal {
    fn discover(&mut self) -> Result<DiscoveryOutcome, DiscoveryError>;
}

pub(crate) struct SiblingDiscovery {
    bridge: PathBuf,
    timeout: Duration,
    maximum_bytes: usize,
}

impl SiblingDiscovery {
    pub(crate) fn production(
        timeout: Duration,
        maximum_bytes: usize,
    ) -> Result<Self, DiscoveryError> {
        let current = env::current_exe().map_err(DiscoveryError::CurrentExecutable)?;
        let directory = current
            .parent()
            .ok_or(DiscoveryError::MissingExecutableDirectory)?;
        Ok(Self {
            bridge: directory.join(bridge_filename()),
            timeout,
            maximum_bytes,
        })
    }

    #[cfg(test)]
    fn bridge_path(&self) -> &Path {
        &self.bridge
    }

    fn execute(&self) -> Result<BridgeOutput, DiscoveryError> {
        let mut child = Command::new(&self.bridge)
            .arg(DISCOVER_ARGUMENT)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(DiscoveryError::Launch)?;
        let stdout = child.stdout.take().ok_or(DiscoveryError::MissingStdout)?;
        let maximum = self.maximum_bytes;
        let reader = thread::spawn(move || read_bounded(stdout, maximum));
        let started = Instant::now();
        let status = loop {
            match child.try_wait().map_err(DiscoveryError::Wait)? {
                Some(status) => break status,
                None if started.elapsed() < self.timeout => thread::sleep(CHILD_WAIT_QUANTUM),
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    return Err(DiscoveryError::Timeout);
                }
            }
        };
        let bytes = reader
            .join()
            .map_err(|_| DiscoveryError::ReaderPanicked)??;
        Ok(BridgeOutput {
            exit_code: status.code(),
            bytes,
        })
    }
}

impl DiscoverTerminal for SiblingDiscovery {
    fn discover(&mut self) -> Result<DiscoveryOutcome, DiscoveryError> {
        parse_bridge_output(self.execute()?)
    }
}

fn bridge_filename() -> &'static str {
    if cfg!(windows) {
        "magic-tdx-native-bridge.exe"
    } else {
        BRIDGE_STEM
    }
}

fn read_bounded(input: impl Read, maximum: usize) -> Result<Vec<u8>, DiscoveryError> {
    let limit = u64::try_from(maximum)
        .map_err(|_| DiscoveryError::InvalidMaximum)?
        .checked_add(1)
        .ok_or(DiscoveryError::InvalidMaximum)?;
    let mut bytes = Vec::new();
    input
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(DiscoveryError::Read)?;
    if bytes.len() > maximum {
        return Err(DiscoveryError::OutputTooLarge { maximum });
    }
    Ok(bytes)
}

#[derive(Debug)]
struct BridgeOutput {
    exit_code: Option<i32>,
    bytes: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    schema_version: u8,
    command: String,
    target_platform: String,
    target_architecture: String,
    status: String,
    reason_code: String,
    message: String,
    #[serde(default)]
    discovery: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateWire {
    process_id: u32,
    session_id: u32,
    image_path: String,
    current_user_identity_verified: bool,
    process_creation_time_100ns_since_1601: u64,
    image_name_verified: bool,
    executable: ExecutableWire,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutableWire {
    size_bytes: Option<u64>,
    sha256: Option<String>,
    hash_failure: Option<Value>,
    pe_machine: Option<u16>,
    architecture: Option<String>,
    architecture_failure: Option<Value>,
    file_version: Option<NumericExecutableVersion>,
    product_version: Option<NumericExecutableVersion>,
    version_source: Option<String>,
    version_failure: Option<Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NumericExecutableVersion {
    pub(crate) major: u16,
    pub(crate) minor: u16,
    pub(crate) build: u16,
    pub(crate) revision: u16,
}

fn parse_bridge_output(output: BridgeOutput) -> Result<DiscoveryOutcome, DiscoveryError> {
    if !matches!(output.exit_code, Some(0 | 3)) {
        return Err(DiscoveryError::UnexpectedExit(output.exit_code));
    }
    let line = single_json_line(&output.bytes)?;
    let report: Report = serde_json::from_str(line).map_err(DiscoveryError::Json)?;
    if report.schema_version != DISCOVERY_SCHEMA_VERSION {
        return Err(DiscoveryError::SchemaVersion(report.schema_version));
    }
    if report.command != "discover" {
        return Err(DiscoveryError::UnexpectedCommand(report.command));
    }
    validate_report_text(&report.target_platform)?;
    validate_report_text(&report.target_architecture)?;
    validate_report_text(&report.status)?;
    validate_report_text(&report.reason_code)?;
    validate_report_text(&report.message)?;

    let Some(discovery) = report.discovery else {
        return Ok(DiscoveryOutcome::Rejected {
            reason: report.reason_code,
        });
    };
    let state = discovery
        .get("state")
        .and_then(Value::as_str)
        .ok_or(DiscoveryError::MissingDiscoveryState)?;
    if state == "not_running" {
        return Ok(DiscoveryOutcome::None {
            reason: report.reason_code,
        });
    }
    if state != "discovered" {
        return Ok(DiscoveryOutcome::Rejected {
            reason: report.reason_code,
        });
    }
    if report.status != "discovered"
        || report.reason_code != "terminal_discovered"
        || report.target_platform != "windows"
        || report.target_architecture != "x86_64"
        || output.exit_code != Some(0)
    {
        return Err(DiscoveryError::InvalidCandidate);
    }
    if discovery
        .get("eligible_for_fixed_loopback_health_probe")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(DiscoveryError::InvalidCandidate);
    }
    let current_session = u32::try_from(
        discovery
            .get("current_session_id")
            .and_then(Value::as_u64)
            .ok_or(DiscoveryError::MissingCurrentSession)?,
    )
    .map_err(|_| DiscoveryError::InvalidCandidate)?;
    let candidate: CandidateWire = serde_json::from_value(
        discovery
            .get("terminal")
            .cloned()
            .ok_or(DiscoveryError::MissingCandidate)?,
    )
    .map_err(DiscoveryError::Json)?;
    if candidate.process_id == 0
        || candidate.session_id != current_session
        || !candidate.current_user_identity_verified
        || !candidate.image_name_verified
        || !is_terminal_image(&candidate.image_path)
    {
        return Err(DiscoveryError::InvalidCandidate);
    }
    let _provenance_detail = (
        candidate.executable.size_bytes,
        candidate.executable.hash_failure,
        candidate.executable.pe_machine,
        candidate.executable.architecture_failure,
    );
    Ok(DiscoveryOutcome::Candidate(CandidateEvidence {
        discovery_schema_version: report.schema_version,
        process_id: candidate.process_id,
        session_id: candidate.session_id,
        process_creation_time_100ns_since_1601: candidate.process_creation_time_100ns_since_1601,
        executable_architecture: candidate.executable.architecture,
        executable_sha256: candidate.executable.sha256,
        executable_file_version: candidate.executable.file_version,
        executable_product_version: candidate.executable.product_version,
        executable_version_source: candidate.executable.version_source,
        executable_version_failure: candidate.executable.version_failure,
    }))
}

fn single_json_line(bytes: &[u8]) -> Result<&str, DiscoveryError> {
    let text = std::str::from_utf8(bytes).map_err(DiscoveryError::Utf8)?;
    let line = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text);
    if line.is_empty() || line.contains(['\r', '\n']) {
        return Err(DiscoveryError::NotSingleLine);
    }
    Ok(line)
}

fn validate_report_text(value: &str) -> Result<(), DiscoveryError> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(DiscoveryError::InvalidReportText);
    }
    Ok(())
}

fn is_terminal_image(path: &str) -> bool {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .is_some_and(|name| name.eq_ignore_ascii_case("TdxW.exe"))
}

#[derive(Debug, Error)]
pub(crate) enum DiscoveryError {
    #[error("unable to resolve the service executable: {0}")]
    CurrentExecutable(#[source] std::io::Error),
    #[error("service executable has no sibling directory")]
    MissingExecutableDirectory,
    #[error("unable to launch fixed sibling discovery bridge: {0}")]
    Launch(#[source] std::io::Error),
    #[error("discovery bridge did not expose stdout")]
    MissingStdout,
    #[error("unable to wait for discovery bridge: {0}")]
    Wait(#[source] std::io::Error),
    #[error("discovery bridge exceeded the injected timeout")]
    Timeout,
    #[error("discovery output reader panicked")]
    ReaderPanicked,
    #[error("unable to read discovery output: {0}")]
    Read(#[source] std::io::Error),
    #[error("invalid discovery byte maximum")]
    InvalidMaximum,
    #[error("discovery output exceeded maximum {maximum}")]
    OutputTooLarge { maximum: usize },
    #[error("discovery bridge returned unexpected exit code {0:?}")]
    UnexpectedExit(Option<i32>),
    #[error("discovery output is not UTF-8: {0}")]
    Utf8(#[source] std::str::Utf8Error),
    #[error("discovery output must contain exactly one non-empty JSON line")]
    NotSingleLine,
    #[error("discovery output is not the strict JSON contract: {0}")]
    Json(#[source] serde_json::Error),
    #[error("unsupported discovery schema version {0}")]
    SchemaVersion(u8),
    #[error("unexpected bridge command {0}")]
    UnexpectedCommand(String),
    #[error("discovery report text is invalid")]
    InvalidReportText,
    #[error("discovery evidence has no typed state")]
    MissingDiscoveryState,
    #[error("discovery evidence has no current session")]
    MissingCurrentSession,
    #[error("candidate discovery evidence is absent")]
    MissingCandidate,
    #[error("candidate is not one verified current-session TdxW process")]
    InvalidCandidate,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(state: Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "command": "discover",
            "target_platform": "windows",
            "target_architecture": "x86_64",
            "status": "unavailable",
            "reason_code": "terminal_not_running",
            "message": "candidate found",
            "discovery": state
        }))
        .unwrap()
    }

    #[test]
    fn production_path_is_only_the_fixed_sibling_name() {
        let discovery = SiblingDiscovery::production(Duration::from_millis(1), 100).unwrap();
        assert_eq!(
            discovery.bridge_path().file_name().unwrap(),
            bridge_filename()
        );
    }

    #[test]
    fn discovered_current_user_candidate_proceeds_only_as_health_probe_evidence() {
        let bytes = report(serde_json::json!({
            "state": "discovered",
            "current_session_id": 7,
            "eligible_for_fixed_loopback_health_probe": true,
            "terminal": {
                "process_id": 42,
                "session_id": 7,
                "process_creation_time_100ns_since_1601": 1234,
                "image_path": "C:\\TDX\\TdxW.exe",
                "image_name_verified": true,
                "current_user_identity_verified": true,
                "executable": {
                    "size_bytes": 123,
                    "sha256": "abc",
                    "hash_failure": null,
                    "pe_machine": 34404,
                    "architecture": "x86_64",
                    "architecture_failure": null,
                    "file_version": {"major": 7, "minor": 72, "build": 90, "revision": 0},
                    "product_version": {"major": 1, "minor": 2, "build": 3, "revision": 4},
                    "version_source": "vs_fixedfileinfo_numeric",
                    "version_failure": null
                }
            }
        }));
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        value["status"] = Value::String("discovered".to_owned());
        value["reason_code"] = Value::String("terminal_discovered".to_owned());
        let outcome = parse_bridge_output(BridgeOutput {
            exit_code: Some(0),
            bytes: serde_json::to_vec(&value).unwrap(),
        })
        .unwrap();
        let DiscoveryOutcome::Candidate(candidate) = outcome else {
            panic!("candidate expected")
        };
        assert_eq!(candidate.process_id, 42);
        assert_eq!(candidate.discovery_schema_version, 1);
        assert_eq!(candidate.process_creation_time_100ns_since_1601, 1234);
        assert_eq!(candidate.executable_sha256.as_deref(), Some("abc"));
        assert_eq!(candidate.executable_file_version.unwrap().major, 7);
    }

    #[test]
    fn none_ambiguous_bad_identity_multiline_oversize_and_exit_fail_closed() {
        let none = report(serde_json::json!({"state": "not_running", "current_session_id": 7}));
        assert!(matches!(
            parse_bridge_output(BridgeOutput {
                exit_code: Some(3),
                bytes: none
            }),
            Ok(DiscoveryOutcome::None { .. })
        ));

        let ambiguous = report(serde_json::json!({"state": "ambiguous", "current_session_id": 7}));
        assert!(matches!(
            parse_bridge_output(BridgeOutput {
                exit_code: Some(3),
                bytes: ambiguous
            }),
            Ok(DiscoveryOutcome::Rejected { .. })
        ));

        let bad = report(serde_json::json!({
            "state": "discovered",
            "current_session_id": 7,
            "eligible_for_fixed_loopback_health_probe": true,
            "terminal": {
                "process_id": 42, "session_id": 8, "process_creation_time_100ns_since_1601": 1,
                "image_path": "C:\\TDX\\TdxW.exe", "image_name_verified": true,
                "current_user_identity_verified": true,
                "executable": {"size_bytes": null, "sha256": null, "hash_failure": null,
                    "pe_machine": null, "architecture": null, "architecture_failure": null,
                    "file_version": null, "product_version": null, "version_source": null,
                    "version_failure": {"stage":"executable_version_resource_size", "os_error": 1813}}
            }
        }));
        assert!(matches!(
            parse_bridge_output(BridgeOutput {
                exit_code: Some(3),
                bytes: bad
            }),
            Err(DiscoveryError::InvalidCandidate)
        ));
        assert!(matches!(
            single_json_line(b"{}\n{}"),
            Err(DiscoveryError::NotSingleLine)
        ));
        assert!(matches!(
            read_bounded(&b"123"[..], 2),
            Err(DiscoveryError::OutputTooLarge { maximum: 2 })
        ));
        assert!(matches!(
            parse_bridge_output(BridgeOutput {
                exit_code: Some(2),
                bytes: b"{}".to_vec()
            }),
            Err(DiscoveryError::UnexpectedExit(Some(2)))
        ));
    }
}
