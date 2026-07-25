//! Stable service facade over TDX clients.
pub mod blocks;
pub mod finance;
pub mod funds;
pub mod profile;
use crate::adapter::{normalize_order_books, order_book_pairs};
use crate::protocol::types::{FinanceInfo, MinuteTimePrice, SecurityInfo, TickData, XdXrInfo};
use crate::{AsyncTdxHqClient, SecurityBar, SecurityQuote, TdxError, TdxSmartClient};
pub use blocks::BlockService;
pub use finance::FinanceService;
pub use funds::FundService;
use magic_market_core::{
    AuctionSnapshot, BarsRequest, DataBatch, HistoricalBars, InstrumentId, MoneyFlow, OrderBook,
    Quote, RealtimeQuotes, SecurityMetadata, SecurityMetadataProvider, Trade, Trades,
    TradesRequest,
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
        normalize_order_books("tdx-async", instruments, quotes)
    }
    /// Fetches the server-declared number of securities.
    pub async fn security_count(&self, market: u8) -> Result<u16, TdxError> {
        self.client.get_security_count(market).await
    }
    /// Fetches one security-list page.
    pub async fn security_list(
        &self,
        market: u8,
        start: u16,
    ) -> Result<Vec<SecurityInfo>, TdxError> {
        self.client.get_security_list(market, start).await
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
        self.client.get_minute_time_data(market, code).await
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
    }
    /// Fetches decoded finance fields.
    pub async fn finance(&self, market: u8, code: &str) -> Result<FinanceInfo, TdxError> {
        self.client.get_finance_info(market, code).await
    }
    /// Fetches corporate-action history.
    pub async fn corporate_actions(
        &self,
        market: u8,
        code: &str,
    ) -> Result<Vec<XdXrInfo>, TdxError> {
        self.client.get_xdxr_info(market, code).await
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
        if instruments.is_empty() {
            return Err(TdxError::InvalidData("quote request is empty".into()));
        }
        let mut records = Vec::with_capacity(instruments.len());
        for chunk in instruments.chunks(60) {
            let pairs: Vec<(u8, &str)> = chunk
                .iter()
                .map(|id| market(id).map(|market| (market, id.code())))
                .collect::<Result<_, _>>()?;
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
