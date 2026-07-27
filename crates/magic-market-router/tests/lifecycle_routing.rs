use magic_market_core::{
    AssetClass, Board, CorporateAction, CorporateActionCategory, CorporateActionRequest,
    CorporateActionResponse, CorporateActionStatus, CorporateActionTerms, CorporateActions,
    DataBatch, DataStatus, Exchange, InstrumentId, IsoDate, PriceLimitRule, ProviderId, Ratio,
    RatioUnit, SecurityMetadata, SecurityMetadataProvider, SourceEvidence,
};
use magic_market_router::{
    corporate_action_source, security_metadata_source, AcceptancePolicy, AttemptStatus,
    CorporateActionRouter, FailureAction, FailureKind, RoutedSource, SecurityMetadataRouter,
    SourceError,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct FixtureError {
    kind: FailureKind,
    action: FailureAction,
}

impl std::fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("fixture")
    }
}

impl std::error::Error for FixtureError {}

fn classify(error: FixtureError) -> SourceError {
    SourceError::new(error.kind, error.action, "fixture")
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap()
}

fn admission_as_of() -> IsoDate {
    IsoDate::new("2026-07-27").unwrap()
}

fn action(
    provider: ProviderId,
    batch_id: &str,
    date: &str,
    category: CorporateActionCategory,
) -> CorporateAction {
    action_with_status(
        provider,
        batch_id,
        date,
        category,
        CorporateActionStatus::Implemented,
    )
}

fn action_with_status(
    provider: ProviderId,
    batch_id: &str,
    date: &str,
    category: CorporateActionCategory,
    status: CorporateActionStatus,
) -> CorporateAction {
    CorporateAction::new(
        instrument(),
        category,
        IsoDate::new(date).unwrap(),
        status,
        CorporateActionTerms::capital_rescaling(
            category,
            Ratio::new(2.0, RatioUnit::Decimal).unwrap(),
        )
        .unwrap(),
        SourceEvidence::new(provider, "2026-07-27T10:00:00+08:00", batch_id).unwrap(),
    )
    .unwrap()
}

fn action_batch(
    coverage: &CorporateActionRequest,
    batch_id: &str,
    records: Vec<CorporateAction>,
) -> CorporateActionResponse {
    let response_evidence = records
        .first()
        .expect("non-empty action fixture")
        .evidence()
        .clone();
    CorporateActionResponse::new(
        coverage.clone(),
        admission_as_of(),
        response_evidence,
        DataBatch::strict(
            records,
            magic_market_core::Provenance::new("fixture", "2026-07-27T10:00:00+08:00")
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        ),
    )
    .unwrap()
}

fn timed_action_response(
    coverage: &CorporateActionRequest,
    provider: ProviderId,
    batch_id: &str,
    observed_at: &str,
    source_at: Option<&str>,
) -> CorporateActionResponse {
    let mut record_evidence = SourceEvidence::new(provider, observed_at, batch_id).unwrap();
    let mut provenance = magic_market_core::Provenance::new("fixture", observed_at)
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap();
    if let Some(source_at) = source_at {
        record_evidence = record_evidence.with_source_at(source_at).unwrap();
        provenance = provenance.with_source_at(source_at).unwrap();
    }
    CorporateActionResponse::new(
        coverage.clone(),
        admission_as_of(),
        record_evidence.clone(),
        DataBatch::strict(
            vec![split_action(
                provider,
                batch_id,
                "2025-06-01",
                record_evidence,
            )],
            provenance,
        ),
    )
    .unwrap()
}

fn split_action(
    provider: ProviderId,
    batch_id: &str,
    date: &str,
    evidence: SourceEvidence,
) -> CorporateAction {
    assert_eq!(evidence.provider(), provider);
    assert_eq!(evidence.batch_id(), batch_id);
    CorporateAction::new(
        instrument(),
        CorporateActionCategory::CapitalRescaling,
        IsoDate::new(date).unwrap(),
        CorporateActionStatus::Implemented,
        CorporateActionTerms::capital_rescaling(
            CorporateActionCategory::CapitalRescaling,
            Ratio::new(2.0, RatioUnit::Decimal).unwrap(),
        )
        .unwrap(),
        evidence,
    )
    .unwrap()
}

fn empty_action_response(
    coverage: &CorporateActionRequest,
    provider: ProviderId,
    batch_id: &str,
) -> CorporateActionResponse {
    CorporateActionResponse::new(
        coverage.clone(),
        admission_as_of(),
        SourceEvidence::new(provider, "2026-07-27T10:00:00+08:00", batch_id).unwrap(),
        DataBatch::strict(
            Vec::new(),
            magic_market_core::Provenance::new("fixture", "2026-07-27T10:00:00+08:00")
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        ),
    )
    .unwrap()
}

#[derive(Clone)]
struct ActionProvider(Result<CorporateActionResponse, FixtureError>);

impl CorporateActions for ActionProvider {
    type Error = FixtureError;

    fn corporate_actions(
        &self,
        _request: &CorporateActionRequest,
    ) -> Result<CorporateActionResponse, Self::Error> {
        self.0.clone()
    }
}

#[test]
fn corporate_action_router_falls_back_on_exact_range_failure() {
    let request = CorporateActionRequest::new(instrument())
        .with_range(
            IsoDate::new("2025-01-01").unwrap(),
            IsoDate::new("2025-12-31").unwrap(),
        )
        .unwrap();
    let outside_coverage = CorporateActionRequest::new(instrument());
    let outside = Arc::new(ActionProvider(Ok(action_batch(
        &outside_coverage,
        "tdx-1",
        vec![action(
            ProviderId::Tdx,
            "tdx-1",
            "2024-12-31",
            CorporateActionCategory::CapitalRescaling,
        )],
    ))));
    let valid = Arc::new(ActionProvider(Ok(action_batch(
        &request,
        "eastmoney-1",
        vec![action(
            ProviderId::Eastmoney,
            "eastmoney-1",
            "2025-06-01",
            CorporateActionCategory::CapitalRescaling,
        )],
    ))));

    let mut router = CorporateActionRouter::new(AcceptancePolicy::new(), admission_as_of());
    router
        .register(corporate_action_source(ProviderId::Tdx, outside, classify))
        .unwrap()
        .register(corporate_action_source(
            ProviderId::Eastmoney,
            valid,
            classify,
        ))
        .unwrap();
    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert_eq!(outcome.admission_as_of(), &admission_as_of());
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn corporate_action_router_requires_exact_typed_coverage_for_verified_empty() {
    let request = CorporateActionRequest::new(instrument())
        .with_range(
            IsoDate::new("1900-01-01").unwrap(),
            IsoDate::new("1900-12-31").unwrap(),
        )
        .unwrap();
    let wrong_coverage = CorporateActionRequest::new(instrument())
        .with_range(
            IsoDate::new("1901-01-01").unwrap(),
            IsoDate::new("1901-12-31").unwrap(),
        )
        .unwrap();
    let wrong = Arc::new(ActionProvider(Ok(empty_action_response(
        &wrong_coverage,
        ProviderId::Tdx,
        "tdx-empty",
    ))));
    let exact = Arc::new(ActionProvider(Ok(empty_action_response(
        &request,
        ProviderId::Eastmoney,
        "eastmoney-empty",
    ))));
    let mut router = CorporateActionRouter::new(
        AcceptancePolicy::new().with_accept_complete_empty(true),
        admission_as_of(),
    );
    router
        .register(corporate_action_source(ProviderId::Tdx, wrong, classify))
        .unwrap()
        .register(corporate_action_source(
            ProviderId::Eastmoney,
            exact,
            classify,
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert!(outcome.batch().records().is_empty());
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn corporate_action_router_rejects_verified_empty_from_the_wrong_provider() {
    let request = CorporateActionRequest::new(instrument())
        .with_range(
            IsoDate::new("1900-01-01").unwrap(),
            IsoDate::new("1900-12-31").unwrap(),
        )
        .unwrap();
    let wrong_provider = Arc::new(ActionProvider(Ok(empty_action_response(
        &request,
        ProviderId::Tencent,
        "wrong-provider-empty",
    ))));
    let mut router = CorporateActionRouter::new(
        AcceptancePolicy::new().with_accept_complete_empty(true),
        admission_as_of(),
    );
    router
        .register(corporate_action_source(
            ProviderId::Tdx,
            wrong_provider,
            classify,
        ))
        .unwrap();

    let error = router.route(&request).unwrap_err();
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn corporate_action_router_rejects_provider_evidence_atomically() {
    let request = CorporateActionRequest::new(instrument())
        .with_range(
            IsoDate::new("2025-01-01").unwrap(),
            IsoDate::new("2025-12-31").unwrap(),
        )
        .unwrap();
    let wrong_provider = Arc::new(ActionProvider(Ok(timed_action_response(
        &request,
        ProviderId::Tencent,
        "wrong-provider",
        "2026-07-27T10:00:00+08:00",
        None,
    ))));
    let valid = Arc::new(ActionProvider(Ok(timed_action_response(
        &request,
        ProviderId::Eastmoney,
        "valid",
        "2026-07-27T10:00:00+08:00",
        Some("2026-07-27T09:59:59+08:00"),
    ))));
    let mut router = CorporateActionRouter::new(AcceptancePolicy::new(), admission_as_of());
    router
        .register(corporate_action_source(
            ProviderId::Tdx,
            wrong_provider,
            classify,
        ))
        .unwrap()
        .register(corporate_action_source(
            ProviderId::Eastmoney,
            valid,
            classify,
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn corporate_action_router_requires_the_explicit_admission_date() {
    let request = CorporateActionRequest::new(instrument());
    let provider = ProviderId::Tdx;
    let batch_id = "future-policy";
    let observed_at = "2026-07-27T10:00:00+08:00";
    let record_evidence = SourceEvidence::new(provider, observed_at, batch_id).unwrap();
    let response = CorporateActionResponse::new(
        request.clone(),
        IsoDate::new("2026-07-28").unwrap(),
        record_evidence.clone(),
        DataBatch::strict(
            vec![split_action(
                provider,
                batch_id,
                "2026-07-28",
                record_evidence,
            )],
            magic_market_core::Provenance::new("fixture", observed_at)
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        ),
    )
    .unwrap();
    let mut router = CorporateActionRouter::new(AcceptancePolicy::new(), admission_as_of());
    router
        .register(corporate_action_source(
            provider,
            Arc::new(ActionProvider(Ok(response))),
            classify,
        ))
        .unwrap();

    let error = router.route(&request).unwrap_err();
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn corporate_action_router_stops_future_ranges_at_its_single_policy_boundary() {
    let request = CorporateActionRequest::new(instrument())
        .with_range(
            IsoDate::new("2026-07-28").unwrap(),
            IsoDate::new("2026-07-29").unwrap(),
        )
        .unwrap();
    let provider = Arc::new(ActionProvider(Err(FixtureError {
        kind: FailureKind::Quality,
        action: FailureAction::TryNext,
    })));
    let mut router = CorporateActionRouter::new(AcceptancePolicy::new(), admission_as_of());
    router
        .register(corporate_action_source(ProviderId::Tdx, provider, classify))
        .unwrap();

    let error = router.route(&request).unwrap_err();
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::InvalidRequest,
            action: FailureAction::Stop,
            ..
        }
    ));
}

#[test]
fn corporate_action_router_accepts_verified_empty_after_explicit_quality_failure() {
    let request = CorporateActionRequest::new(instrument());
    let bad = Arc::new(ActionProvider(Err(FixtureError {
        kind: FailureKind::Quality,
        action: FailureAction::TryNext,
    })));
    let empty = Arc::new(ActionProvider(Ok(empty_action_response(
        &request,
        ProviderId::Eastmoney,
        "eastmoney-empty",
    ))));
    let mut router = CorporateActionRouter::new(
        AcceptancePolicy::new().with_accept_complete_empty(true),
        admission_as_of(),
    );
    router
        .register(corporate_action_source(ProviderId::Tdx, bad, classify))
        .unwrap()
        .register(corporate_action_source(
            ProviderId::Eastmoney,
            empty,
            classify,
        ))
        .unwrap();
    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert!(outcome.batch().records().is_empty());
}

#[test]
fn invalid_request_classification_stops_lifecycle_failover() {
    let request = CorporateActionRequest::new(instrument());
    let invalid = Arc::new(ActionProvider(Err(FixtureError {
        kind: FailureKind::InvalidRequest,
        action: FailureAction::Stop,
    })));
    let later = Arc::new(ActionProvider(Ok(empty_action_response(
        &request,
        ProviderId::Eastmoney,
        "eastmoney-1",
    ))));
    let mut router = CorporateActionRouter::new(
        AcceptancePolicy::new().with_accept_complete_empty(true),
        admission_as_of(),
    );
    router
        .register(corporate_action_source(ProviderId::Tdx, invalid, classify))
        .unwrap()
        .register(corporate_action_source(
            ProviderId::Eastmoney,
            later,
            classify,
        ))
        .unwrap();
    let error = router.route(&request).unwrap_err();
    assert_eq!(error.attempts().len(), 1);
}

#[derive(Clone)]
struct MetadataProvider(DataBatch<SecurityMetadata>);

impl SecurityMetadataProvider for MetadataProvider {
    type Error = FixtureError;

    fn security_metadata(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        Ok(self.0.clone())
    }
}

fn metadata(
    provider: ProviderId,
    record_batch: &str,
    batch_id: &str,
) -> DataBatch<SecurityMetadata> {
    metadata_for(instrument(), provider, record_batch, batch_id)
}

fn metadata_for(
    returned_instrument: InstrumentId,
    provider: ProviderId,
    record_batch: &str,
    batch_id: &str,
) -> DataBatch<SecurityMetadata> {
    let record = SecurityMetadata::new(
        returned_instrument,
        Some("贵州茅台".into()),
        Some(Board::Main),
        Some(false),
        Some("2001-08-27".into()),
        PriceLimitRule::new(None, None).unwrap(),
        DataStatus::Unavailable,
        None,
        "observed",
        provider,
        record_batch,
    )
    .unwrap();
    DataBatch::strict(
        vec![record],
        magic_market_core::Provenance::new("fixture", "observed")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    )
}

#[test]
fn real_security_metadata_router_routes_and_rejects_provider_or_batch_mismatch() {
    let bad_provider = Arc::new(MetadataProvider(metadata(
        ProviderId::Tencent,
        "bad-provider",
        "bad-provider",
    )));
    let bad_batch = Arc::new(MetadataProvider(metadata(
        ProviderId::Eastmoney,
        "record-batch",
        "provenance-batch",
    )));
    let valid = Arc::new(MetadataProvider(metadata(
        ProviderId::Tdx,
        "tdx-batch",
        "tdx-batch",
    )));
    let mut router = SecurityMetadataRouter::new(AcceptancePolicy::new());
    router
        .register(security_metadata_source(
            ProviderId::Tdx,
            bad_provider,
            classify,
        ))
        .unwrap()
        .register(security_metadata_source(
            ProviderId::Eastmoney,
            bad_batch,
            classify,
        ))
        .unwrap()
        .register(security_metadata_source(
            ProviderId::Custom,
            valid,
            classify,
        ))
        .unwrap();

    // The final source is intentionally registered under the wrong ID as well,
    // so all three attempts prove the generic provider/batch evidence gate.
    let error = router.route(&[instrument()]).unwrap_err();
    assert_eq!(error.attempts().len(), 3);
    assert!(error.attempts().iter().all(|attempt| matches!(
        attempt.status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    )));
}

#[test]
fn security_metadata_source_rejects_unrequested_and_duplicate_identities() {
    let other = InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap();
    let wrong = Arc::new(MetadataProvider(metadata_for(
        other,
        ProviderId::Tdx,
        "tdx-wrong",
        "tdx-wrong",
    )));
    let mut router = SecurityMetadataRouter::new(AcceptancePolicy::new());
    router
        .register(security_metadata_source(ProviderId::Tdx, wrong, classify))
        .unwrap();
    let error = router.route(&[instrument()]).unwrap_err();
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));

    let provider = Arc::new(MetadataProvider(metadata(
        ProviderId::Tdx,
        "tdx-ok",
        "tdx-ok",
    )));
    let mut duplicates = SecurityMetadataRouter::new(AcceptancePolicy::new());
    duplicates
        .register(security_metadata_source(
            ProviderId::Tdx,
            provider,
            classify,
        ))
        .unwrap();
    let requested = [instrument(), instrument()];
    let error = duplicates.route(&requested).unwrap_err();
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::InvalidRequest,
            action: FailureAction::Stop,
            ..
        }
    ));
}

#[test]
fn corporate_action_facade_exposes_policy_sources_debug_and_owned_outcomes() {
    let request = CorporateActionRequest::new(instrument());
    let policy = AcceptancePolicy::new();
    let response = action_batch(
        &request,
        "facade-batch",
        vec![action(
            ProviderId::Eastmoney,
            "facade-batch",
            "2026-07-01",
            CorporateActionCategory::CapitalRescaling,
        )],
    );
    let source = corporate_action_source(
        ProviderId::Eastmoney,
        Arc::new(ActionProvider(Ok(response.clone()))),
        classify,
    );
    assert!(format!("{source:?}").contains("Eastmoney"));

    let mut router = CorporateActionRouter::new(policy, admission_as_of());
    assert_eq!(router.policy(), policy);
    assert_eq!(router.admission_as_of(), &admission_as_of());
    router.register(source).unwrap();
    assert_eq!(router.provider_ids(), vec![ProviderId::Eastmoney]);
    assert!(format!("{router:?}").contains("CorporateActionRouter"));

    let owned_batch = router.route(&request).unwrap().into_batch();
    assert_eq!(owned_batch.records().len(), 1);

    let (admission, batch, attempts) = router.route(&request).unwrap().into_parts();
    assert_eq!(admission, admission_as_of());
    assert_eq!(batch.records().len(), 1);
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].status(), &AttemptStatus::Selected);
}

#[test]
fn corporate_action_router_preserves_source_status_and_projects_only_implemented_actions() {
    let request = CorporateActionRequest::new(instrument());
    let response = action_batch(
        &request,
        "status-batch",
        vec![action_with_status(
            ProviderId::Eastmoney,
            "status-batch",
            "2026-07-01",
            CorporateActionCategory::CapitalRescaling,
            CorporateActionStatus::Proposed,
        )],
    );
    let mut router = CorporateActionRouter::new(AcceptancePolicy::new(), admission_as_of());
    router
        .register(corporate_action_source(
            ProviderId::Eastmoney,
            Arc::new(ActionProvider(Ok(response))),
            classify,
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(
        outcome.batch().records()[0].status(),
        CorporateActionStatus::Proposed
    );
    assert_eq!(outcome.implemented_actions().count(), 0);
}

#[test]
fn security_metadata_source_rejects_empty_and_duplicate_returned_identities() {
    let provider = Arc::new(MetadataProvider(metadata(
        ProviderId::Tdx,
        "metadata-batch",
        "metadata-batch",
    )));
    let source = security_metadata_source(ProviderId::Tdx, Arc::clone(&provider), classify);
    let error = source.fetch(&[]).unwrap_err();
    assert_eq!(error.kind(), FailureKind::InvalidRequest);
    assert_eq!(error.action(), FailureAction::Stop);

    let other = InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap();
    let record = provider.0.records()[0].clone();
    let duplicates = DataBatch::strict(
        vec![record.clone(), record],
        magic_market_core::Provenance::new("fixture", "observed")
            .unwrap()
            .with_batch_id("metadata-batch")
            .unwrap(),
    );
    let source = security_metadata_source(
        ProviderId::Tdx,
        Arc::new(MetadataProvider(duplicates)),
        classify,
    );
    let error = source.fetch(&[instrument(), other]).unwrap_err();
    assert_eq!(error.kind(), FailureKind::Quality);
    assert!(error.message().contains("duplicate instrument"));
}

#[test]
fn corporate_action_facade_propagates_duplicate_registration_error() {
    let request = CorporateActionRequest::new(instrument());
    let response = action_batch(
        &request,
        "duplicate-register",
        vec![action(
            ProviderId::Eastmoney,
            "duplicate-register",
            "2026-07-01",
            CorporateActionCategory::CapitalRescaling,
        )],
    );
    let mut router = CorporateActionRouter::new(AcceptancePolicy::new(), admission_as_of());
    router
        .register(corporate_action_source(
            ProviderId::Eastmoney,
            Arc::new(ActionProvider(Ok(response.clone()))),
            classify,
        ))
        .unwrap();
    let error = router
        .register(corporate_action_source(
            ProviderId::Eastmoney,
            Arc::new(ActionProvider(Ok(response))),
            classify,
        ))
        .unwrap_err();
    assert!(matches!(
        error,
        magic_market_router::RouterError::InvalidConfiguration(_)
    ));
}
