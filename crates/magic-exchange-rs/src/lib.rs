#![forbid(unsafe_code)]
//! Bounded read-only adapters for official exchange data.

mod sse;
mod szse;
mod transport;

pub use sse::{SseClient, SseConfig};
pub use szse::{SzseClient, SzseConfig};
pub use transport::{ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, MAX_RESPONSE_BYTES};

use magic_market_core::{ContentCapabilities, ProviderId};
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
    pub content: ContentCapabilities,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HkexClient;

impl HkexClient {
    pub const fn provider_id() -> ProviderId {
        ProviderId::Hkex
    }

    pub const fn capabilities() -> ProviderCapabilities {
        ProviderCapabilities {
            provider: ProviderId::Hkex,
            content: ContentCapabilities {
                instrument_news: false,
                global_news: false,
                announcements: false,
                investor_questions: false,
            },
        }
    }
}
