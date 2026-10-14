use magic_market_core::{
    AnomalyEvent, AnomalyInputEvidence, AnomalyRuleIdentity, AnomalyTransition, AssetClass,
    ContinuityState, CoreError, Exchange, InstrumentId, LocalTerminalObservationEvidence,
    MarketEvent, ObservationTimeBasis, ProviderId, RuleInputDigest, SourceEvidence,
    SourceStatusEvent, SourceStatusKind, StreamContinuity, StreamCursor, StreamGeneration,
    StreamSequence,
};

const GENERATION: &str = "550e8400-e29b-41d4-a716-446655440000";
const OTHER_GENERATION: &str = "550e8400-e29b-41d4-a716-446655440001";
const FIRST_TIME: &str = "2026-10-10T10:00:00.000000001+08:00";
const SAME_INSTANT: &str = "2026-10-10T02:00:00.000000001Z";
const EARLIER_INSTANT: &str = "2026-10-10T02:00:00.000000000Z";

fn input_with_last_observation(last_observed_at: &str) -> Result<AnomalyInputEvidence, CoreError> {
    let generation = StreamGeneration::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
    AnomalyInputEvidence::new(
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
        LocalTerminalObservationEvidence::new(
            SourceEvidence::new(
                ProviderId::LocalTerminal,
                "2026-10-10T10:00:00.000000001+08:00",
                "terminal:first-observation",
            )
            .unwrap(),
            SourceEvidence::new(
                ProviderId::LocalTerminal,
                last_observed_at,
                "terminal:last-observation",
            )
            .unwrap(),
        )
        .unwrap(),
        StreamCursor::new(generation.clone(), StreamSequence::new(7).unwrap()),
        StreamCursor::new(generation, StreamSequence::new(8).unwrap()),
        ObservationTimeBasis::LocalObservationTime,
        StreamContinuity::new(ContinuityState::Continuous, ContinuityState::Unknown),
        RuleInputDigest::from_canonical_fields(&[("window_size", &[2])]).unwrap(),
    )
}

#[test]
fn anomaly_input_rejects_a_malformed_final_observation_in_constructor_and_wire() {
    let valid = input_with_last_observation("2026-10-10T02:00:00.000000002Z").unwrap();
    let mut wire = serde_json::to_value(valid).unwrap();
    let decoded: AnomalyInputEvidence = serde_json::from_value(wire.clone()).unwrap();

    assert_eq!(decoded.instrument().exchange(), Exchange::Shanghai);
    assert_eq!(decoded.instrument().code(), "600396");
    assert_eq!(decoded.instrument().asset_class(), AssetClass::Equity);
    assert_eq!(
        decoded.first_cursor().generation().as_str(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    assert_eq!(
        decoded.last_cursor().generation().as_str(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    assert_eq!(decoded.first_cursor().sequence().get(), 7);
    assert_eq!(decoded.last_cursor().sequence().get(), 8);
    assert_eq!(
        decoded.time_basis(),
        ObservationTimeBasis::LocalObservationTime
    );
    assert_eq!(
        decoded.continuity().observation(),
        ContinuityState::Continuous
    );
    assert_eq!(decoded.continuity().source(), ContinuityState::Unknown);
    assert_eq!(
        decoded.terminal().first().provider(),
        ProviderId::LocalTerminal
    );
    assert_eq!(
        decoded.terminal().last().provider(),
        ProviderId::LocalTerminal
    );
    assert_eq!(
        decoded.terminal().first().observed_at(),
        "2026-10-10T10:00:00.000000001+08:00"
    );
    assert_eq!(
        decoded.terminal().last().observed_at(),
        "2026-10-10T02:00:00.000000002Z"
    );
    assert_eq!(
        decoded.terminal().first().batch_id(),
        "terminal:first-observation"
    );
    assert_eq!(
        decoded.terminal().last().batch_id(),
        "terminal:last-observation"
    );
    assert!(decoded.terminal().first().source_at().is_none());
    assert!(decoded.terminal().last().source_at().is_none());

    assert_eq!(
        input_with_last_observation("2026-10-10T10:00:00.Z").unwrap_err(),
        CoreError::InvalidRequest("invalid evidence timestamp \"2026-10-10T10:00:00.Z\"".into())
    );

    // Keep the valid first endpoint, cursors and commitment unchanged. The
    // malformed final timestamp must be diagnosed before digest comparison.
    wire["terminal"]["last"]["observed_at"] = serde_json::json!("2026-10-10T10:00:00.Z");
    let error = serde_json::from_value::<AnomalyInputEvidence>(wire).unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid request: invalid evidence timestamp \"2026-10-10T10:00:00.Z\""
    );
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Beijing, "430047", AssetClass::Equity).unwrap()
}

fn cursor(generation: &str, sequence: u64) -> StreamCursor {
    StreamCursor::new(
        StreamGeneration::new(generation).unwrap(),
        StreamSequence::new(sequence).unwrap(),
    )
}

fn endpoint(observed_at: &str, source_at: Option<&str>, batch: &str) -> SourceEvidence {
    let evidence = SourceEvidence::new(ProviderId::LocalTerminal, observed_at, batch).unwrap();
    match source_at {
        Some(time) => evidence.with_source_at(time).unwrap(),
        None => evidence,
    }
}

fn input(
    first_observed: &str,
    last_observed: &str,
    first_source: Option<&str>,
    last_source: Option<&str>,
    basis: ObservationTimeBasis,
) -> Result<AnomalyInputEvidence, CoreError> {
    AnomalyInputEvidence::new(
        instrument(),
        LocalTerminalObservationEvidence::new(
            endpoint(first_observed, first_source, "first-observation"),
            endpoint(last_observed, last_source, "last-observation"),
        )
        .unwrap(),
        cursor(GENERATION, 7),
        cursor(GENERATION, 7),
        basis,
        StreamContinuity::new(ContinuityState::Continuous, ContinuityState::Unknown),
        RuleInputDigest::from_canonical_fields(&[("sample_count", &[1])]).unwrap(),
    )
}

fn event(stream_generation: &str, derived_time: &str) -> Result<AnomalyEvent, CoreError> {
    AnomalyEvent::new(
        instrument(),
        AnomalyRuleIdentity::from_canonical_definition(
            "0.release-rule_v1",
            1,
            &[("window_nanos", &[1])],
        )
        .unwrap(),
        AnomalyTransition::Rearmed,
        cursor(stream_generation, 8),
        input(
            FIRST_TIME,
            SAME_INSTANT,
            None,
            None,
            ObservationTimeBasis::LocalObservationTime,
        )
        .unwrap(),
        derived_time,
    )
}

#[test]
fn equal_endpoint_instants_and_equal_cursors_are_valid_and_preserve_evidence() {
    let input = input(
        FIRST_TIME,
        SAME_INSTANT,
        Some(FIRST_TIME),
        Some(SAME_INSTANT),
        ObservationTimeBasis::ProviderSourceTime,
    )
    .unwrap();

    assert_eq!(input.first_cursor().sequence().get(), 7);
    assert_eq!(input.last_cursor().sequence().get(), 7);
    assert_eq!(input.terminal().first().observed_at(), FIRST_TIME);
    assert_eq!(input.terminal().last().observed_at(), SAME_INSTANT);
    assert_eq!(input.terminal().first().source_at(), Some(FIRST_TIME));
    assert_eq!(input.terminal().last().source_at(), Some(SAME_INSTANT));
    assert_eq!(input.terminal().first().batch_id(), "first-observation");
    assert_eq!(input.terminal().last().batch_id(), "last-observation");
    assert_eq!(input.time_basis(), ObservationTimeBasis::ProviderSourceTime);

    let decoded: AnomalyInputEvidence =
        serde_json::from_value(serde_json::to_value(&input).unwrap()).unwrap();
    assert_eq!(decoded.terminal().last().source_at(), Some(SAME_INSTANT));
    assert_eq!(decoded.last_cursor().sequence().get(), 7);
}

fn assert_invalid_request<T: std::fmt::Debug>(result: Result<T, CoreError>, reason: &str) {
    assert_eq!(
        result.unwrap_err(),
        CoreError::InvalidRequest(reason.into())
    );
}

fn assert_serde_error<T: serde::de::DeserializeOwned>(wire: serde_json::Value, reason: &str) {
    let error = match serde_json::from_value::<T>(wire) {
        Ok(_) => panic!("accepted wire value that should fail: {reason}"),
        Err(error) => error,
    };
    assert!(error.to_string().contains(reason), "{error}");
}

#[test]
fn observation_range_rejects_a_one_nanosecond_reversal_across_offsets() {
    let reason = "anomaly input observation times must be non-decreasing";
    assert_invalid_request(
        input(
            FIRST_TIME,
            EARLIER_INSTANT,
            None,
            None,
            ObservationTimeBasis::LocalObservationTime,
        ),
        reason,
    );

    let valid = input(
        FIRST_TIME,
        SAME_INSTANT,
        None,
        None,
        ObservationTimeBasis::LocalObservationTime,
    )
    .unwrap();
    let mut wire = serde_json::to_value(valid).unwrap();
    wire["terminal"]["last"]["observed_at"] = serde_json::json!(EARLIER_INSTANT);
    assert_serde_error::<AnomalyInputEvidence>(wire, reason);
}

#[test]
fn provider_time_requires_each_endpoint_independently_in_constructor_and_wire() {
    let reason = "provider-source-time input requires source time on both endpoint observations";
    let valid = input(
        FIRST_TIME,
        SAME_INSTANT,
        Some(FIRST_TIME),
        Some(SAME_INSTANT),
        ObservationTimeBasis::ProviderSourceTime,
    )
    .unwrap();

    for (missing_endpoint, first_source, last_source) in [
        ("first", None, Some(SAME_INSTANT)),
        ("last", Some(FIRST_TIME), None),
    ] {
        assert_invalid_request(
            input(
                FIRST_TIME,
                SAME_INSTANT,
                first_source,
                last_source,
                ObservationTimeBasis::ProviderSourceTime,
            ),
            reason,
        );
        let mut wire = serde_json::to_value(&valid).unwrap();
        wire["terminal"][missing_endpoint]["source_at"] = serde_json::Value::Null;
        assert_serde_error::<AnomalyInputEvidence>(wire, reason);
    }
}

#[test]
fn selected_provider_clock_rejects_reversal_and_ambiguous_endpoint_time() {
    let valid = input(
        FIRST_TIME,
        SAME_INSTANT,
        Some(FIRST_TIME),
        Some(SAME_INSTANT),
        ObservationTimeBasis::ProviderSourceTime,
    )
    .unwrap();

    for (first_source, last_source, endpoint_name, changed_time, reason) in [
        (
            FIRST_TIME,
            EARLIER_INSTANT,
            "last",
            EARLIER_INSTANT,
            "anomaly input provider source times must be non-decreasing",
        ),
        (
            "2026-10-10T10:00:00",
            SAME_INSTANT,
            "first",
            "2026-10-10T10:00:00",
            "evidence timestamp is not an unambiguous instant \"2026-10-10T10:00:00\"",
        ),
        (
            FIRST_TIME,
            "not-a-time",
            "last",
            "not-a-time",
            "invalid evidence timestamp \"not-a-time\"",
        ),
    ] {
        assert_invalid_request(
            input(
                FIRST_TIME,
                SAME_INSTANT,
                Some(first_source),
                Some(last_source),
                ObservationTimeBasis::ProviderSourceTime,
            ),
            reason,
        );
        let mut wire = serde_json::to_value(&valid).unwrap();
        wire["terminal"][endpoint_name]["source_at"] = serde_json::json!(changed_time);
        assert_serde_error::<AnomalyInputEvidence>(wire, reason);
    }
}

#[test]
fn local_clock_preserves_reversed_source_times_and_still_commits_them() {
    let input = input(
        FIRST_TIME,
        SAME_INSTANT,
        Some(FIRST_TIME),
        Some(EARLIER_INSTANT),
        ObservationTimeBasis::LocalObservationTime,
    )
    .unwrap();
    assert_eq!(input.terminal().last().source_at(), Some(EARLIER_INSTANT));
    assert_eq!(
        input.time_basis(),
        ObservationTimeBasis::LocalObservationTime
    );

    let wire = serde_json::to_value(&input).unwrap();
    let decoded: AnomalyInputEvidence = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(decoded.terminal().last().source_at(), Some(EARLIER_INSTANT));
    let mut tampered = wire;
    tampered["terminal"]["last"]["source_at"] = serde_json::json!(SAME_INSTANT);
    assert_serde_error::<AnomalyInputEvidence>(
        tampered,
        "anomaly input canonical digest does not match its evidence",
    );
}

#[test]
fn derivation_at_last_input_instant_preserves_rule_instrument_and_input_getters() {
    let event = event(GENERATION, FIRST_TIME).unwrap();
    assert_eq!(event.instrument().exchange(), Exchange::Beijing);
    assert_eq!(event.instrument().code(), "430047");
    assert_eq!(event.instrument().asset_class(), AssetClass::Equity);
    assert_eq!(event.rule().id(), "0.release-rule_v1");
    assert_eq!(event.rule().revision(), 1);
    assert_eq!(event.transition(), AnomalyTransition::Rearmed);
    assert_eq!(event.stream().sequence().get(), 8);
    assert_eq!(event.input_evidence().last_cursor().sequence().get(), 7);
    assert_eq!(
        event.input_evidence().terminal().last().observed_at(),
        SAME_INSTANT
    );
    assert_eq!(event.derived_evidence().observed_at(), FIRST_TIME);

    let decoded: AnomalyEvent =
        serde_json::from_value(serde_json::to_value(event).unwrap()).unwrap();
    assert_eq!(decoded.rule().id(), "0.release-rule_v1");
    assert_eq!(decoded.transition(), AnomalyTransition::Rearmed);
    assert_eq!(decoded.derived_evidence().observed_at(), FIRST_TIME);
}

#[test]
fn derivation_one_nanosecond_before_last_input_is_rejected_in_constructor_and_wire() {
    let reason = "anomaly derivation observation time must not precede its last input";
    assert_invalid_request(event(GENERATION, EARLIER_INSTANT), reason);
    let mut wire = serde_json::to_value(event(GENERATION, FIRST_TIME).unwrap()).unwrap();
    wire["derived_evidence"]["observed_at"] = serde_json::json!(EARLIER_INSTANT);
    assert_serde_error::<AnomalyEvent>(wire, reason);
}

#[test]
fn anomaly_event_cannot_cross_its_input_generation_in_constructor_or_wire() {
    let reason = "anomaly event and input range must use the same stream generation";
    assert_invalid_request(event(OTHER_GENERATION, FIRST_TIME), reason);
    let mut wire = serde_json::to_value(event(GENERATION, FIRST_TIME).unwrap()).unwrap();
    wire["stream"]["generation"] = serde_json::json!(OTHER_GENERATION);
    assert_serde_error::<AnomalyEvent>(wire, reason);
}

#[test]
fn source_status_accepts_gap_on_either_channel_and_exposes_instrument_and_union_getters() {
    for (status, observation, source) in [
        (
            SourceStatusKind::PossibleDataGap,
            ContinuityState::PossibleGap,
            ContinuityState::Continuous,
        ),
        (
            SourceStatusKind::PossibleDataGap,
            ContinuityState::Continuous,
            ContinuityState::KnownGap,
        ),
        (
            SourceStatusKind::KnownDataGap,
            ContinuityState::KnownGap,
            ContinuityState::Continuous,
        ),
        (
            SourceStatusKind::KnownDataGap,
            ContinuityState::Continuous,
            ContinuityState::KnownGap,
        ),
        (
            SourceStatusKind::Disconnected,
            ContinuityState::Unknown,
            ContinuityState::Continuous,
        ),
        (
            SourceStatusKind::ContinuityReset,
            ContinuityState::Continuous,
            ContinuityState::Unknown,
        ),
        (
            SourceStatusKind::Recovered,
            ContinuityState::KnownGap,
            ContinuityState::Continuous,
        ),
    ] {
        let previous = (status == SourceStatusKind::Recovered)
            .then(|| StreamGeneration::new(OTHER_GENERATION).unwrap());
        let event = SourceStatusEvent::new(
            status,
            Some(instrument()),
            cursor(GENERATION, 7),
            previous,
            FIRST_TIME,
            StreamContinuity::new(observation, source),
        )
        .unwrap();
        assert_eq!(event.schema_version(), 1);
        assert_eq!(event.instrument().unwrap().code(), "430047");
        assert_eq!(event.observed_at(), FIRST_TIME);
        assert_eq!(event.status(), status);
        assert_eq!(event.continuity().observation(), observation);
        assert_eq!(event.continuity().source(), source);
        assert_eq!(
            event.previous_generation().map(StreamGeneration::as_str),
            (status == SourceStatusKind::Recovered).then_some(OTHER_GENERATION)
        );

        let wrapped = MarketEvent::SourceStatus(event);
        assert_eq!(wrapped.stream().generation().as_str(), GENERATION);
        assert_eq!(wrapped.stream().sequence().get(), 7);
        let decoded: MarketEvent =
            serde_json::from_value(serde_json::to_value(&wrapped).unwrap()).unwrap();
        assert_eq!(decoded.event_id().as_str(), wrapped.event_id().as_str());
        match decoded {
            MarketEvent::SourceStatus(decoded) => {
                assert_eq!(decoded.instrument().unwrap().exchange(), Exchange::Beijing);
                assert_eq!(decoded.observed_at(), FIRST_TIME);
                assert_eq!(decoded.status(), status);
            }
            MarketEvent::MarketAnomaly(_) => panic!("source status decoded as anomaly"),
        }
    }
}

#[test]
fn status_deserialization_enforces_gap_and_disconnect_contracts() {
    for (status, wire_status, observation, source, reason) in [
        (
            SourceStatusKind::PossibleDataGap,
            "possible_data_gap",
            ContinuityState::Unknown,
            ContinuityState::Unknown,
            "possible-gap status requires possible or known gap continuity",
        ),
        (
            SourceStatusKind::Disconnected,
            "disconnected",
            ContinuityState::Continuous,
            ContinuityState::Continuous,
            "disconnect/reset status cannot report fully continuous input",
        ),
        (
            SourceStatusKind::ContinuityReset,
            "continuity_reset",
            ContinuityState::Continuous,
            ContinuityState::Continuous,
            "disconnect/reset status cannot report fully continuous input",
        ),
    ] {
        assert_invalid_request(
            SourceStatusEvent::new(
                status,
                None,
                cursor(GENERATION, 7),
                None,
                FIRST_TIME,
                StreamContinuity::new(observation, source),
            ),
            reason,
        );
        let valid = SourceStatusEvent::new(
            SourceStatusKind::PossibleDataGap,
            None,
            cursor(GENERATION, 7),
            None,
            FIRST_TIME,
            StreamContinuity::new(ContinuityState::PossibleGap, ContinuityState::Unknown),
        )
        .unwrap();
        let mut wire = serde_json::to_value(valid).unwrap();
        wire["status"] = serde_json::json!(wire_status);
        wire["continuity"] =
            serde_json::to_value(StreamContinuity::new(observation, source)).unwrap();
        assert_serde_error::<SourceStatusEvent>(wire, reason);
    }
}

#[test]
fn event_envelopes_and_committed_input_reject_unknown_wire_fields() {
    let valid_event = event(GENERATION, FIRST_TIME).unwrap();
    let mut event_wire = serde_json::to_value(&valid_event).unwrap();
    event_wire["unexpected"] = serde_json::json!(true);
    assert_serde_error::<AnomalyEvent>(event_wire, "unknown field `unexpected`");

    let mut input_wire = serde_json::to_value(valid_event.input_evidence()).unwrap();
    input_wire["unexpected"] = serde_json::json!(true);
    assert_serde_error::<AnomalyInputEvidence>(input_wire, "unknown field `unexpected`");

    let mut continuity_wire = serde_json::to_value(valid_event.input_evidence()).unwrap();
    continuity_wire["continuity"]["unexpected"] = serde_json::json!(true);
    assert_serde_error::<AnomalyInputEvidence>(continuity_wire, "unknown field `unexpected`");

    let status = SourceStatusEvent::new(
        SourceStatusKind::ContinuityReset,
        None,
        cursor(GENERATION, 7),
        None,
        FIRST_TIME,
        StreamContinuity::new(ContinuityState::Unknown, ContinuityState::Unknown),
    )
    .unwrap();
    let mut status_wire = serde_json::to_value(&status).unwrap();
    status_wire["unexpected"] = serde_json::json!(true);
    assert_serde_error::<SourceStatusEvent>(status_wire, "unknown field `unexpected`");

    let mut unsupported_schema = serde_json::to_value(status).unwrap();
    unsupported_schema["schema_version"] = serde_json::json!(2);
    assert_serde_error::<SourceStatusEvent>(
        unsupported_schema,
        "unsupported market event schema version 2",
    );
}

mod source_status_recovery_generation_public_contract {
    use magic_market_core::{
        ContinuityState, ProviderId, SourceStatusEvent, SourceStatusKind, StreamContinuity,
        StreamCursor, StreamGeneration, StreamSequence,
    };

    #[test]
    fn recovered_wire_status_requires_a_different_previous_generation() {
        let recovered = SourceStatusEvent::new(
            SourceStatusKind::Recovered,
            None,
            StreamCursor::new(
                StreamGeneration::new("550e8400-e29b-41d4-a716-446655440001").unwrap(),
                StreamSequence::new(1).unwrap(),
            ),
            Some(StreamGeneration::new("550e8400-e29b-41d4-a716-446655440000").unwrap()),
            "2026-10-10T10:21:45.310+08:00",
            StreamContinuity::new(ContinuityState::Continuous, ContinuityState::PossibleGap),
        )
        .unwrap();

        let mut wire = serde_json::to_value(recovered).unwrap();
        assert_eq!(wire["schema_version"], serde_json::json!(1));
        assert_eq!(wire["status"], serde_json::json!("recovered"));
        assert_eq!(wire["source_provider"], serde_json::json!("LocalTerminal"));
        assert_eq!(wire["instrument"], serde_json::Value::Null);
        assert_eq!(
            wire["stream"],
            serde_json::json!({
                "generation": "550e8400-e29b-41d4-a716-446655440001",
                "sequence": 1
            })
        );
        assert_eq!(
            wire["previous_generation"],
            serde_json::json!("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(
            wire["observed_at"],
            serde_json::json!("2026-10-10T10:21:45.310+08:00")
        );
        assert_eq!(
            wire["continuity"],
            serde_json::json!({"observation": "continuous", "source": "possible_gap"})
        );

        let decoded: SourceStatusEvent = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(decoded.status(), SourceStatusKind::Recovered);
        assert_eq!(decoded.source_provider(), ProviderId::LocalTerminal);
        assert!(decoded.instrument().is_none());
        assert_eq!(
            decoded.stream().generation().as_str(),
            "550e8400-e29b-41d4-a716-446655440001"
        );
        assert_eq!(decoded.stream().sequence().get(), 1);
        assert_eq!(
            decoded.previous_generation().unwrap().as_str(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(decoded.observed_at(), "2026-10-10T10:21:45.310+08:00");
        assert_eq!(
            decoded.continuity().observation(),
            ContinuityState::Continuous
        );
        assert_eq!(decoded.continuity().source(), ContinuityState::PossibleGap);

        // The valid envelope, including its identity, is otherwise unchanged.
        wire["previous_generation"] = serde_json::json!("550e8400-e29b-41d4-a716-446655440001");
        let error = serde_json::from_value::<SourceStatusEvent>(wire).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("source recovery must move to a different stream generation"),
            "recovery-generation contradiction was not diagnosed: {error}"
        );
    }
}

#[test]
fn anomaly_rule_id_rejects_a_forbidden_tail_in_constructor_and_wire() {
    use magic_market_core::{AnomalyRuleDigest, AnomalyRuleIdentity, CoreError};

    let digest = AnomalyRuleDigest::from_canonical_fields(&[("window_nanos", &[1])]).unwrap();
    let normal = AnomalyRuleIdentity::new("price-surge", 1, digest.clone()).unwrap();
    assert_eq!(normal.id(), "price-surge");
    assert_eq!(normal.revision(), 1);
    assert_eq!(normal.definition_digest(), &digest);

    let mut wire = serde_json::json!({
        "id": "price-surge",
        "revision": 1,
        "definition_digest": digest.as_str()
    });
    let decoded: AnomalyRuleIdentity = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(decoded.id(), "price-surge");
    assert_eq!(decoded.revision(), 1);
    assert_eq!(decoded.definition_digest(), &digest);
    assert_eq!(decoded, normal);

    assert_eq!(
        AnomalyRuleIdentity::new("price/surge", 1, digest).unwrap_err(),
        CoreError::InvalidValue {
            field: "anomaly_rule_id",
            value: "price/surge".into(),
            reason: "must contain only lowercase ASCII letters, digits, '-', '_' or '.'",
        }
    );

    // Only the allowed '-' becomes '/'; valid revision and commitment remain.
    wire["id"] = serde_json::json!("price/surge");
    let error = serde_json::from_value::<AnomalyRuleIdentity>(wire).unwrap_err();
    assert!(error.is_data());
    assert_eq!(
        error.to_string(),
        "invalid anomaly_rule_id: price/surge (must contain only lowercase ASCII letters, digits, '-', '_' or '.')"
    );
}

mod source_status_instrument_identity_public_contract {
    use magic_market_core::{
        AssetClass, ContinuityState, Exchange, InstrumentId, MarketEvent, ProviderId,
        SourceStatusEvent, SourceStatusKind, StreamContinuity, StreamCursor, StreamGeneration,
        StreamSequence,
    };
    use std::collections::HashSet;

    #[test]
    fn source_status_preserves_venue_category_identity_and_rejects_noncanonical_wire_labels() {
        // These are local status metadata, not claims that the synthetic code
        // is listed or that any provider has admitted a market-data family.
        let make_status = |exchange, asset_class| {
            SourceStatusEvent::new(
                SourceStatusKind::PossibleDataGap,
                Some(InstrumentId::new(exchange, "shared-code", asset_class).unwrap()),
                StreamCursor::new(
                    StreamGeneration::new("550e8400-e29b-41d4-a716-446655440000").unwrap(),
                    StreamSequence::new(7).unwrap(),
                ),
                None,
                "2026-10-10T10:21:45.310+08:00",
                StreamContinuity::new(ContinuityState::PossibleGap, ContinuityState::Unknown),
            )
            .unwrap()
        };

        // Each complete oracle is literal. Only the dynamic digest is projected
        // to a named placeholder; no production hasher supplies a golden ID.
        let cases = [
            (
                Exchange::Shanghai,
                AssetClass::Equity,
                r#"{
                "type":"source_status","schema_version":1,"event_id":"<canonical-id>",
                "source_provider":"LocalTerminal","status":"possible_data_gap",
                "instrument":{"exchange":"Shanghai","code":"shared-code","asset_class":"Equity"},
                "stream":{"generation":"550e8400-e29b-41d4-a716-446655440000","sequence":7},
                "previous_generation":null,"observed_at":"2026-10-10T10:21:45.310+08:00",
                "continuity":{"observation":"possible_gap","source":"unknown"}
            }"#,
            ),
            (
                Exchange::Shanghai,
                AssetClass::Index,
                r#"{
                "type":"source_status","schema_version":1,"event_id":"<canonical-id>",
                "source_provider":"LocalTerminal","status":"possible_data_gap",
                "instrument":{"exchange":"Shanghai","code":"shared-code","asset_class":"Index"},
                "stream":{"generation":"550e8400-e29b-41d4-a716-446655440000","sequence":7},
                "previous_generation":null,"observed_at":"2026-10-10T10:21:45.310+08:00",
                "continuity":{"observation":"possible_gap","source":"unknown"}
            }"#,
            ),
            (
                Exchange::Shanghai,
                AssetClass::Fund,
                r#"{
                "type":"source_status","schema_version":1,"event_id":"<canonical-id>",
                "source_provider":"LocalTerminal","status":"possible_data_gap",
                "instrument":{"exchange":"Shanghai","code":"shared-code","asset_class":"Fund"},
                "stream":{"generation":"550e8400-e29b-41d4-a716-446655440000","sequence":7},
                "previous_generation":null,"observed_at":"2026-10-10T10:21:45.310+08:00",
                "continuity":{"observation":"possible_gap","source":"unknown"}
            }"#,
            ),
            (
                Exchange::Shanghai,
                AssetClass::Bond,
                r#"{
                "type":"source_status","schema_version":1,"event_id":"<canonical-id>",
                "source_provider":"LocalTerminal","status":"possible_data_gap",
                "instrument":{"exchange":"Shanghai","code":"shared-code","asset_class":"Bond"},
                "stream":{"generation":"550e8400-e29b-41d4-a716-446655440000","sequence":7},
                "previous_generation":null,"observed_at":"2026-10-10T10:21:45.310+08:00",
                "continuity":{"observation":"possible_gap","source":"unknown"}
            }"#,
            ),
            (
                Exchange::Shanghai,
                AssetClass::Option,
                r#"{
                "type":"source_status","schema_version":1,"event_id":"<canonical-id>",
                "source_provider":"LocalTerminal","status":"possible_data_gap",
                "instrument":{"exchange":"Shanghai","code":"shared-code","asset_class":"Option"},
                "stream":{"generation":"550e8400-e29b-41d4-a716-446655440000","sequence":7},
                "previous_generation":null,"observed_at":"2026-10-10T10:21:45.310+08:00",
                "continuity":{"observation":"possible_gap","source":"unknown"}
            }"#,
            ),
            (
                Exchange::Shenzhen,
                AssetClass::Equity,
                r#"{
                "type":"source_status","schema_version":1,"event_id":"<canonical-id>",
                "source_provider":"LocalTerminal","status":"possible_data_gap",
                "instrument":{"exchange":"Shenzhen","code":"shared-code","asset_class":"Equity"},
                "stream":{"generation":"550e8400-e29b-41d4-a716-446655440000","sequence":7},
                "previous_generation":null,"observed_at":"2026-10-10T10:21:45.310+08:00",
                "continuity":{"observation":"possible_gap","source":"unknown"}
            }"#,
            ),
            (
                Exchange::Beijing,
                AssetClass::Equity,
                r#"{
                "type":"source_status","schema_version":1,"event_id":"<canonical-id>",
                "source_provider":"LocalTerminal","status":"possible_data_gap",
                "instrument":{"exchange":"Beijing","code":"shared-code","asset_class":"Equity"},
                "stream":{"generation":"550e8400-e29b-41d4-a716-446655440000","sequence":7},
                "previous_generation":null,"observed_at":"2026-10-10T10:21:45.310+08:00",
                "continuity":{"observation":"possible_gap","source":"unknown"}
            }"#,
            ),
        ];

        let mut identities = HashSet::new();
        let mut controls = Vec::new();
        for (exchange, asset_class, literal) in cases {
            let event = make_status(exchange, asset_class);
            let event_id = event.event_id().as_str().to_owned();
            assert_eq!(event_id.len(), 64);
            assert!(event_id
                .bytes()
                .all(|byte| { byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) }));
            assert!(
                identities.insert(event_id.clone()),
                "distinct instrument identities collided"
            );
            assert_eq!(
                make_status(exchange, asset_class).event_id().as_str(),
                event_id
            );

            let wrapped = MarketEvent::SourceStatus(event);
            let wire = serde_json::to_value(&wrapped).unwrap();
            assert_eq!(wire["event_id"], serde_json::json!(&event_id));
            let expected: serde_json::Value = serde_json::from_str(literal).unwrap();
            let mut projected = wire.clone();
            projected["event_id"] = serde_json::json!("<canonical-id>");
            assert_eq!(
                projected, expected,
                "complete wire oracle for {exchange:?}/{asset_class:?}"
            );

            let decoded: MarketEvent = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(decoded.event_id().as_str(), event_id);
            match &decoded {
                MarketEvent::SourceStatus(status) => {
                    assert_eq!(status.schema_version(), 1);
                    assert_eq!(status.source_provider(), ProviderId::LocalTerminal);
                    assert_eq!(status.status(), SourceStatusKind::PossibleDataGap);
                    let instrument = status.instrument().unwrap();
                    assert_eq!(instrument.exchange(), exchange);
                    assert_eq!(instrument.asset_class(), asset_class);
                    assert_eq!(instrument.code(), "shared-code");
                    assert_eq!(
                        status.stream().generation().as_str(),
                        "550e8400-e29b-41d4-a716-446655440000"
                    );
                    assert_eq!(status.stream().sequence().get(), 7);
                    assert!(status.previous_generation().is_none());
                    assert_eq!(status.observed_at(), "2026-10-10T10:21:45.310+08:00");
                    assert_eq!(
                        status.continuity().observation(),
                        ContinuityState::PossibleGap
                    );
                    assert_eq!(status.continuity().source(), ContinuityState::Unknown);
                }
                MarketEvent::MarketAnomaly(_) => panic!("source-status tag decoded as anomaly"),
            }
            assert_eq!(serde_json::to_value(&decoded).unwrap(), wire);
            controls.push(wire);
        }
        assert_eq!(identities.len(), 7);
        assert_eq!(controls.len(), 7);
        let unchanged_controls = controls.clone();

        // Every destination already succeeded through the public constructor
        // and Deserialize above. Only one valid label changes, with old ID kept.
        for (from, to, field, replacement) in [
            (0, 1, "asset_class", "Index"),
            (1, 2, "asset_class", "Fund"),
            (2, 3, "asset_class", "Bond"),
            (3, 4, "asset_class", "Option"),
            (4, 0, "asset_class", "Equity"),
            (0, 5, "exchange", "Shenzhen"),
            (0, 6, "exchange", "Beijing"),
        ] {
            let mut changed = controls[from].clone();
            changed["instrument"][field] = serde_json::json!(replacement);
            let mut destination_with_old_id = controls[to].clone();
            destination_with_old_id["event_id"] = controls[from]["event_id"].clone();
            assert_eq!(
                changed, destination_with_old_id,
                "only the identity label may change"
            );
            assert_ne!(controls[from]["event_id"], controls[to]["event_id"]);
            let error = serde_json::from_value::<MarketEvent>(changed).unwrap_err();
            assert!(error.is_data());
            assert_eq!(
                error.to_string(),
                "source status event ID does not match its canonical identity"
            );
        }

        // The exact rejected lexeme and Data category identify enum parsing,
        // not an earlier unrelated field fault or the later identity comparison.
        for (pointer, label, reason_prefix) in [
            (
                "/instrument/asset_class",
                "index",
                "unknown variant `index`, expected",
            ),
            (
                "/instrument/asset_class",
                "Index ",
                "unknown variant `Index `, expected",
            ),
            (
                "/instrument/exchange",
                "shenzhen",
                "unknown variant `shenzhen`, expected",
            ),
            (
                "/instrument/exchange",
                "Shenzhen ",
                "unknown variant `Shenzhen `, expected",
            ),
            (
                "/type",
                "SourceStatus",
                "unknown variant `SourceStatus`, expected",
            ),
            (
                "/type",
                "source_status ",
                "unknown variant `source_status `, expected",
            ),
        ] {
            let mut changed = controls[0].clone();
            *changed.pointer_mut(pointer).unwrap() = serde_json::json!(label);
            let error = serde_json::from_value::<MarketEvent>(changed).unwrap_err();
            assert!(error.is_data(), "{pointer}: {error}");
            assert!(
                error.to_string().starts_with(reason_prefix),
                "{pointer}: {error}"
            );
        }
        assert_eq!(
            controls, unchanged_controls,
            "refusal must not alter the valid controls"
        );
    }
}
