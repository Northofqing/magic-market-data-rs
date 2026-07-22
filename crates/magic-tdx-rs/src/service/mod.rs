//! Stable service facade over TDX clients.
pub mod blocks;
pub mod finance;
pub mod funds;
pub mod profile;
use crate::protocol::types::{FinanceInfo, MinuteTimePrice, SecurityInfo, TickData, XdXrInfo};
use crate::{AsyncTdxHqClient, SecurityBar, SecurityQuote, TdxError, TdxSmartClient};
pub use blocks::BlockService;
pub use finance::FinanceService;
pub use funds::FundService;
use magic_market_core::{
    AuctionSnapshot, BarsRequest, BookLevel, DataBatch, DataStatus, HistoricalBars, InstrumentId,
    MoneyFlow, OrderBook, Price, Quantity, RealtimeQuotes,
};
pub use profile::ProfileService;
use std::collections::HashMap;

fn fetched_epoch() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or_else(
            |_| "unknown".to_owned(),
            |value| value.as_secs().to_string(),
        )
}

/// High-level TDX service using SmartClient failover semantics.
pub struct TdxService {
    client: TdxSmartClient,
}

/// Async counterpart of [`TdxService`] for connection-pool concurrency.
pub struct AsyncTdxService {
    client: AsyncTdxHqClient,
}
impl AsyncTdxService {
    /// Creates an asynchronous service with the client's default pool.
    pub fn new() -> Self {
        Self {
            client: AsyncTdxHqClient::new(),
        }
    }
    /// Accesses the underlying asynchronous client.
    pub fn client(&self) -> &AsyncTdxHqClient {
        &self.client
    }
    /// Fetches strict historical bars concurrently through the async client.
    pub async fn bars(&self, request: &BarsRequest) -> Result<DataBatch<SecurityBar>, TdxError> {
        <AsyncTdxHqClient as magic_market_core::AsyncHistoricalBars>::historical_bars_async(
            &self.client,
            request,
        )
        .await
    }
    /// Fetches strict realtime quotes concurrently through the async client.
    pub async fn quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityQuote>, TdxError> {
        <AsyncTdxHqClient as magic_market_core::AsyncRealtimeQuotes>::realtime_quotes_async(
            &self.client,
            instruments,
        )
        .await
    }
    /// Fetches and normalizes five-level books through the async connection pool.
    pub async fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, TdxError> {
        let pairs: Vec<(u8, &str)> = instruments
            .iter()
            .map(|id| {
                let market = match id.exchange() {
                    magic_market_core::Exchange::Shanghai => 1,
                    magic_market_core::Exchange::Shenzhen => 0,
                };
                (market, id.code())
            })
            .collect();
        let quotes = self.client.get_security_quotes(&pairs).await?;
        if quotes.len() != instruments.len() {
            return Err(TdxError::InvalidData(
                "TDX async order-book cardinality mismatch".into(),
            ));
        }
        let level = |price: f64, quantity: f64| -> Result<BookLevel, TdxError> {
            let price = if price > 0.0 {
                Some(Price::new(price).map_err(|e| TdxError::InvalidData(e.to_string()))?)
            } else {
                None
            };
            let quantity = if quantity >= 0.0 {
                Some(Quantity::new(quantity).map_err(|e| TdxError::InvalidData(e.to_string()))?)
            } else {
                None
            };
            Ok(BookLevel { price, quantity })
        };
        let depth = |levels: &[BookLevel; 5]| -> Result<Option<Quantity>, TdxError> {
            let mut found = false;
            let total = levels.iter().filter_map(|level| level.quantity).fold(
                0.0,
                |accumulator, quantity| {
                    found = true;
                    accumulator + quantity.get()
                },
            );
            if found {
                Quantity::new(total)
                    .map(Some)
                    .map_err(|error| TdxError::InvalidData(error.to_string()))
            } else {
                Ok(None)
            }
        };
        let batch_source_at = quotes
            .iter()
            .find(|quote| !quote.servertime.is_empty())
            .map(|quote| quote.servertime.clone());
        let observed_at = fetched_epoch();
        let batch_id = format!("tdx-async:{observed_at}:order-book");
        let mut books = Vec::with_capacity(quotes.len());
        for (id, quote) in instruments.iter().zip(quotes) {
            let bids = [
                level(quote.bid1, quote.bid_vol1)?,
                level(quote.bid2, quote.bid_vol2)?,
                level(quote.bid3, quote.bid_vol3)?,
                level(quote.bid4, quote.bid_vol4)?,
                level(quote.bid5, quote.bid_vol5)?,
            ];
            let asks = [
                level(quote.ask1, quote.ask_vol1)?,
                level(quote.ask2, quote.ask_vol2)?,
                level(quote.ask3, quote.ask_vol3)?,
                level(quote.ask4, quote.ask_vol4)?,
                level(quote.ask5, quote.ask_vol5)?,
            ];
            let total_bid_quantity = depth(&bids)?;
            let total_ask_quantity = depth(&asks)?;
            books.push(OrderBook {
                instrument: id.clone(),
                bids,
                asks,
                total_bid_quantity,
                total_ask_quantity,
                status: DataStatus::Available,
                source_at: (!quote.servertime.is_empty()).then_some(quote.servertime),
                observed_at: observed_at.clone(),
                provider: magic_market_core::ProviderId::Tdx,
                batch_id: batch_id.clone(),
            });
        }
        let mut provenance =
            magic_market_core::Provenance::new("tdx-async", observed_at).with_batch_id(batch_id);
        if let Some(source_at) = batch_source_at {
            provenance = provenance.with_source_at(source_at);
        }
        Ok(DataBatch::strict(books, provenance))
    }
    /// Fetches the server-declared number of securities.
    pub async fn security_count(&self, market: u8) -> Result<u16, TdxError> {
        self.client
            .get_security_count(market)
            .await
            .map_err(Into::into)
    }
    /// Fetches one security-list page.
    pub async fn security_list(
        &self,
        market: u8,
        start: u16,
    ) -> Result<Vec<SecurityInfo>, TdxError> {
        self.client
            .get_security_list(market, start)
            .await
            .map_err(Into::into)
    }
    /// Fetches the complete market list atomically using the declared count.
    pub async fn security_list_all(&self, market: u8) -> Result<Vec<SecurityInfo>, TdxError> {
        const PAGE_SIZE: u16 = 1000;
        let expected = usize::from(self.security_count(market).await?);
        let mut all = Vec::with_capacity(expected);
        let mut start = 0u16;
        while all.len() < expected {
            let page = self.security_list(market, start).await?;
            if page.is_empty() || all.len() + page.len() > expected {
                return Err(TdxError::InvalidData(
                    "TDX security list cardinality mismatch".into(),
                ));
            }
            all.extend(page);
            if all.len() == expected {
                break;
            }
            start = start
                .checked_add(PAGE_SIZE)
                .ok_or_else(|| TdxError::InvalidData("TDX security list offset overflow".into()))?;
        }
        Ok(all)
    }
    /// Fetches current minute data.
    pub async fn minute_data(
        &self,
        market: u8,
        code: &str,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        self.client
            .get_minute_time_data(market, code)
            .await
            .map_err(Into::into)
    }
    /// Fetches minute data for an explicit historical date (YYYYMMDD).
    pub async fn history_minute_data(
        &self,
        market: u8,
        code: &str,
        date: u32,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        self.client
            .get_history_minute_time_data(market, code, date)
            .await
            .map_err(Into::into)
    }
    /// Fetches current transaction data.
    pub async fn transactions(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<Vec<TickData>, TdxError> {
        self.client
            .get_transaction_data(market, code, start, count)
            .await
            .map_err(Into::into)
    }
    /// Fetches historical transactions for an explicit date (YYYYMMDD).
    pub async fn history_transactions(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
        date: u32,
    ) -> Result<Vec<TickData>, TdxError> {
        self.client
            .get_history_transaction_data(market, code, start, count, date)
            .await
            .map_err(Into::into)
    }
    /// Fetches decoded finance fields.
    pub async fn finance(&self, market: u8, code: &str) -> Result<FinanceInfo, TdxError> {
        self.client
            .get_finance_info(market, code)
            .await
            .map_err(Into::into)
    }
    /// Fetches corporate-action history.
    pub async fn corporate_actions(
        &self,
        market: u8,
        code: &str,
    ) -> Result<Vec<XdXrInfo>, TdxError> {
        self.client
            .get_xdxr_info(market, code)
            .await
            .map_err(Into::into)
    }
}
impl Default for AsyncTdxService {
    fn default() -> Self {
        Self::new()
    }
}
impl TdxService {
    /// Creates a disconnected service.
    pub fn new() -> Self {
        Self {
            client: TdxSmartClient::new(),
        }
    }
    /// Accesses the underlying smart client for connection configuration.
    pub fn client(&self) -> &TdxSmartClient {
        &self.client
    }
    /// Fetches strict historical bars.
    pub fn bars(&self, request: &BarsRequest) -> Result<DataBatch<SecurityBar>, TdxError> {
        self.client.historical_bars(request)
    }
    /// Fetches strict realtime quotes.
    pub fn quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityQuote>, TdxError> {
        self.client.realtime_quotes(instruments)
    }
    /// Fetches quotes in protocol-sized chunks and restores the requested order.
    /// Any failed or incomplete chunk aborts the whole operation.
    pub fn quotes_chunked(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityQuote>, TdxError> {
        if instruments.is_empty() {
            return Err(TdxError::InvalidData("quote request is empty".into()));
        }
        let mut records = Vec::with_capacity(instruments.len());
        for chunk in instruments.chunks(60) {
            let pairs: Vec<(u8, &str)> = chunk
                .iter()
                .map(|id| {
                    let market = match id.exchange() {
                        magic_market_core::Exchange::Shanghai => 1,
                        magic_market_core::Exchange::Shenzhen => 0,
                    };
                    (market, id.code())
                })
                .collect();
            let page = self.client.inner().get_security_quotes(&pairs)?;
            if page.len() != chunk.len() {
                return Err(TdxError::InvalidData(
                    "TDX quote chunk cardinality mismatch".into(),
                ));
            }
            records.extend(page);
        }
        // TDX normally preserves order; this validation also prevents silently
        // returning a quote for a different instrument.
        let mut expected = HashMap::<(u8, String), usize>::new();
        for id in instruments {
            let market = match id.exchange() {
                magic_market_core::Exchange::Shanghai => 1,
                magic_market_core::Exchange::Shenzhen => 0,
            };
            *expected.entry((market, id.code().to_owned())).or_default() += 1;
        }
        for quote in &records {
            let key = (quote.market, quote.code.clone());
            let Some(remaining) = expected.get_mut(&key) else {
                return Err(TdxError::InvalidData(
                    "TDX returned an unexpected quote".into(),
                ));
            };
            if *remaining == 0 {
                return Err(TdxError::InvalidData(
                    "TDX returned a duplicate quote".into(),
                ));
            }
            *remaining -= 1;
        }
        if expected.values().any(|count| *count != 0) {
            return Err(TdxError::InvalidData(
                "TDX omitted a requested quote".into(),
            ));
        }
        let mut by_key: HashMap<(u8, String), Vec<SecurityQuote>> = HashMap::new();
        for quote in records {
            by_key
                .entry((quote.market, quote.code.clone()))
                .or_default()
                .push(quote);
        }
        let mut ordered = Vec::with_capacity(instruments.len());
        for id in instruments {
            let market = match id.exchange() {
                magic_market_core::Exchange::Shanghai => 1,
                magic_market_core::Exchange::Shenzhen => 0,
            };
            ordered.push(
                by_key
                    .get_mut(&(market, id.code().to_owned()))
                    .and_then(|values| values.pop())
                    .ok_or_else(|| TdxError::InvalidData("TDX quote ordering mismatch".into()))?,
            );
        }
        let records = ordered;
        let mut provenance = magic_market_core::Provenance::new("tdx-smart", fetched_epoch());
        if let Some(source_at) = records.first().map(|quote| quote.servertime.clone()) {
            provenance = provenance.with_source_at(source_at);
        }
        Ok(DataBatch::strict(records, provenance))
    }
    /// TDX has no standardized auditable money-flow packet.
    pub fn money_flows(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<MoneyFlow>, TdxError> {
        Err(TdxError::Unsupported("money_flow".into()))
    }
    /// TDX has no standardized call-auction snapshot packet.
    pub fn auction_snapshots(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<AuctionSnapshot>, TdxError> {
        Err(TdxError::Unsupported("auction".into()))
    }
    /// Fetches a market security count.
    pub fn security_count(&self, market: u8) -> Result<u16, TdxError> {
        self.client.inner().get_security_count(market)
    }
    /// Fetches one security-list page.
    pub fn security_list(&self, market: u8, start: u16) -> Result<Vec<SecurityInfo>, TdxError> {
        self.client.inner().get_security_list(market, start)
    }
    /// Fetches the complete market list using the server-declared count.
    ///
    /// Pages are assembled atomically: a transport or cardinality mismatch
    /// returns an error rather than a silently truncated list.
    pub fn security_list_all(&self, market: u8) -> Result<Vec<SecurityInfo>, TdxError> {
        const PAGE_SIZE: u16 = 1000;
        let expected = usize::from(self.security_count(market)?);
        let mut all = Vec::with_capacity(expected);
        let mut start: u16 = 0;
        while all.len() < expected {
            let page = self.security_list(market, start)?;
            if page.is_empty() {
                return Err(TdxError::InvalidData(
                    "TDX security list ended before declared count".into(),
                ));
            }
            all.extend(page);
            if all.len() > expected {
                return Err(TdxError::InvalidData(
                    "TDX security list exceeded declared count".into(),
                ));
            }
            if all.len() == expected {
                break;
            }
            start = start
                .checked_add(PAGE_SIZE)
                .ok_or_else(|| TdxError::InvalidData("TDX security list offset overflow".into()))?;
        }
        Ok(all)
    }
    /// Fetches current intraday data.
    pub fn minute_data(&self, market: u8, code: &str) -> Result<Vec<MinuteTimePrice>, TdxError> {
        self.client.inner().get_minute_time_data(market, code)
    }
    /// Fetches minute data for an explicit historical date (YYYYMMDD).
    pub fn history_minute_data(
        &self,
        market: u8,
        code: &str,
        date: u32,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        self.client
            .inner()
            .get_history_minute_time_data(market, code, date)
    }
    /// Fetches current transaction data.
    pub fn transactions(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<Vec<TickData>, TdxError> {
        self.client
            .inner()
            .get_transaction_data(market, code, start, count)
    }
    /// Fetches historical transactions for an explicit date (YYYYMMDD).
    pub fn history_transactions(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
        date: u32,
    ) -> Result<Vec<TickData>, TdxError> {
        self.client
            .inner()
            .get_history_transaction_data(market, code, start, count, date)
    }
    /// Fetches decoded finance fields.
    pub fn finance(&self, market: u8, code: &str) -> Result<FinanceInfo, TdxError> {
        self.client.inner().get_finance_info(market, code)
    }
    /// Fetches corporate-action history.
    pub fn corporate_actions(&self, market: u8, code: &str) -> Result<Vec<XdXrInfo>, TdxError> {
        self.client.inner().get_xdxr_info(market, code)
    }
}
impl Default for TdxService {
    fn default() -> Self {
        Self::new()
    }
}
