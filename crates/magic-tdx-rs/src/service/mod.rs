//! Stable service facade over TDX clients.
pub mod blocks;
pub mod finance;
pub mod funds;
pub mod profile;
use crate::adapter::{order_book_pairs, ordered_order_book_quotes, BlockingTdxQuery};
use crate::protocol::types::{FinanceInfo, MinuteTimePrice, SecurityInfo, TickData, XdXrInfo};
use crate::{AsyncTdxHqClient, SecurityBar, SecurityQuote, TdxError, TdxSmartClient};
pub use blocks::BlockService;
pub use finance::FinanceService;
pub use funds::FundService;
use magic_market_core::{
    AuctionSnapshot, BarsRequest, BookLevel, DataBatch, DataStatus, HistoricalBars, InstrumentId,
    MoneyFlow, OrderBook, Price, Quantity, Quote, RealtimeQuotes, SecurityMetadata,
    SecurityMetadataProvider, Trade, Trades, TradesRequest,
};
pub use profile::ProfileService;
use std::collections::HashMap;

fn market(id: &InstrumentId) -> Result<u8, TdxError> {
    match id.exchange() {
        magic_market_core::Exchange::Shanghai => Ok(1),
        magic_market_core::Exchange::Shenzhen => Ok(0),
        magic_market_core::Exchange::Beijing => Err(TdxError::Unsupported(
            "beijing exchange: TDX market identifier is not verified".into(),
        )),
    }
}

fn fetched_epoch() -> Result<String, TdxError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .map_err(|error| {
            TdxError::InvalidData(format!("system clock is before UNIX epoch: {error}"))
        })
}

fn security_count_with(
    query: &impl BlockingTdxQuery,
    market: u8,
) -> Result<u16, TdxError> {
    query.security_count(market)
}

fn security_list_with(
    query: &impl BlockingTdxQuery,
    market: u8,
    start: u16,
) -> Result<Vec<SecurityInfo>, TdxError> {
    query.security_list(market, start)
}

fn security_list_all_with(
    query: &impl BlockingTdxQuery,
    market: u8,
) -> Result<Vec<SecurityInfo>, TdxError> {
    const PAGE_SIZE: u16 = 1000;
    let expected = usize::from(security_count_with(query, market)?);
    let mut all = Vec::with_capacity(expected);
    let mut start: u16 = 0;
    while all.len() < expected {
        let page = security_list_with(query, market, start)?;
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

fn quotes_chunked_with(
    query: &impl BlockingTdxQuery,
    instruments: &[InstrumentId],
) -> Result<DataBatch<Quote>, TdxError> {
    if instruments.is_empty() {
        return Err(TdxError::InvalidData("quote request is empty".into()));
    }
    let mut records = Vec::with_capacity(instruments.len());
    for chunk in instruments.chunks(60) {
        let pairs: Vec<(u8, &str)> = chunk
            .iter()
            .map(|id| market(id).map(|market| (market, id.code())))
            .collect::<Result<_, _>>()?;
        let page = query.security_quotes(&pairs)?;
        if page.len() != chunk.len() {
            return Err(TdxError::InvalidData(
                "TDX quote chunk cardinality mismatch".into(),
            ));
        }
        records.extend(page);
    }
    let mut expected = HashMap::<(u8, String), usize>::new();
    for id in instruments {
        *expected
            .entry((market(id)?, id.code().to_owned()))
            .or_default() += 1;
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
        ordered.push(
            by_key
                .get_mut(&(market(id)?, id.code().to_owned()))
                .and_then(|values| values.pop())
                .ok_or_else(|| TdxError::InvalidData("TDX quote ordering mismatch".into()))?,
        );
    }
    crate::adapter::normalize_quotes("tdx-smart-chunked", instruments, ordered)
}

fn minute_data_with(
    query: &impl BlockingTdxQuery,
    market: u8,
    code: &str,
) -> Result<Vec<MinuteTimePrice>, TdxError> {
    query.minute_time_data(market, code)
}

fn history_minute_data_with(
    query: &impl BlockingTdxQuery,
    market: u8,
    code: &str,
    date: u32,
) -> Result<Vec<MinuteTimePrice>, TdxError> {
    query.history_minute_time_data(market, code, date)
}

fn transactions_with(
    query: &impl BlockingTdxQuery,
    market: u8,
    code: &str,
    start: u16,
    count: u16,
) -> Result<Vec<TickData>, TdxError> {
    query.transaction_data(market, code, start, count)
}

fn history_transactions_with(
    query: &impl BlockingTdxQuery,
    market: u8,
    code: &str,
    start: u16,
    count: u16,
    date: u32,
) -> Result<Vec<TickData>, TdxError> {
    query.history_transaction_data(market, code, start, count, date)
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
    pub async fn quotes(&self, instruments: &[InstrumentId]) -> Result<DataBatch<Quote>, TdxError> {
        <AsyncTdxHqClient as magic_market_core::AsyncRealtimeQuotes>::realtime_quotes_async(
            &self.client,
            instruments,
        )
        .await
    }
    /// Fetches normalized current or historical trades with automatic paging.
    pub async fn trades(&self, request: &TradesRequest) -> Result<DataBatch<Trade>, TdxError> {
        <AsyncTdxHqClient as magic_market_core::AsyncTrades>::trades_async(&self.client, request)
            .await
    }
    /// Fetches and normalizes five-level books through the async connection pool.
    pub async fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, TdxError> {
        let pairs = order_book_pairs(instruments, "TDX async")?;
        let quotes = self.client.get_security_quotes(&pairs).await?;
        let ordered = ordered_order_book_quotes(instruments, quotes, "TDX async")?;
        let level = |price: f64, quantity: f64| -> Result<BookLevel, TdxError> {
            match (price, quantity) {
                (price, quantity) if !price.is_finite() || !quantity.is_finite() => Err(
                    TdxError::InvalidData("TDX order-book level must be finite".into()),
                ),
                (price, quantity) if price < 0.0 || quantity < 0.0 => Err(TdxError::InvalidData(
                    "TDX order-book level must be non-negative".into(),
                )),
                (0.0, _) => Ok(BookLevel::unavailable()),
                (price, quantity) => Ok(BookLevel::new(
                    Some(Price::new(price)?),
                    Some(Quantity::new(quantity)?),
                )?),
            }
        };
        let depth = |levels: &[BookLevel; 5]| -> Result<Option<Quantity>, TdxError> {
            let mut found = false;
            let total = levels.iter().filter_map(|level| level.quantity()).fold(
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
        let observed_at = fetched_epoch()?;
        let batch_id = format!("tdx-async:{observed_at}:order-book");
        let mut books = Vec::with_capacity(ordered.len());
        let mut issues = Vec::new();
        for (id, quote) in ordered {
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
            let levels_complete = bids
                .iter()
                .chain(&asks)
                .all(|level| level.price().is_some());
            if !levels_complete {
                issues.push(format!(
                    "{}: one or more normalized order-book fields unavailable",
                    id.code()
                ));
            }
            issues.push(format!(
                "{}: TDX order-book source timestamp format is unverified",
                id.code()
            ));
            books.push(OrderBook::new(
                id.clone(),
                bids,
                asks,
                total_bid_quantity,
                total_ask_quantity,
                DataStatus::Unavailable,
                None,
                observed_at.clone(),
                magic_market_core::ProviderId::Tdx,
                batch_id.clone(),
            )?);
        }
        let provenance = magic_market_core::Provenance::new("tdx-async", observed_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::best_effort(books, provenance, issues)?)
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
    pub fn quotes(&self, instruments: &[InstrumentId]) -> Result<DataBatch<Quote>, TdxError> {
        self.client.realtime_quotes(instruments)
    }
    /// Fetches normalized current or historical trades with automatic paging.
    pub fn trades(&self, request: &TradesRequest) -> Result<DataBatch<Trade>, TdxError> {
        self.client.trades(request)
    }
    /// Fetches best-effort source-backed security master data.
    pub fn security_metadata(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, TdxError> {
        self.client.security_metadata(instruments)
    }
    /// Fetches quotes in protocol-sized chunks and restores the requested order.
    /// Any failed or incomplete chunk aborts the whole operation.
    pub fn quotes_chunked(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Quote>, TdxError> {
        quotes_chunked_with(self.client.inner(), instruments)
    }
    /// TDX has no standardized auditable money-flow packet.
    pub fn money_flows(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<MoneyFlow>, TdxError> {
        Err(TdxError::Unsupported(
            "TDX quote/trade packets do not provide auditable main/net inflow fields or source \
             methodology required by MoneyFlow"
                .into(),
        ))
    }
    /// TDX has no standardized call-auction snapshot packet.
    pub fn auction_snapshots(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<AuctionSnapshot>, TdxError> {
        Err(TdxError::Unsupported(
            "TDX packets do not provide the standardized indicative price and matched/unmatched \
             quantities required by AuctionSnapshot"
                .into(),
        ))
    }
    /// Fetches a market security count.
    pub fn security_count(&self, market: u8) -> Result<u16, TdxError> {
        security_count_with(self.client.inner(), market)
    }
    /// Fetches one security-list page.
    pub fn security_list(&self, market: u8, start: u16) -> Result<Vec<SecurityInfo>, TdxError> {
        security_list_with(self.client.inner(), market, start)
    }
    /// Fetches the complete market list using the server-declared count.
    ///
    /// Pages are assembled atomically: a transport or cardinality mismatch
    /// returns an error rather than a silently truncated list.
    pub fn security_list_all(&self, market: u8) -> Result<Vec<SecurityInfo>, TdxError> {
        security_list_all_with(self.client.inner(), market)
    }
    /// Fetches current intraday data.
    pub fn minute_data(&self, market: u8, code: &str) -> Result<Vec<MinuteTimePrice>, TdxError> {
        minute_data_with(self.client.inner(), market, code)
    }
    /// Fetches minute data for an explicit historical date (YYYYMMDD).
    pub fn history_minute_data(
        &self,
        market: u8,
        code: &str,
        date: u32,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        history_minute_data_with(self.client.inner(), market, code, date)
    }
    /// Fetches current transaction data.
    pub fn transactions(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<Vec<TickData>, TdxError> {
        transactions_with(self.client.inner(), market, code, start, count)
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
        history_transactions_with(self.client.inner(), market, code, start, count, date)
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

#[cfg(test)]
#[path = "../../tests/internal/service.rs"]
mod tests;
