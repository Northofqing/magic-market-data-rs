#![forbid(unsafe_code)]

mod catalog;
mod html;
mod transport;
mod xlsx;

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
pub use xlsx::{parse_regional_social_financing_workbook, REGIONAL_SOCIAL_FINANCING_CODES};

pub const MONEY_SUPPLY_ADMITTED: bool = true;
pub const SOCIAL_FINANCING_ADMITTED: bool = true;
pub const REGIONAL_SERIES_ADMITTED: bool = true;

pub(crate) const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_REGIONAL_XLSX_BYTES: usize = 256 * 1024;
pub const REGIONAL_SOCIAL_FINANCING_URL: &str =
    "https://www.pbc.gov.cn/diaochatongjisi/fileDir/resource/cms/2025/05/2025051514404575389.xlsx";
const REGIONAL_SOCIAL_FINANCING_PATH: &str =
    "/diaochatongjisi/fileDir/resource/cms/2025/05/2025051514404575389.xlsx";

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
    money_supply_transport: Arc<dyn HttpTransport>,
    regional_transport: Arc<dyn HttpTransport>,
    gate: Arc<RequestGate>,
    request_probe: Arc<Mutex<ProbeRequestTracker>>,
}

impl PbcClient {
    pub fn new(timeout: Duration) -> Result<Self, PbcError> {
        let money_supply_policy = EndpointPolicy::new(
            "www.pbc.gov.cn",
            vec!["/eportal/fileDir/diaochatongjisi/resource/cms/".into()],
            vec![],
            vec![MediaType::Html],
            MAX_HTML_BYTES,
            timeout,
        )?;
        let regional_policy = EndpointPolicy::new(
            "www.pbc.gov.cn",
            vec![REGIONAL_SOCIAL_FINANCING_PATH.into()],
            vec![],
            vec![MediaType::Xlsx],
            MAX_REGIONAL_XLSX_BYTES,
            timeout,
        )?;
        Self::with_transports(
            Arc::new(ReqwestTransport::new(money_supply_policy)?),
            Arc::new(ReqwestTransport::new(regional_policy)?),
        )
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>) -> Result<Self, PbcError> {
        Self::with_transports(Arc::clone(&transport), transport)
    }

    fn with_transports(
        money_supply_transport: Arc<dyn HttpTransport>,
        regional_transport: Arc<dyn HttpTransport>,
    ) -> Result<Self, PbcError> {
        Ok(Self {
            money_supply_transport,
            regional_transport,
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

    /// Performs the exact, bounded 2025 Q1 regional-flow workbook probe.
    pub fn probe_regional_social_financing(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, PbcError> {
        self.fetch_regional_social_financing(request)
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
        let response = transport::fetch_table(self.money_supply_transport.as_ref(), descriptor);
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

    fn fetch_regional_social_financing(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, PbcError> {
        validate_regional_request(request)?;
        self.gate.wait_for_turn()?;
        self.request_started()?;
        let response = transport::fetch_regional_workbook(self.regional_transport.as_ref());
        self.request_finished()?;
        let response = response?;
        let observed_at = now_timestamp();
        let batch_id = format!("pbc-regional-social-financing:2025-q1:{observed_at}");
        xlsx::parse_regional_social_financing_response(
            response.body(),
            response.content_type(),
            request,
            &observed_at,
            &batch_id,
        )
    }

    fn request_started(&self) -> Result<(), PbcError> {
        self.request_probe
            .lock()
            .map_err(|_| PbcError::Protocol("request probe lock poisoned".into()))?
            .request_started();
        Ok(())
    }

    fn request_finished(&self) -> Result<(), PbcError> {
        self.request_probe
            .lock()
            .map_err(|_| PbcError::Protocol("request probe lock poisoned".into()))?
            .request_finished()
            .map_err(|error| PbcError::Protocol(error.to_string()))
    }
}

impl EconomicSeriesProvider for PbcClient {
    type Error = PbcError;

    fn economic_series(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, Self::Error> {
        match request.series()[0].namespace() {
            "money-supply" => {
                validate_request(request)?;
                if !MONEY_SUPPLY_ADMITTED {
                    return Err(PbcError::Unsupported(
                        "PBC money-supply production access has not passed live admission".into(),
                    ));
                }
                self.fetch_money_supply(request)
            }
            "regional-social-financing-flow" => {
                validate_regional_request(request)?;
                if !SOCIAL_FINANCING_ADMITTED || !REGIONAL_SERIES_ADMITTED {
                    return Err(PbcError::Unsupported(
                        "PBC regional social-financing production access has not passed live admission"
                            .into(),
                    ));
                }
                self.fetch_regional_social_financing(request)
            }
            _ => Err(PbcError::Unsupported(
                "PBC namespace is not cataloged".into(),
            )),
        }
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

fn validate_regional_request(request: &EconomicSeriesRequest) -> Result<(), PbcError> {
    if request.provider() != ProviderId::Pbc {
        return Err(PbcError::InvalidRequest(
            "request provider must be PBC".into(),
        ));
    }
    if request
        .series()
        .iter()
        .any(|series| series.namespace() != "regional-social-financing-flow")
    {
        return Err(PbcError::Unsupported(
            "only the exact regional-social-financing-flow namespace is supported".into(),
        ));
    }
    if request.series().len() > REGIONAL_SOCIAL_FINANCING_CODES.len()
        || request
            .series()
            .iter()
            .any(|series| !REGIONAL_SOCIAL_FINANCING_CODES.contains(&series.code()))
    {
        return Err(PbcError::InvalidRequest(
            "regional social-financing request contains an uncataloged series code".into(),
        ));
    }
    let audited = magic_market_core::EconomicPeriod::quarter(2025, 1)?;
    if request.start() != &audited || request.end() != &audited {
        return Err(PbcError::Unsupported(
            "only the cataloged 2025 Q1 regional workbook is supported".into(),
        ));
    }
    let required_rows = request.series().len() * 31;
    if (request.max_rows().get() as usize) < required_rows {
        return Err(PbcError::InvalidRequest(format!(
            "regional request max_rows must be at least {required_rows} to preserve all 31 regions"
        )));
    }
    Ok(())
}

fn now_timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "observed-time-unavailable".into())
}

#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
