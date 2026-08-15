use magic_market_core::{
    AssetClass, DataBatch, Exchange, InstrumentId, IsoDate, NonEmptyText, Price, Provenance,
    ProviderId, SourceEvidence, TargetPriceConsensus, TargetPriceData, TargetPriceObservation,
    TargetPriceRequest,
};
use magic_market_router::{
    target_price_source, AcceptancePolicy, AttemptStatus, FailureAction, FailureKind, RoutedSource,
    SourceError, TargetPriceRouter,
};
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
#[error("fixture target-price failure")]
struct FixtureError;

#[derive(Clone)]
struct FixtureProvider {
    result: Result<DataBatch<TargetPriceConsensus>, FixtureError>,
}

impl TargetPriceData for FixtureProvider {
    type Error = FixtureError;

    fn target_price_consensus(
        &self,
        _request: &TargetPriceRequest,
    ) -> Result<DataBatch<TargetPriceConsensus>, Self::Error> {
        self.result.clone()
    }
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap()
}

fn request(from: &str, through: &str) -> TargetPriceRequest {
    request_for(instrument(), from, through)
}

fn request_for(instrument: InstrumentId, from: &str, through: &str) -> TargetPriceRequest {
    TargetPriceRequest::new(
        instrument,
        IsoDate::new(from).unwrap(),
        IsoDate::new(through).unwrap(),
    )
    .unwrap()
}

fn evidence_at(
    provider: ProviderId,
    batch_id: &str,
    observed_at: &str,
    source_at: &str,
) -> SourceEvidence {
    SourceEvidence::new(provider, observed_at, batch_id)
        .unwrap()
        .with_source_at(source_at)
        .unwrap()
}

fn consensus(
    request: &TargetPriceRequest,
    provider: ProviderId,
    batch_id: &str,
) -> TargetPriceConsensus {
    consensus_at(
        request,
        provider,
        batch_id,
        "2026-07-27T10:00:00+08:00",
        "2026-07-27T10:00:00+08:00",
        "2026-07-20T08:00:00+08:00",
    )
}

fn consensus_at(
    request: &TargetPriceRequest,
    provider: ProviderId,
    batch_id: &str,
    observation_observed_at: &str,
    aggregate_observed_at: &str,
    source_at: &str,
) -> TargetPriceConsensus {
    let observation = TargetPriceObservation::new(
        request.instrument().clone(),
        NonEmptyText::new("贵州茅台").unwrap(),
        NonEmptyText::new("report-1").unwrap(),
        NonEmptyText::new("institution-1").unwrap(),
        NonEmptyText::new("机构一").unwrap(),
        IsoDate::new("2026-07-20").unwrap(),
        Price::new(1_430.0).unwrap(),
        Price::new(1_400.0).unwrap(),
        evidence_at(provider, batch_id, observation_observed_at, source_at),
    )
    .unwrap();
    TargetPriceConsensus::new(
        request,
        vec![observation],
        evidence_at(provider, batch_id, aggregate_observed_at, source_at),
    )
    .unwrap()
}

fn batch(
    request: &TargetPriceRequest,
    provider: ProviderId,
    record_batch_id: &str,
    provenance_batch_id: &str,
) -> DataBatch<TargetPriceConsensus> {
    DataBatch::strict(
        vec![consensus(request, provider, record_batch_id)],
        Provenance::new("fixture", "2026-07-27T10:00:00+08:00")
            .unwrap()
            .with_source_at("2026-07-20T08:00:00+08:00")
            .unwrap()
            .with_batch_id(provenance_batch_id)
            .unwrap(),
    )
}

fn source(
    registered_provider: ProviderId,
    result: Result<DataBatch<TargetPriceConsensus>, FixtureError>,
) -> impl RoutedSource<TargetPriceRequest, TargetPriceConsensus> {
    target_price_source(
        registered_provider,
        Arc::new(FixtureProvider { result }),
        |_| {
            SourceError::try_next(
                FailureKind::Transport,
                "fixture target-price transport failure",
            )
        },
    )
}

#[test]
fn router_fails_over_and_preserves_target_price_consensus() {
    let request = request("2026-01-01", "2026-07-27");
    let mut router = TargetPriceRouter::new(AcceptancePolicy::new());
    router
        .register(source(ProviderId::Tencent, Err(FixtureError)))
        .unwrap()
        .register(source(
            ProviderId::Eastmoney,
            Ok(batch(
                &request,
                ProviderId::Eastmoney,
                "eastmoney-batch",
                "eastmoney-batch",
            )),
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert_eq!(outcome.batch().records().len(), 1);
    assert_eq!(
        outcome.batch().records()[0].instrument(),
        request.instrument()
    );
    assert_eq!(outcome.attempts().len(), 2);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Transport,
            action: FailureAction::TryNext,
            ..
        }
    ));
    assert_eq!(outcome.attempts()[1].status(), &AttemptStatus::Selected);
}

#[test]
fn source_rejects_non_singular_and_wrong_request_batches() {
    let route_request = request("2026-01-01", "2026-07-27");
    let value = consensus(&route_request, ProviderId::Eastmoney, "batch");
    let two_records = DataBatch::strict(
        vec![value.clone(), value],
        Provenance::new("fixture", "2026-07-27T10:00:00+08:00")
            .unwrap()
            .with_source_at("2026-07-20T08:00:00+08:00")
            .unwrap()
            .with_batch_id("batch")
            .unwrap(),
    );
    let error = source(ProviderId::Eastmoney, Ok(two_records))
        .fetch(&route_request)
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Quality);
    assert_eq!(error.action(), FailureAction::TryNext);

    let different_request = request("2026-06-01", "2026-07-27");
    let error = source(
        ProviderId::Eastmoney,
        Ok(batch(
            &different_request,
            ProviderId::Eastmoney,
            "batch",
            "batch",
        )),
    )
    .fetch(&route_request)
    .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Evidence);
    assert!(error.message().contains("requested range"));
}

#[test]
fn source_rejects_registered_provider_and_batch_evidence_mismatches() {
    let request = request("2026-01-01", "2026-07-27");
    let error = source(
        ProviderId::Eastmoney,
        Ok(batch(&request, ProviderId::Tencent, "batch", "batch")),
    )
    .fetch(&request)
    .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Evidence);
    assert!(error.message().contains("registered provider"));

    let error = source(
        ProviderId::Eastmoney,
        Ok(batch(
            &request,
            ProviderId::Eastmoney,
            "record-batch",
            "provenance-batch",
        )),
    )
    .fetch(&request)
    .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Evidence);
    assert!(error.message().contains("batch"));
}

#[test]
fn source_preserves_explicit_provider_failure_classification() {
    let request = request("2026-01-01", "2026-07-27");
    let error = source(ProviderId::Eastmoney, Err(FixtureError))
        .fetch(&request)
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Transport);
    assert_eq!(error.action(), FailureAction::TryNext);
}

#[test]
fn source_rejects_incomplete_wrong_identity_and_provenance_timestamp_drift() {
    let route_request = request("2026-01-01", "2026-07-27");
    let value = consensus(&route_request, ProviderId::Eastmoney, "batch");
    let incomplete = DataBatch::best_effort(
        vec![value.clone()],
        Provenance::new("fixture", "2026-07-27T10:00:00+08:00")
            .unwrap()
            .with_source_at("2026-07-20T08:00:00+08:00")
            .unwrap()
            .with_batch_id("batch")
            .unwrap(),
        vec!["partial page".into()],
    )
    .unwrap();
    let error = source(ProviderId::Eastmoney, Ok(incomplete))
        .fetch(&route_request)
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Quality);
    assert!(error.message().contains("partial page"));

    let other = InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap();
    let other_request = request_for(other, "2026-01-01", "2026-07-27");
    let error = source(
        ProviderId::Eastmoney,
        Ok(batch(
            &other_request,
            ProviderId::Eastmoney,
            "batch",
            "batch",
        )),
    )
    .fetch(&route_request)
    .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Evidence);
    assert!(error.message().contains("instrument"));

    let observed_drift = DataBatch::strict(
        vec![value.clone()],
        Provenance::new("fixture", "2026-07-27T10:00:01+08:00")
            .unwrap()
            .with_source_at("2026-07-20T08:00:00+08:00")
            .unwrap()
            .with_batch_id("batch")
            .unwrap(),
    );
    let error = source(ProviderId::Eastmoney, Ok(observed_drift))
        .fetch(&route_request)
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Evidence);
    assert!(error.message().contains("observation timestamp"));

    let source_drift = DataBatch::strict(
        vec![value],
        Provenance::new("fixture", "2026-07-27T10:00:00+08:00")
            .unwrap()
            .with_source_at("2026-07-20T09:00:00+08:00")
            .unwrap()
            .with_batch_id("batch")
            .unwrap(),
    );
    let error = source(ProviderId::Eastmoney, Ok(source_drift))
        .fetch(&route_request)
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Evidence);
    assert!(error.message().contains("source timestamp"));
}

#[test]
fn source_rejects_missing_batch_and_malformed_or_future_timestamps() {
    let request = request("2026-01-01", "2026-07-27");
    let missing_batch = serde_json::from_value::<Provenance>(serde_json::json!({
        "source": "fixture",
        "source_at": "2026-07-20T08:00:00+08:00",
        "fetched_at": "2026-07-27T10:00:00+08:00",
        "batch_id": null
    }));
    assert!(missing_batch.is_err());

    for (observed_at, source_at, expected) in [
        (
            "not-an-instant",
            "2026-07-20T08:00:00+08:00",
            "observation timestamp is malformed",
        ),
        (
            "2026-07-27T10:00:00+08:00",
            "2026-07-20Tabc",
            "source timestamp is malformed",
        ),
        (
            "2026-07-19T10:00:00+08:00",
            "2026-07-20T08:00:00+08:00",
            "later than its observation timestamp",
        ),
    ] {
        let value = consensus_at(
            &request,
            ProviderId::Eastmoney,
            "batch",
            observed_at,
            observed_at,
            source_at,
        );
        let batch = DataBatch::strict(
            vec![value],
            Provenance::new("fixture", observed_at)
                .unwrap()
                .with_source_at(source_at)
                .unwrap()
                .with_batch_id("batch")
                .unwrap(),
        );
        let error = source(ProviderId::Eastmoney, Ok(batch))
            .fetch(&request)
            .unwrap_err();
        assert_eq!(error.kind(), FailureKind::Evidence);
        assert!(error.message().contains(expected), "{}", error.message());
    }
}

#[test]
fn source_rejects_observation_evidence_time_drift_from_aggregate() {
    let request = request("2026-01-01", "2026-07-27");
    let value = consensus_at(
        &request,
        ProviderId::Eastmoney,
        "batch",
        "2026-07-27T09:59:59+08:00",
        "2026-07-27T10:00:00+08:00",
        "2026-07-20T08:00:00+08:00",
    );
    let batch = DataBatch::strict(
        vec![value],
        Provenance::new("fixture", "2026-07-27T10:00:00+08:00")
            .unwrap()
            .with_source_at("2026-07-20T08:00:00+08:00")
            .unwrap()
            .with_batch_id("batch")
            .unwrap(),
    );
    let error = source(ProviderId::Eastmoney, Ok(batch))
        .fetch(&request)
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Evidence);
    assert!(error.message().contains("observation evidence"));
}
