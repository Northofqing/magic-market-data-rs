use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use super::*;

#[test]
fn production_registry_rejects_zero_timeout_before_provider_construction() {
    assert!(matches!(
        production_operation_registry(Duration::ZERO, 4096),
        Err(ProductionRegistryError::InvalidLimit(
            "provider timeout must be positive"
        ))
    ));
}

#[test]
fn production_registry_rejects_zero_payload_limit_before_provider_construction() {
    assert!(matches!(
        production_operation_registry(Duration::from_secs(1), 0),
        Err(ProductionRegistryError::InvalidLimit(
            "maximum payload bytes must be positive"
        ))
    ));
}

#[test]
fn production_registry_validates_timeout_first_when_both_limits_are_zero() {
    assert!(matches!(
        production_operation_registry(Duration::ZERO, 0),
        Err(ProductionRegistryError::InvalidLimit(
            "provider timeout must be positive"
        ))
    ));
}

const FIXTURE_OBSERVED_AT: &str = "2026-08-19T16:16:00+08:00";
const FIXTURE_SOURCE_AT: &str = "2026-08-19T16:15:00+08:00";
const FIXTURE_BATCH_ID: &str = "offline-handler-news-fixture";

enum NewsOutcome {
    Batch(Box<DataBatch<NewsItem>>),
    HttpStatus(u16),
    Transport,
}

struct FixtureNewsProvider {
    outcome: NewsOutcome,
    calls: Arc<AtomicUsize>,
    requested_limit: Arc<AtomicU32>,
}

impl NewsProvider for FixtureNewsProvider {
    type Error = ClsError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        panic!("the global-news handler must not invoke instrument news")
    }

    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.requested_limit.store(limit.get(), Ordering::SeqCst);
        match &self.outcome {
            NewsOutcome::Batch(batch) => Ok((**batch).clone()),
            NewsOutcome::HttpStatus(status) => Err(ClsError::HttpStatus(*status)),
            NewsOutcome::Transport => Err(ClsError::Transport("private-fixture-detail".into())),
        }
    }
}

fn fixture_news_batch(provider: ProviderId) -> DataBatch<NewsItem> {
    DataBatch::strict(
        vec![NewsItem {
            item_id: NonEmptyText::new("fixture-rubin").unwrap(),
            title: NonEmptyText::new("fixture NVIDIA Rubin").unwrap(),
            summary: None,
            content: None,
            publisher: NonEmptyText::new("offline fixture").unwrap(),
            canonical_url: magic_market_core::HttpsUrl::new(
                "https://example.invalid/fixture-rubin",
            )
            .unwrap(),
            published_at: NonEmptyText::new(FIXTURE_SOURCE_AT).unwrap(),
            instruments: Vec::new(),
            topics: Vec::new(),
            language: NonEmptyText::new("en").unwrap(),
            evidence: SourceEvidence::new(provider, FIXTURE_OBSERVED_AT, FIXTURE_BATCH_ID)
                .unwrap()
                .with_source_at(FIXTURE_SOURCE_AT)
                .unwrap(),
        }],
        Provenance::new("cailianpress", FIXTURE_OBSERVED_AT)
            .unwrap()
            .with_source_at(FIXTURE_SOURCE_AT)
            .unwrap()
            .with_batch_id(FIXTURE_BATCH_ID)
            .unwrap(),
    )
}

fn fixture_news_registry(
    outcome: NewsOutcome,
    maximum_payload_bytes: usize,
) -> (OperationRegistry, Arc<AtomicUsize>, Arc<AtomicU32>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let requested_limit = Arc::new(AtomicU32::new(0));
    let client = FixtureNewsProvider {
        outcome,
        calls: calls.clone(),
        requested_limit: requested_limit.clone(),
    };
    let mut registry = OperationRegistry::all_unadmitted("offline fixture only");
    register_global_news_provider(
        &mut registry,
        client,
        "Cailianpress",
        ProviderId::Cailianpress,
        "offline fixture; not a source admission witness",
        maximum_payload_bytes,
    )
    .unwrap();
    (registry, calls, requested_limit)
}

fn fixture_command(
    operation: Operation,
    provider: &str,
    schema: &str,
    version: u32,
    data: &[u8],
) -> QueryCommand {
    QueryCommand::new(
        "offline-handler-request",
        operation,
        Some(provider.to_owned()),
        CanonicalPayload::new(schema, version, data.to_vec(), 4096).unwrap(),
    )
    .unwrap()
}

fn valid_news_command() -> QueryCommand {
    fixture_command(
        Operation::GlobalNews,
        "Cailianpress",
        GLOBAL_NEWS_REQUEST_SCHEMA,
        2,
        br#"{"limit":2}"#,
    )
}

#[test]
fn available_production_handlers_reject_a_pinned_wrong_schema() {
    let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
    let mut checked = 0;
    let mut checked_news = false;
    for capability in registry.capabilities() {
        if !capability.repository_admitted || !capability.runtime_available {
            continue;
        }
        let command = fixture_command(
            capability.operation,
            &capability.provider,
            "magic.market.invalid_schema.request",
            99,
            br#"{}"#,
        );
        let result = registry.execute(command);
        assert!(
            matches!(result, Err(ServiceError::InvalidRequest(_))),
            "{:?}/{} accepted or misclassified a wrong envelope: {result:?}",
            capability.operation,
            capability.provider,
        );
        checked += 1;
        checked_news |= capability.operation == Operation::GlobalNews;
    }
    assert!(
        checked > 0 && checked_news,
        "the handler matrix must not be empty"
    );
}

#[test]
fn diagnostic_handlers_reject_ordinary_access_without_opt_in() {
    let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
    let mut checked = 0;
    for capability in registry.capabilities() {
        if capability.repository_admitted || !capability.diagnostic_available {
            continue;
        }
        let command = fixture_command(
            capability.operation,
            &capability.provider,
            "magic.market.invalid_schema.request",
            99,
            br#"{}"#,
        );
        assert!(matches!(
            registry.execute(command),
            Err(ServiceError::Unsupported { operation, .. })
                if operation == capability.operation
        ));
        checked += 1;
    }
    assert!(
        checked > 0,
        "the diagnostic-access matrix must not be empty"
    );
}

#[test]
fn registered_news_handler_rejects_bad_envelopes_before_provider_call() {
    let (registry, calls, _) = fixture_news_registry(
        NewsOutcome::Batch(Box::new(fixture_news_batch(ProviderId::Cailianpress))),
        4096,
    );
    let cases: [(&str, u32, &[u8]); 5] = [
        ("magic.market.invalid_schema.request", 2, br#"{"limit":2}"#),
        (GLOBAL_NEWS_REQUEST_SCHEMA, 1, br#"{"limit":2}"#),
        (GLOBAL_NEWS_REQUEST_SCHEMA, 2, br#"{"limit":0}"#),
        (GLOBAL_NEWS_REQUEST_SCHEMA, 2, br#"{}"#),
        (GLOBAL_NEWS_REQUEST_SCHEMA, 2, b"[]"),
    ];
    for (schema, version, bytes) in cases {
        let command = fixture_command(
            Operation::GlobalNews,
            "Cailianpress",
            schema,
            version,
            bytes,
        );
        assert!(matches!(
            registry.execute(command),
            Err(ServiceError::InvalidRequest(_))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn canonical_news_envelope_rejects_malformed_json_before_dispatch() {
    assert!(matches!(
        CanonicalPayload::new(GLOBAL_NEWS_REQUEST_SCHEMA, 2, b"[".to_vec(), 4096),
        Err(ServiceError::InvalidRequest(_))
    ));
}

#[test]
fn registered_news_handler_preserves_v2_evidence_and_the_requested_limit() {
    let (registry, calls, limit) = fixture_news_registry(
        NewsOutcome::Batch(Box::new(fixture_news_batch(ProviderId::Cailianpress))),
        4096,
    );
    let result = registry.execute(valid_news_command()).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(limit.load(Ordering::SeqCst), 2);
    assert_eq!(result.provider, "Cailianpress");
    assert_eq!(result.batch_id, "offline-handler-news-fixture");
    assert_eq!(result.observed_at, "2026-08-19T16:16:00+08:00");
    assert_eq!(
        result.source_at.as_deref(),
        Some("2026-08-19T16:15:00+08:00")
    );
    assert!(result.complete && result.repository_admitted);
    assert!(result.diagnostic_blocker.is_none());
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].schema(), GLOBAL_NEWS_RECORD_SCHEMA);
    assert_eq!(result.records[0].schema_version(), 2);
    let record: serde_json::Value = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(record["title"], "fixture NVIDIA Rubin");
    assert_eq!(record["evidence"]["provider"], "Cailianpress");
    assert_eq!(record["evidence"]["source_at"], "2026-08-19T16:15:00+08:00");
    assert!(record["summary"].is_null() && record["content"].is_null());
}

#[test]
fn registered_news_handler_preserves_distinct_http_failure_classes() {
    let cases = [
        (
            401,
            ProviderFailureKind::AuthenticationRejected,
            "http_status=401",
        ),
        (
            403,
            ProviderFailureKind::AuthenticationRejected,
            "http_status=403",
        ),
        (429, ProviderFailureKind::RateLimited, "http_status=429"),
        (503, ProviderFailureKind::Unavailable, "http_status=503"),
        (404, ProviderFailureKind::QueryRejected, "http_status=404"),
    ];
    for (status, expected_kind, expected_reason) in cases {
        let (registry, calls, _) = fixture_news_registry(NewsOutcome::HttpStatus(status), 4096);
        let error = registry.execute(valid_news_command()).unwrap_err();
        assert!(matches!(
            error,
            ServiceError::ProviderFailure { operation, provider, kind, provider_reason }
                if operation == Operation::GlobalNews && provider == "Cailianpress"
                    && kind == expected_kind && provider_reason == expected_reason
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn registered_news_handler_does_not_expose_transport_details() {
    let (registry, calls, _) = fixture_news_registry(NewsOutcome::Transport, 4096);
    let error = registry.execute(valid_news_command()).unwrap_err();
    assert!(!format!("{error:?}").contains("private-fixture-detail"));
    assert!(matches!(
        error,
        ServiceError::ProviderFailure { operation, provider, kind, provider_reason }
            if operation == Operation::GlobalNews && provider == "Cailianpress"
                && kind == ProviderFailureKind::Unavailable && provider_reason == "category=transport"
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_news_handler_rejects_conflicting_provider_evidence_atomically() {
    let (registry, calls, _) = fixture_news_registry(
        NewsOutcome::Batch(Box::new(fixture_news_batch(ProviderId::Jin10))),
        4096,
    );
    assert!(matches!(
        registry.execute(valid_news_command()),
        Err(ServiceError::InvalidEvidence { provider, evidence_code, evidence_field, record_index, .. })
            if provider == "Cailianpress" && evidence_code == "record_provider_mismatch"
                && evidence_field == "evidence.provider" && record_index == Some(0)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_news_handler_rejects_an_oversized_response_instead_of_partial_success() {
    let (registry, calls, _) = fixture_news_registry(
        NewsOutcome::Batch(Box::new(fixture_news_batch(ProviderId::Cailianpress))),
        32,
    );
    assert!(matches!(
        registry.execute(valid_news_command()),
        Err(ServiceError::ResourceExhausted(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_news_handler_rejects_incomplete_or_missing_batch_evidence() {
    let original = fixture_news_batch(ProviderId::Cailianpress);
    let records = original.records().to_vec();
    let provenance = original.provenance().clone();
    let cases = [
        (
            DataBatch::best_effort(
                records.clone(),
                provenance.clone(),
                vec!["offline fixture page missing".to_owned()],
            )
            .unwrap(),
            "batch_quality_incomplete",
            "quality",
        ),
        (
            DataBatch::strict(Vec::new(), provenance),
            "batch_records_empty",
            "records",
        ),
        (
            DataBatch::strict(
                records.clone(),
                Provenance::new("cailianpress", FIXTURE_OBSERVED_AT)
                    .unwrap()
                    .with_batch_id(FIXTURE_BATCH_ID)
                    .unwrap(),
            ),
            "batch_evidence_incomplete",
            "source_at",
        ),
        (
            DataBatch::strict(
                records.clone(),
                Provenance::new("cailianpress", FIXTURE_OBSERVED_AT)
                    .unwrap()
                    .with_batch_id(FIXTURE_BATCH_ID)
                    .unwrap()
                    .with_source_at("not-an-instant")
                    .unwrap(),
            ),
            "batch_source_at_invalid",
            "source_at",
        ),
        (
            DataBatch::strict(
                records,
                Provenance::new("cailianpress", "not-an-instant")
                    .unwrap()
                    .with_batch_id(FIXTURE_BATCH_ID)
                    .unwrap()
                    .with_source_at(FIXTURE_SOURCE_AT)
                    .unwrap(),
            ),
            "batch_observed_at_invalid",
            "observed_at",
        ),
    ];
    for (batch, expected_code, expected_field) in cases {
        let (registry, calls, _) = fixture_news_registry(NewsOutcome::Batch(Box::new(batch)), 4096);
        assert!(
            matches!(
                registry.execute(valid_news_command()),
                Err(ServiceError::InvalidEvidence { provider, evidence_code, evidence_field, record_index, .. })
                    if provider == "Cailianpress" && evidence_code == expected_code
                        && evidence_field == expected_field && record_index.is_none()
            ),
            "batch case {expected_code}/{expected_field}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn registered_news_handler_rejects_inconsistent_record_evidence() {
    let original = fixture_news_batch(ProviderId::Cailianpress);
    let cases = [
        (
            Some(FIXTURE_SOURCE_AT),
            FIXTURE_OBSERVED_AT,
            "other-fixture-batch",
            FIXTURE_SOURCE_AT,
            "record_batch_mismatch",
            "evidence.batch_id",
        ),
        (
            None,
            FIXTURE_OBSERVED_AT,
            FIXTURE_BATCH_ID,
            FIXTURE_SOURCE_AT,
            "record_evidence_incomplete",
            "evidence.source_at",
        ),
        (
            Some("not-an-instant"),
            FIXTURE_OBSERVED_AT,
            FIXTURE_BATCH_ID,
            FIXTURE_SOURCE_AT,
            "record_source_at_invalid",
            "evidence.source_at",
        ),
        (
            Some(FIXTURE_SOURCE_AT),
            FIXTURE_OBSERVED_AT,
            FIXTURE_BATCH_ID,
            "not-an-instant",
            "record_published_at_invalid",
            "published_at",
        ),
        (
            Some(FIXTURE_SOURCE_AT),
            FIXTURE_OBSERVED_AT,
            FIXTURE_BATCH_ID,
            "2026-08-19T16:14:00+08:00",
            "record_published_at_mismatch",
            "published_at",
        ),
        (
            Some(FIXTURE_SOURCE_AT),
            "not-an-instant",
            FIXTURE_BATCH_ID,
            FIXTURE_SOURCE_AT,
            "record_observed_at_invalid",
            "evidence.observed_at",
        ),
        (
            Some(FIXTURE_SOURCE_AT),
            "2026-08-19T16:17:00+08:00",
            FIXTURE_BATCH_ID,
            FIXTURE_SOURCE_AT,
            "record_observed_after_batch",
            "evidence.observed_at",
        ),
        (
            Some(FIXTURE_SOURCE_AT),
            "2026-08-19T16:14:00+08:00",
            FIXTURE_BATCH_ID,
            FIXTURE_SOURCE_AT,
            "record_source_after_observation",
            "evidence.source_at",
        ),
        (
            Some("2026-08-19T16:14:00+08:00"),
            FIXTURE_OBSERVED_AT,
            FIXTURE_BATCH_ID,
            "2026-08-19T16:14:00+08:00",
            "batch_source_at_mismatch",
            "source_at",
        ),
    ];
    for (source_at, observed_at, batch_id, published_at, expected_code, expected_field) in cases {
        let mut record = original.records()[0].clone();
        record.published_at = NonEmptyText::new(published_at).unwrap();
        record.evidence =
            SourceEvidence::new(ProviderId::Cailianpress, observed_at, batch_id).unwrap();
        if let Some(source_at) = source_at {
            record.evidence = record.evidence.with_source_at(source_at).unwrap();
        }
        let batch = DataBatch::strict(vec![record], original.provenance().clone());
        let (registry, calls, _) = fixture_news_registry(NewsOutcome::Batch(Box::new(batch)), 4096);
        assert!(
            matches!(
                registry.execute(valid_news_command()),
                Err(ServiceError::InvalidEvidence { provider, evidence_code, evidence_field, record_index, .. })
                    if provider == "Cailianpress" && evidence_code == expected_code
                        && evidence_field == expected_field && record_index == Some(0)
            ),
            "record case {expected_code}/{expected_field}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn registered_news_handler_rejects_a_bad_second_record_without_returning_the_first() {
    let original = fixture_news_batch(ProviderId::Cailianpress);
    let first = original.records()[0].clone();
    for (provider, source_at, expected_code, expected_field) in [
        (
            ProviderId::Jin10,
            FIXTURE_SOURCE_AT,
            "record_provider_mismatch",
            "evidence.provider",
        ),
        (
            ProviderId::Cailianpress,
            "2026-08-19T16:15:30+08:00",
            "record_order_invalid",
            "records",
        ),
    ] {
        let mut second = first.clone();
        second.item_id = NonEmptyText::new("fixture-second-record").unwrap();
        second.published_at = NonEmptyText::new(source_at).unwrap();
        second.evidence = SourceEvidence::new(provider, FIXTURE_OBSERVED_AT, FIXTURE_BATCH_ID)
            .unwrap()
            .with_source_at(source_at)
            .unwrap();
        let batch = DataBatch::strict(vec![first.clone(), second], original.provenance().clone());
        let (registry, calls, _) = fixture_news_registry(NewsOutcome::Batch(Box::new(batch)), 4096);
        assert!(
            matches!(
                registry.execute(valid_news_command()),
                Err(ServiceError::InvalidEvidence { provider, evidence_code, evidence_field, record_index, .. })
                    if provider == "Cailianpress" && evidence_code == expected_code
                        && evidence_field == expected_field && record_index == Some(1)
            ),
            "second record case {expected_code}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn registered_news_handler_accepts_equivalent_instants_without_rewriting_source_evidence() {
    let original = fixture_news_batch(ProviderId::Cailianpress);
    let mut record = original.records()[0].clone();
    record.evidence = SourceEvidence::new(
        ProviderId::Cailianpress,
        FIXTURE_OBSERVED_AT,
        FIXTURE_BATCH_ID,
    )
    .unwrap()
    .with_source_at("2026-08-19T08:15:00Z")
    .unwrap();
    let provenance = Provenance::new("cailianpress", FIXTURE_OBSERVED_AT)
        .unwrap()
        .with_batch_id(FIXTURE_BATCH_ID)
        .unwrap()
        .with_source_at("2026-08-19T08:15:00Z")
        .unwrap();
    let batch = DataBatch::strict(vec![record], provenance);
    let (registry, calls, _) = fixture_news_registry(NewsOutcome::Batch(Box::new(batch)), 4096);
    let result = registry.execute(valid_news_command()).unwrap();
    assert_eq!(result.source_at.as_deref(), Some("2026-08-19T08:15:00Z"));
    let record: serde_json::Value = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(record["evidence"]["source_at"], "2026-08-19T08:15:00Z");
    assert_eq!(record["published_at"], "2026-08-19T16:15:00+08:00");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn provenance_without_a_batch_identity_is_rejected_at_its_public_json_boundary() {
    let generated = Provenance::new("cailianpress", FIXTURE_OBSERVED_AT).unwrap();
    assert!(generated.batch_id().is_some());
    let missing_identity = serde_json::json!({
        "source": "cailianpress",
        "source_at": "2026-08-19T16:15:00+08:00",
        "fetched_at": "2026-08-19T16:16:00+08:00"
    });
    let error = serde_json::from_value::<Provenance>(missing_identity).unwrap_err();
    assert!(error.to_string().contains("missing field `batch_id`"));
}

const OFFLINE_FAILURE_DETAIL: &str = "offline failure fixture";

enum LegacyFailureClass {
    InvalidRequest,
    Unsupported,
    Unavailable,
    PermissionDenied,
    ResourceExhausted,
    FailedPrecondition,
}

fn assert_legacy_provider_failure<E: Error + 'static>(
    operation: Operation,
    error: E,
    expected_class: LegacyFailureClass,
) {
    let source_reason = error.to_string();
    let expected = match expected_class {
        LegacyFailureClass::InvalidRequest => {
            ServiceError::InvalidRequest(OFFLINE_FAILURE_DETAIL.into())
        }
        LegacyFailureClass::Unsupported => ServiceError::Unsupported {
            operation,
            reason: OFFLINE_FAILURE_DETAIL.into(),
        },
        LegacyFailureClass::Unavailable => ServiceError::Unavailable {
            operation,
            reason: source_reason,
        },
        LegacyFailureClass::PermissionDenied => ServiceError::PermissionDenied(source_reason),
        LegacyFailureClass::ResourceExhausted => ServiceError::ResourceExhausted(source_reason),
        LegacyFailureClass::FailedPrecondition => ServiceError::FailedPrecondition(source_reason),
    };
    assert_eq!(provider_error(operation, error), expected);
}

macro_rules! legacy_provider_failure_cases {
    ($name:ident, $error:ident, $operation:expr, $transport:expr) => {
        #[test]
        fn $name() {
            let operation = $operation;
            assert_legacy_provider_failure(
                operation,
                $error::InvalidRequest(OFFLINE_FAILURE_DETAIL.into()),
                LegacyFailureClass::InvalidRequest,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Unsupported(OFFLINE_FAILURE_DETAIL.into()),
                LegacyFailureClass::Unsupported,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Transport($transport),
                LegacyFailureClass::Unavailable,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Decode(OFFLINE_FAILURE_DETAIL.into()),
                LegacyFailureClass::FailedPrecondition,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Protocol(OFFLINE_FAILURE_DETAIL.into()),
                LegacyFailureClass::FailedPrecondition,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Core(magic_market_core::CoreError::InvalidRequest(
                    OFFLINE_FAILURE_DETAIL.into(),
                )),
                LegacyFailureClass::FailedPrecondition,
            );
        }
    };
}

fn offline_transport_error() -> magic_market_transport::TransportError {
    magic_market_transport::TransportError::Network(OFFLINE_FAILURE_DETAIL.into())
}

legacy_provider_failure_cases!(
    baidu_failures_keep_service_categories,
    BaiduError,
    Operation::GlobalNews,
    OFFLINE_FAILURE_DETAIL.into()
);
legacy_provider_failure_cases!(
    cfets_failures_keep_service_categories,
    CfetsError,
    Operation::ReferenceRates,
    offline_transport_error()
);
legacy_provider_failure_cases!(
    nbs_failures_keep_service_categories,
    NbsError,
    Operation::EconomicSeries,
    offline_transport_error()
);
legacy_provider_failure_cases!(
    pbc_failures_keep_service_categories,
    PbcError,
    Operation::OfficialFxFixings,
    offline_transport_error()
);
legacy_provider_failure_cases!(
    stcn_failures_keep_service_categories,
    StcnError,
    Operation::GlobalNews,
    offline_transport_error()
);
legacy_provider_failure_cases!(
    thepaper_failures_keep_service_categories,
    ThePaperError,
    Operation::GlobalNews,
    OFFLINE_FAILURE_DETAIL.into()
);
legacy_provider_failure_cases!(
    wallstreetcn_failures_keep_service_categories,
    WallstreetCnError,
    Operation::GlobalNews,
    OFFLINE_FAILURE_DETAIL.into()
);
legacy_provider_failure_cases!(
    xinhua_failures_keep_service_categories,
    XinhuaError,
    Operation::GlobalNews,
    offline_transport_error()
);
legacy_provider_failure_cases!(
    yicai_failures_keep_service_categories,
    YicaiError,
    Operation::GlobalNews,
    offline_transport_error()
);
legacy_provider_failure_cases!(
    fred_failures_keep_service_categories,
    FredError,
    Operation::EconomicSeries,
    offline_transport_error()
);
legacy_provider_failure_cases!(
    sec_failures_keep_service_categories,
    SecEdgarError,
    Operation::CompanyFilings,
    offline_transport_error()
);
legacy_provider_failure_cases!(
    iwencai_failures_keep_service_categories,
    IwencaiError,
    Operation::SemanticSearch,
    OFFLINE_FAILURE_DETAIL.into()
);
legacy_provider_failure_cases!(
    worldbank_failures_keep_service_categories,
    WorldBankError,
    Operation::EconomicSeries,
    offline_transport_error()
);

// These enums have Schema/Incomplete instead of Protocol; keep their cases explicit.
macro_rules! paginated_provider_failure_cases {
    ($name:ident, $error:ident, $operation:expr) => {
        #[test]
        fn $name() {
            let operation = $operation;
            assert_legacy_provider_failure(
                operation,
                $error::InvalidRequest(OFFLINE_FAILURE_DETAIL.into()),
                LegacyFailureClass::InvalidRequest,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Unsupported(OFFLINE_FAILURE_DETAIL.into()),
                LegacyFailureClass::Unsupported,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Authentication(403),
                LegacyFailureClass::PermissionDenied,
            );
            assert_legacy_provider_failure(
                operation,
                $error::RateLimited,
                LegacyFailureClass::ResourceExhausted,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Transport(OFFLINE_FAILURE_DETAIL.into()),
                LegacyFailureClass::Unavailable,
            );
            assert_legacy_provider_failure(
                operation,
                $error::HttpStatus(499),
                LegacyFailureClass::FailedPrecondition,
            );
            assert_legacy_provider_failure(
                operation,
                $error::HttpStatus(500),
                LegacyFailureClass::Unavailable,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Decode(OFFLINE_FAILURE_DETAIL.into()),
                LegacyFailureClass::FailedPrecondition,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Schema(OFFLINE_FAILURE_DETAIL.into()),
                LegacyFailureClass::FailedPrecondition,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Incomplete(OFFLINE_FAILURE_DETAIL.into()),
                LegacyFailureClass::FailedPrecondition,
            );
            assert_legacy_provider_failure(
                operation,
                $error::Core(magic_market_core::CoreError::InvalidRequest(
                    OFFLINE_FAILURE_DETAIL.into(),
                )),
                LegacyFailureClass::FailedPrecondition,
            );
        }
    };
}

paginated_provider_failure_cases!(
    cninfo_failures_keep_service_categories,
    CninfoError,
    Operation::Announcements
);
paginated_provider_failure_cases!(
    exchange_failures_keep_service_categories,
    ExchangeError,
    Operation::MarketAnnouncements
);
paginated_provider_failure_cases!(
    ths_non_consensus_failures_keep_service_categories,
    ThsError,
    Operation::Popularity
);

#[test]
fn authenticated_providers_keep_their_distinct_authentication_contracts() {
    assert_legacy_provider_failure(
        Operation::EconomicSeries,
        FredError::Authentication(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::PermissionDenied,
    );
    assert_legacy_provider_failure(
        Operation::CompanyFilings,
        SecEdgarError::Authentication(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::PermissionDenied,
    );
    assert_legacy_provider_failure(
        Operation::SemanticSearch,
        IwencaiError::Authentication(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::PermissionDenied,
    );
    assert_legacy_provider_failure(
        Operation::EconomicSeries,
        WorldBankError::Authentication(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::Unavailable,
    );
    assert_legacy_provider_failure(
        Operation::MarketAnnouncements,
        ExchangeError::Tls {
            backend: magic_exchange_rs::TlsBackend::Rustls,
            message: OFFLINE_FAILURE_DETAIL.into(),
        },
        LegacyFailureClass::Unavailable,
    );
}

#[test]
fn eastmoney_and_jin10_error_results_are_not_successful_empty_data() {
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        EastmoneyError::InvalidRequest(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::InvalidRequest,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        EastmoneyError::Unsupported(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::Unsupported,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        EastmoneyError::Authentication(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::PermissionDenied,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        EastmoneyError::Transport(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::Unavailable,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        EastmoneyError::ResponseTooLarge { limit: 17 },
        LegacyFailureClass::FailedPrecondition,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        EastmoneyError::Decode(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::FailedPrecondition,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        EastmoneyError::Protocol(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::FailedPrecondition,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        EastmoneyError::Core(magic_market_core::CoreError::InvalidRequest(
            OFFLINE_FAILURE_DETAIL.into(),
        )),
        LegacyFailureClass::FailedPrecondition,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        Jin10Error::InvalidRequest(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::InvalidRequest,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        Jin10Error::Unsupported(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::Unsupported,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        Jin10Error::Transport(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::Unavailable,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        Jin10Error::Decode(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::FailedPrecondition,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        Jin10Error::Protocol(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::FailedPrecondition,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        Jin10Error::Core(magic_market_core::CoreError::InvalidRequest(
            OFFLINE_FAILURE_DETAIL.into(),
        )),
        LegacyFailureClass::FailedPrecondition,
    );
    for provider in [
        ProviderId::Eastmoney,
        ProviderId::Jin10,
        ProviderId::Tonghuashun,
    ] {
        let evidence = SourceEvidence::new(provider, FIXTURE_OBSERVED_AT, FIXTURE_BATCH_ID)
            .unwrap()
            .with_source_at(FIXTURE_SOURCE_AT)
            .unwrap();
        let provenance = Provenance::new("offline fixture", FIXTURE_OBSERVED_AT)
            .unwrap()
            .with_batch_id(FIXTURE_BATCH_ID)
            .unwrap()
            .with_source_at(FIXTURE_SOURCE_AT)
            .unwrap();
        let empty = magic_market_core::VerifiedEmpty::new(
            "offline-test",
            "fixture-only",
            "fixture empty",
            evidence,
            provenance,
        )
        .unwrap();
        match provider {
            ProviderId::Eastmoney => assert_legacy_provider_failure(
                Operation::GlobalNews,
                EastmoneyError::VerifiedEmpty(Box::new(empty)),
                LegacyFailureClass::FailedPrecondition,
            ),
            ProviderId::Jin10 => assert_legacy_provider_failure(
                Operation::GlobalNews,
                Jin10Error::VerifiedEmpty(Box::new(empty)),
                LegacyFailureClass::FailedPrecondition,
            ),
            ProviderId::Tonghuashun => assert_legacy_provider_failure(
                Operation::Popularity,
                ThsError::VerifiedEmpty(Box::new(empty)),
                LegacyFailureClass::FailedPrecondition,
            ),
            _ => unreachable!(),
        }
    }
    assert_legacy_provider_failure(
        Operation::Popularity,
        ThsError::ProbeAdmission(magic_market_core::ProbeAdmissionError::EmptyBatch),
        LegacyFailureClass::FailedPrecondition,
    );
}

#[test]
fn gov_and_emquant_failures_keep_request_and_response_boundaries() {
    assert_legacy_provider_failure(
        Operation::PolicyDocuments,
        GovError::InvalidRequest(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::InvalidRequest,
    );
    assert_legacy_provider_failure(
        Operation::PolicyDocuments,
        GovError::Transport(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::Unavailable,
    );
    assert_legacy_provider_failure(
        Operation::PolicyDocuments,
        GovError::Decode(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::FailedPrecondition,
    );
    assert_legacy_provider_failure(
        Operation::PolicyDocuments,
        GovError::Protocol(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::FailedPrecondition,
    );
    assert_legacy_provider_failure(
        Operation::PolicyDocuments,
        GovError::Core(magic_market_core::CoreError::InvalidRequest(
            OFFLINE_FAILURE_DETAIL.into(),
        )),
        LegacyFailureClass::FailedPrecondition,
    );
    assert_legacy_provider_failure(
        Operation::HistoricalBars,
        EmQuantError::InvalidRequest(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::InvalidRequest,
    );
    assert_legacy_provider_failure(
        Operation::HistoricalBars,
        EmQuantError::Unsupported(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::Unsupported,
    );
    assert_legacy_provider_failure(
        Operation::HistoricalBars,
        EmQuantError::Bridge(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::Unavailable,
    );
    assert_legacy_provider_failure(
        Operation::HistoricalBars,
        EmQuantError::InvalidResponse(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::FailedPrecondition,
    );
    assert_legacy_provider_failure(
        Operation::HistoricalBars,
        EmQuantError::Core(magic_market_core::CoreError::InvalidRequest(
            OFFLINE_FAILURE_DETAIL.into(),
        )),
        LegacyFailureClass::FailedPrecondition,
    );
}

#[test]
fn sina_failures_preserve_raw_reasons_instead_of_other_provider_prefixes() {
    let operation = Operation::GlobalIndices;
    for (error, expected) in [
        (
            SinaError::InvalidRequest(OFFLINE_FAILURE_DETAIL.into()),
            ServiceError::InvalidRequest(OFFLINE_FAILURE_DETAIL.into()),
        ),
        (
            SinaError::Unsupported(OFFLINE_FAILURE_DETAIL.into()),
            ServiceError::Unsupported {
                operation,
                reason: OFFLINE_FAILURE_DETAIL.into(),
            },
        ),
        (
            SinaError::Transport(OFFLINE_FAILURE_DETAIL.into()),
            ServiceError::Unavailable {
                operation,
                reason: OFFLINE_FAILURE_DETAIL.into(),
            },
        ),
        (
            SinaError::Decode(OFFLINE_FAILURE_DETAIL.into()),
            ServiceError::FailedPrecondition(OFFLINE_FAILURE_DETAIL.into()),
        ),
        (
            SinaError::Protocol(OFFLINE_FAILURE_DETAIL.into()),
            ServiceError::FailedPrecondition(OFFLINE_FAILURE_DETAIL.into()),
        ),
        (
            SinaError::Core(magic_market_core::CoreError::InvalidRequest(
                OFFLINE_FAILURE_DETAIL.into(),
            )),
            ServiceError::FailedPrecondition("invalid request: offline failure fixture".into()),
        ),
    ] {
        assert_eq!(provider_error(operation, error), expected);
    }
}

struct StructuredFailureCase<E> {
    error: E,
    kind: ProviderFailureKind,
    reason: &'static str,
}

fn assert_structured_provider_failures<E: Error + 'static>(
    operation: Operation,
    provider: &str,
    cases: Vec<StructuredFailureCase<E>>,
) {
    for case in cases {
        assert_eq!(
            provider_error(operation, case.error),
            ServiceError::ProviderFailure {
                operation,
                provider: provider.into(),
                kind: case.kind,
                provider_reason: case.reason.into(),
            }
        );
    }
}

macro_rules! structured_status_cases {
    ($error:ident) => {
        vec![
            StructuredFailureCase {
                error: $error::HttpStatus(401),
                kind: ProviderFailureKind::AuthenticationRejected,
                reason: "http_status=401",
            },
            StructuredFailureCase {
                error: $error::HttpStatus(403),
                kind: ProviderFailureKind::AuthenticationRejected,
                reason: "http_status=403",
            },
            StructuredFailureCase {
                error: $error::HttpStatus(429),
                kind: ProviderFailureKind::RateLimited,
                reason: "http_status=429",
            },
            StructuredFailureCase {
                error: $error::HttpStatus(499),
                kind: ProviderFailureKind::QueryRejected,
                reason: "http_status=499",
            },
            StructuredFailureCase {
                error: $error::HttpStatus(500),
                kind: ProviderFailureKind::Unavailable,
                reason: "http_status=500",
            },
            StructuredFailureCase {
                error: $error::HttpStatus(599),
                kind: ProviderFailureKind::Unavailable,
                reason: "http_status=599",
            },
            StructuredFailureCase {
                error: $error::HttpStatus(600),
                kind: ProviderFailureKind::QueryRejected,
                reason: "http_status=600",
            },
        ]
    };
}

#[test]
fn cls_typed_failures_bind_operation_provider_kind_and_source_reason() {
    assert_structured_provider_failures(
        Operation::GlobalNews,
        "Cailianpress",
        structured_status_cases!(ClsError),
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        ClsError::InvalidRequest(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::InvalidRequest,
    );
    assert_legacy_provider_failure(
        Operation::GlobalNews,
        ClsError::Unsupported(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::Unsupported,
    );
    assert_structured_provider_failures(
        Operation::GlobalNews,
        "Cailianpress",
        vec![
            StructuredFailureCase {
                error: ClsError::Transport(OFFLINE_FAILURE_DETAIL.into()),
                kind: ProviderFailureKind::Unavailable,
                reason: "category=transport",
            },
            StructuredFailureCase {
                error: ClsError::ProviderRejected {
                    errno: 1001,
                    message: OFFLINE_FAILURE_DETAIL.into(),
                },
                kind: ProviderFailureKind::QueryRejected,
                reason: "errno=1001 message=offline failure fixture",
            },
            StructuredFailureCase {
                error: ClsError::Decode(OFFLINE_FAILURE_DETAIL.into()),
                kind: ProviderFailureKind::ResponseInvalid,
                reason: "category=decode message=offline failure fixture",
            },
            StructuredFailureCase {
                error: ClsError::Protocol(OFFLINE_FAILURE_DETAIL.into()),
                kind: ProviderFailureKind::ResponseInvalid,
                reason: "category=protocol message=offline failure fixture",
            },
            StructuredFailureCase {
                error: ClsError::Core(magic_market_core::CoreError::InvalidRequest(
                    OFFLINE_FAILURE_DETAIL.into(),
                )),
                kind: ProviderFailureKind::ResponseInvalid,
                reason: "category=core message=invalid request: offline failure fixture",
            },
        ],
    );
}

#[test]
fn hithink_typed_failures_preserve_codes_and_redact_unstructured_details() {
    let operation = Operation::FinancialStatements;
    assert_structured_provider_failures(
        operation,
        "HithinkFinance",
        structured_status_cases!(HithinkError),
    );
    assert_legacy_provider_failure(
        operation,
        HithinkError::InvalidRequest(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::InvalidRequest,
    );
    assert_legacy_provider_failure(
        operation,
        HithinkError::Unsupported(OFFLINE_FAILURE_DETAIL.into()),
        LegacyFailureClass::Unsupported,
    );
    assert_structured_provider_failures(
        operation,
        "HithinkFinance",
        vec![
            StructuredFailureCase {
                error: HithinkError::Authentication {
                    code: 2003,
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::AuthenticationRejected,
                reason: "code=2003 request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::RateLimited {
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::RateLimited,
                reason: "code=4001 request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::Business {
                    code: 1001,
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::QueryRejected,
                reason: "code=1001 request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::Business {
                    code: 1004,
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::QueryRejected,
                reason: "code=1004 request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::Business {
                    code: 3001,
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::QueryRejected,
                reason: "code=3001 request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::Business {
                    code: 3004,
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::QueryRejected,
                reason: "code=3004 request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::Business {
                    code: 3002,
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::Unavailable,
                reason: "code=3002 request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::Business {
                    code: 5001,
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::Unavailable,
                reason: "code=5001 request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::Business {
                    code: 5003,
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::Unavailable,
                reason: "code=5003 request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::Business {
                    code: 9999,
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::ResponseInvalid,
                reason: "code=9999 request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::NotReady {
                    request_id: "offline-request".into(),
                },
                kind: ProviderFailureKind::Unavailable,
                reason: "category=not_ready request_id=offline-request",
            },
            StructuredFailureCase {
                error: HithinkError::Transport(offline_transport_error()),
                kind: ProviderFailureKind::Unavailable,
                reason: "category=transport",
            },
            StructuredFailureCase {
                error: HithinkError::Decode(OFFLINE_FAILURE_DETAIL.into()),
                kind: ProviderFailureKind::ResponseInvalid,
                reason: "category=decode",
            },
            StructuredFailureCase {
                error: HithinkError::Protocol(OFFLINE_FAILURE_DETAIL.into()),
                kind: ProviderFailureKind::ResponseInvalid,
                reason: "category=protocol",
            },
            StructuredFailureCase {
                error: HithinkError::Core(magic_market_core::CoreError::InvalidRequest(
                    OFFLINE_FAILURE_DETAIL.into(),
                )),
                kind: ProviderFailureKind::ResponseInvalid,
                reason: "category=protocol",
            },
        ],
    );
}

#[test]
fn consensus_typed_response_failures_keep_safe_field_identity() {
    struct ConsensusCase {
        message: &'static str,
        code: &'static str,
        field: &'static str,
    }
    let cases = [
        ConsensusCase {
            message: "instrument identity",
            code: "consensus_instrument_identity_invalid",
            field: "consensus.instrument_identity",
        },
        ConsensusCase {
            message: "fiscal year",
            code: "consensus_fiscal_year_invalid",
            field: "consensus.estimates.fiscal_year",
        },
        ConsensusCase {
            message: "institution count",
            code: "consensus_contributor_count_invalid",
            field: "consensus.estimates.contributor_count",
        },
        ConsensusCase {
            message: "minimum",
            code: "consensus_minimum_invalid",
            field: "consensus.estimates.minimum",
        },
        ConsensusCase {
            message: "maximum",
            code: "consensus_maximum_invalid",
            field: "consensus.estimates.maximum",
        },
        ConsensusCase {
            message: "mean",
            code: "consensus_mean_invalid",
            field: "consensus.estimates.mean",
        },
        ConsensusCase {
            message: "estimate values",
            code: "consensus_values_missing",
            field: "consensus.estimates.values",
        },
        ConsensusCase {
            message: "caption",
            code: "consensus_table_invalid",
            field: "consensus.estimates.table",
        },
        ConsensusCase {
            message: "unclassified fixture detail",
            code: "consensus_provider_response_invalid",
            field: "consensus.provider_response",
        },
    ];
    for case in cases {
        for error in [
            ThsError::Schema(case.message.into()),
            ThsError::Incomplete(case.message.into()),
            ThsError::Core(magic_market_core::CoreError::InvalidRequest(
                case.message.into(),
            )),
        ] {
            assert_eq!(
                provider_error(Operation::Consensus, error),
                ServiceError::InvalidEvidence {
                    provider: "Tonghuashun".into(),
                    evidence_code: case.code.into(),
                    evidence_field: case.field.into(),
                    record_index: None,
                    message: format!(
                        "Consensus rejected Tonghuashun evidence ({} at {})",
                        case.code, case.field
                    ),
                }
            );
        }
    }
}

#[test]
fn unknown_provider_errors_fail_conservatively_with_the_requested_operation() {
    let error = std::io::Error::other(OFFLINE_FAILURE_DETAIL);
    assert_eq!(
        provider_error(Operation::GlobalNews, error),
        ServiceError::FailedPrecondition(format!(
            "{} provider request failed: {OFFLINE_FAILURE_DETAIL}",
            Operation::GlobalNews.as_str()
        ))
    );
}

const FINANCIAL_FIXTURE_PROVIDER: &str = "offline-financial-fixture";

#[derive(Debug, Clone, PartialEq)]
struct FinancialFixtureCall {
    instruments: Vec<InstrumentId>,
    kind: StatementKind,
}

enum FinancialFixtureOutcome {
    Batch(Box<DataBatch<FinancialStatement>>),
    HttpStatus(u16),
    Transport,
    Decode,
}

struct FixtureFinancialProvider {
    outcome: FinancialFixtureOutcome,
    calls: std::sync::Mutex<Vec<FinancialFixtureCall>>,
}

impl FinancialStatements for FixtureFinancialProvider {
    type Error = HithinkError;

    fn financial_statements(
        &self,
        instruments: &[InstrumentId],
        kind: StatementKind,
    ) -> Result<DataBatch<FinancialStatement>, Self::Error> {
        self.calls.lock().unwrap().push(FinancialFixtureCall {
            instruments: instruments.to_vec(),
            kind,
        });
        match &self.outcome {
            FinancialFixtureOutcome::Batch(batch) => Ok((**batch).clone()),
            FinancialFixtureOutcome::HttpStatus(status) => Err(HithinkError::HttpStatus(*status)),
            FinancialFixtureOutcome::Transport => {
                Err(HithinkError::Transport(offline_transport_error()))
            }
            FinancialFixtureOutcome::Decode => {
                Err(HithinkError::Decode(OFFLINE_FAILURE_DETAIL.into()))
            }
        }
    }
}

fn financial_fixture_provider(outcome: FinancialFixtureOutcome) -> FixtureFinancialProvider {
    FixtureFinancialProvider {
        outcome,
        calls: std::sync::Mutex::new(Vec::new()),
    }
}

fn financial_fixture_instruments() -> Vec<InstrumentId> {
    use magic_market_core::{AssetClass, Exchange};

    vec![
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap(),
    ]
}

fn financial_fixture_batch(kind: StatementKind) -> DataBatch<FinancialStatement> {
    let records = financial_fixture_instruments()
        .into_iter()
        .zip([
            (
                "2026-06-30",
                "H1",
                "2026-07-31",
                "2026-07-31T09:00:00+08:00",
            ),
            (
                "2025-12-31",
                "FY",
                "2026-04-30",
                "2026-04-30T09:00:00+08:00",
            ),
        ])
        .map(
            |(instrument, (report_period, fiscal_period, announced_on, source_at))| {
                FinancialStatement {
                    instrument,
                    kind,
                    report_period: IsoDate::new(report_period).unwrap(),
                    fiscal_period: Some(NonEmptyText::new(fiscal_period).unwrap()),
                    announced_on: Some(IsoDate::new(announced_on).unwrap()),
                    currency: Some(NonEmptyText::new("CNY").unwrap()),
                    lines: vec![
                        FinancialLine {
                            key: NonEmptyText::new("source_amount").unwrap(),
                            source_label: NonEmptyText::new("原始金额标签").unwrap(),
                            value: Some(magic_market_core::FiniteNumber::new(1_200.5).unwrap()),
                            unit: Some(NonEmptyText::new("source-unit").unwrap()),
                        },
                        FinancialLine {
                            key: NonEmptyText::new("source_missing").unwrap(),
                            source_label: NonEmptyText::new("原始缺项").unwrap(),
                            value: None,
                            unit: None,
                        },
                    ],
                    evidence: SourceEvidence::new(
                        ProviderId::Tonghuashun,
                        FIXTURE_OBSERVED_AT,
                        FINANCIAL_FIXTURE_PROVIDER,
                    )
                    .unwrap()
                    .with_source_at(source_at)
                    .unwrap(),
                }
            },
        )
        .collect();
    DataBatch::strict(
        records,
        Provenance::new(FINANCIAL_FIXTURE_PROVIDER, FIXTURE_OBSERVED_AT)
            .unwrap()
            .with_source_at("2026-07-31T09:00:00+08:00")
            .unwrap()
            .with_batch_id(FINANCIAL_FIXTURE_PROVIDER)
            .unwrap(),
    )
}

fn financial_fixture_command(version: u32, kind: StatementKind) -> QueryCommand {
    let data = serde_json::to_vec(&serde_json::json!({
        "instruments": financial_fixture_instruments(),
        "kind": kind,
    }))
    .unwrap();
    fixture_command(
        Operation::FinancialStatements,
        FINANCIAL_FIXTURE_PROVIDER,
        FINANCIAL_STATEMENTS_REQUEST_SCHEMA,
        version,
        &data,
    )
}

fn assert_financial_fixture_call(provider: &FixtureFinancialProvider, kind: StatementKind) {
    assert_eq!(
        provider.calls.lock().unwrap().as_slice(),
        &[FinancialFixtureCall {
            instruments: financial_fixture_instruments(),
            kind,
        }]
    );
}

fn assert_financial_handler_forwarding(version: u32, kind: StatementKind) {
    let batch = financial_fixture_batch(kind);
    let provider =
        financial_fixture_provider(FinancialFixtureOutcome::Batch(Box::new(batch.clone())));
    let result = execute_financial_statements(
        financial_fixture_command(version, kind),
        &provider,
        FINANCIAL_FIXTURE_PROVIDER,
        4096,
    )
    .unwrap();

    assert_financial_fixture_call(&provider, kind);
    assert_eq!(result.provider, FINANCIAL_FIXTURE_PROVIDER);
    assert_eq!(result.batch_id, FINANCIAL_FIXTURE_PROVIDER);
    assert_eq!(result.observed_at, FIXTURE_OBSERVED_AT);
    assert_eq!(
        result.source_at.as_deref(),
        Some("2026-07-31T09:00:00+08:00")
    );
    assert!(result.complete);
    assert_eq!(result.records.len(), 2);
    for (payload, statement) in result.records.iter().zip(batch.records()) {
        assert_eq!(payload.schema(), FINANCIAL_STATEMENTS_RECORD_SCHEMA);
        assert_eq!(payload.schema_version(), version);
        let value: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(
            value["instrument"],
            serde_json::to_value(&statement.instrument).unwrap()
        );
        assert_eq!(value["kind"], serde_json::to_value(kind).unwrap());
        assert_eq!(value["report_period"], statement.report_period.as_str());
        assert_eq!(
            value["announced_on"],
            statement.announced_on.as_ref().unwrap().as_str()
        );
        assert_eq!(value["currency"], "CNY");
        assert_eq!(value["lines"][0]["key"], "source_amount");
        assert_eq!(value["lines"][0]["source_label"], "原始金额标签");
        assert_eq!(value["lines"][0]["value"], 1_200.5);
        assert_eq!(value["lines"][0]["unit"], "source-unit");
        assert_eq!(value["lines"][1]["key"], "source_missing");
        assert_eq!(value["lines"][1]["source_label"], "原始缺项");
        assert!(value["lines"][1]["value"].is_null());
        assert!(value["lines"][1]["unit"].is_null());
        assert_eq!(
            value["evidence"],
            serde_json::to_value(&statement.evidence).unwrap()
        );
        if version == 1 {
            assert!(value.get("fiscal_period").is_none());
        } else {
            assert_eq!(
                value["fiscal_period"],
                statement.fiscal_period.as_ref().unwrap().as_str()
            );
        }
    }
}

macro_rules! financial_handler_forwarding_test {
    ($name:ident, $version:literal, $kind:ident) => {
        #[test]
        fn $name() {
            assert_financial_handler_forwarding($version, StatementKind::$kind);
        }
    };
}

financial_handler_forwarding_test!(
    financial_v1_forwards_balance_and_omits_fiscal_period,
    1,
    Balance
);
financial_handler_forwarding_test!(
    financial_v1_forwards_income_and_omits_fiscal_period,
    1,
    Income
);
financial_handler_forwarding_test!(
    financial_v1_forwards_cash_flow_and_omits_fiscal_period,
    1,
    CashFlow
);
financial_handler_forwarding_test!(
    financial_v2_forwards_balance_and_preserves_h1_fy,
    2,
    Balance
);
financial_handler_forwarding_test!(financial_v2_forwards_income_and_preserves_h1_fy, 2, Income);
financial_handler_forwarding_test!(
    financial_v2_forwards_cash_flow_and_preserves_h1_fy,
    2,
    CashFlow
);

#[test]
fn financial_v2_does_not_infer_absent_labels_dates_currency_or_values() {
    let original = financial_fixture_batch(StatementKind::Income);
    let records = original
        .records()
        .iter()
        .cloned()
        .map(|mut statement| {
            statement.fiscal_period = None;
            statement.announced_on = None;
            statement.currency = None;
            statement.lines[0].value = None;
            statement
        })
        .collect();
    let provenance = Provenance::new(FINANCIAL_FIXTURE_PROVIDER, FIXTURE_OBSERVED_AT)
        .unwrap()
        .with_batch_id(FINANCIAL_FIXTURE_PROVIDER)
        .unwrap();
    let provider = financial_fixture_provider(FinancialFixtureOutcome::Batch(Box::new(
        DataBatch::strict(records, provenance),
    )));
    let result = execute_financial_statements(
        financial_fixture_command(2, StatementKind::Income),
        &provider,
        FINANCIAL_FIXTURE_PROVIDER,
        4096,
    )
    .unwrap();
    assert_financial_fixture_call(&provider, StatementKind::Income);
    assert!(result.source_at.is_none());
    assert_eq!(result.records.len(), 2);
    for (payload, statement) in result.records.iter().zip(original.records()) {
        let value: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(value["report_period"], statement.report_period.as_str());
        assert!(value.get("fiscal_period").unwrap().is_null());
        assert!(value.get("announced_on").unwrap().is_null());
        assert!(value.get("currency").unwrap().is_null());
        assert!(value["lines"][0].get("value").unwrap().is_null());
        assert_eq!(
            value["evidence"],
            serde_json::to_value(&statement.evidence).unwrap()
        );
    }
}

#[test]
fn financial_invalid_envelopes_and_shapes_never_call_the_provider() {
    struct InvalidFinancialCase {
        name: &'static str,
        schema: &'static str,
        version: u32,
        body: &'static [u8],
        reason_fragment: &'static str,
    }

    let cases = [
        InvalidFinancialCase {
            name: "unsupported version",
            schema: FINANCIAL_STATEMENTS_REQUEST_SCHEMA,
            version: 3,
            body: br#"{}"#,
            reason_fragment: "version 1 or 2",
        },
        InvalidFinancialCase {
            name: "wrong v1 schema",
            schema: "magic.market.wrong.request",
            version: 1,
            body: br#"{}"#,
            reason_fragment: FINANCIAL_STATEMENTS_REQUEST_SCHEMA,
        },
        InvalidFinancialCase {
            name: "wrong v2 schema",
            schema: "magic.market.wrong.request",
            version: 2,
            body: br#"{}"#,
            reason_fragment: FINANCIAL_STATEMENTS_REQUEST_SCHEMA,
        },
        InvalidFinancialCase {
            name: "missing instruments",
            schema: FINANCIAL_STATEMENTS_REQUEST_SCHEMA,
            version: 1,
            body: br#"{"kind":"Income"}"#,
            reason_fragment: "missing field `instruments`",
        },
        InvalidFinancialCase {
            name: "missing kind",
            schema: FINANCIAL_STATEMENTS_REQUEST_SCHEMA,
            version: 2,
            body: br#"{"instruments":[]}"#,
            reason_fragment: "missing field `kind`",
        },
        InvalidFinancialCase {
            name: "wrong instruments type",
            schema: FINANCIAL_STATEMENTS_REQUEST_SCHEMA,
            version: 1,
            body: br#"{"instruments":"600519","kind":"Income"}"#,
            reason_fragment: "invalid type",
        },
        InvalidFinancialCase {
            name: "unknown statement kind",
            schema: FINANCIAL_STATEMENTS_REQUEST_SCHEMA,
            version: 2,
            body: br#"{"instruments":[],"kind":"Annual"}"#,
            reason_fragment: "unknown variant",
        },
        InvalidFinancialCase {
            name: "unknown field",
            schema: FINANCIAL_STATEMENTS_REQUEST_SCHEMA,
            version: 2,
            body: br#"{"instruments":[],"kind":"Income","currency":"CNY"}"#,
            reason_fragment: "unknown field `currency`",
        },
        InvalidFinancialCase {
            name: "null body",
            schema: FINANCIAL_STATEMENTS_REQUEST_SCHEMA,
            version: 1,
            body: br#"null"#,
            reason_fragment: "invalid type",
        },
    ];
    for case in cases {
        let provider = financial_fixture_provider(FinancialFixtureOutcome::Transport);
        let error = execute_financial_statements(
            fixture_command(
                Operation::FinancialStatements,
                FINANCIAL_FIXTURE_PROVIDER,
                case.schema,
                case.version,
                case.body,
            ),
            &provider,
            FINANCIAL_FIXTURE_PROVIDER,
            4096,
        )
        .unwrap_err();
        assert!(
            matches!(error, ServiceError::InvalidRequest(ref reason) if reason.contains(case.reason_fragment)),
            "{}: {error:?}",
            case.name
        );
        assert!(
            provider.calls.lock().unwrap().is_empty(),
            "{} invoked the provider",
            case.name
        );
    }
}

#[test]
fn financial_provider_failures_remain_typed_and_cannot_become_empty_results() {
    struct FinancialFailureCase {
        name: &'static str,
        outcome: FinancialFixtureOutcome,
        kind: ProviderFailureKind,
        reason: &'static str,
    }

    let cases = [
        FinancialFailureCase {
            name: "authentication",
            outcome: FinancialFixtureOutcome::HttpStatus(401),
            kind: ProviderFailureKind::AuthenticationRejected,
            reason: "http_status=401",
        },
        FinancialFailureCase {
            name: "rate limit",
            outcome: FinancialFixtureOutcome::HttpStatus(429),
            kind: ProviderFailureKind::RateLimited,
            reason: "http_status=429",
        },
        FinancialFailureCase {
            name: "server failure",
            outcome: FinancialFixtureOutcome::HttpStatus(500),
            kind: ProviderFailureKind::Unavailable,
            reason: "http_status=500",
        },
        FinancialFailureCase {
            name: "redacted transport",
            outcome: FinancialFixtureOutcome::Transport,
            kind: ProviderFailureKind::Unavailable,
            reason: "category=transport",
        },
        FinancialFailureCase {
            name: "redacted response",
            outcome: FinancialFixtureOutcome::Decode,
            kind: ProviderFailureKind::ResponseInvalid,
            reason: "category=decode",
        },
    ];
    for case in cases {
        let provider = financial_fixture_provider(case.outcome);
        let error = execute_financial_statements(
            financial_fixture_command(2, StatementKind::Income),
            &provider,
            FINANCIAL_FIXTURE_PROVIDER,
            4096,
        )
        .unwrap_err();
        assert_financial_fixture_call(&provider, StatementKind::Income);
        assert_eq!(
            error,
            ServiceError::ProviderFailure {
                operation: Operation::FinancialStatements,
                provider: "HithinkFinance".into(),
                kind: case.kind,
                provider_reason: case.reason.into()
            },
            "{}",
            case.name
        );
    }
}

#[test]
fn financial_second_oversized_record_rejects_the_complete_result() {
    let original = financial_fixture_batch(StatementKind::Balance);
    let mut records = original.records().to_vec();
    records[1].lines[0].source_label =
        NonEmptyText::new("oversized-offline-label".repeat(128)).unwrap();
    let limit = 2048;
    assert!(serde_json::to_vec(&records[0]).unwrap().len() < limit);
    assert!(serde_json::to_vec(&records[1]).unwrap().len() > limit);
    let provider = financial_fixture_provider(FinancialFixtureOutcome::Batch(Box::new(
        DataBatch::strict(records, original.provenance().clone()),
    )));
    let error = execute_financial_statements(
        financial_fixture_command(2, StatementKind::Balance),
        &provider,
        FINANCIAL_FIXTURE_PROVIDER,
        limit,
    )
    .unwrap_err();
    assert_financial_fixture_call(&provider, StatementKind::Balance);
    assert!(
        matches!(error, ServiceError::ResourceExhausted(ref reason) if reason.contains("exceeds maximum 2048")),
        "{error:?}"
    );
}

#[test]
fn financial_projection_does_not_upgrade_partial_batch_quality() {
    let original = financial_fixture_batch(StatementKind::CashFlow);
    let partial = DataBatch::best_effort(
        original.records().to_vec(),
        original.provenance().clone(),
        vec!["offline page missing".into()],
    )
    .unwrap();
    let provider = financial_fixture_provider(FinancialFixtureOutcome::Batch(Box::new(partial)));
    let result = execute_financial_statements(
        financial_fixture_command(2, StatementKind::CashFlow),
        &provider,
        FINANCIAL_FIXTURE_PROVIDER,
        4096,
    )
    .unwrap();
    assert_financial_fixture_call(&provider, StatementKind::CashFlow);
    assert!(!result.complete);
    assert_eq!(result.records.len(), original.records().len());
    assert_eq!(result.observed_at, FIXTURE_OBSERVED_AT);
    assert_eq!(result.batch_id, FINANCIAL_FIXTURE_PROVIDER);
}

#[path = "grpc_sec_typed_entry_tests.rs"]
mod sec_typed_entry_coverage;
