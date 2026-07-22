//! Stable service facade over TDX clients.
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
}
impl Default for TdxService {
    fn default() -> Self {
        Self::new()
    }
}
