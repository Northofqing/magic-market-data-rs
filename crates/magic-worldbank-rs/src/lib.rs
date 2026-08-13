#![forbid(unsafe_code)]

mod parser;
mod transport;

pub use parser::{
    parse_world_bank_namespace, parse_world_bank_responses,
    parse_world_bank_responses_with_metadata, WorldBankNamespace, WorldBankParseContext,
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

/// Admission is deliberately limited to the exact contract checked by
/// `validate_admitted_request`; it is not a catalog-wide claim.
pub const ECONOMIC_SERIES_ADMITTED: bool = true;

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
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, Self::Error> {
        validate_admitted_request(request)?;
        self.probe_economic_series(request)
    }
}

fn validate_admitted_request(request: &EconomicSeriesRequest) -> Result<(), WorldBankError> {
    if !ECONOMIC_SERIES_ADMITTED {
        return Err(WorldBankError::Unsupported(
            "World Bank production access has not passed live admission".into(),
        ));
    }
    if request.series().len() != 1 {
        return Err(WorldBankError::Unsupported(
            "admitted World Bank scope accepts exactly one series".into(),
        ));
    }
    let key = &request.series()[0];
    if key.provider() != ProviderId::WorldBank
        || key.namespace() != "source:2/country:USA"
        || key.code() != "NY.GDP.MKTP.CD"
        || request.start().as_year() != Some(2024)
        || request.end().as_year() != Some(2024)
        || request.max_rows().get() != 1
    {
        return Err(WorldBankError::Unsupported(
            "admitted World Bank scope is exactly source:2/country:USA NY.GDP.MKTP.CD annual 2024 max_rows=1"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
