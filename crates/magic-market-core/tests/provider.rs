use magic_market_core::{AssetClass, BarInterval, BarsRequest, Exchange, InstrumentId};
#[test]
fn request_rejects_zero_limit() {
    let id = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    assert!(BarsRequest::new(id, BarInterval::Day, 0).is_err());
}
