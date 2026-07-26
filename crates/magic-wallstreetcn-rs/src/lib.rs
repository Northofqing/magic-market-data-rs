#![forbid(unsafe_code)]
//! Bounded metadata-only adapter for the WallstreetCN RSS feed.

use magic_market_core::{
    ContentCapabilities, DataBatch, InstrumentDateRangeRequest, NewsItem, NewsProvider, PositiveU32,
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

mod rss;
mod transport;

pub use transport::{HttpRequest, HttpResponse, WallstreetCnTransport, RSS_URL};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RETURNED_ITEMS: u32 = 50;

/// Whether WallstreetCN is admitted to the public global-news capability.
///
/// The bounded production metadata and serial-load probes passed on 2026-07-26.
pub const GLOBAL_NEWS_ADMITTED: bool = true;

/// WallstreetCN adapter failures.
#[derive(Debug, Error)]
pub enum WallstreetCnError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("WallstreetCN RSS decoding failed: {0}")]
    Decode(String),
    #[error("WallstreetCN RSS protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

/// Read-only client for the exact public WallstreetCN RSS endpoint.
#[derive(Clone)]
pub struct WallstreetCnClient {
    transport: Arc<dyn WallstreetCnTransport>,
    minimum_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
}

impl std::fmt::Debug for WallstreetCnClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WallstreetCnClient")
            .finish_non_exhaustive()
    }
}

impl WallstreetCnClient {
    /// Creates a client with the default ten-second transport timeout.
    pub fn new() -> Result<Self, WallstreetCnError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    /// Creates a client with a timeout from one through sixty seconds.
    pub fn with_timeout(timeout: Duration) -> Result<Self, WallstreetCnError> {
        Ok(Self::from_parts(
            Arc::new(transport::HttpsTransport::new(timeout)?),
            MINIMUM_REQUEST_INTERVAL,
        ))
    }

    /// Creates a client over an injected bounded transport.
    pub fn with_transport(transport: impl WallstreetCnTransport + 'static) -> Self {
        Self::from_parts(Arc::new(transport), MINIMUM_REQUEST_INTERVAL)
    }

    pub const fn content_capabilities() -> ContentCapabilities {
        ContentCapabilities {
            instrument_news: false,
            global_news: GLOBAL_NEWS_ADMITTED,
            announcements: false,
            announcement_discovery: false,
            investor_questions: false,
        }
    }

    /// Fetches and validates the public feed for explicit diagnostics.
    pub fn probe_global_news(
        &self,
        limit: PositiveU32,
    ) -> Result<DataBatch<NewsItem>, WallstreetCnError> {
        validate_returned_limit(limit.get())?;
        let response = self.execute(&transport::build_request())?;
        let observed_at = now()?;
        rss::parse_response(response.body(), limit.get(), &observed_at)
    }

    fn from_parts(transport: Arc<dyn WallstreetCnTransport>, minimum_interval: Duration) -> Self {
        Self {
            transport,
            minimum_interval,
            request_gate: Arc::new(Mutex::new(None)),
        }
    }

    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, WallstreetCnError> {
        let mut last_started = self
            .request_gate
            .lock()
            .map_err(|_| WallstreetCnError::Transport("request gate lock poisoned".into()))?;
        if let Some(previous) = *last_started {
            let elapsed = previous.elapsed();
            if elapsed < self.minimum_interval {
                thread::sleep(self.minimum_interval - elapsed);
            }
        }
        *last_started = Some(Instant::now());
        let response = self.transport.get(request);
        drop(last_started);
        let response = response?;
        transport::validate_response(&response)?;
        Ok(response)
    }
}

impl NewsProvider for WallstreetCnClient {
    type Error = WallstreetCnError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(WallstreetCnError::Unsupported(
            "WallstreetCN RSS does not expose a verified instrument/date filter".into(),
        ))
    }

    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        if GLOBAL_NEWS_ADMITTED {
            self.probe_global_news(limit)
        } else {
            Err(WallstreetCnError::Unsupported(
                "WallstreetCN global news is pending bounded live admission; use probe_global_news for explicit diagnostics"
                    .into(),
            ))
        }
    }
}

fn validate_timeout(timeout: Duration) -> Result<(), WallstreetCnError> {
    if (Duration::from_secs(1)..=Duration::from_secs(60)).contains(&timeout) {
        Ok(())
    } else {
        Err(WallstreetCnError::InvalidRequest(
            "timeout must be between 1 and 60 seconds".into(),
        ))
    }
}

fn validate_returned_limit(limit: u32) -> Result<(), WallstreetCnError> {
    if (1..=MAX_RETURNED_ITEMS).contains(&limit) {
        Ok(())
    } else {
        Err(WallstreetCnError::InvalidRequest(format!(
            "WallstreetCN global-news limit must be between 1 and {MAX_RETURNED_ITEMS}"
        )))
    }
}

fn now() -> Result<String, WallstreetCnError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| WallstreetCnError::Transport(format!("local observation clock: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Condvar, Mutex};

    #[derive(Default)]
    struct BlockingState {
        calls: usize,
        starts: Vec<Instant>,
        release_first: bool,
    }

    #[derive(Clone, Default)]
    struct BlockingTransport {
        state: Arc<(Mutex<BlockingState>, Condvar)>,
    }

    impl BlockingTransport {
        fn wait_for_calls(&self, expected: usize) {
            let (state, changed) = &*self.state;
            let mut state = state.lock().expect("blocking fixture lock");
            while state.calls < expected {
                state = changed.wait(state).expect("blocking fixture wait");
            }
        }

        fn release_first(&self) {
            let (state, changed) = &*self.state;
            let mut state = state.lock().expect("blocking fixture lock");
            state.release_first = true;
            changed.notify_all();
        }

        fn snapshot(&self) -> (usize, Vec<Instant>) {
            let (state, _) = &*self.state;
            let state = state.lock().expect("blocking fixture lock");
            (state.calls, state.starts.clone())
        }
    }

    impl WallstreetCnTransport for BlockingTransport {
        fn get(&self, _request: &HttpRequest) -> Result<HttpResponse, WallstreetCnError> {
            let (state, changed) = &*self.state;
            let mut state = state.lock().expect("blocking fixture lock");
            state.calls += 1;
            state.starts.push(Instant::now());
            changed.notify_all();
            if state.calls == 1 {
                while !state.release_first {
                    state = changed.wait(state).expect("blocking fixture wait");
                }
            }
            Ok(valid_response())
        }
    }

    fn valid_response() -> HttpResponse {
        HttpResponse::new(
            RSS_URL,
            Some("text/html; charset=UTF-8".into()),
            r#"<?xml version="1.0"?><rss version="2.0"><channel><title>华尔街见闻</title></channel></rss>"#
                .as_bytes()
                .to_vec(),
        )
    }

    #[test]
    fn request_timeout_and_limit_bounds_are_strict() {
        assert!(validate_timeout(Duration::from_secs(1)).is_ok());
        assert!(validate_timeout(Duration::from_secs(60)).is_ok());
        assert!(validate_timeout(Duration::ZERO).is_err());
        assert!(validate_timeout(Duration::from_secs(61)).is_err());
        assert!(validate_returned_limit(1).is_ok());
        assert!(validate_returned_limit(50).is_ok());
        assert!(validate_returned_limit(0).is_err());
        assert!(validate_returned_limit(51).is_err());
    }

    #[test]
    fn injected_responses_are_revalidated() {
        struct RedirectedTransport;

        impl WallstreetCnTransport for RedirectedTransport {
            fn get(&self, _request: &HttpRequest) -> Result<HttpResponse, WallstreetCnError> {
                Ok(HttpResponse::new(
                    "https://wallstreetcn.com/rss.xml",
                    Some("application/rss+xml".into()),
                    b"<rss/>".to_vec(),
                ))
            }
        }

        let client =
            WallstreetCnClient::from_parts(Arc::new(RedirectedTransport), Duration::from_millis(1));
        assert!(matches!(
            client.execute(&transport::build_request()),
            Err(WallstreetCnError::Protocol(_))
        ));
    }

    #[test]
    fn clones_share_a_gate_held_through_the_first_response() {
        let transport = BlockingTransport::default();
        let client =
            WallstreetCnClient::from_parts(Arc::new(transport.clone()), Duration::from_millis(40));
        let first = {
            let client = client.clone();
            thread::spawn(move || client.execute(&transport::build_request()))
        };
        transport.wait_for_calls(1);
        let second = {
            let client = client.clone();
            thread::spawn(move || client.execute(&transport::build_request()))
        };
        thread::sleep(Duration::from_millis(10));
        assert_eq!(transport.snapshot().0, 1);
        transport.release_first();
        first.join().expect("first request thread").unwrap();
        second.join().expect("second request thread").unwrap();

        let (calls, starts) = transport.snapshot();
        assert_eq!(calls, 2);
        assert!(starts[1].duration_since(starts[0]) >= Duration::from_millis(40));
    }
}
