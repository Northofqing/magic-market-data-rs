use magic_market_core::{
    AnomalyEvent, AssetClass, ContinuityState, Exchange, InstrumentId, Money, Price, ProviderId,
    Quantity, SourceEvidence, StreamContinuity, StreamCursor, StreamGeneration, StreamSequence,
};
use magic_market_monitor::{
    AmountSpikeRule, AmountTransition, DeterministicAmountMonitor, DeterministicPriceMonitor,
    DeterministicVolumeMonitor, InjectedObservation, MonitorLimits, MonitorTransition,
    PriceChangeRule, ResetReason as MonitorResetReason, RuleState, SourceQuantityUnit,
    VolumeSpikeRule, VolumeTransition,
};
use magic_tdx_local_rs::SourceExchange;
use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuleLimits(MonitorLimits);

impl RuleLimits {
    pub(crate) fn new(max_instruments: u16, window_capacity: u16) -> Result<Self, String> {
        MonitorLimits::new(max_instruments, window_capacity)
            .map(Self)
            .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PriceRule {
    inner: PriceChangeRule,
    version: u32,
}

impl PriceRule {
    pub(crate) fn new(
        version: u32,
        window_millis: u64,
        boundary_tolerance_millis: u64,
        trigger_ratio: f64,
        rearm_ratio: f64,
        cooldown_millis: u64,
    ) -> Result<Self, String> {
        if version == 0 {
            return Err("price rule version must be positive".to_owned());
        }
        PriceChangeRule::new(
            window_millis,
            boundary_tolerance_millis,
            trigger_ratio,
            rearm_ratio,
            cooldown_millis,
        )
        .map(|inner| Self { inner, version })
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VolumeRule {
    inner: VolumeSpikeRule,
    version: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AmountRule {
    inner: AmountSpikeRule,
    version: u32,
}

impl AmountRule {
    pub(crate) fn new(
        version: u32,
        window_millis: u64,
        boundary_tolerance_millis: u64,
        trigger_delta: f64,
        rearm_delta: f64,
        cooldown_millis: u64,
    ) -> Result<Self, String> {
        let trigger = Money::new(trigger_delta).map_err(|error| error.to_string())?;
        let rearm = Money::new(rearm_delta).map_err(|error| error.to_string())?;
        AmountSpikeRule::new(
            version,
            window_millis,
            boundary_tolerance_millis,
            trigger,
            rearm,
            cooldown_millis,
        )
        .map(|inner| Self { inner, version })
        .map_err(|error| error.to_string())
    }
}

impl VolumeRule {
    pub(crate) fn new(
        version: u32,
        window_millis: u64,
        boundary_tolerance_millis: u64,
        trigger_delta: f64,
        rearm_delta: f64,
        cooldown_millis: u64,
    ) -> Result<Self, String> {
        let trigger = Quantity::new(trigger_delta).map_err(|error| error.to_string())?;
        let rearm = Quantity::new(rearm_delta).map_err(|error| error.to_string())?;
        VolumeSpikeRule::new(
            version,
            window_millis,
            boundary_tolerance_millis,
            trigger,
            rearm,
            SourceQuantityUnit::Lot,
            cooldown_millis,
        )
        .map(|inner| Self { inner, version })
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnalysisFamily {
    Price,
    Amount,
    Volume,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PublicRuleState {
    WarmingUp,
    Armed,
    Triggered,
    CoolingDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum AnalysisTransition {
    WarmedUp,
    Triggered {
        value: f64,
    },
    EnteredCoolingDown,
    Rearmed {
        value: f64,
    },
    Reset {
        #[serde(flatten)]
        reason: AnalysisResetReason,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub(crate) enum AnalysisResetReason {
    NonContinuous { continuity: ContinuityState },
    CumulativeAmountRollback,
    CumulativeVolumeRollback,
    SourceRecordCountRollback,
    SamplingGap,
    TradingDateChanged,
    SessionBoundary,
    MiddayBreak,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct AnalysisUpdate {
    pub(crate) family: AnalysisFamily,
    pub(crate) instrument: String,
    pub(crate) state: PublicRuleState,
    pub(crate) transition: AnalysisTransition,
    pub(crate) value_unit: &'static str,
    pub(crate) rule_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) anomaly_event: Option<AnomalyEvent>,
}

pub(crate) struct AnalysisInput<'a> {
    pub(crate) instrument_label: &'a str,
    pub(crate) exchange: SourceExchange,
    pub(crate) code: &'a str,
    pub(crate) observed_at_utc: &'a str,
    pub(crate) arrival_millis: u64,
    pub(crate) generation: u64,
    pub(crate) sequence: u64,
    pub(crate) price: Option<f64>,
    pub(crate) cumulative_volume_lots: Option<f64>,
}

pub(crate) struct AmountAnalysisInput<'a> {
    pub(crate) instrument_label: &'a str,
    pub(crate) exchange: SourceExchange,
    pub(crate) code: &'a str,
    pub(crate) observed_at_utc: &'a str,
    pub(crate) arrival_millis: u64,
    pub(crate) generation: u64,
    pub(crate) sequence: u64,
    pub(crate) cumulative_amount_cny: f64,
}

pub(crate) struct Analyzers {
    limits: RuleLimits,
    price_rule: PriceRule,
    volume_rule: VolumeRule,
    amount_rule: AmountRule,
    price: DeterministicPriceMonitor,
    amount: DeterministicAmountMonitor,
    volume: DeterministicVolumeMonitor,
    volume_rule_version: u32,
    fast_active: bool,
    amount_active: bool,
}

impl Analyzers {
    pub(crate) fn new(
        limits: RuleLimits,
        price: PriceRule,
        amount: AmountRule,
        volume: VolumeRule,
    ) -> Self {
        Self {
            limits,
            price_rule: price,
            amount_rule: amount,
            volume_rule: volume,
            price: DeterministicPriceMonitor::new(limits.0, price.inner),
            amount: DeterministicAmountMonitor::new(limits.0, amount.inner),
            volume: DeterministicVolumeMonitor::new(limits.0, volume.inner),
            volume_rule_version: volume.version,
            fast_active: false,
            amount_active: false,
        }
    }

    pub(crate) fn process(
        &mut self,
        input: AnalysisInput<'_>,
    ) -> Result<Vec<AnalysisUpdate>, String> {
        let instrument =
            InstrumentId::new(exchange(input.exchange), input.code, AssetClass::Equity)
                .map_err(|error| error.to_string())?;
        let evidence = SourceEvidence::new(
            ProviderId::LocalTerminal,
            input.observed_at_utc,
            format!("tdx-loopback-{}-{}", input.generation, input.sequence),
        )
        .map_err(|error| error.to_string())?;
        let cursor = stream_cursor(input.generation, input.sequence, 0)?;
        let mut updates = Vec::new();
        if let Some(value) = input.price {
            let price = Price::new(value).map_err(|error| error.to_string())?;
            let observation = InjectedObservation::from_families(
                instrument.clone(),
                evidence.clone(),
                input.arrival_millis,
                Some(price),
                None,
                None,
                None,
                ContinuityState::Continuous,
            )
            .map_err(|error| error.to_string())?
            .with_stream_cursor(cursor.clone());
            let outcome = self
                .price
                .process(observation)
                .map_err(|error| error.to_string())?;
            self.fast_active = true;
            if let Some(transition) = outcome.transition() {
                let anomaly_event = match transition {
                    MonitorTransition::Triggered { .. } | MonitorTransition::Rearmed { .. } => {
                        Some(
                            self.price
                                .window_evidence(&instrument)
                                .ok_or_else(|| {
                                    "price anomaly transition has no window evidence".to_owned()
                                })?
                                .price_core_event(
                                    self.price_rule.inner,
                                    self.price_rule.version,
                                    transition,
                                    cursor.clone(),
                                    anomaly_continuity(),
                                    input.observed_at_utc,
                                )
                                .map_err(|error| error.to_string())?,
                        )
                    }
                    _ => None,
                };
                updates.push(AnalysisUpdate {
                    family: AnalysisFamily::Price,
                    instrument: input.instrument_label.to_owned(),
                    state: state(outcome.state()),
                    transition: price_transition(transition),
                    value_unit: "ratio",
                    rule_version: self.price_rule.version,
                    anomaly_event,
                });
            }
        }
        if let Some(value) = input.cumulative_volume_lots {
            let volume = Quantity::new(value).map_err(|error| error.to_string())?;
            let observation = InjectedObservation::from_families(
                instrument.clone(),
                evidence.clone(),
                input.arrival_millis,
                None,
                None,
                Some(volume),
                Some(SourceQuantityUnit::Lot),
                ContinuityState::Continuous,
            )
            .map_err(|error| error.to_string())?
            .with_stream_cursor(cursor.clone());
            let outcome = self
                .volume
                .process(observation)
                .map_err(|error| error.to_string())?;
            self.fast_active = true;
            if let Some(transition) = outcome.transition() {
                let anomaly_event = match transition {
                    VolumeTransition::Triggered { .. } | VolumeTransition::Rearmed { .. } => Some(
                        outcome
                            .core_event(
                                instrument.clone(),
                                self.volume_rule.inner,
                                cursor,
                                anomaly_continuity(),
                                input.observed_at_utc,
                            )
                            .map_err(|error| error.to_string())?,
                    ),
                    _ => None,
                };
                updates.push(AnalysisUpdate {
                    family: AnalysisFamily::Volume,
                    instrument: input.instrument_label.to_owned(),
                    state: state(outcome.state()),
                    transition: volume_transition(transition),
                    value_unit: "lot",
                    rule_version: self.volume_rule_version,
                    anomaly_event,
                });
            }
        }
        Ok(updates)
    }

    pub(crate) fn process_amount(
        &mut self,
        input: AmountAnalysisInput<'_>,
    ) -> Result<Vec<AnalysisUpdate>, String> {
        let instrument =
            InstrumentId::new(exchange(input.exchange), input.code, AssetClass::Equity)
                .map_err(|error| error.to_string())?;
        let evidence = SourceEvidence::new(
            ProviderId::LocalTerminal,
            input.observed_at_utc,
            format!(
                "tdx-loopback-snapshot-{}-{}",
                input.generation, input.sequence
            ),
        )
        .map_err(|error| error.to_string())?;
        let amount = Money::new(input.cumulative_amount_cny).map_err(|error| error.to_string())?;
        let observation = InjectedObservation::from_families(
            instrument.clone(),
            evidence,
            input.arrival_millis,
            None,
            Some(amount),
            None,
            None,
            ContinuityState::Continuous,
        )
        .map_err(|error| error.to_string())?
        .with_stream_cursor(stream_cursor(input.generation, input.sequence, 1)?);
        let outcome = self
            .amount
            .process(observation)
            .map_err(|error| error.to_string())?;
        self.amount_active = true;
        let output_cursor = stream_cursor(input.generation, input.sequence, 1)?;
        Ok(outcome
            .transition()
            .map(|transition| {
                let anomaly_event = match transition {
                    AmountTransition::Triggered { .. } | AmountTransition::Rearmed { .. } => Some(
                        outcome
                            .core_event(
                                instrument,
                                self.amount_rule.inner,
                                output_cursor,
                                anomaly_continuity(),
                                input.observed_at_utc,
                            )
                            .map_err(|error| error.to_string())?,
                    ),
                    _ => None,
                };
                Ok::<AnalysisUpdate, String>(AnalysisUpdate {
                    family: AnalysisFamily::Amount,
                    instrument: input.instrument_label.to_owned(),
                    state: state(outcome.state()),
                    transition: amount_transition(transition),
                    value_unit: "cny",
                    rule_version: self.amount_rule.version,
                    anomaly_event,
                })
            })
            .transpose()?
            .into_iter()
            .collect())
    }

    pub(crate) fn reset_amount(&mut self) -> bool {
        let active = self.amount_active;
        self.amount = DeterministicAmountMonitor::new(self.limits.0, self.amount_rule.inner);
        self.amount_active = false;
        active
    }

    pub(crate) fn reset(&mut self) -> bool {
        let active = self.fast_active || self.amount_active;
        *self = Self::new(
            self.limits,
            self.price_rule,
            self.amount_rule,
            self.volume_rule,
        );
        active
    }
}

fn anomaly_continuity() -> StreamContinuity {
    StreamContinuity::new(ContinuityState::Continuous, ContinuityState::Unknown)
}

fn exchange(value: SourceExchange) -> Exchange {
    match value {
        SourceExchange::Shanghai => Exchange::Shanghai,
        SourceExchange::Shenzhen => Exchange::Shenzhen,
        SourceExchange::Beijing => Exchange::Beijing,
    }
}

fn stream_cursor(generation: u64, sequence: u64, family: u8) -> Result<StreamCursor, String> {
    let high = generation >> 48;
    let low = generation & 0x0000_ffff_ffff_ffff;
    let generation = StreamGeneration::new(format!(
        "00000000-0000-00{family:02x}-{high:04x}-{low:012x}"
    ))
    .map_err(|error| error.to_string())?;
    let sequence = StreamSequence::new(sequence).map_err(|error| error.to_string())?;
    Ok(StreamCursor::new(generation, sequence))
}

fn amount_transition(value: &AmountTransition) -> AnalysisTransition {
    match value {
        AmountTransition::WarmedUp => AnalysisTransition::WarmedUp,
        AmountTransition::Triggered { amount_delta } => AnalysisTransition::Triggered {
            value: amount_delta.get(),
        },
        AmountTransition::EnteredCoolingDown => AnalysisTransition::EnteredCoolingDown,
        AmountTransition::Rearmed { amount_delta } => AnalysisTransition::Rearmed {
            value: amount_delta.get(),
        },
        AmountTransition::Reset(reason) => AnalysisTransition::Reset {
            reason: reset_reason(*reason),
        },
    }
}

fn state(value: RuleState) -> PublicRuleState {
    match value {
        RuleState::WarmingUp => PublicRuleState::WarmingUp,
        RuleState::Armed => PublicRuleState::Armed,
        RuleState::Triggered => PublicRuleState::Triggered,
        RuleState::CoolingDown => PublicRuleState::CoolingDown,
    }
}

fn price_transition(value: MonitorTransition) -> AnalysisTransition {
    match value {
        MonitorTransition::WarmedUp => AnalysisTransition::WarmedUp,
        MonitorTransition::Triggered { change_ratio } => AnalysisTransition::Triggered {
            value: change_ratio,
        },
        MonitorTransition::EnteredCoolingDown => AnalysisTransition::EnteredCoolingDown,
        MonitorTransition::Rearmed { change_ratio } => AnalysisTransition::Rearmed {
            value: change_ratio,
        },
        MonitorTransition::Reset(reason) => AnalysisTransition::Reset {
            reason: reset_reason(reason),
        },
    }
}

fn volume_transition(value: &VolumeTransition) -> AnalysisTransition {
    match value {
        VolumeTransition::WarmedUp => AnalysisTransition::WarmedUp,
        VolumeTransition::Triggered { volume_delta, .. } => AnalysisTransition::Triggered {
            value: volume_delta.get(),
        },
        VolumeTransition::EnteredCoolingDown => AnalysisTransition::EnteredCoolingDown,
        VolumeTransition::Rearmed { volume_delta, .. } => AnalysisTransition::Rearmed {
            value: volume_delta.get(),
        },
        VolumeTransition::Reset(reason) => AnalysisTransition::Reset {
            reason: reset_reason(*reason),
        },
    }
}

fn reset_reason(value: MonitorResetReason) -> AnalysisResetReason {
    match value {
        MonitorResetReason::NonContinuous(continuity) => {
            AnalysisResetReason::NonContinuous { continuity }
        }
        MonitorResetReason::CumulativeAmountRollback => {
            AnalysisResetReason::CumulativeAmountRollback
        }
        MonitorResetReason::CumulativeVolumeRollback => {
            AnalysisResetReason::CumulativeVolumeRollback
        }
        MonitorResetReason::SourceRecordCountRollback => {
            AnalysisResetReason::SourceRecordCountRollback
        }
        MonitorResetReason::SamplingGap => AnalysisResetReason::SamplingGap,
        MonitorResetReason::TradingDateChanged => AnalysisResetReason::TradingDateChanged,
        MonitorResetReason::SessionBoundary => AnalysisResetReason::SessionBoundary,
        MonitorResetReason::MiddayBreak => AnalysisResetReason::MiddayBreak,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_keeps_amount_unavailable_and_volume_unit_lot() {
        let limits = RuleLimits::new(1, 8).unwrap();
        let price = PriceRule::new(1, 100, 0, 0.1, 0.02, 10).unwrap();
        let amount = AmountRule::new(1, 100, 0, 50.0, 10.0, 10).unwrap();
        let volume = VolumeRule::new(1, 100, 0, 50.0, 10.0, 10).unwrap();
        let mut analyzers = Analyzers::new(limits, price, amount, volume);
        let input = |arrival, sequence, price_value, volume_value| AnalysisInput {
            instrument_label: "SH:600000",
            exchange: SourceExchange::Shanghai,
            code: "600000",
            observed_at_utc: "2026-08-13T01:02:03Z",
            arrival_millis: arrival,
            generation: 1,
            sequence,
            price: Some(price_value),
            cumulative_volume_lots: Some(volume_value),
        };
        let initial = analyzers.process(input(1, 1, 10.0, 100.0)).unwrap();
        assert!(initial
            .iter()
            .all(|update| update.value_unit == "ratio" || update.value_unit == "lot"));
        let warmed = analyzers.process(input(101, 2, 10.0, 160.0)).unwrap();
        assert!(warmed
            .iter()
            .any(|update| update.family == AnalysisFamily::Volume && update.value_unit == "lot"));
        assert!(warmed
            .iter()
            .any(|update| update.transition == AnalysisTransition::WarmedUp));
        assert!(!initial
            .iter()
            .chain(&warmed)
            .any(|update| matches!(update.transition, AnalysisTransition::Reset { .. })));

        let triggered = analyzers.process(input(201, 3, 12.0, 230.0)).unwrap();
        assert!(triggered
            .iter()
            .any(|update| matches!(update.transition, AnalysisTransition::Triggered { .. })));
        assert!(!triggered
            .iter()
            .any(|update| matches!(update.transition, AnalysisTransition::Reset { .. })));

        let gap = analyzers.process(input(1_000, 4, 12.0, 230.0)).unwrap();
        assert!(gap.iter().all(|update| matches!(
            update.transition,
            AnalysisTransition::Reset {
                reason: AnalysisResetReason::SamplingGap
            }
        )));
    }

    #[test]
    fn amount_path_is_independent_and_uses_cny() {
        let limits = RuleLimits::new(1, 8).unwrap();
        let price = PriceRule::new(1, 100, 0, 0.1, 0.02, 10).unwrap();
        let amount = AmountRule::new(2, 100, 0, 50.0, 10.0, 10).unwrap();
        let volume = VolumeRule::new(1, 100, 0, 50.0, 10.0, 10).unwrap();
        let mut analyzers = Analyzers::new(limits, price, amount, volume);
        let input = |arrival, sequence, value| AmountAnalysisInput {
            instrument_label: "EQUITY:SH:600000",
            exchange: SourceExchange::Shanghai,
            code: "600000",
            observed_at_utc: "2026-08-13T01:02:03Z",
            arrival_millis: arrival,
            generation: 1,
            sequence,
            cumulative_amount_cny: value,
        };
        let _ = analyzers.process_amount(input(1, 1, 100.0)).unwrap();
        let warmed = analyzers.process_amount(input(101, 2, 160.0)).unwrap();
        assert!(warmed.iter().any(|update| {
            update.family == AnalysisFamily::Amount
                && update.transition == AnalysisTransition::WarmedUp
                && update.value_unit == "cny"
                && update.rule_version == 2
        }));
        let triggered = analyzers.process_amount(input(201, 3, 230.0)).unwrap();
        assert!(triggered.iter().any(|update| matches!(
            update.transition,
            AnalysisTransition::Triggered { value: 70.0 }
        )));
        assert!(triggered.iter().any(|update| {
            update
                .anomaly_event
                .as_ref()
                .is_some_and(|event| event.rule().id() == "local_amount_spike")
        }));
    }

    #[test]
    fn reset_transition_serializes_the_exact_reason() {
        let transition = AnalysisTransition::Reset {
            reason: AnalysisResetReason::SamplingGap,
        };
        let value = serde_json::to_value(transition).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"kind": "reset", "reason": "sampling_gap"})
        );
    }

    #[test]
    fn price_adapter_completes_trigger_cooldown_and_rearm_without_cross_instrument_leakage() {
        let limits = RuleLimits::new(2, 16).unwrap();
        let price = PriceRule::new(7, 100, 0, 0.1, 0.02, 10).unwrap();
        let amount = AmountRule::new(1, 100, 0, 50.0, 10.0, 10).unwrap();
        let volume = VolumeRule::new(1, 100, 0, 1_000.0, 100.0, 10).unwrap();
        let mut analyzers = Analyzers::new(limits, price, amount, volume);
        let input = |label: &'static str, code: &'static str, arrival, sequence, price_value| {
            AnalysisInput {
                instrument_label: label,
                exchange: SourceExchange::Shanghai,
                code,
                observed_at_utc: "2026-08-13T05:01:00Z",
                arrival_millis: arrival,
                generation: 1,
                sequence,
                price: Some(price_value),
                cumulative_volume_lots: Some(100.0),
            }
        };

        let mut primary = Vec::new();
        for (arrival, sequence, value) in [
            (1, 1, 10.0),
            (101, 2, 10.0),
            (201, 3, 12.0),
            (301, 4, 12.0),
            (401, 5, 12.0),
        ] {
            primary.extend(
                analyzers
                    .process(input(
                        "EQUITY:SH:600000",
                        "600000",
                        arrival,
                        sequence,
                        value,
                    ))
                    .unwrap()
                    .into_iter()
                    .filter(|update| update.family == AnalysisFamily::Price),
            );
        }

        assert_eq!(
            primary
                .iter()
                .map(|update| update.transition)
                .collect::<Vec<_>>(),
            vec![
                AnalysisTransition::WarmedUp,
                AnalysisTransition::Triggered { value: 0.2 },
                AnalysisTransition::EnteredCoolingDown,
                AnalysisTransition::Rearmed { value: 0.0 },
            ]
        );
        assert!(primary.iter().all(|update| {
            update.instrument == "EQUITY:SH:600000"
                && update.rule_version == 7
                && update.value_unit == "ratio"
        }));
        assert!(primary.iter().all(|update| {
            let is_public = matches!(
                update.transition,
                AnalysisTransition::Triggered { .. } | AnalysisTransition::Rearmed { .. }
            );
            update.anomaly_event.is_some() == is_public
        }));
        assert!(
            primary
                .iter()
                .filter_map(|update| update.anomaly_event.as_ref())
                .all(|event| event.rule().id() == "local_price_change"
                    && event.rule().revision() == 7)
        );

        let quiet = [
            (1, 6, 20.0),
            (101, 7, 20.0),
            (201, 8, 20.0),
            (301, 9, 20.0),
            (401, 10, 20.0),
        ]
        .into_iter()
        .flat_map(|(arrival, sequence, value)| {
            analyzers
                .process(input(
                    "EQUITY:SH:600001",
                    "600001",
                    arrival,
                    sequence,
                    value,
                ))
                .unwrap()
        })
        .filter(|update| update.family == AnalysisFamily::Price)
        .collect::<Vec<_>>();
        assert_eq!(quiet.len(), 1);
        assert_eq!(quiet[0].transition, AnalysisTransition::WarmedUp);
    }
}
