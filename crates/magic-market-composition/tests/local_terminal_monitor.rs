use magic_market_composition::{
    DiagnosticLocalTerminalMonitorComposition, EastmoneyProviderTopNRankingRouter,
    LocalMonitorCapability, LocalTerminalMonitorComposition, LocalTerminalMonitorCompositionError,
};
use magic_market_core::{
    AssetClass, ContinuityState, Exchange, InstrumentId, Money, Price, ProviderId, Quantity,
    SourceEvidence,
};
use magic_market_monitor::{
    AmountSpikeRule, InjectedObservation, MonitorLimits, PriceChangeRule, RuleState,
    SourceQuantityUnit, VolumeSpikeRule,
};
use magic_tdx_local_rs::{SupervisorAction, SupervisorEvent, SupervisorState};

#[test]
fn production_constructor_fails_closed_with_every_missing_capability() {
    let error = match LocalTerminalMonitorComposition::new() {
        Ok(_) => panic!("unadmitted production composition unexpectedly constructed"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        LocalTerminalMonitorCompositionError::CapabilityUnadmitted {
            capabilities: vec![
                LocalMonitorCapability::LocalTerminalSourceRecordCount,
                LocalMonitorCapability::LocalPriceChangeAnomaly,
                LocalMonitorCapability::LocalAmountChangeAnomaly,
                LocalMonitorCapability::LocalVolumeChangeAnomaly,
            ],
        }
    );
}

#[test]
fn production_composition_has_a_separate_implementation_gate() {
    assert_ne!(
        LocalTerminalMonitorCompositionError::ImplementationUnavailable,
        LocalTerminalMonitorCompositionError::CapabilityUnadmitted {
            capabilities: Vec::new(),
        }
    );
}

#[test]
fn failed_local_construction_does_not_change_existing_routes() {
    let before = EastmoneyProviderTopNRankingRouter::new()
        .unwrap()
        .provider_ids();
    assert!(LocalTerminalMonitorComposition::new().is_err());
    let after = EastmoneyProviderTopNRankingRouter::new()
        .unwrap()
        .provider_ids();
    assert_eq!(before, [ProviderId::Eastmoney]);
    assert_eq!(after, before);
}

#[test]
fn diagnostic_composition_is_a_distinct_pure_type_with_explicit_policy() {
    assert_ne!(
        std::any::type_name::<DiagnosticLocalTerminalMonitorComposition>(),
        std::any::type_name::<LocalTerminalMonitorComposition>()
    );
    let mut diagnostic = DiagnosticLocalTerminalMonitorComposition::new(
        2,
        MonitorLimits::new(1, 4).unwrap(),
        PriceChangeRule::new(100, 10, 0.10, 0.05, 25).unwrap(),
        AmountSpikeRule::new(
            1,
            100,
            10,
            Money::new(10.0).unwrap(),
            Money::new(5.0).unwrap(),
            25,
        )
        .unwrap(),
        VolumeSpikeRule::new(
            1,
            100,
            10,
            Quantity::new(10.0).unwrap(),
            Quantity::new(5.0).unwrap(),
            SourceQuantityUnit::Lot,
            25,
        )
        .unwrap(),
    );
    assert_eq!(diagnostic.supervisor().state(), SupervisorState::Disabled);
    assert_eq!(
        diagnostic
            .transition_supervisor(SupervisorEvent::Enable)
            .unwrap()
            .action,
        SupervisorAction::RunDiscoveryProbe
    );

    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let observation = InjectedObservation::new(
        instrument,
        SourceEvidence::new(ProviderId::LocalTerminal, "observed", "diagnostic-input").unwrap(),
        1,
        Price::new(100.0).unwrap(),
        Money::new(100.0).unwrap(),
        Quantity::new(100.0).unwrap(),
        SourceQuantityUnit::Lot,
        ContinuityState::Continuous,
    )
    .unwrap();
    let outcome = diagnostic.process_observation(observation).unwrap();
    assert_eq!(outcome.price.state(), RuleState::WarmingUp);
    assert_eq!(outcome.price.transition(), None);
    assert_eq!(outcome.amount.state(), RuleState::WarmingUp);
    assert_eq!(outcome.amount.transition(), None);
    assert_eq!(outcome.volume.state(), RuleState::WarmingUp);
    assert_eq!(outcome.volume.transition(), None);
}
