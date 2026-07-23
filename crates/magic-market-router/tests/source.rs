use magic_market_core::{
    AssetClass, DataBatch, Exchange, InstrumentId, Money, Price, Provenance, ProviderId, Quantity,
    Quote,
};
use magic_market_router::{FailureAction, FailureKind, RoutedSource, SourceError, SourceFn};

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn quote_batch() -> DataBatch<Quote> {
    let quote = Quote::new(
        instrument(),
        Price::new(15.5).unwrap(),
        Quantity::new(100.0).unwrap(),
        Some(Money::new(155_000.0).unwrap()),
        "observed",
        ProviderId::Custom,
        "custom:batch",
    )
    .unwrap();
    DataBatch::strict(
        vec![quote],
        Provenance::new("custom", "observed")
            .unwrap()
            .with_batch_id("custom:batch")
            .unwrap(),
    )
}

#[test]
fn source_fn_exposes_provider_and_fetches() {
    let batch = quote_batch();
    let source =
        SourceFn::<[InstrumentId], Quote>::new(ProviderId::Custom, move |_| Ok(batch.clone()));

    assert_eq!(source.provider_id(), ProviderId::Custom);
    assert_eq!(source.fetch(&[instrument()]).unwrap().records().len(), 1);
    assert!(format!("{source:?}").contains("Custom"));
}

#[test]
fn source_error_keeps_explicit_kind_and_action() {
    let error = SourceError::new(
        FailureKind::InvalidRequest,
        FailureAction::Stop,
        "duplicate instrument",
    );
    assert_eq!(error.kind(), FailureKind::InvalidRequest);
    assert_eq!(error.action(), FailureAction::Stop);
    assert_eq!(error.message(), "duplicate instrument");
}
