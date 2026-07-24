#![forbid(unsafe_code)]
//! Bounded read-only adapter for native financial news on The Paper.

use magic_market_core::{
    ContentCapabilities, DataBatch, HttpsUrl, InstrumentDateRangeRequest, NewsItem, NewsProvider,
    NonEmptyText, PositiveU32, Provenance, ProviderId, SourceEvidence,
};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const ENDPOINT: &str = "https://www.thepaper.cn/channel_25951";
const CHANNEL_ID: &str = "25951";
const NEXT_DATA_OPEN: &[u8] = b"<script id=\"__NEXT_DATA__\" type=\"application/json\">";
const NEXT_DATA_CLOSE: &[u8] = b"</script>";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_PAGE_SIZE: u32 = 20;
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

/// The Paper adapter failures. Upstream contract changes remain explicit.
#[derive(Debug, Error)]
pub enum ThePaperError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("The Paper response decoding failed: {0}")]
    Decode(String),
    #[error("The Paper protocol error: {0}")]
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

/// Bounded transport seam used by production and deterministic fixtures.
pub trait ThePaperTransport: Send + Sync {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, ThePaperError>;
}

#[derive(Clone)]
struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    fn new(timeout: Duration) -> Result<Self, ThePaperError> {
        if timeout.is_zero() {
            return Err(ThePaperError::InvalidRequest(
                "timeout must be greater than zero".into(),
            ));
        }
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

impl ThePaperTransport for HttpsTransport {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, ThePaperError> {
        ensure_official_url(request.url())?;
        let mut call = self.agent.get(request.url());
        for (name, value) in request.headers() {
            call = call.set(name, value);
        }
        let response = call
            .call()
            .map_err(|error| ThePaperError::Transport(error.to_string()))?;
        if response.status() != 200 {
            return Err(ThePaperError::Transport(format!(
                "unexpected HTTP status {}",
                response.status()
            )));
        }
        ensure_html_content_type(response.header("Content-Type"))?;
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|error| ThePaperError::Transport(error.to_string()))?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(ThePaperError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        Ok(body)
    }
}

/// Read-only client for The Paper's native finance-channel articles.
#[derive(Clone)]
pub struct ThePaperClient {
    transport: Arc<dyn ThePaperTransport>,
    minimum_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
}

impl std::fmt::Debug for ThePaperClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ThePaperClient")
            .finish_non_exhaustive()
    }
}

impl ThePaperClient {
    pub fn new() -> Result<Self, ThePaperError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, ThePaperError> {
        Ok(Self::from_parts(
            Arc::new(HttpsTransport::new(timeout)?),
            MINIMUM_REQUEST_INTERVAL,
        ))
    }

    pub fn with_transport(transport: impl ThePaperTransport + 'static) -> Self {
        Self::from_parts(Arc::new(transport), MINIMUM_REQUEST_INTERVAL)
    }

    fn from_parts(transport: Arc<dyn ThePaperTransport>, minimum_interval: Duration) -> Self {
        Self {
            transport,
            minimum_interval,
            request_gate: Arc::new(Mutex::new(None)),
        }
    }

    pub const fn content_capabilities() -> ContentCapabilities {
        ContentCapabilities {
            instrument_news: false,
            global_news: true,
            announcements: false,
            investor_questions: false,
        }
    }

    fn fetch_global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, ThePaperError> {
        if limit.get() > MAX_PAGE_SIZE {
            return Err(ThePaperError::InvalidRequest(format!(
                "The Paper finance limit must be at most {MAX_PAGE_SIZE}"
            )));
        }
        let request = build_request();
        let body = self.execute(&request)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(ThePaperError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        let observed_at = now()?;
        parse_response(&body, limit.get(), &observed_at)
    }

    fn execute(&self, request: &HttpRequest) -> Result<Vec<u8>, ThePaperError> {
        let mut last_started = self
            .request_gate
            .lock()
            .map_err(|_| ThePaperError::Transport("request gate lock poisoned".into()))?;
        if let Some(previous) = *last_started {
            let elapsed = previous.elapsed();
            if elapsed < self.minimum_interval {
                thread::sleep(self.minimum_interval - elapsed);
            }
        }
        *last_started = Some(Instant::now());
        let result = self.transport.get(request);
        drop(last_started);
        result
    }
}

impl NewsProvider for ThePaperClient {
    type Error = ThePaperError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(ThePaperError::Unsupported(
            "The Paper finance channel does not expose a verified instrument/date filter".into(),
        ))
    }

    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        self.fetch_global_news(limit)
    }
}

fn ensure_official_url(url: &str) -> Result<(), ThePaperError> {
    if url != ENDPOINT || url.chars().any(char::is_control) {
        return Err(ThePaperError::InvalidRequest(format!(
            "The Paper transport only permits {ENDPOINT}"
        )));
    }
    Ok(())
}

fn ensure_html_content_type(content_type: Option<&str>) -> Result<(), ThePaperError> {
    if content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("text/html"))
    }) {
        Ok(())
    } else {
        Err(ThePaperError::Protocol(format!(
            "expected an HTML response, received content type {content_type:?}"
        )))
    }
}

fn build_request() -> HttpRequest {
    HttpRequest {
        url: ENDPOINT.into(),
        headers: vec![
            (
                "Accept".into(),
                "text/html,application/xhtml+xml;q=0.9".into(),
            ),
            ("User-Agent".into(), "magic-thepaper-rs/0.2".into()),
        ],
    }
}

fn parse_response(
    body: &[u8],
    limit: u32,
    observed_at: &str,
) -> Result<DataBatch<NewsItem>, ThePaperError> {
    let json = extract_next_data(body)?;
    let root: Value = serde_json::from_slice(json)
        .map_err(|error| ThePaperError::Decode(format!("__NEXT_DATA__ JSON: {error}")))?;
    if root.get("page").and_then(Value::as_str) != Some("/channel/[id]") {
        return Err(ThePaperError::Protocol(
            "__NEXT_DATA__ page is not /channel/[id]".into(),
        ));
    }
    let page_props = root
        .pointer("/props/pageProps")
        .and_then(Value::as_object)
        .ok_or_else(|| ThePaperError::Protocol("props.pageProps must be an object".into()))?;
    if page_props.get("id").and_then(Value::as_str) != Some(CHANNEL_ID) {
        return Err(ThePaperError::Protocol(format!(
            "The Paper page is not finance channel {CHANNEL_ID}"
        )));
    }
    let response = page_props
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| ThePaperError::Protocol("pageProps.data must be an object".into()))?;
    let code = response
        .get("code")
        .and_then(Value::as_i64)
        .ok_or_else(|| ThePaperError::Protocol("finance payload code must be an integer".into()))?;
    if code != 200 {
        let description = response
            .get("desc")
            .and_then(Value::as_str)
            .unwrap_or("unknown The Paper error");
        return Err(ThePaperError::Protocol(format!(
            "The Paper returned code {code}: {description}"
        )));
    }
    let rows = response
        .get("data")
        .and_then(Value::as_object)
        .and_then(|data| data.get("list"))
        .and_then(Value::as_array)
        .ok_or_else(|| ThePaperError::Protocol("finance data.list must be an array".into()))?;
    if rows.len() > MAX_PAGE_SIZE as usize {
        return Err(ThePaperError::Protocol(format!(
            "The Paper returned {} rows, maximum is {MAX_PAGE_SIZE}",
            rows.len()
        )));
    }
    if rows.is_empty() {
        return Err(ThePaperError::Protocol(
            "The Paper returned an empty finance list".into(),
        ));
    }

    let batch_id = format!("thepaper:{observed_at}:finance");
    let mut seen = HashSet::with_capacity(rows.len());
    let mut parsed = Vec::with_capacity(rows.len());
    for row in rows {
        let object = row
            .as_object()
            .ok_or_else(|| ThePaperError::Protocol("finance row must be an object".into()))?;
        let item_id = required_id(object.get("contId"))?;
        if !seen.insert(item_id.clone()) {
            return Err(ThePaperError::Protocol(format!(
                "duplicate The Paper contId {item_id}"
            )));
        }
        if !is_native(object)? {
            continue;
        }
        parsed.push(parse_item(object, item_id, observed_at, &batch_id)?);
    }
    if parsed.is_empty() {
        return Err(ThePaperError::Protocol(
            "The Paper returned no eligible native finance rows".into(),
        ));
    }
    parsed.sort_by(|left, right| right.published_at.as_str().cmp(left.published_at.as_str()));
    parsed.truncate(limit as usize);

    let source_at = parsed
        .first()
        .map(|item| item.published_at.as_str())
        .ok_or_else(|| ThePaperError::Protocol("latest source time is missing".into()))?;
    let provenance = Provenance::new("thepaper-finance-v1", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(parsed, provenance))
}

fn extract_next_data(body: &[u8]) -> Result<&[u8], ThePaperError> {
    let start = find_subslice(body, NEXT_DATA_OPEN)
        .ok_or_else(|| ThePaperError::Protocol("__NEXT_DATA__ script is missing".into()))?;
    let content_start = start + NEXT_DATA_OPEN.len();
    if find_subslice(&body[content_start..], NEXT_DATA_OPEN).is_some() {
        return Err(ThePaperError::Protocol(
            "multiple __NEXT_DATA__ scripts are not allowed".into(),
        ));
    }
    let relative_end = find_subslice(&body[content_start..], NEXT_DATA_CLOSE)
        .ok_or_else(|| ThePaperError::Protocol("__NEXT_DATA__ script is not closed".into()))?;
    let content = &body[content_start..content_start + relative_end];
    if content.is_empty() {
        return Err(ThePaperError::Protocol(
            "__NEXT_DATA__ script is empty".into(),
        ));
    }
    Ok(content)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn required_id(value: Option<&Value>) -> Result<String, ThePaperError> {
    let value = value
        .and_then(Value::as_str)
        .ok_or_else(|| ThePaperError::Protocol("contId must be a string".into()))?;
    if value.is_empty() || value.len() > 32 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ThePaperError::Protocol(
            "contId must be a non-empty ASCII integer".into(),
        ));
    }
    Ok(value.to_owned())
}

fn is_native(row: &Map<String, Value>) -> Result<bool, ThePaperError> {
    let forward = row
        .get("isOutForward")
        .and_then(Value::as_str)
        .ok_or_else(|| ThePaperError::Protocol("isOutForward must be a string".into()))?;
    let legacy_forward = row
        .get("isOutForword")
        .and_then(Value::as_str)
        .ok_or_else(|| ThePaperError::Protocol("isOutForword must be a string".into()))?;
    if forward != legacy_forward {
        return Err(ThePaperError::Protocol(
            "The Paper forward flags disagree".into(),
        ));
    }
    if !matches!(forward, "0" | "1") {
        return Err(ThePaperError::Protocol(
            "The Paper forward flags must be 0 or 1".into(),
        ));
    }
    let link = match row.get("link") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value.trim().is_empty() => None,
        Some(Value::String(value)) => Some(value.as_str()),
        Some(_) => {
            return Err(ThePaperError::Protocol(
                "The Paper link must be a string or null".into(),
            ))
        }
    };
    if forward == "0" {
        if link.is_some() {
            return Err(ThePaperError::Protocol(
                "native The Paper row unexpectedly has an external link".into(),
            ));
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

fn parse_item(
    row: &Map<String, Value>,
    item_id: String,
    observed_at: &str,
    batch_id: &str,
) -> Result<NewsItem, ThePaperError> {
    let title = required_text(row.get("name"), "name")?;
    let milliseconds = row
        .get("pubTimeLong")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| ThePaperError::Protocol("pubTimeLong must be positive".into()))?;
    let published_at = milliseconds_to_china_time(milliseconds)?;
    let evidence = SourceEvidence::new(ProviderId::ThePaper, observed_at, batch_id)?
        .with_source_at(published_at.clone())?;
    Ok(NewsItem {
        item_id: NonEmptyText::new(item_id.clone())?,
        title: NonEmptyText::new(title)?,
        summary: None,
        content: None,
        publisher: NonEmptyText::new("澎湃新闻")?,
        canonical_url: HttpsUrl::new(format!(
            "https://www.thepaper.cn/newsDetail_forward_{item_id}"
        ))?,
        published_at: NonEmptyText::new(published_at)?,
        instruments: Vec::new(),
        topics: parse_topics(row)?,
        language: NonEmptyText::new("zh-CN")?,
        evidence,
    })
}

fn parse_topics(row: &Map<String, Value>) -> Result<Vec<NonEmptyText>, ThePaperError> {
    let node = row
        .get("nodeInfo")
        .and_then(Value::as_object)
        .ok_or_else(|| ThePaperError::Protocol("nodeInfo must be an object".into()))?;
    let subsection = required_text(node.get("name"), "nodeInfo.name")?;
    let mut seen = HashSet::new();
    seen.insert(subsection.clone());
    let mut topics = vec![NonEmptyText::new(subsection)?];
    let tags = row
        .get("tagList")
        .and_then(Value::as_array)
        .ok_or_else(|| ThePaperError::Protocol("tagList must be an array".into()))?;
    for tag in tags {
        let object = tag
            .as_object()
            .ok_or_else(|| ThePaperError::Protocol("tagList row must be an object".into()))?;
        let topic = required_text(object.get("tag"), "tagList.tag")?;
        if seen.insert(topic.clone()) {
            topics.push(NonEmptyText::new(topic)?);
        }
    }
    Ok(topics)
}

fn required_text(value: Option<&Value>, field: &'static str) -> Result<String, ThePaperError> {
    value
        .and_then(Value::as_str)
        .map(normalized_text)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ThePaperError::Protocol(format!("{field} must be a non-empty string")))
}

fn normalized_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn milliseconds_to_china_time(milliseconds: i64) -> Result<String, ThePaperError> {
    let seconds = milliseconds.div_euclid(1_000);
    let shifted = seconds
        .checked_add(8 * 60 * 60)
        .ok_or_else(|| ThePaperError::Protocol("pubTimeLong overflow".into()))?;
    let days = shifted.div_euclid(86_400);
    let day_seconds = shifted.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days)?;
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    let second = day_seconds % 60;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}+08:00"
    ))
}

fn civil_from_days(days_since_epoch: i64) -> Result<(i64, i64, i64), ThePaperError> {
    let z = days_since_epoch
        .checked_add(719_468)
        .ok_or_else(|| ThePaperError::Protocol("pubTimeLong is out of range".into()))?;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    if !(1970..=9999).contains(&year) {
        return Err(ThePaperError::Protocol(
            "pubTimeLong is outside the supported range".into(),
        ));
    }
    Ok((year, month, day))
}

fn now() -> Result<String, ThePaperError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| {
            ThePaperError::Protocol(format!("system clock precedes UNIX epoch: {error}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Mutex};

    const FIXTURE: &str = r#"<!doctype html><html><head></head><body>
<script id="__NEXT_DATA__" type="application/json">{
  "page": "/channel/[id]",
  "query": {"id": "25951"},
  "props": {"pageProps": {
    "id": "25951",
    "data": {"code": 200, "desc": "成功", "data": {"list": [
      {
        "contId": "33653301",
        "name": "证监会相关新闻",
        "pubTimeLong": 1784894461587,
        "link": "",
        "isOutForward": "0",
        "isOutForword": "0",
        "nodeInfo": {"name": "牛市点线面"},
        "tagList": [{"tag": "证监会"}, {"tag": "证监会"}]
      },
      {
        "contId": "33650000",
        "name": "外部转载",
        "pubTimeLong": 1784904000000,
        "link": "https://example.com/external",
        "isOutForward": "1",
        "isOutForword": "1",
        "nodeInfo": {"name": "财经上下游"},
        "tagList": []
      },
      {
        "contId": "33654589",
        "name": "宁德时代上半年净利润同比增长",
        "pubTimeLong": 1784903331309,
        "link": null,
        "isOutForward": "0",
        "isOutForword": "0",
        "nodeInfo": {"name": "能见度"},
        "tagList": [{"tag": "宁德时代"}, {"tag": "业绩"}]
      }
    ]}}
  }}
}</script></body></html>"#;

    #[derive(Debug)]
    struct FixtureTransport {
        response: Vec<u8>,
        request: Mutex<Option<HttpRequest>>,
    }

    impl ThePaperTransport for FixtureTransport {
        fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, ThePaperError> {
            *self
                .request
                .lock()
                .map_err(|_| ThePaperError::Transport("fixture lock poisoned".into()))? =
                Some(request.clone());
            Ok(self.response.clone())
        }
    }

    #[derive(Debug)]
    struct BlockingTransport {
        response: Vec<u8>,
        starts: mpsc::Sender<Instant>,
        releases: Mutex<mpsc::Receiver<()>>,
    }

    impl ThePaperTransport for BlockingTransport {
        fn get(&self, _request: &HttpRequest) -> Result<Vec<u8>, ThePaperError> {
            self.starts
                .send(Instant::now())
                .map_err(|error| ThePaperError::Transport(error.to_string()))?;
            self.releases
                .lock()
                .map_err(|_| ThePaperError::Transport("release lock poisoned".into()))?
                .recv()
                .map_err(|error| ThePaperError::Transport(error.to_string()))?;
            Ok(self.response.clone())
        }
    }

    #[test]
    fn native_finance_rows_map_and_sort_all_contract_fields() {
        let client = ThePaperClient::with_transport(FixtureTransport {
            response: FIXTURE.as_bytes().to_vec(),
            request: Mutex::new(None),
        });
        let batch = client
            .global_news(PositiveU32::new(20).unwrap())
            .expect("fixture should parse");
        assert_eq!(batch.records().len(), 2);
        let item = &batch.records()[0];
        assert_eq!(item.item_id.as_str(), "33654589");
        assert_eq!(item.title.as_str(), "宁德时代上半年净利润同比增长");
        assert_eq!(item.publisher.as_str(), "澎湃新闻");
        assert_eq!(
            item.canonical_url.as_str(),
            "https://www.thepaper.cn/newsDetail_forward_33654589"
        );
        assert_eq!(item.published_at.as_str(), "2026-07-24T22:28:51+08:00");
        assert_eq!(item.topics.len(), 3);
        assert_eq!(item.evidence.provider(), ProviderId::ThePaper);
        assert_eq!(
            batch.provenance().source_at(),
            Some("2026-07-24T22:28:51+08:00")
        );
        assert_eq!(batch.records()[1].topics.len(), 2);
    }

    #[test]
    fn request_and_embedded_json_boundary_are_exact() {
        let request = build_request();
        assert_eq!(request.url(), ENDPOINT);
        assert!(ensure_official_url(request.url()).is_ok());
        assert!(ensure_official_url("https://www.thepaper.cn/channel_25950").is_err());
        assert!(extract_next_data(FIXTURE.as_bytes()).is_ok());
        assert!(extract_next_data(b"<html></html>").is_err());
        let duplicate = format!("{FIXTURE}{FIXTURE}");
        assert!(extract_next_data(duplicate.as_bytes()).is_err());
    }

    #[test]
    fn channel_status_duplicates_and_native_flags_are_strict() {
        let wrong_channel = FIXTURE.replace("\"id\": \"25951\"", "\"id\": \"25950\"");
        assert!(matches!(
            parse_response(wrong_channel.as_bytes(), 20, "observed"),
            Err(ThePaperError::Protocol(_))
        ));
        let bad_status = FIXTURE.replace("\"code\": 200", "\"code\": 500");
        assert!(matches!(
            parse_response(bad_status.as_bytes(), 20, "observed"),
            Err(ThePaperError::Protocol(_))
        ));
        let duplicate = FIXTURE.replace("\"contId\": \"33654589\"", "\"contId\": \"33653301\"");
        assert!(matches!(
            parse_response(duplicate.as_bytes(), 20, "observed"),
            Err(ThePaperError::Protocol(_))
        ));
        let inconsistent = FIXTURE.replace("\"isOutForword\": \"1\"", "\"isOutForword\": \"0\"");
        assert!(matches!(
            parse_response(inconsistent.as_bytes(), 20, "observed"),
            Err(ThePaperError::Protocol(_))
        ));
        let unknown = FIXTURE
            .replace("\"isOutForward\": \"1\"", "\"isOutForward\": \"2\"")
            .replace("\"isOutForword\": \"1\"", "\"isOutForword\": \"2\"");
        assert!(matches!(
            parse_response(unknown.as_bytes(), 20, "observed"),
            Err(ThePaperError::Protocol(_))
        ));
    }

    #[test]
    fn timestamp_fields_and_bounds_are_strict() {
        let bad_time = FIXTURE.replace("1784903331309", "-1");
        assert!(matches!(
            parse_response(bad_time.as_bytes(), 20, "observed"),
            Err(ThePaperError::Protocol(_))
        ));
        let missing_node = FIXTURE.replace("\"nodeInfo\": {\"name\": \"能见度\"},", "");
        assert!(matches!(
            parse_response(missing_node.as_bytes(), 20, "observed"),
            Err(ThePaperError::Protocol(_))
        ));
        let client = ThePaperClient::with_transport(FixtureTransport {
            response: FIXTURE.as_bytes().to_vec(),
            request: Mutex::new(None),
        });
        assert!(matches!(
            client.global_news(PositiveU32::new(21).unwrap()),
            Err(ThePaperError::InvalidRequest(_))
        ));
        assert!(matches!(
            ThePaperClient::with_timeout(Duration::ZERO),
            Err(ThePaperError::InvalidRequest(_))
        ));
    }

    #[test]
    fn clones_hold_the_shared_gate_through_response_completion() {
        let (starts_tx, starts_rx) = mpsc::channel();
        let (releases_tx, releases_rx) = mpsc::channel();
        let client = ThePaperClient::from_parts(
            Arc::new(BlockingTransport {
                response: FIXTURE.as_bytes().to_vec(),
                starts: starts_tx,
                releases: Mutex::new(releases_rx),
            }),
            Duration::ZERO,
        );
        let first = client.clone();
        let first_handle = thread::spawn(move || first.global_news(PositiveU32::new(1).unwrap()));
        starts_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first request should start");
        let second = client.clone();
        let second_handle = thread::spawn(move || second.global_news(PositiveU32::new(1).unwrap()));
        assert!(starts_rx.recv_timeout(Duration::from_millis(50)).is_err());
        releases_tx.send(()).unwrap();
        starts_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second request should start after first completes");
        releases_tx.send(()).unwrap();
        first_handle.join().unwrap().unwrap();
        second_handle.join().unwrap().unwrap();
    }

    #[test]
    fn response_size_and_content_type_are_bounded() {
        assert!(ensure_html_content_type(Some("text/html; charset=utf-8")).is_ok());
        assert!(ensure_html_content_type(Some("text/html-invalid")).is_err());
        assert!(ensure_html_content_type(Some("application/json")).is_err());
        let client = ThePaperClient::with_transport(FixtureTransport {
            response: vec![b'x'; MAX_RESPONSE_BYTES + 1],
            request: Mutex::new(None),
        });
        assert!(matches!(
            client.global_news(PositiveU32::new(1).unwrap()),
            Err(ThePaperError::Protocol(_))
        ));
    }

    #[test]
    fn injected_transports_keep_production_pacing() {
        let client = ThePaperClient::with_transport(FixtureTransport {
            response: FIXTURE.as_bytes().to_vec(),
            request: Mutex::new(None),
        });
        assert_eq!(client.minimum_interval, MINIMUM_REQUEST_INTERVAL);
    }
}
