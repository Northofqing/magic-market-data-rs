use magic_market_core::{
    AssetClass, DataBatch, Exchange, InstrumentId, MarketStatistics, MarketStatisticsProvider,
    Money, Provenance, ProviderId, SourceEvidence,
};
use magic_market_router::{
    market_statistics_source, AcceptancePolicy, AttemptStatus, FailureKind, MarketStatisticsRouter,
    SourceError,
};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("fixture")]
struct FixtureError;

struct FixtureProvider {
    record_provider: ProviderId,
    record_batch: &'static str,
    batch_provider_name: &'static str,
    batch_id: &'static str,
}

impl MarketStatisticsProvider for FixtureProvider {
    type Error = FixtureError;

    fn market_statistics(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<MarketStatistics>, Self::Error> {
        let record = MarketStatistics::new(
            instruments[0].clone(),
            None,
            None,
            None,
            None,
            Some(Money::new(1.0).unwrap()),
            None,
            None,
            None,
            None,
            SourceEvidence::new(self.record_provider, "observed", self.record_batch).unwrap(),
        )
        .unwrap();
        Ok(DataBatch::strict(
            vec![record],
            Provenance::new(self.batch_provider_name, "observed")
                .unwrap()
                .with_batch_id(self.batch_id)
                .unwrap(),
        ))
    }
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Transport, "fixture")
}

#[test]
fn intelligence_adapters_reuse_evidence_preserving_failover() {
    let wrong = Arc::new(FixtureProvider {
        record_provider: ProviderId::Tencent,
        record_batch: "wrong",
        batch_provider_name: "eastmoney",
        batch_id: "wrong",
    });
    let valid = Arc::new(FixtureProvider {
        record_provider: ProviderId::Tencent,
        record_batch: "valid",
        batch_provider_name: "tencent",
        batch_id: "valid",
    });
    let mut router = MarketStatisticsRouter::new(AcceptancePolicy::new());
    router
        .register(market_statistics_source(
            ProviderId::Eastmoney,
            wrong,
            classify,
        ))
        .unwrap();
    router
        .register(market_statistics_source(
            ProviderId::Tencent,
            valid,
            classify,
        ))
        .unwrap();

    let outcome = router.route(&[instrument()]).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Tencent);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Selected
    ));
}
