//! Read-only discovery for an already-running local terminal.
//!
//! The Windows implementation is the crate's only operating-system FFI
//! boundary. It reads process identity and executable provenance only. The
//! process image path and the file later opened through that path are not an
//! atomic observation, so the optional hash, PE architecture, and fixed numeric
//! version resource are provenance, not provider admission or data-family
//! compatibility evidence.

#![cfg_attr(windows, allow(unsafe_code))]
#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;

const TERMINAL_IMAGE_NAME: &str = "TdxW.exe";
const MAX_AMBIGUOUS_PROCESS_IDS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProcessObservation {
    pub(crate) process_id: u32,
    pub(crate) session_id: u32,
    pub(crate) image_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProcessSelection {
    None,
    One(ProcessObservation),
    Multiple {
        matching_process_count: u32,
        evidence: Vec<ProcessObservation>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecutableArchitecture {
    X86,
    X86_64,
    Arm64,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct NumericExecutableVersion {
    pub(crate) major: u16,
    pub(crate) minor: u16,
    pub(crate) build: u16,
    pub(crate) revision: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) enum ExecutableVersionSource {
    #[serde(rename = "vs_fixedfileinfo_numeric")]
    VsFixedFileInfoNumeric,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ProvenanceFailure {
    pub(crate) stage: DiscoveryStage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) os_error: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ntstatus: Option<i32>,
}

impl From<DiscoveryFailure> for ProvenanceFailure {
    fn from(failure: DiscoveryFailure) -> Self {
        Self {
            stage: failure.stage,
            os_error: failure.os_error,
            ntstatus: failure.ntstatus,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ExecutableProvenance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hash_failure: Option<ProvenanceFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pe_machine: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) architecture: Option<ExecutableArchitecture>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) architecture_failure: Option<ProvenanceFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) file_version: Option<NumericExecutableVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) product_version: Option<NumericExecutableVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version_source: Option<ExecutableVersionSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version_failure: Option<ProvenanceFailure>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DiscoveredTerminal {
    pub(crate) process_id: u32,
    pub(crate) session_id: u32,
    pub(crate) process_creation_time_100ns_since_1601: u64,
    pub(crate) image_path: String,
    pub(crate) image_name_verified: bool,
    pub(crate) current_user_identity_verified: bool,
    pub(crate) executable: ExecutableProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum DiscoveryEvidence {
    NotRunning {
        current_session_id: u32,
    },
    IdentityMismatch {
        current_session_id: u32,
        process_id: u32,
        candidate_session_id: u32,
        process_creation_time_100ns_since_1601: u64,
        image_path: String,
    },
    Discovered {
        current_session_id: u32,
        terminal: DiscoveredTerminal,
        eligible_for_fixed_loopback_health_probe: bool,
    },
    Ambiguous {
        current_session_id: u32,
        matching_process_count: u32,
        process_ids: Vec<u32>,
        process_ids_truncated: bool,
    },
    Failed {
        stage: DiscoveryStage,
        #[serde(skip_serializing_if = "Option::is_none")]
        process_id: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        os_error: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ntstatus: Option<i32>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiscoveryStage {
    CurrentSession,
    ProcessSnapshot,
    ProcessEnumeration,
    ProcessSession,
    OpenProcess,
    ProcessIdentity,
    CurrentUserToken,
    CandidateUserToken,
    TokenUserInformation,
    TokenUserSid,
    ImagePath,
    ImagePathNotAbsolute,
    ImagePathNotUnicode,
    ExecutableMetadata,
    ExecutableNotRegularFile,
    ExecutableOpenForHash,
    ExecutableTooLargeForHash,
    ExecutableReadForHash,
    ExecutableChangedDuringHash,
    ExecutableOpenForArchitecture,
    ExecutableReadForArchitecture,
    ExecutableInvalidPe,
    ExecutableVersionResourceSize,
    ExecutableVersionResourceTooLarge,
    ExecutableVersionResourceRead,
    ExecutableVersionFixedQuery,
    ExecutableVersionFixedInvalid,
    HashAlgorithmProvider,
    HashProperty,
    HashCreate,
    HashUpdate,
    HashFinish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DiscoveryFailure {
    pub(crate) stage: DiscoveryStage,
    pub(crate) process_id: Option<u32>,
    pub(crate) os_error: Option<u32>,
    pub(crate) ntstatus: Option<i32>,
}

pub(crate) fn select_processes(
    entries: impl IntoIterator<Item = ProcessObservation>,
    current_session_id: u32,
) -> ProcessSelection {
    let mut matching_process_count = 0_u32;
    let mut evidence = Vec::with_capacity(MAX_AMBIGUOUS_PROCESS_IDS);
    for entry in entries.into_iter().filter(|entry| {
        entry.session_id == current_session_id
            && entry.image_name.eq_ignore_ascii_case(TERMINAL_IMAGE_NAME)
    }) {
        matching_process_count = matching_process_count.saturating_add(1);
        if evidence.len() < MAX_AMBIGUOUS_PROCESS_IDS {
            evidence.push(entry);
            continue;
        }
        let Some((largest_index, largest)) = evidence
            .iter()
            .enumerate()
            .max_by_key(|(_, observed)| observed.process_id)
        else {
            continue;
        };
        if entry.process_id < largest.process_id {
            evidence[largest_index] = entry;
        }
    }
    evidence.sort_by_key(|entry| entry.process_id);

    match matching_process_count {
        0 => ProcessSelection::None,
        1 => ProcessSelection::One(evidence.remove(0)),
        _ => ProcessSelection::Multiple {
            matching_process_count,
            evidence,
        },
    }
}

fn exact_sid_match(current_user_sid: &[u8], candidate_user_sid: &[u8]) -> bool {
    current_user_sid == candidate_user_sid
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

fn architecture_for_machine(machine: u16) -> ExecutableArchitecture {
    match machine {
        0x014c => ExecutableArchitecture::X86,
        0x8664 => ExecutableArchitecture::X86_64,
        0xaa64 => ExecutableArchitecture::Arm64,
        _ => ExecutableArchitecture::Other,
    }
}

fn numeric_version_from_words(ms: u32, ls: u32) -> NumericExecutableVersion {
    NumericExecutableVersion {
        major: (ms >> 16) as u16,
        minor: ms as u16,
        build: (ls >> 16) as u16,
        revision: ls as u16,
    }
}

#[cfg(windows)]
pub(crate) fn discover_current_session() -> DiscoveryEvidence {
    match windows::discover_current_session() {
        Ok(evidence) => evidence,
        Err(failure) => DiscoveryEvidence::Failed {
            stage: failure.stage,
            process_id: failure.process_id,
            os_error: failure.os_error,
            ntstatus: failure.ntstatus,
        },
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::{c_void, OsString};
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read, Seek, SeekFrom};
    use std::mem::size_of;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::{Path, PathBuf};
    use std::ptr;

    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_FILES, FILETIME,
        HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Security::Cryptography::{
        BCryptCloseAlgorithmProvider, BCryptCreateHash, BCryptDestroyHash, BCryptFinishHash,
        BCryptGetProperty, BCryptHashData, BCryptOpenAlgorithmProvider, BCRYPT_ALG_HANDLE,
        BCRYPT_HASH_HANDLE, BCRYPT_HASH_LENGTH, BCRYPT_OBJECT_LENGTH, BCRYPT_SHA256_ALGORITHM,
    };
    use windows_sys::Win32::Security::{
        GetLengthSid, GetTokenInformation, IsValidSid, TokenUser, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, VS_FIXEDFILEINFO,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, GetProcessTimes, OpenProcess, OpenProcessToken,
        QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    use super::{
        architecture_for_machine, exact_sid_match, lower_hex, numeric_version_from_words,
        select_processes, DiscoveredTerminal, DiscoveryEvidence, DiscoveryFailure, DiscoveryStage,
        ExecutableProvenance, ExecutableVersionSource, ProcessObservation, ProcessSelection,
        ProvenanceFailure, TERMINAL_IMAGE_NAME,
    };

    const MAX_IMAGE_PATH_UTF16: usize = 32_768;
    const MAX_TOKEN_USER_BYTES: usize = 64 * 1024;
    const MIN_SID_BYTES: usize = 8;
    const HASH_BUFFER_BYTES: usize = 64 * 1024;
    const MAX_HASH_FILE_BYTES: u64 = 256 * 1024 * 1024;
    const MAX_HASH_OBJECT_BYTES: usize = 64 * 1024;
    const MAX_PE_HEADER_OFFSET: u64 = 16 * 1024 * 1024;
    const MAX_VERSION_RESOURCE_BYTES: usize = 16 * 1024 * 1024;
    const VS_FIXEDFILEINFO_SIGNATURE: u32 = 0xfeef_04bd;
    const VERSION_ROOT_SUBBLOCK: [u16; 2] = [b'\\' as u16, 0];
    const SHA256_BYTES: usize = 32;
    const FILE_SHARE_ALL: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;

    struct OwnedHandle(HANDLE);
    struct OwnedAlgorithm(BCRYPT_ALG_HANDLE);
    struct OwnedHash(BCRYPT_HASH_HANDLE);

    #[derive(Debug)]
    struct FileDigest {
        size_bytes: u64,
        sha256: String,
    }

    impl OwnedHandle {
        fn snapshot() -> Result<Self, DiscoveryFailure> {
            // SAFETY: Flags and process ID are scalar values. The returned
            // handle is validated and then closed by `OwnedHandle`.
            let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
            if handle == INVALID_HANDLE_VALUE || handle.is_null() {
                return Err(failure(DiscoveryStage::ProcessSnapshot, None, last_error()));
            }
            Ok(Self(handle))
        }

        fn process(process_id: u32) -> Result<Self, DiscoveryFailure> {
            // SAFETY: No handle is inherited and only read-only process-query
            // access is requested. The checked handle is owned exactly once.
            let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
            if handle.is_null() {
                return Err(failure(
                    DiscoveryStage::OpenProcess,
                    Some(process_id),
                    last_error(),
                ));
            }
            Ok(Self(handle))
        }

        fn token(
            process: HANDLE,
            process_id: Option<u32>,
            stage: DiscoveryStage,
        ) -> Result<Self, DiscoveryFailure> {
            let mut handle = ptr::null_mut();
            // SAFETY: `process` is a current-process pseudo handle or a live
            // query handle. TOKEN_QUERY is read-only and output is writable.
            if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut handle) } == 0 {
                return Err(failure(stage, process_id, last_error()));
            }
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                return Err(failure(stage, process_id, None));
            }
            Ok(Self(handle))
        }
    }

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: The wrapper uniquely owns one validated Win32 handle.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    impl Drop for OwnedAlgorithm {
        fn drop(&mut self) {
            // SAFETY: The wrapper owns one successful provider result.
            let _ = unsafe { BCryptCloseAlgorithmProvider(self.0, 0) };
        }
    }

    impl Drop for OwnedHash {
        fn drop(&mut self) {
            // SAFETY: The wrapper owns one successful hash-creation result.
            let _ = unsafe { BCryptDestroyHash(self.0) };
        }
    }

    pub(super) fn discover_current_session() -> Result<DiscoveryEvidence, DiscoveryFailure> {
        let current_session_id = current_session_id()?;
        match select_processes(enumerate_named_processes()?, current_session_id) {
            ProcessSelection::None => Ok(DiscoveryEvidence::NotRunning { current_session_id }),
            ProcessSelection::Multiple {
                matching_process_count,
                evidence,
            } => Ok(DiscoveryEvidence::Ambiguous {
                current_session_id,
                matching_process_count,
                process_ids_truncated: usize::try_from(matching_process_count)
                    .map_or(true, |count| count > evidence.len()),
                process_ids: evidence.into_iter().map(|entry| entry.process_id).collect(),
            }),
            ProcessSelection::One(entry) => discover_one(current_session_id, entry),
        }
    }

    fn discover_one(
        current_session_id: u32,
        entry: ProcessObservation,
    ) -> Result<DiscoveryEvidence, DiscoveryFailure> {
        let process = OwnedHandle::process(entry.process_id)?;
        let verified_session_id = process_session_id(entry.process_id)?;
        if verified_session_id != current_session_id {
            return Err(failure(
                DiscoveryStage::ProcessSession,
                Some(entry.process_id),
                None,
            ));
        }
        let creation_time = process_creation_time(&process, entry.process_id)?;
        let image_path = process_image_path(&process, entry.process_id)?;
        image_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| name.eq_ignore_ascii_case(TERMINAL_IMAGE_NAME))
            .ok_or_else(|| failure(DiscoveryStage::ImagePath, Some(entry.process_id), None))?;
        let image_path_string = unicode_path(&image_path, entry.process_id)?;
        let current_user_sid = current_user_sid()?;
        let candidate_user_sid = process_user_sid(
            process.0,
            Some(entry.process_id),
            DiscoveryStage::CandidateUserToken,
        )?;
        if !exact_sid_match(&current_user_sid, &candidate_user_sid) {
            return Ok(DiscoveryEvidence::IdentityMismatch {
                current_session_id,
                process_id: entry.process_id,
                candidate_session_id: verified_session_id,
                process_creation_time_100ns_since_1601: creation_time,
                image_path: image_path_string,
            });
        }

        Ok(DiscoveryEvidence::Discovered {
            current_session_id,
            terminal: DiscoveredTerminal {
                process_id: entry.process_id,
                session_id: verified_session_id,
                process_creation_time_100ns_since_1601: creation_time,
                image_path: image_path_string,
                image_name_verified: true,
                current_user_identity_verified: true,
                executable: collect_executable_provenance(&image_path, entry.process_id),
            },
            eligible_for_fixed_loopback_health_probe: true,
        })
    }

    fn current_session_id() -> Result<u32, DiscoveryFailure> {
        // SAFETY: This call takes no pointers and has no preconditions.
        let process_id = unsafe { GetCurrentProcessId() };
        process_session_id(process_id).map_err(|mut error| {
            error.stage = DiscoveryStage::CurrentSession;
            error
        })
    }

    fn enumerate_named_processes() -> Result<Vec<ProcessObservation>, DiscoveryFailure> {
        let snapshot = OwnedHandle::snapshot()?;
        let mut entry = PROCESSENTRY32W {
            dwSize: u32::try_from(size_of::<PROCESSENTRY32W>())
                .map_err(|_| failure(DiscoveryStage::ProcessEnumeration, None, None))?,
            ..PROCESSENTRY32W::default()
        };

        // SAFETY: `entry` has the required size and remains writable while
        // the valid snapshot is held.
        if unsafe { Process32FirstW(snapshot.0, &mut entry) } == 0 {
            let os_error = last_error();
            if os_error == Some(ERROR_NO_MORE_FILES) {
                return Ok(Vec::new());
            }
            return Err(failure(DiscoveryStage::ProcessEnumeration, None, os_error));
        }

        let mut observations = Vec::new();
        loop {
            if wide_name_matches(&entry.szExeFile, TERMINAL_IMAGE_NAME) {
                let process_id = entry.th32ProcessID;
                // A Toolhelp snapshot races process exit. A same-name process
                // that vanished before its session could be queried is not a
                // candidate; the selected current-session process is still
                // reopened and revalidated fail-closed below.
                if let Ok(session_id) = process_session_id(process_id) {
                    observations.push(ProcessObservation {
                        process_id,
                        session_id,
                        image_name: TERMINAL_IMAGE_NAME.to_owned(),
                    });
                }
            }
            // SAFETY: The initialized entry buffer and snapshot remain valid.
            if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
                let os_error = last_error();
                if os_error == Some(ERROR_NO_MORE_FILES) {
                    break;
                }
                return Err(failure(DiscoveryStage::ProcessEnumeration, None, os_error));
            }
        }
        Ok(observations)
    }

    fn process_session_id(process_id: u32) -> Result<u32, DiscoveryFailure> {
        let mut session_id = 0;
        // SAFETY: Output is a live writable u32 and process ID is scalar.
        if unsafe { ProcessIdToSessionId(process_id, &mut session_id) } == 0 {
            return Err(failure(
                DiscoveryStage::ProcessSession,
                Some(process_id),
                last_error(),
            ));
        }
        Ok(session_id)
    }

    fn process_creation_time(
        process: &OwnedHandle,
        process_id: u32,
    ) -> Result<u64, DiscoveryFailure> {
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: All four outputs are writable FILETIME values and the
        // process query handle remains live for the call.
        if unsafe { GetProcessTimes(process.0, &mut creation, &mut exit, &mut kernel, &mut user) }
            == 0
        {
            return Err(failure(
                DiscoveryStage::ProcessIdentity,
                Some(process_id),
                last_error(),
            ));
        }
        Ok((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
    }

    fn process_image_path(
        process: &OwnedHandle,
        process_id: u32,
    ) -> Result<PathBuf, DiscoveryFailure> {
        let mut buffer = vec![0_u16; MAX_IMAGE_PATH_UTF16];
        let mut length = u32::try_from(buffer.len())
            .map_err(|_| failure(DiscoveryStage::ImagePath, Some(process_id), None))?;
        // SAFETY: Buffer and length pointer are valid and bounded; the process
        // handle remains live.
        if unsafe { QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &mut length) }
            == 0
        {
            return Err(failure(
                DiscoveryStage::ImagePath,
                Some(process_id),
                last_error(),
            ));
        }
        let length = usize::try_from(length)
            .map_err(|_| failure(DiscoveryStage::ImagePath, Some(process_id), None))?;
        if length == 0 || length > buffer.len() {
            return Err(failure(DiscoveryStage::ImagePath, Some(process_id), None));
        }
        let path = PathBuf::from(OsString::from_wide(&buffer[..length]));
        if !path.is_absolute() {
            return Err(failure(
                DiscoveryStage::ImagePathNotAbsolute,
                Some(process_id),
                None,
            ));
        }
        Ok(path)
    }

    fn current_user_sid() -> Result<Vec<u8>, DiscoveryFailure> {
        // SAFETY: The pseudo handle takes no input pointer and is not closed.
        let process = unsafe { GetCurrentProcess() };
        process_user_sid(process, None, DiscoveryStage::CurrentUserToken)
    }

    fn process_user_sid(
        process: HANDLE,
        process_id: Option<u32>,
        open_stage: DiscoveryStage,
    ) -> Result<Vec<u8>, DiscoveryFailure> {
        let token = OwnedHandle::token(process, process_id, open_stage)?;
        let mut required_length = 0_u32;
        // SAFETY: This is the documented null-buffer sizing call and the size
        // output is writable.
        let sizing_result = unsafe {
            GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut required_length)
        };
        let sizing_error = last_error();
        if sizing_result != 0
            || sizing_error != Some(ERROR_INSUFFICIENT_BUFFER)
            || required_length == 0
        {
            return Err(failure(
                DiscoveryStage::TokenUserInformation,
                process_id,
                sizing_error,
            ));
        }

        let required_length = usize::try_from(required_length)
            .map_err(|_| failure(DiscoveryStage::TokenUserInformation, process_id, None))?;
        if !(size_of::<TOKEN_USER>()..=MAX_TOKEN_USER_BYTES).contains(&required_length) {
            return Err(failure(
                DiscoveryStage::TokenUserInformation,
                process_id,
                None,
            ));
        }
        let word_size = size_of::<usize>();
        let word_count = required_length
            .checked_add(word_size - 1)
            .and_then(|length| length.checked_div(word_size))
            .ok_or_else(|| failure(DiscoveryStage::TokenUserInformation, process_id, None))?;
        let mut buffer = vec![0_usize; word_count];
        let buffer_bytes = buffer
            .len()
            .checked_mul(word_size)
            .ok_or_else(|| failure(DiscoveryStage::TokenUserInformation, process_id, None))?;
        let buffer_length = u32::try_from(buffer_bytes)
            .map_err(|_| failure(DiscoveryStage::TokenUserInformation, process_id, None))?;
        let mut returned_length = 0_u32;
        // SAFETY: The usize-backed buffer is aligned for TOKEN_USER and is
        // writable for its checked byte length.
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast::<c_void>(),
                buffer_length,
                &mut returned_length,
            )
        } == 0
        {
            return Err(failure(
                DiscoveryStage::TokenUserInformation,
                process_id,
                last_error(),
            ));
        }
        let returned_length = usize::try_from(returned_length)
            .map_err(|_| failure(DiscoveryStage::TokenUserInformation, process_id, None))?;
        if returned_length < size_of::<TOKEN_USER>() || returned_length > buffer_bytes {
            return Err(failure(
                DiscoveryStage::TokenUserInformation,
                process_id,
                None,
            ));
        }

        // SAFETY: The aligned buffer contains at least TOKEN_USER bytes.
        let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        let buffer_start = buffer.as_ptr().cast::<u8>() as usize;
        let buffer_end = buffer_start
            .checked_add(returned_length)
            .ok_or_else(|| failure(DiscoveryStage::TokenUserSid, process_id, None))?;
        let sid_start = sid as usize;
        let minimum_sid_end = sid_start
            .checked_add(MIN_SID_BYTES)
            .ok_or_else(|| failure(DiscoveryStage::TokenUserSid, process_id, None))?;
        if sid.is_null() || sid_start < buffer_start || minimum_sid_end > buffer_end {
            return Err(failure(DiscoveryStage::TokenUserSid, process_id, None));
        }
        // SAFETY: SID header lies in the bounded token result.
        if unsafe { IsValidSid(sid) } == 0 {
            return Err(failure(DiscoveryStage::TokenUserSid, process_id, None));
        }
        // SAFETY: IsValidSid succeeded for this live token buffer.
        let sid_length = usize::try_from(unsafe { GetLengthSid(sid) })
            .map_err(|_| failure(DiscoveryStage::TokenUserSid, process_id, None))?;
        let sid_end = sid_start
            .checked_add(sid_length)
            .ok_or_else(|| failure(DiscoveryStage::TokenUserSid, process_id, None))?;
        if sid_length < MIN_SID_BYTES || sid_end > buffer_end {
            return Err(failure(DiscoveryStage::TokenUserSid, process_id, None));
        }
        // SAFETY: The validated SID range lies wholly in the live buffer and
        // is copied before the buffer is dropped.
        Ok(unsafe { std::slice::from_raw_parts(sid.cast::<u8>(), sid_length) }.to_vec())
    }

    fn collect_executable_provenance(path: &Path, process_id: u32) -> ExecutableProvenance {
        let (size_bytes, metadata_failure) = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_file() => (Some(metadata.len()), None),
            Ok(_) => (
                None,
                Some(failure(
                    DiscoveryStage::ExecutableNotRegularFile,
                    Some(process_id),
                    None,
                )),
            ),
            Err(error) => (
                None,
                Some(io_failure(
                    DiscoveryStage::ExecutableMetadata,
                    process_id,
                    &error,
                )),
            ),
        };

        let digest = match metadata_failure {
            None => sha256_file(path, process_id),
            Some(error) => Err(error),
        };
        let (sha256, hash_failure, digest_size) = match digest {
            Ok(digest) => (Some(digest.sha256), None, Some(digest.size_bytes)),
            Err(error) => (None, Some(ProvenanceFailure::from(error)), None),
        };
        let architecture = match metadata_failure {
            None => read_pe_machine(path, process_id),
            Some(error) => Err(error),
        };
        let (pe_machine, architecture, architecture_failure) = match architecture {
            Ok(machine) => (Some(machine), Some(architecture_for_machine(machine)), None),
            Err(error) => (None, None, Some(ProvenanceFailure::from(error))),
        };
        let fixed_version = match metadata_failure {
            None => read_fixed_version(path, process_id),
            Some(error) => Err(error),
        };
        let (file_version, product_version, version_source, version_failure) = match fixed_version {
            Ok((file_version, product_version)) => (
                Some(file_version),
                Some(product_version),
                Some(ExecutableVersionSource::VsFixedFileInfoNumeric),
                None,
            ),
            Err(error) => (None, None, None, Some(ProvenanceFailure::from(error))),
        };

        ExecutableProvenance {
            size_bytes: digest_size.or(size_bytes),
            sha256,
            hash_failure,
            pe_machine,
            architecture,
            architecture_failure,
            file_version,
            product_version,
            version_source,
            version_failure,
        }
    }

    fn shared_read(path: &Path) -> std::io::Result<File> {
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_ALL)
            .open(path)
    }

    fn read_pe_machine(path: &Path, process_id: u32) -> Result<u16, DiscoveryFailure> {
        let mut file = shared_read(path).map_err(|error| {
            io_failure(
                DiscoveryStage::ExecutableOpenForArchitecture,
                process_id,
                &error,
            )
        })?;
        let mut dos_header = [0_u8; 64];
        file.read_exact(&mut dos_header).map_err(|error| {
            io_failure(
                DiscoveryStage::ExecutableReadForArchitecture,
                process_id,
                &error,
            )
        })?;
        if &dos_header[..2] != b"MZ" {
            return Err(failure(
                DiscoveryStage::ExecutableInvalidPe,
                Some(process_id),
                None,
            ));
        }
        let pe_offset = u64::from(u32::from_le_bytes(
            dos_header[0x3c..0x40].try_into().map_err(|_| {
                failure(DiscoveryStage::ExecutableInvalidPe, Some(process_id), None)
            })?,
        ));
        if pe_offset < u64::try_from(dos_header.len()).unwrap_or(u64::MAX)
            || pe_offset > MAX_PE_HEADER_OFFSET
        {
            return Err(failure(
                DiscoveryStage::ExecutableInvalidPe,
                Some(process_id),
                None,
            ));
        }
        file.seek(SeekFrom::Start(pe_offset)).map_err(|error| {
            io_failure(
                DiscoveryStage::ExecutableReadForArchitecture,
                process_id,
                &error,
            )
        })?;
        let mut pe_header = [0_u8; 6];
        file.read_exact(&mut pe_header).map_err(|error| {
            io_failure(
                DiscoveryStage::ExecutableReadForArchitecture,
                process_id,
                &error,
            )
        })?;
        if &pe_header[..4] != b"PE\0\0" {
            return Err(failure(
                DiscoveryStage::ExecutableInvalidPe,
                Some(process_id),
                None,
            ));
        }
        Ok(u16::from_le_bytes([pe_header[4], pe_header[5]]))
    }

    fn read_fixed_version(
        path: &Path,
        process_id: u32,
    ) -> Result<
        (
            super::NumericExecutableVersion,
            super::NumericExecutableVersion,
        ),
        DiscoveryFailure,
    > {
        let wide_path = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut ignored_handle = 0_u32;
        // SAFETY: `wide_path` is a live NUL-terminated UTF-16 path and the
        // ignored handle output is writable. This reads version-resource
        // metadata; it does not load or execute the image.
        let resource_size =
            unsafe { GetFileVersionInfoSizeW(wide_path.as_ptr(), &mut ignored_handle) };
        if resource_size == 0 {
            return Err(failure(
                DiscoveryStage::ExecutableVersionResourceSize,
                Some(process_id),
                last_error(),
            ));
        }
        let resource_size = usize::try_from(resource_size).map_err(|_| {
            failure(
                DiscoveryStage::ExecutableVersionResourceTooLarge,
                Some(process_id),
                None,
            )
        })?;
        if resource_size > MAX_VERSION_RESOURCE_BYTES {
            return Err(failure(
                DiscoveryStage::ExecutableVersionResourceTooLarge,
                Some(process_id),
                None,
            ));
        }
        let mut resource = vec![0_u8; resource_size];
        // SAFETY: The path remains live and NUL-terminated. `resource` is a
        // writable buffer of the exact bounded length supplied to the API.
        if unsafe {
            GetFileVersionInfoW(
                wide_path.as_ptr(),
                0,
                u32::try_from(resource.len()).map_err(|_| {
                    failure(
                        DiscoveryStage::ExecutableVersionResourceTooLarge,
                        Some(process_id),
                        None,
                    )
                })?,
                resource.as_mut_ptr().cast::<c_void>(),
            )
        } == 0
        {
            return Err(failure(
                DiscoveryStage::ExecutableVersionResourceRead,
                Some(process_id),
                last_error(),
            ));
        }

        let mut fixed = ptr::null_mut::<c_void>();
        let mut fixed_length = 0_u32;
        // SAFETY: `resource` contains a successful version-info result, the
        // root subblock is static NUL-terminated UTF-16, and both outputs are
        // writable. The returned pointer is validated before copying.
        if unsafe {
            VerQueryValueW(
                resource.as_ptr().cast::<c_void>(),
                VERSION_ROOT_SUBBLOCK.as_ptr(),
                &mut fixed,
                &mut fixed_length,
            )
        } == 0
        {
            return Err(failure(
                DiscoveryStage::ExecutableVersionFixedQuery,
                Some(process_id),
                last_error(),
            ));
        }

        let fixed_size = size_of::<VS_FIXEDFILEINFO>();
        let resource_start = resource.as_ptr() as usize;
        let resource_end = resource_start.checked_add(resource.len()).ok_or_else(|| {
            failure(
                DiscoveryStage::ExecutableVersionFixedInvalid,
                Some(process_id),
                None,
            )
        })?;
        let fixed_start = fixed as usize;
        let fixed_end = fixed_start.checked_add(fixed_size).ok_or_else(|| {
            failure(
                DiscoveryStage::ExecutableVersionFixedInvalid,
                Some(process_id),
                None,
            )
        })?;
        if fixed.is_null()
            || usize::try_from(fixed_length)
                .ok()
                .is_none_or(|length| length < fixed_size)
            || fixed_start < resource_start
            || fixed_end > resource_end
        {
            return Err(failure(
                DiscoveryStage::ExecutableVersionFixedInvalid,
                Some(process_id),
                None,
            ));
        }
        // SAFETY: The complete fixed structure range was verified to be in
        // the live resource buffer. `read_unaligned` avoids assuming that the
        // variable resource payload has Rust alignment.
        let fixed = unsafe { ptr::read_unaligned(fixed.cast::<VS_FIXEDFILEINFO>()) };
        if fixed.dwSignature != VS_FIXEDFILEINFO_SIGNATURE {
            return Err(failure(
                DiscoveryStage::ExecutableVersionFixedInvalid,
                Some(process_id),
                None,
            ));
        }
        Ok((
            numeric_version_from_words(fixed.dwFileVersionMS, fixed.dwFileVersionLS),
            numeric_version_from_words(fixed.dwProductVersionMS, fixed.dwProductVersionLS),
        ))
    }

    fn sha256_file(path: &Path, process_id: u32) -> Result<FileDigest, DiscoveryFailure> {
        sha256_file_with_limit(path, process_id, MAX_HASH_FILE_BYTES)
    }

    fn sha256_file_with_limit(
        path: &Path,
        process_id: u32,
        maximum_bytes: u64,
    ) -> Result<FileDigest, DiscoveryFailure> {
        let mut file = shared_read(path).map_err(|error| {
            io_failure(DiscoveryStage::ExecutableOpenForHash, process_id, &error)
        })?;
        let initial_size = file
            .metadata()
            .map_err(|error| io_failure(DiscoveryStage::ExecutableMetadata, process_id, &error))?
            .len();
        if initial_size > maximum_bytes {
            return Err(failure(
                DiscoveryStage::ExecutableTooLargeForHash,
                Some(process_id),
                None,
            ));
        }

        let mut algorithm = ptr::null_mut();
        // SAFETY: Output is writable, the algorithm identifier is static and
        // NUL-terminated, and the default implementation is requested.
        let status = unsafe {
            BCryptOpenAlgorithmProvider(&mut algorithm, BCRYPT_SHA256_ALGORITHM, ptr::null(), 0)
        };
        if !nt_success(status) {
            return Err(crypto_failure(
                DiscoveryStage::HashAlgorithmProvider,
                process_id,
                status,
            ));
        }
        if algorithm.is_null() {
            return Err(failure(
                DiscoveryStage::HashAlgorithmProvider,
                Some(process_id),
                None,
            ));
        }
        let algorithm = OwnedAlgorithm(algorithm);
        let object_length = usize::try_from(bcrypt_u32_property(
            algorithm.0,
            BCRYPT_OBJECT_LENGTH,
            process_id,
        )?)
        .map_err(|_| failure(DiscoveryStage::HashProperty, Some(process_id), None))?;
        if !(1..=MAX_HASH_OBJECT_BYTES).contains(&object_length) {
            return Err(failure(
                DiscoveryStage::HashProperty,
                Some(process_id),
                None,
            ));
        }
        let hash_length = bcrypt_u32_property(algorithm.0, BCRYPT_HASH_LENGTH, process_id)?;
        if usize::try_from(hash_length).ok() != Some(SHA256_BYTES) {
            return Err(failure(
                DiscoveryStage::HashProperty,
                Some(process_id),
                None,
            ));
        }

        let mut hash_object = vec![0_u8; object_length];
        let mut hash = ptr::null_mut();
        // SAFETY: The provider is live, output is writable, and the bounded
        // object buffer outlives the hash handle. No key is provided.
        let status = unsafe {
            BCryptCreateHash(
                algorithm.0,
                &mut hash,
                hash_object.as_mut_ptr(),
                u32::try_from(hash_object.len())
                    .map_err(|_| failure(DiscoveryStage::HashCreate, Some(process_id), None))?,
                ptr::null(),
                0,
                0,
            )
        };
        if !nt_success(status) {
            return Err(crypto_failure(
                DiscoveryStage::HashCreate,
                process_id,
                status,
            ));
        }
        if hash.is_null() {
            return Err(failure(DiscoveryStage::HashCreate, Some(process_id), None));
        }
        let hash = OwnedHash(hash);
        let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
        let mut total_bytes = 0_u64;
        loop {
            let bytes_read = file.read(&mut buffer).map_err(|error| {
                io_failure(DiscoveryStage::ExecutableReadForHash, process_id, &error)
            })?;
            if bytes_read == 0 {
                break;
            }
            total_bytes = total_bytes
                .checked_add(u64::try_from(bytes_read).map_err(|_| {
                    failure(
                        DiscoveryStage::ExecutableTooLargeForHash,
                        Some(process_id),
                        None,
                    )
                })?)
                .ok_or_else(|| {
                    failure(
                        DiscoveryStage::ExecutableTooLargeForHash,
                        Some(process_id),
                        None,
                    )
                })?;
            if total_bytes > maximum_bytes {
                return Err(failure(
                    DiscoveryStage::ExecutableTooLargeForHash,
                    Some(process_id),
                    None,
                ));
            }
            // SAFETY: Hash is live and input is valid for checked bytes_read.
            let status = unsafe {
                BCryptHashData(
                    hash.0,
                    buffer.as_ptr(),
                    u32::try_from(bytes_read)
                        .map_err(|_| failure(DiscoveryStage::HashUpdate, Some(process_id), None))?,
                    0,
                )
            };
            if !nt_success(status) {
                return Err(crypto_failure(
                    DiscoveryStage::HashUpdate,
                    process_id,
                    status,
                ));
            }
        }
        let final_size = file
            .metadata()
            .map_err(|error| io_failure(DiscoveryStage::ExecutableMetadata, process_id, &error))?
            .len();
        if initial_size != total_bytes || final_size != total_bytes {
            return Err(failure(
                DiscoveryStage::ExecutableChangedDuringHash,
                Some(process_id),
                None,
            ));
        }

        let mut digest = [0_u8; SHA256_BYTES];
        // SAFETY: Hash is live and digest has the provider-verified length.
        let status = unsafe {
            BCryptFinishHash(
                hash.0,
                digest.as_mut_ptr(),
                u32::try_from(digest.len())
                    .map_err(|_| failure(DiscoveryStage::HashFinish, Some(process_id), None))?,
                0,
            )
        };
        if !nt_success(status) {
            return Err(crypto_failure(
                DiscoveryStage::HashFinish,
                process_id,
                status,
            ));
        }
        Ok(FileDigest {
            size_bytes: total_bytes,
            sha256: lower_hex(&digest),
        })
    }

    fn bcrypt_u32_property(
        handle: BCRYPT_ALG_HANDLE,
        property: windows_sys::core::PCWSTR,
        process_id: u32,
    ) -> Result<u32, DiscoveryFailure> {
        let mut value = 0_u32;
        let mut returned = 0_u32;
        // SAFETY: Provider is live and value is a correctly sized output.
        let status = unsafe {
            BCryptGetProperty(
                handle,
                property,
                (&mut value as *mut u32).cast::<u8>(),
                u32::try_from(size_of::<u32>())
                    .map_err(|_| failure(DiscoveryStage::HashProperty, Some(process_id), None))?,
                &mut returned,
                0,
            )
        };
        if !nt_success(status) {
            return Err(crypto_failure(
                DiscoveryStage::HashProperty,
                process_id,
                status,
            ));
        }
        if usize::try_from(returned).ok() != Some(size_of::<u32>()) {
            return Err(failure(
                DiscoveryStage::HashProperty,
                Some(process_id),
                None,
            ));
        }
        Ok(value)
    }

    fn nt_success(status: i32) -> bool {
        status >= 0
    }

    fn unicode_path(path: &Path, process_id: u32) -> Result<String, DiscoveryFailure> {
        path.to_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| failure(DiscoveryStage::ImagePathNotUnicode, Some(process_id), None))
    }

    fn wide_name_matches(value: &[u16], expected: &str) -> bool {
        let end = value
            .iter()
            .position(|code_unit| *code_unit == 0)
            .unwrap_or(value.len());
        let value = &value[..end];
        value.len() == expected.len()
            && value.iter().zip(expected.bytes()).all(|(left, right)| {
                u8::try_from(*left).is_ok_and(|left| left.eq_ignore_ascii_case(&right))
            })
    }

    fn last_error() -> Option<u32> {
        // SAFETY: No pointers; called immediately after the failed Win32 API.
        let error = unsafe { GetLastError() };
        (error != 0).then_some(error)
    }

    fn io_failure(
        stage: DiscoveryStage,
        process_id: u32,
        error: &std::io::Error,
    ) -> DiscoveryFailure {
        failure(
            stage,
            Some(process_id),
            error
                .raw_os_error()
                .and_then(|code| u32::try_from(code).ok()),
        )
    }

    fn failure(
        stage: DiscoveryStage,
        process_id: Option<u32>,
        os_error: Option<u32>,
    ) -> DiscoveryFailure {
        DiscoveryFailure {
            stage,
            process_id,
            os_error,
            ntstatus: None,
        }
    }

    fn crypto_failure(stage: DiscoveryStage, process_id: u32, ntstatus: i32) -> DiscoveryFailure {
        DiscoveryFailure {
            stage,
            process_id: Some(process_id),
            os_error: None,
            ntstatus: Some(ntstatus),
        }
    }

    #[cfg(test)]
    mod tests {
        use std::sync::atomic::{AtomicU64, Ordering};

        use super::*;

        static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

        struct TempFile(PathBuf);

        impl TempFile {
            fn with_bytes(bytes: &[u8]) -> Self {
                let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "magic-tdx-native-discovery-{}-{sequence}.tmp",
                    std::process::id()
                ));
                fs::write(&path, bytes).unwrap();
                Self(path)
            }
        }

        impl Drop for TempFile {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.0);
            }
        }

        fn synthetic_pe(machine: u16) -> Vec<u8> {
            let mut bytes = vec![0_u8; 134];
            bytes[..2].copy_from_slice(b"MZ");
            bytes[0x3c..0x40].copy_from_slice(&128_u32.to_le_bytes());
            bytes[128..132].copy_from_slice(b"PE\0\0");
            bytes[132..134].copy_from_slice(&machine.to_le_bytes());
            bytes
        }

        #[test]
        fn bcrypt_sha256_streams_known_vector() {
            let file = TempFile::with_bytes(b"abc");
            let digest = sha256_file_with_limit(&file.0, std::process::id(), 3).unwrap();
            assert_eq!(digest.size_bytes, 3);
            assert_eq!(
                digest.sha256,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            );
        }

        #[test]
        fn file_size_limit_is_typed_provenance_failure() {
            let file = TempFile::with_bytes(b"abc");
            let failure = sha256_file_with_limit(&file.0, std::process::id(), 2).unwrap_err();
            assert_eq!(failure.stage, DiscoveryStage::ExecutableTooLargeForHash);
            assert_eq!(failure.ntstatus, None);
        }

        #[test]
        fn pe_machine_is_read_from_bounded_headers() {
            let file = TempFile::with_bytes(&synthetic_pe(0x8664));
            assert_eq!(
                read_pe_machine(&file.0, std::process::id()).unwrap(),
                0x8664
            );
        }

        #[test]
        fn missing_provenance_never_becomes_a_discovery_failure() {
            let missing = std::env::temp_dir().join("magic-tdx-native-definitely-missing.exe");
            let evidence = collect_executable_provenance(&missing, std::process::id());
            assert!(evidence.sha256.is_none());
            assert!(evidence.hash_failure.is_some());
            assert!(evidence.architecture.is_none());
            assert!(evidence.architecture_failure.is_some());
            assert!(evidence.file_version.is_none());
            assert!(evidence.product_version.is_none());
            assert!(evidence.version_source.is_none());
            assert!(evidence.version_failure.is_some());
        }

        #[test]
        fn missing_version_resource_is_typed_optional_provenance() {
            let file = TempFile::with_bytes(&synthetic_pe(0x8664));
            let evidence = collect_executable_provenance(&file.0, std::process::id());
            assert_eq!(evidence.pe_machine, Some(0x8664));
            assert!(evidence.file_version.is_none());
            assert!(evidence.product_version.is_none());
            assert!(evidence.version_source.is_none());
            assert_eq!(
                evidence.version_failure.map(|failure| failure.stage),
                Some(DiscoveryStage::ExecutableVersionResourceSize)
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(process_id: u32, session_id: u32, image_name: &str) -> ProcessObservation {
        ProcessObservation {
            process_id,
            session_id,
            image_name: image_name.to_owned(),
        }
    }

    #[test]
    fn selection_reports_zero_exact_matches() {
        assert_eq!(
            select_processes([process(1, 7, "Other.exe")], 7),
            ProcessSelection::None
        );
    }

    #[test]
    fn selection_accepts_one_exact_case_insensitive_windows_name() {
        assert_eq!(
            select_processes([process(9, 7, "tdxw.EXE")], 7),
            ProcessSelection::One(process(9, 7, "tdxw.EXE"))
        );
    }

    #[test]
    fn selection_rejects_similar_names_and_other_sessions() {
        assert_eq!(
            select_processes(
                [
                    process(1, 7, "TdxW.exe.bak"),
                    process(2, 8, "TdxW.exe"),
                    process(3, 7, "TdxW"),
                ],
                7,
            ),
            ProcessSelection::None
        );
    }

    #[test]
    fn selection_reports_multiple_matches_in_pid_order() {
        assert_eq!(
            select_processes([process(12, 7, "TdxW.exe"), process(4, 7, "tdxw.exe")], 7),
            ProcessSelection::Multiple {
                matching_process_count: 2,
                evidence: vec![process(4, 7, "tdxw.exe"), process(12, 7, "TdxW.exe")],
            }
        );
    }

    #[test]
    fn selection_caps_ambiguous_evidence_while_preserving_total_count() {
        let entries = (0_u32..32)
            .rev()
            .map(|process_id| process(process_id, 7, "TdxW.exe"));
        let ProcessSelection::Multiple {
            matching_process_count,
            evidence,
        } = select_processes(entries, 7)
        else {
            panic!("expected multiple terminal processes");
        };
        assert_eq!(matching_process_count, 32);
        assert_eq!(evidence.len(), MAX_AMBIGUOUS_PROCESS_IDS);
        assert_eq!(
            evidence
                .iter()
                .map(|entry| entry.process_id)
                .collect::<Vec<_>>(),
            (0_u32..16).collect::<Vec<_>>()
        );
    }

    #[test]
    fn sid_comparison_requires_exact_bytes() {
        assert!(exact_sid_match(&[1, 2, 3], &[1, 2, 3]));
        assert!(!exact_sid_match(&[1, 2, 3], &[1, 2, 4]));
        assert!(!exact_sid_match(&[1, 2, 3], &[1, 2, 3, 0]));
    }

    #[test]
    fn architecture_mapping_is_explicit() {
        assert_eq!(
            architecture_for_machine(0x014c),
            ExecutableArchitecture::X86
        );
        assert_eq!(
            architecture_for_machine(0x8664),
            ExecutableArchitecture::X86_64
        );
        assert_eq!(
            architecture_for_machine(0xaa64),
            ExecutableArchitecture::Arm64
        );
        assert_eq!(architecture_for_machine(0), ExecutableArchitecture::Other);
    }

    #[test]
    fn numeric_version_tuple_uses_high_and_low_words() {
        assert_eq!(
            numeric_version_from_words(0x0001_0002, 0x0003_0004),
            NumericExecutableVersion {
                major: 1,
                minor: 2,
                build: 3,
                revision: 4,
            }
        );
    }

    #[test]
    fn lower_hex_preserves_leading_zeroes() {
        assert_eq!(lower_hex(&[0x00, 0x0f, 0x10, 0xff]), "000f10ff");
    }
}
