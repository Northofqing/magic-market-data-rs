use super::*;
use magic_market_core::{AssetClass, Exchange};
use magic_tencent_rs::SnapshotTransport;

const INDEX_BODY: &[u8] = br#"v_sh000001="1~Offline index~000001~3876.78~3867.03~3868.09~562122601~0~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~~20260723161420~9.75~0.25~3878.83~3851.71~3876.78/562122601/1025875517700~562122601~102587552~1.16~17.30~~3878.83~3851.71~0.70~602482.43~646317.73~0.00~-1~-1~0.87~0~3867.94~";"#;

struct IndexDocument {
    body: Vec<u8>,
    calls: Arc<AtomicUsize>,
}

impl SnapshotTransport for IndexDocument {
    fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.body.clone())
    }
}

fn index() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).unwrap()
}

fn registry(body: Vec<u8>, ceiling: usize) -> (OperationRegistry, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = TencentClient::with_transport(IndexDocument {
        body,
        calls: calls.clone(),
    });
    let registry = registry_with_tencent(client, Duration::from_secs(1), ceiling).unwrap();
    (registry, calls)
}

fn command(age: u64) -> QueryCommand {
    fixture_command(
        Operation::IndexQuotes,
        "Tencent",
        INDEX_QUOTES_REQUEST_SCHEMA,
        1,
        &serde_json::to_vec(&serde_json::json!({
            "indices": [index()], "maximum_source_age_millis": age
        }))
        .unwrap(),
    )
}

#[test]
fn registered_index_route_keeps_index_identity_units_and_original_source_time() {
    let (registry, calls) = registry(INDEX_BODY.to_vec(), 65_536);
    // Explicit caller policy accepts this fixed historical fixture, not a live signal.
    let result = registry.execute(command(u64::MAX)).unwrap();
    assert!(result.repository_admitted && result.complete);
    assert_eq!(result.provider, "Tencent");
    assert_eq!(
        result.source_at.as_deref(),
        Some("2026-07-23T16:14:20+08:00")
    );
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].schema(), INDEX_QUOTES_RECORD_SCHEMA);
    assert_eq!(result.records[0].schema_version(), 1);
    let quote: Quote = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(quote.instrument(), &index());
    assert_eq!(quote.name(), Some("Offline index"));
    assert_eq!(quote.provider(), ProviderId::Tencent);
    assert_eq!(quote.status(), DataStatus::Available);
    assert_eq!(quote.price().get(), 3876.78);
    assert_eq!(quote.volume().get(), 562_122_601.0);
    // The exact price/volume/amount source composite supplies this CNY amount.
    assert_eq!(quote.amount().unwrap().get(), 1_025_875_517_700.0);
    assert_eq!(quote.source_at(), Some("2026-07-23T16:14:20+08:00"));
    assert_eq!(quote.batch_id(), result.batch_id);
    assert_eq!(quote.observed_at(), result.observed_at);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_index_route_refuses_oversized_output_instead_of_partial_success() {
    let (registry, calls) = registry(INDEX_BODY.to_vec(), 1);
    let error = registry.execute(command(u64::MAX)).unwrap_err();
    assert!(
        matches!(error, ServiceError::ResourceExhausted(ref reason)
        if reason.contains("exceeds maximum 1")),
        "{error:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_index_route_rejects_stale_source_without_promoting_observation_time() {
    let (registry, calls) = registry(INDEX_BODY.to_vec(), 65_536);
    let error = registry.execute(command(1)).unwrap_err();
    let ServiceError::ProviderRouteFailure {
        operation,
        exhausted,
        attempts,
    } = error
    else {
        panic!("unexpected stale-source result: {error:?}");
    };
    assert_eq!(operation, Operation::IndexQuotes);
    assert!(exhausted);
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].provider(), "Tencent");
    assert_eq!(attempts[0].outcome(), "rejected");
    assert_eq!(attempts[0].reason_code(), "quality");
    assert!(!attempts[0].retryable() && !attempts[0].terminal());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_index_route_rejects_missing_source_time_even_with_large_age_allowance() {
    let body = String::from_utf8(INDEX_BODY.to_vec())
        .unwrap()
        .replace("20260723161420", "")
        .into_bytes();
    let (registry, calls) = registry(body, 65_536);
    let error = registry.execute(command(u64::MAX)).unwrap_err();
    let ServiceError::ProviderRouteFailure {
        operation,
        exhausted,
        attempts,
    } = error
    else {
        panic!("unexpected missing-source result: {error:?}");
    };
    assert_eq!(operation, Operation::IndexQuotes);
    assert!(exhausted);
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].provider(), "Tencent");
    assert_eq!(attempts[0].outcome(), "rejected");
    assert_eq!(attempts[0].reason_code(), "quality");
    assert!(!attempts[0].retryable() && !attempts[0].terminal());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_index_route_exposes_terminal_protocol_failure_not_empty_success() {
    let (registry, calls) = registry(br#"v_sh000001="1~Broken~000001";"#.to_vec(), 65_536);
    let error = registry.execute(command(u64::MAX)).unwrap_err();
    let ServiceError::ProviderRouteFailure {
        operation,
        exhausted,
        attempts,
    } = error
    else {
        panic!("unexpected protocol result: {error:?}");
    };
    assert_eq!(operation, Operation::IndexQuotes);
    assert!(!exhausted);
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].provider(), "Tencent");
    assert_eq!(attempts[0].outcome(), "failed");
    assert_eq!(attempts[0].reason_code(), "protocol");
    assert!(!attempts[0].retryable() && attempts[0].terminal());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

enum IndexSourceFault {
    Transport,
}

struct FailingIndexDocument {
    fault: IndexSourceFault,
    calls: Arc<AtomicUsize>,
}

impl SnapshotTransport for FailingIndexDocument {
    fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.fault {
            IndexSourceFault::Transport => Err(TencentError::Transport(
                "offline raw diagnostic MUST_NOT_ESCAPE".into(),
            )),
        }
    }
}

fn fault_registry(fault: IndexSourceFault) -> (OperationRegistry, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = TencentClient::with_transport(FailingIndexDocument {
        fault,
        calls: calls.clone(),
    });
    (
        registry_with_tencent(client, Duration::from_secs(1), 65_536).unwrap(),
        calls,
    )
}

#[test]
fn registered_index_route_keeps_retryable_outage_trace_without_exposing_raw_detail() {
    let (registry, calls) = fault_registry(IndexSourceFault::Transport);
    let error = registry.execute(command(u64::MAX)).unwrap_err();
    assert!(!format!("{error:?}").contains("MUST_NOT_ESCAPE"));
    let ServiceError::ProviderRouteFailure {
        operation,
        exhausted,
        attempts,
    } = error
    else {
        panic!("unexpected outage result: {error:?}");
    };
    assert_eq!(operation, Operation::IndexQuotes);
    assert!(exhausted);
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].provider(), "Tencent");
    assert_eq!(attempts[0].outcome(), "failed");
    assert_eq!(attempts[0].reason_code(), "transport");
    assert!(attempts[0].retryable() && !attempts[0].terminal());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_index_requests_reject_invalid_identities_cardinality_and_policy_before_transport() {
    let (registry, calls) = registry(Vec::new(), 65_536);
    let indices: Vec<_> = (1..=7)
        .map(|number| {
            InstrumentId::new(
                Exchange::Shanghai,
                format!("{number:06}"),
                AssetClass::Index,
            )
            .unwrap()
        })
        .collect();
    let mut unknown = serde_json::json!({"indices": [index()], "maximum_source_age_millis": 5000});
    unknown["allow_stale"] = serde_json::json!(true);
    let cases = [
        serde_json::json!({"indices": [], "maximum_source_age_millis": 5000}),
        serde_json::json!({"indices": indices, "maximum_source_age_millis": 5000}),
        serde_json::json!({"indices": [index(), index()], "maximum_source_age_millis": 5000}),
        serde_json::json!({"indices": [index()], "maximum_source_age_millis": 0}),
        serde_json::json!({"indices": [{"exchange":"Shanghai", "code":"600396", "asset_class":"Equity"}], "maximum_source_age_millis": 5000}),
        serde_json::json!({"indices": [{"exchange":"Beijing", "code":"899050", "asset_class":"Index"}], "maximum_source_age_millis": 5000}),
        serde_json::json!({"indices": [{"exchange":"Shanghai", "code":"00001", "asset_class":"Index"}], "maximum_source_age_millis": 5000}),
        unknown,
    ];
    for data in cases {
        let query = fixture_command(
            Operation::IndexQuotes,
            "Tencent",
            INDEX_QUOTES_REQUEST_SCHEMA,
            1,
            &serde_json::to_vec(&data).unwrap(),
        );
        let error = registry.execute(query).unwrap_err();
        assert!(
            matches!(error, ServiceError::InvalidRequest(_)),
            "{error:?}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn registered_t0_requests_refuse_invalid_capture_bounds_and_identity_in_the_decoder() {
    let (registry, _tencent_calls) = registry(Vec::new(), 65_536);
    let equity =
        serde_json::json!({"exchange":"Shanghai", "code":"600396", "asset_class":"Equity"});
    let base = serde_json::json!({"instruments": [equity.clone()], "daily_bar_count": 20,
        "five_minute_bar_count": 48, "requested_at": "2026-07-23T09:24:10.123456+08:00"});
    let nine: Vec<_> = (1..=9)
        .map(|number| {
            serde_json::json!({
                "exchange":"Shanghai", "code":format!("600{number:03}"), "asset_class":"Equity"
            })
        })
        .collect();
    let changes = [
        ("daily_bar_count", serde_json::json!(801)),
        ("five_minute_bar_count", serde_json::json!(801)),
        ("daily_bar_count", serde_json::json!(0)),
        ("instruments", serde_json::json!([])),
        ("instruments", serde_json::json!([equity.clone(), equity])),
        ("instruments", serde_json::json!(nine)),
        ("instruments", serde_json::json!([index()])),
        ("requested_at", serde_json::json!("2026-07-23T09:24:10")),
        ("requested_at", serde_json::json!("")),
        ("allow_unadmitted", serde_json::json!(true)),
    ];
    for (field, value) in changes {
        let mut data = base.clone();
        data[field] = value;
        let query = fixture_command(
            Operation::T0Evidence,
            "Tdx",
            T0_EVIDENCE_REQUEST_SCHEMA,
            2,
            &serde_json::to_vec(&data).unwrap(),
        );
        let error = registry.execute(query).unwrap_err();
        assert!(
            matches!(error, ServiceError::InvalidRequest(_)),
            "{field}: {error:?}"
        );
    }
    let mut missing_capture = base;
    missing_capture
        .as_object_mut()
        .unwrap()
        .remove("requested_at");
    let error = registry
        .execute(fixture_command(
            Operation::T0Evidence,
            "Tdx",
            T0_EVIDENCE_REQUEST_SCHEMA,
            2,
            &serde_json::to_vec(&missing_capture).unwrap(),
        ))
        .unwrap_err();
    assert!(
        matches!(error, ServiceError::InvalidRequest(_)),
        "{error:?}"
    );
    // Invalid TDX requests only: the Tencent counter is not a TCP invocation witness.
}

#[test]
fn registered_outcome_requests_refuse_early_or_unqualified_session_deadlines() {
    let (registry, _tencent_calls) = registry(Vec::new(), 65_536);
    let base = serde_json::json!({
        "instrument":{"exchange":"Shanghai", "code":"600396", "asset_class":"Equity"},
        "through":"2026-07-23", "limit":20, "outcome_due_at":"2026-07-23T15:35:00+08:00"
    });
    let changes = [
        (
            "outcome_due_at",
            serde_json::json!("2026-07-23T14:59:59+08:00"),
        ),
        ("outcome_due_at", serde_json::json!("2026-07-23T06:59:59Z")),
        ("outcome_due_at", serde_json::json!("2026-07-23T15:35:00")),
        ("outcome_due_at", serde_json::json!("")),
        ("through", serde_json::json!("2026-02-29")),
        ("limit", serde_json::json!(801)),
        ("limit", serde_json::json!(0)),
        ("instrument", serde_json::json!(index())),
        ("allow_partial", serde_json::json!(true)),
    ];
    for (field, value) in changes {
        let mut data = base.clone();
        data[field] = value;
        let error = registry
            .execute(fixture_command(
                Operation::OutcomeDailyBars,
                "Tdx",
                OUTCOME_DAILY_BARS_REQUEST_SCHEMA,
                1,
                &serde_json::to_vec(&data).unwrap(),
            ))
            .unwrap_err();
        assert!(
            matches!(error, ServiceError::InvalidRequest(_)),
            "{field}: {error:?}"
        );
    }
}
