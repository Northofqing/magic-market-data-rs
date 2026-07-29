#![forbid(unsafe_code)]

mod parser;
mod transport;

pub use parser::{parse_fred_responses, FredParseContext};

use magic_market_core::{
    CoreError, DataBatch, EconomicDataCapabilities, EconomicObservation, EconomicSeriesProvider,
    EconomicSeriesRequest, ProviderId,
};
use magic_market_transport::{HttpTransport, RequestGate, ReqwestTransport, TransportError};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub const ECONOMIC_SERIES_ADMITTED: bool = false;

#[derive(Clone)]
struct ApiKey(String);

impl fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiKey([REDACTED])")
    }
}

#[derive(Debug, Error)]
pub enum FredError {
    #[error("invalid FRED request: {0}")]
    InvalidRequest(String),
    #[error("FRED authentication failed: {0}")]
    Authentication(String),
    #[error(transparent)]
    Transport(TransportError),
    #[error("FRED response decode failed: {0}")]
    Decode(String),
    #[error("FRED protocol violation: {0}")]
    Protocol(String),
    #[error("FRED capability unsupported: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] CoreError),
}

impl From<TransportError> for FredError {
    fn from(error: TransportError) -> Self {
        match error {
            TransportError::Authentication(_) => {
                Self::Authentication("official API rejected the supplied credentials".into())
            }
            other => Self::Transport(other),
        }
    }
}

#[derive(Clone)]
pub struct FredClient {
    api_key: ApiKey,
    transport: Arc<dyn HttpTransport>,
    gate: Arc<RequestGate>,
}

impl fmt::Debug for FredClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FredClient")
            .field("api_key", &self.api_key)
            .field("transport", &"[REDACTED]")
            .field("gate", &self.gate)
            .finish()
    }
}

impl FredClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self, FredError> {
        let api_key = checked_api_key(api_key.into())?;
        let transport = Arc::new(ReqwestTransport::new(transport::policy()?)?);
        Self::with_transport_inner(api_key, transport)
    }

    pub fn with_transport(
        api_key: impl Into<String>,
        transport: Arc<dyn HttpTransport>,
    ) -> Result<Self, FredError> {
        let api_key = checked_api_key(api_key.into())?;
        Self::with_transport_inner(api_key, transport)
    }

    fn with_transport_inner(
        api_key: ApiKey,
        transport: Arc<dyn HttpTransport>,
    ) -> Result<Self, FredError> {
        Ok(Self {
            api_key,
            transport,
            gate: Arc::new(RequestGate::new(Duration::from_secs(1))?),
        })
    }

    pub const fn economic_data_capabilities() -> EconomicDataCapabilities {
        EconomicDataCapabilities {
            economic_series: ECONOMIC_SERIES_ADMITTED,
            regional_series: false,
        }
    }

    /// Executes the bounded technical probe without advertising production
    /// admission through the formal Provider contract.
    pub fn probe_economic_series(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, FredError> {
        validate_request(request)?;
        transport::fetch_series(
            self.transport.as_ref(),
            self.gate.as_ref(),
            &self.api_key.0,
            request,
        )
    }
}

impl EconomicSeriesProvider for FredClient {
    type Error = FredError;

    fn economic_series(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, Self::Error> {
        validate_request(request)?;
        if !ECONOMIC_SERIES_ADMITTED {
            return Err(FredError::Unsupported(
                "FRED production access has not passed live admission".into(),
            ));
        }
        self.probe_economic_series(request)
    }
}

fn validate_request(request: &EconomicSeriesRequest) -> Result<(), FredError> {
    if request.provider() != ProviderId::Fred {
        return Err(FredError::InvalidRequest(
            "request provider must be FRED".into(),
        ));
    }
    if request.series().len() > 20 {
        return Err(FredError::InvalidRequest(
            "FRED accepts at most 20 series per call".into(),
        ));
    }
    Ok(())
}

fn checked_api_key(value: String) -> Result<ApiKey, FredError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(FredError::Authentication(
            "runtime API key is required".into(),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(FredError::Authentication(
            "runtime API key contains forbidden characters".into(),
        ));
    }
    if trimmed.len() > 512 {
        return Err(FredError::Authentication(
            "runtime API key exceeds the provider limit".into(),
        ));
    }
    Ok(ApiKey(trimmed.to_owned()))
}

#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
