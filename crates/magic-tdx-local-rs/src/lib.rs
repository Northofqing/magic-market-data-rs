#![forbid(unsafe_code)]

//! Safe protocol, fixed-loopback HTTP and lifecycle primitives for optional
//! local TDX observation.
//!
//! This crate does not discover processes, launch executables, load vendor
//! libraries or automate the TDX terminal. Its only network path is bounded
//! synchronous POST access to the fixed vendor endpoint
//! `http://127.0.0.1:17709/`, with proxies and redirects disabled. Runtime code
//! can also drive the pure supervisor state machine and bounded stdio codec.

pub mod admission;
pub mod loopback;
pub mod protocol;
pub mod supervisor;

pub use admission::{
    CapabilityAvailability, LocalTerminalAdmission, LocalTerminalCapabilityAvailability,
    LocalTerminalRuntimeAvailability, RepositoryAdmission, RuntimeAvailability,
    LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED, LOCAL_TERMINAL_CUMULATIVE_VOLUME_ADMITTED,
    LOCAL_TERMINAL_PRICE_ADMITTED, LOCAL_TERMINAL_SOURCE_RECORD_COUNT_ADMITTED,
};
pub use loopback::{
    TqEquityUniverseEvidence, TqInstrument, TqLoopbackClient, TqLoopbackError,
    TqLoopbackErrorCategory, TqLoopbackLimits, TqReadMethod, TQ_LOOPBACK_ENDPOINT,
};
pub use protocol::{
    ArtifactIdentity, BridgeCommand, BridgeErrorCode, BridgeErrorReport, BridgeMessage,
    BridgeRuntimeState, BridgeSequenceTracker, BridgeStatus, BridgeStatusReason,
    DecimalObservation, FrameCodec, Hello, ObservationUnit, ProtocolError, Shutdown,
    SourceExchange, SourceInstrument, SourceObservation, Stopped, TerminalState, PROTOCOL_VERSION,
    SCHEMA_VERSION,
};
pub use supervisor::{
    StopDestination, SupervisorAction, SupervisorEvent, SupervisorMachine, SupervisorState,
    SupervisorTransition, TransitionError,
};
