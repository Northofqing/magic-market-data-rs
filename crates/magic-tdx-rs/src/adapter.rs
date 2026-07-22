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
fn nonempty<T>(records: Vec<T>) -> Result<Vec<T>, TdxError> {
    if records.is_empty() {
        Err(TdxError::InvalidData(
            "TDX returned an empty successful response".into(),
        ))
    } else {
        Ok(records)
    }
}
fn fetched_at() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or_else(|_| "unknown".into(), |d| d.as_secs().to_string())
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
        Ok(DataBatch::strict(
            nonempty(records)?,
            magic_market_core::Provenance::new("tdx", fetched_at()),
        ))
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
        Ok(DataBatch::strict(
            nonempty(records)?,
            magic_market_core::Provenance::new("tdx", fetched_at()),
        ))
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
        Ok(DataBatch::strict(
            nonempty(records)?,
            magic_market_core::Provenance::new("tdx-smart", fetched_at()),
        ))
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
        Ok(DataBatch::strict(
            nonempty(records)?,
            magic_market_core::Provenance::new("tdx-smart", fetched_at()),
        ))
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
        Ok(DataBatch::strict(
            nonempty(records)?,
            magic_market_core::Provenance::new("tdx-direct", fetched_at()),
        ))
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
        Ok(DataBatch::strict(
            nonempty(records)?,
            magic_market_core::Provenance::new("tdx-direct", fetched_at()),
        ))
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
        Ok(DataBatch::strict(
            nonempty(records)?,
            magic_market_core::Provenance::new("tdx-async", fetched_at()),
        ))
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
        Ok(DataBatch::strict(
            nonempty(records)?,
            magic_market_core::Provenance::new("tdx-async", fetched_at()),
        ))
    }
}
