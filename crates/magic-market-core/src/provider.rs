use crate::{DataBatch, InstrumentId};

/// Provider identity used in provenance and capability reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId {
    Tdx,
    Tencent,
    Eastmoney,
    Sina,
    Baostock,
    /// Read-only data exposed by an authorized local terminal/SDK.
    LocalTerminal,
    Custom,
}

/// Normalized realtime quote contract for cross-provider consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub instrument: InstrumentId,
    pub price: crate::Price,
    pub volume: crate::Quantity,
    pub amount: Option<crate::Money>,
    /// Timestamp supplied by the source, when the packet proves one.
    pub source_at: Option<String>,
    /// Local observation timestamp, kept separate from `source_at`.
    pub observed_at: String,
    pub provider: ProviderId,
    pub batch_id: String,
}
impl Quote {
    /// Creates a quote with explicit observation evidence.
    pub fn new(
        instrument: InstrumentId,
        price: crate::Price,
        volume: crate::Quantity,
        amount: Option<crate::Money>,
        observed_at: impl Into<String>,
        provider: ProviderId,
        batch_id: impl Into<String>,
    ) -> Self {
        Self {
            instrument,
            price,
            volume,
            amount,
            source_at: None,
            observed_at: observed_at.into(),
            provider,
            batch_id: batch_id.into(),
        }
    }
    pub fn with_source_at(mut self, source_at: impl Into<String>) -> Self {
        self.source_at = Some(source_at.into());
        self
    }
}

/// Availability state for optional source fields; absence is never encoded as zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataStatus {
    Available,
    Unavailable,
    Stale,
    Conflicted,
    Unsupported,
}

/// Normalized money-flow snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct MoneyFlow {
    pub instrument: InstrumentId,
    pub main_net: Option<crate::Money>,
    pub super_large_net: Option<crate::Money>,
    pub large_net: Option<crate::Money>,
    pub medium_net: Option<crate::Money>,
    pub small_net: Option<crate::Money>,
    pub status: DataStatus,
    pub source_at: Option<String>,
    pub observed_at: String,
    pub provider: ProviderId,
    pub batch_id: String,
}

/// One level of a normalized five-level order book.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BookLevel {
    pub price: Option<crate::Price>,
    pub quantity: Option<crate::Quantity>,
}

/// Normalized order-book snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBook {
    pub instrument: InstrumentId,
    pub bids: [BookLevel; 5],
    pub asks: [BookLevel; 5],
    pub status: DataStatus,
}

/// Normalized call-auction snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct AuctionSnapshot {
    pub instrument: InstrumentId,
    pub matched_price: Option<crate::Price>,
    pub matched_quantity: Option<crate::Quantity>,
    pub status: DataStatus,
}

/// Declares which data families a provider implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub quotes: bool,
    pub bars: bool,
    pub minute: bool,
    pub trades: bool,
    pub fundamentals: bool,
    pub corporate_actions: bool,
    pub blocks: bool,
    pub money_flow: bool,
    pub order_book: bool,
    pub auction: bool,
}
impl Capabilities {
    pub const fn new() -> Self {
        Self {
            quotes: false,
            bars: false,
            minute: false,
            trades: false,
            fundamentals: false,
            corporate_actions: false,
            blocks: false,
            money_flow: false,
            order_book: false,
            auction: false,
        }
    }
}
impl Default for Capabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard bar interval shared by market-data providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarInterval {
    Minute1,
    Minute5,
    Minute15,
    Minute30,
    Hour1,
    Day,
    Week,
    Month,
    Year,
}

/// Price adjustment applied by the source to a historical bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adjustment {
    Unadjusted,
    Forward,
    Backward,
}

/// Provider-neutral OHLCV bar with record-level source evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub instrument: InstrumentId,
    pub interval: BarInterval,
    pub bar_start: String,
    pub bar_end: String,
    pub open: crate::Price,
    pub high: crate::Price,
    pub low: crate::Price,
    pub close: crate::Price,
    pub volume: crate::Quantity,
    pub amount: Option<crate::Money>,
    pub adjustment: Adjustment,
    pub source_at: Option<String>,
    pub provider: ProviderId,
    pub batch_id: String,
}
impl Bar {
    /// Builds a bar and rejects inconsistent OHLC ranges.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instrument: InstrumentId,
        interval: BarInterval,
        bar_start: impl Into<String>,
        bar_end: impl Into<String>,
        open: crate::Price,
        high: crate::Price,
        low: crate::Price,
        close: crate::Price,
        volume: crate::Quantity,
        amount: Option<crate::Money>,
        adjustment: Adjustment,
        provider: ProviderId,
        batch_id: impl Into<String>,
    ) -> Result<Self, crate::CoreError> {
        let bar_start = bar_start.into();
        let bar_end = bar_end.into();
        if bar_start.is_empty() || bar_end.is_empty() || bar_start > bar_end {
            return Err(crate::CoreError::InvalidRequest(
                "invalid bar time range".into(),
            ));
        }
        if low.get() > open.get().min(close.get())
            || high.get() < open.get().max(close.get())
            || low.get() > high.get()
        {
            return Err(crate::CoreError::InvalidRequest(
                "inconsistent OHLC range".into(),
            ));
        }
        Ok(Self {
            instrument,
            interval,
            bar_start,
            bar_end,
            open,
            high,
            low,
            close,
            volume,
            amount,
            adjustment,
            source_at: None,
            provider,
            batch_id: batch_id.into(),
        })
    }

    pub fn with_source_at(mut self, source_at: impl Into<String>) -> Self {
        self.source_at = Some(source_at.into());
        self
    }
}

/// Validated historical-bar request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarsRequest {
    pub instrument: InstrumentId,
    pub interval: BarInterval,
    pub start: Option<String>,
    pub end: Option<String>,
    pub limit: u16,
}
impl BarsRequest {
    /// Creates a bounded request.
    pub fn new(
        instrument: InstrumentId,
        interval: BarInterval,
        limit: u16,
    ) -> Result<Self, crate::CoreError> {
        if limit == 0 {
            return Err(crate::CoreError::InvalidValue {
                field: "limit",
                value: limit.to_string(),
                reason: "must be positive",
            });
        }
        Ok(Self {
            instrument,
            interval,
            start: None,
            end: None,
            limit,
        })
    }
    /// Adds an inclusive ISO date range after validating ordering and format.
    pub fn with_range(
        mut self,
        start: impl Into<String>,
        end: impl Into<String>,
    ) -> Result<Self, crate::CoreError> {
        let start = start.into();
        let end = end.into();
        let valid = |s: &str| {
            if s.len() != 10
                || s.as_bytes()[4] != b'-'
                || s.as_bytes()[7] != b'-'
                || !s
                    .bytes()
                    .enumerate()
                    .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
            {
                return false;
            }
            let year: u32 = s[0..4].parse().unwrap_or(0);
            let month: u32 = s[5..7].parse().unwrap_or(0);
            let day: u32 = s[8..10].parse().unwrap_or(0);
            let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
            let max_day = match month {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 if leap => 29,
                2 => 28,
                _ => 0,
            };
            max_day != 0 && day >= 1 && day <= max_day
        };
        if !valid(&start) || !valid(&end) || start > end {
            return Err(crate::CoreError::InvalidRequest(
                "invalid date range".into(),
            ));
        }
        self.start = Some(start);
        self.end = Some(end);
        Ok(self)
    }
}

/// Provider capability for historical bars.
pub trait HistoricalBars {
    type Bar;
    type Error: std::error::Error + Send + Sync + 'static;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error>;
}

/// Provider capability for realtime quotes.
pub trait RealtimeQuotes {
    type Quote;
    type Error: std::error::Error + Send + Sync + 'static;
    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error>;
}

/// Provider capability for money-flow snapshots.
pub trait MoneyFlows {
    type Error: std::error::Error + Send + Sync + 'static;
    fn money_flows(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<MoneyFlow>, Self::Error>;
}

/// Provider capability for order books.
pub trait OrderBooks {
    type Error: std::error::Error + Send + Sync + 'static;
    fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error>;
}

/// Provider capability for call-auction snapshots.
pub trait Auctions {
    type Error: std::error::Error + Send + Sync + 'static;
    fn auction_snapshots(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<AuctionSnapshot>, Self::Error>;
}

/// Async provider capability for historical bars.
#[allow(async_fn_in_trait)]
pub trait AsyncHistoricalBars {
    type Bar;
    type Error: std::error::Error + Send + Sync + 'static;
    async fn historical_bars_async(
        &self,
        request: &BarsRequest,
    ) -> Result<DataBatch<Self::Bar>, Self::Error>;
}

/// Async provider capability for realtime quotes.
#[allow(async_fn_in_trait)]
pub trait AsyncRealtimeQuotes {
    type Quote;
    type Error: std::error::Error + Send + Sync + 'static;
    async fn realtime_quotes_async(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error>;
}
