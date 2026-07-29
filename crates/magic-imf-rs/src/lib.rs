#![forbid(unsafe_code)]

mod parser;
mod transport;

pub use parser::{parse_imf_responses, parse_namespace, ImfParseContext};

use magic_market_core::{
    CoreError, DataBatch, EconomicDataCapabilities, EconomicFrequency, EconomicObservation,
    EconomicSeriesProvider, EconomicSeriesRequest, ProviderId,
};
use magic_market_transport::{HttpTransport, RequestGate, ReqwestTransport, TransportError};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub const ECONOMIC_SERIES_ADMITTED: bool = false;

#[derive(Debug, Error)]
pub enum ImfError {
    #[error("invalid IMF request: {0}")]
    InvalidRequest(String),
    #[error("IMF authentication failed: {0}")]
    Authentication(String),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("IMF response decode failed: {0}")]
    Decode(String),
    #[error("IMF protocol violation: {0}")]
    Protocol(String),
    #[error("IMF capability unsupported: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] CoreError),
}

#[derive(Clone)]
pub struct ImfClient {
    transport: Arc<dyn HttpTransport>,
    gate: Arc<RequestGate>,
}

impl fmt::Debug for ImfClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImfClient")
            .field("transport", &"[REDACTED]")
            .field("gate", &self.gate)
            .finish()
    }
}

impl ImfClient {
    pub fn new() -> Result<Self, ImfError> {
        let transport = Arc::new(ReqwestTransport::new(transport::policy()?)?);
        Self::with_transport(transport)
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>) -> Result<Self, ImfError> {
        Ok(Self {
            transport,
            gate: Arc::new(RequestGate::new(Duration::from_secs(1))?),
        })
    }

    pub const fn economic_data_capabilities() -> EconomicDataCapabilities {
        EconomicDataCapabilities {
            economic_series: ECONOMIC_SERIES_ADMITTED,
            regional_series: ECONOMIC_SERIES_ADMITTED,
        }
    }

    /// Executes the bounded technical probe without advertising production
    /// admission through the formal Provider contract.
    pub fn probe_economic_series(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, ImfError> {
        validate_request(request)?;
        transport::fetch_series(self.transport.as_ref(), self.gate.as_ref(), request)
    }
}

impl EconomicSeriesProvider for ImfClient {
    type Error = ImfError;

    fn economic_series(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, Self::Error> {
        validate_request(request)?;
        if !ECONOMIC_SERIES_ADMITTED {
            return Err(ImfError::Unsupported(
                "IMF production access has not passed live admission".into(),
            ));
        }
        self.probe_economic_series(request)
    }
}

fn validate_request(request: &EconomicSeriesRequest) -> Result<(), ImfError> {
    if request.provider() != ProviderId::Imf {
        return Err(ImfError::InvalidRequest(
            "request provider must be IMF".into(),
        ));
    }
    if request.series().len() > 20 {
        return Err(ImfError::InvalidRequest(
            "IMF accepts at most 20 series per call".into(),
        ));
    }
    if request.start().frequency() != EconomicFrequency::Annual {
        return Err(ImfError::Unsupported(
            "IMF DataMapper production path supports annual periods".into(),
        ));
    }
    Ok(())
}
