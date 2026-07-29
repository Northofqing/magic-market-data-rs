#![forbid(unsafe_code)]
//! Bounded, metadata-only adapter for Securities Times quick-news metadata.

mod json;

use magic_market_core::{
    ContentCapabilities, DataBatch, InstrumentDateRangeRequest, LoadProbeSnapshot, NewsItem,
    NewsProvider, PositiveU32, ProbeRequestTracker,
};
use magic_market_transport::{
    EndpointPolicy, HttpMethod, HttpRequest, HttpTransport, MediaType, RequestGate,
    ReqwestTransport, TransportError,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

pub use json::parse_quick_news;

const LIST_URL: &str = "https://www.stcn.com/article/list.html?type=kx";
const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_RETURNED_ITEMS: u32 = 30;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_INTERVAL: Duration = Duration::from_secs(1);

/// Admitted after two bounded live probes and the three-call serial load probe.
pub const GLOBAL_NEWS_ADMITTED: bool = true;

#[derive(Debug, Error)]
pub enum StcnError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("Securities Times response decoding failed: {0}")]
    Decode(String),
    #[error("Securities Times protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Clone)]
pub struct StcnClient {
    transport: Arc<dyn HttpTransport>,
    policy: EndpointPolicy,
    gate: Arc<RequestGate>,
    request_probe: Arc<Mutex<ProbeRequestTracker>>,
}

impl std::fmt::Debug for StcnClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("StcnClient").finish_non_exhaustive()
    }
}

impl StcnClient {
    pub fn new() -> Result<Self, StcnError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, StcnError> {
        let policy = endpoint_policy(timeout)?;
        let transport = Arc::new(ReqwestTransport::new(policy.clone())?);
        Self::from_parts(transport, policy)
    }

    pub fn with_transport(transport: impl HttpTransport + 'static) -> Result<Self, StcnError> {
        let policy = endpoint_policy(DEFAULT_TIMEOUT)?;
        Self::from_parts(Arc::new(transport), policy)
    }

    fn from_parts(
        transport: Arc<dyn HttpTransport>,
        policy: EndpointPolicy,
    ) -> Result<Self, StcnError> {
        Ok(Self {
            transport,
            policy,
            gate: Arc::new(RequestGate::new(REQUEST_INTERVAL)?),
            request_probe: Arc::new(Mutex::new(ProbeRequestTracker::default())),
        })
    }

    pub const fn content_capabilities() -> ContentCapabilities {
        ContentCapabilities {
            instrument_news: false,
            global_news: GLOBAL_NEWS_ADMITTED,
            announcements: false,
            market_announcements: false,
            investor_questions: false,
        }
    }

    pub fn load_probe_snapshot(&self) -> Result<LoadProbeSnapshot, StcnError> {
        self.request_probe
            .lock()
            .map(|probe| probe.snapshot())
            .map_err(|_| tracker_error("request tracker lock poisoned"))
    }

    /// Executes the same bounded source contract explicitly for admission checks.
    pub fn probe_global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, StcnError> {
        validate_limit(limit)?;
        let request = request()?;
        self.policy.validate_request(&request)?;
        self.gate.wait_for_turn()?;
        self.request_probe
            .lock()
            .map_err(|_| tracker_error("request tracker lock poisoned"))?
            .request_started();
        let response = self.transport.execute(&request);
        self.request_probe
            .lock()
            .map_err(|_| tracker_error("request tracker lock poisoned"))?
            .request_finished()
            .map_err(|error| tracker_error(&error.to_string()))?;
        let response = response?;
        let response = self.policy.validate_response_for(&request, response)?;
        parse_quick_news(response.body(), limit.get())
    }
}

fn tracker_error(message: &str) -> StcnError {
    StcnError::Transport(TransportError::Internal(message.into()))
}

impl NewsProvider for StcnClient {
    type Error = StcnError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(StcnError::Unsupported(
            "Securities Times quick-news listing has no verified instrument/date filter".into(),
        ))
    }

    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        if GLOBAL_NEWS_ADMITTED {
            self.probe_global_news(limit)
        } else {
            Err(StcnError::Unsupported(
                "Securities Times global-news capability is disabled; use probe_global_news for explicit diagnostics".into(),
            ))
        }
    }
}

fn endpoint_policy(timeout: Duration) -> Result<EndpointPolicy, StcnError> {
    Ok(EndpointPolicy::new(
        "www.stcn.com",
        vec!["/article/list.html".into()],
        vec!["type".into()],
        vec![MediaType::Json],
        MAX_JSON_BYTES,
        timeout,
    )?)
}

fn request() -> Result<HttpRequest, StcnError> {
    Ok(HttpRequest::new(
        HttpMethod::Get,
        LIST_URL,
        vec![
            (
                "Accept".into(),
                "application/json, text/javascript, */*; q=0.01".into(),
            ),
            ("X-Requested-With".into(), "XMLHttpRequest".into()),
            (
                "Referer".into(),
                "https://www.stcn.com/article/list/kx.html".into(),
            ),
            ("User-Agent".into(), "magic-stcn-rs/0.2".into()),
        ],
        Vec::new(),
    )?)
}

fn validate_limit(limit: PositiveU32) -> Result<(), StcnError> {
    if limit.get() <= MAX_RETURNED_ITEMS {
        Ok(())
    } else {
        Err(StcnError::InvalidRequest(format!(
            "Securities Times limit must be between 1 and {MAX_RETURNED_ITEMS}"
        )))
    }
}
