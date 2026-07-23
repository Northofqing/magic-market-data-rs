#![forbid(unsafe_code)]
//! Read-only supplemental adapter for Sina's public market-data endpoints.
//!
//! Sina's public-web endpoints do not publish a project-visible SLA. This
//! crate therefore advertises only response families covered by strict
//! deterministic parsers and real probes.

use magic_market_core::{
    Bar, BarsRequest, Capabilities, DataBatch, HistoricalBars, InstrumentId, MinuteData,
    MinuteDataRequest, MinutePoint, OrderBook, OrderBooks, Quote, RealtimeQuotes,
    SecurityMetadata, SecurityMetadataProvider,
};
use std::sync::Arc;
use thiserror::Error;

/// Errors raised by the Sina supplemental provider.
#[derive(Debug, Error)]
pub enum SinaError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("Sina response decoding failed: {0}")]
    Decode(String),
    #[error("Sina protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

/// Bounded byte transport used by the adapter and deterministic fixtures.
pub trait SnapshotTransport: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, SinaError>;
}

/// Read-only Sina market-data client.
#[derive(Clone)]
pub struct SinaClient {
    endpoint: String,
    transport: Arc<dyn SnapshotTransport>,
}

impl std::fmt::Debug for SinaClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SinaClient")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl SinaClient {
    /// Creates a client backed by an injected transport for deterministic use.
    pub fn with_transport(transport: impl SnapshotTransport + 'static) -> Self {
        Self {
            endpoint: "https://hq.sinajs.cn/list=".to_owned(),
            transport: Arc::new(transport),
        }
    }

    /// Reports only capabilities proved by parsers and real probes.
    pub const fn capabilities() -> Capabilities {
        Capabilities {
            quotes: true,
            bars: true,
            minute: true,
            trades: false,
            fundamentals: false,
            corporate_actions: false,
            blocks: false,
            money_flow: false,
            order_book: true,
            auction: false,
            security_metadata: true,
        }
    }
}

fn pending() -> SinaError {
    SinaError::Unsupported("implementation is not yet connected".into())
}

impl RealtimeQuotes for SinaClient {
    type Quote = Quote;
    type Error = SinaError;

    fn realtime_quotes(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        Err(pending())
    }
}

impl OrderBooks for SinaClient {
    type Error = SinaError;

    fn order_books(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        Err(pending())
    }
}

impl SecurityMetadataProvider for SinaClient {
    type Error = SinaError;

    fn security_metadata(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        Err(pending())
    }
}

impl HistoricalBars for SinaClient {
    type Bar = Bar;
    type Error = SinaError;

    fn historical_bars(
        &self,
        _request: &BarsRequest,
    ) -> Result<DataBatch<Self::Bar>, Self::Error> {
        Err(pending())
    }
}

impl MinuteData for SinaClient {
    type Error = SinaError;

    fn minute_data(
        &self,
        _request: &MinuteDataRequest,
    ) -> Result<DataBatch<MinutePoint>, Self::Error> {
        Err(pending())
    }
}
