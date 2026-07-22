use magic_market_core::{AssetClass, BarInterval, BarsRequest, Exchange, InstrumentId};
#[test]
fn service_facade_constructs_without_network() {
    let service = magic_tdx_rs::TdxService::new();
    let id = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let request = BarsRequest::new(id, BarInterval::Day, 1).unwrap();
    assert_eq!(request.limit, 1);
    let _ = service.client();
}
