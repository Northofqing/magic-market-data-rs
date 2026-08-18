#![forbid(unsafe_code)]
//! Bounded read-only adapters for official exchange data.

mod cffex;
mod dragon_tiger;
mod hkex;
mod sse;
mod szse;
mod szse_quote;
mod transport;

pub use cffex::{
    CffexAccessMode, CffexClient, CffexConfig, CffexTlsBackend,
    CFFEX_2026_FUTURES_DELIVERY_ADMITTED,
};
pub use dragon_tiger::{
    parse_sse_response, parse_szse_detail_response, parse_szse_list_response,
    DragonTigerParseError, OfficialDragonTigerRequest, ParsedDragonTiger, SzseDragonTigerDetailKey,
    SzseDragonTigerListItem, SzseDragonTigerListPage, MAX_DRAGON_TIGER_RESPONSE_BYTES,
};
pub use hkex::{HkexClient, HkexConfig};
pub use sse::{SseClient, SseConfig};
pub use szse::{SzseClient, SzseConfig};
pub use transport::{
    ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, TlsBackend, MAX_RESPONSE_BYTES,
};

use magic_market_core::{
    Capabilities, CapitalCapabilities, ContentCapabilities, ProviderId, SignalCapabilities,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExchangeError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("authentication or anti-bot rejection: HTTP {0}")]
    Authentication(u16),
    #[error("rate limited: HTTP 429")]
    RateLimited,
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("HTTPS TLS error using {backend:?}: {message}")]
    Tls {
        backend: TlsBackend,
        message: String,
    },
    #[error("unexpected HTTP status {0}")]
    HttpStatus(u16),
    #[error("official exchange response decoding failed: {0}")]
    Decode(String),
    #[error("official exchange schema drift: {0}")]
    Schema(String),
    #[error("official exchange paginated response is incomplete: {0}")]
    Incomplete(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub provider: ProviderId,
    pub market: Capabilities,
    pub content: ContentCapabilities,
    pub capital: CapitalCapabilities,
    pub signals: SignalCapabilities,
}
