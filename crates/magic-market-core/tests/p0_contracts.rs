use magic_market_core::{
    Adjustment, AuctionSnapshot, Bar, BarInterval, BookLevel, DataStatus, Money, MoneyFlow,
    OrderBook, Price, ProviderId, Quantity, Quote, Ratio, RatioUnit,
};

#[test]
fn unavailable_fields_are_explicit() {
    assert_eq!(DataStatus::Unavailable, DataStatus::Unavailable);
    let level = BookLevel::unavailable();
    assert!(level.price().is_none());
    let _flow = MoneyFlow::new(
        magic_market_core::InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            "600519",
            magic_market_core::AssetClass::Equity,
        )
        .unwrap(),
        None,
        None,
        None,
        None,
        None,
        DataStatus::Unavailable,
        None,
        "observed",
        ProviderId::Eastmoney,
        "batch-1",
    )
    .unwrap();
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
fn normalized_bar_rejects_unparseable_market_times() {
    let instrument = magic_market_core::InstrumentId::new(
        magic_market_core::Exchange::Shanghai,
        "600519",
        magic_market_core::AssetClass::Equity,
    )
    .unwrap();
    let make = |start: &str, end: &str| {
        Bar::new(
            instrument.clone(),
            BarInterval::Minute1,
            start,
            end,
            Price::new(100.0).unwrap(),
            Price::new(101.0).unwrap(),
            Price::new(99.0).unwrap(),
            Price::new(100.0).unwrap(),
            Quantity::new(1.0).unwrap(),
            None,
            Adjustment::Unadjusted,
            ProviderId::Eastmoney,
            "batch-1",
        )
    };

    assert!(make("2026-02-30 09:30:00", "2026-02-30 09:31:00").is_err());
    assert!(make("2026-07-22 25:00:00", "2026-07-22 25:01:00").is_err());
}

#[test]
fn order_book_has_fixed_five_level_shape() {
    let level = BookLevel::unavailable();
    let book = OrderBook::new(
        magic_market_core::InstrumentId::new(
            magic_market_core::Exchange::Shenzhen,
            "000001",
            magic_market_core::AssetClass::Equity,
        )
        .unwrap(),
        [level; 5],
        [level; 5],
        None,
        None,
        DataStatus::Unsupported,
        None,
        "observed",
        ProviderId::Tdx,
        "batch-1",
    )
    .unwrap();
    assert_eq!(book.bids().len(), 5);
    assert_eq!(book.asks().len(), 5);
    assert_eq!(book.observed_at(), "observed");
    assert_eq!(book.batch_id(), "batch-1");
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
    .unwrap()
    .with_source_at("source")
    .unwrap();
    assert_eq!(quote.source_at(), Some("source"));
    assert_eq!(quote.observed_at(), "observed");
    assert_eq!(quote.batch_id(), "batch-1");
}

#[test]
fn complete_quote_and_bar_expose_all_normalized_fields() {
    let instrument = magic_market_core::InstrumentId::new(
        magic_market_core::Exchange::Shanghai,
        "600519",
        magic_market_core::AssetClass::Equity,
    )
    .unwrap();
    let quote = Quote::from_parts(
        instrument.clone(),
        Some("贵州茅台".into()),
        Price::new(1_308.0).unwrap(),
        Some(Price::new(1_300.0).unwrap()),
        Some(Price::new(1_301.0).unwrap()),
        Some(Price::new(1_310.0).unwrap()),
        Some(Price::new(1_295.0).unwrap()),
        Some(Ratio::new(0.62, RatioUnit::Percent).unwrap()),
        Quantity::new(100.0).unwrap(),
        Some(Money::new(130_800.0).unwrap()),
        DataStatus::Available,
        Some("2026-07-22T10:00:00+08:00".into()),
        "2026-07-22T10:00:01+08:00",
        ProviderId::Tdx,
        "quote-batch",
    )
    .unwrap();
    assert_eq!(quote.instrument(), &instrument);
    assert_eq!(quote.name(), Some("贵州茅台"));
    assert_eq!(quote.price().get(), 1_308.0);
    assert_eq!(quote.previous_close().unwrap().get(), 1_300.0);
    assert_eq!(quote.open().unwrap().get(), 1_301.0);
    assert_eq!(quote.high().unwrap().get(), 1_310.0);
    assert_eq!(quote.low().unwrap().get(), 1_295.0);
    assert_eq!(quote.change_percent().unwrap().get(), 0.62);
    assert_eq!(quote.volume().get(), 100.0);
    assert_eq!(quote.amount().unwrap().get(), 130_800.0);
    assert_eq!(quote.status(), DataStatus::Available);
    assert_eq!(quote.source_at(), Some("2026-07-22T10:00:00+08:00"));
    assert_eq!(quote.observed_at(), "2026-07-22T10:00:01+08:00");
    assert_eq!(quote.provider(), ProviderId::Tdx);
    assert_eq!(quote.batch_id(), "quote-batch");

    let bar = Bar::new(
        instrument.clone(),
        BarInterval::Day,
        "2026-07-22",
        "2026-07-22",
        Price::new(1_301.0).unwrap(),
        Price::new(1_310.0).unwrap(),
        Price::new(1_295.0).unwrap(),
        Price::new(1_308.0).unwrap(),
        Quantity::new(100.0).unwrap(),
        Some(Money::new(130_800.0).unwrap()),
        Adjustment::Forward,
        ProviderId::Tdx,
        "bar-batch",
    )
    .unwrap()
    .with_source_at("2026-07-22T15:00:00+08:00")
    .unwrap();
    assert_eq!(bar.instrument(), &instrument);
    assert_eq!(bar.interval(), BarInterval::Day);
    assert_eq!(bar.bar_start(), "2026-07-22");
    assert_eq!(bar.bar_end(), "2026-07-22");
    assert_eq!(bar.open().get(), 1_301.0);
    assert_eq!(bar.high().get(), 1_310.0);
    assert_eq!(bar.low().get(), 1_295.0);
    assert_eq!(bar.close().get(), 1_308.0);
    assert_eq!(bar.volume().get(), 100.0);
    assert_eq!(bar.amount().unwrap().get(), 130_800.0);
    assert_eq!(bar.adjustment(), Adjustment::Forward);
    assert_eq!(bar.source_at(), Some("2026-07-22T15:00:00+08:00"));
    assert_eq!(bar.provider(), ProviderId::Tdx);
    assert_eq!(bar.batch_id(), "bar-batch");
    assert_eq!(
        serde_json::from_value::<Bar>(serde_json::to_value(&bar).unwrap()).unwrap(),
        bar
    );
}

#[test]
fn auction_contract_preserves_missing_fields_and_evidence() {
    let snapshot = AuctionSnapshot::new(
        magic_market_core::InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            "600519",
            magic_market_core::AssetClass::Equity,
        )
        .unwrap(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        DataStatus::Unavailable,
        None,
        "observed",
        ProviderId::Eastmoney,
        "batch-1",
    )
    .unwrap();
    assert_eq!(snapshot.status(), DataStatus::Unavailable);
    assert!(snapshot.unmatched_bid_quantity().is_none());
    assert_eq!(snapshot.batch_id(), "batch-1");
}
