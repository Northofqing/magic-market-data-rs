use crate::{DataBatch, InstrumentId};

/// Standard bar interval shared by market-data providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarInterval { Minute1, Minute5, Minute15, Minute30, Hour1, Day, Week, Month, Year }

/// Validated historical-bar request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarsRequest { pub instrument: InstrumentId, pub interval: BarInterval, pub start: Option<String>, pub end: Option<String>, pub limit: u16 }
impl BarsRequest {
    /// Creates a bounded request.
    pub fn new(instrument: InstrumentId, interval: BarInterval, limit: u16) -> Result<Self, crate::CoreError> {
        if limit == 0 { return Err(crate::CoreError::InvalidValue { field: "limit", value: limit.to_string(), reason: "must be positive" }); }
        Ok(Self { instrument, interval, start: None, end: None, limit })
    }
}

/// Provider capability for historical bars.
pub trait HistoricalBars { type Bar; type Error: std::error::Error + Send + Sync + 'static; fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error>; }

/// Provider capability for realtime quotes.
pub trait RealtimeQuotes { type Quote; type Error: std::error::Error + Send + Sync + 'static; fn realtime_quotes(&self, instruments: &[InstrumentId]) -> Result<DataBatch<Self::Quote>, Self::Error>; }
