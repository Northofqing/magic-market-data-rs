use crate::transport::{
    map_shared_error, new_request_gate, validate_minimum_interval, validate_response,
    validate_timeout, wait_for_request_start, ExchangeTransport, HttpMethod, HttpRequest,
    HttpsTransport, SharedMediaType, SharedRequestGate, TlsBackend, MAX_RESPONSE_BYTES,
};
use crate::ExchangeError;
use magic_market_core::{
    CalendarCapabilities, DataBatch, FuturesDeliveryCalendar, FuturesDeliveryEvent,
    FuturesDeliveryMethod, FuturesDeliveryRequest, FuturesProduct, HttpsUrl, IsoDate, NonEmptyText,
    Provenance, ProviderId, SourceEvidence,
};
use magic_market_transport::{
    EndpointPolicy, HttpMethod as SharedHttpMethod, HttpRequest as SharedHttpRequest,
    HttpTransport as SharedHttpTransport, ReqwestTransport,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

const HOST: &str = "www.cffex.com.cn";
const LIST_ENDPOINT: &str = "https://www.cffex.com.cn/cn/jystz.html";
const PLAIN_HTTP_LIST_ENDPOINT: &str = "http://www.cffex.com.cn/cn/jystz.html";
const DELIVERY_TITLE: &str = "股指期货和股指期权合约交割的通知";
const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; magic-exchange-rs/0.2; read-only CFFEX notice parser)";

/// The formal production scope is a checked-in, revisioned 2026 schedule. It
/// performs no runtime HTTP and therefore cannot downgrade transport security.
pub const CFFEX_2026_FUTURES_DELIVERY_ADMITTED: bool = true;
const FIXED_SCHEDULE_YEAR: u32 = 2026;
const FIXED_SCHEDULE_REVISION: &str = "cffex-equity-index-delivery-2026-v1";
const FIXED_SCHEDULE_NOTICE_URL: &str = "https://www.cffex.com.cn/jystz/20251217/46425.html";
const FIXED_DELIVERY_DATES: [&str; 12] = [
    "2026-01-16",
    "2026-02-24",
    "2026-03-20",
    "2026-04-17",
    "2026-05-15",
    "2026-06-22",
    "2026-07-17",
    "2026-08-21",
    "2026-09-18",
    "2026-10-16",
    "2026-11-20",
    "2026-12-18",
];

pub type CffexTlsBackend = TlsBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CffexAccessMode {
    Https,
    PlainHttpDiagnostic,
}

impl CffexAccessMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::PlainHttpDiagnostic => "plaintext_http_diagnostic",
        }
    }

    const fn scheme(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::PlainHttpDiagnostic => "http",
        }
    }

    const fn port(self) -> u16 {
        match self {
            Self::Https => 443,
            Self::PlainHttpDiagnostic => 80,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CffexConfig {
    pub list_endpoint: String,
    pub timeout: Duration,
    pub minimum_interval: Duration,
    pub max_pages: u32,
    pub tls_backend: CffexTlsBackend,
    pub access_mode: CffexAccessMode,
}

impl Default for CffexConfig {
    fn default() -> Self {
        Self {
            list_endpoint: LIST_ENDPOINT.into(),
            timeout: Duration::from_secs(20),
            minimum_interval: Duration::from_secs(1),
            max_pages: 120,
            tls_backend: CffexTlsBackend::Rustls,
            access_mode: CffexAccessMode::Https,
        }
    }
}

impl CffexConfig {
    pub fn plaintext_http_diagnostic() -> Self {
        Self {
            list_endpoint: PLAIN_HTTP_LIST_ENDPOINT.into(),
            access_mode: CffexAccessMode::PlainHttpDiagnostic,
            ..Self::default()
        }
    }

    fn validate(&self) -> Result<(), ExchangeError> {
        let expected_endpoint = match self.access_mode {
            CffexAccessMode::Https => LIST_ENDPOINT,
            CffexAccessMode::PlainHttpDiagnostic => PLAIN_HTTP_LIST_ENDPOINT,
        };
        if self.list_endpoint != expected_endpoint {
            return Err(ExchangeError::InvalidRequest(
                "CFFEX list endpoint must exactly match its fixed access mode".into(),
            ));
        }
        validate_timeout(self.timeout)?;
        validate_minimum_interval(self.minimum_interval)?;
        if self.max_pages == 0 || self.max_pages > 120 {
            return Err(ExchangeError::InvalidRequest(
                "CFFEX max_pages must be between 1 and 120".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
struct PlainHttpTransport {
    agent: ureq::Agent,
}

impl PlainHttpTransport {
    fn new(timeout: Duration) -> Result<Self, ExchangeError> {
        validate_timeout(timeout)?;
        Ok(Self {
            agent: ureq::AgentBuilder::new()
                .timeout_connect(timeout)
                .timeout_read(timeout)
                .timeout_write(timeout)
                .redirects(0)
                .try_proxy_from_env(false)
                .build(),
        })
    }
}

impl ExchangeTransport for PlainHttpTransport {
    fn execute(
        &self,
        request: &HttpRequest,
    ) -> Result<crate::transport::HttpResponse, ExchangeError> {
        if request.method != HttpMethod::Get || !request.body.is_empty() {
            return Err(ExchangeError::InvalidRequest(
                "CFFEX plaintext transport accepts only bodyless GET requests".into(),
            ));
        }
        let mut wire = self.agent.get(&request.url);
        for (name, value) in &request.headers {
            wire = wire.set(name, value);
        }
        match wire.call() {
            Ok(response) => HttpsTransport::collect(response),
            Err(ureq::Error::Status(_, response)) => HttpsTransport::collect(response),
            Err(ureq::Error::Transport(error)) => Err(ExchangeError::Transport(error.to_string())),
        }
    }
}

#[derive(Clone)]
struct SharedCffexHttpsTransport {
    inner: ReqwestTransport,
}

impl SharedCffexHttpsTransport {
    fn new(timeout: Duration) -> Result<Self, ExchangeError> {
        validate_timeout(timeout)?;
        let mut paths = Vec::with_capacity(120);
        paths.push("/cn/jystz.html".to_owned());
        for page in 2..=120 {
            paths.push(format!("/cn/jystz_{page}.html"));
        }
        paths.push("/cn/jystz/".to_owned());
        let policy = EndpointPolicy::new(
            HOST,
            paths,
            Vec::new(),
            vec![SharedMediaType::Html],
            MAX_RESPONSE_BYTES,
            timeout,
        )
        .map_err(map_shared_error)?;
        Ok(Self {
            inner: ReqwestTransport::new(policy).map_err(map_shared_error)?,
        })
    }
}

impl ExchangeTransport for SharedCffexHttpsTransport {
    fn execute(
        &self,
        request: &HttpRequest,
    ) -> Result<crate::transport::HttpResponse, ExchangeError> {
        let method = match request.method {
            HttpMethod::Get => SharedHttpMethod::Get,
            HttpMethod::Post => SharedHttpMethod::Post,
        };
        let shared_request = SharedHttpRequest::new(
            method,
            request.url.clone(),
            request.headers.clone(),
            request.body.clone(),
        )
        .map_err(map_shared_error)?;
        let response = self
            .inner
            .execute(&shared_request)
            .map_err(map_shared_error)?;
        Ok(crate::transport::HttpResponse {
            status: response.status(),
            final_url: response.final_url().to_owned(),
            content_type: response.content_type().map(str::to_owned),
            body: response.body().to_vec(),
        })
    }
}

#[derive(Clone)]
pub struct CffexClient {
    config: CffexConfig,
    transport: Arc<dyn ExchangeTransport>,
    gate: Arc<SharedRequestGate>,
}

impl std::fmt::Debug for CffexClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CffexClient")
            .field("config", &self.config)
            .field("tls_backend", &self.config.tls_backend.as_str())
            .field("access_mode", &self.config.access_mode.as_str())
            .finish_non_exhaustive()
    }
}

impl CffexClient {
    pub fn new() -> Result<Self, ExchangeError> {
        Self::with_config(CffexConfig::default())
    }

    pub fn with_config(config: CffexConfig) -> Result<Self, ExchangeError> {
        config.validate()?;
        let transport: Arc<dyn ExchangeTransport> = match config.access_mode {
            CffexAccessMode::Https => {
                if config.tls_backend != CffexTlsBackend::Rustls {
                    return Err(ExchangeError::Unsupported(
                        "CFFEX formal HTTPS uses only the shared reqwest/rustls transport".into(),
                    ));
                }
                Arc::new(SharedCffexHttpsTransport::new(config.timeout)?)
            }
            CffexAccessMode::PlainHttpDiagnostic => {
                Arc::new(PlainHttpTransport::new(config.timeout)?)
            }
        };
        Self::from_parts(config, transport)
    }

    pub fn with_transport(
        config: CffexConfig,
        transport: impl ExchangeTransport + 'static,
    ) -> Result<Self, ExchangeError> {
        config.validate()?;
        Self::from_parts(config, Arc::new(transport))
    }

    fn from_parts(
        config: CffexConfig,
        transport: Arc<dyn ExchangeTransport>,
    ) -> Result<Self, ExchangeError> {
        Ok(Self {
            gate: Arc::new(new_request_gate(config.minimum_interval)?),
            config,
            transport,
        })
    }

    pub const fn provider_id() -> ProviderId {
        ProviderId::Cffex
    }

    pub const fn tls_backend(&self) -> CffexTlsBackend {
        self.config.tls_backend
    }

    pub const fn access_mode(&self) -> CffexAccessMode {
        self.config.access_mode
    }

    pub const fn calendar_capabilities() -> CalendarCapabilities {
        CalendarCapabilities {
            economic_releases: false,
            futures_delivery: CFFEX_2026_FUTURES_DELIVERY_ADMITTED,
        }
    }

    /// Exercises the separately unadmitted bounded official-notice diagnostic.
    /// It does not alter the checked-in production schedule's admission state.
    pub fn probe_futures_delivery_calendar(
        &self,
        request: &FuturesDeliveryRequest,
    ) -> Result<DataBatch<FuturesDeliveryEvent>, ExchangeError> {
        self.fetch_futures_delivery_calendar(request)
    }

    fn fetch_futures_delivery_calendar(
        &self,
        request: &FuturesDeliveryRequest,
    ) -> Result<DataBatch<FuturesDeliveryEvent>, ExchangeError> {
        let (notice_url, published_date, detail) = self.find_notice(request)?;
        let observed_at = now()?;
        parse_delivery_notice(
            &detail,
            request,
            &notice_url,
            &published_date,
            &observed_at,
            self.config.access_mode,
        )
    }

    fn get_html(&self, url: &str) -> Result<Vec<u8>, ExchangeError> {
        validate_cffex_url(url, self.config.access_mode)?;
        let referer = format!("{}://{HOST}/", self.config.access_mode.scheme());
        let request = HttpRequest {
            method: HttpMethod::Get,
            url: url.to_owned(),
            headers: vec![
                ("User-Agent".into(), USER_AGENT.into()),
                ("Accept".into(), "text/html,application/xhtml+xml".into()),
                ("Referer".into(), referer),
            ],
            body: Vec::new(),
        };
        let parsed =
            Url::parse(url).map_err(|error| ExchangeError::InvalidRequest(error.to_string()))?;
        let https_policy = match self.config.access_mode {
            CffexAccessMode::Https => Some(crate::transport::validate_request(
                &request,
                HttpMethod::Get,
                HOST,
                parsed.path(),
                &[],
                &[SharedMediaType::Html],
                self.config.timeout,
            )?),
            CffexAccessMode::PlainHttpDiagnostic => {
                validate_plain_http_request(&request)?;
                None
            }
        };
        wait_for_request_start(&self.gate)?;
        let response = self.transport.execute(&request)?;
        match self.config.access_mode {
            CffexAccessMode::Https => {
                let policy = https_policy.as_ref().ok_or_else(|| {
                    ExchangeError::InvalidRequest("CFFEX HTTPS policy is missing".into())
                })?;
                validate_response(policy, &request, &response)?;
            }
            CffexAccessMode::PlainHttpDiagnostic => {
                validate_plain_http_response(&request.url, &response)?;
            }
        }
        Ok(response.body)
    }

    fn find_notice(
        &self,
        request: &FuturesDeliveryRequest,
    ) -> Result<(String, IsoDate, Vec<u8>), ExchangeError> {
        let target_month = format!("{:04}-{:02}", request.year().get(), request.month().get());
        for page in 1..=self.config.max_pages {
            let page_url = if page == 1 {
                self.config.list_endpoint.clone()
            } else {
                format!(
                    "{}://{HOST}/cn/jystz_{page}.html",
                    self.config.access_mode.scheme()
                )
            };
            let body = self.get_html(&page_url)?;
            let html = std::str::from_utf8(&body)
                .map_err(|error| ExchangeError::Decode(format!("CFFEX list UTF-8: {error}")))?;
            let links = parse_notice_links(html)?;
            if links.is_empty() {
                return Err(ExchangeError::Schema(format!(
                    "CFFEX notice page {page} contains no dated notice links"
                )));
            }
            for link in &links {
                if link.title.contains(DELIVERY_TITLE) && link.date.starts_with(&target_month) {
                    let url = official_notice_url(&link.href, self.config.access_mode)?;
                    let published_date = IsoDate::new(link.date.clone())?;
                    let detail = self.get_html(&url)?;
                    return Ok((url, published_date, detail));
                }
            }
            let oldest = links
                .iter()
                .map(|link| link.date.as_str())
                .min()
                .ok_or_else(|| ExchangeError::Schema("CFFEX list has no date".into()))?;
            if oldest
                .get(..7)
                .is_some_and(|month| month < target_month.as_str())
            {
                return Err(ExchangeError::Incomplete(format!(
                    "CFFEX has no official equity-index delivery notice for {target_month}"
                )));
            }
        }
        Err(ExchangeError::Incomplete(format!(
            "CFFEX notice search exceeded {} pages",
            self.config.max_pages
        )))
    }
}

impl FuturesDeliveryCalendar for CffexClient {
    type Error = ExchangeError;

    fn futures_delivery_calendar(
        &self,
        request: &FuturesDeliveryRequest,
    ) -> Result<DataBatch<FuturesDeliveryEvent>, Self::Error> {
        fixed_2026_futures_delivery_calendar(request)
    }
}

fn fixed_2026_futures_delivery_calendar(
    request: &FuturesDeliveryRequest,
) -> Result<DataBatch<FuturesDeliveryEvent>, ExchangeError> {
    if request.year().get() != FIXED_SCHEDULE_YEAR {
        return Err(ExchangeError::Unsupported(format!(
            "formal CFFEX futures delivery is admitted only for {FIXED_SCHEDULE_YEAR}; requested {}",
            request.year().get()
        )));
    }
    let month = request.month().get();
    let index = usize::try_from(month - 1)
        .map_err(|_| ExchangeError::InvalidRequest("CFFEX month index overflow".into()))?;
    let date = *FIXED_DELIVERY_DATES
        .get(index)
        .ok_or_else(|| ExchangeError::InvalidRequest("CFFEX month is outside 1..=12".into()))?;
    let delivery_date = IsoDate::new(date)?;
    let observed_at = now()?;
    let batch_id = format!("{FIXED_SCHEDULE_REVISION}:{month:02}");
    let provenance = Provenance::new(FIXED_SCHEDULE_REVISION, observed_at.clone())?
        .with_batch_id(batch_id.clone())?;
    let evidence = SourceEvidence::new(ProviderId::Cffex, observed_at, batch_id)?;
    let notice_url = HttpsUrl::new(FIXED_SCHEDULE_NOTICE_URL)?;
    let suffix = format!("{:02}{month:02}", FIXED_SCHEDULE_YEAR % 100);
    let records = [
        (FuturesProduct::If, "IF"),
        (FuturesProduct::Ih, "IH"),
        (FuturesProduct::Ic, "IC"),
        (FuturesProduct::Im, "IM"),
    ]
    .into_iter()
    .map(|(product, prefix)| {
        Ok(FuturesDeliveryEvent {
            product,
            contract_code: NonEmptyText::new(format!("{prefix}{suffix}"))?,
            last_trading_date: Some(delivery_date.clone()),
            delivery_date: delivery_date.clone(),
            method: FuturesDeliveryMethod::Cash,
            notice_url: notice_url.clone(),
            evidence: evidence.clone(),
        })
    })
    .collect::<Result<Vec<_>, ExchangeError>>()?;
    Ok(DataBatch::strict(records, provenance))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NoticeLink {
    href: String,
    title: String,
    date: String,
}

fn parse_notice_links(html: &str) -> Result<Vec<NoticeLink>, ExchangeError> {
    let mut links = Vec::new();
    let mut remainder = html;
    while let Some(start) = remainder.find("<a") {
        remainder = &remainder[start + 2..];
        let Some(tag_end) = remainder.find('>') else {
            return Err(ExchangeError::Schema(
                "CFFEX list contains an unterminated anchor".into(),
            ));
        };
        let tag = &remainder[..tag_end];
        let after_tag = &remainder[tag_end + 1..];
        let Some(close) = after_tag.find("</a>") else {
            remainder = after_tag;
            continue;
        };
        let title = strip_html(&after_tag[..close]);
        let href = extract_attribute(tag, "href");
        let after_anchor = &after_tag[close + 4..];
        if let (Some(href), Some(date)) =
            (href, find_iso_date(bounded_utf8_prefix(after_anchor, 240)))
        {
            if href.contains("/cn/jystz/") && href.ends_with(".html") {
                links.push(NoticeLink { href, title, date });
            }
        }
        remainder = after_anchor;
    }
    let mut identities = HashSet::with_capacity(links.len());
    for link in &links {
        if !identities.insert(link.href.as_str()) {
            return Err(ExchangeError::Schema(format!(
                "CFFEX list contains duplicate notice link {}",
                link.href
            )));
        }
    }
    Ok(links)
}

fn bounded_utf8_prefix(value: &str, maximum_bytes: usize) -> &str {
    if value.len() <= maximum_bytes {
        return value;
    }
    let mut end = maximum_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn extract_attribute(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(start) = tag.find(&needle) {
            let value = &tag[start + needle.len()..];
            if let Some(end) = value.find(quote) {
                return Some(value[..end].trim().to_owned());
            }
        }
    }
    None
}

fn find_iso_date(value: &str) -> Option<String> {
    value.as_bytes().windows(10).find_map(|window| {
        let candidate = std::str::from_utf8(window).ok()?;
        IsoDate::new(candidate).ok().map(|_| candidate.to_owned())
    })
}

fn official_notice_url(value: &str, access_mode: CffexAccessMode) -> Result<String, ExchangeError> {
    let expected_prefix = format!("{}://", access_mode.scheme());
    let url = if value.starts_with(&expected_prefix) {
        value.to_owned()
    } else {
        let path = if value.starts_with('/') {
            value.to_owned()
        } else {
            format!("/cn/jystz/{value}")
        };
        format!("{}://{HOST}{path}", access_mode.scheme())
    };
    validate_cffex_url(&url, access_mode)?;
    if !is_detail_path(
        Url::parse(&url)
            .map_err(|error| ExchangeError::Schema(error.to_string()))?
            .path(),
    ) {
        return Err(ExchangeError::Schema(
            "CFFEX delivery link is not a dated detail path".into(),
        ));
    }
    Ok(url)
}

fn validate_cffex_url(value: &str, access_mode: CffexAccessMode) -> Result<(), ExchangeError> {
    let url =
        Url::parse(value).map_err(|error| ExchangeError::InvalidRequest(error.to_string()))?;
    let path = url.path();
    let list_path = path == "/cn/jystz.html"
        || path
            .strip_prefix("/cn/jystz_")
            .and_then(|value| value.strip_suffix(".html"))
            .is_some_and(|page| {
                page.parse::<u32>()
                    .is_ok_and(|page| (2..=120).contains(&page))
            });
    if url.scheme() != access_mode.scheme()
        || url.host_str() != Some(HOST)
        || url.port_or_known_default() != Some(access_mode.port())
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (!list_path && !is_detail_path(path))
    {
        return Err(ExchangeError::InvalidRequest(
            "CFFEX request URL is outside the official bounded notice paths".into(),
        ));
    }
    Ok(())
}

fn validate_plain_http_request(request: &HttpRequest) -> Result<(), ExchangeError> {
    validate_cffex_url(&request.url, CffexAccessMode::PlainHttpDiagnostic)?;
    if request.method != HttpMethod::Get || !request.body.is_empty() {
        return Err(ExchangeError::InvalidRequest(
            "CFFEX plaintext diagnostic must be a bodyless GET".into(),
        ));
    }
    if request.headers.iter().any(|(name, _)| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "cookie" | "authorization"
        )
    }) {
        return Err(ExchangeError::InvalidRequest(
            "CFFEX plaintext diagnostic cannot send credentials".into(),
        ));
    }
    Ok(())
}

fn validate_plain_http_response(
    request_url: &str,
    response: &crate::transport::HttpResponse,
) -> Result<(), ExchangeError> {
    if response.status != 200 {
        return Err(ExchangeError::HttpStatus(response.status));
    }
    if response.final_url != request_url {
        return Err(ExchangeError::Schema(
            "CFFEX plaintext diagnostic redirect or final URL mismatch".into(),
        ));
    }
    let content_type = response
        .content_type
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !content_type
        .split(';')
        .next()
        .is_some_and(|media_type| media_type.trim() == "text/html")
    {
        return Err(ExchangeError::Schema(
            "CFFEX plaintext diagnostic requires text/html".into(),
        ));
    }
    if response.body.len() > MAX_RESPONSE_BYTES {
        return Err(ExchangeError::Incomplete(format!(
            "response body exceeds {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    Ok(())
}

fn is_detail_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/cn/jystz/") else {
        return false;
    };
    let Some((date, file)) = rest.split_once('/') else {
        return false;
    };
    date.len() == 8
        && date.bytes().all(|byte| byte.is_ascii_digit())
        && file
            .strip_suffix(".html")
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
}

fn parse_delivery_notice(
    body: &[u8],
    request: &FuturesDeliveryRequest,
    notice_url: &str,
    published_date: &IsoDate,
    observed_at: &str,
    access_mode: CffexAccessMode,
) -> Result<DataBatch<FuturesDeliveryEvent>, ExchangeError> {
    validate_cffex_url(notice_url, access_mode)?;
    let html = std::str::from_utf8(body)
        .map_err(|error| ExchangeError::Decode(format!("CFFEX detail UTF-8: {error}")))?;
    let text = strip_html(html);
    if !text.contains(DELIVERY_TITLE) {
        return Err(ExchangeError::Schema(
            "CFFEX detail title is not the equity-index delivery notice".into(),
        ));
    }
    if !text.contains("交割结算价") {
        return Err(ExchangeError::Schema(
            "CFFEX detail does not prove a delivery settlement price".into(),
        ));
    }
    let suffix = format!(
        "{:02}{:02}",
        request.year().get() % 100,
        request.month().get()
    );
    let products = [
        (FuturesProduct::If, "IF"),
        (FuturesProduct::Ih, "IH"),
        (FuturesProduct::Ic, "IC"),
        (FuturesProduct::Im, "IM"),
    ];
    for (_, prefix) in products {
        let contract = format!("{prefix}{suffix}");
        if !text.contains(&contract) {
            return Err(ExchangeError::Incomplete(format!(
                "CFFEX notice does not contain required contract {contract}"
            )));
        }
    }
    let delivery_date = parse_delivery_date(&text)?;
    if delivery_date.as_str().get(..7)
        != Some(&format!(
            "{:04}-{:02}",
            request.year().get(),
            request.month().get()
        ))
    {
        return Err(ExchangeError::Schema(format!(
            "CFFEX delivery date {} does not match requested contract month",
            delivery_date.as_str()
        )));
    }
    let batch_id = format!(
        "cffex:{}:{}:{}:{observed_at}",
        access_mode.as_str(),
        suffix,
        delivery_date.as_str()
    );
    let canonical_notice_url = match access_mode {
        CffexAccessMode::Https => notice_url.to_owned(),
        CffexAccessMode::PlainHttpDiagnostic => notice_url.replacen("http://", "https://", 1),
    };
    let url = HttpsUrl::new(canonical_notice_url)?;
    let mut records = Vec::with_capacity(products.len());
    for (product, prefix) in products {
        let evidence = SourceEvidence::new(ProviderId::Cffex, observed_at, batch_id.clone())?
            .with_source_at(published_date.as_str())?;
        records.push(FuturesDeliveryEvent {
            product,
            contract_code: NonEmptyText::new(format!("{prefix}{suffix}"))?,
            last_trading_date: None,
            delivery_date: delivery_date.clone(),
            method: FuturesDeliveryMethod::NotProvided,
            notice_url: url.clone(),
            evidence,
        });
    }
    let provenance = Provenance::new(
        match access_mode {
            CffexAccessMode::Https => "cffex-official-notice",
            CffexAccessMode::PlainHttpDiagnostic => {
                "cffex-official-notice-plaintext-http-diagnostic"
            }
        },
        observed_at,
    )?
    .with_source_at(published_date.as_str())?
    .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn parse_delivery_date(text: &str) -> Result<IsoDate, ExchangeError> {
    let marker = "进行交割";
    let marker_at = text
        .find(marker)
        .ok_or_else(|| ExchangeError::Schema("CFFEX delivery sentence is missing".into()))?;
    let before = &text[..marker_at];
    let year_end = before
        .rfind('年')
        .ok_or_else(|| ExchangeError::Schema("delivery year marker is missing".into()))?;
    let year_start = before[..year_end]
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_ascii_digit())
        .map_or(0, |(index, character)| index + character.len_utf8());
    let month_end_relative = before[year_end + '年'.len_utf8()..]
        .find('月')
        .ok_or_else(|| ExchangeError::Schema("delivery month marker is missing".into()))?;
    let month_end = year_end + '年'.len_utf8() + month_end_relative;
    let day_end_relative = before[month_end + '月'.len_utf8()..]
        .find('日')
        .ok_or_else(|| ExchangeError::Schema("delivery day marker is missing".into()))?;
    let day_end = month_end + '月'.len_utf8() + day_end_relative;
    let year = &before[year_start..year_end];
    let month = &before[year_end + '年'.len_utf8()..month_end];
    let day = &before[month_end + '月'.len_utf8()..day_end];
    if year.len() != 4
        || !year.bytes().all(|byte| byte.is_ascii_digit())
        || month.is_empty()
        || !month.bytes().all(|byte| byte.is_ascii_digit())
        || day.is_empty()
        || !day.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ExchangeError::Schema(
            "CFFEX delivery date contains invalid digits".into(),
        ));
    }
    let month = month
        .parse::<u32>()
        .map_err(|error| ExchangeError::Schema(error.to_string()))?;
    let day = day
        .parse::<u32>()
        .map_err(|error| ExchangeError::Schema(error.to_string()))?;
    Ok(IsoDate::new(format!("{year}-{month:02}-{day:02}"))?)
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
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn now() -> Result<String, ExchangeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| ExchangeError::Transport(format!("system clock error: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::HttpResponse;
    use magic_market_core::PositiveU32;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use std::time::Instant;

    struct FixtureTransport {
        responses: Mutex<VecDeque<HttpResponse>>,
    }

    impl ExchangeTransport for FixtureTransport {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
            let mut response = self
                .responses
                .lock()
                .map_err(|_| ExchangeError::Transport("fixture lock poisoned".into()))?
                .pop_front()
                .ok_or_else(|| ExchangeError::Transport("no fixture response".into()))?;
            response.final_url = request.url.clone();
            Ok(response)
        }
    }

    struct RejectTransport;

    impl ExchangeTransport for RejectTransport {
        fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
            panic!("production CFFEX fixed schedule must never touch transport");
        }
    }

    #[derive(Clone)]
    struct SlowTransport {
        starts: Arc<Mutex<Vec<Instant>>>,
        active: Arc<AtomicUsize>,
        maximum_active: Arc<AtomicUsize>,
    }

    impl ExchangeTransport for SlowTransport {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
            self.starts.lock().unwrap().push(Instant::now());
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum_active.fetch_max(active, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(1_250));
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(HttpResponse {
                status: 200,
                final_url: request.url.clone(),
                content_type: Some("text/html".into()),
                body: b"<html></html>".to_vec(),
            })
        }
    }

    fn response(body: &str) -> HttpResponse {
        HttpResponse {
            status: 200,
            final_url: String::new(),
            content_type: Some("text/html; charset=UTF-8".into()),
            body: body.as_bytes().to_vec(),
        }
    }

    fn request() -> FuturesDeliveryRequest {
        FuturesDeliveryRequest::new(
            PositiveU32::new(2026).unwrap(),
            PositiveU32::new(2).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn production_trait_uses_fixed_schedule_without_touching_transport() {
        let client = CffexClient::with_transport(CffexConfig::default(), RejectTransport).unwrap();
        let batch = client.futures_delivery_calendar(&request()).unwrap();
        assert_eq!(batch.records().len(), 4);
        for (record, (product, code)) in batch.records().iter().zip([
            (FuturesProduct::If, "IF2602"),
            (FuturesProduct::Ih, "IH2602"),
            (FuturesProduct::Ic, "IC2602"),
            (FuturesProduct::Im, "IM2602"),
        ]) {
            assert_eq!(record.product, product);
            assert_eq!(record.contract_code.as_str(), code);
            assert_eq!(record.delivery_date.as_str(), "2026-02-24");
            assert_eq!(
                record.last_trading_date.as_ref(),
                Some(&record.delivery_date)
            );
            assert_eq!(record.method, FuturesDeliveryMethod::Cash);
            assert_eq!(record.notice_url.as_str(), FIXED_SCHEDULE_NOTICE_URL);
            assert_eq!(
                record.evidence.batch_id(),
                batch.provenance().batch_id().unwrap()
            );
        }
    }

    #[test]
    fn fixed_schedule_covers_all_2026_months_and_rejects_other_years_before_io() {
        const EXPECTED_DATES: [&str; 12] = [
            "2026-01-16",
            "2026-02-24",
            "2026-03-20",
            "2026-04-17",
            "2026-05-15",
            "2026-06-22",
            "2026-07-17",
            "2026-08-21",
            "2026-09-18",
            "2026-10-16",
            "2026-11-20",
            "2026-12-18",
        ];
        assert_eq!(FIXED_DELIVERY_DATES, EXPECTED_DATES);
        let client = CffexClient::with_transport(CffexConfig::default(), RejectTransport).unwrap();
        for (month, expected) in EXPECTED_DATES.into_iter().enumerate() {
            let request = FuturesDeliveryRequest::new(
                PositiveU32::new(2026).unwrap(),
                PositiveU32::new(u32::try_from(month + 1).unwrap()).unwrap(),
            )
            .unwrap();
            let batch = client.futures_delivery_calendar(&request).unwrap();
            assert_eq!(batch.records().len(), 4);
            assert!(batch.records().iter().all(|record| {
                record.delivery_date.as_str() == expected
                    && record.last_trading_date.as_ref() == Some(&record.delivery_date)
                    && record.method == FuturesDeliveryMethod::Cash
                    && record.notice_url.as_str() == FIXED_SCHEDULE_NOTICE_URL
            }));
        }
        for year in [2025, 2027] {
            let outside = FuturesDeliveryRequest::new(
                PositiveU32::new(year).unwrap(),
                PositiveU32::new(1).unwrap(),
            )
            .unwrap();
            assert!(matches!(
                client.futures_delivery_calendar(&outside),
                Err(ExchangeError::Unsupported(_))
            ));
        }
    }

    #[test]
    fn cloned_cffex_client_spaces_starts_without_serializing_slow_io() {
        let transport = SlowTransport {
            starts: Arc::new(Mutex::new(Vec::new())),
            active: Arc::new(AtomicUsize::new(0)),
            maximum_active: Arc::new(AtomicUsize::new(0)),
        };
        let starts = Arc::clone(&transport.starts);
        let maximum_active = Arc::clone(&transport.maximum_active);
        let client = CffexClient::with_transport(CffexConfig::default(), transport).unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for _ in 0..2 {
            let client = client.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                barrier.wait();
                client.get_html(LIST_ENDPOINT).unwrap();
            }));
        }
        barrier.wait();
        for worker in workers {
            worker.join().unwrap();
        }
        let mut starts = starts.lock().unwrap().clone();
        starts.sort_unstable();
        assert_eq!(starts.len(), 2);
        assert!(starts[1].duration_since(starts[0]) >= Duration::from_millis(950));
        assert_eq!(maximum_active.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn parses_holiday_adjusted_notice_without_inventing_delivery_method() {
        let list = r#"
          <ul><li><a href="/cn/jystz/20260224/46999.html">
          关于股指期货和股指期权合约交割的通知</a><span>2026-02-23</span></li></ul>
        "#;
        let detail = r#"
          <html><h1>关于股指期货和股指期权合约交割的通知</h1>
          <p>IF2602等合约于2026年2月24日进行交割，各合约的交割结算价具体如下：</p>
          <p>沪深300股指期货IF2602合约交割结算价；</p>
          <p>中证500股指期货IC2602合约交割结算价；</p>
          <p>中证1000股指期货IM2602合约交割结算价；</p>
          <p>上证50股指期货IH2602合约交割结算价。</p></html>
        "#;
        let client = CffexClient::with_transport(
            CffexConfig {
                minimum_interval: Duration::from_secs(1),
                ..CffexConfig::default()
            },
            FixtureTransport {
                responses: Mutex::new(VecDeque::from([response(list), response(detail)])),
            },
        )
        .unwrap();
        let formal = client.futures_delivery_calendar(&request()).unwrap();
        assert!(formal
            .records()
            .iter()
            .all(|record| record.method == FuturesDeliveryMethod::Cash));
        let batch = client.probe_futures_delivery_calendar(&request()).unwrap();
        assert_eq!(batch.records().len(), 4);
        assert_eq!(batch.records()[0].contract_code.as_str(), "IF2602");
        assert_eq!(batch.records()[0].delivery_date.as_str(), "2026-02-24");
        assert_eq!(batch.provenance().source_at(), Some("2026-02-23"));
        assert!(batch.records().iter().all(|record| {
            record.last_trading_date.is_none()
                && record.method == FuturesDeliveryMethod::NotProvided
                && record.evidence.source_at() == Some("2026-02-23")
        }));
    }

    #[test]
    fn plaintext_diagnostic_preserves_transport_provenance_and_https_reference() {
        let list = r#"
          <a href="/cn/jystz/20260224/46999.html">
          关于股指期货和股指期权合约交割的通知</a><span>2026-02-23</span>
        "#;
        let detail = r#"
          <h1>关于股指期货和股指期权合约交割的通知</h1>
          <p>IF2602等合约于2026年2月24日进行交割，各合约的交割结算价具体如下：</p>
          <p>IF2602 IC2602 IM2602 IH2602 合约交割结算价。</p>
        "#;
        let mut config = CffexConfig::plaintext_http_diagnostic();
        config.minimum_interval = Duration::from_secs(1);
        let client = CffexClient::with_transport(
            config,
            FixtureTransport {
                responses: Mutex::new(VecDeque::from([response(list), response(detail)])),
            },
        )
        .unwrap();

        let batch = client.probe_futures_delivery_calendar(&request()).unwrap();
        assert_eq!(
            batch.provenance().source(),
            "cffex-official-notice-plaintext-http-diagnostic"
        );
        assert!(batch
            .provenance()
            .batch_id()
            .is_some_and(|value| value.contains("plaintext_http_diagnostic")));
        assert_eq!(
            batch.records()[0].notice_url.as_str(),
            "https://www.cffex.com.cn/cn/jystz/20260224/46999.html"
        );
    }

    #[test]
    fn rejects_formula_only_or_incomplete_notices() {
        let detail = r#"
          <h1>关于股指期货和股指期权合约交割的通知</h1>
          IF2602合约于2026年2月20日进行交割，交割结算价具体如下：
          IC2602 IM2602
        "#;
        assert!(parse_delivery_notice(
            detail.as_bytes(),
            &request(),
            "https://www.cffex.com.cn/cn/jystz/20260220/1.html",
            &IsoDate::new("2026-02-20").unwrap(),
            "observed",
            CffexAccessMode::Https,
        )
        .is_err());
    }

    #[test]
    fn list_parser_requires_unique_official_dated_links() {
        let list = r#"
          <a href="/cn/jystz/20260224/1.html">关于股指期货和股指期权合约交割的通知</a>2026-02-24
          <a href="/cn/jystz/20260224/1.html">重复</a>2026-02-24
        "#;
        assert!(parse_notice_links(list).is_err());
        assert!(official_notice_url("https://example.com/x", CffexAccessMode::Https).is_err());
        for url in [
            "https://www.cffex.com.cn/cn/jystz.html",
            "https://www.cffex.com.cn/cn/jystz_2.html",
            "https://www.cffex.com.cn/cn/jystz/20260224/1.html",
        ] {
            assert!(
                validate_cffex_url(url, CffexAccessMode::Https).is_ok(),
                "{url}"
            );
        }
        for old_or_unbounded in [
            "https://www.cffex.com.cn/jystz/",
            "https://www.cffex.com.cn/jystz/index_2.html",
            "https://www.cffex.com.cn/cn/jystz_121.html",
        ] {
            assert!(validate_cffex_url(old_or_unbounded, CffexAccessMode::Https).is_err());
        }
    }

    #[test]
    fn plaintext_mode_accepts_only_fixed_public_notice_paths() {
        for url in [
            "http://www.cffex.com.cn/cn/jystz.html",
            "http://www.cffex.com.cn/cn/jystz_2.html",
            "http://www.cffex.com.cn/cn/jystz/20260717/48292.html",
        ] {
            assert!(
                validate_cffex_url(url, CffexAccessMode::PlainHttpDiagnostic).is_ok(),
                "{url}"
            );
        }
        for url in [
            "https://www.cffex.com.cn/cn/jystz.html",
            "http://user@www.cffex.com.cn/cn/jystz.html",
            "http://www.cffex.com.cn:8080/cn/jystz.html",
            "http://www.cffex.com.cn/cn/jystz.html?query=1",
            "http://www.cffex.com.cn/cn/jystz.html#fragment",
            "http://www.cffex.com.cn/cn/jystz_121.html",
        ] {
            assert!(
                validate_cffex_url(url, CffexAccessMode::PlainHttpDiagnostic).is_err(),
                "{url}"
            );
        }
    }

    #[test]
    fn notice_date_window_never_slices_inside_utf8() {
        let value = format!("{}2026-07-17", "意".repeat(80));
        let prefix = bounded_utf8_prefix(&value, 240);
        assert!(std::str::from_utf8(prefix.as_bytes()).is_ok());
        assert_eq!(prefix.len(), 240);
    }
}
