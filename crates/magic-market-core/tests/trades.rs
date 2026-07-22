use magic_market_core::{AssetClass, Exchange, InstrumentId, TradesRequest};

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap()
}

#[test]
fn trade_request_validates_limit_and_calendar_date() {
    assert!(TradesRequest::new(instrument(), 0).is_err());
    assert!(TradesRequest::new(instrument(), 20)
        .unwrap()
        .with_date("2026-02-29")
        .is_err());
    let request = TradesRequest::new(instrument(), 2_000)
        .unwrap()
        .with_date("2024-02-29")
        .unwrap();
    assert_eq!(request.date.as_deref(), Some("2024-02-29"));
    assert_eq!(request.limit, 2_000);
}
