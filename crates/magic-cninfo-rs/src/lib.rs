#![forbid(unsafe_code)]
//! Bounded read-only adapter for CNInfo announcements and investor interaction.

mod transport;

pub use transport::{CninfoTransport, HttpMethod, HttpRequest, HttpResponse};

use magic_market_core::{
    Announcement, AnnouncementDiscovery, AnnouncementDiscoveryRequest, Announcements, AssetClass,
    ContentCapabilities, DataBatch, Exchange, HttpsUrl, InstrumentDateRangeRequest, InstrumentId,
    InvestorQuestion, InvestorQuestions, NonEmptyText, Provenance, ProviderId, SourceEvidence,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use transport::HttpsTransport;
use url::{form_urlencoded, Url};

const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; magic-cninfo-rs/0.2; read-only public-data probe)";
const DEFAULT_MAPPING_URL: &str = "https://www.cninfo.com.cn/new/data/szse_stock.json";
const DEFAULT_ANNOUNCEMENT_URL: &str = "https://www.cninfo.com.cn/new/hisAnnouncement/query";
const DEFAULT_IRM_LOOKUP_URL: &str = "https://irm.cninfo.com.cn/newircs/index/queryKeyboardInfo";
const DEFAULT_IRM_QUESTION_URL: &str = "https://irm.cninfo.com.cn/newircs/company/question";
const ALLOWED_HOSTS: [&str; 3] = [
    "www.cninfo.com.cn",
    "irm.cninfo.com.cn",
    "static.cninfo.com.cn",
];
const PAGE_SIZE: u32 = 30;
const MAX_RECORDS: u32 = 300;
pub(crate) const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum CninfoError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("authentication or anti-bot rejection: HTTP {0}")]
    Authentication(u16),
    #[error("rate limited: HTTP 429")]
    RateLimited,
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("unexpected HTTP status {0}")]
    HttpStatus(u16),
    #[error("CNInfo response decoding failed: {0}")]
    Decode(String),
    #[error("CNInfo schema drift: {0}")]
    Schema(String),
    #[error("CNInfo paginated response is incomplete: {0}")]
    Incomplete(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Debug, Clone)]
pub struct CninfoConfig {
    pub mapping_url: String,
    pub announcement_url: String,
    pub irm_lookup_url: String,
    pub irm_question_url: String,
    pub timeout: Duration,
    pub minimum_interval: Duration,
    pub mapping_cache_ttl: Duration,
    pub max_pages: u32,
    pub max_discovery_pages: u32,
}

impl Default for CninfoConfig {
    fn default() -> Self {
        Self {
            mapping_url: DEFAULT_MAPPING_URL.into(),
            announcement_url: DEFAULT_ANNOUNCEMENT_URL.into(),
            irm_lookup_url: DEFAULT_IRM_LOOKUP_URL.into(),
            irm_question_url: DEFAULT_IRM_QUESTION_URL.into(),
            timeout: Duration::from_secs(15),
            minimum_interval: Duration::from_secs(1),
            mapping_cache_ttl: Duration::from_secs(24 * 60 * 60),
            max_pages: 10,
            max_discovery_pages: 334,
        }
    }
}

impl CninfoConfig {
    fn validate(&self) -> Result<(), CninfoError> {
        for endpoint in [
            &self.mapping_url,
            &self.announcement_url,
            &self.irm_lookup_url,
            &self.irm_question_url,
        ] {
            validate_url(endpoint)?;
        }
        if self.timeout.is_zero() || self.timeout > Duration::from_secs(60) {
            return Err(CninfoError::InvalidRequest(
                "timeout must be positive and at most 60 seconds".into(),
            ));
        }
        if self.minimum_interval < Duration::from_secs(1) {
            return Err(CninfoError::InvalidRequest(
                "minimum request interval must be at least one second".into(),
            ));
        }
        if self.mapping_cache_ttl.is_zero() {
            return Err(CninfoError::InvalidRequest(
                "mapping cache TTL must be positive".into(),
            ));
        }
        if self.max_pages == 0 || self.max_pages > 10 {
            return Err(CninfoError::InvalidRequest(
                "max_pages must be between 1 and 10".into(),
            ));
        }
        if self.max_discovery_pages == 0 || self.max_discovery_pages > 334 {
            return Err(CninfoError::InvalidRequest(
                "max_discovery_pages must be between 1 and 334".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationMapping {
    pub instrument: InstrumentId,
    pub org_id: NonEmptyText,
}

#[derive(Debug)]
struct CachedOrganizations {
    fetched_at: Instant,
    by_code: HashMap<String, String>,
}

#[derive(Clone)]
pub struct CninfoClient {
    config: CninfoConfig,
    transport: Arc<dyn CninfoTransport>,
    pacing_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
    organization_cache: Arc<Mutex<Option<CachedOrganizations>>>,
}

impl std::fmt::Debug for CninfoClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CninfoClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl CninfoClient {
    pub fn new() -> Result<Self, CninfoError> {
        Self::with_config(CninfoConfig::default())
    }

    pub fn with_config(config: CninfoConfig) -> Result<Self, CninfoError> {
        config.validate()?;
        let transport = HttpsTransport::new(config.timeout)?;
        Ok(Self::from_parts(
            config.minimum_interval,
            config,
            Arc::new(transport),
        ))
    }

    pub fn with_transport(
        config: CninfoConfig,
        transport: impl CninfoTransport + 'static,
    ) -> Result<Self, CninfoError> {
        config.validate()?;
        Ok(Self::from_parts(
            config.minimum_interval,
            config,
            Arc::new(transport),
        ))
    }

    fn from_parts(
        interval: Duration,
        config: CninfoConfig,
        transport: Arc<dyn CninfoTransport>,
    ) -> Self {
        Self {
            config,
            transport,
            pacing_interval: interval,
            request_gate: Arc::new(Mutex::new(None)),
            organization_cache: Arc::new(Mutex::new(None)),
        }
    }

    #[cfg(test)]
    fn with_test_transport(transport: impl CninfoTransport + 'static) -> Self {
        Self::from_parts(Duration::ZERO, CninfoConfig::default(), Arc::new(transport))
    }

    pub const fn capabilities() -> ContentCapabilities {
        ContentCapabilities {
            instrument_news: false,
            global_news: false,
            announcements: true,
            announcement_discovery: true,
            investor_questions: true,
        }
    }

    pub fn organization_mapping(
        &self,
        instrument: &InstrumentId,
    ) -> Result<OrganizationMapping, CninfoError> {
        validate_instrument(instrument)?;
        let code = instrument.code();
        if let Some(org_id) = self.cached_org_id(code)? {
            return Ok(OrganizationMapping {
                instrument: instrument.clone(),
                org_id: NonEmptyText::new(org_id)?,
            });
        }
        self.refresh_organization_cache()?;
        let org_id = self.cached_org_id(code)?.ok_or_else(|| {
            CninfoError::Unsupported(format!(
                "CNInfo organization mapping has no exact entry for {code}"
            ))
        })?;
        Ok(OrganizationMapping {
            instrument: instrument.clone(),
            org_id: NonEmptyText::new(org_id)?,
        })
    }

    fn cached_org_id(&self, code: &str) -> Result<Option<String>, CninfoError> {
        let cache = self
            .organization_cache
            .lock()
            .map_err(|_| CninfoError::Transport("organization cache mutex poisoned".into()))?;
        Ok(cache
            .as_ref()
            .filter(|cache| cache.fetched_at.elapsed() < self.config.mapping_cache_ttl)
            .and_then(|cache| cache.by_code.get(code))
            .cloned())
    }

    fn refresh_organization_cache(&self) -> Result<(), CninfoError> {
        let response = self.execute(HttpRequest {
            method: HttpMethod::Get,
            url: self.config.mapping_url.clone(),
            headers: json_headers("https://www.cninfo.com.cn/"),
            body: Vec::new(),
        })?;
        ensure_json(&response)?;
        let document: OrganizationDocument = serde_json::from_slice(&response.body)
            .map_err(|error| CninfoError::Decode(error.to_string()))?;
        let rows = document
            .stock_list
            .ok_or_else(|| CninfoError::Schema("stockList is missing".into()))?;
        if rows.is_empty() || rows.len() > 10_000 {
            return Err(CninfoError::Schema(format!(
                "stockList count {} is outside verified bounds",
                rows.len()
            )));
        }
        let mut by_code = HashMap::with_capacity(rows.len());
        for row in rows {
            let code = required_text(row.code, "stockList.code")?;
            if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(CninfoError::Schema(format!(
                    "stockList contains invalid code {code:?}"
                )));
            }
            let org_id = required_text(row.org_id, "stockList.orgId")?;
            if let Some(previous) = by_code.insert(code.clone(), org_id.clone()) {
                if previous != org_id {
                    return Err(CninfoError::Schema(format!(
                        "stockList contains conflicting orgId values for {code}"
                    )));
                }
            }
        }
        *self
            .organization_cache
            .lock()
            .map_err(|_| CninfoError::Transport("organization cache mutex poisoned".into()))? =
            Some(CachedOrganizations {
                fetched_at: Instant::now(),
                by_code,
            });
        Ok(())
    }

    fn execute(&self, request: HttpRequest) -> Result<HttpResponse, CninfoError> {
        validate_request(&request)?;
        let mut last_started = self
            .request_gate
            .lock()
            .map_err(|_| CninfoError::Transport("request limiter mutex poisoned".into()))?;
        if let Some(previous) = *last_started {
            let elapsed = previous.elapsed();
            if elapsed < self.pacing_interval {
                thread::sleep(self.pacing_interval - elapsed);
            }
        }
        *last_started = Some(Instant::now());
        let response = self.transport.execute(&request)?;
        drop(last_started);
        validate_response(&request, &response)?;
        Ok(response)
    }

    fn announcement_page(
        &self,
        code: &str,
        org_id: &str,
        page: u32,
        page_size: u32,
        request: &InstrumentDateRangeRequest,
    ) -> Result<AnnouncementPage, CninfoError> {
        let date_range = request
            .start()
            .zip(request.end())
            .map(|(start, end)| format!("{}~{}", start.as_str(), end.as_str()))
            .unwrap_or_default();
        let body = encode_form(&[
            ("stock", format!("{code},{org_id}")),
            ("tabName", "fulltext".into()),
            ("pageSize", page_size.to_string()),
            ("pageNum", page.to_string()),
            ("column", String::new()),
            ("category", String::new()),
            ("plate", String::new()),
            ("seDate", date_range),
            ("searchkey", String::new()),
            ("secid", String::new()),
            ("sortName", String::new()),
            ("sortType", String::new()),
            ("isHLtitle", "false".into()),
        ]);
        let response = self.execute(HttpRequest {
            method: HttpMethod::Post,
            url: self.config.announcement_url.clone(),
            headers: form_headers(
                "https://www.cninfo.com.cn",
                "https://www.cninfo.com.cn/new/disclosure",
            ),
            body,
        })?;
        ensure_json(&response)?;
        serde_json::from_slice(&response.body)
            .map_err(|error| CninfoError::Decode(error.to_string()))
    }

    fn announcement_discovery_page(
        &self,
        page: u32,
        request: &AnnouncementDiscoveryRequest,
    ) -> Result<AnnouncementPage, CninfoError> {
        let body = encode_form(&[
            ("stock", String::new()),
            ("tabName", "fulltext".into()),
            ("pageSize", PAGE_SIZE.to_string()),
            ("pageNum", page.to_string()),
            ("column", "szse".into()),
            ("category", String::new()),
            ("plate", String::new()),
            (
                "seDate",
                format!("{}~{}", request.start().as_str(), request.end().as_str()),
            ),
            ("searchkey", String::new()),
            ("secid", String::new()),
            ("sortName", String::new()),
            ("sortType", String::new()),
            ("isHLtitle", "false".into()),
        ]);
        let response = self.execute(HttpRequest {
            method: HttpMethod::Post,
            url: self.config.announcement_url.clone(),
            headers: form_headers(
                "https://www.cninfo.com.cn",
                "https://www.cninfo.com.cn/new/disclosure",
            ),
            body,
        })?;
        ensure_json(&response)?;
        serde_json::from_slice(&response.body)
            .map_err(|error| CninfoError::Decode(error.to_string()))
    }

    fn irm_org_id(&self, code: &str) -> Result<String, CninfoError> {
        let response = self.execute(HttpRequest {
            method: HttpMethod::Post,
            url: self.config.irm_lookup_url.clone(),
            headers: form_headers("https://irm.cninfo.com.cn", "https://irm.cninfo.com.cn/"),
            body: encode_form(&[("keyWord", code.to_owned())]),
        })?;
        ensure_json(&response)?;
        let document: IrmLookupDocument = serde_json::from_slice(&response.body)
            .map_err(|error| CninfoError::Decode(error.to_string()))?;
        let matches: Vec<_> = document
            .data
            .unwrap_or_default()
            .into_iter()
            .filter(|row| row.stock_code.as_deref() == Some(code))
            .collect();
        if matches.len() != 1 {
            return Err(CninfoError::Unsupported(format!(
                "IRM lookup returned {} exact mappings for {code}",
                matches.len()
            )));
        }
        required_text(
            matches.into_iter().next().and_then(|row| row.secid),
            "IRM secid",
        )
    }

    fn question_page(
        &self,
        code: &str,
        org_id: &str,
        page: u32,
        page_size: u32,
        request: &InstrumentDateRangeRequest,
    ) -> Result<IrmQuestionPage, CninfoError> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        serializer
            .append_pair("_t", "1")
            .append_pair("stockcode", code)
            .append_pair("orgId", org_id)
            .append_pair("pageSize", &page_size.to_string())
            .append_pair("pageNum", &page.to_string())
            .append_pair("keyWord", "")
            .append_pair(
                "startDay",
                request
                    .start()
                    .map(|date| date.as_str())
                    .unwrap_or_default(),
            )
            .append_pair(
                "endDay",
                request.end().map(|date| date.as_str()).unwrap_or_default(),
            );
        let response = self.execute(HttpRequest {
            method: HttpMethod::Post,
            url: format!("{}?{}", self.config.irm_question_url, serializer.finish()),
            headers: form_headers("https://irm.cninfo.com.cn", "https://irm.cninfo.com.cn/"),
            body: Vec::new(),
        })?;
        ensure_json(&response)?;
        serde_json::from_slice(&response.body)
            .map_err(|error| CninfoError::Decode(error.to_string()))
    }
}

impl Announcements for CninfoClient {
    type Error = CninfoError;

    fn announcements(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error> {
        validate_bounded_request(request)?;
        let mapping = self.organization_mapping(request.instrument())?;
        let limit = request.limit().get() as usize;
        let mut rows_to_map = Vec::with_capacity(limit);
        let mut seen = HashSet::new();
        let mut page = 1;
        loop {
            if rows_to_map.len() == limit {
                break;
            }
            if page > self.config.max_pages {
                return Err(CninfoError::Incomplete(format!(
                    "announcement limit {limit} requires more than {} pages",
                    self.config.max_pages
                )));
            }
            let page_size = PAGE_SIZE;
            let response = self.announcement_page(
                request.instrument().code(),
                mapping.org_id.as_str(),
                page,
                page_size,
                request,
            )?;
            let has_more = response
                .has_more
                .ok_or_else(|| CninfoError::Schema("announcement hasMore is missing".into()))?;
            let rows = response.announcements.unwrap_or_default();
            if has_more && rows.is_empty() {
                return Err(CninfoError::Schema(
                    "announcement page is empty while hasMore is true".into(),
                ));
            }
            for row in rows {
                if rows_to_map.len() == limit {
                    break;
                }
                let announcement_id =
                    required_text(row.announcement_id.clone(), "announcement.announcementId")?;
                if !seen.insert(announcement_id.clone()) {
                    return Err(CninfoError::Schema(format!(
                        "duplicate announcement {announcement_id} across pages"
                    )));
                }
                rows_to_map.push(row);
            }
            if !has_more {
                break;
            }
            page += 1;
        }
        if rows_to_map.is_empty() {
            return Err(CninfoError::Incomplete(format!(
                "CNInfo returned no announcements for {}",
                request.instrument().code()
            )));
        }
        let observed_at = now()?;
        let batch_id = format!(
            "cninfo:{observed_at}:announcements:{}",
            request.instrument().code()
        );
        let mut records = Vec::with_capacity(rows_to_map.len());
        let mut source_times = Vec::with_capacity(rows_to_map.len());
        for row in rows_to_map {
            let record = map_announcement(
                row,
                request,
                mapping.org_id.as_str(),
                &observed_at,
                &batch_id,
            )?;
            source_times.push(record.published_at.as_str().to_owned());
            records.push(record);
        }
        let provenance = provenance(
            "cninfo",
            &observed_at,
            &batch_id,
            source_times.iter().max().map(String::as_str),
        )?;
        Ok(DataBatch::strict(records, provenance))
    }
}

impl AnnouncementDiscovery for CninfoClient {
    type Error = CninfoError;

    fn discover_announcements(
        &self,
        request: &AnnouncementDiscoveryRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error> {
        let limit = request.limit().get() as usize;
        let mut rows_to_map = Vec::with_capacity(limit);
        let mut seen = HashSet::new();
        let mut expected_total = None;
        let mut expected_pages = None;
        let mut page = 1_u32;

        loop {
            if rows_to_map.len() == limit {
                break;
            }
            if page > self.config.max_discovery_pages {
                return Err(CninfoError::Incomplete(format!(
                    "announcement discovery requires more than {} pages",
                    self.config.max_discovery_pages
                )));
            }

            let response = self.announcement_discovery_page(page, request)?;
            let total = required_page_count(
                response
                    .total_announcement
                    .as_ref()
                    .or(response.total_record_num.as_ref()),
                "announcement total",
            )?;
            let pages = required_page_count(response.total_pages.as_ref(), "announcement pages")?;
            if expected_total
                .replace(total)
                .is_some_and(|value| value != total)
                || expected_pages
                    .replace(pages)
                    .is_some_and(|value| value != pages)
            {
                return Err(CninfoError::Schema(
                    "announcement totals changed across pages".into(),
                ));
            }

            let has_more = response
                .has_more
                .ok_or_else(|| CninfoError::Schema("announcement hasMore is missing".into()))?;
            let rows = response.announcements.unwrap_or_default();
            if (total > 0 || has_more) && rows.is_empty() {
                return Err(CninfoError::Schema(
                    "announcement discovery page is empty before completion".into(),
                ));
            }
            for row in rows {
                if rows_to_map.len() == limit {
                    break;
                }
                let announcement_id =
                    required_text(row.announcement_id.clone(), "announcement.announcementId")?;
                if !seen.insert(announcement_id.clone()) {
                    return Err(CninfoError::Schema(format!(
                        "duplicate announcement {announcement_id} across discovery pages"
                    )));
                }
                rows_to_map.push(row);
            }

            if !has_more {
                if rows_to_map.len() < total as usize && rows_to_map.len() < limit {
                    return Err(CninfoError::Incomplete(format!(
                        "announcement discovery ended after {} of {total} source rows",
                        rows_to_map.len()
                    )));
                }
                break;
            }
            if u64::from(page) >= pages {
                return Err(CninfoError::Schema(
                    "announcement hasMore remained true on the declared final page".into(),
                ));
            }
            page += 1;
        }

        if rows_to_map.is_empty() {
            return Err(CninfoError::Incomplete(
                "CNInfo returned no full-market announcements".into(),
            ));
        }
        let observed_at = now()?;
        let batch_id = format!(
            "cninfo:{observed_at}:announcement-discovery:{}:{}",
            request.start().as_str(),
            request.end().as_str()
        );
        let mut records = Vec::with_capacity(rows_to_map.len());
        let mut source_times = Vec::with_capacity(rows_to_map.len());
        for row in rows_to_map {
            let record = map_discovered_announcement(row, request, &observed_at, &batch_id)?;
            source_times.push(record.published_at.as_str().to_owned());
            records.push(record);
        }
        let provenance = provenance(
            "cninfo-full-market",
            &observed_at,
            &batch_id,
            source_times.iter().max().map(String::as_str),
        )?;
        Ok(DataBatch::strict(records, provenance))
    }
}

impl InvestorQuestions for CninfoClient {
    type Error = CninfoError;

    fn investor_questions(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<InvestorQuestion>, Self::Error> {
        validate_bounded_request(request)?;
        validate_instrument(request.instrument())?;
        let code = request.instrument().code();
        let org_id = self.irm_org_id(code)?;
        let limit = request.limit().get() as usize;
        let mut rows_to_map = Vec::with_capacity(limit);
        let mut seen = HashSet::new();
        let mut page = 1;
        loop {
            if rows_to_map.len() == limit {
                break;
            }
            if page > self.config.max_pages {
                return Err(CninfoError::Incomplete(format!(
                    "question limit {limit} requires more than {} pages",
                    self.config.max_pages
                )));
            }
            let page_size = PAGE_SIZE;
            let response = self.question_page(code, &org_id, page, page_size, request)?;
            let total = parse_optional_u64(response.total.as_ref(), "IRM total")?
                .ok_or_else(|| CninfoError::Schema("IRM total is missing".into()))?;
            let rows = response.rows.unwrap_or_default();
            if total > 0 && rows.is_empty() {
                return Err(CninfoError::Schema(
                    "IRM page is empty while total is non-zero".into(),
                ));
            }
            let returned = rows.len();
            for row in rows {
                if rows_to_map.len() == limit {
                    break;
                }
                let question_id = required_text(row.index_id.clone(), "IRM indexId")?;
                if !seen.insert(question_id.clone()) {
                    return Err(CninfoError::Schema(format!(
                        "duplicate IRM question {question_id} across pages"
                    )));
                }
                rows_to_map.push(row);
            }
            let consumed = (page as usize - 1) * page_size as usize + returned;
            if consumed >= total as usize || returned < page_size as usize {
                break;
            }
            page += 1;
        }
        if rows_to_map.is_empty() {
            return Err(CninfoError::Incomplete(format!(
                "CNInfo IRM returned no investor questions for {code}"
            )));
        }
        let observed_at = now()?;
        let batch_id = format!("cninfo:{observed_at}:investor-questions:{code}");
        let mut records = Vec::with_capacity(rows_to_map.len());
        let mut source_times = Vec::with_capacity(rows_to_map.len());
        for row in rows_to_map {
            let record = map_question(row, request, &observed_at, &batch_id)?;
            source_times.push(record.question_at().as_str().to_owned());
            records.push(record);
        }
        let provenance = provenance(
            "cninfo-irm",
            &observed_at,
            &batch_id,
            source_times.iter().max().map(String::as_str),
        )?;
        Ok(DataBatch::strict(records, provenance))
    }
}

#[derive(Debug, Deserialize)]
struct OrganizationDocument {
    #[serde(rename = "stockList")]
    stock_list: Option<Vec<OrganizationWire>>,
}

#[derive(Debug, Deserialize)]
struct OrganizationWire {
    code: Option<String>,
    #[serde(rename = "orgId")]
    org_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnnouncementPage {
    #[serde(rename = "hasMore")]
    has_more: Option<bool>,
    #[serde(rename = "totalAnnouncement")]
    total_announcement: Option<Value>,
    #[serde(rename = "totalRecordNum")]
    total_record_num: Option<Value>,
    #[serde(rename = "totalpages")]
    total_pages: Option<Value>,
    announcements: Option<Vec<AnnouncementWire>>,
}

#[derive(Debug, Deserialize)]
struct AnnouncementWire {
    #[serde(rename = "announcementId")]
    announcement_id: Option<String>,
    #[serde(rename = "secCode")]
    sec_code: Option<String>,
    #[serde(rename = "secName")]
    sec_name: Option<String>,
    #[serde(rename = "announcementTitle")]
    title: Option<String>,
    #[serde(rename = "announcementTypeName")]
    category_name: Option<String>,
    #[serde(rename = "announcementType")]
    category: Option<String>,
    #[serde(rename = "announcementTime")]
    published_at: Option<Value>,
    #[serde(rename = "adjunctUrl")]
    adjunct_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IrmLookupDocument {
    data: Option<Vec<IrmLookupWire>>,
}

#[derive(Debug, Deserialize)]
struct IrmLookupWire {
    #[serde(rename = "stockCode")]
    stock_code: Option<String>,
    secid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IrmQuestionPage {
    total: Option<Value>,
    rows: Option<Vec<IrmQuestionWire>>,
}

#[derive(Debug, Deserialize)]
struct IrmQuestionWire {
    #[serde(rename = "indexId")]
    index_id: Option<String>,
    #[serde(rename = "stockCode")]
    stock_code: Option<String>,
    #[serde(rename = "companyShortName")]
    company: Option<String>,
    #[serde(rename = "mainContent")]
    question: Option<String>,
    #[serde(rename = "attachedContent")]
    answer: Option<String>,
    #[serde(rename = "attachedAuthor")]
    answerer: Option<String>,
    #[serde(rename = "pubDate")]
    question_at: Option<Value>,
    #[serde(rename = "attachedPubDate")]
    answer_at: Option<Value>,
}

fn map_announcement(
    row: AnnouncementWire,
    request: &InstrumentDateRangeRequest,
    org_id: &str,
    observed_at: &str,
    batch_id: &str,
) -> Result<Announcement, CninfoError> {
    let code = required_text(row.sec_code, "announcement.secCode")?;
    if code != request.instrument().code() {
        return Err(CninfoError::Schema(format!(
            "announcement identity {code} does not match requested {}",
            request.instrument().code()
        )));
    }
    let announcement_id = required_text(row.announcement_id, "announcement.announcementId")?;
    let published_at = parse_required_millis(row.published_at.as_ref(), "announcementTime")?;
    ensure_in_range(&published_at, request)?;
    let mut evidence = SourceEvidence::new(ProviderId::Cninfo, observed_at, batch_id.to_owned())?;
    evidence = evidence.with_source_at(published_at.clone())?;
    let canonical_url = announcement_url(
        request.instrument().code(),
        org_id,
        &announcement_id,
        &published_at,
    )?;
    Ok(Announcement {
        announcement_id: NonEmptyText::new(announcement_id.clone())?,
        instrument: request.instrument().clone(),
        instrument_name: optional_nonempty(row.sec_name)?,
        category: optional_nonempty(row.category_name.or(row.category))?,
        title: NonEmptyText::new(normalize_required(row.title, "announcementTitle")?)?,
        published_at: NonEmptyText::new(published_at)?,
        canonical_url,
        pdf_url: row
            .adjunct_url
            .and_then(nonblank)
            .map(pdf_url)
            .transpose()?,
        evidence,
    })
}

fn map_discovered_announcement(
    row: AnnouncementWire,
    request: &AnnouncementDiscoveryRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<Announcement, CninfoError> {
    let code = required_text(row.sec_code, "announcement.secCode")?;
    let instrument = discovered_equity(&code)?;
    if request
        .exchange()
        .is_some_and(|exchange| exchange != instrument.exchange())
    {
        return Err(CninfoError::Schema(format!(
            "announcement identity {code} does not match requested exchange"
        )));
    }
    let announcement_id = required_text(row.announcement_id, "announcement.announcementId")?;
    let published_at = parse_required_millis(row.published_at.as_ref(), "announcementTime")?;
    ensure_discovery_range(&published_at, request)?;
    let instrument_name = optional_nonempty(row.sec_name)?
        .ok_or_else(|| CninfoError::Schema("announcement secName is missing".into()))?;
    let pdf = row
        .adjunct_url
        .and_then(nonblank)
        .ok_or_else(|| CninfoError::Schema("announcement adjunctUrl is missing".into()))
        .and_then(pdf_url)?;
    let mut evidence = SourceEvidence::new(ProviderId::Cninfo, observed_at, batch_id.to_owned())?;
    evidence = evidence.with_source_at(published_at.clone())?;
    Ok(Announcement {
        announcement_id: NonEmptyText::new(announcement_id)?,
        instrument,
        instrument_name: Some(instrument_name),
        category: optional_nonempty(row.category_name.or(row.category))?,
        title: NonEmptyText::new(normalize_required(row.title, "announcementTitle")?)?,
        published_at: NonEmptyText::new(published_at)?,
        canonical_url: pdf.clone(),
        pdf_url: Some(pdf),
        evidence,
    })
}

fn discovered_equity(code: &str) -> Result<InstrumentId, CninfoError> {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CninfoError::Schema(format!(
            "announcement secCode {code:?} is not a six-digit equity code"
        )));
    }
    let exchange = match code.as_bytes()[0] {
        b'6' => Exchange::Shanghai,
        b'0' | b'3' => Exchange::Shenzhen,
        b'4' | b'8' => Exchange::Beijing,
        b'9' if code.starts_with("920") => Exchange::Beijing,
        prefix => {
            return Err(CninfoError::Unsupported(format!(
                "announcement code {code} has unverified prefix {:?}",
                char::from(prefix)
            )));
        }
    };
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
}

fn map_question(
    row: IrmQuestionWire,
    request: &InstrumentDateRangeRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<InvestorQuestion, CninfoError> {
    let code = required_text(row.stock_code, "IRM stockCode")?;
    if code != request.instrument().code() {
        return Err(CninfoError::Schema(format!(
            "IRM identity {code} does not match requested {}",
            request.instrument().code()
        )));
    }
    let source_id = required_text(row.index_id, "IRM indexId")?;
    let question_at = parse_required_millis(row.question_at.as_ref(), "IRM pubDate")?;
    ensure_in_range(&question_at, request)?;
    let answer = optional_nonempty(row.answer)?;
    let answer_at = if answer.is_some() {
        parse_optional_millis(row.answer_at.as_ref(), "IRM attachedPubDate")?
            .map(NonEmptyText::new)
            .transpose()?
    } else {
        None
    };
    let answerer = if answer.is_some() {
        optional_nonempty(row.answerer)?
    } else {
        None
    };
    let mut evidence = SourceEvidence::new(ProviderId::Cninfo, observed_at, batch_id.to_owned())?;
    evidence = evidence.with_source_at(question_at.clone())?;
    Ok(InvestorQuestion::new_with_metadata(
        NonEmptyText::new(source_id.clone())?,
        request.instrument().clone(),
        NonEmptyText::new(normalize_required(row.company, "IRM companyShortName")?)?,
        NonEmptyText::new(normalize_required(row.question, "IRM mainContent")?)?,
        NonEmptyText::new(question_at)?,
        answer,
        answer_at,
        Some(NonEmptyText::new(source_id)?),
        answerer,
        evidence,
    )?)
}

fn validate_bounded_request(request: &InstrumentDateRangeRequest) -> Result<(), CninfoError> {
    validate_instrument(request.instrument())?;
    if request.limit().get() > MAX_RECORDS {
        return Err(CninfoError::InvalidRequest(format!(
            "CNInfo request limit must be at most {MAX_RECORDS}"
        )));
    }
    Ok(())
}

fn validate_instrument(instrument: &InstrumentId) -> Result<(), CninfoError> {
    if instrument.asset_class() != AssetClass::Equity {
        return Err(CninfoError::Unsupported(format!(
            "CNInfo capability supports equities, not {:?}",
            instrument.asset_class()
        )));
    }
    let code = instrument.code();
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CninfoError::InvalidRequest(
            "CNInfo stock code must contain exactly six digits".into(),
        ));
    }
    let expected_exchange = match code.as_bytes()[0] {
        b'6' => Exchange::Shanghai,
        b'0' | b'3' => Exchange::Shenzhen,
        b'4' | b'8' => Exchange::Beijing,
        b'9' if code.starts_with("920") => Exchange::Beijing,
        prefix => {
            return Err(CninfoError::Unsupported(format!(
                "CNInfo stock-code prefix {:?} has no verified exchange mapping",
                char::from(prefix)
            )));
        }
    };
    if instrument.exchange() != expected_exchange {
        return Err(CninfoError::InvalidRequest(format!(
            "CNInfo code {code} implies {expected_exchange:?} exchange, not {:?}",
            instrument.exchange()
        )));
    }
    Ok(())
}

fn validate_request(request: &HttpRequest) -> Result<(), CninfoError> {
    validate_url(&request.url)?;
    if request.body.len() > 64 * 1024 {
        return Err(CninfoError::InvalidRequest(
            "request body exceeds 64 KiB".into(),
        ));
    }
    Ok(())
}

fn validate_url(value: &str) -> Result<(), CninfoError> {
    let parsed =
        Url::parse(value).map_err(|error| CninfoError::InvalidRequest(error.to_string()))?;
    if parsed.scheme() != "https"
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.port_or_known_default() != Some(443)
    {
        return Err(CninfoError::InvalidRequest(
            "CNInfo endpoints must use credential-free HTTPS on port 443".into(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| CninfoError::InvalidRequest("endpoint host is missing".into()))?;
    if !ALLOWED_HOSTS.contains(&host) {
        return Err(CninfoError::InvalidRequest(format!(
            "CNInfo host {host} is not allowlisted"
        )));
    }
    Ok(())
}

fn validate_response(request: &HttpRequest, response: &HttpResponse) -> Result<(), CninfoError> {
    if response.body.len() > MAX_RESPONSE_BYTES {
        return Err(CninfoError::Incomplete(format!(
            "response body exceeds {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    validate_url(&response.final_url)?;
    let expected =
        Url::parse(&request.url).map_err(|error| CninfoError::InvalidRequest(error.to_string()))?;
    let actual =
        Url::parse(&response.final_url).map_err(|error| CninfoError::Schema(error.to_string()))?;
    if expected != actual {
        return Err(CninfoError::Schema(
            "redirected or final response URL does not match the request".into(),
        ));
    }
    match response.status {
        200..=299 => Ok(()),
        401 | 403 => Err(CninfoError::Authentication(response.status)),
        429 => Err(CninfoError::RateLimited),
        status => Err(CninfoError::HttpStatus(status)),
    }
}

fn ensure_json(response: &HttpResponse) -> Result<(), CninfoError> {
    if response
        .content_type
        .as_deref()
        .is_some_and(|value| !value.to_ascii_lowercase().contains("json"))
    {
        return Err(CninfoError::Schema(format!(
            "expected JSON but received {:?}",
            response.content_type
        )));
    }
    let first = response
        .body
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace());
    if !matches!(first, Some(b'{') | Some(b'[')) {
        return Err(CninfoError::Schema(
            "successful response is not a JSON document".into(),
        ));
    }
    Ok(())
}

fn json_headers(referer: &str) -> Vec<(String, String)> {
    vec![
        ("User-Agent".into(), USER_AGENT.into()),
        ("Accept".into(), "application/json".into()),
        ("Referer".into(), referer.into()),
    ]
}

fn form_headers(origin: &str, referer: &str) -> Vec<(String, String)> {
    vec![
        ("User-Agent".into(), USER_AGENT.into()),
        ("Accept".into(), "application/json".into()),
        (
            "Content-Type".into(),
            "application/x-www-form-urlencoded".into(),
        ),
        ("Origin".into(), origin.into()),
        ("Referer".into(), referer.into()),
    ]
}

fn encode_form(values: &[(&str, String)]) -> Vec<u8> {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (key, value) in values {
        serializer.append_pair(key, value);
    }
    serializer.finish().into_bytes()
}

fn announcement_url(
    code: &str,
    org_id: &str,
    announcement_id: &str,
    published_at: &str,
) -> Result<HttpsUrl, CninfoError> {
    let date = published_at
        .get(..10)
        .ok_or_else(|| CninfoError::Schema("announcement timestamp has no date prefix".into()))?;
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer
        .append_pair("stockCode", code)
        .append_pair("announcementId", announcement_id)
        .append_pair("orgId", org_id)
        .append_pair("announcementTime", date);
    Ok(HttpsUrl::new(format!(
        "https://www.cninfo.com.cn/new/disclosure/detail?{}",
        serializer.finish()
    ))?)
}

fn pdf_url(relative: String) -> Result<HttpsUrl, CninfoError> {
    let relative = relative.trim().trim_start_matches('/');
    if relative.is_empty()
        || relative.contains("..")
        || relative.contains('\\')
        || relative.contains(':')
    {
        return Err(CninfoError::Schema(format!(
            "announcement adjunctUrl is not a safe relative path: {relative:?}"
        )));
    }
    Ok(HttpsUrl::new(format!(
        "https://static.cninfo.com.cn/{relative}"
    ))?)
}

fn required_text(value: Option<String>, field: &str) -> Result<String, CninfoError> {
    value
        .and_then(nonblank)
        .ok_or_else(|| CninfoError::Schema(format!("{field} is missing")))
}

fn normalize_required(value: Option<String>, field: &str) -> Result<String, CninfoError> {
    value
        .map(normalize_text)
        .and_then(nonblank)
        .ok_or_else(|| CninfoError::Schema(format!("{field} is missing")))
}

fn optional_nonempty(value: Option<String>) -> Result<Option<NonEmptyText>, CninfoError> {
    value
        .map(normalize_text)
        .and_then(nonblank)
        .map(NonEmptyText::new)
        .transpose()
        .map_err(Into::into)
}

fn normalize_text(value: String) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn nonblank(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_required_millis(value: Option<&Value>, field: &str) -> Result<String, CninfoError> {
    parse_optional_millis(value, field)?
        .ok_or_else(|| CninfoError::Schema(format!("{field} is missing")))
}

fn parse_optional_millis(
    value: Option<&Value>,
    field: &str,
) -> Result<Option<String>, CninfoError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let millis = match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
    .ok_or_else(|| CninfoError::Schema(format!("{field} is not an integer millisecond value")))?;
    unix_seconds_to_china_iso(millis.div_euclid(1000))
        .map(Some)
        .ok_or_else(|| CninfoError::Schema(format!("{field} is outside supported time bounds")))
}

fn parse_optional_u64(value: Option<&Value>, field: &str) -> Result<Option<u64>, CninfoError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
    .map(Some)
    .ok_or_else(|| CninfoError::Schema(format!("{field} is not a non-negative integer")))
}

fn required_page_count(value: Option<&Value>, field: &str) -> Result<u64, CninfoError> {
    parse_optional_u64(value, field)?
        .ok_or_else(|| CninfoError::Schema(format!("{field} is missing")))
}

fn ensure_in_range(
    timestamp: &str,
    request: &InstrumentDateRangeRequest,
) -> Result<(), CninfoError> {
    let date = timestamp
        .get(..10)
        .ok_or_else(|| CninfoError::Schema("source timestamp has no date prefix".into()))?;
    if request.start().is_some_and(|start| date < start.as_str())
        || request.end().is_some_and(|end| date > end.as_str())
    {
        return Err(CninfoError::Schema(format!(
            "source record date {date} is outside the requested range"
        )));
    }
    Ok(())
}

fn ensure_discovery_range(
    timestamp: &str,
    request: &AnnouncementDiscoveryRequest,
) -> Result<(), CninfoError> {
    let date = timestamp
        .get(..10)
        .ok_or_else(|| CninfoError::Schema("source timestamp has no date prefix".into()))?;
    if date < request.start().as_str() || date > request.end().as_str() {
        return Err(CninfoError::Schema(format!(
            "source record date {date} is outside the requested discovery range"
        )));
    }
    Ok(())
}

fn provenance(
    source: &str,
    observed_at: &str,
    batch_id: &str,
    source_at: Option<&str>,
) -> Result<Provenance, CninfoError> {
    let mut provenance =
        Provenance::new(source, observed_at.to_owned())?.with_batch_id(batch_id.to_owned())?;
    if let Some(source_at) = source_at {
        provenance = provenance.with_source_at(source_at.to_owned())?;
    }
    Ok(provenance)
}

fn now() -> Result<String, CninfoError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| CninfoError::Transport(format!("system clock error: {error}")))
}

fn unix_seconds_to_china_iso(seconds: i64) -> Option<String> {
    let local = seconds.checked_add(8 * 60 * 60)?;
    let days = local.div_euclid(86_400);
    let day_seconds = local.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days)?;
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    let second = day_seconds % 60;
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}+08:00"
    ))
}

fn civil_from_days(days_since_epoch: i64) -> Option<(i64, i64, i64)> {
    let z = days_since_epoch.checked_add(719_468)?;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2).div_euclid(153);
    let day = doy - (153 * mp + 2).div_euclid(5) + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (1..=9999).contains(&year).then_some((year, month, day))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Clone)]
    struct FixtureTransport {
        responses: Arc<Mutex<VecDeque<HttpResponse>>>,
        requests: Arc<Mutex<Vec<HttpRequest>>>,
    }

    #[derive(Debug, Clone, Copy)]
    enum PagedFixtureKind {
        Announcements,
        Questions,
    }

    #[derive(Clone)]
    struct PagedFixtureTransport {
        kind: PagedFixtureKind,
        requests: Arc<Mutex<Vec<HttpRequest>>>,
    }

    impl PagedFixtureTransport {
        fn new(kind: PagedFixtureKind) -> Self {
            Self {
                kind,
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn page_response(
            request: &HttpRequest,
            page: usize,
            page_size: usize,
            kind: PagedFixtureKind,
        ) -> Result<HttpResponse, CninfoError> {
            let offset = page
                .checked_sub(1)
                .and_then(|page| page.checked_mul(page_size))
                .ok_or_else(|| CninfoError::Transport("fixture page offset overflow".into()))?;
            let end = offset.saturating_add(page_size).min(50);
            let rows = (offset..end)
                .map(|index| match kind {
                    PagedFixtureKind::Announcements => serde_json::json!({
                        "announcementId": format!("announcement-{index:02}"),
                        "secCode": "600396",
                        "announcementTitle": format!("announcement {index}"),
                        "announcementTime": 1_784_822_400_000_i64,
                        "adjunctUrl": format!("finalpage/2026-07-24/{index}.PDF")
                    }),
                    PagedFixtureKind::Questions => serde_json::json!({
                        "indexId": format!("question-{index:02}"),
                        "stockCode": "002594",
                        "companyShortName": "比亚迪",
                        "mainContent": format!("question {index}"),
                        "attachedContent": null,
                        "attachedAuthor": null,
                        "pubDate": 1_784_822_400_000_i64,
                        "attachedPubDate": null
                    }),
                })
                .collect::<Vec<_>>();
            let body = match kind {
                PagedFixtureKind::Announcements => {
                    serde_json::json!({"hasMore": end < 50, "announcements": rows})
                }
                PagedFixtureKind::Questions => {
                    serde_json::json!({"total": 50, "rows": rows})
                }
            };
            Ok(HttpResponse {
                status: 200,
                final_url: request.url.clone(),
                content_type: Some("application/json".into()),
                body: serde_json::to_vec(&body)
                    .map_err(|error| CninfoError::Transport(error.to_string()))?,
            })
        }
    }

    impl CninfoTransport for PagedFixtureTransport {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError> {
            self.requests
                .lock()
                .map_err(|_| CninfoError::Transport("fixture lock poisoned".into()))?
                .push(request.clone());
            match self.kind {
                PagedFixtureKind::Announcements if request.url == DEFAULT_MAPPING_URL => {
                    Ok(response(
                        DEFAULT_MAPPING_URL,
                        include_str!("../tests/fixtures/organizations.json"),
                    ))
                }
                PagedFixtureKind::Questions if request.url == DEFAULT_IRM_LOOKUP_URL => {
                    Ok(response(
                        DEFAULT_IRM_LOOKUP_URL,
                        include_str!("../tests/fixtures/irm_lookup.json"),
                    ))
                }
                PagedFixtureKind::Announcements => {
                    let values = form_urlencoded::parse(&request.body)
                        .into_owned()
                        .collect::<HashMap<_, _>>();
                    let page = values
                        .get("pageNum")
                        .and_then(|value| value.parse::<usize>().ok())
                        .ok_or_else(|| CninfoError::Transport("fixture pageNum missing".into()))?;
                    let page_size = values
                        .get("pageSize")
                        .and_then(|value| value.parse::<usize>().ok())
                        .ok_or_else(|| CninfoError::Transport("fixture pageSize missing".into()))?;
                    Self::page_response(request, page, page_size, self.kind)
                }
                PagedFixtureKind::Questions => {
                    let url = Url::parse(&request.url)
                        .map_err(|error| CninfoError::Transport(error.to_string()))?;
                    let values = url.query_pairs().into_owned().collect::<HashMap<_, _>>();
                    let page = values
                        .get("pageNum")
                        .and_then(|value| value.parse::<usize>().ok())
                        .ok_or_else(|| CninfoError::Transport("fixture pageNum missing".into()))?;
                    let page_size = values
                        .get("pageSize")
                        .and_then(|value| value.parse::<usize>().ok())
                        .ok_or_else(|| CninfoError::Transport("fixture pageSize missing".into()))?;
                    Self::page_response(request, page, page_size, self.kind)
                }
            }
        }
    }

    impl FixtureTransport {
        fn new(responses: Vec<HttpResponse>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses.into())),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl CninfoTransport for FixtureTransport {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError> {
            self.requests.lock().unwrap().push(request.clone());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| CninfoError::Transport("fixture response exhausted".into()))
        }
    }

    #[derive(Clone)]
    struct CompletionTransport {
        inner: FixtureTransport,
        completed_at: Arc<Mutex<Option<u128>>>,
    }

    impl CompletionTransport {
        fn new(responses: Vec<HttpResponse>) -> Self {
            Self {
                inner: FixtureTransport::new(responses),
                completed_at: Arc::new(Mutex::new(None)),
            }
        }
    }

    impl CninfoTransport for CompletionTransport {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError> {
            let response = self.inner.execute(request)?;
            let completed_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| CninfoError::Transport(error.to_string()))?
                .as_nanos();
            *self
                .completed_at
                .lock()
                .map_err(|_| CninfoError::Transport("completion lock poisoned".into()))? =
                Some(completed_at);
            Ok(response)
        }
    }

    fn response(url: &str, body: &str) -> HttpResponse {
        HttpResponse {
            status: 200,
            final_url: url.into(),
            content_type: Some("application/json;charset=UTF-8".into()),
            body: body.as_bytes().to_vec(),
        }
    }

    fn instrument(code: &str) -> InstrumentId {
        InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            code,
            AssetClass::Equity,
        )
        .unwrap()
    }

    fn request(code: &str, limit: u32) -> InstrumentDateRangeRequest {
        InstrumentDateRangeRequest::new(
            instrument(code),
            magic_market_core::PositiveU32::new(limit).unwrap(),
        )
        .unwrap()
    }

    fn timestamp_nanos(value: &str) -> u128 {
        let (seconds, nanos) = value.split_once('.').unwrap();
        seconds.parse::<u128>().unwrap() * 1_000_000_000 + nanos.parse::<u128>().unwrap()
    }

    #[test]
    fn organization_mapping_and_announcement_preserve_optional_category_and_pdf() {
        let transport = FixtureTransport::new(vec![
            response(
                DEFAULT_MAPPING_URL,
                include_str!("../tests/fixtures/organizations.json"),
            ),
            response(
                DEFAULT_ANNOUNCEMENT_URL,
                include_str!("../tests/fixtures/announcements_page.json"),
            ),
        ]);
        let observed = transport.clone();
        let client = CninfoClient::with_test_transport(transport);
        let batch = client.announcements(&request("600396", 2)).unwrap();
        assert_eq!(batch.records().len(), 2);
        let first = &batch.records()[0];
        assert_eq!(first.announcement_id.as_str(), "1225438962");
        assert!(first.category.is_none());
        assert_eq!(
            first.pdf_url.as_ref().map(HttpsUrl::as_str),
            Some("https://static.cninfo.com.cn/finalpage/2026-07-24/1225438962.PDF")
        );
        assert_eq!(
            first.canonical_url.as_str(),
            "https://www.cninfo.com.cn/new/disclosure/detail?stockCode=600396&announcementId=1225438962&orgId=gssh0600396&announcementTime=2026-07-24"
        );
        assert_eq!(first.evidence.provider(), ProviderId::Cninfo);
        assert!(batch.quality().is_complete());
        let requests = observed.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(String::from_utf8_lossy(&requests[1].body).contains("isHLtitle=false"));
        assert!(!String::from_utf8_lossy(&requests[1].body).contains("Cookie"));
    }

    #[test]
    fn full_market_announcement_discovery_keeps_stock_code_and_name() {
        let body = r#"{
          "hasMore": false,
          "totalAnnouncement": 2,
          "totalRecordNum": 2,
          "totalpages": 1,
          "announcements": [
            {
              "announcementId": "A-SH",
              "secCode": "600396",
              "secName": "华电辽能",
              "announcementTitle": "上海公告",
              "announcementTypeName": "公司公告",
              "announcementTime": 1784822400000,
              "adjunctUrl": "finalpage/2026-07-24/A-SH.PDF"
            },
            {
              "announcementId": "A-SZ",
              "secCode": "002594",
              "secName": "比亚迪",
              "announcementTitle": "深圳公告",
              "announcementTypeName": "公司公告",
              "announcementTime": 1784822400000,
              "adjunctUrl": "finalpage/2026-07-24/A-SZ.PDF"
            }
          ]
        }"#;
        let client = CninfoClient::with_test_transport(FixtureTransport::new(vec![response(
            DEFAULT_ANNOUNCEMENT_URL,
            body,
        )]));
        let request = AnnouncementDiscoveryRequest::new(
            magic_market_core::IsoDate::new("2026-07-24").unwrap(),
            magic_market_core::IsoDate::new("2026-07-24").unwrap(),
            magic_market_core::PositiveU32::new(2).unwrap(),
        )
        .unwrap();
        let batch = client.discover_announcements(&request).unwrap();
        assert_eq!(batch.records().len(), 2);
        assert_eq!(batch.records()[0].instrument.code(), "600396");
        assert_eq!(
            batch.records()[0]
                .instrument_name
                .as_ref()
                .unwrap()
                .as_str(),
            "华电辽能"
        );
        assert_eq!(batch.records()[1].instrument.code(), "002594");
        assert_eq!(
            batch.records()[1]
                .instrument_name
                .as_ref()
                .unwrap()
                .as_str(),
            "比亚迪"
        );
    }

    #[test]
    fn investor_questions_keep_answer_absence_and_only_source_answer_time() {
        let transport = FixtureTransport::new(vec![
            response(
                DEFAULT_IRM_LOOKUP_URL,
                include_str!("../tests/fixtures/irm_lookup.json"),
            ),
            response(
                "https://irm.cninfo.com.cn/newircs/company/question?_t=1&stockcode=002594&orgId=gshk0001211&pageSize=30&pageNum=1&keyWord=&startDay=&endDay=",
                include_str!("../tests/fixtures/irm_questions.json"),
            ),
        ]);
        let client = CninfoClient::with_test_transport(transport);
        let shenzhen = InstrumentId::new(
            magic_market_core::Exchange::Shenzhen,
            "002594",
            AssetClass::Equity,
        )
        .unwrap();
        let request = InstrumentDateRangeRequest::new(
            shenzhen,
            magic_market_core::PositiveU32::new(2).unwrap(),
        )
        .unwrap();
        let batch = client.investor_questions(&request).unwrap();
        assert_eq!(batch.records().len(), 2);
        assert!(batch.records()[0].answer().is_none());
        assert!(batch.records()[0].answerer().is_none());
        assert!(batch.records()[1].answer().is_some());
        assert_eq!(
            batch.records()[1].answerer().map(NonEmptyText::as_str),
            Some("比亚迪")
        );
        assert!(batch.records()[1].answer_at().is_none());
        assert_eq!(
            batch.records()[1]
                .source_question_id()
                .map(NonEmptyText::as_str),
            Some("2310153346199089152")
        );
    }

    #[test]
    fn pagination_failure_and_identity_mismatch_are_explicit() {
        let config = CninfoConfig {
            max_pages: 1,
            ..CninfoConfig::default()
        };
        let transport = FixtureTransport::new(vec![
            response(
                DEFAULT_MAPPING_URL,
                include_str!("../tests/fixtures/organizations.json"),
            ),
            response(
                DEFAULT_ANNOUNCEMENT_URL,
                r#"{"hasMore":true,"announcements":[]}"#,
            ),
        ]);
        let client = CninfoClient::from_parts(Duration::ZERO, config, Arc::new(transport));
        assert!(matches!(
            client.announcements(&request("600396", 2)),
            Err(CninfoError::Schema(message)) if message.contains("hasMore")
        ));
    }

    #[test]
    fn empty_announcement_and_question_results_are_explicitly_incomplete() {
        let announcements = CninfoClient::with_test_transport(FixtureTransport::new(vec![
            response(
                DEFAULT_MAPPING_URL,
                include_str!("../tests/fixtures/organizations.json"),
            ),
            response(
                DEFAULT_ANNOUNCEMENT_URL,
                r#"{"hasMore":false,"announcements":[]}"#,
            ),
        ]));
        assert!(matches!(
            announcements.announcements(&request("600396", 1)),
            Err(CninfoError::Incomplete(message)) if message.contains("no announcements")
        ));

        let questions = CninfoClient::with_test_transport(FixtureTransport::new(vec![
            response(
                DEFAULT_IRM_LOOKUP_URL,
                include_str!("../tests/fixtures/irm_lookup.json"),
            ),
            response(
                "https://irm.cninfo.com.cn/newircs/company/question?_t=1&stockcode=002594&orgId=gshk0001211&pageSize=30&pageNum=1&keyWord=&startDay=&endDay=",
                r#"{"total":0,"rows":[]}"#,
            ),
        ]));
        let instrument = InstrumentId::new(
            magic_market_core::Exchange::Shenzhen,
            "002594",
            AssetClass::Equity,
        )
        .unwrap();
        let request = InstrumentDateRangeRequest::new(
            instrument,
            magic_market_core::PositiveU32::new(1).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            questions.investor_questions(&request),
            Err(CninfoError::Incomplete(message)) if message.contains("no investor questions")
        ));
    }

    #[test]
    fn remote_page_width_stays_fixed_for_fifty_record_requests() {
        let announcements = PagedFixtureTransport::new(PagedFixtureKind::Announcements);
        let observed_announcements = announcements.clone();
        let client = CninfoClient::with_test_transport(announcements);
        let batch = client
            .announcements(&request("600396", 50))
            .expect("two fixed-width announcement pages should not overlap");
        assert_eq!(batch.records().len(), 50);
        assert_eq!(
            batch
                .records()
                .iter()
                .map(|record| record.announcement_id.as_str())
                .collect::<HashSet<_>>()
                .len(),
            50
        );
        let requests = observed_announcements.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests[1]
            .body
            .windows(11)
            .any(|part| part == b"pageSize=30"));
        assert!(requests[2]
            .body
            .windows(11)
            .any(|part| part == b"pageSize=30"));

        let questions = PagedFixtureTransport::new(PagedFixtureKind::Questions);
        let observed_questions = questions.clone();
        let client = CninfoClient::with_test_transport(questions);
        let instrument = InstrumentId::new(
            magic_market_core::Exchange::Shenzhen,
            "002594",
            AssetClass::Equity,
        )
        .unwrap();
        let request = InstrumentDateRangeRequest::new(
            instrument,
            magic_market_core::PositiveU32::new(50).unwrap(),
        )
        .unwrap();
        let batch = client
            .investor_questions(&request)
            .expect("two fixed-width question pages should not overlap");
        assert_eq!(batch.records().len(), 50);
        assert_eq!(
            batch
                .records()
                .iter()
                .map(|record| record.question_id().as_str())
                .collect::<HashSet<_>>()
                .len(),
            50
        );
        let requests = observed_questions.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests[1].url.contains("pageSize=30"));
        assert!(requests[2].url.contains("pageSize=30"));
    }

    #[test]
    fn code_prefix_must_match_the_declared_exchange() {
        let mismatches = [
            (magic_market_core::Exchange::Shanghai, "002594"),
            (magic_market_core::Exchange::Shenzhen, "600396"),
            (magic_market_core::Exchange::Beijing, "300001"),
        ];
        for (exchange, code) in mismatches {
            let instrument = InstrumentId::new(exchange, code, AssetClass::Equity).unwrap();
            assert!(matches!(
                validate_instrument(&instrument),
                Err(CninfoError::InvalidRequest(message)) if message.contains("exchange")
            ));
        }
        let unsupported = instrument("100001");
        assert!(matches!(
            validate_instrument(&unsupported),
            Err(CninfoError::Unsupported(message)) if message.contains("prefix")
        ));

        let verified_beijing = InstrumentId::new(
            magic_market_core::Exchange::Beijing,
            "920001",
            AssetClass::Equity,
        )
        .unwrap();
        assert!(validate_instrument(&verified_beijing).is_ok());

        let unverified_nine_prefix = InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            "900901",
            AssetClass::Equity,
        )
        .unwrap();
        assert!(matches!(
            validate_instrument(&unverified_nine_prefix),
            Err(CninfoError::Unsupported(message)) if message.contains("prefix")
        ));
    }

    #[test]
    fn batch_observation_time_is_not_before_the_final_response() {
        let announcements = CompletionTransport::new(vec![
            response(
                DEFAULT_MAPPING_URL,
                include_str!("../tests/fixtures/organizations.json"),
            ),
            response(
                DEFAULT_ANNOUNCEMENT_URL,
                include_str!("../tests/fixtures/announcements_page.json"),
            ),
        ]);
        let observed_announcements = announcements.clone();
        let batch = CninfoClient::with_test_transport(announcements)
            .announcements(&request("600396", 2))
            .unwrap();
        let completed_at = observed_announcements.completed_at.lock().unwrap().unwrap();
        assert!(timestamp_nanos(batch.provenance().fetched_at()) >= completed_at);

        let questions = CompletionTransport::new(vec![
            response(
                DEFAULT_IRM_LOOKUP_URL,
                include_str!("../tests/fixtures/irm_lookup.json"),
            ),
            response(
                "https://irm.cninfo.com.cn/newircs/company/question?_t=1&stockcode=002594&orgId=gshk0001211&pageSize=30&pageNum=1&keyWord=&startDay=&endDay=",
                include_str!("../tests/fixtures/irm_questions.json"),
            ),
        ]);
        let observed_questions = questions.clone();
        let instrument = InstrumentId::new(
            magic_market_core::Exchange::Shenzhen,
            "002594",
            AssetClass::Equity,
        )
        .unwrap();
        let request = InstrumentDateRangeRequest::new(
            instrument,
            magic_market_core::PositiveU32::new(2).unwrap(),
        )
        .unwrap();
        let batch = CninfoClient::with_test_transport(questions)
            .investor_questions(&request)
            .unwrap();
        let completed_at = observed_questions.completed_at.lock().unwrap().unwrap();
        assert!(timestamp_nanos(batch.provenance().fetched_at()) >= completed_at);
    }

    #[test]
    fn strict_hosts_content_type_and_body_caps_are_enforced() {
        let config = CninfoConfig {
            mapping_url: "https://example.com/map.json".into(),
            ..CninfoConfig::default()
        };
        assert!(matches!(
            CninfoClient::with_transport(config, FixtureTransport::new(Vec::new())),
            Err(CninfoError::InvalidRequest(message)) if message.contains("allowlisted")
        ));

        let oversized = HttpResponse {
            status: 200,
            final_url: DEFAULT_MAPPING_URL.into(),
            content_type: Some("application/json".into()),
            body: vec![b' '; MAX_RESPONSE_BYTES + 1],
        };
        let client = CninfoClient::with_test_transport(FixtureTransport::new(vec![oversized]));
        assert!(matches!(
            client.organization_mapping(&instrument("600396")),
            Err(CninfoError::Incomplete(_))
        ));
    }

    #[test]
    fn timestamp_conversion_is_timezone_explicit() {
        assert_eq!(
            unix_seconds_to_china_iso(1_784_822_400).as_deref(),
            Some("2026-07-24T00:00:00+08:00")
        );
    }

    #[test]
    fn capabilities_are_conservative() {
        let capabilities = CninfoClient::capabilities();
        assert!(capabilities.announcements);
        assert!(capabilities.investor_questions);
        assert!(!capabilities.instrument_news);
        assert!(!capabilities.global_news);
    }
}
