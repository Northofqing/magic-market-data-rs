#![forbid(unsafe_code)]
//! Authenticated read-only iWencai semantic-search adapter.

use magic_market_core::{
    DataBatch, HttpsUrl, LoadProbeSnapshot, NonEmptyText, ProbeRequestTracker, Provenance,
    ProviderId, ResearchCapabilities, SemanticChannel, SemanticSearch, SemanticSearchDocument,
    SemanticSearchRequest, SourceEvidence,
};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const OFFICIAL_BASE_URL: &str = "https://openapi.iwencai.com";
const SEARCH_PATH: &str = "/v1/comprehensive/search";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_LIMIT: u32 = 50;

/// Repository admission for the bounded authorized semantic-search slice.
pub const SEMANTIC_SEARCH_ADMITTED: bool = true;
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
static TRACE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum IwencaiError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("iWencai authentication failed: {0}")]
    Authentication(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("iWencai response decoding failed: {0}")]
    Decode(String),
    #[error("iWencai protocol error: {0}")]
    Protocol(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

/// Immutable authenticated POST request passed to an injected transport.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpRequest {
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl std::fmt::Debug for HttpRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpRequest")
            .field("url", &self.url)
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

impl HttpRequest {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

/// HTTP result that retains status so 401/403 can remain typed auth errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self { status, body }
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

pub trait IwencaiTransport: Send + Sync {
    fn post(&self, request: &HttpRequest) -> Result<HttpResponse, IwencaiError>;
}

#[derive(Clone)]
struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    fn new(timeout: Duration) -> Result<Self, IwencaiError> {
        if timeout.is_zero() {
            return Err(IwencaiError::InvalidRequest(
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

impl IwencaiTransport for HttpsTransport {
    fn post(&self, request: &HttpRequest) -> Result<HttpResponse, IwencaiError> {
        ensure_official_url(request.url())?;
        let mut call = self.agent.post(request.url());
        for (name, value) in request.headers() {
            call = call.set(name, value);
        }
        let response = match call.send_bytes(request.body()) {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(error) => return Err(IwencaiError::Transport(error.to_string())),
        };
        let status = response.status();
        let content_type = response.header("Content-Type").map(str::to_owned);
        read_http_response(status, content_type.as_deref(), response.into_reader())
    }
}

/// Configured iWencai client. API keys are never included in Debug output.
#[derive(Clone)]
pub struct IwencaiClient {
    api_key: Arc<str>,
    transport: Arc<dyn IwencaiTransport>,
    minimum_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
    request_probe: Arc<Mutex<ProbeRequestTracker>>,
}

impl std::fmt::Debug for IwencaiClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IwencaiClient")
            .field("api_key", &"[REDACTED]")
            .field("base_url", &OFFICIAL_BASE_URL)
            .finish_non_exhaustive()
    }
}

impl IwencaiClient {
    /// Reads an explicit API key from `MAGIC_IWENCAI_API_KEY`, with
    /// `IWENCAI_API_KEY` retained as a compatibility alias.
    pub fn from_env() -> Result<Self, IwencaiError> {
        let base = std::env::var("MAGIC_IWENCAI_BASE_URL")
            .or_else(|_| std::env::var("IWENCAI_BASE_URL"))
            .unwrap_or_else(|_| OFFICIAL_BASE_URL.to_owned());
        validate_base_url(&base)?;
        let key = std::env::var("MAGIC_IWENCAI_API_KEY")
            .or_else(|_| std::env::var("IWENCAI_API_KEY"))
            .map_err(|_| {
                IwencaiError::Authentication(
                    "set MAGIC_IWENCAI_API_KEY to an authorized SkillHub API key".into(),
                )
            })?;
        Self::with_timeout(key, DEFAULT_TIMEOUT)
    }

    pub fn new(api_key: impl Into<String>) -> Result<Self, IwencaiError> {
        Self::with_timeout(api_key, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        api_key: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, IwencaiError> {
        let api_key = validate_api_key(api_key.into())?;
        Ok(Self::from_parts(
            Arc::from(api_key),
            Arc::new(HttpsTransport::new(timeout)?),
            MINIMUM_REQUEST_INTERVAL,
        ))
    }

    pub fn with_transport(
        api_key: impl Into<String>,
        transport: impl IwencaiTransport + 'static,
    ) -> Result<Self, IwencaiError> {
        Ok(Self::from_parts(
            Arc::from(validate_api_key(api_key.into())?),
            Arc::new(transport),
            Duration::ZERO,
        ))
    }

    fn from_parts(
        api_key: Arc<str>,
        transport: Arc<dyn IwencaiTransport>,
        minimum_interval: Duration,
    ) -> Self {
        Self {
            api_key,
            transport,
            minimum_interval,
            request_gate: Arc::new(Mutex::new(None)),
            request_probe: Arc::new(Mutex::new(ProbeRequestTracker::default())),
        }
    }

    pub const fn research_capabilities() -> ResearchCapabilities {
        ResearchCapabilities {
            reports: false,
            consensus: false,
            target_price_consensus: false,
            semantic_search: SEMANTIC_SEARCH_ADMITTED,
            pdf_download: false,
            document_body: false,
        }
    }

    pub fn load_probe_snapshot(&self) -> Result<LoadProbeSnapshot, IwencaiError> {
        self.request_probe
            .lock()
            .map(|probe| probe.snapshot())
            .map_err(|_| IwencaiError::Transport("request probe lock poisoned".into()))
    }

    fn search(
        &self,
        request: &SemanticSearchRequest,
    ) -> Result<DataBatch<SemanticSearchDocument>, IwencaiError> {
        if request.limit().get() > MAX_LIMIT {
            return Err(IwencaiError::InvalidRequest(format!(
                "iWencai semantic-search limit must be at most {MAX_LIMIT}"
            )));
        }
        let http_request = build_request(request, &self.api_key, &trace_id()?)?;
        let response = self.execute(&http_request)?;
        let observed_at = now()?;
        if response.body().len() > MAX_RESPONSE_BYTES {
            return Err(IwencaiError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        match response.status() {
            401 | 403 => {
                return Err(IwencaiError::Authentication(format!(
                    "server rejected the configured API key with HTTP {}",
                    response.status()
                )));
            }
            200 => {}
            status => {
                return Err(IwencaiError::Transport(format!(
                    "unexpected HTTP status {status}"
                )));
            }
        }
        parse_response(
            response.body(),
            request.channel(),
            request.limit().get(),
            &observed_at,
        )
    }

    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, IwencaiError> {
        let mut last_started = self
            .request_gate
            .lock()
            .map_err(|_| IwencaiError::Transport("request gate lock poisoned".into()))?;
        if let Some(previous) = *last_started {
            let elapsed = previous.elapsed();
            if elapsed < self.minimum_interval {
                thread::sleep(self.minimum_interval - elapsed);
            }
        }
        *last_started = Some(Instant::now());
        self.request_probe
            .lock()
            .map_err(|_| IwencaiError::Transport("request probe lock poisoned".into()))?
            .request_started();
        let result = self.transport.post(request);
        self.request_probe
            .lock()
            .map_err(|_| IwencaiError::Transport("request probe lock poisoned".into()))?
            .request_finished()
            .map_err(|error| IwencaiError::Transport(error.to_string()))?;
        drop(last_started);
        result
    }
}

impl SemanticSearch for IwencaiClient {
    type Error = IwencaiError;

    fn semantic_search(
        &self,
        request: &SemanticSearchRequest,
    ) -> Result<DataBatch<SemanticSearchDocument>, Self::Error> {
        self.search(request)
    }
}

fn validate_api_key(value: String) -> Result<String, IwencaiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(IwencaiError::Authentication(
            "iWencai API key must not be empty".into(),
        ));
    }
    if trimmed.len() > 4096
        || trimmed
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(IwencaiError::Authentication(
            "iWencai API key has an invalid format".into(),
        ));
    }
    Ok(trimmed.to_owned())
}

fn validate_base_url(value: &str) -> Result<(), IwencaiError> {
    if value.trim_end_matches('/') != OFFICIAL_BASE_URL {
        return Err(IwencaiError::InvalidRequest(
            "iWencai base URL must be https://openapi.iwencai.com".into(),
        ));
    }
    Ok(())
}

fn ensure_official_url(url: &str) -> Result<(), IwencaiError> {
    let valid = url
        .strip_prefix("https://openapi.iwencai.com/")
        .is_some_and(|path| !path.is_empty() && !path.starts_with('/'));
    if !valid || url.chars().any(|character| character.is_control()) {
        return Err(IwencaiError::InvalidRequest(
            "iWencai transport only permits https://openapi.iwencai.com/".into(),
        ));
    }
    Ok(())
}

fn ensure_json_content_type(content_type: Option<&str>) -> Result<(), IwencaiError> {
    if content_type.is_some_and(|value| value.to_ascii_lowercase().contains("json")) {
        Ok(())
    } else {
        Err(IwencaiError::Protocol(format!(
            "expected a JSON response, received content type {content_type:?}"
        )))
    }
}

fn read_http_response(
    status: u16,
    content_type: Option<&str>,
    reader: impl Read,
) -> Result<HttpResponse, IwencaiError> {
    if status == 200 {
        ensure_json_content_type(content_type)?;
    }
    let mut body = Vec::new();
    reader
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|error| IwencaiError::Transport(error.to_string()))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(IwencaiError::Protocol(format!(
            "response exceeds {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    Ok(HttpResponse::new(status, body))
}

fn channel_name(channel: SemanticChannel) -> &'static str {
    match channel {
        SemanticChannel::Report => "report",
        SemanticChannel::News => "news",
        SemanticChannel::Announcement => "announcement",
        SemanticChannel::General => "general",
    }
}

fn build_request(
    request: &SemanticSearchRequest,
    api_key: &str,
    trace_id: &str,
) -> Result<HttpRequest, IwencaiError> {
    if trace_id.len() != 64 || !trace_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(IwencaiError::Protocol(
            "X-Claw trace id must contain 64 hexadecimal characters".into(),
        ));
    }
    let body = serde_json::to_vec(&serde_json::json!({
        "channels": [channel_name(request.channel())],
        "app_id": "AIME_SKILL",
        "query": request.query().as_str(),
        "size": request.limit().get()
    }))
    .map_err(|error| IwencaiError::Decode(format!("request JSON: {error}")))?;
    Ok(HttpRequest {
        url: format!("{OFFICIAL_BASE_URL}{SEARCH_PATH}"),
        headers: vec![
            ("Authorization".into(), format!("Bearer {api_key}")),
            ("Content-Type".into(), "application/json".into()),
            ("Accept".into(), "application/json".into()),
            ("X-Claw-Call-Type".into(), "normal".into()),
            ("X-Claw-Skill-Id".into(), "report-search".into()),
            ("X-Claw-Skill-Version".into(), "2.0.0".into()),
            ("X-Claw-Plugin-Id".into(), "none".into()),
            ("X-Claw-Plugin-Version".into(), "none".into()),
            ("X-Claw-Trace-Id".into(), trace_id.to_owned()),
            ("User-Agent".into(), "magic-iwencai-rs/0.2".into()),
        ],
        body,
    })
}

fn parse_response(
    body: &[u8],
    channel: SemanticChannel,
    expected_limit: u32,
    observed_at: &str,
) -> Result<DataBatch<SemanticSearchDocument>, IwencaiError> {
    let root: Value = serde_json::from_slice(body)
        .map_err(|error| IwencaiError::Decode(format!("semantic-search JSON: {error}")))?;
    let status = root
        .get("status_code")
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
        })
        .ok_or_else(|| IwencaiError::Protocol("status_code is missing or invalid".into()))?;
    if status != 0 {
        let message = root
            .get("status_msg")
            .and_then(Value::as_str)
            .unwrap_or("unknown iWencai error");
        if matches!(status, 401 | 403) || message.to_ascii_lowercase().contains("apikey") {
            return Err(IwencaiError::Authentication(
                "server rejected the configured API key".into(),
            ));
        }
        return Err(IwencaiError::Protocol(format!(
            "iWencai returned nonzero status_code {status}"
        )));
    }
    let rows = match root.get("data") {
        Some(Value::Array(rows)) => rows,
        Some(Value::Null) | None => {
            return Err(IwencaiError::Protocol(
                "successful iWencai response is missing data".into(),
            ));
        }
        Some(_) => {
            return Err(IwencaiError::Protocol(
                "iWencai data must be an array".into(),
            ));
        }
    };
    if rows.is_empty() {
        return Err(IwencaiError::Protocol(
            "iWencai returned an empty semantic-search batch".into(),
        ));
    }
    if rows.len() > expected_limit as usize {
        return Err(IwencaiError::Protocol(format!(
            "iWencai returned {} records for requested limit {expected_limit}",
            rows.len()
        )));
    }
    let batch_id = format!("iwencai:{observed_at}:semantic-search");
    let mut records: Vec<(SemanticSearchDocument, Option<f64>)> = Vec::with_capacity(rows.len());
    let mut positions = HashMap::<String, usize>::with_capacity(rows.len());
    for row in rows {
        let (record, score) = parse_document(row, channel, observed_at, &batch_id)?;
        let id = record.document_id.as_str().to_owned();
        if let Some(position) = positions.get(&id).copied() {
            let previous_score = records[position].1.unwrap_or(f64::NEG_INFINITY);
            if score.unwrap_or(f64::NEG_INFINITY) > previous_score {
                records[position] = (record, score);
            }
        } else {
            positions.insert(id, records.len());
            records.push((record, score));
        }
    }
    let records = records
        .into_iter()
        .map(|(record, _score)| record)
        .collect::<Vec<_>>();
    let provenance = Provenance::new("iwencai-openapi", observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn parse_document(
    row: &Value,
    channel: SemanticChannel,
    observed_at: &str,
    batch_id: &str,
) -> Result<(SemanticSearchDocument, Option<f64>), IwencaiError> {
    let object = row
        .as_object()
        .ok_or_else(|| IwencaiError::Protocol("semantic-search row must be an object".into()))?;
    let extra = parse_extra(object.get("extra"))?;
    let id = text_alias(object, &["uid", "id", "document_id"])
        .ok_or_else(|| IwencaiError::Protocol("semantic-search document id is missing".into()))?;
    let title = text_alias(object, &["title", "name"])
        .ok_or_else(|| IwencaiError::Protocol("semantic-search title is missing".into()))?;
    let canonical_url = text_alias(object, &["url", "link", "source_url"])
        .or_else(|| {
            extra
                .as_ref()
                .and_then(|value| text_alias(value, &["url", "link"]))
        })
        .ok_or_else(|| IwencaiError::Protocol("semantic-search URL is missing".into()))?;
    let excerpt = text_alias(object, &["summary", "excerpt", "abstract", "content"]);
    let published_at = text_alias(object, &["publish_date", "published_at", "time"]);
    let score = object.get("score").and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
    });
    if score.is_some_and(|value| !value.is_finite()) {
        return Err(IwencaiError::Protocol(
            "semantic-search score must be finite".into(),
        ));
    }
    let mut evidence = SourceEvidence::new(ProviderId::Iwencai, observed_at, batch_id)?;
    if let Some(source_at) = published_at.as_deref() {
        evidence = evidence.with_source_at(source_at)?;
    }
    Ok((
        SemanticSearchDocument {
            document_id: NonEmptyText::new(id)?,
            channel,
            title: NonEmptyText::new(title)?,
            excerpt: excerpt.map(NonEmptyText::new).transpose()?,
            canonical_url: HttpsUrl::new(canonical_url)?,
            published_at: published_at.map(NonEmptyText::new).transpose()?,
            evidence,
        },
        score,
    ))
}

fn parse_extra(
    value: Option<&Value>,
) -> Result<Option<serde_json::Map<String, Value>>, IwencaiError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(value)) => Ok(Some(value.clone())),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Value::String(value)) => serde_json::from_str::<Value>(value)
            .map_err(|error| IwencaiError::Decode(format!("semantic-search extra JSON: {error}")))?
            .as_object()
            .cloned()
            .ok_or_else(|| IwencaiError::Protocol("semantic-search extra must be an object".into()))
            .map(Some),
        Some(_) => Err(IwencaiError::Protocol(
            "semantic-search extra must be an object or JSON object string".into(),
        )),
    }
}

fn text_alias(object: &serde_json::Map<String, Value>, aliases: &[&str]) -> Option<String> {
    aliases.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(|value| match value {
                Value::String(value) => Some(value.clone()),
                Value::Number(value) => Some(value.to_string()),
                _ => None,
            })
            .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|value| !value.is_empty())
    })
}

fn trace_id() -> Result<String, IwencaiError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            IwencaiError::Protocol(format!("system clock precedes UNIX epoch: {error}"))
        })?
        .as_nanos();
    let counter = u128::from(TRACE_COUNTER.fetch_add(1, Ordering::Relaxed));
    Ok(format!("{nanos:032x}{counter:032x}"))
}

fn now() -> Result<String, IwencaiError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| {
            IwencaiError::Protocol(format!("system clock precedes UNIX epoch: {error}"))
        })
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
