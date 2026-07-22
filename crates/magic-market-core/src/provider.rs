use crate::{DataBatch, InstrumentId};

/// Provider identity used in provenance and capability reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId { Tdx, Tencent, Eastmoney, Sina, Baostock, Custom }

/// Normalized realtime quote contract for cross-provider consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct Quote { pub instrument: InstrumentId, pub price: crate::Price, pub volume: crate::Quantity, pub amount: Option<crate::Money> }

/// Declares which data families a provider implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities { pub quotes: bool, pub bars: bool, pub minute: bool, pub trades: bool, pub fundamentals: bool, pub corporate_actions: bool, pub blocks: bool }
impl Capabilities { pub const fn new() -> Self { Self { quotes: false, bars: false, minute: false, trades: false, fundamentals: false, corporate_actions: false, blocks: false } } }

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

/// Async provider capability for historical bars.
#[allow(async_fn_in_trait)]
pub trait AsyncHistoricalBars { type Bar; type Error: std::error::Error + Send + Sync + 'static; async fn historical_bars_async(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error>; }

/// Async provider capability for realtime quotes.
#[allow(async_fn_in_trait)]
pub trait AsyncRealtimeQuotes { type Quote; type Error: std::error::Error + Send + Sync + 'static; async fn realtime_quotes_async(&self, instruments: &[InstrumentId]) -> Result<DataBatch<Self::Quote>, Self::Error>; }
