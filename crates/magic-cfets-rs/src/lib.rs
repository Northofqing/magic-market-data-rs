#![forbid(unsafe_code)]

mod fx;
mod rates;
mod transport;

use magic_market_core::{
    DataBatch, LoadProbeSnapshot, OfficialFxFixing, OfficialFxFixingProvider,
    OfficialFxFixingRequest, ProbeRequestTracker, ReferenceDataCapabilities, ReferenceRateKind,
    ReferenceRateObservation, ReferenceRateProvider, ReferenceRateRequest,
};
use magic_market_transport::{
    EndpointPolicy, HttpTransport, MediaType, RequestGate, ReqwestTransport, TransportError,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

pub use fx::{parse_central_parity_pages, source_heading};
pub use rates::{parse_lpr_payload, parse_shibor_payload};

pub const SHIBOR_ADMITTED: bool = true;
pub const LPR_ADMITTED: bool = true;
pub const DR007_ADMITTED: bool = false;
pub const OFFICIAL_FX_ADMITTED: bool = true;

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfetsCapabilities {
    pub shibor: bool,
    pub loan_prime_rate: bool,
    pub dr007: bool,
    pub official_fx_fixings: bool,
}

#[derive(Debug, Error)]
pub enum CfetsError {
    #[error("invalid CFETS request: {0}")]
    InvalidRequest(String),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("CFETS response decoding failed: {0}")]
    Decode(String),
    #[error("CFETS source contract failed: {0}")]
    Protocol(String),
    #[error("unsupported CFETS capability: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Clone)]
pub struct CfetsClient {
    transport: Arc<dyn HttpTransport>,
    gate: Arc<RequestGate>,
    request_probe: Arc<Mutex<ProbeRequestTracker>>,
}

impl CfetsClient {
    pub fn new(timeout: Duration) -> Result<Self, CfetsError> {
        let policy = EndpointPolicy::new(
            "www.chinamoney.com.cn",
            vec!["/ags/ms/".into()],
            vec![
                "lang".into(),
                "startDate".into(),
                "endDate".into(),
                "strStartDate".into(),
                "strEndDate".into(),
                "currency".into(),
                "pageNum".into(),
                "pageSize".into(),
            ],
            vec![MediaType::Json],
            MAX_RESPONSE_BYTES,
            timeout,
        )?;
        Self::with_transport(Arc::new(ReqwestTransport::new(policy)?))
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>) -> Result<Self, CfetsError> {
        Ok(Self {
            transport,
            gate: Arc::new(RequestGate::new(Duration::from_secs(1))?),
            request_probe: Arc::new(Mutex::new(ProbeRequestTracker::default())),
        })
    }

    pub const fn capabilities() -> CfetsCapabilities {
        CfetsCapabilities {
            shibor: SHIBOR_ADMITTED,
            loan_prime_rate: LPR_ADMITTED,
            dr007: DR007_ADMITTED,
            official_fx_fixings: OFFICIAL_FX_ADMITTED,
        }
    }

    pub const fn reference_data_capabilities() -> ReferenceDataCapabilities {
        ReferenceDataCapabilities {
            benchmark_rates: SHIBOR_ADMITTED || LPR_ADMITTED || DR007_ADMITTED,
            official_fx_fixings: OFFICIAL_FX_ADMITTED,
        }
    }

    pub fn load_probe_snapshot(&self) -> Result<LoadProbeSnapshot, CfetsError> {
        self.request_probe
            .lock()
            .map(|probe| probe.snapshot())
            .map_err(|_| CfetsError::Protocol("request probe lock poisoned".into()))
    }

    fn record_start(&self) -> Result<(), CfetsError> {
        self.request_probe
            .lock()
            .map_err(|_| CfetsError::Protocol("request probe lock poisoned".into()))?
            .request_started();
        Ok(())
    }

    fn record_finish(&self) -> Result<(), CfetsError> {
        self.request_probe
            .lock()
            .map_err(|_| CfetsError::Protocol("request probe lock poisoned".into()))?
            .request_finished()
            .map_err(|error| CfetsError::Protocol(error.to_string()))
    }

    pub fn probe_reference_rates(
        &self,
        request: &ReferenceRateRequest,
    ) -> Result<DataBatch<ReferenceRateObservation>, CfetsError> {
        let family = validate_rate_request(request)?;
        self.fetch_reference_rates(request, family)
    }

    pub fn probe_official_fx_fixings(
        &self,
        request: &OfficialFxFixingRequest,
    ) -> Result<DataBatch<OfficialFxFixing>, CfetsError> {
        validate_fx_request(request)?;
        self.fetch_official_fx_fixings(request)
    }

    fn fetch_reference_rates(
        &self,
        request: &ReferenceRateRequest,
        family: RateFamily,
    ) -> Result<DataBatch<ReferenceRateObservation>, CfetsError> {
        self.gate.wait_for_turn()?;
        self.record_start()?;
        let response = match family {
            RateFamily::Shibor => transport::fetch_shibor(self.transport.as_ref(), request),
            RateFamily::Lpr => transport::fetch_lpr(self.transport.as_ref(), request),
        };
        self.record_finish()?;
        let response = response?;
        let observed_at = now_timestamp();
        let name = match family {
            RateFamily::Shibor => "shibor",
            RateFamily::Lpr => "lpr",
        };
        let batch_id = format!("cfets-{name}:{observed_at}");
        match family {
            RateFamily::Shibor => {
                parse_shibor_payload(response.body(), request, &observed_at, &batch_id)
            }
            RateFamily::Lpr => parse_lpr_payload(response.body(), request, &observed_at, &batch_id),
        }
    }

    fn fetch_official_fx_fixings(
        &self,
        request: &OfficialFxFixingRequest,
    ) -> Result<DataBatch<OfficialFxFixing>, CfetsError> {
        let headings = request
            .pairs()
            .iter()
            .map(source_heading)
            .collect::<Result<Vec<_>, _>>()?;
        let mut pages = Vec::new();
        for page in 1..=20 {
            self.gate.wait_for_turn()?;
            self.record_start()?;
            let response =
                transport::fetch_fx_page(self.transport.as_ref(), request, &headings, page);
            self.record_finish()?;
            let response = response?;
            let page_total = fx::page_total(response.body())?;
            pages.push(response.body().to_vec());
            if page >= page_total {
                break;
            }
        }
        let observed_at = now_timestamp();
        let batch_id = format!("cfets-ccpr:{observed_at}");
        parse_central_parity_pages(&pages, request, &observed_at, &batch_id)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RateFamily {
    Shibor,
    Lpr,
}

fn validate_rate_request(request: &ReferenceRateRequest) -> Result<RateFamily, CfetsError> {
    if request.provider() != magic_market_core::ProviderId::Cfets {
        return Err(CfetsError::InvalidRequest(
            "reference-rate provider must be CFETS".into(),
        ));
    }
    let mut family = None;
    for rate in request.rates() {
        let current = match rate.kind() {
            ReferenceRateKind::Shibor(_) => RateFamily::Shibor,
            ReferenceRateKind::LoanPrimeRate(_) => RateFamily::Lpr,
            ReferenceRateKind::Dr007 => {
                return Err(CfetsError::Unsupported(
                    "CFETS DR007 history has no separately proven public contract".into(),
                ));
            }
            ReferenceRateKind::SourceDefined(_) => {
                return Err(CfetsError::Unsupported(
                    "source-defined CFETS reference rates are not admitted".into(),
                ));
            }
        };
        if family
            .replace(current)
            .is_some_and(|prior| prior != current)
        {
            return Err(CfetsError::InvalidRequest(
                "one CFETS request cannot mix Shibor and LPR".into(),
            ));
        }
    }
    family.ok_or_else(|| CfetsError::InvalidRequest("reference-rate request is empty".into()))
}

fn validate_fx_request(request: &OfficialFxFixingRequest) -> Result<(), CfetsError> {
    if request.provider() != magic_market_core::ProviderId::Cfets {
        return Err(CfetsError::InvalidRequest(
            "official FX request provider must be CFETS".into(),
        ));
    }
    for pair in request.pairs() {
        let _ = source_heading(pair)?;
    }
    Ok(())
}

impl ReferenceRateProvider for CfetsClient {
    type Error = CfetsError;

    fn reference_rates(
        &self,
        request: &ReferenceRateRequest,
    ) -> Result<DataBatch<ReferenceRateObservation>, Self::Error> {
        let family = validate_rate_request(request)?;
        let admitted = match family {
            RateFamily::Shibor => SHIBOR_ADMITTED,
            RateFamily::Lpr => LPR_ADMITTED,
        };
        if !admitted {
            return Err(CfetsError::Unsupported(
                "CFETS reference-rate family has not passed live admission".into(),
            ));
        }
        self.fetch_reference_rates(request, family)
    }
}

impl OfficialFxFixingProvider for CfetsClient {
    type Error = CfetsError;

    fn official_fx_fixings(
        &self,
        request: &OfficialFxFixingRequest,
    ) -> Result<DataBatch<OfficialFxFixing>, Self::Error> {
        validate_fx_request(request)?;
        if !OFFICIAL_FX_ADMITTED {
            return Err(CfetsError::Unsupported(
                "CFETS official FX production access has not passed live admission".into(),
            ));
        }
        self.fetch_official_fx_fixings(request)
    }
}

fn now_timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "observed-time-unavailable".into())
}
