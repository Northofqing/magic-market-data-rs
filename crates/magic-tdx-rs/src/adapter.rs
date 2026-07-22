use crate::error::TdxError;
use crate::{SecurityBar, SecurityQuote, TdxHqClient};
use magic_market_core::{
    AsyncHistoricalBars, AsyncRealtimeQuotes, BarInterval, BarsRequest, DataBatch, HistoricalBars,
    InstrumentId, RealtimeQuotes,
};

impl TdxHqClient {
    /// Returns the data families exposed through the core provider boundary.
    pub const fn capabilities() -> magic_market_core::Capabilities {
        magic_market_core::Capabilities {
            quotes: true,
            bars: true,
            minute: true,
            trades: true,
            fundamentals: true,
            corporate_actions: true,
            blocks: true,
            money_flow: false,
            order_book: false,
            auction: false,
        }
    }
}

fn market(id: &InstrumentId) -> u8 {
    match id.exchange() {
        magic_market_core::Exchange::Shanghai => 1,
        magic_market_core::Exchange::Shenzhen => 0,
    }
}
fn category(interval: BarInterval) -> u8 {
    match interval {
        BarInterval::Minute1 => 7,
        BarInterval::Minute5 => 0,
        BarInterval::Minute15 => 1,
        BarInterval::Minute30 => 2,
        BarInterval::Hour1 => 3,
        BarInterval::Day => 4,
        BarInterval::Week => 5,
        BarInterval::Month => 6,
        BarInterval::Year => 6,
    }
}
fn ensure_nonempty<T>(records: &[T]) -> Result<(), TdxError> {
    if records.is_empty() {
        Err(TdxError::InvalidData(
            "TDX returned an empty successful response".into(),
        ))
    } else {
        Ok(())
    }
}
fn fetched_at() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or_else(|_| "unknown".into(), |d| d.as_secs().to_string())
}
fn bars_provenance(source: &str, records: &[SecurityBar]) -> magic_market_core::Provenance {
    let p = magic_market_core::Provenance::new(source, fetched_at());
    match records.first() {
        Some(bar) => p.with_source_at(bar.datetime.clone()),
        None => p,
    }
}
fn quotes_provenance(source: &str, records: &[SecurityQuote]) -> magic_market_core::Provenance {
    let p = magic_market_core::Provenance::new(source, fetched_at());
    match records.first() {
        Some(quote) => p.with_source_at(quote.servertime.clone()),
        None => p,
    }
}
fn strict_bars(
    source: &str,
    records: Vec<SecurityBar>,
) -> Result<DataBatch<SecurityBar>, TdxError> {
    ensure_nonempty(&records)?;
    let provenance = bars_provenance(source, &records);
    Ok(DataBatch::strict(records, provenance))
}
fn strict_quotes(
    source: &str,
    records: Vec<SecurityQuote>,
) -> Result<DataBatch<SecurityQuote>, TdxError> {
    ensure_nonempty(&records)?;
    let provenance = quotes_provenance(source, &records);
    Ok(DataBatch::strict(records, provenance))
}

impl HistoricalBars for TdxHqClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        let records = self.get_security_bars(
            category(request.interval),
            market(&request.instrument),
            request.instrument.code(),
            0,
            request.limit,
            0,
        )?;
        strict_bars("tdx", records)
    }
}

impl RealtimeQuotes for TdxHqClient {
    type Quote = SecurityQuote;
    type Error = TdxError;
    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let pairs: Vec<(u8, &str)> = instruments
            .iter()
            .map(|id| (market(id), id.code()))
            .collect();
        let records = self.get_security_quotes(&pairs)?;
        strict_quotes("tdx", records)
    }
}

impl HistoricalBars for crate::TdxSmartClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        let records = self.get_security_bars(
            category(request.interval),
            market(&request.instrument),
            request.instrument.code(),
            0,
            request.limit,
            0,
        )?;
        strict_bars("tdx-smart", records)
    }
}

impl RealtimeQuotes for crate::TdxSmartClient {
    type Quote = SecurityQuote;
    type Error = TdxError;
    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let pairs: Vec<(u8, &str)> = instruments
            .iter()
            .map(|id| (market(id), id.code()))
            .collect();
        let records = self.get_security_quotes(&pairs)?;
        strict_quotes("tdx-smart", records)
    }
}

impl HistoricalBars for crate::TdxDirectClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        let records = self.get_security_bars(
            category(request.interval),
            market(&request.instrument),
            request.instrument.code(),
            0,
            request.limit,
            0,
        )?;
        strict_bars("tdx-direct", records)
    }
}

impl RealtimeQuotes for crate::TdxDirectClient {
    type Quote = SecurityQuote;
    type Error = TdxError;
    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let pairs: Vec<(u8, &str)> = instruments
            .iter()
            .map(|id| (market(id), id.code()))
            .collect();
        let records = self.get_security_quotes(&pairs)?;
        strict_quotes("tdx-direct", records)
    }
}

impl AsyncHistoricalBars for crate::AsyncTdxHqClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    async fn historical_bars_async(
        &self,
        request: &BarsRequest,
    ) -> Result<DataBatch<Self::Bar>, Self::Error> {
        let records = self
            .get_security_bars(
                category(request.interval),
                market(&request.instrument),
                request.instrument.code(),
                0,
                request.limit,
                0,
            )
            .await?;
        strict_bars("tdx-async", records)
    }
}

impl AsyncRealtimeQuotes for crate::AsyncTdxHqClient {
    type Quote = SecurityQuote;
    type Error = TdxError;
    async fn realtime_quotes_async(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let pairs: Vec<(u8, &str)> = instruments
            .iter()
            .map(|id| (market(id), id.code()))
            .collect();
        let records = self.get_security_quotes(&pairs).await?;
        strict_quotes("tdx-async", records)
    }
}
