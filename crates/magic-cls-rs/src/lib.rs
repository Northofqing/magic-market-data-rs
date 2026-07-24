#![forbid(unsafe_code)]
//! Bounded read-only adapter for Cailianpress global telegraph news.

use magic_market_core::{
    AssetClass, ContentCapabilities, DataBatch, Exchange, HttpsUrl, InstrumentDateRangeRequest,
    InstrumentId, LoadProbeSnapshot, NewsItem, NewsProvider, NonEmptyText, PositiveU32,
    ProbeRequestTracker, Provenance, ProviderId, SourceEvidence,
};
use md5::{Digest, Md5};
use serde_json::Value;
use sha1::Sha1;
use std::collections::HashSet;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const ENDPOINT: &str = "https://www.cls.cn/v1/roll/get_roll_list";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_PAGE_SIZE: u32 = 50;
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

/// CLS adapter failures. Authentication and protocol failures are never
/// converted into empty successful batches.
#[derive(Debug, Error)]
pub enum ClsError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("CLS response decoding failed: {0}")]
    Decode(String),
    #[error("CLS protocol error: {0}")]
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
pub trait ClsTransport: Send + Sync {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, ClsError>;
}

#[derive(Clone)]
struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    fn new(timeout: Duration) -> Result<Self, ClsError> {
        if timeout.is_zero() {
            return Err(ClsError::InvalidRequest(
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

impl ClsTransport for HttpsTransport {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, ClsError> {
        ensure_official_url(request.url())?;
        let mut call = self.agent.get(request.url());
        for (name, value) in request.headers() {
            call = call.set(name, value);
        }
        let response = call
            .call()
            .map_err(|error| ClsError::Transport(error.to_string()))?;
        if response.status() != 200 {
            return Err(ClsError::Transport(format!(
                "unexpected HTTP status {}",
                response.status()
            )));
        }
        ensure_json_content_type(response.header("Content-Type"))?;
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|error| ClsError::Transport(error.to_string()))?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(ClsError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        Ok(body)
    }
}

/// Read-only CLS telegraph client.
#[derive(Clone)]
pub struct ClsClient {
    transport: Arc<dyn ClsTransport>,
    minimum_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
    request_probe: Arc<Mutex<ProbeRequestTracker>>,
}

impl std::fmt::Debug for ClsClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ClsClient").finish_non_exhaustive()
    }
}

impl ClsClient {
    pub fn new() -> Result<Self, ClsError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, ClsError> {
        Ok(Self::from_parts(
            Arc::new(HttpsTransport::new(timeout)?),
            MINIMUM_REQUEST_INTERVAL,
        ))
    }

    pub fn with_transport(transport: impl ClsTransport + 'static) -> Self {
        Self::from_parts(Arc::new(transport), Duration::ZERO)
    }

    fn from_parts(transport: Arc<dyn ClsTransport>, minimum_interval: Duration) -> Self {
        Self {
            transport,
            minimum_interval,
            request_gate: Arc::new(Mutex::new(None)),
            request_probe: Arc::new(Mutex::new(ProbeRequestTracker::default())),
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

    pub fn load_probe_snapshot(&self) -> Result<LoadProbeSnapshot, ClsError> {
        self.request_probe
            .lock()
            .map(|probe| probe.snapshot())
            .map_err(|_| ClsError::Transport("request probe lock poisoned".into()))
    }

    fn fetch_global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, ClsError> {
        if limit.get() > MAX_PAGE_SIZE {
            return Err(ClsError::InvalidRequest(format!(
                "CLS rn must be at most {MAX_PAGE_SIZE}"
            )));
        }
        let request = build_request(limit.get());
        let body = self.execute(&request)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(ClsError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        let observed_at = now()?;
        parse_response(&body, limit.get(), &observed_at)
    }

    fn execute(&self, request: &HttpRequest) -> Result<Vec<u8>, ClsError> {
        let mut last_started = self
            .request_gate
            .lock()
            .map_err(|_| ClsError::Transport("request gate lock poisoned".into()))?;
        if let Some(previous) = *last_started {
            let elapsed = previous.elapsed();
            if elapsed < self.minimum_interval {
                thread::sleep(self.minimum_interval - elapsed);
            }
        }
        *last_started = Some(Instant::now());
        self.request_probe
            .lock()
            .map_err(|_| ClsError::Transport("request probe lock poisoned".into()))?
            .request_started();
        let result = self.transport.get(request);
        self.request_probe
            .lock()
            .map_err(|_| ClsError::Transport("request probe lock poisoned".into()))?
            .request_finished()
            .map_err(|error| ClsError::Transport(error.to_string()))?;
        drop(last_started);
        result
    }
}

impl NewsProvider for ClsClient {
    type Error = ClsError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(ClsError::Unsupported(
            "CLS telegraph does not expose a verified instrument/date filter".into(),
        ))
    }

    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        self.fetch_global_news(limit)
    }
}

fn ensure_official_url(url: &str) -> Result<(), ClsError> {
    let valid = url
        .strip_prefix("https://www.cls.cn/")
        .is_some_and(|path| !path.is_empty() && !path.starts_with('/'));
    if !valid || url.chars().any(|character| character.is_control()) {
        return Err(ClsError::InvalidRequest(
            "CLS transport only permits https://www.cls.cn/".into(),
        ));
    }
    Ok(())
}

fn ensure_json_content_type(content_type: Option<&str>) -> Result<(), ClsError> {
    if content_type.is_some_and(|value| value.to_ascii_lowercase().contains("json")) {
        Ok(())
    } else {
        Err(ClsError::Protocol(format!(
            "expected a JSON response, received content type {content_type:?}"
        )))
    }
}

fn build_request(limit: u32) -> HttpRequest {
    let query =
        format!("appName=CailianpressWeb&last_time=&os=web&refresh_type=1&rn={limit}&sv=7.7.5");
    let sha1 = format!("{:x}", Sha1::digest(query.as_bytes()));
    let sign = format!("{:x}", Md5::digest(sha1.as_bytes()));
    HttpRequest {
        url: format!("{ENDPOINT}?{query}&sign={sign}"),
        headers: vec![
            ("Accept".into(), "application/json".into()),
            ("Referer".into(), "https://www.cls.cn/".into()),
            ("User-Agent".into(), "magic-cls-rs/0.2".into()),
        ],
    }
}

fn parse_response(
    body: &[u8],
    limit: u32,
    observed_at: &str,
) -> Result<DataBatch<NewsItem>, ClsError> {
    let root: Value = serde_json::from_slice(body)
        .map_err(|error| ClsError::Decode(format!("telegraph JSON: {error}")))?;
    let errno = root
        .get("errno")
        .and_then(Value::as_i64)
        .ok_or_else(|| ClsError::Protocol("errno must be an integer".into()))?;
    if errno != 0 {
        let message = root
            .get("errmsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown CLS error");
        return Err(ClsError::Protocol(format!(
            "CLS returned errno {errno}: {message}"
        )));
    }
    let rows = root
        .pointer("/data/roll_data")
        .and_then(Value::as_array)
        .ok_or_else(|| ClsError::Protocol("data.roll_data must be an array".into()))?;
    if rows.len() > limit as usize {
        return Err(ClsError::Protocol(format!(
            "CLS returned {} records for rn={limit}",
            rows.len()
        )));
    }
    if rows.is_empty() {
        return Err(ClsError::Protocol(
            "CLS returned an empty telegraph batch".into(),
        ));
    }

    let batch_id = format!("cls:{observed_at}:telegraph");
    let mut records = Vec::with_capacity(rows.len());
    let mut seen = HashSet::with_capacity(rows.len());
    let mut previous_source_at: Option<String> = None;
    for row in rows {
        let record = parse_item(row, observed_at, &batch_id, &mut seen)?;
        if previous_source_at
            .as_deref()
            .is_some_and(|previous| previous < record.published_at.as_str())
        {
            return Err(ClsError::Protocol(
                "CLS telegraph rows are not ordered newest first".into(),
            ));
        }
        previous_source_at = Some(record.published_at.as_str().to_owned());
        records.push(record);
    }
    let source_at = records
        .first()
        .map(|record| record.published_at.as_str())
        .ok_or_else(|| ClsError::Protocol("latest CLS source time is missing".into()))?;
    let provenance = Provenance::new("cls-v1", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn parse_item(
    row: &Value,
    observed_at: &str,
    batch_id: &str,
    seen: &mut HashSet<String>,
) -> Result<NewsItem, ClsError> {
    let object = row
        .as_object()
        .ok_or_else(|| ClsError::Protocol("telegraph row must be an object".into()))?;
    let item_id = match object.get("id") {
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::String(value)) => value.clone(),
        _ => return Err(ClsError::Protocol("telegraph id is missing".into())),
    };
    if !seen.insert(item_id.clone()) {
        return Err(ClsError::Protocol(format!(
            "duplicate telegraph id {item_id}"
        )));
    }
    let brief = optional_text(object.get("brief"));
    let title = optional_text(object.get("title"))
        .or_else(|| brief.clone())
        .ok_or_else(|| ClsError::Protocol("telegraph title and brief are empty".into()))?;
    let content = optional_text(object.get("content")).or_else(|| brief.clone());
    let ctime = object
        .get("ctime")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| ClsError::Protocol("telegraph ctime must be positive".into()))?;
    let published_at = unix_to_china_time(ctime)?;
    let canonical_url = object
        .get("shareurl")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ClsError::Protocol("telegraph shareurl is missing".into()))?;
    let evidence = SourceEvidence::new(ProviderId::Cailianpress, observed_at, batch_id)?
        .with_source_at(published_at.clone())?;
    Ok(NewsItem {
        item_id: NonEmptyText::new(item_id)?,
        title: NonEmptyText::new(title)?,
        summary: brief.map(NonEmptyText::new).transpose()?,
        content: content.map(NonEmptyText::new).transpose()?,
        publisher: NonEmptyText::new("财联社")?,
        canonical_url: HttpsUrl::new(canonical_url)?,
        published_at: NonEmptyText::new(published_at)?,
        instruments: parse_instruments(object.get("stock_list"))?,
        topics: parse_topics(object.get("subjects"), object.get("tags"))?,
        language: NonEmptyText::new("zh-CN")?,
        evidence,
    })
}

fn optional_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(normalized_text)
        .filter(|value| !value.is_empty())
}

fn normalized_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_instruments(value: Option<&Value>) -> Result<Vec<InstrumentId>, ClsError> {
    let rows = match value {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(rows)) => rows,
        Some(_) => {
            return Err(ClsError::Protocol(
                "telegraph stock_list must be an array when present".into(),
            ))
        }
    };
    let mut instruments = Vec::new();
    let mut seen = HashSet::new();
    for row in rows {
        let object = row.as_object().ok_or_else(|| {
            ClsError::Protocol("telegraph stock_list row must be an object".into())
        })?;
        let stock_id = object
            .get("StockID")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ClsError::Protocol("telegraph stock_list StockID must be a non-empty string".into())
            })?;
        let (exchange, code) = if let Some(code) = stock_id.strip_prefix("sh") {
            (Exchange::Shanghai, code)
        } else if let Some(code) = stock_id.strip_prefix("sz") {
            (Exchange::Shenzhen, code)
        } else if let Some(code) = stock_id.strip_prefix("bj") {
            (Exchange::Beijing, code)
        } else {
            return Err(ClsError::Protocol(format!(
                "telegraph StockID has an unsupported exchange prefix: {stock_id}"
            )));
        };
        if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ClsError::Protocol(format!(
                "telegraph StockID has an invalid security code: {stock_id}"
            )));
        }
        if seen.insert((exchange, code.to_owned())) {
            let asset_class = classify_associated_asset(exchange, code)?;
            instruments.push(InstrumentId::new(exchange, code, asset_class)?);
        }
    }
    Ok(instruments)
}

fn classify_associated_asset(exchange: Exchange, code: &str) -> Result<AssetClass, ClsError> {
    let asset_class = match exchange {
        Exchange::Shanghai if code.starts_with("000") => AssetClass::Index,
        Exchange::Shanghai if code.starts_with("510") => AssetClass::Fund,
        Exchange::Shanghai
            if ["600", "601", "603", "605", "688"]
                .iter()
                .any(|prefix| code.starts_with(prefix)) =>
        {
            AssetClass::Equity
        }
        Exchange::Shenzhen if code.starts_with("399") => AssetClass::Index,
        Exchange::Shenzhen if code.starts_with("159") => AssetClass::Fund,
        Exchange::Shenzhen
            if ["000", "001", "002", "003", "300", "301"]
                .iter()
                .any(|prefix| code.starts_with(prefix)) =>
        {
            AssetClass::Equity
        }
        Exchange::Beijing
            if code.starts_with('4') || code.starts_with('8') || code.starts_with("920") =>
        {
            AssetClass::Equity
        }
        _ => {
            return Err(ClsError::Protocol(format!(
                "telegraph security identity {exchange:?}:{code} has an unverified asset class"
            )))
        }
    };
    Ok(asset_class)
}

fn parse_topics(
    subjects: Option<&Value>,
    tags: Option<&Value>,
) -> Result<Vec<NonEmptyText>, ClsError> {
    let mut topics = Vec::new();
    let mut seen = HashSet::new();
    if let Some(rows) = optional_array(subjects, "subjects")? {
        for row in rows {
            let topic = row
                .as_object()
                .ok_or_else(|| {
                    ClsError::Protocol("telegraph subjects row must be an object".into())
                })?
                .get("subject_name")
                .and_then(Value::as_str)
                .map(normalized_text)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ClsError::Protocol("telegraph subject_name must be a non-empty string".into())
                })?;
            if seen.insert(topic.clone()) {
                topics.push(NonEmptyText::new(topic)?);
            }
        }
    }
    if let Some(rows) = optional_array(tags, "tags")? {
        for row in rows {
            let topic = row
                .as_str()
                .or_else(|| row.get("name").and_then(Value::as_str))
                .map(normalized_text)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ClsError::Protocol("telegraph tag must contain a non-empty name".into())
                })?;
            if seen.insert(topic.clone()) {
                topics.push(NonEmptyText::new(topic)?);
            }
        }
    }
    Ok(topics)
}

fn optional_array<'a>(
    value: Option<&'a Value>,
    field: &'static str,
) -> Result<Option<&'a Vec<Value>>, ClsError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(rows)) => Ok(Some(rows)),
        Some(_) => Err(ClsError::Protocol(format!(
            "telegraph {field} must be an array when present"
        ))),
    }
}

fn unix_to_china_time(seconds: i64) -> Result<String, ClsError> {
    let shifted = seconds
        .checked_add(8 * 60 * 60)
        .ok_or_else(|| ClsError::Protocol("telegraph ctime overflow".into()))?;
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

fn civil_from_days(days_since_epoch: i64) -> Result<(i64, i64, i64), ClsError> {
    let z = days_since_epoch
        .checked_add(719_468)
        .ok_or_else(|| ClsError::Protocol("telegraph ctime is out of range".into()))?;
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
        return Err(ClsError::Protocol(
            "telegraph ctime is outside the supported range".into(),
        ));
    }
    Ok((year, month, day))
}

fn now() -> Result<String, ClsError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| ClsError::Protocol(format!("system clock precedes UNIX epoch: {error}")))
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
