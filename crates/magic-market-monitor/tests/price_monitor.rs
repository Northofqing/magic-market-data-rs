use magic_market_core::{
    AssetClass, ContinuityState, Exchange, InstrumentId, Money, Price, ProviderId, Quantity,
    SourceEvidence,
};
use magic_market_monitor::{
    DeterministicPriceMonitor, InjectedObservation, MonitorError, MonitorLimits, MonitorTransition,
    ObservationFamily, PriceChangeRule, ResetReason, RuleState, SourceQuantityUnit,
};

fn stock(code: &str) -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap()
}

fn observation(
    instrument: &InstrumentId,
    time: u64,
    price: f64,
    amount: f64,
    volume: f64,
    continuity: ContinuityState,
) -> InjectedObservation {
    InjectedObservation::new(
        instrument.clone(),
        SourceEvidence::new(
            ProviderId::LocalTerminal,
            format!("observed-{time}"),
            format!("terminal-{time}"),
        )
        .unwrap(),
        time,
        Price::new(price).unwrap(),
        Money::new(amount).unwrap(),
        Quantity::new(volume).unwrap(),
        SourceQuantityUnit::Lot,
        continuity,
    )
    .unwrap()
}

fn monitor(capacity: u16) -> DeterministicPriceMonitor {
    DeterministicPriceMonitor::new(
        MonitorLimits::new(2, capacity).unwrap(),
        PriceChangeRule::new(100, 100, 0.10, 0.05, 10).unwrap(),
    )
}

#[test]
fn deterministic_state_machine_uses_explicit_hysteresis_and_cooldown() {
    let instrument = stock("600519");
    let inputs = [
        observation(
            &instrument,
            0,
            100.0,
            100.0,
            100.0,
            ContinuityState::Continuous,
        ),
        observation(
            &instrument,
            100,
            100.0,
            110.0,
            110.0,
            ContinuityState::Continuous,
        ),
        observation(
            &instrument,
            101,
            111.0,
            120.0,
            120.0,
            ContinuityState::Continuous,
        ),
        observation(
            &instrument,
            102,
            111.0,
            130.0,
            130.0,
            ContinuityState::Continuous,
        ),
        observation(
            &instrument,
            110,
            104.0,
            140.0,
            140.0,
            ContinuityState::Continuous,
        ),
        observation(
            &instrument,
            111,
            104.0,
            150.0,
            150.0,
            ContinuityState::Continuous,
        ),
    ];

    let run = || {
        let mut monitor = monitor(16);
        inputs
            .iter()
            .cloned()
            .map(|input| monitor.process(input).unwrap())
            .collect::<Vec<_>>()
    };
    let first = run();
    let second = run();
    assert_eq!(first, second);
    assert_eq!(first[0].state(), RuleState::WarmingUp);
    let mut evidence_monitor = monitor(16);
    evidence_monitor.process(inputs[0].clone()).unwrap();
    assert_eq!(
        evidence_monitor
            .window_endpoints(&instrument)
            .unwrap()
            .0
            .evidence()
            .batch_id(),
        "terminal-0"
    );
    assert_eq!(first[1].transition(), Some(MonitorTransition::WarmedUp));
    assert!(matches!(
        first[2].transition(),
        Some(MonitorTransition::Triggered { change_ratio }) if change_ratio > 0.10
    ));
    assert_eq!(
        first[3].transition(),
        Some(MonitorTransition::EnteredCoolingDown)
    );
    assert_eq!(first[4].state(), RuleState::CoolingDown);
    assert!(matches!(
        first[5].transition(),
        Some(MonitorTransition::Rearmed { change_ratio }) if change_ratio <= 0.05
    ));
    assert_eq!(first[5].state(), RuleState::Armed);
}

#[test]
fn duplicate_and_out_of_order_input_are_rejected_without_mutation() {
    let instrument = stock("600519");
    let mut monitor = monitor(8);
    monitor
        .process(observation(
            &instrument,
            10,
            100.0,
            100.0,
            100.0,
            ContinuityState::Continuous,
        ))
        .unwrap();

    assert_eq!(
        monitor
            .process(observation(
                &instrument,
                10,
                101.0,
                101.0,
                101.0,
                ContinuityState::Continuous,
            ))
            .unwrap_err(),
        MonitorError::DuplicateObservation { arrival_millis: 10 }
    );
    assert_eq!(
        monitor
            .process(observation(
                &instrument,
                9,
                101.0,
                101.0,
                101.0,
                ContinuityState::Continuous,
            ))
            .unwrap_err(),
        MonitorError::OutOfOrderObservation {
            previous: 10,
            actual: 9,
        }
    );
    assert_eq!(monitor.window_len(&instrument), Some(1));
}

#[test]
fn continuity_resets_but_unrelated_counter_rollbacks_do_not_reset_price() {
    let instrument = stock("600519");
    let mut monitor = monitor(8);
    monitor
        .process(observation(
            &instrument,
            0,
            100.0,
            100.0,
            100.0,
            ContinuityState::Continuous,
        ))
        .unwrap();
    let gap = monitor
        .process(observation(
            &instrument,
            1,
            100.0,
            101.0,
            101.0,
            ContinuityState::PossibleGap,
        ))
        .unwrap();
    assert_eq!(
        gap.transition(),
        Some(MonitorTransition::Reset(ResetReason::NonContinuous(
            ContinuityState::PossibleGap
        )))
    );
    assert_eq!(monitor.window_len(&instrument), Some(1));

    let rollback = monitor
        .process(observation(
            &instrument,
            2,
            100.0,
            99.0,
            99.0,
            ContinuityState::Continuous,
        ))
        .unwrap();
    assert_eq!(rollback.transition(), None);
    assert_eq!(rollback.state(), RuleState::WarmingUp);
    assert_eq!(monitor.window_len(&instrument), Some(2));
}

#[test]
fn price_monitor_accepts_price_only_and_rejects_missing_price_without_mutation() {
    let instrument = stock("600519");
    let price_only = InjectedObservation::from_families(
        instrument.clone(),
        SourceEvidence::new(ProviderId::LocalTerminal, "observed-1", "terminal-1").unwrap(),
        1,
        Some(Price::new(100.0).unwrap()),
        None,
        None,
        None,
        ContinuityState::Continuous,
    )
    .unwrap();
    assert_eq!(price_only.price().unwrap().get(), 100.0);
    assert_eq!(price_only.cumulative_amount(), None);
    assert_eq!(price_only.cumulative_volume(), None);

    let mut monitor = monitor(8);
    monitor.process(price_only).unwrap();
    let before_len = monitor.window_len(&instrument);
    let missing_price = InjectedObservation::from_families(
        instrument.clone(),
        SourceEvidence::new(ProviderId::LocalTerminal, "observed-2", "terminal-2").unwrap(),
        2,
        None,
        Some(Money::new(200.0).unwrap()),
        Some(Quantity::new(200.0).unwrap()),
        Some(SourceQuantityUnit::Lot),
        ContinuityState::Continuous,
    )
    .unwrap();
    assert_eq!(
        monitor.process(missing_price).unwrap_err(),
        MonitorError::FamilyUnavailable {
            family: ObservationFamily::Price
        }
    );
    assert_eq!(monitor.window_len(&instrument), before_len);
    assert_eq!(monitor.instrument_count(), 1);

    let unknown = stock("600000");
    let missing_price = InjectedObservation::from_families(
        unknown,
        SourceEvidence::new(ProviderId::LocalTerminal, "observed-3", "terminal-3").unwrap(),
        3,
        None,
        None,
        Some(Quantity::new(1.0).unwrap()),
        Some(SourceQuantityUnit::Lot),
        ContinuityState::Continuous,
    )
    .unwrap();
    assert!(matches!(
        monitor.process(missing_price),
        Err(MonitorError::FamilyUnavailable {
            family: ObservationFamily::Price
        })
    ));
    assert_eq!(monitor.instrument_count(), 1);
}

#[test]
fn explicit_bounds_reject_invalid_limits_and_reset_on_window_overflow() {
    assert!(MonitorLimits::new(0, 2).is_err());
    assert!(MonitorLimits::new(1, 1).is_err());
    assert!(PriceChangeRule::new(0, 0, 0.1, 0.05, 1).is_err());
    assert!(PriceChangeRule::new(10, 11, 0.1, 0.05, 1).is_err());
    assert!(PriceChangeRule::new(10, 1, 0.1, 0.1, 1).is_err());

    let instrument = stock("600519");
    let mut monitor = monitor(2);
    for time in 0..2 {
        monitor
            .process(observation(
                &instrument,
                time,
                100.0,
                100.0 + time as f64,
                100.0 + time as f64,
                ContinuityState::Continuous,
            ))
            .unwrap();
    }
    assert_eq!(
        monitor
            .process(observation(
                &instrument,
                2,
                100.0,
                102.0,
                102.0,
                ContinuityState::Continuous,
            ))
            .unwrap_err(),
        MonitorError::WindowOverflow { capacity: 2 }
    );
    assert_eq!(monitor.state(&instrument), Some(RuleState::WarmingUp));
    assert_eq!(monitor.window_len(&instrument), Some(1));

    let other = stock("600000");
    monitor
        .process(observation(
            &other,
            0,
            100.0,
            100.0,
            100.0,
            ContinuityState::Continuous,
        ))
        .unwrap();
    let third = stock("600001");
    assert_eq!(
        monitor
            .process(observation(
                &third,
                0,
                100.0,
                100.0,
                100.0,
                ContinuityState::Continuous,
            ))
            .unwrap_err(),
        MonitorError::InstrumentLimit { limit: 2 }
    );
}

#[test]
fn observations_reject_wrong_provider_and_negative_cumulative_amount() {
    let instrument = stock("600519");
    let wrong_provider = InjectedObservation::new(
        instrument.clone(),
        SourceEvidence::new(ProviderId::Tdx, "observed", "tdx").unwrap(),
        0,
        Price::new(1.0).unwrap(),
        Money::new(1.0).unwrap(),
        Quantity::new(1.0).unwrap(),
        SourceQuantityUnit::Lot,
        ContinuityState::Continuous,
    );
    assert!(matches!(
        wrong_provider,
        Err(MonitorError::InvalidObservation(_))
    ));

    let negative_amount = InjectedObservation::new(
        instrument,
        SourceEvidence::new(ProviderId::LocalTerminal, "observed", "terminal").unwrap(),
        0,
        Price::new(1.0).unwrap(),
        Money::new(-1.0).unwrap(),
        Quantity::new(1.0).unwrap(),
        SourceQuantityUnit::Lot,
        ContinuityState::Continuous,
    );
    assert!(matches!(
        negative_amount,
        Err(MonitorError::InvalidObservation(_))
    ));
}

#[test]
fn extreme_prices_fail_explicitly_instead_of_emitting_an_infinite_change() {
    let instrument = stock("600519");
    let mut monitor = DeterministicPriceMonitor::new(
        MonitorLimits::new(1, 4).unwrap(),
        PriceChangeRule::new(1, 0, 0.1, 0.05, 0).unwrap(),
    );
    monitor
        .process(observation(
            &instrument,
            0,
            f64::MIN_POSITIVE,
            1.0,
            1.0,
            ContinuityState::Continuous,
        ))
        .unwrap();
    assert_eq!(
        monitor
            .process(observation(
                &instrument,
                1,
                f64::MAX,
                2.0,
                2.0,
                ContinuityState::Continuous,
            ))
            .unwrap_err(),
        MonitorError::PriceChangeOverflow
    );
    assert_eq!(monitor.state(&instrument), Some(RuleState::WarmingUp));
    assert_eq!(monitor.window_len(&instrument), Some(1));
}
