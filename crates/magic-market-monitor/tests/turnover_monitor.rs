use magic_market_core::{
    AssetClass, ContinuityState, Exchange, InstrumentId, Money, Price, ProviderId, Quantity,
    SourceEvidence, StreamContinuity, StreamCursor, StreamGeneration, StreamSequence,
};
use magic_market_monitor::{
    AmountSpikeRule, AmountTransition, DeterministicAmountMonitor, DeterministicPriceMonitor,
    DeterministicVolumeMonitor, InjectedObservation, InjectedResetSignal, MonitorError,
    MonitorLimits, ObservationFamily, PriceChangeRule, ResetReason, RuleState, SourceQuantityUnit,
    VolumeSpikeRule, VolumeTransition, LOCAL_AMOUNT_CHANGE_ANOMALY_ADMITTED,
    LOCAL_PRICE_CHANGE_ANOMALY_ADMITTED, LOCAL_VOLUME_CHANGE_ANOMALY_ADMITTED,
};

fn stock() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap()
}

fn observation(
    time: u64,
    amount: f64,
    volume: f64,
    source_record_count: Option<u64>,
) -> InjectedObservation {
    observation_with_unit(
        time,
        amount,
        volume,
        SourceQuantityUnit::Lot,
        source_record_count,
    )
}

fn observation_with_unit(
    time: u64,
    amount: f64,
    volume: f64,
    volume_unit: SourceQuantityUnit,
    source_record_count: Option<u64>,
) -> InjectedObservation {
    let observation = InjectedObservation::new(
        stock(),
        SourceEvidence::new(
            ProviderId::LocalTerminal,
            format!("2026-08-12T10:21:15.{time:03}+08:00"),
            format!("terminal-{time}"),
        )
        .unwrap(),
        time,
        Price::new(100.0).unwrap(),
        Money::new(amount).unwrap(),
        Quantity::new(volume).unwrap(),
        volume_unit,
        ContinuityState::Continuous,
    )
    .unwrap();
    let observation = match source_record_count {
        Some(count) => observation.with_source_record_count(count),
        None => observation,
    };
    observation.with_stream_cursor(cursor(time + 1))
}

fn selective_observation(
    time: u64,
    price: Option<f64>,
    amount: Option<f64>,
    volume: Option<f64>,
) -> InjectedObservation {
    InjectedObservation::from_families(
        stock(),
        SourceEvidence::new(
            ProviderId::LocalTerminal,
            format!("2026-08-12T10:21:15.{time:03}+08:00"),
            format!("terminal-selective-{time}"),
        )
        .unwrap(),
        time,
        price.map(|value| Price::new(value).unwrap()),
        amount.map(|value| Money::new(value).unwrap()),
        volume.map(|value| Quantity::new(value).unwrap()),
        volume.map(|_| SourceQuantityUnit::Lot),
        ContinuityState::Continuous,
    )
    .unwrap()
}

fn cursor(sequence: u64) -> StreamCursor {
    StreamCursor::new(
        StreamGeneration::new("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        StreamSequence::new(sequence).unwrap(),
    )
}

fn limits() -> MonitorLimits {
    MonitorLimits::new(1, 16).unwrap()
}

#[test]
fn amount_and_volume_are_independent_deterministic_families() {
    let amount_rule = AmountSpikeRule::new(
        1,
        100,
        0,
        Money::new(50.0).unwrap(),
        Money::new(20.0).unwrap(),
        5,
    )
    .unwrap();
    let volume_rule = VolumeSpikeRule::new(
        1,
        100,
        0,
        Quantity::new(500.0).unwrap(),
        Quantity::new(200.0).unwrap(),
        SourceQuantityUnit::Lot,
        5,
    )
    .unwrap();
    assert_eq!(AmountSpikeRule::ID, "local_amount_spike");
    assert_eq!(amount_rule.version(), 1);
    assert_eq!(VolumeSpikeRule::ID, "local_volume_spike");
    assert_eq!(volume_rule.version(), 1);

    let mut amount = DeterministicAmountMonitor::new(limits(), amount_rule);
    let mut volume = DeterministicVolumeMonitor::new(limits(), volume_rule);
    for input in [
        observation(0, 100.0, 1_000.0, Some(10)),
        observation(100, 110.0, 1_100.0, Some(12)),
    ] {
        amount.process(input.clone()).unwrap();
        volume.process(input).unwrap();
    }
    let amount_trigger = amount
        .process(observation(200, 170.0, 1_110.0, Some(13)))
        .unwrap();
    let volume_no_trigger = volume
        .process(observation(200, 170.0, 1_110.0, Some(13)))
        .unwrap();

    assert!(matches!(
        amount_trigger.transition(),
        Some(AmountTransition::Triggered { amount_delta }) if amount_delta.get() == 60.0
    ));
    assert_eq!(amount_trigger.state(), RuleState::Triggered);
    assert_eq!(volume_no_trigger.transition(), None);
    assert_eq!(volume_no_trigger.state(), RuleState::Armed);
}

#[test]
fn transition_evidence_carries_endpoint_counts_and_builds_core_evidence() {
    let rule = AmountSpikeRule::new(
        7,
        100,
        0,
        Money::new(50.0).unwrap(),
        Money::new(20.0).unwrap(),
        5,
    )
    .unwrap();
    let mut monitor = DeterministicAmountMonitor::new(limits(), rule);
    monitor
        .process(observation(0, 100.0, 1_000.0, Some(100)))
        .unwrap();
    monitor
        .process(observation(100, 110.0, 1_100.0, Some(105)))
        .unwrap();
    let triggered = monitor
        .process(observation(200, 170.0, 1_110.0, Some(126)))
        .unwrap();
    let evidence = triggered.evidence().unwrap();

    assert_eq!(evidence.first().batch_id(), "terminal-100");
    assert_eq!(evidence.last().batch_id(), "terminal-200");
    assert_eq!(evidence.first_source_record_count(), Some(105));
    assert_eq!(evidence.last_source_record_count(), Some(126));
    assert_eq!(evidence.observed_source_record_count(), Some(21));
    assert_eq!(
        evidence.core_input_evidence().unwrap().first().provider(),
        ProviderId::LocalTerminal
    );
    let event = evidence
        .core_event_evidence(
            SourceEvidence::new(ProviderId::LocalAnalysis, "derived", "analysis-1").unwrap(),
        )
        .unwrap();
    assert_eq!(event.derived().provider(), ProviderId::LocalAnalysis);
    assert!(evidence
        .core_event_evidence(
            SourceEvidence::new(ProviderId::Tdx, "derived", "impersonation").unwrap()
        )
        .is_err());
    let core_event = triggered
        .core_event(
            stock(),
            rule,
            cursor(201),
            StreamContinuity::new(ContinuityState::Continuous, ContinuityState::Unknown),
            "2026-08-12T10:21:16.000+08:00",
        )
        .unwrap();
    assert_eq!(core_event.rule().id(), AmountSpikeRule::ID);
    assert_eq!(core_event.rule().revision(), 7);
    assert_eq!(core_event.stream().sequence().get(), 201);
    assert_eq!(
        core_event.derived_evidence().provider(),
        ProviderId::LocalAnalysis
    );
}

#[test]
fn absent_record_count_stays_absent_and_rollback_resets() {
    let rule = VolumeSpikeRule::new(
        1,
        100,
        0,
        Quantity::new(50.0).unwrap(),
        Quantity::new(20.0).unwrap(),
        SourceQuantityUnit::Lot,
        5,
    )
    .unwrap();
    let mut monitor = DeterministicVolumeMonitor::new(limits(), rule);
    monitor
        .process(observation(0, 100.0, 1_000.0, None))
        .unwrap();
    let warmed = monitor
        .process(observation(100, 110.0, 1_010.0, None))
        .unwrap();
    assert!(matches!(
        warmed.transition(),
        Some(VolumeTransition::WarmedUp)
    ));
    assert_eq!(
        warmed.evidence().unwrap().observed_source_record_count(),
        None
    );

    let with_count = monitor
        .process(observation(200, 120.0, 1_020.0, Some(10)))
        .unwrap();
    assert_eq!(with_count.state(), RuleState::Armed);
    let rollback = monitor
        .process(observation(201, 130.0, 1_030.0, Some(9)))
        .unwrap();
    assert!(matches!(
        rollback.transition(),
        Some(VolumeTransition::Reset(
            ResetReason::SourceRecordCountRollback
        ))
    ));
    assert_eq!(rollback.state(), RuleState::WarmingUp);
}

#[test]
fn calendar_and_session_resets_are_explicit_injected_signals() {
    let rule = AmountSpikeRule::new(
        1,
        100,
        0,
        Money::new(50.0).unwrap(),
        Money::new(20.0).unwrap(),
        5,
    )
    .unwrap();
    let mut monitor = DeterministicAmountMonitor::new(limits(), rule);
    monitor
        .process(observation(0, 100.0, 1_000.0, Some(1)))
        .unwrap();
    let outcome = monitor
        .inject_reset(&stock(), InjectedResetSignal::MiddayBreak)
        .unwrap();
    assert!(matches!(
        outcome.transition(),
        Some(AmountTransition::Reset(ResetReason::MiddayBreak))
    ));
    assert_eq!(monitor.window_len(&stock()), Some(0));

    monitor
        .process(observation(200, 200.0, 2_000.0, Some(20)))
        .unwrap();
    let outcome = monitor
        .inject_reset(&stock(), InjectedResetSignal::TradingDateChanged)
        .unwrap();
    assert!(matches!(
        outcome.transition(),
        Some(AmountTransition::Reset(ResetReason::TradingDateChanged))
    ));
}

#[test]
fn amount_and_volume_admissions_remain_false() {
    const {
        assert!(!LOCAL_PRICE_CHANGE_ANOMALY_ADMITTED);
        assert!(!LOCAL_AMOUNT_CHANGE_ANOMALY_ADMITTED);
        assert!(!LOCAL_VOLUME_CHANGE_ANOMALY_ADMITTED);
    }
}

#[test]
fn price_volume_without_amount_is_not_fabricated_and_each_family_is_independent() {
    let amount_rule = AmountSpikeRule::new(
        1,
        100,
        0,
        Money::new(50.0).unwrap(),
        Money::new(20.0).unwrap(),
        5,
    )
    .unwrap();
    let volume_rule = VolumeSpikeRule::new(
        1,
        100,
        0,
        Quantity::new(50.0).unwrap(),
        Quantity::new(20.0).unwrap(),
        SourceQuantityUnit::Lot,
        5,
    )
    .unwrap();
    let input = selective_observation(0, Some(100.0), None, Some(1_000.0));
    assert_eq!(input.cumulative_amount(), None);
    assert_eq!(input.cumulative_volume().unwrap().get(), 1_000.0);

    let mut volume = DeterministicVolumeMonitor::new(limits(), volume_rule);
    volume.process(input.clone()).unwrap();
    assert_eq!(volume.window_len(&stock()), Some(1));
    let evidence = volume.window_evidence(&stock()).unwrap();
    assert_eq!(evidence.first_cumulative_amount(), None);
    assert_eq!(evidence.last_cumulative_amount(), None);
    assert_eq!(evidence.first_cumulative_volume().unwrap().get(), 1_000.0);
    assert_eq!(
        evidence.first_cumulative_volume_unit(),
        Some(SourceQuantityUnit::Lot)
    );
    assert_eq!(
        evidence.last_cumulative_volume_unit(),
        Some(SourceQuantityUnit::Lot)
    );

    let mut amount = DeterministicAmountMonitor::new(limits(), amount_rule);
    assert_eq!(
        amount.process(input).unwrap_err(),
        MonitorError::FamilyUnavailable {
            family: ObservationFamily::CumulativeAmount
        }
    );
    assert_eq!(amount.state(&stock()), None);
    assert_eq!(amount.window_len(&stock()), None);
}

#[test]
fn cumulative_rollbacks_reset_only_the_monitor_for_that_family() {
    let amount_rule = AmountSpikeRule::new(
        1,
        100,
        0,
        Money::new(50.0).unwrap(),
        Money::new(20.0).unwrap(),
        5,
    )
    .unwrap();
    let volume_rule = VolumeSpikeRule::new(
        1,
        100,
        0,
        Quantity::new(50.0).unwrap(),
        Quantity::new(20.0).unwrap(),
        SourceQuantityUnit::Lot,
        5,
    )
    .unwrap();
    let mut amount = DeterministicAmountMonitor::new(limits(), amount_rule);
    let mut volume = DeterministicVolumeMonitor::new(limits(), volume_rule);
    let first = selective_observation(0, None, Some(100.0), Some(1_000.0));
    amount.process(first.clone()).unwrap();
    volume.process(first).unwrap();

    let volume_rollback = selective_observation(1, None, Some(110.0), Some(900.0));
    let amount_outcome = amount.process(volume_rollback.clone()).unwrap();
    let volume_outcome = volume.process(volume_rollback).unwrap();
    assert_eq!(amount_outcome.transition(), None);
    assert!(matches!(
        volume_outcome.transition(),
        Some(VolumeTransition::Reset(
            ResetReason::CumulativeVolumeRollback
        ))
    ));
    assert_eq!(amount.window_len(&stock()), Some(2));
    assert_eq!(volume.window_len(&stock()), Some(1));

    let amount_rollback = selective_observation(2, None, Some(90.0), Some(910.0));
    let amount_outcome = amount.process(amount_rollback.clone()).unwrap();
    let volume_outcome = volume.process(amount_rollback).unwrap();
    assert!(matches!(
        amount_outcome.transition(),
        Some(AmountTransition::Reset(
            ResetReason::CumulativeAmountRollback
        ))
    ));
    assert_eq!(volume_outcome.transition(), None);
    assert_eq!(amount.window_len(&stock()), Some(1));
    assert_eq!(volume.window_len(&stock()), Some(2));
}

#[test]
fn unrelated_missing_fields_do_not_block_amount_or_volume() {
    let amount_rule = AmountSpikeRule::new(
        1,
        100,
        0,
        Money::new(50.0).unwrap(),
        Money::new(20.0).unwrap(),
        5,
    )
    .unwrap();
    let volume_rule = VolumeSpikeRule::new(
        1,
        100,
        0,
        Quantity::new(50.0).unwrap(),
        Quantity::new(20.0).unwrap(),
        SourceQuantityUnit::Lot,
        5,
    )
    .unwrap();
    let mut amount = DeterministicAmountMonitor::new(limits(), amount_rule);
    let mut volume = DeterministicVolumeMonitor::new(limits(), volume_rule);

    amount
        .process(selective_observation(0, None, Some(100.0), None))
        .unwrap();
    volume
        .process(selective_observation(0, None, None, Some(100.0)))
        .unwrap();
    assert_eq!(amount.window_len(&stock()), Some(1));
    assert_eq!(volume.window_len(&stock()), Some(1));

    assert_eq!(
        amount
            .process(selective_observation(1, Some(100.0), None, Some(101.0)))
            .unwrap_err(),
        MonitorError::FamilyUnavailable {
            family: ObservationFamily::CumulativeAmount
        }
    );
    assert_eq!(
        volume
            .process(selective_observation(1, Some(100.0), Some(101.0), None))
            .unwrap_err(),
        MonitorError::FamilyUnavailable {
            family: ObservationFamily::CumulativeVolume
        }
    );
    assert_eq!(amount.window_len(&stock()), Some(1));
    assert_eq!(volume.window_len(&stock()), Some(1));
}

#[test]
fn volume_requires_present_matching_unit_without_mutating_state() {
    let rule = VolumeSpikeRule::new(
        1,
        100,
        0,
        Quantity::new(50.0).unwrap(),
        Quantity::new(20.0).unwrap(),
        SourceQuantityUnit::Lot,
        5,
    )
    .unwrap();
    assert_eq!(rule.unit(), SourceQuantityUnit::Lot);
    let mut monitor = DeterministicVolumeMonitor::new(limits(), rule);
    monitor
        .process(observation_with_unit(
            0,
            100.0,
            1_000.0,
            SourceQuantityUnit::Lot,
            None,
        ))
        .unwrap();

    let missing_unit = InjectedObservation::from_families(
        stock(),
        SourceEvidence::new(
            ProviderId::LocalTerminal,
            "2026-08-12T10:21:15.001+08:00",
            "terminal-missing-unit",
        )
        .unwrap(),
        1,
        None,
        None,
        Some(Quantity::new(1_001.0).unwrap()),
        None,
        ContinuityState::Continuous,
    )
    .unwrap();
    assert_eq!(
        monitor.process(missing_unit).unwrap_err(),
        MonitorError::UnitUnavailable {
            family: ObservationFamily::CumulativeVolume
        }
    );
    assert_eq!(monitor.window_len(&stock()), Some(1));

    assert_eq!(
        monitor
            .process(observation_with_unit(
                2,
                102.0,
                1_002.0,
                SourceQuantityUnit::Share,
                None,
            ))
            .unwrap_err(),
        MonitorError::SourceQuantityUnitMismatch {
            expected: SourceQuantityUnit::Lot,
            actual: SourceQuantityUnit::Share,
        }
    );
    assert_eq!(monitor.window_len(&stock()), Some(1));
}

#[test]
fn missing_or_mismatched_volume_unit_does_not_block_price_or_amount() {
    let amount_rule = AmountSpikeRule::new(
        1,
        100,
        0,
        Money::new(50.0).unwrap(),
        Money::new(20.0).unwrap(),
        5,
    )
    .unwrap();
    let mut amount = DeterministicAmountMonitor::new(limits(), amount_rule);
    let input = InjectedObservation::from_families(
        stock(),
        SourceEvidence::new(
            ProviderId::LocalTerminal,
            "2026-08-12T10:21:15.000+08:00",
            "terminal-independent-unit",
        )
        .unwrap(),
        0,
        Some(Price::new(100.0).unwrap()),
        Some(Money::new(100.0).unwrap()),
        Some(Quantity::new(100.0).unwrap()),
        None,
        ContinuityState::Continuous,
    )
    .unwrap();
    let mut price = DeterministicPriceMonitor::new(
        limits(),
        PriceChangeRule::new(100, 0, 0.1, 0.05, 5).unwrap(),
    );
    amount.process(input.clone()).unwrap();
    price.process(input).unwrap();
    let share_input = observation_with_unit(1, 101.0, 101.0, SourceQuantityUnit::Share, None);
    amount.process(share_input.clone()).unwrap();
    price.process(share_input).unwrap();
    assert_eq!(amount.window_len(&stock()), Some(2));
    assert_eq!(price.window_len(&stock()), Some(2));
}

#[test]
fn volume_unit_is_bound_into_rule_and_input_digests() {
    let make_rule = |unit| {
        VolumeSpikeRule::new(
            1,
            100,
            0,
            Quantity::new(50.0).unwrap(),
            Quantity::new(20.0).unwrap(),
            unit,
            5,
        )
        .unwrap()
    };
    let lot_rule = make_rule(SourceQuantityUnit::Lot);
    let share_rule = make_rule(SourceQuantityUnit::Share);
    assert_ne!(
        lot_rule.core_identity().unwrap().definition_digest(),
        share_rule.core_identity().unwrap().definition_digest()
    );

    let run = |rule: VolumeSpikeRule, unit: SourceQuantityUnit| {
        let mut monitor = DeterministicVolumeMonitor::new(limits(), rule);
        for (time, volume) in [(0, 1_000.0), (100, 1_010.0)] {
            monitor
                .process(observation_with_unit(time, 100.0, volume, unit, None))
                .unwrap();
        }
        let triggered = monitor
            .process(observation_with_unit(200, 100.0, 1_070.0, unit, None))
            .unwrap();
        assert!(matches!(
            triggered.transition(),
            Some(VolumeTransition::Triggered {
                volume_delta,
                unit: transition_unit,
            }) if volume_delta.get() == 60.0 && *transition_unit == unit
        ));
        triggered
            .core_event(
                stock(),
                rule,
                cursor(201),
                StreamContinuity::new(ContinuityState::Continuous, ContinuityState::Unknown),
                "2026-08-12T10:21:16.000+08:00",
            )
            .unwrap()
    };
    let lot_event = run(lot_rule, SourceQuantityUnit::Lot);
    let share_event = run(share_rule, SourceQuantityUnit::Share);
    assert_ne!(
        lot_event.input_evidence().rule_inputs_digest(),
        share_event.input_evidence().rule_inputs_digest()
    );
    assert_eq!(
        lot_event.input_evidence().terminal().first().provider(),
        ProviderId::LocalTerminal
    );
}
