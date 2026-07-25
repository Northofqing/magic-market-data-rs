#![forbid(unsafe_code)]
//! Bounded metadata-only adapter for official Yonhap Chinese RSS feeds.

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_RETURNED_ITEMS: u32 = 50;
const MAX_SOURCE_ITEMS: usize = 100;

/// One official simplified-Chinese Yonhap RSS channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YonhapChannel {
    Rolling,
    Politics,
    Economy,
    Society,
    CultureSports,
    NorthKorea,
    ChinaKorea,
}

impl YonhapChannel {
    pub const ALL: [Self; 7] = [
        Self::Rolling,
        Self::Politics,
        Self::Economy,
        Self::Society,
        Self::CultureSports,
        Self::NorthKorea,
        Self::ChinaKorea,
    ];

    pub const fn endpoint(self) -> &'static str {
        match self {
            Self::Rolling => "https://cn.yna.co.kr/RSS/news.xml",
            Self::Politics => "https://cn.yna.co.kr/RSS/politics.xml",
            Self::Economy => "https://cn.yna.co.kr/RSS/economy.xml",
            Self::Society => "https://cn.yna.co.kr/RSS/society.xml",
            Self::CultureSports => "https://cn.yna.co.kr/RSS/culture-sports.xml",
            Self::NorthKorea => "https://cn.yna.co.kr/RSS/nk.xml",
            Self::ChinaKorea => "https://cn.yna.co.kr/RSS/china-relationship.xml",
        }
    }

    pub const fn topic(self) -> &'static str {
        match self {
            Self::Rolling => "滚动",
            Self::Politics => "政治",
            Self::Economy => "经济",
            Self::Society => "社会",
            Self::CultureSports => "文化体育",
            Self::NorthKorea => "朝鲜",
            Self::ChinaKorea => "中韩关系",
        }
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Rolling => "rolling",
            Self::Politics => "politics",
            Self::Economy => "economy",
            Self::Society => "society",
            Self::CultureSports => "culture-sports",
            Self::NorthKorea => "north-korea",
            Self::ChinaKorea => "china-korea",
        }
    }
}

/// Yonhap adapter failures.
#[derive(Debug, Error)]
pub enum YonhapError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("Yonhap RSS decoding failed: {0}")]
    Decode(String),
    #[error("Yonhap RSS protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

/// Immutable request passed to an injected transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    url: String,
    headers: Vec<(String, String)>,
}

impl HttpRequest {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }
}

/// Complete bounded response returned by a transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    final_url: String,
    content_type: Option<String>,
    body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(final_url: impl Into<String>, content_type: Option<String>, body: Vec<u8>) -> Self {
        Self {
            final_url: final_url.into(),
            content_type,
            body,
        }
    }

    pub fn final_url(&self) -> &str {
        &self.final_url
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

/// Bounded transport seam used by production and deterministic fixtures.
pub trait YonhapTransport: Send + Sync {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, YonhapError>;
}

#[derive(Clone)]
struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    fn new(timeout: Duration) -> Result<Self, YonhapError> {
        validate_timeout(timeout)?;
        Ok(Self {
            agent: ureq::AgentBuilder::new()
                .timeout_connect(timeout)
                .timeout_read(timeout)
                .timeout_write(timeout)
                .redirects(0)
                .build(),
        })
    }
}

impl YonhapTransport for HttpsTransport {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, YonhapError> {
        ensure_official_feed_url(request.url())?;
        let mut call = self.agent.get(request.url());
        for (name, value) in request.headers() {
            call = call.set(name, value);
        }
        let response = match call.call() {
            Ok(response) => response,
            Err(ureq::Error::Status(status, _)) => {
                return Err(YonhapError::Transport(format!(
                    "unexpected HTTP status {status}"
                )));
            }
            Err(error) => return Err(YonhapError::Transport(error.to_string())),
        };
        ensure_success_status(response.status())?;
        let final_url = response.get_url().to_owned();
        ensure_official_final_url(&final_url)?;
        let content_type = response.header("Content-Type").map(str::to_owned);
        ensure_xml_content_type(content_type.as_deref())?;
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|error| YonhapError::Transport(error.to_string()))?;
        ensure_body_size(&body)?;
        Ok(HttpResponse::new(final_url, content_type, body))
    }
}

/// Read-only client for official Yonhap Chinese RSS metadata.
#[derive(Clone)]
pub struct YonhapClient {
    channel: YonhapChannel,
    transport: Arc<dyn YonhapTransport>,
    minimum_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
}

impl std::fmt::Debug for YonhapClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("YonhapClient")
            .field("channel", &self.channel)
            .finish_non_exhaustive()
    }
}

impl YonhapClient {
    pub fn new() -> Result<Self, YonhapError> {
        Self::for_channel(YonhapChannel::Rolling)
    }

    pub fn for_channel(channel: YonhapChannel) -> Result<Self, YonhapError> {
        Self::for_channel_with_timeout(channel, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, YonhapError> {
        Self::for_channel_with_timeout(YonhapChannel::Rolling, timeout)
    }

    pub fn with_transport(transport: impl YonhapTransport + 'static) -> Self {
        Self::from_parts(
            YonhapChannel::Rolling,
            Arc::new(transport),
            MINIMUM_REQUEST_INTERVAL,
        )
    }

    pub fn with_channel_and_transport(
        channel: YonhapChannel,
        transport: impl YonhapTransport + 'static,
    ) -> Self {
        Self::from_parts(channel, Arc::new(transport), MINIMUM_REQUEST_INTERVAL)
    }

    pub const fn channel(&self) -> YonhapChannel {
        self.channel
    }

    fn for_channel_with_timeout(
        channel: YonhapChannel,
        timeout: Duration,
    ) -> Result<Self, YonhapError> {
        Ok(Self::from_parts(
            channel,
            Arc::new(HttpsTransport::new(timeout)?),
            MINIMUM_REQUEST_INTERVAL,
        ))
    }

    fn from_parts(
        channel: YonhapChannel,
        transport: Arc<dyn YonhapTransport>,
        minimum_interval: Duration,
    ) -> Self {
        Self {
            channel,
            transport,
            minimum_interval,
            request_gate: Arc::new(Mutex::new(None)),
        }
    }

    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, YonhapError> {
        let mut last_started = self
            .request_gate
            .lock()
            .map_err(|_| YonhapError::Transport("request gate lock poisoned".into()))?;
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
        validate_response(&response)?;
        Ok(response)
    }
}

fn validate_timeout(timeout: Duration) -> Result<(), YonhapError> {
    if (Duration::from_secs(1)..=Duration::from_secs(60)).contains(&timeout) {
        Ok(())
    } else {
        Err(YonhapError::InvalidRequest(
            "timeout must be between 1 and 60 seconds".into(),
        ))
    }
}

fn validate_returned_limit(limit: u32) -> Result<(), YonhapError> {
    if (1..=MAX_RETURNED_ITEMS).contains(&limit) {
        Ok(())
    } else {
        Err(YonhapError::InvalidRequest(format!(
            "Yonhap global-news limit must be between 1 and {MAX_RETURNED_ITEMS}"
        )))
    }
}

fn build_request(channel: YonhapChannel) -> HttpRequest {
    HttpRequest {
        url: channel.endpoint().to_owned(),
        headers: vec![
            (
                "Accept".into(),
                "application/rss+xml, application/xml;q=0.9, text/xml;q=0.8".into(),
            ),
            ("User-Agent".into(), "magic-yonhap-rs/0.2".into()),
        ],
    }
}

fn ensure_official_feed_url(url: &str) -> Result<(), YonhapError> {
    if YonhapChannel::ALL
        .into_iter()
        .any(|channel| url == channel.endpoint())
        && !url.chars().any(char::is_control)
    {
        Ok(())
    } else {
        Err(YonhapError::InvalidRequest(
            "Yonhap transport permits only the seven exact official Chinese RSS endpoints".into(),
        ))
    }
}

fn ensure_success_status(status: u16) -> Result<(), YonhapError> {
    if status == 200 {
        Ok(())
    } else {
        Err(YonhapError::Transport(format!(
            "unexpected HTTP status {status}"
        )))
    }
}

fn ensure_official_final_url(url: &str) -> Result<(), YonhapError> {
    ensure_official_feed_url(url).map_err(|_| {
        YonhapError::Protocol(format!(
            "response final URL is not an official Yonhap Chinese RSS endpoint: {url}"
        ))
    })
}

fn ensure_xml_content_type(content_type: Option<&str>) -> Result<(), YonhapError> {
    let accepted = content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .map(str::trim)
            .is_some_and(|media_type| {
                ["application/rss+xml", "application/xml", "text/xml"]
                    .into_iter()
                    .any(|allowed| media_type.eq_ignore_ascii_case(allowed))
            })
    });
    if accepted {
        Ok(())
    } else {
        Err(YonhapError::Protocol(format!(
            "expected an XML response, received content type {content_type:?}"
        )))
    }
}

fn ensure_body_size(body: &[u8]) -> Result<(), YonhapError> {
    if body.len() <= MAX_RESPONSE_BYTES {
        Ok(())
    } else {
        Err(YonhapError::Protocol(format!(
            "response exceeds {MAX_RESPONSE_BYTES} bytes"
        )))
    }
}

fn validate_response(response: &HttpResponse) -> Result<(), YonhapError> {
    ensure_official_final_url(response.final_url())?;
    ensure_xml_content_type(response.content_type())?;
    ensure_body_size(response.body())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};
    use std::time::Duration;

    struct StaticTransport {
        response: HttpResponse,
        calls: Arc<AtomicUsize>,
    }

    impl YonhapTransport for StaticTransport {
        fn get(&self, _request: &HttpRequest) -> Result<HttpResponse, YonhapError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.response.clone())
        }
    }

    fn static_client(response: HttpResponse) -> (YonhapClient, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let client = YonhapClient::from_parts(
            YonhapChannel::Rolling,
            Arc::new(StaticTransport {
                response,
                calls: Arc::clone(&calls),
            }),
            Duration::ZERO,
        );
        (client, calls)
    }

    fn valid_response() -> HttpResponse {
        HttpResponse::new(
            YonhapChannel::Rolling.endpoint(),
            Some("application/rss+xml; charset=utf-8".into()),
            b"<rss/>".to_vec(),
        )
    }

    #[test]
    fn channel_and_request_channel_matrix_is_closed() {
        let cases = [
            (
                YonhapChannel::Rolling,
                "https://cn.yna.co.kr/RSS/news.xml",
                "滚动",
            ),
            (
                YonhapChannel::Politics,
                "https://cn.yna.co.kr/RSS/politics.xml",
                "政治",
            ),
            (
                YonhapChannel::Economy,
                "https://cn.yna.co.kr/RSS/economy.xml",
                "经济",
            ),
            (
                YonhapChannel::Society,
                "https://cn.yna.co.kr/RSS/society.xml",
                "社会",
            ),
            (
                YonhapChannel::CultureSports,
                "https://cn.yna.co.kr/RSS/culture-sports.xml",
                "文化体育",
            ),
            (
                YonhapChannel::NorthKorea,
                "https://cn.yna.co.kr/RSS/nk.xml",
                "朝鲜",
            ),
            (
                YonhapChannel::ChinaKorea,
                "https://cn.yna.co.kr/RSS/china-relationship.xml",
                "中韩关系",
            ),
        ];
        for (channel, endpoint, topic) in cases {
            assert_eq!(channel.endpoint(), endpoint);
            assert_eq!(channel.topic(), topic);
        }
    }

    #[test]
    fn channel_and_request_default_and_selected_channels_are_explicit() {
        assert_eq!(
            YonhapClient::new().unwrap().channel(),
            YonhapChannel::Rolling
        );
        assert_eq!(
            YonhapClient::for_channel(YonhapChannel::Economy)
                .unwrap()
                .channel(),
            YonhapChannel::Economy
        );
    }

    #[test]
    fn channel_and_request_timeout_and_limit_bounds_are_checked() {
        assert!(YonhapClient::with_timeout(Duration::ZERO).is_err());
        assert!(YonhapClient::with_timeout(Duration::from_secs(1)).is_ok());
        assert!(YonhapClient::with_timeout(Duration::from_secs(60)).is_ok());
        assert!(YonhapClient::with_timeout(Duration::from_secs(61)).is_err());
        assert!(validate_returned_limit(50).is_ok());
        assert!(validate_returned_limit(51).is_err());
    }

    #[test]
    fn channel_and_request_headers_are_minimal_and_stable() {
        let request = build_request(YonhapChannel::Economy);
        assert_eq!(request.url(), "https://cn.yna.co.kr/RSS/economy.xml");
        assert_eq!(
            request.headers(),
            &[
                (
                    "Accept".to_owned(),
                    "application/rss+xml, application/xml;q=0.9, text/xml;q=0.8".to_owned(),
                ),
                ("User-Agent".to_owned(), "magic-yonhap-rs/0.2".to_owned(),),
            ]
        );
    }

    #[test]
    fn channel_and_request_feed_allowlist_rejects_url_confusion() {
        for channel in YonhapChannel::ALL {
            assert!(ensure_official_feed_url(channel.endpoint()).is_ok());
        }
        for invalid in [
            "http://cn.yna.co.kr/RSS/news.xml",
            "https://user@cn.yna.co.kr/RSS/news.xml",
            "https://cn.yna.co.kr:444/RSS/news.xml",
            "https://cn.yna.co.kr.example/RSS/news.xml",
            "https://cn.yna.co.kr/RSS/news.xml?x=1",
            "https://cn.yna.co.kr/RSS/news.xml#fragment",
            "https://cn.yna.co.kr/RSS/unknown.xml",
            "https://cn.yna.co.kr//RSS/news.xml",
            "https://cn.yna.co.kr/RSS/news.xml\n",
        ] {
            assert!(
                ensure_official_feed_url(invalid).is_err(),
                "unexpectedly accepted {invalid:?}"
            );
        }
    }

    #[test]
    fn channel_and_request_xml_media_types_are_exact() {
        for valid in [
            "application/rss+xml",
            "application/rss+xml; charset=utf-8",
            "APPLICATION/XML",
            "text/xml ; charset=UTF-8",
        ] {
            assert!(ensure_xml_content_type(Some(valid)).is_ok());
        }
        for invalid in [
            None,
            Some("text/html"),
            Some("application/json"),
            Some("application/xmlx"),
        ] {
            assert!(ensure_xml_content_type(invalid).is_err());
        }
    }

    #[test]
    fn transport_revalidates_injected_response_bounds() {
        let (wrong_url, calls) = static_client(HttpResponse::new(
            "https://example.com/RSS/news.xml",
            Some("application/xml".into()),
            b"<rss/>".to_vec(),
        ));
        assert!(matches!(
            wrong_url.execute(&build_request(YonhapChannel::Rolling)),
            Err(YonhapError::Protocol(message)) if message.contains("final URL")
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let (wrong_mime, _) = static_client(HttpResponse::new(
            YonhapChannel::Rolling.endpoint(),
            Some("text/html".into()),
            b"<rss/>".to_vec(),
        ));
        assert!(matches!(
            wrong_mime.execute(&build_request(YonhapChannel::Rolling)),
            Err(YonhapError::Protocol(message)) if message.contains("content type")
        ));

        let (oversized, _) = static_client(HttpResponse::new(
            YonhapChannel::Rolling.endpoint(),
            Some("application/xml".into()),
            vec![b'x'; MAX_RESPONSE_BYTES + 1],
        ));
        assert!(matches!(
            oversized.execute(&build_request(YonhapChannel::Rolling)),
            Err(YonhapError::Protocol(message)) if message.contains("exceeds")
        ));
    }

    #[test]
    fn transport_status_failures_remain_transport_errors() {
        assert!(ensure_success_status(200).is_ok());
        assert!(matches!(
            ensure_success_status(404),
            Err(YonhapError::Transport(message)) if message.contains("404")
        ));
    }

    #[derive(Default)]
    struct BlockingState {
        calls: usize,
        starts: Vec<Instant>,
        release_first: bool,
    }

    struct BlockingTransport {
        state: Arc<(Mutex<BlockingState>, Condvar)>,
    }

    impl YonhapTransport for BlockingTransport {
        fn get(&self, _request: &HttpRequest) -> Result<HttpResponse, YonhapError> {
            let (lock, signal) = &*self.state;
            let mut state = lock.lock().unwrap();
            state.calls += 1;
            state.starts.push(Instant::now());
            signal.notify_all();
            if state.calls == 1 {
                while !state.release_first {
                    state = signal.wait(state).unwrap();
                }
            }
            Ok(valid_response())
        }
    }

    #[test]
    fn transport_gate_is_clone_shared_and_held_through_response() {
        let state = Arc::new((Mutex::new(BlockingState::default()), Condvar::new()));
        let client = YonhapClient::from_parts(
            YonhapChannel::Rolling,
            Arc::new(BlockingTransport {
                state: Arc::clone(&state),
            }),
            Duration::from_millis(40),
        );
        let first = client.clone();
        let first_thread =
            std::thread::spawn(move || first.execute(&build_request(YonhapChannel::Rolling)));

        let (lock, signal) = &*state;
        let mut current = lock.lock().unwrap();
        while current.calls == 0 {
            current = signal.wait(current).unwrap();
        }
        drop(current);

        let second = client.clone();
        let second_thread =
            std::thread::spawn(move || second.execute(&build_request(YonhapChannel::Rolling)));
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(lock.lock().unwrap().calls, 1);

        let mut current = lock.lock().unwrap();
        current.release_first = true;
        signal.notify_all();
        drop(current);

        assert!(first_thread.join().unwrap().is_ok());
        assert!(second_thread.join().unwrap().is_ok());
        let current = lock.lock().unwrap();
        assert_eq!(current.calls, 2);
        assert!(current.starts[1].duration_since(current.starts[0]) >= Duration::from_millis(40));
    }
}
