use magic_market_core::{AssetClass, BarInterval, BarsRequest, Exchange, InstrumentId};
#[test]
fn request_rejects_zero_limit() {
    let id = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    assert!(BarsRequest::new(id, BarInterval::Day, 0).is_err());
}

#[test]
fn request_validates_date_range() {
    let id = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let req = BarsRequest::new(id, BarInterval::Day, 1).unwrap();
    assert!(req.clone().with_range("2026-07-22", "2026-07-21").is_err());
    assert!(req.clone().with_range("2026-07-21", "2026-07-22").is_ok());
    assert!(req.with_range("2026-99-01", "2026-99-02").is_err());
}
