use magic_market_core::{BarInterval, BarsRequest, DataBatch, HistoricalBars, InstrumentId, RealtimeQuotes};
use crate::{SecurityBar, SecurityQuote, TdxHqClient};
use crate::error::TdxError;

fn market(id: &InstrumentId) -> u8 { match id.exchange() { magic_market_core::Exchange::Shanghai => 1, magic_market_core::Exchange::Shenzhen => 0 } }
fn category(interval: BarInterval) -> u8 { match interval { BarInterval::Minute1 => 7, BarInterval::Minute5 => 0, BarInterval::Minute15 => 1, BarInterval::Minute30 => 2, BarInterval::Hour1 => 3, BarInterval::Day => 4, BarInterval::Week => 5, BarInterval::Month => 6, BarInterval::Year => 6 } }

impl HistoricalBars for TdxHqClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        let records = self.get_security_bars(category(request.interval), market(&request.instrument), request.instrument.code(), 0, request.limit, 0)?;
        Ok(DataBatch::strict(records, magic_market_core::Provenance::new("tdx", "runtime")))
    }
}

impl RealtimeQuotes for TdxHqClient {
    type Quote = SecurityQuote;
    type Error = TdxError;
    fn realtime_quotes(&self, instruments: &[InstrumentId]) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let pairs: Vec<(u8, &str)> = instruments.iter().map(|id| (market(id), id.code())).collect();
        let records = self.get_security_quotes(&pairs)?;
        Ok(DataBatch::strict(records, magic_market_core::Provenance::new("tdx", "runtime")))
    }
}
