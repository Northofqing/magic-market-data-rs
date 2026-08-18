use magic_market_core::{
    AnomalyEvent, AnomalyInputEvidence, AnomalyRuleIdentity, AnomalyTransition, ContinuityState,
    CoreError, InstrumentId, LocalAnalysisEventEvidence, LocalTerminalObservationEvidence, Money,
    ObservationTimeBasis, Price, ProviderId, Quantity, RuleInputDigest, SourceEvidence,
    StreamContinuity, StreamCursor,
};
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt;

/// Explicit resource limits. No production defaults are supplied by Core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorLimits {
    max_instruments: u16,
    window_capacity: u16,
}

impl MonitorLimits {
    pub fn new(max_instruments: u16, window_capacity: u16) -> Result<Self, MonitorError> {
        if max_instruments == 0 {
            return Err(MonitorError::InvalidConfiguration(
                "max instruments must be positive",
            ));
        }
        if window_capacity < 2 {
            return Err(MonitorError::InvalidConfiguration(
                "window capacity must be at least two",
            ));
        }
        Ok(Self {
            max_instruments,
            window_capacity,
        })
    }

    pub fn max_instruments(self) -> u16 {
        self.max_instruments
    }

    pub fn window_capacity(self) -> u16 {
        self.window_capacity
    }
}

/// Complete policy for one positive price-change rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceChangeRule {
    window_millis: u64,
    boundary_tolerance_millis: u64,
    trigger_ratio: f64,
    rearm_ratio: f64,
    cooldown_millis: u64,
}

impl PriceChangeRule {
    pub const ID: &'static str = "local_price_change";

    pub fn new(
        window_millis: u64,
        boundary_tolerance_millis: u64,
        trigger_ratio: f64,
        rearm_ratio: f64,
        cooldown_millis: u64,
    ) -> Result<Self, MonitorError> {
        if window_millis == 0 {
            return Err(MonitorError::InvalidConfiguration(
                "price window duration must be positive",
            ));
        }
        if boundary_tolerance_millis > window_millis {
            return Err(MonitorError::InvalidConfiguration(
                "boundary tolerance must not exceed the price window",
            ));
        }
        if !trigger_ratio.is_finite() || trigger_ratio <= 0.0 {
            return Err(MonitorError::InvalidConfiguration(
                "trigger ratio must be finite and positive",
            ));
        }
        if !rearm_ratio.is_finite() || rearm_ratio < 0.0 || rearm_ratio >= trigger_ratio {
            return Err(MonitorError::InvalidConfiguration(
                "rearm ratio must be finite, non-negative, and below the trigger ratio",
            ));
        }
        Ok(Self {
            window_millis,
            boundary_tolerance_millis,
            trigger_ratio,
            rearm_ratio,
            cooldown_millis,
        })
    }

    pub fn window_millis(self) -> u64 {
        self.window_millis
    }

    pub fn boundary_tolerance_millis(self) -> u64 {
        self.boundary_tolerance_millis
    }

    pub fn trigger_ratio(self) -> f64 {
        self.trigger_ratio
    }

    pub fn rearm_ratio(self) -> f64 {
        self.rearm_ratio
    }

    pub fn cooldown_millis(self) -> u64 {
        self.cooldown_millis
    }

    pub fn core_identity(self, version: u32) -> Result<AnomalyRuleIdentity, CoreError> {
        let boundary = self.boundary_tolerance_millis.to_be_bytes();
        let cooldown = self.cooldown_millis.to_be_bytes();
        let rearm = self.rearm_ratio.to_bits().to_be_bytes();
        let trigger = self.trigger_ratio.to_bits().to_be_bytes();
        let window = self.window_millis.to_be_bytes();
        AnomalyRuleIdentity::from_canonical_definition(
            Self::ID,
            version,
            &[
                ("accepted_continuity", b"continuous"),
                ("boundary_tolerance_millis", boundary.as_slice()),
                ("cooldown_millis", cooldown.as_slice()),
                ("family", b"price"),
                ("rearm_ratio_bits", rearm.as_slice()),
                ("session_reset_policy", b"explicit_injected_signal"),
                ("time_basis", b"local_observation_time"),
                ("trigger_ratio_bits", trigger.as_slice()),
                ("unit", b"cny_per_share"),
                ("window_millis", window.as_slice()),
            ],
        )
    }
}

/// One already-normalized local-terminal observation injected into the kernel.
#[derive(Debug, Clone, PartialEq)]
pub struct InjectedObservation {
    instrument: InstrumentId,
    evidence: SourceEvidence,
    arrival_millis: u64,
    price: Option<Price>,
    cumulative_amount: Option<Money>,
    cumulative_volume: Option<Quantity>,
    cumulative_volume_unit: Option<SourceQuantityUnit>,
    source_record_count: Option<u64>,
    stream_cursor: Option<StreamCursor>,
    continuity: ContinuityState,
}

impl InjectedObservation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instrument: InstrumentId,
        evidence: SourceEvidence,
        arrival_millis: u64,
        price: Price,
        cumulative_amount: Money,
        cumulative_volume: Quantity,
        cumulative_volume_unit: SourceQuantityUnit,
        continuity: ContinuityState,
    ) -> Result<Self, MonitorError> {
        Self::from_families(
            instrument,
            evidence,
            arrival_millis,
            Some(price),
            Some(cumulative_amount),
            Some(cumulative_volume),
            Some(cumulative_volume_unit),
            continuity,
        )
    }

    /// Constructs an observation whose data families are independently
    /// optional.
    ///
    /// `None` means that the source did not provide that family. It is retained
    /// as unavailable and is never converted into numeric zero. Each monitor
    /// accepts observations missing unrelated families and rejects a missing
    /// required family before mutating state.
    #[allow(clippy::too_many_arguments)]
    pub fn from_families(
        instrument: InstrumentId,
        evidence: SourceEvidence,
        arrival_millis: u64,
        price: Option<Price>,
        cumulative_amount: Option<Money>,
        cumulative_volume: Option<Quantity>,
        cumulative_volume_unit: Option<SourceQuantityUnit>,
        continuity: ContinuityState,
    ) -> Result<Self, MonitorError> {
        if evidence.provider() != ProviderId::LocalTerminal {
            return Err(MonitorError::InvalidObservation(
                "observation evidence must use LocalTerminal",
            ));
        }
        if cumulative_amount.is_some_and(|amount| amount.get() < 0.0) {
            return Err(MonitorError::InvalidObservation(
                "cumulative amount must be non-negative",
            ));
        }
        Ok(Self {
            instrument,
            evidence,
            arrival_millis,
            price,
            cumulative_amount,
            cumulative_volume,
            cumulative_volume_unit,
            source_record_count: None,
            stream_cursor: None,
            continuity,
        })
    }

    /// Binds the local source's cumulative received-record count to this
    /// observation.
    ///
    /// This is not an exchange trade count and does not prove complete provider
    /// tick coverage. The value is optional so an unavailable, unadmitted
    /// source field is never fabricated as zero.
    pub fn with_source_record_count(mut self, source_record_count: u64) -> Self {
        self.source_record_count = Some(source_record_count);
        self
    }

    /// Binds the local observation stream position used by BR-044 event
    /// evidence.
    pub fn with_stream_cursor(mut self, stream_cursor: StreamCursor) -> Self {
        self.stream_cursor = Some(stream_cursor);
        self
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }

    pub fn arrival_millis(&self) -> u64 {
        self.arrival_millis
    }

    pub fn price(&self) -> Option<Price> {
        self.price
    }

    pub fn cumulative_amount(&self) -> Option<Money> {
        self.cumulative_amount
    }

    pub fn cumulative_volume(&self) -> Option<Quantity> {
        self.cumulative_volume
    }

    pub fn cumulative_volume_unit(&self) -> Option<SourceQuantityUnit> {
        self.cumulative_volume_unit
    }

    pub fn source_record_count(&self) -> Option<u64> {
        self.source_record_count
    }

    pub fn stream_cursor(&self) -> Option<&StreamCursor> {
        self.stream_cursor.as_ref()
    }

    pub fn continuity(&self) -> ContinuityState {
        self.continuity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleState {
    WarmingUp,
    Armed,
    Triggered,
    CoolingDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetReason {
    NonContinuous(ContinuityState),
    CumulativeAmountRollback,
    CumulativeVolumeRollback,
    SourceRecordCountRollback,
    SamplingGap,
    TradingDateChanged,
    SessionBoundary,
    MiddayBreak,
}

/// A reset authorized by an explicit upstream calendar/session decision.
///
/// The monitor owns no calendar and never infers these signals from weekdays,
/// wall-clock time, or proposal defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectedResetSignal {
    TradingDateChanged,
    SessionBoundary,
    MiddayBreak,
}

impl From<InjectedResetSignal> for ResetReason {
    fn from(value: InjectedResetSignal) -> Self {
        match value {
            InjectedResetSignal::TradingDateChanged => Self::TradingDateChanged,
            InjectedResetSignal::SessionBoundary => Self::SessionBoundary,
            InjectedResetSignal::MiddayBreak => Self::MiddayBreak,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MonitorTransition {
    WarmedUp,
    Triggered { change_ratio: f64 },
    EnteredCoolingDown,
    Rearmed { change_ratio: f64 },
    Reset(ResetReason),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessOutcome {
    state: RuleState,
    transition: Option<MonitorTransition>,
}

impl ProcessOutcome {
    pub fn state(self) -> RuleState {
        self.state
    }

    pub fn transition(self) -> Option<MonitorTransition> {
        self.transition
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorError {
    InvalidConfiguration(&'static str),
    InvalidObservation(&'static str),
    FamilyUnavailable {
        family: ObservationFamily,
    },
    UnitUnavailable {
        family: ObservationFamily,
    },
    SourceQuantityUnitMismatch {
        expected: SourceQuantityUnit,
        actual: SourceQuantityUnit,
    },
    InstrumentLimit {
        limit: u16,
    },
    DuplicateObservation {
        arrival_millis: u64,
    },
    OutOfOrderObservation {
        previous: u64,
        actual: u64,
    },
    WindowOverflow {
        capacity: u16,
    },
    PriceChangeOverflow,
    CumulativeDeltaOverflow,
}

impl fmt::Display for MonitorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(formatter, "invalid monitor configuration: {message}")
            }
            Self::InvalidObservation(message) => {
                write!(formatter, "invalid injected observation: {message}")
            }
            Self::FamilyUnavailable { family } => {
                write!(formatter, "observation family {family} is unavailable")
            }
            Self::UnitUnavailable { family } => {
                write!(formatter, "observation family {family} has no source unit")
            }
            Self::SourceQuantityUnitMismatch { expected, actual } => write!(
                formatter,
                "source quantity unit mismatch: expected {expected}, got {actual}"
            ),
            Self::InstrumentLimit { limit } => {
                write!(formatter, "monitor instrument limit {limit} reached")
            }
            Self::DuplicateObservation { arrival_millis } => {
                write!(formatter, "duplicate observation time {arrival_millis}")
            }
            Self::OutOfOrderObservation { previous, actual } => write!(
                formatter,
                "out-of-order observation time {actual} follows {previous}"
            ),
            Self::WindowOverflow { capacity } => {
                write!(formatter, "monitor window capacity {capacity} exceeded")
            }
            Self::PriceChangeOverflow => formatter.write_str("price-change computation overflowed"),
            Self::CumulativeDeltaOverflow => {
                formatter.write_str("cumulative-delta computation overflowed")
            }
        }
    }
}

impl Error for MonitorError {}

/// Independently available observation family required by one monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationFamily {
    Price,
    CumulativeAmount,
    CumulativeVolume,
}

/// Closed provider-source quantity units. No conversion or defaulting is
/// performed by the monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceQuantityUnit {
    Lot,
    Share,
}

impl SourceQuantityUnit {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lot => "lot",
            Self::Share => "share",
        }
    }
}

impl fmt::Display for SourceQuantityUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for ObservationFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Price => "price",
            Self::CumulativeAmount => "cumulative_amount",
            Self::CumulativeVolume => "cumulative_volume",
        })
    }
}

/// I/O-free, single-writer price monitor with independent instrument state.
pub struct DeterministicPriceMonitor {
    limits: MonitorLimits,
    rule: PriceChangeRule,
    instruments: HashMap<InstrumentId, InstrumentState>,
}

impl DeterministicPriceMonitor {
    pub fn new(limits: MonitorLimits, rule: PriceChangeRule) -> Self {
        Self {
            limits,
            rule,
            instruments: HashMap::new(),
        }
    }

    pub fn process(
        &mut self,
        observation: InjectedObservation,
    ) -> Result<ProcessOutcome, MonitorError> {
        require_family(&observation, ObservationFamily::Price)?;
        let instrument = observation.instrument.clone();
        if !self.instruments.contains_key(&instrument)
            && self.instruments.len() >= usize::from(self.limits.max_instruments)
        {
            return Err(MonitorError::InstrumentLimit {
                limit: self.limits.max_instruments,
            });
        }
        let state = self
            .instruments
            .entry(instrument)
            .or_insert_with(InstrumentState::new);
        state.process(observation, self.rule, self.limits.window_capacity)
    }

    pub fn state(&self, instrument: &InstrumentId) -> Option<RuleState> {
        self.instruments
            .get(instrument)
            .map(|state| state.rule_state)
    }

    pub fn window_len(&self, instrument: &InstrumentId) -> Option<usize> {
        self.instruments
            .get(instrument)
            .map(|state| state.window.len())
    }

    pub fn instrument_count(&self) -> usize {
        self.instruments.len()
    }

    /// Applies a caller-authorized calendar/session reset without seeding a
    /// synthetic observation.
    pub fn inject_reset(
        &mut self,
        instrument: &InstrumentId,
        signal: InjectedResetSignal,
    ) -> Option<ProcessOutcome> {
        let state = self.instruments.get_mut(instrument)?;
        state.reset_without_seed();
        Some(state.outcome(Some(MonitorTransition::Reset(signal.into()))))
    }

    /// Exact retained boundary observations for later evidence-bound events.
    pub fn window_endpoints(
        &self,
        instrument: &InstrumentId,
    ) -> Option<(&InjectedObservation, &InjectedObservation)> {
        let window = &self.instruments.get(instrument)?.window;
        Some((window.front()?, window.back()?))
    }

    /// Typed endpoint evidence, including the explicitly available local-source
    /// record-count range.
    pub fn window_evidence(&self, instrument: &InstrumentId) -> Option<WindowEndpointEvidence> {
        let (first, last) = self.window_endpoints(instrument)?;
        Some(WindowEndpointEvidence::new(first, last))
    }
}

/// Exact endpoints retained by one deterministic window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowEndpointEvidence {
    instrument: InstrumentId,
    first: SourceEvidence,
    last: SourceEvidence,
    first_arrival_millis: u64,
    last_arrival_millis: u64,
    first_source_record_count: Option<u64>,
    last_source_record_count: Option<u64>,
    first_stream_cursor: Option<StreamCursor>,
    last_stream_cursor: Option<StreamCursor>,
    first_price_bits: Option<u64>,
    last_price_bits: Option<u64>,
    first_amount_bits: Option<u64>,
    last_amount_bits: Option<u64>,
    first_volume_bits: Option<u64>,
    last_volume_bits: Option<u64>,
    first_volume_unit: Option<SourceQuantityUnit>,
    last_volume_unit: Option<SourceQuantityUnit>,
}

impl WindowEndpointEvidence {
    pub(crate) fn new(first: &InjectedObservation, last: &InjectedObservation) -> Self {
        Self {
            instrument: first.instrument.clone(),
            first: first.evidence.clone(),
            last: last.evidence.clone(),
            first_arrival_millis: first.arrival_millis,
            last_arrival_millis: last.arrival_millis,
            first_source_record_count: first.source_record_count,
            last_source_record_count: last.source_record_count,
            first_stream_cursor: first.stream_cursor.clone(),
            last_stream_cursor: last.stream_cursor.clone(),
            first_price_bits: first.price.map(|value| value.get().to_bits()),
            last_price_bits: last.price.map(|value| value.get().to_bits()),
            first_amount_bits: first.cumulative_amount.map(|value| value.get().to_bits()),
            last_amount_bits: last.cumulative_amount.map(|value| value.get().to_bits()),
            first_volume_bits: first.cumulative_volume.map(|value| value.get().to_bits()),
            last_volume_bits: last.cumulative_volume.map(|value| value.get().to_bits()),
            first_volume_unit: first.cumulative_volume_unit,
            last_volume_unit: last.cumulative_volume_unit,
        }
    }

    pub fn first(&self) -> &SourceEvidence {
        &self.first
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn last(&self) -> &SourceEvidence {
        &self.last
    }

    pub fn first_arrival_millis(&self) -> u64 {
        self.first_arrival_millis
    }

    pub fn last_arrival_millis(&self) -> u64 {
        self.last_arrival_millis
    }

    pub fn first_source_record_count(&self) -> Option<u64> {
        self.first_source_record_count
    }

    pub fn last_source_record_count(&self) -> Option<u64> {
        self.last_source_record_count
    }

    pub fn first_stream_cursor(&self) -> Option<&StreamCursor> {
        self.first_stream_cursor.as_ref()
    }

    pub fn last_stream_cursor(&self) -> Option<&StreamCursor> {
        self.last_stream_cursor.as_ref()
    }

    pub fn first_price(&self) -> Option<Price> {
        self.first_price_bits.map(|bits| {
            Price::new(f64::from_bits(bits)).expect("retained optional price was validated")
        })
    }

    pub fn last_price(&self) -> Option<Price> {
        self.last_price_bits.map(|bits| {
            Price::new(f64::from_bits(bits)).expect("retained optional price was validated")
        })
    }

    pub fn first_cumulative_amount(&self) -> Option<Money> {
        self.first_amount_bits.map(|bits| {
            Money::new(f64::from_bits(bits)).expect("retained optional amount was validated")
        })
    }

    pub fn last_cumulative_amount(&self) -> Option<Money> {
        self.last_amount_bits.map(|bits| {
            Money::new(f64::from_bits(bits)).expect("retained optional amount was validated")
        })
    }

    pub fn first_cumulative_volume(&self) -> Option<Quantity> {
        self.first_volume_bits.map(|bits| {
            Quantity::new(f64::from_bits(bits)).expect("retained optional volume was validated")
        })
    }

    pub fn last_cumulative_volume(&self) -> Option<Quantity> {
        self.last_volume_bits.map(|bits| {
            Quantity::new(f64::from_bits(bits)).expect("retained optional volume was validated")
        })
    }

    pub fn first_cumulative_volume_unit(&self) -> Option<SourceQuantityUnit> {
        self.first_volume_unit
    }

    pub fn last_cumulative_volume_unit(&self) -> Option<SourceQuantityUnit> {
        self.last_volume_unit
    }

    /// Records received by the local source between the two endpoints. This
    /// value is absent unless both cumulative endpoint counts are present and
    /// ordered.
    pub fn observed_source_record_count(&self) -> Option<u64> {
        self.last_source_record_count?
            .checked_sub(self.first_source_record_count?)
    }

    pub fn core_input_evidence(&self) -> Result<LocalTerminalObservationEvidence, CoreError> {
        LocalTerminalObservationEvidence::new(self.first.clone(), self.last.clone())
    }

    /// Completes Core's provider-identity boundary with caller-constructed
    /// `LocalAnalysis` evidence. Core validates that derived provider identity.
    pub fn core_event_evidence(
        &self,
        derived: SourceEvidence,
    ) -> Result<LocalAnalysisEventEvidence, CoreError> {
        LocalAnalysisEventEvidence::new(self.core_input_evidence()?, derived)
    }

    /// Constructs the replay-stable Core event for a price trigger or rearm.
    /// Warm-up, cooling and reset transitions are status updates and cannot be
    /// promoted into anomaly events.
    pub fn price_core_event(
        &self,
        rule: PriceChangeRule,
        version: u32,
        transition: MonitorTransition,
        stream: StreamCursor,
        continuity: StreamContinuity,
        derived_observed_at: impl Into<String>,
    ) -> Result<AnomalyEvent, CoreError> {
        let (transition, change_ratio) = match transition {
            MonitorTransition::Triggered { change_ratio } => {
                (AnomalyTransition::Triggered, change_ratio)
            }
            MonitorTransition::Rearmed { change_ratio } => {
                (AnomalyTransition::Rearmed, change_ratio)
            }
            _ => {
                return Err(CoreError::InvalidRequest(
                    "price outcome has no public anomaly transition".into(),
                ))
            }
        };
        let first = self
            .first_price()
            .ok_or_else(|| CoreError::InvalidRequest("price window has no first value".into()))?
            .get()
            .to_bits()
            .to_be_bytes();
        let last = self
            .last_price()
            .ok_or_else(|| CoreError::InvalidRequest("price window has no last value".into()))?
            .get()
            .to_bits()
            .to_be_bytes();
        let ratio = change_ratio.to_bits().to_be_bytes();
        let first_arrival = self.first_arrival_millis.to_be_bytes();
        let last_arrival = self.last_arrival_millis.to_be_bytes();
        let record_count = encode_optional_u64(self.observed_source_record_count());
        let inputs = RuleInputDigest::from_canonical_fields(&[
            ("change_ratio_bits", ratio.as_slice()),
            ("first_arrival_millis", first_arrival.as_slice()),
            ("first_price_bits", first.as_slice()),
            ("last_arrival_millis", last_arrival.as_slice()),
            ("last_price_bits", last.as_slice()),
            ("observed_source_record_count", record_count.as_slice()),
            ("unit", b"cny_per_share"),
        ])?;
        let input_evidence = self.core_anomaly_input_evidence(
            ObservationTimeBasis::LocalObservationTime,
            continuity,
            inputs,
        )?;
        AnomalyEvent::new(
            self.instrument.clone(),
            rule.core_identity(version)?,
            transition,
            stream,
            input_evidence,
            derived_observed_at,
        )
    }

    pub(crate) fn core_anomaly_input_evidence(
        &self,
        time_basis: ObservationTimeBasis,
        continuity: StreamContinuity,
        rule_inputs_digest: RuleInputDigest,
    ) -> Result<AnomalyInputEvidence, CoreError> {
        let first_cursor = self.first_stream_cursor.clone().ok_or_else(|| {
            CoreError::InvalidRequest("anomaly endpoint is missing its first stream cursor".into())
        })?;
        let last_cursor = self.last_stream_cursor.clone().ok_or_else(|| {
            CoreError::InvalidRequest("anomaly endpoint is missing its last stream cursor".into())
        })?;
        AnomalyInputEvidence::new(
            self.instrument.clone(),
            self.core_input_evidence()?,
            first_cursor,
            last_cursor,
            time_basis,
            continuity,
            rule_inputs_digest,
        )
    }
}

fn encode_optional_u64(value: Option<u64>) -> [u8; 9] {
    let mut encoded = [0_u8; 9];
    if let Some(value) = value {
        encoded[0] = 1;
        encoded[1..].copy_from_slice(&value.to_be_bytes());
    }
    encoded
}

#[derive(Debug, Clone)]
struct InstrumentState {
    rule_state: RuleState,
    window: VecDeque<InjectedObservation>,
    last_arrival_millis: Option<u64>,
    last_source_record_count: Option<u64>,
    triggered_at_millis: Option<u64>,
}

impl InstrumentState {
    fn new() -> Self {
        Self {
            rule_state: RuleState::WarmingUp,
            window: VecDeque::new(),
            last_arrival_millis: None,
            last_source_record_count: None,
            triggered_at_millis: None,
        }
    }

    fn process(
        &mut self,
        observation: InjectedObservation,
        rule: PriceChangeRule,
        window_capacity: u16,
    ) -> Result<ProcessOutcome, MonitorError> {
        self.validate_order(observation.arrival_millis)?;

        let reset_reason = self.reset_reason(&observation);
        if let Some(reason) = reset_reason {
            self.reset_and_seed(&observation);
            return Ok(self.outcome(Some(MonitorTransition::Reset(reason))));
        }

        self.prune_for(observation.arrival_millis, rule.window_millis);
        if self.window.len() >= usize::from(window_capacity) {
            self.reset_and_seed(&observation);
            return Err(MonitorError::WindowOverflow {
                capacity: window_capacity,
            });
        }
        self.seed(&observation);

        let change_ratio = match self.price_change(observation.arrival_millis, rule) {
            Ok(change_ratio) => change_ratio,
            Err(error) => {
                self.reset_and_seed(&observation);
                return Err(error);
            }
        };
        let Some(change_ratio) = change_ratio else {
            if self.rule_state != RuleState::WarmingUp {
                self.reset_and_seed(&observation);
                return Ok(self.outcome(Some(MonitorTransition::Reset(ResetReason::SamplingGap))));
            }
            return Ok(self.outcome(None));
        };

        match self.rule_state {
            RuleState::WarmingUp => {
                self.rule_state = RuleState::Armed;
                Ok(self.outcome(Some(MonitorTransition::WarmedUp)))
            }
            RuleState::Armed if change_ratio >= rule.trigger_ratio => {
                self.rule_state = RuleState::Triggered;
                self.triggered_at_millis = Some(observation.arrival_millis);
                Ok(self.outcome(Some(MonitorTransition::Triggered { change_ratio })))
            }
            RuleState::Armed => Ok(self.outcome(None)),
            RuleState::Triggered => {
                self.rule_state = RuleState::CoolingDown;
                Ok(self.outcome(Some(MonitorTransition::EnteredCoolingDown)))
            }
            RuleState::CoolingDown => {
                let triggered_at =
                    self.triggered_at_millis
                        .ok_or(MonitorError::InvalidObservation(
                            "cooling state has no trigger time",
                        ))?;
                let cooldown_complete =
                    observation.arrival_millis - triggered_at >= rule.cooldown_millis;
                if cooldown_complete && change_ratio <= rule.rearm_ratio {
                    self.rule_state = RuleState::Armed;
                    self.triggered_at_millis = None;
                    Ok(self.outcome(Some(MonitorTransition::Rearmed { change_ratio })))
                } else {
                    Ok(self.outcome(None))
                }
            }
        }
    }

    fn validate_order(&self, arrival_millis: u64) -> Result<(), MonitorError> {
        if let Some(previous) = self.last_arrival_millis {
            if arrival_millis == previous {
                return Err(MonitorError::DuplicateObservation { arrival_millis });
            }
            if arrival_millis < previous {
                return Err(MonitorError::OutOfOrderObservation {
                    previous,
                    actual: arrival_millis,
                });
            }
        }
        Ok(())
    }

    fn reset_reason(&self, observation: &InjectedObservation) -> Option<ResetReason> {
        if observation.continuity != ContinuityState::Continuous {
            return Some(ResetReason::NonContinuous(observation.continuity));
        }
        let source_record_count_rollback = self
            .last_source_record_count
            .zip(observation.source_record_count)
            .is_some_and(|(previous, actual)| actual < previous);
        if source_record_count_rollback {
            return Some(ResetReason::SourceRecordCountRollback);
        }
        None
    }

    fn prune_for(&mut self, arrival_millis: u64, window_millis: u64) {
        let Some(cutoff) = arrival_millis.checked_sub(window_millis) else {
            return;
        };
        while self.window.len() > 1
            && self
                .window
                .get(1)
                .is_some_and(|point| point.arrival_millis <= cutoff)
        {
            self.window.pop_front();
        }
    }

    fn price_change(
        &mut self,
        arrival_millis: u64,
        rule: PriceChangeRule,
    ) -> Result<Option<f64>, MonitorError> {
        let Some(cutoff) = arrival_millis.checked_sub(rule.window_millis) else {
            return Ok(None);
        };
        let Some(boundary) = self.window.front() else {
            return Ok(None);
        };
        if boundary.arrival_millis > cutoff
            || cutoff - boundary.arrival_millis > rule.boundary_tolerance_millis
        {
            return Ok(None);
        }
        let current = self.window.back().ok_or(MonitorError::InvalidObservation(
            "price window unexpectedly has no current observation",
        ))?;
        let boundary_price = boundary
            .price()
            .ok_or(MonitorError::FamilyUnavailable {
                family: ObservationFamily::Price,
            })?
            .get();
        let current_price = current
            .price()
            .ok_or(MonitorError::FamilyUnavailable {
                family: ObservationFamily::Price,
            })?
            .get();
        let change_ratio = (current_price - boundary_price) / boundary_price;
        if !change_ratio.is_finite() {
            return Err(MonitorError::PriceChangeOverflow);
        }
        Ok(Some(change_ratio))
    }

    fn reset_and_seed(&mut self, observation: &InjectedObservation) {
        self.reset_without_seed();
        self.seed(observation);
    }

    fn reset_without_seed(&mut self) {
        self.rule_state = RuleState::WarmingUp;
        self.window.clear();
        self.last_arrival_millis = None;
        self.last_source_record_count = None;
        self.triggered_at_millis = None;
    }

    fn seed(&mut self, observation: &InjectedObservation) {
        self.window.push_back(observation.clone());
        self.last_arrival_millis = Some(observation.arrival_millis);
        self.last_source_record_count = observation.source_record_count;
    }

    fn outcome(&self, transition: Option<MonitorTransition>) -> ProcessOutcome {
        ProcessOutcome {
            state: self.rule_state,
            transition,
        }
    }
}

pub(crate) fn require_family(
    observation: &InjectedObservation,
    family: ObservationFamily,
) -> Result<(), MonitorError> {
    let available = match family {
        ObservationFamily::Price => observation.price().is_some(),
        ObservationFamily::CumulativeAmount => observation.cumulative_amount().is_some(),
        ObservationFamily::CumulativeVolume => observation.cumulative_volume().is_some(),
    };
    if available {
        Ok(())
    } else {
        Err(MonitorError::FamilyUnavailable { family })
    }
}
