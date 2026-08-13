#![forbid(unsafe_code)]

mod parser;
mod transport;

use magic_market_core::{
    CompanyFiling, CompanyFilingRequest, CompanyFilingsProvider, DataBatch, FilingCapabilities,
};
use magic_market_transport::{HttpTransport, RequestGate};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub const FILING_METADATA_ADMITTED: bool = true;
const REQUEST_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Error)]
pub enum SecEdgarError {
    #[error("invalid SEC request: {0}")]
    InvalidRequest(String),
    #[error("SEC authentication/identification failed: {0}")]
    Authentication(String),
    #[error(transparent)]
    Transport(#[from] magic_market_transport::TransportError),
    #[error("SEC response decoding failed: {0}")]
    Decode(String),
    #[error("SEC submissions protocol failed: {0}")]
    Protocol(String),
    #[error("unsupported SEC capability: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Clone)]
struct SecUserAgent(String);

impl SecUserAgent {
    fn new(value: impl Into<String>) -> Result<Self, SecEdgarError> {
        let value = value.into();
        let value = value.trim();
        if !(10..=256).contains(&value.len()) || value.chars().any(char::is_control) {
            return Err(SecEdgarError::InvalidRequest(
                "SEC User-Agent must contain 10 through 256 non-control characters".into(),
            ));
        }
        let has_application = value.split_ascii_whitespace().any(|token| {
            token.is_ascii()
                && token
                    .split_once('/')
                    .is_some_and(|(name, version)| !name.is_empty() && !version.is_empty())
        });
        let has_contact = value.split_ascii_whitespace().any(|token| {
            token
                .split_once('@')
                .is_some_and(|(left, right)| !left.is_empty() && !right.is_empty())
        });
        if !has_application || !has_contact {
            return Err(SecEdgarError::InvalidRequest(
                "SEC User-Agent requires application/version and contact@domain tokens".into(),
            ));
        }
        Ok(Self(value.to_owned()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecUserAgent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecUserAgent([REDACTED])")
    }
}

#[derive(Clone)]
pub struct SecEdgarClient {
    user_agent: SecUserAgent,
    transport: Arc<dyn HttpTransport>,
    gate: Arc<RequestGate>,
}

impl fmt::Debug for SecEdgarClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecEdgarClient")
            .field("user_agent", &self.user_agent)
            .field("transport", &"[REDACTED]")
            .field("request_interval", &REQUEST_INTERVAL)
            .finish()
    }
}

impl SecEdgarClient {
    pub fn new(user_agent: impl Into<String>) -> Result<Self, SecEdgarError> {
        let user_agent = SecUserAgent::new(user_agent)?;
        let transport = Arc::new(transport::production_transport(DEFAULT_TIMEOUT)?);
        Self::from_parts(user_agent, transport)
    }

    pub fn with_transport(
        user_agent: impl Into<String>,
        transport: Arc<dyn HttpTransport>,
    ) -> Result<Self, SecEdgarError> {
        Self::from_parts(SecUserAgent::new(user_agent)?, transport)
    }

    fn from_parts(
        user_agent: SecUserAgent,
        transport: Arc<dyn HttpTransport>,
    ) -> Result<Self, SecEdgarError> {
        Ok(Self {
            user_agent,
            transport,
            gate: Arc::new(RequestGate::new(REQUEST_INTERVAL)?),
        })
    }

    pub fn capabilities() -> FilingCapabilities {
        FilingCapabilities {
            filing_metadata: FILING_METADATA_ADMITTED,
            filing_documents: false,
            xbrl_facts: false,
        }
    }

    pub fn probe_company_filings(
        &self,
        request: &CompanyFilingRequest,
    ) -> Result<DataBatch<CompanyFiling>, SecEdgarError> {
        transport::fetch_company_filings(self, request)
    }
}

impl CompanyFilingsProvider for SecEdgarClient {
    type Error = SecEdgarError;

    fn company_filings(
        &self,
        request: &CompanyFilingRequest,
    ) -> Result<DataBatch<CompanyFiling>, Self::Error> {
        if !FILING_METADATA_ADMITTED {
            return Err(SecEdgarError::Unsupported(
                "SEC filing metadata has not passed live admission".into(),
            ));
        }
        self.probe_company_filings(request)
    }
}
