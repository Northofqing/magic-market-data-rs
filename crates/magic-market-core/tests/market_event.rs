use magic_market_core::{
    AnomalyEvent, AnomalyInputDigest, AnomalyInputEvidence, AnomalyRuleDigest, AnomalyRuleIdentity,
    AnomalyTransition, AssetClass, ContinuityState, Exchange, InstrumentId,
    LocalAnalysisEventEvidence, LocalTerminalObservationEvidence, MarketEvent,
    ObservationTimeBasis, ProviderId, RuleInputDigest, SourceEvidence, SourceStatusEvent,
    SourceStatusKind, StreamContinuity, StreamCursor, StreamGeneration, StreamSequence,
};

fn evidence(provider: ProviderId, batch_id: &str) -> SourceEvidence {
    SourceEvidence::new(provider, "2026-08-12T10:21:15.310+08:00", batch_id).unwrap()
}

fn evidence_with_time(
    provider: ProviderId,
    observed_at: &str,
    batch_id: &str,
    source_at: Option<&str>,
) -> SourceEvidence {
    let evidence = SourceEvidence::new(provider, observed_at, batch_id).unwrap();
    source_at.map_or(evidence.clone(), |value| {
        evidence.with_source_at(value).unwrap()
    })
}

fn generation(value: &str) -> StreamGeneration {
    StreamGeneration::new(value).unwrap()
}

fn cursor(generation: &StreamGeneration, sequence: u64) -> StreamCursor {
    StreamCursor::new(generation.clone(), StreamSequence::new(sequence).unwrap())
}

fn rule() -> AnomalyRuleIdentity {
    let duration = 30_000_u64.to_be_bytes();
    let threshold = 10_000_u64.to_be_bytes();
    AnomalyRuleIdentity::from_canonical_definition(
        "price-surge",
        3,
        &[
            ("duration_millis", duration.as_slice()),
            ("threshold_millionths", threshold.as_slice()),
        ],
    )
    .unwrap()
}

fn anomaly_input(
    generation: &StreamGeneration,
    time_basis: ObservationTimeBasis,
) -> AnomalyInputEvidence {
    let source_at = (time_basis == ObservationTimeBasis::ProviderSourceTime)
        .then_some("2026-08-12T10:21:15.000+08:00");
    let terminal = LocalTerminalObservationEvidence::new(
        evidence_with_time(
            ProviderId::LocalTerminal,
            "2026-08-12T10:21:15.100+08:00",
            "terminal:first",
            source_at,
        ),
        evidence_with_time(
            ProviderId::LocalTerminal,
            "2026-08-12T10:21:45.100+08:00",
            "terminal:last",
            source_at,
        ),
    )
    .unwrap();
    let end_price = 1_427_000_000_u64.to_be_bytes();
    let start_price = 1_412_000_000_u64.to_be_bytes();
    let rule_inputs_digest = RuleInputDigest::from_canonical_fields(&[
        ("end_price_microunits", end_price.as_slice()),
        ("start_price_microunits", start_price.as_slice()),
    ])
    .unwrap();
    AnomalyInputEvidence::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        terminal,
        cursor(generation, 1_200),
        cursor(generation, 1_326),
        time_basis,
        StreamContinuity::new(ContinuityState::Continuous, ContinuityState::Unknown),
        rule_inputs_digest,
    )
    .unwrap()
}

fn anomaly_event(derived_observed_at: &str) -> AnomalyEvent {
    let generation = generation("550e8400-e29b-41d4-a716-446655440000");
    AnomalyEvent::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        rule(),
        AnomalyTransition::Triggered,
        cursor(&generation, 1_883),
        anomaly_input(&generation, ObservationTimeBasis::LocalObservationTime),
        derived_observed_at,
    )
    .unwrap()
}

#[test]
fn stream_identity_round_trips_and_advances_with_checked_arithmetic() {
    let generation = StreamGeneration::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let sequence = StreamSequence::new(41).unwrap();
    let cursor = StreamCursor::new(generation.clone(), sequence);

    assert_eq!(generation.as_str(), "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(sequence.get(), 41);
    assert_eq!(sequence.checked_next().unwrap().get(), 42);
    assert_eq!(cursor.generation(), &generation);
    assert_eq!(cursor.sequence(), sequence);

    let json = serde_json::to_string(&cursor).unwrap();
    assert_eq!(serde_json::from_str::<StreamCursor>(&json).unwrap(), cursor);
}

#[test]
fn stream_identity_rejects_invalid_or_exhausted_values() {
    for invalid in [
        "",
        "550e8400-e29b-41d4-a716-44665544000",
        "550E8400-E29B-41D4-A716-446655440000",
        "00000000-0000-0000-0000-000000000000",
        "550e8400-e29b-41d4-a716-44665544000z",
    ] {
        assert!(
            StreamGeneration::new(invalid).is_err(),
            "accepted {invalid}"
        );
        assert!(serde_json::from_str::<StreamGeneration>(&format!("\"{invalid}\"")).is_err());
    }

    assert!(StreamSequence::new(0).is_err());
    assert!(serde_json::from_str::<StreamSequence>("0").is_err());
    assert!(StreamSequence::new(u64::MAX)
        .unwrap()
        .checked_next()
        .is_err());
}

#[test]
fn time_basis_and_continuity_have_explicit_wire_names() {
    let cases = [
        (
            serde_json::to_string(&ObservationTimeBasis::ProviderSourceTime).unwrap(),
            "\"provider_source_time\"",
        ),
        (
            serde_json::to_string(&ObservationTimeBasis::LocalObservationTime).unwrap(),
            "\"local_observation_time\"",
        ),
        (
            serde_json::to_string(&ContinuityState::Continuous).unwrap(),
            "\"continuous\"",
        ),
        (
            serde_json::to_string(&ContinuityState::Unknown).unwrap(),
            "\"unknown\"",
        ),
        (
            serde_json::to_string(&ContinuityState::PossibleGap).unwrap(),
            "\"possible_gap\"",
        ),
        (
            serde_json::to_string(&ContinuityState::KnownGap).unwrap(),
            "\"known_gap\"",
        ),
    ];
    for (actual, expected) in cases {
        assert_eq!(actual, expected);
    }
}

#[test]
fn event_evidence_binds_local_terminal_inputs_to_local_analysis_output() {
    let first = evidence(ProviderId::LocalTerminal, "terminal:first");
    let last = evidence(ProviderId::LocalTerminal, "terminal:last");
    let input = LocalTerminalObservationEvidence::new(first, last).unwrap();
    let event = LocalAnalysisEventEvidence::new(
        input,
        evidence(ProviderId::LocalAnalysis, "analysis:event"),
    )
    .unwrap();

    assert_eq!(event.input().first().provider(), ProviderId::LocalTerminal);
    assert_eq!(event.input().last().provider(), ProviderId::LocalTerminal);
    assert_eq!(event.derived().provider(), ProviderId::LocalAnalysis);
    assert_eq!(event.input().first().source_at(), None);

    let json = serde_json::to_string(&event).unwrap();
    assert_eq!(
        serde_json::from_str::<LocalAnalysisEventEvidence>(&json).unwrap(),
        event
    );
}

#[test]
fn event_evidence_rejects_provider_impersonation_in_construction_and_serde() {
    assert!(LocalTerminalObservationEvidence::new(
        evidence(ProviderId::Tdx, "wrong:first"),
        evidence(ProviderId::LocalTerminal, "terminal:last"),
    )
    .is_err());
    assert!(LocalTerminalObservationEvidence::new(
        evidence(ProviderId::LocalTerminal, "terminal:first"),
        evidence(ProviderId::Tencent, "wrong:last"),
    )
    .is_err());

    let input = LocalTerminalObservationEvidence::new(
        evidence(ProviderId::LocalTerminal, "terminal:first"),
        evidence(ProviderId::LocalTerminal, "terminal:last"),
    )
    .unwrap();
    assert!(
        LocalAnalysisEventEvidence::new(input, evidence(ProviderId::Tdx, "wrong:derived")).is_err()
    );

    let valid = LocalAnalysisEventEvidence::new(
        LocalTerminalObservationEvidence::new(
            evidence(ProviderId::LocalTerminal, "terminal:first"),
            evidence(ProviderId::LocalTerminal, "terminal:last"),
        )
        .unwrap(),
        evidence(ProviderId::LocalAnalysis, "analysis:event"),
    )
    .unwrap();
    let mut wrong_input = serde_json::to_value(&valid).unwrap();
    wrong_input["input"]["first"]["provider"] = serde_json::json!("Tdx");
    assert!(serde_json::from_value::<LocalAnalysisEventEvidence>(wrong_input).is_err());

    let mut wrong_derived = serde_json::to_value(&valid).unwrap();
    wrong_derived["derived"]["provider"] = serde_json::json!("LocalTerminal");
    assert!(serde_json::from_value::<LocalAnalysisEventEvidence>(wrong_derived).is_err());
}

#[test]
fn canonical_digests_are_domain_separated_ordered_and_golden() {
    let duration = 30_000_u64.to_be_bytes();
    let threshold = 10_000_u64.to_be_bytes();
    let fields = [
        ("duration_millis", duration.as_slice()),
        ("threshold_millionths", threshold.as_slice()),
    ];
    let rule_digest = AnomalyRuleDigest::from_canonical_fields(&fields).unwrap();
    let input_digest = RuleInputDigest::from_canonical_fields(&fields).unwrap();

    assert_eq!(
        rule_digest.as_str(),
        "937782f6e9befa7ba85afe2646d508b22771444e66614216c469bbb4dcb43273"
    );
    assert_eq!(
        input_digest.as_str(),
        "7757b42f39810f4b9238f5ba9347d66f34e97ebf6a5d264255051940ceaa5a77"
    );
    assert_ne!(rule_digest.as_str(), input_digest.as_str());
    assert_eq!(
        AnomalyRuleDigest::from_canonical_fields(&fields).unwrap(),
        rule_digest
    );

    assert!(AnomalyRuleDigest::from_canonical_fields(&[]).is_err());
    assert!(AnomalyRuleDigest::from_canonical_fields(&[
        ("threshold_millionths", threshold.as_slice()),
        ("duration_millis", duration.as_slice()),
    ])
    .is_err());
    assert!(AnomalyRuleDigest::from_canonical_fields(&[
        ("duration_millis", duration.as_slice()),
        ("duration_millis", threshold.as_slice()),
    ])
    .is_err());
    assert!(
        AnomalyRuleDigest::from_canonical_fields(&[("Bad Field", duration.as_slice(),)]).is_err()
    );
    assert!(serde_json::from_str::<AnomalyRuleDigest>(&format!("\"{}\"", "a".repeat(63))).is_err());
    assert!(
        serde_json::from_str::<AnomalyInputDigest>(&format!("\"{}\"", "A".repeat(64))).is_err()
    );
}

#[test]
fn anomaly_rule_identity_validates_revision_id_and_serde_contract() {
    let rule = rule();
    assert_eq!(rule.id(), "price-surge");
    assert_eq!(rule.revision(), 3);
    assert_eq!(
        serde_json::from_str::<AnomalyRuleIdentity>(&serde_json::to_string(&rule).unwrap())
            .unwrap(),
        rule
    );

    let digest = rule.definition_digest().clone();
    assert!(AnomalyRuleIdentity::new("Price Surge", 3, digest.clone()).is_err());
    assert!(AnomalyRuleIdentity::new("price-surge", 0, digest).is_err());

    let mut zero_revision = serde_json::to_value(&rule).unwrap();
    zero_revision["revision"] = serde_json::json!(0);
    assert!(serde_json::from_value::<AnomalyRuleIdentity>(zero_revision).is_err());
    let mut extra_field = serde_json::to_value(&rule).unwrap();
    extra_field["threshold"] = serde_json::json!(1.0);
    assert!(serde_json::from_value::<AnomalyRuleIdentity>(extra_field).is_err());
}

#[test]
fn anomaly_input_binds_endpoints_cursor_time_basis_and_continuity() {
    let generation = generation("550e8400-e29b-41d4-a716-446655440000");
    let local = anomaly_input(&generation, ObservationTimeBasis::LocalObservationTime);
    assert_eq!(local.first_cursor().sequence().get(), 1_200);
    assert_eq!(local.last_cursor().sequence().get(), 1_326);
    assert_eq!(local.instrument().code(), "600519");
    assert_eq!(
        local.terminal().first().provider(),
        ProviderId::LocalTerminal
    );
    assert_eq!(
        local.time_basis(),
        ObservationTimeBasis::LocalObservationTime
    );
    assert_eq!(
        local.continuity().observation(),
        ContinuityState::Continuous
    );
    assert_eq!(local.continuity().source(), ContinuityState::Unknown);
    assert_eq!(
        serde_json::from_str::<AnomalyInputEvidence>(&serde_json::to_string(&local).unwrap())
            .unwrap(),
        local
    );

    let provider_time = anomaly_input(&generation, ObservationTimeBasis::ProviderSourceTime);
    assert_ne!(local.canonical_digest(), provider_time.canonical_digest());

    let mut changed_endpoint = serde_json::to_value(&local).unwrap();
    changed_endpoint["terminal"]["first"]["batch_id"] = serde_json::json!("tampered:first");
    assert!(serde_json::from_value::<AnomalyInputEvidence>(changed_endpoint).is_err());
    let mut changed_instrument = serde_json::to_value(&local).unwrap();
    changed_instrument["instrument"]["code"] = serde_json::json!("600000");
    assert!(serde_json::from_value::<AnomalyInputEvidence>(changed_instrument).is_err());
    let mut changed_continuity = serde_json::to_value(&local).unwrap();
    changed_continuity["continuity"]["source"] = serde_json::json!("possible_gap");
    assert!(serde_json::from_value::<AnomalyInputEvidence>(changed_continuity).is_err());
    let mut changed_digest = serde_json::to_value(&local).unwrap();
    changed_digest["canonical_digest"] = serde_json::json!("0".repeat(64));
    assert!(serde_json::from_value::<AnomalyInputEvidence>(changed_digest).is_err());
}

#[test]
fn anomaly_input_rejects_invalid_ranges_and_unproved_provider_time() {
    let first_generation = generation("550e8400-e29b-41d4-a716-446655440000");
    let other_generation = generation("550e8400-e29b-41d4-a716-446655440001");
    let terminal = LocalTerminalObservationEvidence::new(
        evidence(ProviderId::LocalTerminal, "terminal:first"),
        evidence(ProviderId::LocalTerminal, "terminal:last"),
    )
    .unwrap();
    let value = 1_u64.to_be_bytes();
    let digest = RuleInputDigest::from_canonical_fields(&[("value", value.as_slice())]).unwrap();
    let continuity = StreamContinuity::new(ContinuityState::Continuous, ContinuityState::Unknown);

    assert!(AnomalyInputEvidence::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        terminal.clone(),
        cursor(&first_generation, 1),
        cursor(&other_generation, 2),
        ObservationTimeBasis::LocalObservationTime,
        continuity,
        digest.clone(),
    )
    .is_err());
    assert!(AnomalyInputEvidence::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        terminal.clone(),
        cursor(&first_generation, 2),
        cursor(&first_generation, 1),
        ObservationTimeBasis::LocalObservationTime,
        continuity,
        digest.clone(),
    )
    .is_err());
    assert!(AnomalyInputEvidence::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        terminal,
        cursor(&first_generation, 1),
        cursor(&first_generation, 2),
        ObservationTimeBasis::ProviderSourceTime,
        continuity,
        digest,
    )
    .is_err());

    let invalid_observation_time = LocalTerminalObservationEvidence::new(
        SourceEvidence::new(ProviderId::LocalTerminal, "not-a-time", "terminal:first").unwrap(),
        evidence(ProviderId::LocalTerminal, "terminal:last"),
    )
    .unwrap();
    let value = 1_u64.to_be_bytes();
    let digest = RuleInputDigest::from_canonical_fields(&[("value", value.as_slice())]).unwrap();
    assert!(AnomalyInputEvidence::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        invalid_observation_time,
        cursor(&first_generation, 1),
        cursor(&first_generation, 2),
        ObservationTimeBasis::LocalObservationTime,
        continuity,
        digest,
    )
    .is_err());
}

#[test]
fn anomaly_event_id_is_deterministic_and_excludes_derived_observation_time() {
    let first = anomaly_event("2026-08-12T10:21:45.310+08:00");
    let replayed = anomaly_event("2026-08-12T10:22:00.000+08:00");

    assert_eq!(first.schema_version(), 1);
    assert_eq!(first.event_id(), replayed.event_id());
    assert_ne!(
        first.derived_evidence().observed_at(),
        replayed.derived_evidence().observed_at()
    );
    assert_eq!(
        first.derived_evidence().provider(),
        ProviderId::LocalAnalysis
    );
    assert_eq!(first.derived_evidence().source_at(), None);
    assert_eq!(
        first.derived_evidence().batch_id(),
        first.event_id().as_str()
    );
    assert_eq!(first.transition(), AnomalyTransition::Triggered);

    let event = MarketEvent::MarketAnomaly(Box::new(first.clone()));
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "market_anomaly");
    assert_eq!(serde_json::from_value::<MarketEvent>(json).unwrap(), event);
    assert_eq!(event.event_id(), first.event_id());
    assert_eq!(event.stream(), first.stream());
}

#[test]
fn anomaly_event_serde_rejects_contradictory_identity_or_evidence() {
    let event = anomaly_event("2026-08-12T10:21:45.310+08:00");

    let mut transition = serde_json::to_value(&event).unwrap();
    transition["transition"] = serde_json::json!("escalated");
    assert!(serde_json::from_value::<AnomalyEvent>(transition).is_err());
    let mut instrument = serde_json::to_value(&event).unwrap();
    instrument["instrument"]["code"] = serde_json::json!("600000");
    assert!(serde_json::from_value::<AnomalyEvent>(instrument).is_err());
    let mut event_id = serde_json::to_value(&event).unwrap();
    event_id["event_id"] = serde_json::json!("0".repeat(64));
    assert!(serde_json::from_value::<AnomalyEvent>(event_id).is_err());
    let mut provider = serde_json::to_value(&event).unwrap();
    provider["derived_evidence"]["provider"] = serde_json::json!("LocalTerminal");
    assert!(serde_json::from_value::<AnomalyEvent>(provider).is_err());
    let mut source_at = serde_json::to_value(&event).unwrap();
    source_at["derived_evidence"]["source_at"] = serde_json::json!("2026-08-12T10:21:45+08:00");
    assert!(serde_json::from_value::<AnomalyEvent>(source_at).is_err());
    let mut batch = serde_json::to_value(&event).unwrap();
    batch["derived_evidence"]["batch_id"] = serde_json::json!("not-the-event-id");
    assert!(serde_json::from_value::<AnomalyEvent>(batch).is_err());
    let mut schema = serde_json::to_value(&event).unwrap();
    schema["schema_version"] = serde_json::json!(2);
    assert!(serde_json::from_value::<AnomalyEvent>(schema).is_err());

    let generation = generation("550e8400-e29b-41d4-a716-446655440000");
    assert!(AnomalyEvent::new(
        InstrumentId::new(Exchange::Shanghai, "600000", AssetClass::Equity).unwrap(),
        rule(),
        AnomalyTransition::Triggered,
        cursor(&generation, 1_883),
        anomaly_input(&generation, ObservationTimeBasis::LocalObservationTime),
        "2026-08-12T10:21:45.310+08:00",
    )
    .is_err());
    assert!(AnomalyEvent::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        rule(),
        AnomalyTransition::Triggered,
        cursor(&generation, 1_883),
        anomaly_input(&generation, ObservationTimeBasis::LocalObservationTime),
        "2026-08-12T10:21:45.000+08:00",
    )
    .is_err());
}

#[test]
fn source_status_events_are_typed_deterministic_and_provider_bound() {
    let current = generation("550e8400-e29b-41d4-a716-446655440001");
    let previous = generation("550e8400-e29b-41d4-a716-446655440000");
    let continuity =
        StreamContinuity::new(ContinuityState::Continuous, ContinuityState::PossibleGap);
    let event = SourceStatusEvent::new(
        SourceStatusKind::Recovered,
        None,
        cursor(&current, 1),
        Some(previous.clone()),
        "2026-08-12T10:21:45.310+08:00",
        continuity,
    )
    .unwrap();
    let duplicate = SourceStatusEvent::new(
        SourceStatusKind::Recovered,
        None,
        cursor(&current, 1),
        Some(previous),
        "2026-08-12T10:21:45.310+08:00",
        continuity,
    )
    .unwrap();

    assert_eq!(event.event_id(), duplicate.event_id());
    assert_eq!(event.source_provider(), ProviderId::LocalTerminal);
    assert_eq!(
        event.previous_generation().unwrap().as_str(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    assert_eq!(event.status(), SourceStatusKind::Recovered);
    assert_eq!(event.continuity(), continuity);

    let wrapped = MarketEvent::SourceStatus(event.clone());
    let json = serde_json::to_value(&wrapped).unwrap();
    assert_eq!(json["type"], "source_status");
    assert_eq!(
        serde_json::from_value::<MarketEvent>(json).unwrap(),
        wrapped
    );

    let mut provider = serde_json::to_value(&event).unwrap();
    provider["source_provider"] = serde_json::json!("Tdx");
    assert!(serde_json::from_value::<SourceStatusEvent>(provider).is_err());
    let mut event_id = serde_json::to_value(&event).unwrap();
    event_id["event_id"] = serde_json::json!("f".repeat(64));
    assert!(serde_json::from_value::<SourceStatusEvent>(event_id).is_err());
}

#[test]
fn source_status_rejects_contradictory_generation_and_continuity() {
    let current = generation("550e8400-e29b-41d4-a716-446655440001");
    let previous = generation("550e8400-e29b-41d4-a716-446655440000");
    let continuous =
        StreamContinuity::new(ContinuityState::Continuous, ContinuityState::Continuous);
    let possible_gap =
        StreamContinuity::new(ContinuityState::Continuous, ContinuityState::PossibleGap);

    assert!(SourceStatusEvent::new(
        SourceStatusKind::Recovered,
        None,
        cursor(&current, 1),
        None,
        "2026-08-12T10:21:45.310+08:00",
        possible_gap,
    )
    .is_err());
    assert!(SourceStatusEvent::new(
        SourceStatusKind::Recovered,
        None,
        cursor(&current, 1),
        Some(current.clone()),
        "2026-08-12T10:21:45.310+08:00",
        possible_gap,
    )
    .is_err());
    assert!(SourceStatusEvent::new(
        SourceStatusKind::Recovered,
        None,
        cursor(&current, 1),
        Some(previous.clone()),
        "2026-08-12T10:21:45.310+08:00",
        continuous,
    )
    .is_err());
    assert!(SourceStatusEvent::new(
        SourceStatusKind::Disconnected,
        None,
        cursor(&current, 1),
        Some(previous),
        "2026-08-12T10:21:45.310+08:00",
        possible_gap,
    )
    .is_err());
    assert!(SourceStatusEvent::new(
        SourceStatusKind::KnownDataGap,
        None,
        cursor(&current, 1),
        None,
        "2026-08-12T10:21:45.310+08:00",
        possible_gap,
    )
    .is_err());
    assert!(SourceStatusEvent::new(
        SourceStatusKind::PossibleDataGap,
        None,
        cursor(&current, 1),
        None,
        "not-a-time",
        possible_gap,
    )
    .is_err());
}
