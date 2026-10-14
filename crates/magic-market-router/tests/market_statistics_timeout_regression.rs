use magic_market_core::{
    AssetClass, DataBatch, Exchange, FiniteNumber, InstrumentId, MarketStatistics,
    MarketStatisticsProvider, Money, Provenance, ProviderId, SourceEvidence,
};
use magic_market_router::{
    market_statistics_source, AcceptancePolicy, AttemptStatus, FailureAction, FailureKind,
    MarketStatisticsRouter, SourceError,
};
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
enum ExternalStatisticsError {
    #[error("offline market-statistics deadline exceeded")]
    DeadlineExceeded,
}

#[derive(Clone)]
struct ExternalStatisticsFixture(Result<DataBatch<MarketStatistics>, ExternalStatisticsError>);

impl MarketStatisticsProvider for ExternalStatisticsFixture {
    type Error = ExternalStatisticsError;

    fn market_statistics(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<MarketStatistics>, Self::Error> {
        self.0.clone()
    }
}

fn classify_external_error(error: ExternalStatisticsError) -> SourceError {
    match error {
        ExternalStatisticsError::DeadlineExceeded => SourceError::try_next(
            FailureKind::Timeout,
            "market-statistics request deadline exceeded",
        ),
    }
}

#[test]
fn market_statistics_route_retains_timeout_and_preserves_negative_pe_zero_and_missing_values() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
    let record = MarketStatistics::new(
        instrument.clone(),
        None,
        Some(FiniteNumber::new(-12.5).unwrap()),
        None,
        Some(FiniteNumber::new(3.25).unwrap()),
        Some(Money::new(0.0).unwrap()),
        None,
        None,
        None,
        None,
        SourceEvidence::new(
            ProviderId::Tencent,
            "2026-10-09T09:30:01+08:00",
            "statistics-secondary:20261009:600396",
        )
        .unwrap()
        .with_source_at("2026-10-09T09:30:00+08:00")
        .unwrap(),
    )
    .unwrap();
    let secondary_batch = DataBatch::strict(
        vec![record],
        Provenance::new("offline-statistics-secondary", "2026-10-09T09:30:01+08:00")
            .unwrap()
            .with_source_at("2026-10-09T09:30:00+08:00")
            .unwrap()
            .with_batch_id("statistics-secondary:20261009:600396")
            .unwrap(),
    );
    let mut router = MarketStatisticsRouter::new(
        AcceptancePolicy::new()
            .with_require_complete(true)
            .with_require_source_at(true),
    );
    router
        .register(market_statistics_source(
            ProviderId::Eastmoney,
            Arc::new(ExternalStatisticsFixture(Err(
                ExternalStatisticsError::DeadlineExceeded,
            ))),
            classify_external_error,
        ))
        .unwrap()
        .register(market_statistics_source(
            ProviderId::Tencent,
            Arc::new(ExternalStatisticsFixture(Ok(secondary_batch))),
            classify_external_error,
        ))
        .unwrap();

    let outcome = router.route(std::slice::from_ref(&instrument)).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Tencent);
    assert_eq!(outcome.attempts().len(), 2);
    assert_eq!(outcome.attempts()[0].provider_id(), ProviderId::Eastmoney);
    assert_eq!(
        outcome.attempts()[0].status(),
        &AttemptStatus::Failed {
            kind: FailureKind::Timeout,
            action: FailureAction::TryNext,
            message: "market-statistics request deadline exceeded".into(),
        }
    );
    assert_eq!(outcome.attempts()[1].provider_id(), ProviderId::Tencent);
    assert_eq!(outcome.attempts()[1].status(), &AttemptStatus::Selected);

    assert!(outcome.batch().quality().is_complete());
    assert_eq!(outcome.batch().records().len(), 1);
    let record = &outcome.batch().records()[0];
    assert_eq!(record.instrument().exchange(), Exchange::Shanghai);
    assert_eq!(record.instrument().code(), "600396");
    assert_eq!(record.instrument().asset_class(), AssetClass::Equity);
    assert_eq!(record.trailing_pe().unwrap().get(), -12.5);
    assert_eq!(record.pb().unwrap().get(), 3.25);
    assert_eq!(record.total_market_cap().unwrap().get(), 0.0);
    assert!(record.static_pe().is_none());
    assert!(record.floating_market_cap().is_none());
    assert_eq!(record.evidence().provider(), ProviderId::Tencent);
    assert_eq!(
        record.evidence().batch_id(),
        "statistics-secondary:20261009:600396"
    );
    assert_eq!(record.evidence().observed_at(), "2026-10-09T09:30:01+08:00");
    assert_eq!(
        record.evidence().source_at(),
        Some("2026-10-09T09:30:00+08:00")
    );
    let provenance = outcome.batch().provenance();
    assert_eq!(provenance.source(), "offline-statistics-secondary");
    assert_eq!(
        provenance.batch_id(),
        Some("statistics-secondary:20261009:600396")
    );
    assert_eq!(provenance.fetched_at(), "2026-10-09T09:30:01+08:00");
    assert_eq!(provenance.source_at(), Some("2026-10-09T09:30:00+08:00"));
}
