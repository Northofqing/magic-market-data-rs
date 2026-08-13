#![deny(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

mod discovery;

#[cfg(all(windows, not(target_arch = "x86_64")))]
compile_error!("magic-tdx-native-bridge Windows diagnostics require x86_64");

use std::env;
use std::process::ExitCode;

use serde::Serialize;

const SCHEMA_VERSION: u8 = 1;
const DIAGNOSTIC_JSON_LIMIT: usize = 64 * 1024;
const EXIT_UNAVAILABLE: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Command {
    Discover,
    Probe,
    Serve,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Platform {
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Architecture {
    X86_64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Discovered,
    #[cfg_attr(windows, allow(dead_code))]
    Unsupported,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ReasonCode {
    TerminalDiscovered,
    #[cfg_attr(windows, allow(dead_code))]
    PlatformUnsupported,
    TerminalNotRunning,
    AmbiguousTerminal,
    IdentityMismatch,
    DiscoveryEvidenceUnavailable,
    ImplementationUnavailable,
    DiagnosticOutputUnavailable,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
struct CommandReport {
    schema_version: u8,
    command: Command,
    target_platform: Platform,
    target_architecture: Architecture,
    status: Status,
    reason_code: ReasonCode,
    message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    discovery: Option<discovery::DiscoveryEvidence>,
}

#[derive(Debug, Eq, PartialEq)]
enum CliError {
    MissingCommand,
    UnexpectedArgument,
    UnknownCommand,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = match parse_command(&args) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{}", cli_error_message(error));
            return ExitCode::from(2);
        }
    };

    let report = execute(command);
    let encoded = encode_report(&report, DIAGNOSTIC_JSON_LIMIT);
    println!("{}", encoded.json);
    if encoded.used_fallback {
        eprintln!("diagnostic status used the typed bounded fallback");
        return ExitCode::from(1);
    }
    ExitCode::from(exit_code(&report))
}

fn exit_code(report: &CommandReport) -> u8 {
    if report.status == Status::Discovered {
        0
    } else {
        EXIT_UNAVAILABLE
    }
}

#[derive(Debug, Eq, PartialEq)]
struct EncodedReport {
    json: String,
    used_fallback: bool,
}

fn encode_report(report: &CommandReport, maximum: usize) -> EncodedReport {
    if let Ok(json) = serde_json::to_string(report) {
        if json.len() <= maximum {
            return EncodedReport {
                json,
                used_fallback: false,
            };
        }
    }
    let json = typed_fallback_json(report.command);
    debug_assert!(json.len() <= DIAGNOSTIC_JSON_LIMIT);
    EncodedReport {
        json,
        used_fallback: true,
    }
}

fn typed_fallback_json(command: Command) -> String {
    let fallback = CommandReport {
        schema_version: SCHEMA_VERSION,
        command,
        target_platform: Platform::Windows,
        target_architecture: Architecture::X86_64,
        status: Status::Unavailable,
        reason_code: ReasonCode::DiagnosticOutputUnavailable,
        message: "diagnostic report could not be encoded within the output bound",
        discovery: None,
    };
    if let Ok(json) = serde_json::to_string(&fallback) {
        if json.len() <= DIAGNOSTIC_JSON_LIMIT {
            return json;
        }
    }
    static_fallback_json(command).to_owned()
}

fn static_fallback_json(command: Command) -> &'static str {
    match command {
        Command::Discover => concat!(
            "{\"schema_version\":1,\"command\":\"discover\",",
            "\"target_platform\":\"windows\",\"target_architecture\":\"x86_64\",",
            "\"status\":\"unavailable\",\"reason_code\":\"diagnostic_output_unavailable\",",
            "\"message\":\"diagnostic report could not be encoded within the output bound\"}"
        ),
        Command::Probe => concat!(
            "{\"schema_version\":1,\"command\":\"probe\",",
            "\"target_platform\":\"windows\",\"target_architecture\":\"x86_64\",",
            "\"status\":\"unavailable\",\"reason_code\":\"diagnostic_output_unavailable\",",
            "\"message\":\"diagnostic report could not be encoded within the output bound\"}"
        ),
        Command::Serve => concat!(
            "{\"schema_version\":1,\"command\":\"serve\",",
            "\"target_platform\":\"windows\",\"target_architecture\":\"x86_64\",",
            "\"status\":\"unavailable\",\"reason_code\":\"diagnostic_output_unavailable\",",
            "\"message\":\"diagnostic report could not be encoded within the output bound\"}"
        ),
    }
}

fn parse_command(args: &[String]) -> Result<Command, CliError> {
    let [argument] = args else {
        return Err(if args.is_empty() {
            CliError::MissingCommand
        } else {
            CliError::UnexpectedArgument
        });
    };

    match argument.as_str() {
        "--discover" => Ok(Command::Discover),
        "--probe" => Ok(Command::Probe),
        "--serve" => Ok(Command::Serve),
        _ => Err(CliError::UnknownCommand),
    }
}

fn cli_error_message(error: CliError) -> &'static str {
    match error {
        CliError::MissingCommand => "one command is required: --discover, --probe, or --serve",
        CliError::UnexpectedArgument => "exactly one command is accepted",
        CliError::UnknownCommand => "unknown command; expected --discover, --probe, or --serve",
    }
}

fn execute(command: Command) -> CommandReport {
    match command {
        Command::Discover => discover(),
        Command::Probe | Command::Serve => unavailable_command(command),
    }
}

#[cfg(not(windows))]
fn discover() -> CommandReport {
    CommandReport {
        schema_version: SCHEMA_VERSION,
        command: Command::Discover,
        target_platform: Platform::Windows,
        target_architecture: Architecture::X86_64,
        status: Status::Unsupported,
        reason_code: ReasonCode::PlatformUnsupported,
        message: "terminal discovery is supported only on Windows",
        discovery: None,
    }
}

#[cfg(windows)]
fn discover() -> CommandReport {
    report_for_discovery(discovery::discover_current_session())
}

#[cfg(windows)]
fn report_for_discovery(evidence: discovery::DiscoveryEvidence) -> CommandReport {
    let (status, reason_code, message) = match &evidence {
        discovery::DiscoveryEvidence::Discovered { .. } => (
            Status::Discovered,
            ReasonCode::TerminalDiscovered,
            "one current-user current-session terminal was discovered; fixed loopback health may now be probed",
        ),
        discovery::DiscoveryEvidence::NotRunning { .. } => (
            Status::Unavailable,
            ReasonCode::TerminalNotRunning,
            "no exact terminal process is running in the current session",
        ),
        discovery::DiscoveryEvidence::IdentityMismatch { .. } => (
            Status::Unavailable,
            ReasonCode::IdentityMismatch,
            "the terminal candidate is not owned by the current user",
        ),
        discovery::DiscoveryEvidence::Ambiguous { .. } => (
            Status::Unavailable,
            ReasonCode::AmbiguousTerminal,
            "multiple terminal processes are running in the current session",
        ),
        discovery::DiscoveryEvidence::Failed { .. } => (
            Status::Unavailable,
            ReasonCode::DiscoveryEvidenceUnavailable,
            "terminal discovery evidence could not be completed",
        ),
    };
    CommandReport {
        schema_version: SCHEMA_VERSION,
        command: Command::Discover,
        target_platform: Platform::Windows,
        target_architecture: Architecture::X86_64,
        status,
        reason_code,
        message,
        discovery: Some(evidence),
    }
}

fn unavailable_command(command: Command) -> CommandReport {
    CommandReport {
        schema_version: SCHEMA_VERSION,
        command,
        target_platform: Platform::Windows,
        target_architecture: Architecture::X86_64,
        status: Status::Unavailable,
        reason_code: ReasonCode::ImplementationUnavailable,
        message: "this command is not implemented and performs no network or module operation",
        discovery: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn command_parser_accepts_only_one_known_switch() {
        assert_eq!(parse_command(&args(&["--discover"])), Ok(Command::Discover));
        assert_eq!(parse_command(&args(&["--probe"])), Ok(Command::Probe));
        assert_eq!(parse_command(&args(&["--serve"])), Ok(Command::Serve));
        assert_eq!(parse_command(&[]), Err(CliError::MissingCommand));
        assert_eq!(
            parse_command(&args(&["--discover", "extra"])),
            Err(CliError::UnexpectedArgument)
        );
        assert_eq!(
            parse_command(&args(&["--unknown"])),
            Err(CliError::UnknownCommand)
        );
    }

    #[test]
    fn probe_and_serve_are_explicitly_unavailable() {
        for command in [Command::Probe, Command::Serve] {
            let report = execute(command);
            assert_eq!(report.status, Status::Unavailable);
            assert_eq!(report.reason_code, ReasonCode::ImplementationUnavailable);
            assert_eq!(exit_code(&report), EXIT_UNAVAILABLE);
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn discovery_is_typed_unsupported_off_windows() {
        let report = execute(Command::Discover);
        assert_eq!(report.status, Status::Unsupported);
        assert_eq!(report.reason_code, ReasonCode::PlatformUnsupported);
        assert_eq!(exit_code(&report), EXIT_UNAVAILABLE);
    }

    #[cfg(windows)]
    #[test]
    fn runtime_discovery_exit_matches_typed_status() {
        let report = execute(Command::Discover);
        assert!(report.discovery.is_some());
        assert_eq!(exit_code(&report) == 0, report.status == Status::Discovered);
    }

    #[test]
    fn report_serialization_is_stable_and_typed() {
        let report = execute(Command::Probe);
        let encoded = serde_json::to_vec(&report).unwrap();
        assert!(encoded.len() <= DIAGNOSTIC_JSON_LIMIT);
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["command"], "probe");
        assert_eq!(value["status"], "unavailable");
        assert_eq!(value["reason_code"], "implementation_unavailable");
    }

    #[test]
    fn oversized_report_uses_bounded_typed_json_fallback() {
        let report = execute(Command::Probe);
        let encoded = encode_report(&report, 1);
        assert!(encoded.used_fallback);
        assert!(encoded.json.len() <= DIAGNOSTIC_JSON_LIMIT);
        let value: serde_json::Value = serde_json::from_str(&encoded.json).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["command"], "probe");
        assert_eq!(value["status"], "unavailable");
        assert_eq!(value["reason_code"], "diagnostic_output_unavailable");
    }
}
