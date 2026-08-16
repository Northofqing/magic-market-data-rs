#![forbid(unsafe_code)]
//! Bounded metadata-only adapter for official Yonhap Chinese RSS feeds.

use magic_market_core::{
    ContentCapabilities, DataBatch, HttpsUrl, InstrumentDateRangeRequest, NewsItem, NewsProvider,
    NonEmptyText, PositiveU32, Provenance, ProviderId, SourceEvidence,
};
use quick_xml::events::{BytesRef, Event};
use quick_xml::reader::Reader;
use std::collections::HashSet;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use time::format_description::well_known::{Rfc2822, Rfc3339};
use time::{OffsetDateTime, UtcOffset};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_RETURNED_ITEMS: u32 = 50;
const MAX_SOURCE_ITEMS: usize = 100;

/// The fixed Economy feed passed the repository admission gate on 2026-08-16.
/// Other Yonhap channels remain explicit diagnostics and are not promoted by
/// this family-level capability.
pub const GLOBAL_NEWS_ADMITTED: bool = true;

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
        Self::for_channel(YonhapChannel::Economy)
    }

    pub fn for_channel(channel: YonhapChannel) -> Result<Self, YonhapError> {
        Self::for_channel_with_timeout(channel, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, YonhapError> {
        Self::for_channel_with_timeout(YonhapChannel::Economy, timeout)
    }

    pub fn with_transport(transport: impl YonhapTransport + 'static) -> Self {
        Self::from_parts(
            YonhapChannel::Economy,
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

    pub const fn content_capabilities() -> ContentCapabilities {
        ContentCapabilities {
            instrument_news: false,
            global_news: GLOBAL_NEWS_ADMITTED,
            announcements: false,
            market_announcements: false,
            investor_questions: false,
        }
    }

    /// Explicit diagnostic fetch while the public trait capability is subject
    /// to live admission.
    pub fn probe_global_news(
        &self,
        limit: PositiveU32,
    ) -> Result<DataBatch<NewsItem>, YonhapError> {
        validate_returned_limit(limit.get())?;
        let response = self.execute(&build_request(self.channel))?;
        let observed_at = now()?;
        parse_response(response.body(), self.channel, limit.get(), &observed_at)
    }

    pub fn for_channel_with_timeout(
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

impl NewsProvider for YonhapClient {
    type Error = YonhapError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(YonhapError::Unsupported(
            "Yonhap RSS does not expose a verified instrument/date filter".into(),
        ))
    }

    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        if self.channel != YonhapChannel::Economy {
            return Err(YonhapError::Unsupported(
                "only the fixed Yonhap Economy RSS feed is production-admitted; use probe_global_news for other explicit channel diagnostics".into(),
            ));
        }
        self.probe_global_news(limit)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemField {
    Title,
    Link,
    Guid,
    PublishedAt,
}

impl ItemField {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "title" => Some(Self::Title),
            "link" => Some(Self::Link),
            "guid" => Some(Self::Guid),
            "pubDate" => Some(Self::PublishedAt),
            _ => None,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Link => "link",
            Self::Guid => "guid",
            Self::PublishedAt => "pubDate",
        }
    }
}

#[derive(Debug, Default)]
struct RawItem {
    title: Option<String>,
    link: Option<String>,
    guid: Option<String>,
    published_at: Option<String>,
}

impl RawItem {
    fn has_field(&self, field: ItemField) -> bool {
        match field {
            ItemField::Title => self.title.is_some(),
            ItemField::Link => self.link.is_some(),
            ItemField::Guid => self.guid.is_some(),
            ItemField::PublishedAt => self.published_at.is_some(),
        }
    }

    fn set_field(&mut self, field: ItemField, value: String) {
        match field {
            ItemField::Title => self.title = Some(value),
            ItemField::Link => self.link = Some(value),
            ItemField::Guid => self.guid = Some(value),
            ItemField::PublishedAt => self.published_at = Some(value),
        }
    }
}

#[derive(Debug, Default)]
struct RssState {
    stack: Vec<String>,
    saw_rss: bool,
    closed_rss: bool,
    saw_channel: bool,
    closed_channel: bool,
    current: Option<RawItem>,
    active_field: Option<(ItemField, String)>,
    items: Vec<RawItem>,
}

impl RssState {
    fn start(&mut self, name: String) -> Result<(), YonhapError> {
        if self.active_field.is_some() {
            return Err(YonhapError::Protocol(
                "recognized RSS item fields must not contain nested elements".into(),
            ));
        }
        match self.stack.as_slice() {
            [] => {
                if name != "rss" || self.saw_rss {
                    return Err(YonhapError::Protocol(
                        "RSS document must contain exactly one rss root".into(),
                    ));
                }
                self.saw_rss = true;
            }
            [root] if root == "rss" => {
                if name != "channel" || self.saw_channel {
                    return Err(YonhapError::Protocol(
                        "RSS root must contain exactly one channel".into(),
                    ));
                }
                self.saw_channel = true;
            }
            [root, channel] if root == "rss" && channel == "channel" && name == "item" => {
                if self.current.is_some() {
                    return Err(YonhapError::Protocol(
                        "nested RSS items are not permitted".into(),
                    ));
                }
                self.current = Some(RawItem::default());
            }
            [root, channel, item] if root == "rss" && channel == "channel" && item == "item" => {
                if let Some(field) = ItemField::from_name(&name) {
                    let current = self.current.as_ref().ok_or_else(|| {
                        YonhapError::Protocol("RSS item parser state is missing".into())
                    })?;
                    if current.has_field(field) {
                        return Err(YonhapError::Protocol(format!(
                            "RSS item contains duplicate {} field",
                            field.name()
                        )));
                    }
                    self.active_field = Some((field, String::new()));
                }
            }
            _ if name == "item" => {
                return Err(YonhapError::Protocol(
                    "RSS item must be a direct channel child".into(),
                ));
            }
            _ => {}
        }
        self.stack.push(name);
        Ok(())
    }

    fn end(&mut self, name: &str) -> Result<(), YonhapError> {
        if self.stack.last().map(String::as_str) != Some(name) {
            return Err(YonhapError::Decode(format!(
                "RSS closing element {name} does not match parser state"
            )));
        }
        if self.stack.len() == 4 {
            if let Some((field, value)) = self.active_field.take() {
                if field.name() != name {
                    return Err(YonhapError::Protocol(
                        "RSS item field parser state disagrees with closing element".into(),
                    ));
                }
                self.current
                    .as_mut()
                    .ok_or_else(|| {
                        YonhapError::Protocol("RSS item parser state is missing".into())
                    })?
                    .set_field(field, value);
            }
        } else if self.stack.len() == 3 && name == "item" {
            let item = self.current.take().ok_or_else(|| {
                YonhapError::Protocol("RSS item closed without parser state".into())
            })?;
            self.items.push(item);
            if self.items.len() > MAX_SOURCE_ITEMS {
                return Err(YonhapError::Protocol(format!(
                    "Yonhap RSS exceeds the {MAX_SOURCE_ITEMS}-item source bound"
                )));
            }
        } else if self.stack.len() == 2 && name == "channel" {
            self.closed_channel = true;
        } else if self.stack.len() == 1 && name == "rss" {
            self.closed_rss = true;
        }
        self.stack.pop();
        Ok(())
    }

    fn append_text(&mut self, value: &str) {
        if let Some((_, text)) = self.active_field.as_mut() {
            text.push_str(value);
        }
    }

    fn finish(self) -> Result<Vec<RawItem>, YonhapError> {
        if !self.saw_rss
            || !self.closed_rss
            || !self.saw_channel
            || !self.closed_channel
            || !self.stack.is_empty()
            || self.current.is_some()
            || self.active_field.is_some()
        {
            return Err(YonhapError::Protocol(
                "RSS document structure is incomplete".into(),
            ));
        }
        if self.items.is_empty() {
            return Err(YonhapError::Protocol(
                "Yonhap returned an empty RSS feed".into(),
            ));
        }
        Ok(self.items)
    }
}

fn parse_response(
    body: &[u8],
    channel: YonhapChannel,
    limit: u32,
    observed_at: &str,
) -> Result<DataBatch<NewsItem>, YonhapError> {
    validate_returned_limit(limit)?;
    ensure_body_size(body)?;
    std::str::from_utf8(body)
        .map_err(|error| YonhapError::Decode(format!("RSS is not UTF-8: {error}")))?;
    if body.iter().all(u8::is_ascii_whitespace) {
        return Err(YonhapError::Protocol(
            "Yonhap returned an empty RSS body".into(),
        ));
    }

    let mut reader = Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    reader.config_mut().check_end_names = true;
    let mut state = RssState::default();
    loop {
        let event = reader
            .read_event()
            .map_err(|error| YonhapError::Decode(format!("RSS XML: {error}")))?;
        match event {
            Event::Start(element) => {
                validate_attributes(&element)?;
                state.start(xml_name(element.name().as_ref())?)?;
            }
            Event::Empty(element) => {
                validate_attributes(&element)?;
                let name = xml_name(element.name().as_ref())?;
                state.start(name.clone())?;
                state.end(&name)?;
            }
            Event::End(element) => {
                let name = xml_name(element.name().as_ref())?;
                state.end(&name)?;
            }
            Event::Text(text) => {
                let decoded = text
                    .xml10_content()
                    .map_err(|error| YonhapError::Decode(format!("RSS text: {error}")))?;
                state.append_text(&decoded);
            }
            Event::CData(text) => {
                let decoded = text
                    .xml10_content()
                    .map_err(|error| YonhapError::Decode(format!("RSS CDATA: {error}")))?;
                state.append_text(&decoded);
            }
            Event::GeneralRef(reference) => {
                let resolved = resolve_reference(&reference)?;
                state.append_text(&resolved);
            }
            Event::DocType(_) => {
                return Err(YonhapError::Protocol(
                    "Yonhap RSS must not contain a DOCTYPE".into(),
                ));
            }
            Event::Decl(declaration) => {
                if let Some(encoding) = declaration.encoding() {
                    let encoding = encoding.map_err(|error| {
                        YonhapError::Decode(format!("RSS XML declaration: {error}"))
                    })?;
                    if !encoding.eq_ignore_ascii_case(b"utf-8") {
                        return Err(YonhapError::Protocol(
                            "Yonhap RSS must declare UTF-8 when encoding is present".into(),
                        ));
                    }
                }
            }
            Event::Comment(_) | Event::PI(_) => {}
            Event::Eof => break,
        }
    }

    let raw_items = state.finish()?;
    let batch_id = format!("yonhap:{observed_at}:{}", channel.slug());
    let mut seen_ids = HashSet::with_capacity(raw_items.len());
    let mut seen_urls = HashSet::with_capacity(raw_items.len());
    let mut previous_time = None;
    let mut parsed = Vec::with_capacity(raw_items.len());
    for raw in raw_items {
        let title = required_text("title", raw.title)?;
        let canonical_url = required_text("link", raw.link)?;
        let article_id = validate_article_url(&canonical_url)?;
        if let Some(guid) = raw.guid {
            let guid = normalize_text(&guid);
            if guid.is_empty() || (guid != canonical_url && guid != article_id) {
                return Err(YonhapError::Protocol(format!(
                    "Yonhap GUID disagrees with canonical article {article_id}"
                )));
            }
        }
        let published_source = required_text("pubDate", raw.published_at)?;
        let source_time = OffsetDateTime::parse(&published_source, &Rfc2822)
            .map_err(|error| YonhapError::Protocol(format!("invalid RSS pubDate: {error}")))?;
        let korea = UtcOffset::from_hms(9, 0, 0)
            .map_err(|error| YonhapError::Protocol(format!("invalid Korea offset: {error}")))?;
        let source_time = source_time.to_offset(korea);
        if previous_time.is_some_and(|previous| previous < source_time) {
            return Err(YonhapError::Protocol(
                "Yonhap RSS items are not newest-first".into(),
            ));
        }
        previous_time = Some(source_time);
        let published_at = source_time
            .format(&Rfc3339)
            .map_err(|error| YonhapError::Protocol(format!("RSS time format: {error}")))?;
        if !seen_ids.insert(article_id.clone()) || !seen_urls.insert(canonical_url.clone()) {
            return Err(YonhapError::Protocol(format!(
                "duplicate Yonhap article identity {article_id}"
            )));
        }
        let evidence = SourceEvidence::new(ProviderId::Yonhap, observed_at, &batch_id)?
            .with_source_at(&published_at)?;
        parsed.push(NewsItem {
            item_id: NonEmptyText::new(article_id)?,
            title: NonEmptyText::new(title)?,
            summary: None,
            content: None,
            publisher: NonEmptyText::new("韩联社")?,
            canonical_url: HttpsUrl::new(canonical_url)?,
            published_at: NonEmptyText::new(published_at)?,
            instruments: Vec::new(),
            topics: vec![NonEmptyText::new(channel.topic())?],
            language: NonEmptyText::new("zh-CN")?,
            evidence,
        });
    }

    parsed.truncate(limit as usize);
    let source_at = parsed
        .first()
        .map(|item| item.published_at.as_str())
        .ok_or_else(|| YonhapError::Protocol("latest Yonhap source time is missing".into()))?;
    let provenance = Provenance::new("yonhap-cn-rss-v1", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(parsed, provenance))
}

fn validate_attributes(element: &quick_xml::events::BytesStart<'_>) -> Result<(), YonhapError> {
    for attribute in element.attributes() {
        attribute.map_err(|error| YonhapError::Decode(format!("RSS XML attribute: {error}")))?;
    }
    Ok(())
}

fn xml_name(bytes: &[u8]) -> Result<String, YonhapError> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|error| YonhapError::Decode(format!("RSS element name is not UTF-8: {error}")))
}

fn resolve_reference(reference: &BytesRef<'_>) -> Result<String, YonhapError> {
    if let Some(character) = reference
        .resolve_char_ref()
        .map_err(|error| YonhapError::Protocol(format!("invalid numeric entity: {error}")))?
    {
        return Ok(character.to_string());
    }
    let name = reference
        .decode()
        .map_err(|error| YonhapError::Decode(format!("RSS entity name: {error}")))?;
    let value = match name.as_ref() {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "apos" => "'",
        "quot" => "\"",
        _ => {
            return Err(YonhapError::Protocol(format!(
                "custom RSS entity &{name}; is not permitted"
            )));
        }
    };
    Ok(value.to_owned())
}

fn required_text(field: &'static str, value: Option<String>) -> Result<String, YonhapError> {
    let value = value.ok_or_else(|| {
        YonhapError::Protocol(format!("Yonhap RSS item is missing required {field}"))
    })?;
    let normalized = normalize_text(&value);
    if normalized.is_empty() {
        return Err(YonhapError::Protocol(format!(
            "Yonhap RSS item {field} must not be empty"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(YonhapError::Protocol(format!(
            "Yonhap RSS item {field} contains control characters"
        )));
    }
    Ok(normalized)
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_article_url(url: &str) -> Result<String, YonhapError> {
    const PREFIX: &str = "https://cn.yna.co.kr/view/";
    let article_id = url.strip_prefix(PREFIX).ok_or_else(|| {
        YonhapError::Protocol("Yonhap article URL must use the exact official HTTPS path".into())
    })?;
    let digits = article_id.strip_prefix("ACK").ok_or_else(|| {
        YonhapError::Protocol("Yonhap Chinese article ID must begin with ACK".into())
    })?;
    if digits.len() != 17 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(YonhapError::Protocol(
            "Yonhap Chinese article ID must contain ACK plus 17 ASCII digits".into(),
        ));
    }
    Ok(article_id.to_owned())
}

fn now() -> Result<String, YonhapError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| YonhapError::Transport(format!("local observation clock: {error}")))
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

    fn rss_item(id: &str, title: &str, published_at: &str) -> String {
        format!(
            "<item>\
               <title>{title}</title>\
               <link>https://cn.yna.co.kr/view/{id}</link>\
               <guid isPermaLink=\"true\">https://cn.yna.co.kr/view/{id}</guid>\
               <pubDate>{published_at}</pubDate>\
             </item>"
        )
    }

    fn rss_feed(items: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
             <rss version=\"2.0\" xmlns:content=\"http://purl.org/rss/1.0/modules/content/\">\
               <channel><title>韩联社中文</title>{items}</channel>\
             </rss>"
        )
    }

    fn valid_rss() -> String {
        let first = "<item>\
               <title>韩国与美国扩大芯片合作</title>\
               <link>https://cn.yna.co.kr/view/ACK20260725001100881</link>\
               <guid>https://cn.yna.co.kr/view/ACK20260725001100881</guid>\
               <pubDate>Sat, 25 Jul 2026 15:35:00 +0900</pubDate>\
               <description><![CDATA[NEVER_EXPOSE_DESCRIPTION]]></description>\
               <content:encoded><![CDATA[NEVER_EXPOSE_BODY]]></content:encoded>\
             </item>"
            .to_owned();
        let second = rss_item(
            "ACK20260725001000881",
            "韩国&amp;亚洲市场动态",
            "Sat, 25 Jul 2026 15:30:00 +0900",
        );
        let third = rss_item(
            "ACK20260725000900881",
            "韩国社会资讯",
            "Sat, 25 Jul 2026 15:25:00 +0900",
        );
        rss_feed(&format!("{first}{second}{third}"))
    }

    fn parse_fixture(
        body: &[u8],
        channel: YonhapChannel,
        limit: u32,
    ) -> Result<magic_market_core::DataBatch<magic_market_core::NewsItem>, YonhapError> {
        parse_response(body, channel, limit, "2026-07-25T15:36:00+09:00")
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
            YonhapChannel::Economy
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

    #[test]
    fn parser_maps_valid_feed_to_metadata_only_news() {
        let response = HttpResponse::new(
            YonhapChannel::Economy.endpoint(),
            Some("application/rss+xml; charset=utf-8".into()),
            valid_rss().into_bytes(),
        );
        let client = YonhapClient::from_parts(
            YonhapChannel::Economy,
            Arc::new(StaticTransport {
                response,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Duration::ZERO,
        );
        let batch = client
            .probe_global_news(magic_market_core::PositiveU32::new(2).unwrap())
            .unwrap();

        assert_eq!(batch.records().len(), 2);
        let first = &batch.records()[0];
        assert_eq!(first.item_id.as_str(), "ACK20260725001100881");
        assert_eq!(first.title.as_str(), "韩国与美国扩大芯片合作");
        assert_eq!(first.publisher.as_str(), "韩联社");
        assert_eq!(
            first.canonical_url.as_str(),
            "https://cn.yna.co.kr/view/ACK20260725001100881"
        );
        assert_eq!(first.published_at.as_str(), "2026-07-25T15:35:00+09:00");
        assert!(first.summary.is_none());
        assert!(first.content.is_none());
        assert!(first.instruments.is_empty());
        assert_eq!(first.topics[0].as_str(), "经济");
        assert_eq!(first.language.as_str(), "zh-CN");
        assert_eq!(
            first.evidence.provider(),
            magic_market_core::ProviderId::Yonhap
        );
        assert_eq!(
            first.evidence.source_at(),
            Some("2026-07-25T15:35:00+09:00")
        );
        assert_eq!(batch.provenance().source(), "yonhap-cn-rss-v1");
        assert_eq!(
            batch.provenance().source_at(),
            Some("2026-07-25T15:35:00+09:00")
        );
        assert!(batch.quality().is_complete());
        assert_eq!(batch.records()[1].title.as_str(), "韩国&亚洲市场动态");
    }

    #[test]
    fn parser_ignores_description_content_and_extensions() {
        let batch = parse_fixture(valid_rss().as_bytes(), YonhapChannel::Economy, 3).unwrap();
        let debug = format!("{batch:?}");
        assert!(!debug.contains("NEVER_EXPOSE_DESCRIPTION"));
        assert!(!debug.contains("NEVER_EXPOSE_BODY"));
        assert!(batch
            .records()
            .iter()
            .all(|item| item.summary.is_none() && item.content.is_none()));
    }

    #[test]
    fn parser_rejects_empty_malformed_and_non_utf8_xml() {
        for body in [
            b"".as_slice(),
            b" \n\t".as_slice(),
            b"<rss><channel><item></channel></rss>".as_slice(),
            b"<rss><channel>".as_slice(),
        ] {
            assert!(parse_fixture(body, YonhapChannel::Rolling, 1).is_err());
        }
        let invalid_utf8 = b"<rss><channel><item><title>\xff</title></item></channel></rss>";
        assert!(matches!(
            parse_fixture(invalid_utf8, YonhapChannel::Rolling, 1),
            Err(YonhapError::Decode(_))
        ));
    }

    #[test]
    fn parser_rejects_doctype_and_custom_named_entities() {
        let with_doctype = b"<!DOCTYPE rss [<!ENTITY xxe SYSTEM \"file:///etc/passwd\">]><rss/>";
        assert!(matches!(
            parse_fixture(with_doctype, YonhapChannel::Rolling, 1),
            Err(YonhapError::Protocol(message)) if message.contains("DOCTYPE")
        ));
        let custom = rss_feed(
            "<item><title>&custom;</title>\
             <link>https://cn.yna.co.kr/view/ACK20260725001100881</link>\
             <pubDate>Sat, 25 Jul 2026 15:35:00 +0900</pubDate></item>",
        );
        assert!(matches!(
            parse_fixture(custom.as_bytes(), YonhapChannel::Rolling, 1),
            Err(YonhapError::Protocol(message)) if message.contains("entity")
        ));
    }

    #[test]
    fn parser_accepts_only_predefined_and_numeric_references() {
        let item = rss_item(
            "ACK20260725001100881",
            "A&amp;B&lt;C&gt;&#x4E2D;&#25991;&quot;&apos;",
            "Sat, 25 Jul 2026 15:35:00 +0900",
        );
        let batch = parse_fixture(rss_feed(&item).as_bytes(), YonhapChannel::Rolling, 1).unwrap();
        assert_eq!(batch.records()[0].title.as_str(), "A&B<C>中文\"'");
    }

    #[test]
    fn parser_rejects_control_character_references() {
        let item = rss_item(
            "ACK20260725001100881",
            "invalid&#x1;title",
            "Sat, 25 Jul 2026 15:35:00 +0900",
        );
        assert!(matches!(
            parse_fixture(rss_feed(&item).as_bytes(), YonhapChannel::Rolling, 1),
            Err(YonhapError::Protocol(message)) if message.contains("control")
        ));
    }

    #[test]
    fn parser_rejects_wrong_structure_and_source_row_bounds() {
        for body in [
            "<channel></channel>".to_owned(),
            "<rss></rss>".to_owned(),
            "<rss><item/></rss>".to_owned(),
            rss_feed(""),
        ] {
            assert!(parse_fixture(body.as_bytes(), YonhapChannel::Rolling, 1).is_err());
        }

        let items = (0..=MAX_SOURCE_ITEMS)
            .map(|index| {
                rss_item(
                    &format!("ACK20260725{index:09}"),
                    "bounded row",
                    "Sat, 25 Jul 2026 15:35:00 +0900",
                )
            })
            .collect::<String>();
        assert!(matches!(
            parse_fixture(rss_feed(&items).as_bytes(), YonhapChannel::Rolling, 1),
            Err(YonhapError::Protocol(message)) if message.contains("100")
        ));
    }

    #[test]
    fn parser_rejects_missing_or_empty_required_fields() {
        let valid = rss_item(
            "ACK20260725001100881",
            "required",
            "Sat, 25 Jul 2026 15:35:00 +0900",
        );
        for invalid in [
            valid.replace("<title>required</title>", ""),
            valid.replace("required", "   "),
            valid.replace(
                "<link>https://cn.yna.co.kr/view/ACK20260725001100881</link>",
                "",
            ),
            valid.replace("<pubDate>Sat, 25 Jul 2026 15:35:00 +0900</pubDate>", ""),
        ] {
            assert!(
                parse_fixture(rss_feed(&invalid).as_bytes(), YonhapChannel::Rolling, 1).is_err()
            );
        }
    }

    #[test]
    fn parser_rejects_noncanonical_article_urls() {
        let canonical = "https://cn.yna.co.kr/view/ACK20260725001100881";
        for invalid in [
            "http://cn.yna.co.kr/view/ACK20260725001100881",
            "https://user@cn.yna.co.kr/view/ACK20260725001100881",
            "https://cn.yna.co.kr:444/view/ACK20260725001100881",
            "https://cn.yna.co.kr.example/view/ACK20260725001100881",
            "https://cn.yna.co.kr/view/ACK20260725001100881?x=1",
            "https://cn.yna.co.kr/view/ACK20260725001100881#x",
            "https://cn.yna.co.kr/news/ACK20260725001100881",
            "https://cn.yna.co.kr/view/AEN20260725001100881",
            "https://cn.yna.co.kr/view/ACK2026072500110088",
            "https://cn.yna.co.kr/view/ACK202607250011008811",
            "https://cn.yna.co.kr/view/ACK2026072500110088A",
        ] {
            let item = rss_item(
                "ACK20260725001100881",
                "canonical",
                "Sat, 25 Jul 2026 15:35:00 +0900",
            )
            .replace(canonical, invalid);
            assert!(
                parse_fixture(rss_feed(&item).as_bytes(), YonhapChannel::Rolling, 1).is_err(),
                "unexpectedly accepted {invalid}"
            );
        }
    }

    #[test]
    fn parser_rejects_guid_disagreement() {
        let item = rss_item(
            "ACK20260725001100881",
            "guid",
            "Sat, 25 Jul 2026 15:35:00 +0900",
        )
        .replace(
            "<guid isPermaLink=\"true\">https://cn.yna.co.kr/view/ACK20260725001100881</guid>",
            "<guid>https://cn.yna.co.kr/view/ACK20260725001000881</guid>",
        );
        assert!(matches!(
            parse_fixture(rss_feed(&item).as_bytes(), YonhapChannel::Rolling, 1),
            Err(YonhapError::Protocol(message)) if message.contains("GUID")
        ));
    }

    #[test]
    fn parser_rejects_bad_time_and_source_order_regression() {
        let bad_time = rss_item("ACK20260725001100881", "bad time", "2026-07-25 15:35");
        assert!(parse_fixture(rss_feed(&bad_time).as_bytes(), YonhapChannel::Rolling, 1).is_err());

        let older = rss_item(
            "ACK20260725001000881",
            "older",
            "Sat, 25 Jul 2026 15:30:00 +0900",
        );
        let newer = rss_item(
            "ACK20260725001100881",
            "newer",
            "Sat, 25 Jul 2026 15:35:00 +0900",
        );
        assert!(matches!(
            parse_fixture(
                rss_feed(&format!("{older}{newer}")).as_bytes(),
                YonhapChannel::Rolling,
                2
            ),
            Err(YonhapError::Protocol(message)) if message.contains("newest-first")
        ));
    }

    #[test]
    fn parser_rejects_duplicate_ids_and_urls() {
        let item = rss_item(
            "ACK20260725001100881",
            "duplicate",
            "Sat, 25 Jul 2026 15:35:00 +0900",
        );
        assert!(matches!(
            parse_fixture(
                rss_feed(&format!("{item}{item}")).as_bytes(),
                YonhapChannel::Rolling,
                2
            ),
            Err(YonhapError::Protocol(message)) if message.contains("duplicate")
        ));
    }

    #[test]
    fn parser_validates_complete_feed_before_truncating() {
        let mut feed = valid_rss();
        feed = feed.replace(
            "</channel>",
            "<item><title>invalid trailing row</title></item></channel>",
        );
        assert!(parse_fixture(feed.as_bytes(), YonhapChannel::Rolling, 1).is_err());
    }
}
