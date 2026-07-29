#![forbid(unsafe_code)]

mod parser;
mod transport;

pub use parser::{
    parse_world_bank_namespace, parse_world_bank_responses, WorldBankNamespace,
    WorldBankParseContext,
};

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

#[derive(Debug, Error)]
pub enum WorldBankError {
    #[error("invalid World Bank request: {0}")]
    InvalidRequest(String),
    #[error("World Bank authentication failed: {0}")]
    Authentication(String),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("World Bank response decode failed: {0}")]
    Decode(String),
    #[error("World Bank protocol violation: {0}")]
    Protocol(String),
    #[error("World Bank capability unsupported: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] CoreError),
}

#[derive(Clone)]
pub struct WorldBankClient {
    transport: Arc<dyn HttpTransport>,
    gate: Arc<RequestGate>,
}

impl fmt::Debug for WorldBankClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorldBankClient")
            .field("transport", &"[REDACTED]")
            .field("gate", &self.gate)
            .finish()
    }
}

impl WorldBankClient {
    pub fn new() -> Result<Self, WorldBankError> {
        let transport = Arc::new(ReqwestTransport::new(transport::policy()?)?);
        Self::with_transport(transport)
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>) -> Result<Self, WorldBankError> {
        Ok(Self {
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

    pub fn probe_economic_series(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, WorldBankError> {
        if request.provider() != ProviderId::WorldBank {
            return Err(WorldBankError::InvalidRequest(
                "request provider must be World Bank".into(),
            ));
        }
        transport::fetch_series(self.transport.as_ref(), self.gate.as_ref(), request)
    }
}

impl EconomicSeriesProvider for WorldBankClient {
    type Error = WorldBankError;

    fn economic_series(
        &self,
        _request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, Self::Error> {
        Err(WorldBankError::Unsupported(
            "structured indicator units are empty under the mandatory-unit contract".into(),
        ))
    }
}
