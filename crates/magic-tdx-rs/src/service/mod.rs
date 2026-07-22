//! Stable service facade over TDX clients.
use crate::protocol::types::{FinanceInfo, MinuteTimePrice, SecurityInfo, TickData, XdXrInfo};
use crate::{SecurityBar, SecurityQuote, TdxError, TdxSmartClient};
use magic_market_core::{BarsRequest, DataBatch, HistoricalBars, InstrumentId, RealtimeQuotes};

/// High-level TDX service using SmartClient failover semantics.
pub struct TdxService {
    client: TdxSmartClient,
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
