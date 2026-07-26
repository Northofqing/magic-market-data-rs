use magic_market_core::{
    Adjustment, AssetClass, Bar, BarInterval, BarsRequest, DataBatch, Exchange, HistoricalBars,
    InstrumentId, Money, Price, Provenance, ProviderId, Quantity, SourcedRecord,
};
use magic_market_router::{
    bars_source, AcceptancePolicy, AttemptStatus, FailoverChain, FailureKind, SourceError,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("scripted bars failure")]
struct ScriptedError;

struct ScriptedBars {
    batch: DataBatch<Bar>,
    calls: Arc<AtomicUsize>,
}

impl HistoricalBars for ScriptedBars {
    type Bar = Bar;
    type Error = ScriptedError;

    fn historical_bars(&self, _request: &BarsRequest) -> Result<DataBatch<Bar>, Self::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.batch.clone())
    }
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn batch(provider: ProviderId, record_batch_id: &str, batch_id: &str) -> DataBatch<Bar> {
    let bar = Bar::new(
        instrument(),
        BarInterval::Day,
        "2026-07-23",
        "2026-07-23",
        Price::new(15.0).unwrap(),
        Price::new(16.0).unwrap(),
        Price::new(14.8).unwrap(),
        Price::new(15.5).unwrap(),
        Quantity::new(10_000.0).unwrap(),
        Some(Money::new(15_500_000.0).unwrap()),
        Adjustment::Unadjusted,
        provider,
        record_batch_id,
    )
    .unwrap()
    .with_source_at("2026-07-23")
    .unwrap();
    let provenance = Provenance::new("scripted", "1784775600")
        .unwrap()
        .with_source_at("2026-07-23")
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap();
    DataBatch::strict(vec![bar], provenance)
}

fn source(
    provider: ProviderId,
    batch: DataBatch<Bar>,
    calls: Arc<AtomicUsize>,
) -> impl magic_market_router::RoutedSource<BarsRequest, Bar> {
    bars_source(provider, Arc::new(ScriptedBars { batch, calls }), |_| {
        SourceError::try_next(FailureKind::Provider, "scripted bars failure")
    })
}

#[test]
fn complete_tdx_batch_is_selected_without_calling_fallback() {
    let tdx_calls = Arc::new(AtomicUsize::new(0));
    let tencent_calls = Arc::new(AtomicUsize::new(0));
    let mut chain = FailoverChain::new(
        AcceptancePolicy::new()
            .with_require_complete(true)
            .with_require_source_at(true),
    );
    chain
        .register(source(
            ProviderId::Tdx,
            batch(ProviderId::Tdx, "tdx:1", "tdx:1"),
            Arc::clone(&tdx_calls),
        ))
        .unwrap();
    chain
        .register(source(
            ProviderId::Tencent,
            batch(ProviderId::Tencent, "tencent:1", "tencent:1"),
            Arc::clone(&tencent_calls),
        ))
        .unwrap();

    let request = BarsRequest::new(instrument(), BarInterval::Day, 1).unwrap();
    let outcome = chain.route(&request).unwrap();

    assert_eq!(outcome.selected_provider(), ProviderId::Tdx);
    assert_eq!(tdx_calls.load(Ordering::SeqCst), 1);
    assert_eq!(tencent_calls.load(Ordering::SeqCst), 0);
    assert_eq!(outcome.batch().records()[0].provider_id(), ProviderId::Tdx);
}

#[test]
fn invalid_tdx_evidence_fails_over_to_one_complete_tencent_batch() {
    let tdx_calls = Arc::new(AtomicUsize::new(0));
    let tencent_calls = Arc::new(AtomicUsize::new(0));
    let mut chain = FailoverChain::new(
        AcceptancePolicy::new()
            .with_require_complete(true)
            .with_require_source_at(true),
    );
    chain
        .register(source(
            ProviderId::Tdx,
            batch(ProviderId::Tdx, "tdx:record", "tdx:batch"),
            Arc::clone(&tdx_calls),
        ))
        .unwrap();
    chain
        .register(source(
            ProviderId::Tencent,
            batch(ProviderId::Tencent, "tencent:1", "tencent:1"),
            Arc::clone(&tencent_calls),
        ))
        .unwrap();

    let request = BarsRequest::new(instrument(), BarInterval::Day, 1).unwrap();
    let outcome = chain.route(&request).unwrap();

    assert_eq!(outcome.selected_provider(), ProviderId::Tencent);
    assert_eq!(tdx_calls.load(Ordering::SeqCst), 1);
    assert_eq!(tencent_calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(outcome
        .batch()
        .records()
        .iter()
        .all(|record| record.provider_id() == ProviderId::Tencent
            && record.evidence_batch_id() == outcome.batch().provenance().batch_id().unwrap()));
}
