#![forbid(unsafe_code)]

mod catalog;
mod html;
mod transport;

use magic_market_core::{
    DataBatch, EconomicDataCapabilities, EconomicObservation, EconomicSeriesProvider,
    EconomicSeriesRequest, LoadProbeSnapshot, ProbeRequestTracker, ProviderId,
};
use magic_market_transport::{
    EndpointPolicy, HttpTransport, MediaType, RequestGate, ReqwestTransport, TransportError,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

pub use catalog::{descriptor_for_year, PbcTableDescriptor};
pub use html::parse_money_supply_table;

pub const MONEY_SUPPLY_ADMITTED: bool = true;
pub const SOCIAL_FINANCING_ADMITTED: bool = false;
pub const REGIONAL_SERIES_ADMITTED: bool = false;

pub(crate) const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum PbcError {
    #[error("invalid PBC request: {0}")]
    InvalidRequest(String),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("PBC response decoding failed: {0}")]
    Decode(String),
    #[error("PBC source contract failed: {0}")]
    Protocol(String),
    #[error("unsupported PBC capability: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Clone)]
pub struct PbcClient {
    transport: Arc<dyn HttpTransport>,
    gate: Arc<RequestGate>,
    request_probe: Arc<Mutex<ProbeRequestTracker>>,
}

impl PbcClient {
    pub fn new(timeout: Duration) -> Result<Self, PbcError> {
        let policy = EndpointPolicy::new(
            "www.pbc.gov.cn",
            vec!["/eportal/fileDir/diaochatongjisi/resource/cms/".into()],
            vec![],
            vec![MediaType::Html],
            MAX_HTML_BYTES,
            timeout,
        )?;
        Self::with_transport(Arc::new(ReqwestTransport::new(policy)?))
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>) -> Result<Self, PbcError> {
        Ok(Self {
            transport,
            gate: Arc::new(RequestGate::new(Duration::from_secs(1))?),
            request_probe: Arc::new(Mutex::new(ProbeRequestTracker::default())),
        })
    }

    pub const fn economic_data_capabilities() -> EconomicDataCapabilities {
        EconomicDataCapabilities {
            economic_series: MONEY_SUPPLY_ADMITTED,
            regional_series: REGIONAL_SERIES_ADMITTED,
        }
    }

    pub fn load_probe_snapshot(&self) -> Result<LoadProbeSnapshot, PbcError> {
        self.request_probe
            .lock()
            .map(|probe| probe.snapshot())
            .map_err(|_| PbcError::Protocol("request probe lock poisoned".into()))
    }

    /// Performs the bounded, strict technical probe without advertising the
    /// family through the formal Provider contract.
    pub fn probe_money_supply(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, PbcError> {
        self.fetch_money_supply(request)
    }

    fn fetch_money_supply(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, PbcError> {
        validate_request(request)?;
        let (start_year, _) = request
            .start()
            .as_month()
            .ok_or_else(|| PbcError::InvalidRequest("monthly range required".into()))?;
        let (end_year, _) = request
            .end()
            .as_month()
            .ok_or_else(|| PbcError::InvalidRequest("monthly range required".into()))?;
        if start_year != end_year {
            return Err(PbcError::Unsupported(
                "PBC requests spanning multiple catalog years are not admitted".into(),
            ));
        }
        let descriptor = descriptor_for_year(start_year as u16)?;
        self.gate.wait_for_turn()?;
        self.request_probe
            .lock()
            .map_err(|_| PbcError::Protocol("request probe lock poisoned".into()))?
            .request_started();
        let response = transport::fetch_table(self.transport.as_ref(), descriptor);
        self.request_probe
            .lock()
            .map_err(|_| PbcError::Protocol("request probe lock poisoned".into()))?
            .request_finished()
            .map_err(|error| PbcError::Protocol(error.to_string()))?;
        let response = response?;
        let observed_at = now_timestamp();
        let batch_id = format!("pbc-money-supply:{start_year}:{observed_at}");
        html::parse_money_supply_response(
            response.body(),
            response.content_type(),
            descriptor,
            request,
            &observed_at,
            &batch_id,
        )
    }
}

impl EconomicSeriesProvider for PbcClient {
    type Error = PbcError;

    fn economic_series(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, Self::Error> {
        validate_request(request)?;
        if !MONEY_SUPPLY_ADMITTED {
            return Err(PbcError::Unsupported(
                "PBC money-supply production access has not passed live admission".into(),
            ));
        }
        self.fetch_money_supply(request)
    }
}

fn validate_request(request: &EconomicSeriesRequest) -> Result<(), PbcError> {
    if request.provider() != ProviderId::Pbc {
        return Err(PbcError::InvalidRequest(
            "request provider must be PBC".into(),
        ));
    }
    if request
        .series()
        .iter()
        .any(|series| series.namespace() != "money-supply")
    {
        return Err(PbcError::Unsupported(
            "only cataloged money-supply HTML tables are supported; social financing and regional series are not admitted"
                .into(),
        ));
    }
    if request.series().len() > 3
        || request
            .series()
            .iter()
            .any(|series| !matches!(series.code(), "M0" | "M1" | "M2"))
    {
        return Err(PbcError::InvalidRequest(
            "money-supply requests accept unique M0, M1 and M2 codes only".into(),
        ));
    }
    if request.start().as_month().is_none() || request.end().as_month().is_none() {
        return Err(PbcError::InvalidRequest(
            "money-supply requests must be monthly".into(),
        ));
    }
    Ok(())
}

fn now_timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "observed-time-unavailable".into())
}
