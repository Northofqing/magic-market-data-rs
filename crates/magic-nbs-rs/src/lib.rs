#![forbid(unsafe_code)]

mod parser;
mod transport;

use magic_market_core::{
    DataBatch, EconomicDataCapabilities, EconomicObservation, EconomicSeriesProvider,
    EconomicSeriesRequest,
};
use magic_market_transport::{
    EndpointPolicy, HttpTransport, MediaType, RequestGate, ReqwestTransport, TransportError,
};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub use parser::parse_national_monthly_payload;

pub const NATIONAL_SERIES_ADMITTED: bool = false;
pub const REGIONAL_SERIES_ADMITTED: bool = false;

const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct NbsDiagnosticRequest {
    request: EconomicSeriesRequest,
    body: Vec<u8>,
}

impl NbsDiagnosticRequest {
    pub fn new(request: EconomicSeriesRequest, body: Vec<u8>) -> Result<Self, NbsError> {
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(NbsError::InvalidRequest(
                "diagnostic body exceeds 4 MiB".into(),
            ));
        }
        Ok(Self { request, body })
    }

    pub fn request(&self) -> &EconomicSeriesRequest {
        &self.request
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

#[derive(Debug, Error)]
pub enum NbsError {
    #[error("invalid NBS request: {0}")]
    InvalidRequest(String),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("NBS response decoding failed: {0}")]
    Decode(String),
    #[error("NBS source contract failed: {0}")]
    Protocol(String),
    #[error("unsupported NBS capability: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Clone)]
pub struct NbsClient {
    transport: Arc<dyn HttpTransport>,
    gate: Arc<RequestGate>,
}

impl NbsClient {
    pub fn new(timeout: Duration) -> Result<Self, NbsError> {
        let policy = EndpointPolicy::new(
            "www.stats.gov.cn",
            vec!["/".into()],
            vec![],
            vec![MediaType::Html],
            512 * 1024,
            timeout,
        )?;
        Self::with_transport(Arc::new(ReqwestTransport::new(policy)?))
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>) -> Result<Self, NbsError> {
        Ok(Self {
            transport,
            gate: Arc::new(RequestGate::new(Duration::from_secs(1))?),
        })
    }

    pub const fn economic_data_capabilities() -> EconomicDataCapabilities {
        EconomicDataCapabilities {
            economic_series: NATIONAL_SERIES_ADMITTED,
            regional_series: REGIONAL_SERIES_ADMITTED,
        }
    }

    pub fn probe_national_payload(
        &self,
        request: &EconomicSeriesRequest,
        body: &[u8],
        observed_at: &str,
    ) -> Result<DataBatch<EconomicObservation>, NbsError> {
        let batch_id = format!("nbs-diagnostic:{observed_at}");
        parse_national_monthly_payload(body, request, observed_at, &batch_id)
    }

    pub fn probe_national_diagnostic(
        &self,
        diagnostic: &NbsDiagnosticRequest,
        observed_at: &str,
    ) -> Result<DataBatch<EconomicObservation>, NbsError> {
        self.probe_national_payload(diagnostic.request(), diagnostic.body(), observed_at)
    }

    pub fn probe_public_landing_page(&self) -> Result<usize, NbsError> {
        transport::probe_landing_page(self.transport.as_ref(), self.gate.as_ref())
    }
}

impl EconomicSeriesProvider for NbsClient {
    type Error = NbsError;

    fn economic_series(
        &self,
        _request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, Self::Error> {
        Err(NbsError::Unsupported(
            "NBS production access is not admitted: the official site exposes no supported machine contract and rejected the audited minimal client"
                .into(),
        ))
    }
}

pub(crate) const fn max_response_bytes() -> usize {
    MAX_RESPONSE_BYTES
}
