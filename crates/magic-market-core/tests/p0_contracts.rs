use magic_market_core::{BookLevel, DataStatus, MoneyFlow, OrderBook};

#[test]
fn unavailable_fields_are_explicit() {
    assert_eq!(DataStatus::Unavailable, DataStatus::Unavailable);
    let level = BookLevel { price: None, quantity: None };
    assert!(level.price.is_none());
    let _flow = MoneyFlow {
        instrument: magic_market_core::InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            "600519",
            magic_market_core::AssetClass::Equity,
        ).unwrap(),
        main_net: None,
        super_large_net: None,
        large_net: None,
        medium_net: None,
        small_net: None,
        status: DataStatus::Unavailable,
    };
}

#[test]
fn order_book_has_fixed_five_level_shape() {
    let level = BookLevel { price: None, quantity: None };
    let book = OrderBook {
        instrument: magic_market_core::InstrumentId::new(
            magic_market_core::Exchange::Shenzhen,
            "000001",
            magic_market_core::AssetClass::Equity,
        ).unwrap(),
        bids: [level; 5],
        asks: [level; 5],
        status: DataStatus::Unsupported,
    };
    assert_eq!(book.bids.len(), 5);
    assert_eq!(book.asks.len(), 5);
}
