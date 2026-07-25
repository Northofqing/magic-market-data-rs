#![forbid(unsafe_code)]
//! Bounded read-only adapter for public Jin10 financial flashes.

use magic_market_core::{
    CalendarCapabilities, ContentCapabilities, DataBatch, EconomicCalendarProvider,
    EconomicCalendarRequest, EconomicEvent, HttpsUrl, InstrumentDateRangeRequest, NewsItem,
    NewsProvider, NonEmptyText, PositiveU32, Provenance, ProviderId, SourceEvidence,
};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const ENDPOINT: &str = "https://flash-api.jin10.com/get_flash_list";
const OFFICIAL_APP_ID: &str = "bVBF4FyRTn5NJF5n";
const OFFICIAL_API_VERSION: &str = "1.0.0";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_PAGE_SIZE: u32 = 20;
const MAX_SOURCE_ROWS: usize = 21;
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

/// Jin10 adapter failures. Protected or malformed upstream data is never
/// converted into an empty successful batch.
#[derive(Debug, Error)]
pub enum Jin10Error {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("Jin10 response decoding failed: {0}")]
    Decode(String),
    #[error("Jin10 protocol error: {0}")]
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
pub trait Jin10Transport: Send + Sync {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, Jin10Error>;
}

#[derive(Clone)]
struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    fn new(timeout: Duration) -> Result<Self, Jin10Error> {
        if timeout.is_zero() {
            return Err(Jin10Error::InvalidRequest(
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

impl Jin10Transport for HttpsTransport {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, Jin10Error> {
        ensure_official_url(request.url())?;
        let mut call = self.agent.get(request.url());
        for (name, value) in request.headers() {
            call = call.set(name, value);
        }
        let response = call
            .call()
            .map_err(|error| Jin10Error::Transport(error.to_string()))?;
        if response.status() != 200 {
            return Err(Jin10Error::Transport(format!(
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
            .map_err(|error| Jin10Error::Transport(error.to_string()))?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(Jin10Error::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        Ok(body)
    }
}

/// Read-only Jin10 public-flash client.
#[derive(Clone)]
pub struct Jin10Client {
    transport: Arc<dyn Jin10Transport>,
    minimum_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
}

impl std::fmt::Debug for Jin10Client {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Jin10Client")
            .finish_non_exhaustive()
    }
}

impl Jin10Client {
    pub fn new() -> Result<Self, Jin10Error> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, Jin10Error> {
        Ok(Self::from_parts(
            Arc::new(HttpsTransport::new(timeout)?),
            MINIMUM_REQUEST_INTERVAL,
        ))
    }

    pub fn with_transport(transport: impl Jin10Transport + 'static) -> Self {
        Self::from_parts(Arc::new(transport), MINIMUM_REQUEST_INTERVAL)
    }

    fn from_parts(transport: Arc<dyn Jin10Transport>, minimum_interval: Duration) -> Self {
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
            market_announcements: false,
            investor_questions: false,
        }
    }

    pub const fn calendar_capabilities() -> CalendarCapabilities {
        CalendarCapabilities {
            economic_releases: true,
            futures_delivery: false,
        }
    }

    fn fetch_global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Jin10Error> {
        if limit.get() > MAX_PAGE_SIZE {
            return Err(Jin10Error::InvalidRequest(format!(
                "Jin10 public flash limit must be at most {MAX_PAGE_SIZE}"
            )));
        }
        let request = build_request();
        let body = self.execute(&request)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(Jin10Error::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        let observed_at = now()?;
        parse_response(&body, limit.get(), &observed_at)
    }

    fn execute(&self, request: &HttpRequest) -> Result<Vec<u8>, Jin10Error> {
        let mut last_started = self
            .request_gate
            .lock()
            .map_err(|_| Jin10Error::Transport("request gate lock poisoned".into()))?;
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

impl NewsProvider for Jin10Client {
    type Error = Jin10Error;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(Jin10Error::Unsupported(
            "Jin10 public flash does not expose a verified instrument/date filter".into(),
        ))
    }

    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        self.fetch_global_news(limit)
    }
}

impl EconomicCalendarProvider for Jin10Client {
    type Error = Jin10Error;

    fn economic_calendar(
        &self,
        request: &EconomicCalendarRequest,
    ) -> Result<DataBatch<EconomicEvent>, Self::Error> {
        let body = self.execute(&build_request())?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(Jin10Error::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        let observed_at = now()?;
        parse_economic_response(&body, request, &observed_at)
    }
}

fn ensure_official_url(url: &str) -> Result<(), Jin10Error> {
    let valid = url
        .strip_prefix("https://flash-api.jin10.com/")
        .is_some_and(|path| !path.is_empty() && !path.starts_with('/'));
    if !valid || url.chars().any(char::is_control) {
        return Err(Jin10Error::InvalidRequest(
            "Jin10 transport only permits https://flash-api.jin10.com/".into(),
        ));
    }
    Ok(())
}

fn ensure_json_content_type(content_type: Option<&str>) -> Result<(), Jin10Error> {
    if content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
    }) {
        Ok(())
    } else {
        Err(Jin10Error::Protocol(format!(
            "expected a JSON response, received content type {content_type:?}"
        )))
    }
}

fn build_request() -> HttpRequest {
    HttpRequest {
        url: format!("{ENDPOINT}?channel=-8200&vip=1"),
        headers: vec![
            ("Accept".into(), "application/json".into()),
            ("Origin".into(), "https://www.jin10.com".into()),
            ("Referer".into(), "https://www.jin10.com/".into()),
            ("User-Agent".into(), "magic-jin10-rs/0.2".into()),
            ("x-app-id".into(), OFFICIAL_APP_ID.into()),
            ("x-version".into(), OFFICIAL_API_VERSION.into()),
        ],
    }
}

fn parse_response(
    body: &[u8],
    limit: u32,
    observed_at: &str,
) -> Result<DataBatch<NewsItem>, Jin10Error> {
    let root: Value = serde_json::from_slice(body)
        .map_err(|error| Jin10Error::Decode(format!("flash JSON: {error}")))?;
    let object = root
        .as_object()
        .ok_or_else(|| Jin10Error::Protocol("response root must be an object".into()))?;
    let status = object
        .get("status")
        .and_then(Value::as_i64)
        .ok_or_else(|| Jin10Error::Protocol("status must be an integer".into()))?;
    let message = object
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| Jin10Error::Protocol("message must be a string".into()))?;
    if status != 200 || message != "OK" {
        return Err(Jin10Error::Protocol(format!(
            "Jin10 returned status {status}: {message}"
        )));
    }
    let rows = object
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| Jin10Error::Protocol("data must be an array".into()))?;
    if rows.len() > MAX_SOURCE_ROWS {
        return Err(Jin10Error::Protocol(format!(
            "Jin10 returned {} rows, verified source maximum is {MAX_SOURCE_ROWS}",
            rows.len()
        )));
    }
    if rows.is_empty() {
        return Err(Jin10Error::Protocol(
            "Jin10 returned an empty flash batch".into(),
        ));
    }

    let batch_id = format!("jin10:{observed_at}:flash");
    let mut seen = HashSet::with_capacity(rows.len());
    let mut parsed = Vec::with_capacity(rows.len());
    for row in rows {
        let object = row
            .as_object()
            .ok_or_else(|| Jin10Error::Protocol("flash row must be an object".into()))?;
        let item_id = required_id(object.get("id"))?;
        if !seen.insert(item_id.clone()) {
            return Err(Jin10Error::Protocol(format!(
                "duplicate Jin10 flash id {item_id}"
            )));
        }
        if is_locked(object)? || !is_news_type(object)? || !is_public_news_channel(object)? {
            continue;
        }
        parsed.push(parse_item(object, item_id, observed_at, &batch_id)?);
    }
    if parsed.is_empty() {
        return Err(Jin10Error::Protocol(
            "Jin10 returned no eligible public news rows".into(),
        ));
    }
    parsed.sort_by(|left, right| right.published_at.as_str().cmp(left.published_at.as_str()));
    parsed.truncate(limit as usize);

    let source_at = parsed
        .first()
        .map(|item| item.published_at.as_str())
        .ok_or_else(|| Jin10Error::Protocol("latest Jin10 source time is missing".into()))?;
    let provenance = Provenance::new("jin10-flash-v1", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(parsed, provenance))
}

fn parse_economic_response(
    body: &[u8],
    request: &EconomicCalendarRequest,
    observed_at: &str,
) -> Result<DataBatch<EconomicEvent>, Jin10Error> {
    let root: Value = serde_json::from_slice(body)
        .map_err(|error| Jin10Error::Decode(format!("flash JSON: {error}")))?;
    let object = checked_envelope(&root)?;
    let rows = object
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| Jin10Error::Protocol("data must be an array".into()))?;
    if rows.is_empty() || rows.len() > MAX_SOURCE_ROWS {
        return Err(Jin10Error::Protocol(format!(
            "Jin10 economic source row count {} is outside 1..={MAX_SOURCE_ROWS}",
            rows.len()
        )));
    }

    let batch_id = format!("jin10:{observed_at}:economic-calendar");
    let mut seen = HashSet::with_capacity(rows.len());
    let mut records = Vec::new();
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| Jin10Error::Protocol("flash row must be an object".into()))?;
        let event_id = required_id(row.get("id"))?;
        if !seen.insert(event_id.clone()) {
            return Err(Jin10Error::Protocol(format!(
                "duplicate Jin10 flash id {event_id}"
            )));
        }
        if is_locked(row)? {
            continue;
        }
        let item_type = row
            .get("type")
            .and_then(Value::as_i64)
            .ok_or_else(|| Jin10Error::Protocol("flash type must be an integer".into()))?;
        if item_type != 1 {
            continue;
        }
        let record = parse_economic_event(row, event_id, observed_at, &batch_id)?;
        if request
            .country()
            .is_some_and(|country| country.as_str() != record.country.as_str())
        {
            continue;
        }
        records.push(record);
    }
    if records.is_empty() {
        return Err(Jin10Error::Protocol(
            "Jin10 returned no eligible public economic releases".into(),
        ));
    }
    records.sort_by(|left, right| right.released_at.as_str().cmp(left.released_at.as_str()));
    records.truncate(request.limit().get() as usize);
    let source_at = records
        .first()
        .map(|record| record.released_at.as_str())
        .ok_or_else(|| Jin10Error::Protocol("latest economic release time is missing".into()))?;
    let provenance = Provenance::new("jin10-flash-v1", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn checked_envelope(root: &Value) -> Result<&Map<String, Value>, Jin10Error> {
    let object = root
        .as_object()
        .ok_or_else(|| Jin10Error::Protocol("response root must be an object".into()))?;
    let status = object
        .get("status")
        .and_then(Value::as_i64)
        .ok_or_else(|| Jin10Error::Protocol("status must be an integer".into()))?;
    let message = object
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| Jin10Error::Protocol("message must be a string".into()))?;
    if status != 200 || message != "OK" {
        return Err(Jin10Error::Protocol(format!(
            "Jin10 returned status {status}: {message}"
        )));
    }
    Ok(object)
}

fn parse_economic_event(
    row: &Map<String, Value>,
    event_id: String,
    observed_at: &str,
    batch_id: &str,
) -> Result<EconomicEvent, Jin10Error> {
    let data = row
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| Jin10Error::Protocol("economic data must be an object".into()))?;
    let released_at =
        jin10_time(row.get("time").and_then(Value::as_str).ok_or_else(|| {
            Jin10Error::Protocol("economic release time must be a string".into())
        })?)?;
    let scheduled_at = jin10_time(
        data.get("pub_time")
            .and_then(Value::as_str)
            .ok_or_else(|| Jin10Error::Protocol("economic pub_time must be a string".into()))?,
    )?;
    let indicator = required_positive_u32(data.get("indicator_id"), "indicator_id")?;
    let star = required_positive_u32(data.get("star"), "star")?;
    if star.get() > 5 {
        return Err(Jin10Error::Protocol(
            "economic star must be in 1..=5".into(),
        ));
    }
    let country = required_scalar_text(data.get("country"), "country")?;
    let name = required_scalar_text(data.get("name"), "name")?;
    let evidence = SourceEvidence::new(ProviderId::Jin10, observed_at, batch_id)?
        .with_source_at(released_at.clone())?;
    Ok(EconomicEvent {
        event_id: NonEmptyText::new(event_id)?,
        indicator_id: indicator,
        country: NonEmptyText::new(country)?,
        name: NonEmptyText::new(name)?,
        period: optional_scalar_text(data.get("time_period"))?
            .map(NonEmptyText::new)
            .transpose()?,
        scheduled_at: NonEmptyText::new(scheduled_at)?,
        released_at: NonEmptyText::new(released_at)?,
        previous: optional_scalar_text(data.get("previous"))?
            .map(NonEmptyText::new)
            .transpose()?,
        consensus: optional_scalar_text(data.get("consensus"))?
            .map(NonEmptyText::new)
            .transpose()?,
        actual: optional_scalar_text(data.get("actual"))?
            .map(NonEmptyText::new)
            .transpose()?,
        revised: optional_scalar_text(data.get("revised"))?
            .map(NonEmptyText::new)
            .transpose()?,
        unit: optional_scalar_text(data.get("unit"))?
            .map(NonEmptyText::new)
            .transpose()?,
        importance: star,
        impact: optional_scalar_text(data.get("affect"))?
            .map(NonEmptyText::new)
            .transpose()?,
        evidence,
    })
}

fn required_positive_u32(value: Option<&Value>, field: &str) -> Result<PositiveU32, Jin10Error> {
    let number = match value {
        Some(Value::Number(value)) => value.as_u64(),
        Some(Value::String(value)) => value.trim().parse::<u64>().ok(),
        _ => None,
    }
    .filter(|value| *value <= u64::from(u32::MAX))
    .ok_or_else(|| Jin10Error::Protocol(format!("{field} must be a positive u32")))?;
    PositiveU32::new(number as u32).map_err(Into::into)
}

fn required_scalar_text(value: Option<&Value>, field: &str) -> Result<String, Jin10Error> {
    optional_scalar_text(value)?
        .ok_or_else(|| Jin10Error::Protocol(format!("economic {field} is missing")))
}

fn optional_scalar_text(value: Option<&Value>) -> Result<Option<String>, Jin10Error> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            let value = normalized_text(value);
            Ok((!value.is_empty()).then_some(value))
        }
        Some(Value::Number(value)) => Ok(Some(value.to_string())),
        Some(_) => Err(Jin10Error::Protocol(
            "economic scalar field must be text, number, or null".into(),
        )),
    }
}

fn required_id(value: Option<&Value>) -> Result<String, Jin10Error> {
    match value {
        Some(Value::String(value)) if valid_id(value) => Ok(value.clone()),
        Some(Value::Number(value)) if valid_id(&value.to_string()) => Ok(value.to_string()),
        _ => Err(Jin10Error::Protocol(
            "flash id must be a non-empty ASCII identifier".into(),
        )),
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn is_locked(row: &Map<String, Value>) -> Result<bool, Jin10Error> {
    let data = row
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| Jin10Error::Protocol("flash data must be an object".into()))?;
    match data.get("lock") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => Ok(false),
        Some(Value::Bool(true)) => {
            if data.get("vip_level").and_then(Value::as_i64).is_none() {
                return Err(Jin10Error::Protocol(
                    "locked Jin10 flash is missing vip_level".into(),
                ));
            }
            Ok(true)
        }
        Some(_) => Err(Jin10Error::Protocol(
            "flash data.lock must be a boolean when present".into(),
        )),
    }
}

fn is_news_type(row: &Map<String, Value>) -> Result<bool, Jin10Error> {
    let item_type = row
        .get("type")
        .and_then(Value::as_i64)
        .ok_or_else(|| Jin10Error::Protocol("flash type must be an integer".into()))?;
    Ok(matches!(item_type, 0 | 2))
}

fn is_public_news_channel(row: &Map<String, Value>) -> Result<bool, Jin10Error> {
    let channels = row
        .get("channel")
        .and_then(Value::as_array)
        .ok_or_else(|| Jin10Error::Protocol("flash channel must be an array".into()))?;
    if channels.is_empty() {
        return Err(Jin10Error::Protocol(
            "flash channel must not be empty".into(),
        ));
    }
    let mut public_news = false;
    for channel in channels {
        let channel = channel
            .as_u64()
            .filter(|channel| *channel > 0)
            .ok_or_else(|| {
                Jin10Error::Protocol("flash channel entries must be positive integers".into())
            })?;
        if matches!(channel, 1..=3) {
            public_news = true;
        }
    }
    Ok(public_news)
}

fn parse_item(
    row: &Map<String, Value>,
    item_id: String,
    observed_at: &str,
    batch_id: &str,
) -> Result<NewsItem, Jin10Error> {
    let item_type = row
        .get("type")
        .and_then(Value::as_i64)
        .ok_or_else(|| Jin10Error::Protocol("flash type must be an integer".into()))?;
    let data = row
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| Jin10Error::Protocol("flash data must be an object".into()))?;
    let content = optional_text(data.get("content"))
        .ok_or_else(|| Jin10Error::Protocol("public Jin10 news content is empty".into()))?;
    let title = optional_text(data.get("title")).unwrap_or_else(|| content.clone());
    let published_at = jin10_time(
        row.get("time")
            .and_then(Value::as_str)
            .ok_or_else(|| Jin10Error::Protocol("flash time must be a string".into()))?,
    )?;
    let source = optional_text(data.get("source"));
    let publisher = source.unwrap_or_else(|| "金十数据".into());
    let canonical_url = if item_type == 2 {
        optional_text(data.get("link"))
            .map(HttpsUrl::new)
            .transpose()?
            .unwrap_or(HttpsUrl::new(format!(
                "https://flash.jin10.com/detail/{item_id}"
            ))?)
    } else {
        HttpsUrl::new(format!("https://flash.jin10.com/detail/{item_id}"))?
    };
    let evidence = SourceEvidence::new(ProviderId::Jin10, observed_at, batch_id)?
        .with_source_at(published_at.clone())?;
    Ok(NewsItem {
        item_id: NonEmptyText::new(item_id)?,
        title: NonEmptyText::new(title)?,
        summary: None,
        content: Some(NonEmptyText::new(content)?),
        publisher: NonEmptyText::new(publisher)?,
        canonical_url,
        published_at: NonEmptyText::new(published_at)?,
        instruments: Vec::new(),
        topics: parse_topics(row)?,
        language: NonEmptyText::new("zh-CN")?,
        evidence,
    })
}

fn parse_topics(row: &Map<String, Value>) -> Result<Vec<NonEmptyText>, Jin10Error> {
    let mut topics = Vec::new();
    let mut seen = HashSet::new();
    if row.get("important").and_then(Value::as_i64).unwrap_or(0) > 0 {
        seen.insert("重要".to_owned());
        topics.push(NonEmptyText::new("重要")?);
    }
    let tags = match row.get("tags") {
        None | Some(Value::Null) => return Ok(topics),
        Some(Value::Array(tags)) => tags,
        Some(_) => {
            return Err(Jin10Error::Protocol(
                "flash tags must be an array when present".into(),
            ))
        }
    };
    for tag in tags {
        let topic = match tag {
            Value::String(value) => normalized_text(value),
            Value::Object(object) => ["name", "tag", "title"]
                .iter()
                .find_map(|field| object.get(*field).and_then(Value::as_str))
                .map(normalized_text)
                .unwrap_or_default(),
            _ => {
                return Err(Jin10Error::Protocol(
                    "flash tag must be a string or named object".into(),
                ))
            }
        };
        if topic.is_empty() {
            return Err(Jin10Error::Protocol(
                "flash tag name must not be empty".into(),
            ));
        }
        if seen.insert(topic.clone()) {
            topics.push(NonEmptyText::new(topic)?);
        }
    }
    Ok(topics)
}

fn optional_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(strip_html)
        .map(|value| normalized_text(&value))
        .filter(|value| !value.is_empty())
}

fn normalized_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    output
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn jin10_time(value: &str) -> Result<String, Jin10Error> {
    if value.len() != 19 {
        return Err(Jin10Error::Protocol(
            "flash time must use YYYY-MM-DD HH:MM:SS".into(),
        ));
    }
    let bytes = value.as_bytes();
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b' '
        || bytes[13] != b':'
        || bytes[16] != b':'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7 | 10 | 13 | 16) || byte.is_ascii_digit())
    {
        return Err(Jin10Error::Protocol(
            "flash time must use YYYY-MM-DD HH:MM:SS".into(),
        ));
    }
    let parse = |range: std::ops::Range<usize>| {
        value[range]
            .parse::<u32>()
            .map_err(|_| Jin10Error::Protocol("flash time contains invalid digits".into()))
    };
    let year = parse(0..4)?;
    let month = parse(5..7)?;
    let day = parse(8..10)?;
    let hour = parse(11..13)?;
    let minute = parse(14..16)?;
    let second = parse(17..19)?;
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return Err(Jin10Error::Protocol("flash time is out of range".into()));
    }
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}+08:00"
    ))
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => 31,
    }
}

fn now() -> Result<String, Jin10Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| Jin10Error::Protocol(format!("system clock precedes UNIX epoch: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Mutex};

    const FIXTURE: &str = r#"{
      "status": 200,
      "message": "OK",
      "data": [
        {
          "id": "20260724224037091800",
          "time": "2026-07-24 22:40:37",
          "type": 0,
          "important": 1,
          "channel": [1, 2, 3],
          "data": {
            "content": "【晶泰控股：预计上半年由盈转亏】<a href=\"https://example.com\">详情</a>",
            "exclusive_to": [],
            "source": "金十数据",
            "source_link": "",
            "title": ""
          },
          "tags": [{"name": "港股"}, "业绩"]
        },
        {
          "id": "20260724224012910800",
          "time": "2026-07-24 22:40:12",
          "type": 0,
          "important": 0,
          "channel": [5],
          "data": {
            "content": "",
            "lock": true,
            "vip_level": 1,
            "vip_title": "protected"
          },
          "tags": []
        },
        {
          "id": "20260724222659975800",
          "time": "2026-07-24 22:26:59",
          "type": 2,
          "important": 0,
          "channel": [2],
          "data": {
            "content": "公开文章摘要",
            "link": "https://xnews.jin10.com/details/225718",
            "source": "",
            "source_link": "",
            "tag": "精选",
            "title": "公开文章"
          },
          "tags": []
        },
        {
          "id": "calendar-1",
          "time": "2026-07-24 22:00:00",
          "type": 1,
          "important": 0,
          "channel": [1],
          "data": {"content": "economic calendar"},
          "tags": []
        }
      ]
    }"#;

    #[derive(Debug)]
    struct FixtureTransport {
        response: Vec<u8>,
        request: Mutex<Option<HttpRequest>>,
    }

    impl Jin10Transport for FixtureTransport {
        fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, Jin10Error> {
            *self
                .request
                .lock()
                .map_err(|_| Jin10Error::Transport("fixture lock poisoned".into()))? =
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

    impl Jin10Transport for BlockingTransport {
        fn get(&self, _request: &HttpRequest) -> Result<Vec<u8>, Jin10Error> {
            self.starts
                .send(Instant::now())
                .map_err(|error| Jin10Error::Transport(error.to_string()))?;
            self.releases
                .lock()
                .map_err(|_| Jin10Error::Transport("release lock poisoned".into()))?
                .recv()
                .map_err(|error| Jin10Error::Transport(error.to_string()))?;
            Ok(self.response.clone())
        }
    }

    fn fixture_with_row_count(row_count: usize) -> Vec<u8> {
        let mut root: Value = serde_json::from_str(FIXTURE).unwrap();
        let rows = root.get_mut("data").and_then(Value::as_array_mut).unwrap();
        let template = rows
            .iter()
            .find(|row| row.get("type").and_then(Value::as_i64) == Some(1))
            .cloned()
            .unwrap();
        while rows.len() < row_count {
            let mut row = template.clone();
            row.as_object_mut().unwrap().insert(
                "id".into(),
                Value::String(format!("calendar-{}", rows.len())),
            );
            rows.push(row);
        }
        serde_json::to_vec(&root).unwrap()
    }

    #[test]
    fn public_news_maps_all_contract_fields_and_omits_locked_rows() {
        let transport = FixtureTransport {
            response: FIXTURE.as_bytes().to_vec(),
            request: Mutex::new(None),
        };
        let client = Jin10Client::with_transport(transport);
        let batch = client
            .global_news(PositiveU32::new(20).unwrap())
            .expect("fixture should parse");
        assert_eq!(batch.records().len(), 2);
        let flash = &batch.records()[0];
        assert_eq!(flash.item_id.as_str(), "20260724224037091800");
        assert_eq!(
            flash.title.as_str(),
            "【晶泰控股：预计上半年由盈转亏】 详情"
        );
        assert_eq!(flash.publisher.as_str(), "金十数据");
        assert_eq!(
            flash.canonical_url.as_str(),
            "https://flash.jin10.com/detail/20260724224037091800"
        );
        assert_eq!(flash.published_at.as_str(), "2026-07-24T22:40:37+08:00");
        assert_eq!(flash.topics.len(), 3);
        assert_eq!(flash.language.as_str(), "zh-CN");
        assert_eq!(flash.evidence.provider(), ProviderId::Jin10);
        assert_eq!(
            batch.records()[1].canonical_url.as_str(),
            "https://xnews.jin10.com/details/225718"
        );
        assert_eq!(
            batch.provenance().source_at(),
            Some("2026-07-24T22:40:37+08:00")
        );
    }

    #[test]
    fn public_news_omits_source_channel_five_only_promotions() {
        let fixture = r#"{
          "status": 200,
          "message": "OK",
          "data": [
            {
              "id": "promotion-text",
              "time": "2026-07-25 09:01:43",
              "type": 0,
              "important": 0,
              "channel": [5],
              "data": {
                "content": "VIP年会员限时9折",
                "source": "",
                "title": "VIP·9折"
              },
              "tags": []
            },
            {
              "id": "promotion-image",
              "time": "2026-07-25 09:01:41",
              "type": 0,
              "important": 0,
              "channel": [5],
              "data": {
                "content": "<a href=\"https://www.jin10.com/activity\"><img src=\"promotion.jpg\"/></a>",
                "source": "",
                "title": ""
              },
              "tags": []
            },
            {
              "id": "public-news",
              "time": "2026-07-25 08:55:45",
              "type": 0,
              "important": 1,
              "channel": [1, 2, 3],
              "data": {
                "content": "公开财经快讯",
                "source": "",
                "title": ""
              },
              "tags": []
            }
          ]
        }"#;
        let batch = parse_response(fixture.as_bytes(), 5, "observed").unwrap();
        assert_eq!(batch.records().len(), 1);
        assert_eq!(batch.records()[0].item_id.as_str(), "public-news");
    }

    #[test]
    fn economic_release_preserves_zero_actual_and_source_fields() {
        let economic = r#"{
          "status": 200,
          "message": "OK",
          "data": [{
            "id": "202607250001",
            "time": "2026-07-25 09:30:01",
            "type": 1,
            "important": 1,
            "data": {
              "lock": false,
              "indicator_id": 950,
              "country": "中国",
              "name": "规模以上工业企业利润",
              "time_period": "6月",
              "pub_time": "2026-07-25 09:30:00",
              "previous": -9.1,
              "consensus": null,
              "actual": 0,
              "revised": null,
              "unit": "%",
              "star": 3,
              "affect": 1
            },
            "tags": []
          }]
        }"#;
        let client = Jin10Client::from_parts(
            Arc::new(FixtureTransport {
                response: economic.as_bytes().to_vec(),
                request: Mutex::new(None),
            }),
            Duration::ZERO,
        );
        let request = EconomicCalendarRequest::new(PositiveU32::new(20).unwrap())
            .unwrap()
            .with_country("中国")
            .unwrap();
        let batch = client.economic_calendar(&request).unwrap();
        assert_eq!(batch.records()[0].actual.as_ref().unwrap().as_str(), "0");
        assert_eq!(batch.records()[0].indicator_id.get(), 950);
        assert_eq!(
            batch.provenance().source_at(),
            Some("2026-07-25T09:30:01+08:00")
        );
    }

    #[test]
    fn request_uses_only_verified_public_contract() {
        let request = build_request();
        assert_eq!(
            request.url(),
            "https://flash-api.jin10.com/get_flash_list?channel=-8200&vip=1"
        );
        assert!(request
            .headers()
            .contains(&("x-app-id".into(), OFFICIAL_APP_ID.into())));
        assert!(request
            .headers()
            .contains(&("x-version".into(), OFFICIAL_API_VERSION.into())));
        assert!(ensure_official_url(request.url()).is_ok());
        assert!(ensure_official_url("https://example.com/get_flash_list").is_err());
    }

    #[test]
    fn duplicate_and_malformed_public_rows_are_explicit_failures() {
        let duplicate =
            FIXTURE.replace("\"id\": \"calendar-1\"", "\"id\": \"20260724224037091800\"");
        assert!(matches!(
            parse_response(duplicate.as_bytes(), 20, "observed"),
            Err(Jin10Error::Protocol(_))
        ));
        let malformed = FIXTURE.replace("\"content\": \"公开文章摘要\"", "\"content\": \"\"");
        assert!(matches!(
            parse_response(malformed.as_bytes(), 20, "observed"),
            Err(Jin10Error::Protocol(_))
        ));
    }

    #[test]
    fn envelope_time_and_bounds_are_strict() {
        let bad_status = FIXTURE.replace("\"status\": 200", "\"status\": 500");
        assert!(matches!(
            parse_response(bad_status.as_bytes(), 20, "observed"),
            Err(Jin10Error::Protocol(_))
        ));
        let bad_time = FIXTURE.replace("2026-07-24 22:40:37", "2026-02-30 22:40:37");
        assert!(matches!(
            parse_response(bad_time.as_bytes(), 20, "observed"),
            Err(Jin10Error::Protocol(_))
        ));
        let client = Jin10Client::with_transport(FixtureTransport {
            response: FIXTURE.as_bytes().to_vec(),
            request: Mutex::new(None),
        });
        assert!(matches!(
            client.global_news(PositiveU32::new(21).unwrap()),
            Err(Jin10Error::InvalidRequest(_))
        ));
        assert!(matches!(
            Jin10Client::with_timeout(Duration::ZERO),
            Err(Jin10Error::InvalidRequest(_))
        ));
    }

    #[test]
    fn transient_twenty_one_row_source_window_is_bounded_separately_from_caller_limit() {
        let twenty_one = fixture_with_row_count(21);
        assert!(parse_response(&twenty_one, 20, "observed").is_ok());
        let twenty_two = fixture_with_row_count(22);
        assert!(matches!(
            parse_response(&twenty_two, 20, "observed"),
            Err(Jin10Error::Protocol(_))
        ));
    }

    #[test]
    fn clones_hold_the_shared_gate_through_response_completion() {
        let (starts_tx, starts_rx) = mpsc::channel();
        let (releases_tx, releases_rx) = mpsc::channel();
        let client = Jin10Client::from_parts(
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
        assert!(ensure_json_content_type(Some("application/json; charset=utf-8")).is_ok());
        assert!(ensure_json_content_type(Some("application/not-json")).is_err());
        assert!(ensure_json_content_type(Some("text/html")).is_err());
        let oversized = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        let client = Jin10Client::with_transport(FixtureTransport {
            response: oversized,
            request: Mutex::new(None),
        });
        assert!(matches!(
            client.global_news(PositiveU32::new(1).unwrap()),
            Err(Jin10Error::Protocol(_))
        ));
    }

    #[test]
    fn injected_transports_keep_production_pacing() {
        let client = Jin10Client::with_transport(FixtureTransport {
            response: FIXTURE.as_bytes().to_vec(),
            request: Mutex::new(None),
        });
        assert_eq!(client.minimum_interval, MINIMUM_REQUEST_INTERVAL);
    }
}
