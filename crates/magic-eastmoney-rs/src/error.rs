use thiserror::Error;

/// Explicit failures raised by the Eastmoney public-web adapter.
#[derive(Debug, Error)]
pub enum EastmoneyError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("response exceeds the {limit} byte limit")]
    ResponseTooLarge { limit: usize },
    #[error("Eastmoney response decoding failed: {0}")]
    Decode(String),
    #[error("Eastmoney protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("verified empty result: {0}")]
    VerifiedEmpty(Box<magic_market_core::VerifiedEmpty>),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

impl EastmoneyError {
    /// Stable diagnostic category for probes and operational metrics.
    pub const fn category(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::Transport(_) => "transport",
            Self::ResponseTooLarge { .. } => "response_too_large",
            Self::Decode(_) => "decode",
            Self::Protocol(_) => "protocol",
            Self::Unsupported(_) => "unsupported",
            Self::VerifiedEmpty(_) => "verified_empty",
            Self::Core(_) => "core",
        }
    }
}
