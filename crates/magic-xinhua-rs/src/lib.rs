#![forbid(unsafe_code)]
//! Bounded, metadata-only adapter for the official Xinhua Finance news list.

mod html;

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

pub use html::parse_listing;

const LIST_URL: &str = "https://www.cnfin.com/news/index.html";
const MAX_HTML_BYTES: usize = 1024 * 1024;
const MAX_RETURNED_ITEMS: u32 = 13;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_INTERVAL: Duration = Duration::from_secs(1);

/// Admitted after two bounded live probes and the three-call serial load probe.
pub const GLOBAL_NEWS_ADMITTED: bool = true;

#[derive(Debug, Error)]
pub enum XinhuaError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("Xinhua Finance response decoding failed: {0}")]
    Decode(String),
    #[error("Xinhua Finance protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Clone)]
pub struct XinhuaClient {
    transport: Arc<dyn HttpTransport>,
    policy: EndpointPolicy,
    gate: Arc<RequestGate>,
    request_probe: Arc<Mutex<ProbeRequestTracker>>,
}

impl std::fmt::Debug for XinhuaClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("XinhuaClient")
            .finish_non_exhaustive()
    }
}

impl XinhuaClient {
    pub fn new() -> Result<Self, XinhuaError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, XinhuaError> {
        let policy = endpoint_policy(timeout)?;
        let transport = Arc::new(ReqwestTransport::new(policy.clone())?);
        Self::from_parts(transport, policy)
    }

    pub fn with_transport(transport: impl HttpTransport + 'static) -> Result<Self, XinhuaError> {
        let policy = endpoint_policy(DEFAULT_TIMEOUT)?;
        Self::from_parts(Arc::new(transport), policy)
    }

    fn from_parts(
        transport: Arc<dyn HttpTransport>,
        policy: EndpointPolicy,
    ) -> Result<Self, XinhuaError> {
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

    pub fn load_probe_snapshot(&self) -> Result<LoadProbeSnapshot, XinhuaError> {
        self.request_probe
            .lock()
            .map(|probe| probe.snapshot())
            .map_err(|_| tracker_error("request tracker lock poisoned"))
    }

    /// Executes the same bounded source contract explicitly for admission checks.
    pub fn probe_global_news(
        &self,
        limit: PositiveU32,
    ) -> Result<DataBatch<NewsItem>, XinhuaError> {
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
        let html = std::str::from_utf8(response.body())
            .map_err(|_| XinhuaError::Decode("response is not valid UTF-8".into()))?;
        parse_listing(html, limit.get())
    }
}

fn tracker_error(message: &str) -> XinhuaError {
    XinhuaError::Transport(TransportError::Internal(message.into()))
}

impl NewsProvider for XinhuaClient {
    type Error = XinhuaError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(XinhuaError::Unsupported(
            "Xinhua Finance listing has no verified instrument/date filter".into(),
        ))
    }

    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        if GLOBAL_NEWS_ADMITTED {
            self.probe_global_news(limit)
        } else {
            Err(XinhuaError::Unsupported(
                "Xinhua Finance global-news capability is disabled; use probe_global_news for explicit diagnostics".into(),
            ))
        }
    }
}

fn endpoint_policy(timeout: Duration) -> Result<EndpointPolicy, XinhuaError> {
    Ok(EndpointPolicy::new(
        "www.cnfin.com",
        vec!["/news/index.html".into()],
        Vec::new(),
        vec![MediaType::Html],
        MAX_HTML_BYTES,
        timeout,
    )?)
}

fn request() -> Result<HttpRequest, XinhuaError> {
    Ok(HttpRequest::new(
        HttpMethod::Get,
        LIST_URL,
        vec![
            ("Accept".into(), "text/html,application/xhtml+xml".into()),
            ("User-Agent".into(), "magic-xinhua-rs/0.2".into()),
        ],
        Vec::new(),
    )?)
}

fn validate_limit(limit: PositiveU32) -> Result<(), XinhuaError> {
    if limit.get() <= MAX_RETURNED_ITEMS {
        Ok(())
    } else {
        Err(XinhuaError::InvalidRequest(format!(
            "Xinhua Finance limit must be between 1 and {MAX_RETURNED_ITEMS}"
        )))
    }
}
