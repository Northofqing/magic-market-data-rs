use magic_market_core::{
    Adjustment, Bar, BarInterval, BookLevel, DataStatus, MoneyFlow, OrderBook, Price, ProviderId,
    Quantity, Quote,
};

#[test]
fn unavailable_fields_are_explicit() {
    assert_eq!(DataStatus::Unavailable, DataStatus::Unavailable);
    let level = BookLevel {
        price: None,
        quantity: None,
    };
    assert!(level.price.is_none());
    let _flow = MoneyFlow {
        instrument: magic_market_core::InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            "600519",
            magic_market_core::AssetClass::Equity,
        )
        .unwrap(),
        main_net: None,
        super_large_net: None,
        large_net: None,
        medium_net: None,
        small_net: None,
        status: DataStatus::Unavailable,
        source_at: None,
        observed_at: "observed".into(),
        provider: ProviderId::Eastmoney,
        batch_id: "batch-1".into(),
    };
}

#[test]
fn normalized_bar_rejects_inconsistent_ohlc() {
    let instrument = magic_market_core::InstrumentId::new(
        magic_market_core::Exchange::Shanghai,
        "600519",
        magic_market_core::AssetClass::Equity,
    )
    .unwrap();
    let result = Bar::new(
        instrument,
        BarInterval::Day,
        "2026-07-22",
        "2026-07-22",
        Price::new(100.0).unwrap(),
        Price::new(99.0).unwrap(),
        Price::new(98.0).unwrap(),
        Price::new(100.0).unwrap(),
        Quantity::new(1.0).unwrap(),
        None,
        Adjustment::Unadjusted,
        ProviderId::Eastmoney,
        "batch-1",
    );
    assert!(result.is_err());
}

#[test]
fn order_book_has_fixed_five_level_shape() {
    let level = BookLevel {
        price: None,
        quantity: None,
    };
    let book = OrderBook {
        instrument: magic_market_core::InstrumentId::new(
            magic_market_core::Exchange::Shenzhen,
            "000001",
            magic_market_core::AssetClass::Equity,
        )
        .unwrap(),
        bids: [level; 5],
        asks: [level; 5],
        status: DataStatus::Unsupported,
    };
    assert_eq!(book.bids.len(), 5);
    assert_eq!(book.asks.len(), 5);
}

#[test]
fn quote_keeps_source_and_observation_times_separate() {
    let instrument = magic_market_core::InstrumentId::new(
        magic_market_core::Exchange::Shanghai,
        "600519",
        magic_market_core::AssetClass::Equity,
    )
    .unwrap();
    let quote = Quote::new(
        instrument,
        Price::new(1300.0).unwrap(),
        Quantity::new(10.0).unwrap(),
        None,
        "observed",
        ProviderId::Tdx,
        "batch-1",
    )
    .with_source_at("source");
    assert_eq!(quote.source_at.as_deref(), Some("source"));
    assert_eq!(quote.observed_at, "observed");
    assert_eq!(quote.batch_id, "batch-1");
}
