use magic_market_core::{AssetClass, BarsRequest, Exchange, HistoricalBars, InstrumentId};
use magic_tdx_rs::{TdxDirectClient, TdxHqClient, TdxSmartClient};
#[test]
fn tdx_client_implements_core_bars_contract() {
    fn accepts<P: HistoricalBars<Bar = magic_tdx_rs::SecurityBar, Error = magic_tdx_rs::TdxError>>(_: &P) {}
    let _ = BarsRequest::new(InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(), magic_market_core::BarInterval::Day, 1).unwrap();
    accepts(&TdxHqClient::new());
    accepts(&TdxSmartClient::new());
    accepts(&TdxDirectClient::new("127.0.0.1", 7709, 1.0));
}
