//! Versioned, length-prefixed JSON protocol shared with the native bridge.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use thiserror::Error;

/// Protocol version implemented by this crate.
pub const PROTOCOL_VERSION: u16 = 1;

/// JSON schema version implemented by this crate.
pub const SCHEMA_VERSION: u16 = 1;

/// A validated codec for four-byte big-endian length-prefixed JSON frames.
///
/// The caller supplies the frame bound from its approved deployment policy.
/// This crate intentionally does not invent a production throughput limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameCodec {
    max_payload_len: usize,
}

impl FrameCodec {
    /// Creates a codec with a positive maximum payload length representable by
    /// the four-byte wire prefix.
    pub fn new(max_payload_len: usize) -> Result<Self, ProtocolError> {
        if max_payload_len == 0 || max_payload_len > u32::MAX as usize {
            return Err(ProtocolError::InvalidFrameLimit {
                requested: max_payload_len,
                maximum: u32::MAX as usize,
            });
        }
        Ok(Self { max_payload_len })
    }

    /// Returns the exact configured payload bound in bytes.
    pub fn max_payload_len(self) -> usize {
        self.max_payload_len
    }

    /// Serializes and writes one complete frame.
    pub fn write_json<T: Serialize>(
        self,
        writer: &mut impl Write,
        value: &T,
    ) -> Result<(), ProtocolError> {
        let mut payload = BoundedPayloadWriter::new(self.max_payload_len);
        if let Err(source) = serde_json::to_writer(&mut payload, value) {
            return if payload.overflowed {
                Err(ProtocolError::EncodedFrameTooLarge {
                    maximum: self.max_payload_len,
                })
            } else {
                Err(ProtocolError::EncodeJson(source))
            };
        }
        self.write_payload(writer, &payload.bytes)
    }

    /// Reads and deserializes one complete frame.
    pub fn read_json<T: DeserializeOwned>(
        self,
        reader: &mut impl Read,
    ) -> Result<T, ProtocolError> {
        let payload = self.read_payload(reader)?;
        let text = String::from_utf8(payload).map_err(ProtocolError::InvalidUtf8)?;
        serde_json::from_str(&text).map_err(ProtocolError::DecodeJson)
    }

    /// Validates and writes one bridge protocol message.
    pub fn write_message(
        self,
        writer: &mut impl Write,
        message: &BridgeMessage,
    ) -> Result<(), ProtocolError> {
        message.validate()?;
        self.write_json(writer, message)
    }

    /// Reads, deserializes and validates one bridge protocol message.
    pub fn read_message(self, reader: &mut impl Read) -> Result<BridgeMessage, ProtocolError> {
        let message: BridgeMessage = self.read_json(reader)?;
        message.validate()?;
        Ok(message)
    }

    /// Validates and writes one bridge control command.
    pub fn write_command(
        self,
        writer: &mut impl Write,
        command: &BridgeCommand,
    ) -> Result<(), ProtocolError> {
        command.validate()?;
        self.write_json(writer, command)
    }

    /// Reads, deserializes and validates one bridge control command.
    pub fn read_command(self, reader: &mut impl Read) -> Result<BridgeCommand, ProtocolError> {
        let command: BridgeCommand = self.read_json(reader)?;
        command.validate()?;
        Ok(command)
    }

    fn write_payload(self, writer: &mut impl Write, payload: &[u8]) -> Result<(), ProtocolError> {
        if payload.is_empty() {
            return Err(ProtocolError::EmptyFrame);
        }
        if payload.len() > self.max_payload_len {
            return Err(ProtocolError::FrameTooLarge {
                announced: payload.len(),
                maximum: self.max_payload_len,
            });
        }
        let wire_len = u32::try_from(payload.len()).map_err(|_| ProtocolError::FrameTooLarge {
            announced: payload.len(),
            maximum: self.max_payload_len,
        })?;
        writer
            .write_all(&wire_len.to_be_bytes())
            .map_err(|source| ProtocolError::Io {
                operation: FrameIoOperation::WritePrefix,
                source,
            })?;
        writer
            .write_all(payload)
            .map_err(|source| ProtocolError::Io {
                operation: FrameIoOperation::WritePayload,
                source,
            })?;
        writer.flush().map_err(|source| ProtocolError::Io {
            operation: FrameIoOperation::Flush,
            source,
        })
    }

    fn read_payload(self, reader: &mut impl Read) -> Result<Vec<u8>, ProtocolError> {
        let mut prefix = [0_u8; 4];
        reader
            .read_exact(&mut prefix)
            .map_err(|source| ProtocolError::Io {
                operation: FrameIoOperation::ReadPrefix,
                source,
            })?;
        let announced = u32::from_be_bytes(prefix) as usize;
        if announced == 0 {
            return Err(ProtocolError::EmptyFrame);
        }
        if announced > self.max_payload_len {
            return Err(ProtocolError::FrameTooLarge {
                announced,
                maximum: self.max_payload_len,
            });
        }
        let mut payload = vec![0_u8; announced];
        reader
            .read_exact(&mut payload)
            .map_err(|source| ProtocolError::Io {
                operation: FrameIoOperation::ReadPayload,
                source,
            })?;
        Ok(payload)
    }
}

/// The exact framed message variants supported in the initial protocol.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum BridgeMessage {
    /// First message emitted by a newly started long-lived bridge.
    Hello(Box<Hello>),
    /// Source lifecycle state emitted after the hello handshake.
    Status(BridgeStatus),
    /// One source observation. Presence on the wire is not admission.
    Observation(Box<SourceObservation>),
    /// A typed bridge failure that can be acted on without parsing text.
    Error(BridgeErrorReport),
    /// Final acknowledgement after a graceful shutdown command.
    Stopped(Stopped),
}

impl BridgeMessage {
    /// Validates semantic constraints that Serde alone cannot express.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Hello(hello) => hello.validate(),
            Self::Status(status) => status.validate(),
            Self::Observation(observation) => observation.validate(),
            Self::Error(error) => error.validate(),
            Self::Stopped(stopped) => stopped.validate(),
        }
    }

    /// Returns the bridge-local sequence when this message participates in
    /// ordered runtime delivery. Hello establishes identity and is unsequenced.
    pub const fn bridge_sequence(&self) -> Option<u64> {
        match self {
            Self::Hello(_) => None,
            Self::Status(status) => Some(status.bridge_sequence),
            Self::Observation(observation) => Some(observation.bridge_sequence),
            Self::Error(error) => Some(error.bridge_sequence),
            Self::Stopped(stopped) => Some(stopped.bridge_sequence),
        }
    }
}

/// Parent-to-bridge commands supported by the first protocol slice.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum BridgeCommand {
    Shutdown(Shutdown),
}

impl BridgeCommand {
    /// Validates command version negotiation.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Shutdown(shutdown) => shutdown.validate(),
        }
    }
}

/// Graceful shutdown request sent over the existing stdio session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Shutdown {
    pub protocol_version: u16,
    pub schema_version: u16,
}

impl Shutdown {
    pub fn current() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
        }
    }

    pub fn validate(self) -> Result<(), ProtocolError> {
        validate_versions(self.protocol_version, self.schema_version)
    }
}

/// Graceful shutdown acknowledgement emitted before bridge exit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stopped {
    pub protocol_version: u16,
    pub schema_version: u16,
    pub bridge_sequence: u64,
}

impl Stopped {
    pub fn current(bridge_sequence: u64) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
            bridge_sequence,
        }
    }

    pub fn validate(self) -> Result<(), ProtocolError> {
        validate_versions(self.protocol_version, self.schema_version)?;
        validate_bridge_sequence(self.bridge_sequence)
    }
}

/// Bridge-local source lifecycle state. `detail` is diagnostic only; callers
/// use `state` and `reason` for decisions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeStatus {
    pub protocol_version: u16,
    pub schema_version: u16,
    pub bridge_sequence: u64,
    pub state: BridgeRuntimeState,
    pub reason: BridgeStatusReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl BridgeStatus {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_versions(self.protocol_version, self.schema_version)?;
        validate_bridge_sequence(self.bridge_sequence)?;
        if let Some(detail) = &self.detail {
            validate_message_text("status.detail", detail)?;
        }
        Ok(())
    }
}

/// Runtime lifecycle state reported by the isolated bridge process.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeRuntimeState {
    Starting,
    Ready,
    Degraded,
    Stopping,
}

/// Machine-readable reason accompanying a bridge lifecycle state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeStatusReason {
    ProcessStarted,
    SourceReady,
    SourceUnavailable,
    Reconnecting,
    ShutdownRequested,
}

/// Typed runtime failure emitted by the bridge. This never promotes a source
/// capability or repository admission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeErrorReport {
    pub protocol_version: u16,
    pub schema_version: u16,
    pub bridge_sequence: u64,
    pub code: BridgeErrorCode,
    pub retryable: bool,
    pub message: String,
}

impl BridgeErrorReport {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_versions(self.protocol_version, self.schema_version)?;
        validate_bridge_sequence(self.bridge_sequence)?;
        validate_message_text("error.message", &self.message)
    }
}

/// Machine-readable bridge error classification.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeErrorCode {
    TerminalUnavailable,
    CapabilityUnavailable,
    EntitlementUnavailable,
    SourceReadFailed,
    ProtocolViolation,
    Internal,
}

/// One observation from the local terminal. Decimal values remain exact text
/// on the wire, and each field carries an explicit unit. A decoded observation
/// is still unusable until its independent repository and runtime gates pass.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceObservation {
    pub protocol_version: u16,
    pub schema_version: u16,
    pub bridge_sequence: u64,
    pub instrument: SourceInstrument,
    pub observed_at_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<DecimalObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative_amount: Option<DecimalObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative_volume: Option<DecimalObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_record_count: Option<u64>,
}

impl SourceObservation {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_versions(self.protocol_version, self.schema_version)?;
        validate_bridge_sequence(self.bridge_sequence)?;
        self.instrument.validate()?;
        validate_message_text("observation.observed_at_utc", &self.observed_at_utc)?;
        if let Some(source_timestamp) = &self.source_timestamp {
            validate_message_text("observation.source_timestamp", source_timestamp)?;
        }
        if self.price.is_none()
            && self.cumulative_amount.is_none()
            && self.cumulative_volume.is_none()
            && self.source_record_count.is_none()
        {
            return Err(ProtocolError::InvalidMessageField {
                field: "observation",
                reason: "must carry at least one source field",
            });
        }
        validate_decimal_field(
            "observation.price",
            &self.price,
            ObservationUnit::CnyPerShare,
        )?;
        validate_decimal_field(
            "observation.cumulative_amount",
            &self.cumulative_amount,
            ObservationUnit::Cny,
        )?;
        validate_decimal_field(
            "observation.cumulative_volume",
            &self.cumulative_volume,
            ObservationUnit::Lot,
        )
    }
}

/// Exact exchange and code carried by one observation. No prefix heuristic is
/// applied by this protocol layer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInstrument {
    pub exchange: SourceExchange,
    pub code: String,
}

impl SourceInstrument {
    fn validate(&self) -> Result<(), ProtocolError> {
        if self.code.len() != 6 || !self.code.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ProtocolError::InvalidMessageField {
                field: "observation.instrument.code",
                reason: "must be exactly six ASCII digits",
            });
        }
        Ok(())
    }
}

/// Explicit exchange identity supplied by the bridge.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceExchange {
    Shanghai,
    Shenzhen,
    Beijing,
}

/// Exact non-negative decimal text with an explicit source unit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecimalObservation {
    pub value: String,
    pub unit: ObservationUnit,
}

impl DecimalObservation {
    pub(crate) fn validate(&self, field: &'static str) -> Result<(), ProtocolError> {
        if !is_non_negative_decimal(&self.value) {
            return Err(ProtocolError::InvalidMessageField {
                field,
                reason: "must be canonical non-negative decimal text",
            });
        }
        Ok(())
    }
}

/// Units supported by the initial local-terminal observation schema.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationUnit {
    CnyPerShare,
    Cny,
    /// A-share trading lots (手). One lot is normally 100 shares, but the
    /// protocol retains the source unit instead of applying an instrument-wide
    /// conversion assumption.
    Lot,
}

/// Stateful validation of the bridge-local sequence within one supervisor
/// generation. It intentionally makes no claim about source completeness.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BridgeSequenceTracker {
    last: Option<u64>,
}

impl BridgeSequenceTracker {
    pub const fn new() -> Self {
        Self { last: None }
    }

    pub const fn last(self) -> Option<u64> {
        self.last
    }

    pub fn observe(&mut self, message: &BridgeMessage) -> Result<(), ProtocolError> {
        message.validate()?;
        let Some(actual) = message.bridge_sequence() else {
            return Ok(());
        };
        match self.last {
            None => {
                if actual != 1 {
                    return Err(ProtocolError::BridgeSequenceGap {
                        expected: 1,
                        actual,
                    });
                }
                self.last = Some(actual);
                Ok(())
            }
            Some(previous) => {
                let expected = previous
                    .checked_add(1)
                    .ok_or(ProtocolError::BridgeSequenceExhausted { previous })?;
                if actual != expected {
                    return Err(if actual <= previous {
                        ProtocolError::BridgeSequenceRollback { previous, actual }
                    } else {
                        ProtocolError::BridgeSequenceGap { expected, actual }
                    });
                }
                self.last = Some(actual);
                Ok(())
            }
        }
    }
}

/// First-frame identity and compatibility evidence from a supervised test or
/// transport peer. The production TQ-Local client calls HTTP directly and does
/// not require this handshake.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub protocol_version: u16,
    pub schema_version: u16,
    pub peer: ArtifactIdentity,
    pub peer_architecture: String,
    pub terminal: ArtifactIdentity,
    pub transport_profile_id: String,
    pub terminal_state: TerminalState,
    pub capabilities: BTreeMap<String, bool>,
    pub entitlements: BTreeMap<String, bool>,
}

impl Hello {
    /// Validates versions and exact artifact identity evidence.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_versions(self.protocol_version, self.schema_version)?;
        validate_text("peer_architecture", &self.peer_architecture)?;
        validate_text("transport_profile_id", &self.transport_profile_id)?;
        self.peer.validate("peer")?;
        self.terminal.validate("terminal")?;
        validate_fact_map("capabilities", &self.capabilities)?;
        validate_fact_map("entitlements", &self.entitlements)
    }
}

/// Version and digest identity of one executable peer or terminal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIdentity {
    pub filename: String,
    pub product_version: String,
    pub file_version: String,
    pub sha256: String,
}

impl ArtifactIdentity {
    fn validate(&self, field: &'static str) -> Result<(), ProtocolError> {
        validate_text(field, &self.filename)?;
        validate_text(field, &self.product_version)?;
        validate_text(field, &self.file_version)?;
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ProtocolError::InvalidHelloField {
                field,
                reason: "SHA-256 must be exactly 64 lowercase hexadecimal characters",
            });
        }
        Ok(())
    }
}

/// Source-reported terminal readiness. This is evidence, not capability
/// admission by itself.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalState {
    Ready,
    NotLoggedIn,
    Unavailable,
}

/// Exact operation that failed while reading or writing a frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameIoOperation {
    ReadPrefix,
    ReadPayload,
    WritePrefix,
    WritePayload,
    Flush,
}

impl std::fmt::Display for FrameIoOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::ReadPrefix => "read frame prefix",
            Self::ReadPayload => "read frame payload",
            Self::WritePrefix => "write frame prefix",
            Self::WritePayload => "write frame payload",
            Self::Flush => "flush frame",
        };
        formatter.write_str(label)
    }
}

/// Typed local IPC failures. Display text is diagnostic only and is never a
/// machine-readable recovery contract.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("frame limit {requested} is invalid; expected 1..={maximum}")]
    InvalidFrameLimit { requested: usize, maximum: usize },
    #[error("empty bridge frame is not a JSON document")]
    EmptyFrame,
    #[error("bridge frame length {announced} exceeds configured maximum {maximum}")]
    FrameTooLarge { announced: usize, maximum: usize },
    #[error("encoded bridge frame exceeds configured maximum {maximum}")]
    EncodedFrameTooLarge { maximum: usize },
    #[error("failed to {operation}: {source}")]
    Io {
        operation: FrameIoOperation,
        #[source]
        source: io::Error,
    },
    #[error("bridge frame is not UTF-8: {0}")]
    InvalidUtf8(#[source] std::string::FromUtf8Error),
    #[error("unable to encode bridge JSON: {0}")]
    EncodeJson(#[source] serde_json::Error),
    #[error("unable to decode bridge JSON: {0}")]
    DecodeJson(#[source] serde_json::Error),
    #[error("unsupported bridge protocol version {actual}; expected {expected}")]
    UnsupportedProtocolVersion { expected: u16, actual: u16 },
    #[error("unsupported bridge schema version {actual}; expected {expected}")]
    UnsupportedSchemaVersion { expected: u16, actual: u16 },
    #[error("invalid hello field {field}: {reason}")]
    InvalidHelloField {
        field: &'static str,
        reason: &'static str,
    },
    #[error("invalid bridge message field {field}: {reason}")]
    InvalidMessageField {
        field: &'static str,
        reason: &'static str,
    },
    #[error("bridge sequence must be greater than zero")]
    InvalidBridgeSequence,
    #[error("bridge sequence gap: expected {expected}, received {actual}")]
    BridgeSequenceGap { expected: u64, actual: u64 },
    #[error("bridge sequence rolled back from {previous} to {actual}")]
    BridgeSequenceRollback { previous: u64, actual: u64 },
    #[error("bridge sequence exhausted after {previous}")]
    BridgeSequenceExhausted { previous: u64 },
}

struct BoundedPayloadWriter {
    bytes: Vec<u8>,
    maximum: usize,
    overflowed: bool,
}

impl BoundedPayloadWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            overflowed: false,
        }
    }
}

impl Write for BoundedPayloadWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let remaining = self.maximum.saturating_sub(self.bytes.len());
        if input.len() > remaining {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "encoded bridge frame exceeds configured maximum",
            ));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn validate_text(field: &'static str, value: &str) -> Result<(), ProtocolError> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(ProtocolError::InvalidHelloField {
            field,
            reason: "must be non-empty, unpadded text without control characters",
        });
    }
    Ok(())
}

fn validate_versions(protocol_version: u16, schema_version: u16) -> Result<(), ProtocolError> {
    if protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedProtocolVersion {
            expected: PROTOCOL_VERSION,
            actual: protocol_version,
        });
    }
    if schema_version != SCHEMA_VERSION {
        return Err(ProtocolError::UnsupportedSchemaVersion {
            expected: SCHEMA_VERSION,
            actual: schema_version,
        });
    }
    Ok(())
}

fn validate_bridge_sequence(sequence: u64) -> Result<(), ProtocolError> {
    if sequence == 0 {
        Err(ProtocolError::InvalidBridgeSequence)
    } else {
        Ok(())
    }
}

fn validate_message_text(field: &'static str, value: &str) -> Result<(), ProtocolError> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(ProtocolError::InvalidMessageField {
            field,
            reason: "must be non-empty, unpadded text without control characters",
        });
    }
    Ok(())
}

fn validate_decimal_field(
    field: &'static str,
    value: &Option<DecimalObservation>,
    expected_unit: ObservationUnit,
) -> Result<(), ProtocolError> {
    let Some(value) = value else {
        return Ok(());
    };
    value.validate(field)?;
    if value.unit != expected_unit {
        return Err(ProtocolError::InvalidMessageField {
            field,
            reason: "unit does not match the observation field",
        });
    }
    Ok(())
}

fn is_non_negative_decimal(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let mut parts = value.split('.');
    let Some(integer) = parts.next() else {
        return false;
    };
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
    {
        return false;
    }
    if integer.len() > 1 && integer.starts_with('0') {
        return false;
    }
    match fraction {
        Some(fraction) => {
            !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
        }
        None => true,
    }
}

fn validate_fact_map(
    field: &'static str,
    values: &BTreeMap<String, bool>,
) -> Result<(), ProtocolError> {
    for name in values.keys() {
        validate_text(field, name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn artifact(filename: &str, hash_byte: char) -> ArtifactIdentity {
        ArtifactIdentity {
            filename: filename.into(),
            product_version: "1.0.0".into(),
            file_version: "1.0.0.1".into(),
            sha256: std::iter::repeat_n(hash_byte, 64).collect(),
        }
    }

    fn hello() -> Hello {
        Hello {
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
            peer: artifact("magic-tdx-fake-peer.exe", 'a'),
            peer_architecture: "x86_64-pc-windows-msvc".into(),
            terminal: artifact("TdxW.exe", 'b'),
            transport_profile_id: "official-tq-local-http-v1".into(),
            terminal_state: TerminalState::Ready,
            capabilities: BTreeMap::from([("quotes".into(), true)]),
            entitlements: BTreeMap::from([("level1".into(), true)]),
        }
    }

    fn observation(sequence: u64) -> SourceObservation {
        SourceObservation {
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
            bridge_sequence: sequence,
            instrument: SourceInstrument {
                exchange: SourceExchange::Shanghai,
                code: "600000".into(),
            },
            observed_at_utc: "2026-08-13T01:02:03Z".into(),
            source_timestamp: Some("2026-08-13T09:02:03+08:00".into()),
            price: Some(DecimalObservation {
                value: "12.34".into(),
                unit: ObservationUnit::CnyPerShare,
            }),
            cumulative_amount: Some(DecimalObservation {
                value: "123456.78".into(),
                unit: ObservationUnit::Cny,
            }),
            cumulative_volume: Some(DecimalObservation {
                value: "10000".into(),
                unit: ObservationUnit::Lot,
            }),
            source_record_count: Some(42),
        }
    }

    #[test]
    fn round_trips_one_big_endian_length_prefixed_message() {
        let codec = FrameCodec::new(16 * 1024).unwrap();
        let message = BridgeMessage::Hello(Box::new(hello()));
        let mut bytes = Vec::new();
        codec.write_message(&mut bytes, &message).unwrap();

        let announced = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert_eq!(announced, bytes.len() - 4);
        assert_eq!(
            codec.read_message(&mut Cursor::new(bytes)).unwrap(),
            message
        );
    }

    #[test]
    fn round_trips_versioned_shutdown_and_stopped_frames() {
        let codec = FrameCodec::new(1024).unwrap();
        let command = BridgeCommand::Shutdown(Shutdown::current());
        let mut command_bytes = Vec::new();
        codec.write_command(&mut command_bytes, &command).unwrap();
        assert_eq!(
            codec.read_command(&mut Cursor::new(command_bytes)).unwrap(),
            command
        );

        let message = BridgeMessage::Stopped(Stopped::current(1));
        let mut message_bytes = Vec::new();
        codec.write_message(&mut message_bytes, &message).unwrap();
        assert_eq!(
            codec.read_message(&mut Cursor::new(message_bytes)).unwrap(),
            message
        );
    }

    #[test]
    fn rejects_zero_or_unrepresentable_frame_limits() {
        assert!(matches!(
            FrameCodec::new(0),
            Err(ProtocolError::InvalidFrameLimit { requested: 0, .. })
        ));
        if let Ok(unrepresentable) = usize::try_from(u64::from(u32::MAX) + 1) {
            assert!(matches!(
                FrameCodec::new(unrepresentable),
                Err(ProtocolError::InvalidFrameLimit { .. })
            ));
        }
    }

    #[test]
    fn rejects_announced_oversize_before_reading_a_payload() {
        let codec = FrameCodec::new(8).unwrap();
        let mut input = Cursor::new(9_u32.to_be_bytes());
        let error = codec
            .read_json::<serde_json::Value>(&mut input)
            .unwrap_err();
        assert!(matches!(
            error,
            ProtocolError::FrameTooLarge {
                announced: 9,
                maximum: 8
            }
        ));
        assert_eq!(input.position(), 4);
    }

    #[test]
    fn rejects_empty_partial_non_utf8_and_non_json_frames_explicitly() {
        let codec = FrameCodec::new(64).unwrap();

        let empty = codec
            .read_json::<serde_json::Value>(&mut Cursor::new(0_u32.to_be_bytes()))
            .unwrap_err();
        assert!(matches!(empty, ProtocolError::EmptyFrame));

        let mut partial = Vec::from(4_u32.to_be_bytes());
        partial.extend_from_slice(b"{}");
        let partial = codec
            .read_json::<serde_json::Value>(&mut Cursor::new(partial))
            .unwrap_err();
        assert!(matches!(
            partial,
            ProtocolError::Io {
                operation: FrameIoOperation::ReadPayload,
                ..
            }
        ));

        let mut invalid_utf8 = Vec::from(1_u32.to_be_bytes());
        invalid_utf8.push(0xff);
        assert!(matches!(
            codec.read_json::<serde_json::Value>(&mut Cursor::new(invalid_utf8)),
            Err(ProtocolError::InvalidUtf8(_))
        ));

        let mut invalid_json = Vec::from(1_u32.to_be_bytes());
        invalid_json.push(b'{');
        assert!(matches!(
            codec.read_json::<serde_json::Value>(&mut Cursor::new(invalid_json)),
            Err(ProtocolError::DecodeJson(_))
        ));
    }

    #[test]
    fn rejects_oversize_encoding_without_writing_a_prefix() {
        let codec = FrameCodec::new(2).unwrap();
        let mut output = Vec::new();
        let error = codec.write_json(&mut output, &"long").unwrap_err();
        assert!(matches!(
            error,
            ProtocolError::EncodedFrameTooLarge { maximum: 2 }
        ));
        assert!(output.is_empty());
    }

    #[test]
    fn hello_rejects_wrong_versions_and_incomplete_identity() {
        let mut wrong_protocol = hello();
        wrong_protocol.protocol_version += 1;
        assert!(matches!(
            wrong_protocol.validate(),
            Err(ProtocolError::UnsupportedProtocolVersion { .. })
        ));

        let mut wrong_schema = hello();
        wrong_schema.schema_version += 1;
        assert!(matches!(
            wrong_schema.validate(),
            Err(ProtocolError::UnsupportedSchemaVersion { .. })
        ));

        let mut missing_transport = hello();
        missing_transport.transport_profile_id.clear();
        assert!(matches!(
            missing_transport.validate(),
            Err(ProtocolError::InvalidHelloField {
                field: "transport_profile_id",
                ..
            })
        ));

        let mut invalid_hash = hello();
        invalid_hash.terminal.sha256 = "ABC".into();
        assert!(matches!(
            invalid_hash.validate(),
            Err(ProtocolError::InvalidHelloField {
                field: "terminal",
                ..
            })
        ));
    }

    #[test]
    fn hello_rejects_unknown_json_fields() {
        let codec = FrameCodec::new(16 * 1024).unwrap();
        let mut value = serde_json::to_value(BridgeMessage::Hello(Box::new(hello()))).unwrap();
        value["payload"]["unexpected"] = serde_json::Value::Bool(true);
        let payload = serde_json::to_vec(&value).unwrap();
        let mut frame = Vec::from(u32::try_from(payload.len()).unwrap().to_be_bytes());
        frame.extend(payload);
        assert!(matches!(
            codec.read_message(&mut Cursor::new(frame)),
            Err(ProtocolError::DecodeJson(_))
        ));
    }

    #[test]
    fn message_rejects_unknown_envelope_fields() {
        let codec = FrameCodec::new(16 * 1024).unwrap();
        let mut value = serde_json::to_value(BridgeMessage::Hello(Box::new(hello()))).unwrap();
        value["unexpected"] = serde_json::Value::Bool(true);
        let payload = serde_json::to_vec(&value).unwrap();
        let mut frame = Vec::from(u32::try_from(payload.len()).unwrap().to_be_bytes());
        frame.extend(payload);
        assert!(matches!(
            codec.read_message(&mut Cursor::new(frame)),
            Err(ProtocolError::DecodeJson(_))
        ));
    }

    #[test]
    fn observation_round_trips_all_four_source_fields_with_explicit_units() {
        let codec = FrameCodec::new(16 * 1024).unwrap();
        let message = BridgeMessage::Observation(Box::new(observation(1)));
        let mut bytes = Vec::new();
        codec.write_message(&mut bytes, &message).unwrap();
        assert_eq!(
            codec.read_message(&mut Cursor::new(bytes)).unwrap(),
            message
        );
    }

    #[test]
    fn observation_rejects_missing_fields_wrong_units_and_noncanonical_decimals() {
        let mut empty = observation(1);
        empty.price = None;
        empty.cumulative_amount = None;
        empty.cumulative_volume = None;
        empty.source_record_count = None;
        assert!(matches!(
            empty.validate(),
            Err(ProtocolError::InvalidMessageField {
                field: "observation",
                ..
            })
        ));

        let mut wrong_unit = observation(1);
        wrong_unit.price.as_mut().unwrap().unit = ObservationUnit::Cny;
        assert!(matches!(
            wrong_unit.validate(),
            Err(ProtocolError::InvalidMessageField {
                field: "observation.price",
                ..
            })
        ));

        for invalid in ["", "-1", ".1", "1.", "01", "1e3", "1.2.3"] {
            let mut malformed = observation(1);
            malformed.price.as_mut().unwrap().value = invalid.into();
            assert!(matches!(
                malformed.validate(),
                Err(ProtocolError::InvalidMessageField {
                    field: "observation.price",
                    ..
                })
            ));
        }
    }

    #[test]
    fn sequence_tracker_detects_gaps_rollbacks_and_exhaustion() {
        let mut starts_late = BridgeSequenceTracker::new();
        assert!(matches!(
            starts_late.observe(&BridgeMessage::Observation(Box::new(observation(2)))),
            Err(ProtocolError::BridgeSequenceGap {
                expected: 1,
                actual: 2
            })
        ));

        let mut tracker = BridgeSequenceTracker::new();
        tracker
            .observe(&BridgeMessage::Observation(Box::new(observation(1))))
            .unwrap();
        assert!(matches!(
            tracker.observe(&BridgeMessage::Observation(Box::new(observation(3)))),
            Err(ProtocolError::BridgeSequenceGap {
                expected: 2,
                actual: 3
            })
        ));
        assert_eq!(tracker.last(), Some(1));
        assert!(matches!(
            tracker.observe(&BridgeMessage::Observation(Box::new(observation(1)))),
            Err(ProtocolError::BridgeSequenceRollback {
                previous: 1,
                actual: 1
            })
        ));

        let mut exhausted = BridgeSequenceTracker {
            last: Some(u64::MAX),
        };
        assert!(matches!(
            exhausted.observe(&BridgeMessage::Stopped(Stopped::current(1))),
            Err(ProtocolError::BridgeSequenceExhausted { previous: u64::MAX })
        ));
    }

    #[test]
    fn status_and_error_messages_require_versions_sequence_and_bounded_text_shape() {
        let status = BridgeMessage::Status(BridgeStatus {
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
            bridge_sequence: 1,
            state: BridgeRuntimeState::Ready,
            reason: BridgeStatusReason::SourceReady,
            detail: None,
        });
        status.validate().unwrap();

        let invalid = BridgeMessage::Error(BridgeErrorReport {
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
            bridge_sequence: 0,
            code: BridgeErrorCode::SourceReadFailed,
            retryable: true,
            message: "fixture failure".into(),
        });
        assert!(matches!(
            invalid.validate(),
            Err(ProtocolError::InvalidBridgeSequence)
        ));
    }
}
