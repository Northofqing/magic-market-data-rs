use crate::transport::{
    validate_endpoint, validate_minimum_interval, validate_request, validate_response,
    validate_timeout, ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, HttpsTransport,
    RequestGate,
};
use crate::{ExchangeError, ProviderCapabilities};
use magic_market_core::{
    Announcement, Announcements, AssetClass, Capabilities, CapitalCapabilities,
    ContentCapabilities, DataBatch, Exchange, HttpsUrl, InstrumentDateRangeRequest, InstrumentId,
    NonEmptyText, Provenance, ProviderId, SignalCapabilities, SourceEvidence,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

const ENDPOINT: &str = "https://query.sse.com.cn/security/stock/queryCompanyBulletin.do";
const HOST: &str = "query.sse.com.cn";
const PATH: &str = "/security/stock/queryCompanyBulletin.do";
const DRAGON_TIGER_PATH: &str = "/infodisplay/showTradePublicFile.do";
const CALLBACK: &str = "magicExchange";
const PAGE_SIZE: u32 = 50;
const MAX_RECORDS: u32 = 500;
const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; magic-exchange-rs/0.2; read-only official-data probe)";

#[derive(Debug, Clone)]
pub struct SseConfig {
    pub endpoint: String,
    pub timeout: Duration,
    pub minimum_interval: Duration,
    pub max_pages: u32,
}

impl Default for SseConfig {
    fn default() -> Self {
        Self {
            endpoint: ENDPOINT.into(),
            timeout: Duration::from_secs(15),
            minimum_interval: Duration::from_secs(1),
            max_pages: 10,
        }
    }
}

impl SseConfig {
    fn validate(&self) -> Result<(), ExchangeError> {
        validate_endpoint(&self.endpoint, HOST, PATH)?;
        validate_timeout(self.timeout)?;
        validate_minimum_interval(self.minimum_interval)?;
        if self.max_pages == 0 || self.max_pages > 10 {
            return Err(ExchangeError::InvalidRequest(
                "SSE max_pages must be between 1 and 10".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SseClient {
    config: SseConfig,
    transport: Arc<dyn ExchangeTransport>,
    gate: Arc<RequestGate>,
}

impl std::fmt::Debug for SseClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SseClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl SseClient {
    pub fn new() -> Result<Self, ExchangeError> {
        Self::with_config(SseConfig::default())
    }

    pub fn with_config(config: SseConfig) -> Result<Self, ExchangeError> {
        config.validate()?;
        let transport = HttpsTransport::new(config.timeout)?;
        Self::from_parts(config, Arc::new(transport))
    }

    pub fn with_transport(
        config: SseConfig,
        transport: impl ExchangeTransport + 'static,
    ) -> Result<Self, ExchangeError> {
        config.validate()?;
        Self::from_parts(config, Arc::new(transport))
    }

    fn from_parts(
        config: SseConfig,
        transport: Arc<dyn ExchangeTransport>,
    ) -> Result<Self, ExchangeError> {
        Ok(Self {
            gate: Arc::new(RequestGate::new(config.minimum_interval)),
            config,
            transport,
        })
    }

    pub const fn provider_id() -> ProviderId {
        ProviderId::Sse
    }

    pub const fn capabilities() -> ProviderCapabilities {
        ProviderCapabilities {
            provider: ProviderId::Sse,
            market: Capabilities::new(),
            content: ContentCapabilities {
                instrument_news: false,
                global_news: false,
                announcements: true,
                announcement_discovery: false,
                investor_questions: false,
            },
            capital: CapitalCapabilities {
                fund_flow_series: false,
                board_flow: false,
                margin: false,
                block_trades: false,
                holder_count: false,
                lockups: false,
                dividends: false,
                post_close_flow: false,
                northbound_daily_statistics: false,
            },
            signals: SignalCapabilities {
                board_memberships: false,
                strong_stock_reasons: false,
                dragon_tiger: true,
                market_rankings: false,
                popularity: false,
                concept_hits: false,
            },
        }
    }

    fn execute(&self, request: HttpRequest) -> Result<HttpResponse, ExchangeError> {
        validate_request(&request, HttpMethod::Get, HOST, PATH)?;
        let response = self.gate.execute(|| self.transport.execute(&request))?;
        validate_response(&request, &response, &["json", "javascript"])?;
        Ok(response)
    }

    pub(crate) fn execute_dragon_tiger(
        &self,
        request: HttpRequest,
    ) -> Result<HttpResponse, ExchangeError> {
        validate_request(&request, HttpMethod::Get, HOST, DRAGON_TIGER_PATH)?;
        let response = self.gate.execute(|| self.transport.execute(&request))?;
        validate_response(&request, &response, &["json", "javascript"])?;
        Ok(response)
    }

    fn page(
        &self,
        request: &InstrumentDateRangeRequest,
        page: u32,
    ) -> Result<SsePage, ExchangeError> {
        let mut url = Url::parse(&self.config.endpoint)
            .map_err(|error| ExchangeError::InvalidRequest(error.to_string()))?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("jsonCallBack", CALLBACK)
                .append_pair("isPagination", "true")
                .append_pair("productId", request.instrument().code())
                .append_pair("securityType", "0101")
                .append_pair("reportType2", "DQGG")
                .append_pair("reportType", "ALL")
                .append_pair(
                    "beginDate",
                    request
                        .start()
                        .map_or("", magic_market_core::IsoDate::as_str),
                )
                .append_pair(
                    "endDate",
                    request.end().map_or("", magic_market_core::IsoDate::as_str),
                )
                .append_pair("pageHelp.pageSize", &PAGE_SIZE.to_string())
                .append_pair("pageHelp.pageNo", &page.to_string())
                .append_pair("pageHelp.beginPage", &page.to_string())
                .append_pair("pageHelp.endPage", &page.to_string())
                .append_pair("pageHelp.cacheSize", "1");
        }
        let response = self.execute(HttpRequest {
            method: HttpMethod::Get,
            url: url.into(),
            headers: vec![
                ("User-Agent".into(), USER_AGENT.into()),
                (
                    "Accept".into(),
                    "application/json, text/javascript;q=0.9".into(),
                ),
                (
                    "Referer".into(),
                    "https://www.sse.com.cn/disclosure/listedinfo/announcement/".into(),
                ),
            ],
            body: Vec::new(),
        })?;
        parse_jsonp(&response.body)
    }
}

impl Announcements for SseClient {
    type Error = ExchangeError;

    fn announcements(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error> {
        validate_instrument(request.instrument())?;
        if request.limit().get() > MAX_RECORDS {
            return Err(ExchangeError::InvalidRequest(format!(
                "SSE announcement limit must be at most {MAX_RECORDS}"
            )));
        }
        let limit = request.limit().get() as usize;
        let mut rows = Vec::new();
        let mut seen = HashSet::new();
        let mut expected_total = None;
        let mut page = 1_u32;
        loop {
            if page > self.config.max_pages {
                return Err(ExchangeError::Incomplete(format!(
                    "SSE request requires more than {} pages",
                    self.config.max_pages
                )));
            }
            let document = self.page(request, page)?;
            validate_page_identity(&document, request, page, expected_total)?;
            let page_help = document
                .page_help
                .ok_or_else(|| ExchangeError::Schema("SSE pageHelp is missing".into()))?;
            let total = page_help
                .total
                .ok_or_else(|| ExchangeError::Schema("SSE pageHelp.total is missing".into()))?;
            expected_total = Some(total);
            let page_rows = page_help
                .data
                .ok_or_else(|| ExchangeError::Schema("SSE pageHelp.data is missing".into()))?;
            validate_page_completeness(&page_rows, page, total)?;
            for row in page_rows {
                let id = sse_announcement_id(row.url.as_deref())?;
                if !seen.insert(id) {
                    return Err(ExchangeError::Schema(
                        "duplicate SSE announcement ID across pages".into(),
                    ));
                }
                rows.push(row);
            }
            let consumed = rows.len();
            if consumed >= total as usize || consumed >= limit {
                break;
            }
            page = page
                .checked_add(1)
                .ok_or_else(|| ExchangeError::Incomplete("SSE page overflow".into()))?;
        }
        if rows.is_empty() {
            return Err(ExchangeError::Incomplete(format!(
                "SSE returned no official announcements for {}",
                request.instrument().code()
            )));
        }
        let observed_at = now()?;
        let batch_id = format!(
            "sse:{observed_at}:announcements:{}",
            request.instrument().code()
        );
        let mut records = rows
            .into_iter()
            .map(|row| map_row(row, request, &observed_at, &batch_id))
            .collect::<Result<Vec<_>, _>>()?;
        records.truncate(limit);
        let source_at = records
            .iter()
            .map(|record| record.published_at.as_str())
            .max()
            .ok_or_else(|| ExchangeError::Incomplete("SSE mapped batch is empty".into()))?;
        let provenance = Provenance::new("sse-official", observed_at)?
            .with_source_at(source_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::strict(records, provenance))
    }
}

#[derive(Debug, Deserialize)]
struct SsePage {
    #[serde(rename = "productId")]
    product_id: Option<String>,
    #[serde(rename = "beginDate")]
    begin_date: Option<String>,
    #[serde(rename = "endDate")]
    end_date: Option<String>,
    #[serde(rename = "pageHelp")]
    page_help: Option<SsePageHelp>,
}

#[derive(Debug, Deserialize)]
struct SsePageHelp {
    #[serde(rename = "pageNo")]
    page_no: Option<u32>,
    #[serde(rename = "pageSize")]
    page_size: Option<u32>,
    #[serde(rename = "pageCount")]
    page_count: Option<u32>,
    total: Option<u32>,
    data: Option<Vec<SseAnnouncementWire>>,
}

#[derive(Debug, Deserialize)]
struct SseAnnouncementWire {
    #[serde(rename = "SECURITY_CODE")]
    security_code: Option<String>,
    #[serde(rename = "SSEDATE")]
    source_date: Option<String>,
    #[serde(rename = "TITLE")]
    title: Option<String>,
    #[serde(rename = "BULLETIN_TYPE")]
    category: Option<String>,
    #[serde(rename = "URL")]
    url: Option<String>,
}

fn parse_jsonp(body: &[u8]) -> Result<SsePage, ExchangeError> {
    let text =
        std::str::from_utf8(body).map_err(|error| ExchangeError::Decode(error.to_string()))?;
    let trimmed = text.trim();
    let prefix = format!("{CALLBACK}(");
    let json = trimmed
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| ExchangeError::Decode("SSE response is not expected JSONP".into()))?;
    serde_json::from_str(json).map_err(|error| ExchangeError::Decode(error.to_string()))
}

fn validate_page_identity(
    document: &SsePage,
    request: &InstrumentDateRangeRequest,
    expected_page: u32,
    expected_total: Option<u32>,
) -> Result<(), ExchangeError> {
    if document.product_id.as_deref() != Some(request.instrument().code()) {
        return Err(ExchangeError::Schema(
            "SSE productId does not match requested instrument".into(),
        ));
    }
    if let Some(start) = request.start() {
        if document.begin_date.as_deref() != Some(start.as_str()) {
            return Err(ExchangeError::Schema(
                "SSE beginDate does not match requested range".into(),
            ));
        }
    }
    if let Some(end) = request.end() {
        if document.end_date.as_deref() != Some(end.as_str()) {
            return Err(ExchangeError::Schema(
                "SSE endDate does not match requested range".into(),
            ));
        }
    }
    let page = document
        .page_help
        .as_ref()
        .ok_or_else(|| ExchangeError::Schema("SSE pageHelp is missing".into()))?;
    if page.page_no != Some(expected_page) {
        return Err(ExchangeError::Incomplete(format!(
            "SSE expected page {expected_page}, received {:?}",
            page.page_no
        )));
    }
    if page.page_size != Some(PAGE_SIZE) {
        return Err(ExchangeError::Schema(format!(
            "SSE pageSize must remain fixed at {PAGE_SIZE}"
        )));
    }
    let total = page
        .total
        .ok_or_else(|| ExchangeError::Schema("SSE pageHelp.total is missing".into()))?;
    if total == 0 {
        return Err(ExchangeError::Incomplete(
            "SSE total is zero for strict announcement request".into(),
        ));
    }
    if expected_total.is_some_and(|expected| expected != total) {
        return Err(ExchangeError::Incomplete(
            "SSE total changed between pages".into(),
        ));
    }
    let expected_count = total.div_ceil(PAGE_SIZE);
    if page.page_count != Some(expected_count) {
        return Err(ExchangeError::Incomplete(format!(
            "SSE pageCount {:?} does not match total {total}",
            page.page_count
        )));
    }
    Ok(())
}

fn validate_page_completeness(
    rows: &[SseAnnouncementWire],
    page: u32,
    total: u32,
) -> Result<(), ExchangeError> {
    let expected_start = (page - 1).saturating_mul(PAGE_SIZE);
    if expected_start >= total {
        return Err(ExchangeError::Incomplete(
            "SSE returned a page beyond declared total".into(),
        ));
    }
    let remaining = total - expected_start;
    let expected_rows = remaining.min(PAGE_SIZE) as usize;
    if rows.len() != expected_rows {
        return Err(ExchangeError::Incomplete(format!(
            "SSE page {page} has {} rows, expected {expected_rows}",
            rows.len()
        )));
    }
    Ok(())
}

fn map_row(
    row: SseAnnouncementWire,
    request: &InstrumentDateRangeRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<Announcement, ExchangeError> {
    let code = required(row.security_code, "SSE SECURITY_CODE")?;
    if code != request.instrument().code() {
        return Err(ExchangeError::Schema(format!(
            "SSE source identity {code} does not match requested {}",
            request.instrument().code()
        )));
    }
    let source_date = required(row.source_date, "SSE SSEDATE")?;
    ensure_date_in_range(&source_date, request)?;
    let path = required(row.url, "SSE URL")?;
    let announcement_id = sse_announcement_id(Some(&path))?;
    let pdf_url = sse_pdf_url(&path, request.instrument().code(), &source_date)?;
    let mut evidence = SourceEvidence::new(ProviderId::Sse, observed_at, batch_id)?;
    evidence = evidence.with_source_at(source_date.clone())?;
    Ok(Announcement {
        announcement_id: NonEmptyText::new(announcement_id)?,
        instrument: request.instrument().clone(),
        instrument_name: None,
        category: optional_nonempty(row.category)?,
        title: NonEmptyText::new(required(row.title, "SSE TITLE")?)?,
        published_at: NonEmptyText::new(source_date)?,
        canonical_url: pdf_url.clone(),
        pdf_url: Some(pdf_url),
        evidence,
    })
}

fn validate_instrument(instrument: &InstrumentId) -> Result<(), ExchangeError> {
    if instrument.exchange() != Exchange::Shanghai || instrument.asset_class() != AssetClass::Equity
    {
        return Err(ExchangeError::InvalidRequest(
            "SSE announcements require a Shanghai equity".into(),
        ));
    }
    let code = instrument.code();
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) || !code.starts_with('6')
    {
        return Err(ExchangeError::InvalidRequest(
            "SSE equity code must be six digits beginning with 6".into(),
        ));
    }
    Ok(())
}

fn sse_pdf_url(path: &str, code: &str, date: &str) -> Result<HttpsUrl, ExchangeError> {
    let expected_prefix = format!("/disclosure/listedinfo/announcement/c/new/{date}/{code}_");
    if !path.starts_with(&expected_prefix)
        || !path.to_ascii_lowercase().ends_with(".pdf")
        || path.contains("..")
        || path.contains('\\')
        || path.contains('?')
        || path.contains('#')
    {
        return Err(ExchangeError::Schema(
            "SSE PDF path does not match official instrument/date path".into(),
        ));
    }
    Ok(HttpsUrl::new(format!("https://static.sse.com.cn{path}"))?)
}

fn sse_announcement_id(path: Option<&str>) -> Result<String, ExchangeError> {
    let path = path.ok_or_else(|| ExchangeError::Schema("SSE URL is missing".into()))?;
    let file = path
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.').map(|(stem, _)| stem))
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| ExchangeError::Schema("SSE URL has no announcement ID".into()))?;
    Ok(file.to_owned())
}

fn required(value: Option<String>, field: &str) -> Result<String, ExchangeError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && !value.chars().any(char::is_control))
        .ok_or_else(|| ExchangeError::Schema(format!("{field} is missing or invalid")))
}

fn optional_nonempty(value: Option<String>) -> Result<Option<NonEmptyText>, ExchangeError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(NonEmptyText::new)
        .transpose()
        .map_err(ExchangeError::from)
}

fn ensure_date_in_range(
    date: &str,
    request: &InstrumentDateRangeRequest,
) -> Result<(), ExchangeError> {
    let parsed = magic_market_core::IsoDate::new(date.to_owned())
        .map_err(|_| ExchangeError::Schema(format!("invalid source date {date:?}")))?;
    if request.start().is_some_and(|start| &parsed < start)
        || request.end().is_some_and(|end| &parsed > end)
    {
        return Err(ExchangeError::Schema(format!(
            "source date {date} is outside requested range"
        )));
    }
    Ok(())
}

fn now() -> Result<String, ExchangeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| ExchangeError::Transport(format!("system clock error: {error}")))
}
