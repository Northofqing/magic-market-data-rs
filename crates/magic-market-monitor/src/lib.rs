#![forbid(unsafe_code)]
//! Deterministic, I/O-free monitoring primitives.
//!
//! This crate owns no process, network, calendar, or system-clock integration.
//! Callers inject validated observations, monotonic arrival time, explicit
//! resource limits, and a complete versioned rule policy.

mod price;
mod replay;
mod turnover;

/// No local anomaly family is admitted before bounded live and shadow evidence.
pub const LOCAL_PRICE_CHANGE_ANOMALY_ADMITTED: bool = true;
/// No local amount-change anomaly is admitted before independent evidence.
pub const LOCAL_AMOUNT_CHANGE_ANOMALY_ADMITTED: bool = true;
/// No local volume-change anomaly is admitted before independent evidence.
pub const LOCAL_VOLUME_CHANGE_ANOMALY_ADMITTED: bool = true;

pub use price::{
    DeterministicPriceMonitor, InjectedObservation, InjectedResetSignal, MonitorError,
    MonitorLimits, MonitorTransition, ObservationFamily, PriceChangeRule, ProcessOutcome,
    ResetReason, RuleState, SourceQuantityUnit, WindowEndpointEvidence,
};
pub use replay::{ReplayEntry, ReplayError, ReplayLimits, ReplayLog, ReplayUnavailable};
pub use turnover::{
    AmountProcessOutcome, AmountSpikeRule, AmountTransition, DeterministicAmountMonitor,
    DeterministicVolumeMonitor, VolumeProcessOutcome, VolumeSpikeRule, VolumeTransition,
};
