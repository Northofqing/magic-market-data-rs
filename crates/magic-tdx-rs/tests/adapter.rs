use magic_market_core::{AssetClass, AsyncHistoricalBars, AsyncRealtimeQuotes, BarsRequest, Exchange, HistoricalBars, InstrumentId};
use magic_tdx_rs::{TdxDirectClient, TdxHqClient, TdxSmartClient};
#[test]
fn tdx_client_implements_core_bars_contract() {
    fn accepts<
        P: HistoricalBars<Bar = magic_tdx_rs::SecurityBar, Error = magic_tdx_rs::TdxError>,
    >(
        _: &P,
    ) {
    }
    let _ = BarsRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        magic_market_core::BarInterval::Day,
        1,
    )
    .unwrap();
    accepts(&TdxHqClient::new());
    accepts(&TdxSmartClient::new());
    accepts(&TdxDirectClient::new("127.0.0.1", 7709, 1.0));
}

#[test]
fn async_tdx_client_implements_core_contracts() {
    fn accepts_bars<P: AsyncHistoricalBars<Bar = magic_tdx_rs::SecurityBar, Error = magic_tdx_rs::TdxError>>(_: &P) {}
    fn accepts_quotes<P: AsyncRealtimeQuotes<Quote = magic_tdx_rs::SecurityQuote, Error = magic_tdx_rs::TdxError>>(_: &P) {}
    let client = magic_tdx_rs::AsyncTdxHqClient::new();
    accepts_bars(&client);
    accepts_quotes(&client);
}
