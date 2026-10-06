use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use super::*;

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
