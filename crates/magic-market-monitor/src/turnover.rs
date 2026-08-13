use crate::price::{
    require_family, InjectedObservation, InjectedResetSignal, MonitorError, MonitorLimits,
    ObservationFamily, ResetReason, RuleState, SourceQuantityUnit, WindowEndpointEvidence,
};
use magic_market_core::{
    AnomalyEvent, AnomalyRuleIdentity, AnomalyTransition, ContinuityState, CoreError, InstrumentId,
    Money, ObservationTimeBasis, Quantity, RuleInputDigest, StreamContinuity, StreamCursor,
};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq)]
struct DeltaRule {
    version: u32,
    window_millis: u64,
    boundary_tolerance_millis: u64,
    trigger_delta: f64,
    rearm_delta: f64,
    cooldown_millis: u64,
}

impl DeltaRule {
    fn new(
        version: u32,
        window_millis: u64,
        boundary_tolerance_millis: u64,
        trigger_delta: f64,
        rearm_delta: f64,
        cooldown_millis: u64,
    ) -> Result<Self, MonitorError> {
        if version == 0 {
            return Err(MonitorError::InvalidConfiguration(
                "rule version must be positive",
            ));
        }
        if window_millis == 0 {
            return Err(MonitorError::InvalidConfiguration(
                "cumulative-delta window duration must be positive",
            ));
        }
        if boundary_tolerance_millis > window_millis {
            return Err(MonitorError::InvalidConfiguration(
                "boundary tolerance must not exceed the cumulative-delta window",
            ));
        }
        if !trigger_delta.is_finite() || trigger_delta <= 0.0 {
            return Err(MonitorError::InvalidConfiguration(
                "cumulative-delta trigger must be finite and positive",
            ));
        }
        if !rearm_delta.is_finite() || rearm_delta < 0.0 || rearm_delta >= trigger_delta {
            return Err(MonitorError::InvalidConfiguration(
                "cumulative-delta rearm must be finite, non-negative, and below the trigger",
            ));
        }
        Ok(Self {
            version,
            window_millis,
            boundary_tolerance_millis,
            trigger_delta,
            rearm_delta,
            cooldown_millis,
        })
    }
}

/// Complete, versioned policy for the independent amount-spike family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmountSpikeRule(DeltaRule);

impl AmountSpikeRule {
    pub const ID: &'static str = "local_amount_spike";

    pub fn new(
        version: u32,
        window_millis: u64,
        boundary_tolerance_millis: u64,
        trigger_delta: Money,
        rearm_delta: Money,
        cooldown_millis: u64,
    ) -> Result<Self, MonitorError> {
        Ok(Self(DeltaRule::new(
            version,
            window_millis,
            boundary_tolerance_millis,
            trigger_delta.get(),
            rearm_delta.get(),
            cooldown_millis,
        )?))
    }

    pub fn version(self) -> u32 {
        self.0.version
    }

    pub fn window_millis(self) -> u64 {
        self.0.window_millis
    }

    pub fn boundary_tolerance_millis(self) -> u64 {
        self.0.boundary_tolerance_millis
    }

    pub fn trigger_delta(self) -> Money {
        Money::new(self.0.trigger_delta).expect("validated amount trigger")
    }

    pub fn rearm_delta(self) -> Money {
        Money::new(self.0.rearm_delta).expect("validated amount rearm")
    }

    pub fn cooldown_millis(self) -> u64 {
        self.0.cooldown_millis
    }

    pub fn core_identity(self) -> Result<AnomalyRuleIdentity, CoreError> {
        core_rule_identity(Self::ID, self.0, b"amount", b"cny_yuan")
    }
}

/// Complete, versioned policy for the independent volume-spike family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeSpikeRule {
    rule: DeltaRule,
    unit: SourceQuantityUnit,
}

impl VolumeSpikeRule {
    pub const ID: &'static str = "local_volume_spike";

    pub fn new(
        version: u32,
        window_millis: u64,
        boundary_tolerance_millis: u64,
        trigger_delta: Quantity,
        rearm_delta: Quantity,
        unit: SourceQuantityUnit,
        cooldown_millis: u64,
    ) -> Result<Self, MonitorError> {
        Ok(Self {
            rule: DeltaRule::new(
                version,
                window_millis,
                boundary_tolerance_millis,
                trigger_delta.get(),
                rearm_delta.get(),
                cooldown_millis,
            )?,
            unit,
        })
    }

    pub fn version(self) -> u32 {
        self.rule.version
    }

    pub fn window_millis(self) -> u64 {
        self.rule.window_millis
    }

    pub fn boundary_tolerance_millis(self) -> u64 {
        self.rule.boundary_tolerance_millis
    }

    pub fn trigger_delta(self) -> Quantity {
        Quantity::new(self.rule.trigger_delta).expect("validated volume trigger")
    }

    pub fn rearm_delta(self) -> Quantity {
        Quantity::new(self.rule.rearm_delta).expect("validated volume rearm")
    }

    pub fn cooldown_millis(self) -> u64 {
        self.rule.cooldown_millis
    }

    pub fn unit(self) -> SourceQuantityUnit {
        self.unit
    }

    pub fn core_identity(self) -> Result<AnomalyRuleIdentity, CoreError> {
        core_rule_identity(
            Self::ID,
            self.rule,
            b"volume",
            self.unit.as_str().as_bytes(),
        )
    }
}

fn core_rule_identity(
    id: &str,
    rule: DeltaRule,
    family: &[u8],
    unit: &[u8],
) -> Result<AnomalyRuleIdentity, CoreError> {
    let boundary = rule.boundary_tolerance_millis.to_be_bytes();
    let cooldown = rule.cooldown_millis.to_be_bytes();
    let rearm = rule.rearm_delta.to_bits().to_be_bytes();
    let trigger = rule.trigger_delta.to_bits().to_be_bytes();
    let window = rule.window_millis.to_be_bytes();
    AnomalyRuleIdentity::from_canonical_definition(
        id,
        rule.version,
        &[
            ("accepted_continuity", b"continuous"),
            ("boundary_tolerance_millis", boundary.as_slice()),
            ("cooldown_millis", cooldown.as_slice()),
            ("family", family),
            ("rearm_delta_bits", rearm.as_slice()),
            ("session_reset_policy", b"explicit_injected_signal"),
            ("time_basis", b"local_observation_time"),
            ("trigger_delta_bits", trigger.as_slice()),
            ("unit", unit),
            ("window_millis", window.as_slice()),
        ],
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DeltaTransition {
    WarmedUp,
    Triggered { delta: f64 },
    EnteredCoolingDown,
    Rearmed { delta: f64 },
    Reset(ResetReason),
}

#[derive(Debug, Clone, PartialEq)]
struct DeltaOutcome {
    state: RuleState,
    transition: Option<DeltaTransition>,
    evidence: Option<WindowEndpointEvidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AmountTransition {
    WarmedUp,
    Triggered { amount_delta: Money },
    EnteredCoolingDown,
    Rearmed { amount_delta: Money },
    Reset(ResetReason),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmountProcessOutcome {
    state: RuleState,
    transition: Option<AmountTransition>,
    evidence: Option<WindowEndpointEvidence>,
}

impl AmountProcessOutcome {
    pub fn state(&self) -> RuleState {
        self.state
    }

    pub fn transition(&self) -> Option<&AmountTransition> {
        self.transition.as_ref()
    }

    pub fn evidence(&self) -> Option<&WindowEndpointEvidence> {
        self.evidence.as_ref()
    }

    /// Constructs Core's replay-stable typed event for a public transition.
    /// Warming, cooling and reset transitions are status changes, not anomaly
    /// events, and fail explicitly here.
    pub fn core_event(
        &self,
        instrument: InstrumentId,
        rule: AmountSpikeRule,
        stream: StreamCursor,
        continuity: StreamContinuity,
        derived_observed_at: impl Into<String>,
    ) -> Result<AnomalyEvent, CoreError> {
        let (transition, delta) = match self.transition.as_ref() {
            Some(AmountTransition::Triggered { amount_delta }) => {
                (AnomalyTransition::Triggered, amount_delta.get())
            }
            Some(AmountTransition::Rearmed { amount_delta }) => {
                (AnomalyTransition::Rearmed, amount_delta.get())
            }
            _ => {
                return Err(CoreError::InvalidRequest(
                    "amount outcome has no public anomaly transition".into(),
                ))
            }
        };
        core_event(
            instrument,
            rule.core_identity()?,
            transition,
            stream,
            self.evidence.as_ref(),
            continuity,
            DeltaFamily::Amount,
            delta,
            derived_observed_at,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VolumeTransition {
    WarmedUp,
    Triggered {
        volume_delta: Quantity,
        unit: SourceQuantityUnit,
    },
    EnteredCoolingDown,
    Rearmed {
        volume_delta: Quantity,
        unit: SourceQuantityUnit,
    },
    Reset(ResetReason),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeProcessOutcome {
    state: RuleState,
    transition: Option<VolumeTransition>,
    evidence: Option<WindowEndpointEvidence>,
}

impl VolumeProcessOutcome {
    pub fn state(&self) -> RuleState {
        self.state
    }

    pub fn transition(&self) -> Option<&VolumeTransition> {
        self.transition.as_ref()
    }

    pub fn evidence(&self) -> Option<&WindowEndpointEvidence> {
        self.evidence.as_ref()
    }

    pub fn core_event(
        &self,
        instrument: InstrumentId,
        rule: VolumeSpikeRule,
        stream: StreamCursor,
        continuity: StreamContinuity,
        derived_observed_at: impl Into<String>,
    ) -> Result<AnomalyEvent, CoreError> {
        let (transition, delta) = match self.transition.as_ref() {
            Some(VolumeTransition::Triggered { volume_delta, unit }) => {
                if *unit != rule.unit() {
                    return Err(CoreError::InvalidRequest(
                        "volume transition unit does not match its rule".into(),
                    ));
                }
                (AnomalyTransition::Triggered, volume_delta.get())
            }
            Some(VolumeTransition::Rearmed { volume_delta, unit }) => {
                if *unit != rule.unit() {
                    return Err(CoreError::InvalidRequest(
                        "volume transition unit does not match its rule".into(),
                    ));
                }
                (AnomalyTransition::Rearmed, volume_delta.get())
            }
            _ => {
                return Err(CoreError::InvalidRequest(
                    "volume outcome has no public anomaly transition".into(),
                ))
            }
        };
        core_event(
            instrument,
            rule.core_identity()?,
            transition,
            stream,
            self.evidence.as_ref(),
            continuity,
            DeltaFamily::Volume(rule.unit()),
            delta,
            derived_observed_at,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn core_event(
    instrument: InstrumentId,
    rule: AnomalyRuleIdentity,
    transition: AnomalyTransition,
    stream: StreamCursor,
    evidence: Option<&WindowEndpointEvidence>,
    continuity: StreamContinuity,
    family: DeltaFamily,
    delta: f64,
    derived_observed_at: impl Into<String>,
) -> Result<AnomalyEvent, CoreError> {
    let evidence = evidence.ok_or_else(|| {
        CoreError::InvalidRequest("anomaly transition is missing endpoint evidence".into())
    })?;
    let delta_bits = delta.to_bits().to_be_bytes();
    let first_arrival = evidence.first_arrival_millis().to_be_bytes();
    let last_arrival = evidence.last_arrival_millis().to_be_bytes();
    let first_value = family
        .cumulative_evidence(evidence, true)?
        .to_bits()
        .to_be_bytes();
    let last_value = family
        .cumulative_evidence(evidence, false)?
        .to_bits()
        .to_be_bytes();
    let record_count = encoded_optional_u64(evidence.observed_source_record_count());
    let unit = family.unit_name().as_bytes();
    let inputs = RuleInputDigest::from_canonical_fields(&[
        ("delta_bits", delta_bits.as_slice()),
        ("first_arrival_millis", first_arrival.as_slice()),
        ("first_cumulative_bits", first_value.as_slice()),
        ("last_arrival_millis", last_arrival.as_slice()),
        ("last_cumulative_bits", last_value.as_slice()),
        ("observed_source_record_count", record_count.as_slice()),
        ("unit", unit),
    ])?;
    let input_evidence = evidence.core_anomaly_input_evidence(
        ObservationTimeBasis::LocalObservationTime,
        continuity,
        inputs,
    )?;
    AnomalyEvent::new(
        instrument,
        rule,
        transition,
        stream,
        input_evidence,
        derived_observed_at,
    )
}

fn encoded_optional_u64(value: Option<u64>) -> [u8; 9] {
    let mut encoded = [0_u8; 9];
    if let Some(value) = value {
        encoded[0] = 1;
        encoded[1..].copy_from_slice(&value.to_be_bytes());
    }
    encoded
}

#[derive(Debug, Clone, Copy)]
enum DeltaFamily {
    Amount,
    Volume(SourceQuantityUnit),
}

impl DeltaFamily {
    fn observation_family(self) -> ObservationFamily {
        match self {
            Self::Amount => ObservationFamily::CumulativeAmount,
            Self::Volume(_) => ObservationFamily::CumulativeVolume,
        }
    }

    fn validate_observation(self, observation: &InjectedObservation) -> Result<(), MonitorError> {
        require_family(observation, self.observation_family())?;
        if let Self::Volume(expected) = self {
            let actual =
                observation
                    .cumulative_volume_unit()
                    .ok_or(MonitorError::UnitUnavailable {
                        family: ObservationFamily::CumulativeVolume,
                    })?;
            if actual != expected {
                return Err(MonitorError::SourceQuantityUnitMismatch { expected, actual });
            }
        }
        Ok(())
    }

    fn unit_name(self) -> &'static str {
        match self {
            Self::Amount => "cny_yuan",
            Self::Volume(unit) => unit.as_str(),
        }
    }

    fn cumulative(self, observation: &InjectedObservation) -> Result<f64, MonitorError> {
        match self {
            Self::Amount => observation.cumulative_amount().map(Money::get).ok_or(
                MonitorError::FamilyUnavailable {
                    family: ObservationFamily::CumulativeAmount,
                },
            ),
            Self::Volume(_) => observation.cumulative_volume().map(Quantity::get).ok_or(
                MonitorError::FamilyUnavailable {
                    family: ObservationFamily::CumulativeVolume,
                },
            ),
        }
    }

    fn rollback_reason(self) -> ResetReason {
        match self {
            Self::Amount => ResetReason::CumulativeAmountRollback,
            Self::Volume(_) => ResetReason::CumulativeVolumeRollback,
        }
    }

    fn cumulative_evidence(
        self,
        evidence: &WindowEndpointEvidence,
        first: bool,
    ) -> Result<f64, CoreError> {
        match (self, first) {
            (Self::Amount, true) => evidence.first_cumulative_amount().map(Money::get),
            (Self::Amount, false) => evidence.last_cumulative_amount().map(Money::get),
            (Self::Volume(expected), true) => evidence
                .first_cumulative_volume()
                .zip(evidence.first_cumulative_volume_unit())
                .filter(|(_, actual)| *actual == expected)
                .map(|(value, _)| value.get()),
            (Self::Volume(expected), false) => evidence
                .last_cumulative_volume()
                .zip(evidence.last_cumulative_volume_unit())
                .filter(|(_, actual)| *actual == expected)
                .map(|(value, _)| value.get()),
        }
        .ok_or_else(|| {
            CoreError::InvalidRequest(format!(
                "{} endpoint evidence is unavailable",
                self.observation_family()
            ))
        })
    }
}

struct DeltaMonitor {
    limits: MonitorLimits,
    rule: DeltaRule,
    family: DeltaFamily,
    instruments: HashMap<InstrumentId, DeltaState>,
}

impl DeltaMonitor {
    fn new(limits: MonitorLimits, rule: DeltaRule, family: DeltaFamily) -> Self {
        Self {
            limits,
            rule,
            family,
            instruments: HashMap::new(),
        }
    }

    fn process(&mut self, observation: InjectedObservation) -> Result<DeltaOutcome, MonitorError> {
        self.family.validate_observation(&observation)?;
        let instrument = observation.instrument().clone();
        if !self.instruments.contains_key(&instrument)
            && self.instruments.len() >= usize::from(self.limits.max_instruments())
        {
            return Err(MonitorError::InstrumentLimit {
                limit: self.limits.max_instruments(),
            });
        }
        self.instruments
            .entry(instrument)
            .or_insert_with(DeltaState::new)
            .process(
                observation,
                self.rule,
                self.family,
                self.limits.window_capacity(),
            )
    }

    fn inject_reset(
        &mut self,
        instrument: &InstrumentId,
        signal: InjectedResetSignal,
    ) -> Option<DeltaOutcome> {
        let state = self.instruments.get_mut(instrument)?;
        state.reset_without_seed();
        Some(DeltaOutcome {
            state: state.rule_state,
            transition: Some(DeltaTransition::Reset(signal.into())),
            evidence: None,
        })
    }

    fn state(&self, instrument: &InstrumentId) -> Option<RuleState> {
        self.instruments
            .get(instrument)
            .map(|state| state.rule_state)
    }

    fn window_len(&self, instrument: &InstrumentId) -> Option<usize> {
        self.instruments
            .get(instrument)
            .map(|state| state.window.len())
    }

    fn window_evidence(&self, instrument: &InstrumentId) -> Option<WindowEndpointEvidence> {
        let window = &self.instruments.get(instrument)?.window;
        Some(WindowEndpointEvidence::new(window.front()?, window.back()?))
    }
}

struct DeltaState {
    rule_state: RuleState,
    window: VecDeque<InjectedObservation>,
    last_arrival_millis: Option<u64>,
    last_cumulative: Option<f64>,
    last_source_record_count: Option<u64>,
    triggered_at_millis: Option<u64>,
}

impl DeltaState {
    fn new() -> Self {
        Self {
            rule_state: RuleState::WarmingUp,
            window: VecDeque::new(),
            last_arrival_millis: None,
            last_cumulative: None,
            last_source_record_count: None,
            triggered_at_millis: None,
        }
    }

    fn process(
        &mut self,
        observation: InjectedObservation,
        rule: DeltaRule,
        family: DeltaFamily,
        window_capacity: u16,
    ) -> Result<DeltaOutcome, MonitorError> {
        self.validate_order(observation.arrival_millis())?;

        if let Some(reason) = self.reset_reason(&observation, family)? {
            self.reset_and_seed(&observation, family)?;
            return Ok(self.outcome(Some(DeltaTransition::Reset(reason)), false));
        }

        self.prune_for(observation.arrival_millis(), rule.window_millis);
        if self.window.len() >= usize::from(window_capacity) {
            self.reset_and_seed(&observation, family)?;
            return Err(MonitorError::WindowOverflow {
                capacity: window_capacity,
            });
        }
        self.seed(&observation, family)?;

        let Some(delta) = self.delta(observation.arrival_millis(), rule, family)? else {
            if self.rule_state != RuleState::WarmingUp {
                self.reset_and_seed(&observation, family)?;
                return Ok(self.outcome(
                    Some(DeltaTransition::Reset(ResetReason::SamplingGap)),
                    false,
                ));
            }
            return Ok(self.outcome(None, false));
        };

        match self.rule_state {
            RuleState::WarmingUp => {
                self.rule_state = RuleState::Armed;
                Ok(self.outcome(Some(DeltaTransition::WarmedUp), true))
            }
            RuleState::Armed if delta >= rule.trigger_delta => {
                self.rule_state = RuleState::Triggered;
                self.triggered_at_millis = Some(observation.arrival_millis());
                Ok(self.outcome(Some(DeltaTransition::Triggered { delta }), true))
            }
            RuleState::Armed => Ok(self.outcome(None, false)),
            RuleState::Triggered => {
                self.rule_state = RuleState::CoolingDown;
                Ok(self.outcome(Some(DeltaTransition::EnteredCoolingDown), true))
            }
            RuleState::CoolingDown => {
                let triggered_at =
                    self.triggered_at_millis
                        .ok_or(MonitorError::InvalidObservation(
                            "cooling state has no trigger time",
                        ))?;
                let cooldown_complete =
                    observation.arrival_millis() - triggered_at >= rule.cooldown_millis;
                if cooldown_complete && delta <= rule.rearm_delta {
                    self.rule_state = RuleState::Armed;
                    self.triggered_at_millis = None;
                    Ok(self.outcome(Some(DeltaTransition::Rearmed { delta }), true))
                } else {
                    Ok(self.outcome(None, false))
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

    fn reset_reason(
        &self,
        observation: &InjectedObservation,
        family: DeltaFamily,
    ) -> Result<Option<ResetReason>, MonitorError> {
        if observation.continuity() != ContinuityState::Continuous {
            return Ok(Some(ResetReason::NonContinuous(observation.continuity())));
        }
        if self
            .last_source_record_count
            .zip(observation.source_record_count())
            .is_some_and(|(previous, actual)| actual < previous)
        {
            return Ok(Some(ResetReason::SourceRecordCountRollback));
        }
        let current = family.cumulative(observation)?;
        Ok(self
            .last_cumulative
            .is_some_and(|previous| current < previous)
            .then(|| family.rollback_reason()))
    }

    fn prune_for(&mut self, arrival_millis: u64, window_millis: u64) {
        let Some(cutoff) = arrival_millis.checked_sub(window_millis) else {
            return;
        };
        while self.window.len() > 1
            && self
                .window
                .get(1)
                .is_some_and(|point| point.arrival_millis() <= cutoff)
        {
            self.window.pop_front();
        }
    }

    fn delta(
        &self,
        arrival_millis: u64,
        rule: DeltaRule,
        family: DeltaFamily,
    ) -> Result<Option<f64>, MonitorError> {
        let Some(cutoff) = arrival_millis.checked_sub(rule.window_millis) else {
            return Ok(None);
        };
        let Some(boundary) = self.window.front() else {
            return Ok(None);
        };
        if boundary.arrival_millis() > cutoff
            || cutoff - boundary.arrival_millis() > rule.boundary_tolerance_millis
        {
            return Ok(None);
        }
        let current = self.window.back().ok_or(MonitorError::InvalidObservation(
            "cumulative-delta window unexpectedly has no current observation",
        ))?;
        let delta = family.cumulative(current)? - family.cumulative(boundary)?;
        if !delta.is_finite() || delta < 0.0 {
            return Err(MonitorError::CumulativeDeltaOverflow);
        }
        Ok(Some(delta))
    }

    fn reset_and_seed(
        &mut self,
        observation: &InjectedObservation,
        family: DeltaFamily,
    ) -> Result<(), MonitorError> {
        self.reset_without_seed();
        self.seed(observation, family)
    }

    fn reset_without_seed(&mut self) {
        self.rule_state = RuleState::WarmingUp;
        self.window.clear();
        self.last_arrival_millis = None;
        self.last_cumulative = None;
        self.last_source_record_count = None;
        self.triggered_at_millis = None;
    }

    fn seed(
        &mut self,
        observation: &InjectedObservation,
        family: DeltaFamily,
    ) -> Result<(), MonitorError> {
        let cumulative = family.cumulative(observation)?;
        self.window.push_back(observation.clone());
        self.last_arrival_millis = Some(observation.arrival_millis());
        self.last_cumulative = Some(cumulative);
        self.last_source_record_count = observation.source_record_count();
        Ok(())
    }

    fn outcome(&self, transition: Option<DeltaTransition>, retain_evidence: bool) -> DeltaOutcome {
        DeltaOutcome {
            state: self.rule_state,
            transition,
            evidence: retain_evidence.then(|| {
                WindowEndpointEvidence::new(
                    self.window
                        .front()
                        .expect("transition window has a boundary"),
                    self.window
                        .back()
                        .expect("transition window has a current point"),
                )
            }),
        }
    }
}

pub struct DeterministicAmountMonitor(DeltaMonitor);

impl DeterministicAmountMonitor {
    pub fn new(limits: MonitorLimits, rule: AmountSpikeRule) -> Self {
        Self(DeltaMonitor::new(limits, rule.0, DeltaFamily::Amount))
    }

    pub fn process(
        &mut self,
        observation: InjectedObservation,
    ) -> Result<AmountProcessOutcome, MonitorError> {
        let outcome = self.0.process(observation)?;
        Ok(AmountProcessOutcome {
            state: outcome.state,
            transition: outcome.transition.map(|transition| match transition {
                DeltaTransition::WarmedUp => AmountTransition::WarmedUp,
                DeltaTransition::Triggered { delta } => AmountTransition::Triggered {
                    amount_delta: Money::new(delta).expect("validated amount delta"),
                },
                DeltaTransition::EnteredCoolingDown => AmountTransition::EnteredCoolingDown,
                DeltaTransition::Rearmed { delta } => AmountTransition::Rearmed {
                    amount_delta: Money::new(delta).expect("validated amount delta"),
                },
                DeltaTransition::Reset(reason) => AmountTransition::Reset(reason),
            }),
            evidence: outcome.evidence,
        })
    }

    pub fn inject_reset(
        &mut self,
        instrument: &InstrumentId,
        signal: InjectedResetSignal,
    ) -> Option<AmountProcessOutcome> {
        let outcome = self.0.inject_reset(instrument, signal)?;
        Some(AmountProcessOutcome {
            state: outcome.state,
            transition: outcome.transition.map(|transition| match transition {
                DeltaTransition::Reset(reason) => AmountTransition::Reset(reason),
                _ => unreachable!("injected reset only emits reset"),
            }),
            evidence: outcome.evidence,
        })
    }

    pub fn state(&self, instrument: &InstrumentId) -> Option<RuleState> {
        self.0.state(instrument)
    }

    pub fn window_len(&self, instrument: &InstrumentId) -> Option<usize> {
        self.0.window_len(instrument)
    }

    pub fn window_evidence(&self, instrument: &InstrumentId) -> Option<WindowEndpointEvidence> {
        self.0.window_evidence(instrument)
    }
}

pub struct DeterministicVolumeMonitor(DeltaMonitor);

impl DeterministicVolumeMonitor {
    pub fn new(limits: MonitorLimits, rule: VolumeSpikeRule) -> Self {
        Self(DeltaMonitor::new(
            limits,
            rule.rule,
            DeltaFamily::Volume(rule.unit),
        ))
    }

    pub fn process(
        &mut self,
        observation: InjectedObservation,
    ) -> Result<VolumeProcessOutcome, MonitorError> {
        let unit = match self.0.family {
            DeltaFamily::Volume(unit) => unit,
            DeltaFamily::Amount => {
                return Err(MonitorError::InvalidConfiguration(
                    "volume monitor has an amount family",
                ))
            }
        };
        let outcome = self.0.process(observation)?;
        Ok(VolumeProcessOutcome {
            state: outcome.state,
            transition: outcome.transition.map(|transition| match transition {
                DeltaTransition::WarmedUp => VolumeTransition::WarmedUp,
                DeltaTransition::Triggered { delta } => VolumeTransition::Triggered {
                    volume_delta: Quantity::new(delta).expect("validated volume delta"),
                    unit,
                },
                DeltaTransition::EnteredCoolingDown => VolumeTransition::EnteredCoolingDown,
                DeltaTransition::Rearmed { delta } => VolumeTransition::Rearmed {
                    volume_delta: Quantity::new(delta).expect("validated volume delta"),
                    unit,
                },
                DeltaTransition::Reset(reason) => VolumeTransition::Reset(reason),
            }),
            evidence: outcome.evidence,
        })
    }

    pub fn inject_reset(
        &mut self,
        instrument: &InstrumentId,
        signal: InjectedResetSignal,
    ) -> Option<VolumeProcessOutcome> {
        let outcome = self.0.inject_reset(instrument, signal)?;
        Some(VolumeProcessOutcome {
            state: outcome.state,
            transition: outcome.transition.map(|transition| match transition {
                DeltaTransition::Reset(reason) => VolumeTransition::Reset(reason),
                _ => unreachable!("injected reset only emits reset"),
            }),
            evidence: outcome.evidence,
        })
    }

    pub fn state(&self, instrument: &InstrumentId) -> Option<RuleState> {
        self.0.state(instrument)
    }

    pub fn window_len(&self, instrument: &InstrumentId) -> Option<usize> {
        self.0.window_len(instrument)
    }

    pub fn window_evidence(&self, instrument: &InstrumentId) -> Option<WindowEndpointEvidence> {
        self.0.window_evidence(instrument)
    }
}
