use crate::szse_quote::{build_quote_url, parse_quote_snapshot};
use crate::transport::{
    new_request_gate, validate_endpoint, validate_minimum_interval, validate_request,
    validate_response, validate_timeout, wait_for_request_start, ExchangeTransport, HttpMethod,
    HttpRequest, HttpResponse, HttpsTransport, SharedRequestGate,
};
use crate::{ExchangeError, ProviderCapabilities};
use magic_market_core::{
    Announcement, Announcements, AssetClass, Capabilities, CapitalCapabilities,
    ContentCapabilities, DataBatch, DataStatus, Exchange, HttpsUrl, InstrumentDateRangeRequest,
    InstrumentId, NonEmptyText, OrderBook, OrderBooks, Provenance, ProviderId, Quote,
    RealtimeQuotes, SignalCapabilities, SourceEvidence,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ENDPOINT: &str = "https://www.szse.cn/api/disc/announcement/annList";
const HOST: &str = "www.szse.cn";
const PATH: &str = "/api/disc/announcement/annList";
const QUOTE_PATH: &str = "/api/market/ssjjhq/getTimeData";
const DRAGON_TIGER_PATH: &str = "/api/report/ShowReport/data";
const PAGE_SIZE: u32 = 50;
const MAX_RECORDS: u32 = 500;
const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; magic-exchange-rs/0.2; read-only official-data probe)";

#[derive(Debug, Clone)]
pub struct SzseConfig {
    pub endpoint: String,
    pub timeout: Duration,
    pub minimum_interval: Duration,
    pub max_pages: u32,
}

impl Default for SzseConfig {
    fn default() -> Self {
        Self {
            endpoint: ENDPOINT.into(),
            timeout: Duration::from_secs(15),
            minimum_interval: Duration::from_secs(1),
            max_pages: 10,
        }
    }
}

impl SzseConfig {
    fn validate(&self) -> Result<(), ExchangeError> {
        validate_endpoint(&self.endpoint, HOST, PATH)?;
        validate_timeout(self.timeout)?;
        validate_minimum_interval(self.minimum_interval)?;
        if self.max_pages == 0 || self.max_pages > 10 {
            return Err(ExchangeError::InvalidRequest(
                "SZSE max_pages must be between 1 and 10".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SzseClient {
    config: SzseConfig,
    transport: Arc<dyn ExchangeTransport>,
    gate: Arc<SharedRequestGate>,
}

impl std::fmt::Debug for SzseClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SzseClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl SzseClient {
    pub fn new() -> Result<Self, ExchangeError> {
        Self::with_config(SzseConfig::default())
    }

    pub fn with_config(config: SzseConfig) -> Result<Self, ExchangeError> {
        config.validate()?;
        let transport = HttpsTransport::new(config.timeout)?;
        Self::from_parts(config, Arc::new(transport))
    }

    pub fn with_transport(
        config: SzseConfig,
        transport: impl ExchangeTransport + 'static,
    ) -> Result<Self, ExchangeError> {
        config.validate()?;
        Self::from_parts(config, Arc::new(transport))
    }

    fn from_parts(
        config: SzseConfig,
        transport: Arc<dyn ExchangeTransport>,
    ) -> Result<Self, ExchangeError> {
        Ok(Self {
            gate: Arc::new(new_request_gate(config.minimum_interval)?),
            config,
            transport,
        })
    }

    pub const fn provider_id() -> ProviderId {
        ProviderId::Szse
    }

    pub const fn capabilities() -> ProviderCapabilities {
        ProviderCapabilities {
            provider: ProviderId::Szse,
            market: Capabilities {
                quotes: true,
                bars: false,
                minute: false,
                trades: false,
                fundamentals: false,
                corporate_actions: false,
                blocks: false,
                money_flow: false,
                order_book: true,
                auction: false,
                security_metadata: false,
            },
            content: ContentCapabilities {
                instrument_news: false,
                global_news: false,
                announcements: true,
                market_announcements: false,
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
        validate_request(&request, HttpMethod::Post, HOST, PATH)?;
        wait_for_request_start(&self.gate)?;
        let response = self.transport.execute(&request)?;
        validate_response(&request, &response, &["json"])?;
        Ok(response)
    }

    pub(crate) fn execute_dragon_tiger(
        &self,
        request: HttpRequest,
    ) -> Result<HttpResponse, ExchangeError> {
        validate_request(&request, HttpMethod::Get, HOST, DRAGON_TIGER_PATH)?;
        wait_for_request_start(&self.gate)?;
        let response = self.transport.execute(&request)?;
        validate_response(&request, &response, &["json"])?;
        Ok(response)
    }

    fn quote_parts(
        &self,
        instruments: &[InstrumentId],
        kind: &str,
    ) -> Result<QuoteParts, ExchangeError> {
        if instruments.is_empty() || instruments.len() > 20 {
            return Err(ExchangeError::InvalidRequest(
                "SZSE Quote/OrderBook request must contain 1..=20 instruments".into(),
            ));
        }
        let mut seen = HashSet::with_capacity(instruments.len());
        if instruments
            .iter()
            .any(|instrument| !seen.insert(instrument.clone()))
        {
            return Err(ExchangeError::InvalidRequest(
                "SZSE Quote/OrderBook request contains duplicate instruments".into(),
            ));
        }
        let batch_id = format!("szse-official:{}:{kind}", now()?);
        let mut records = Vec::with_capacity(instruments.len());
        let mut source_at = Vec::with_capacity(instruments.len());
        for instrument in instruments {
            let request = HttpRequest {
                method: HttpMethod::Get,
                url: build_quote_url(instrument)?,
                headers: vec![
                    ("User-Agent".into(), USER_AGENT.into()),
                    ("Accept".into(), "application/json".into()),
                ],
                body: Vec::new(),
            };
            validate_request(&request, HttpMethod::Get, HOST, QUOTE_PATH)?;
            wait_for_request_start(&self.gate)?;
            let response = self.transport.execute(&request)?;
            validate_response(&request, &response, &["json"])?;
            let observed_at = now()?;
            let (quote, order_book) =
                parse_quote_snapshot(instrument, &response.body, &observed_at, &batch_id)?
                    .into_parts();
            source_at.push(
                quote
                    .source_at()
                    .ok_or_else(|| ExchangeError::Schema("SZSE Quote lacks source_at".into()))?
                    .to_owned(),
            );
            records.push((quote, order_book));
        }
        let fetched_at = now()?;
        let oldest_source_at = source_at
            .into_iter()
            .min()
            .ok_or_else(|| ExchangeError::Incomplete("SZSE Quote batch is empty".into()))?;
        Ok(QuoteParts {
            records,
            fetched_at,
            batch_id,
            oldest_source_at,
        })
    }

    fn page(
        &self,
        request: &InstrumentDateRangeRequest,
        page: u32,
    ) -> Result<SzsePage, ExchangeError> {
        let range = request
            .start()
            .zip(request.end())
            .map(|(start, end)| vec![start.as_str(), end.as_str()]);
        let body = serde_json::to_vec(&SzseRequest {
            channel_code: ["listedNotice_disc"],
            page_size: PAGE_SIZE,
            page_num: page,
            stock: [request.instrument().code()],
            se_date: range,
        })
        .map_err(|error| ExchangeError::InvalidRequest(error.to_string()))?;
        let response = self.execute(HttpRequest {
            method: HttpMethod::Post,
            url: self.config.endpoint.clone(),
            headers: vec![
                ("User-Agent".into(), USER_AGENT.into()),
                ("Accept".into(), "application/json".into()),
                (
                    "Content-Type".into(),
                    "application/json;charset=UTF-8".into(),
                ),
                (
                    "Referer".into(),
                    "https://www.szse.cn/disclosure/listed/notice/index.html".into(),
                ),
            ],
            body,
        })?;
        serde_json::from_slice(&response.body)
            .map_err(|error| ExchangeError::Decode(error.to_string()))
    }
}

struct QuoteParts {
    records: Vec<(Quote, OrderBook)>,
    fetched_at: String,
    batch_id: String,
    oldest_source_at: String,
}

impl QuoteParts {
    fn provenance(&self) -> Result<Provenance, ExchangeError> {
        Ok(Provenance::new("szse-official", &self.fetched_at)?
            .with_source_at(&self.oldest_source_at)?
            .with_batch_id(&self.batch_id)?)
    }
}

impl RealtimeQuotes for SzseClient {
    type Quote = Quote;
    type Error = ExchangeError;

    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let parts = self.quote_parts(instruments, "quote")?;
        let provenance = parts.provenance()?;
        let records = parts.records.into_iter().map(|(quote, _)| quote).collect();
        Ok(DataBatch::strict(records, provenance))
    }
}

impl OrderBooks for SzseClient {
    type Error = ExchangeError;

    fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        let parts = self.quote_parts(instruments, "order-book")?;
        let provenance = parts.provenance()?;
        let records = parts
            .records
            .into_iter()
            .map(|(_, order_book)| order_book)
            .collect::<Vec<_>>();
        let issues = records
            .iter()
            .filter(|record| record.status() != DataStatus::Available)
            .map(|record| {
                format!(
                    "SZSE order book for {} does not contain all five bid and ask levels",
                    record.instrument().code()
                )
            })
            .collect();
        DataBatch::best_effort(records, provenance, issues).map_err(ExchangeError::from)
    }
}

impl Announcements for SzseClient {
    type Error = ExchangeError;

    fn announcements(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error> {
        validate_instrument(request.instrument())?;
        if request.limit().get() > MAX_RECORDS {
            return Err(ExchangeError::InvalidRequest(format!(
                "SZSE announcement limit must be at most {MAX_RECORDS}"
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
                    "SZSE request requires more than {} pages",
                    self.config.max_pages
                )));
            }
            let document = self.page(request, page)?;
            let total = document
                .announce_count
                .ok_or_else(|| ExchangeError::Schema("SZSE announceCount is missing".into()))?;
            if total == 0 {
                return Err(ExchangeError::Incomplete(
                    "SZSE total is zero for strict announcement request".into(),
                ));
            }
            if expected_total.is_some_and(|expected| expected != total) {
                return Err(ExchangeError::Incomplete(
                    "SZSE announceCount changed between pages".into(),
                ));
            }
            expected_total = Some(total);
            let page_rows = document
                .data
                .ok_or_else(|| ExchangeError::Schema("SZSE data is missing".into()))?;
            validate_page_completeness(&page_rows, page, total)?;
            for row in page_rows {
                let id = row
                    .ann_id
                    .ok_or_else(|| ExchangeError::Schema("SZSE annId is missing".into()))?;
                if !seen.insert(id) {
                    return Err(ExchangeError::Schema(format!(
                        "duplicate SZSE announcement ID {id} across pages"
                    )));
                }
                rows.push(row);
            }
            let consumed = rows.len();
            if consumed >= total as usize || consumed >= limit {
                break;
            }
            page = page
                .checked_add(1)
                .ok_or_else(|| ExchangeError::Incomplete("SZSE page overflow".into()))?;
        }
        if rows.is_empty() {
            return Err(ExchangeError::Incomplete(format!(
                "SZSE returned no official announcements for {}",
                request.instrument().code()
            )));
        }
        let observed_at = now()?;
        let batch_id = format!(
            "szse:{observed_at}:announcements:{}",
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
            .ok_or_else(|| ExchangeError::Incomplete("SZSE mapped batch is empty".into()))?;
        let provenance = Provenance::new("szse-official", observed_at)?
            .with_source_at(source_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::strict(records, provenance))
    }
}

#[derive(Debug, Serialize)]
struct SzseRequest<'a> {
    #[serde(rename = "channelCode")]
    channel_code: [&'a str; 1],
    #[serde(rename = "pageSize")]
    page_size: u32,
    #[serde(rename = "pageNum")]
    page_num: u32,
    stock: [&'a str; 1],
    #[serde(rename = "seDate", skip_serializing_if = "Option::is_none")]
    se_date: Option<Vec<&'a str>>,
}

#[derive(Debug, Deserialize)]
struct SzsePage {
    #[serde(rename = "announceCount")]
    announce_count: Option<u32>,
    data: Option<Vec<SzseAnnouncementWire>>,
}

#[derive(Debug, Deserialize)]
struct SzseAnnouncementWire {
    id: Option<String>,
    #[serde(rename = "annId")]
    ann_id: Option<u64>,
    title: Option<String>,
    #[serde(rename = "publishTime")]
    publish_time: Option<String>,
    #[serde(rename = "attachPath")]
    attach_path: Option<String>,
    #[serde(rename = "attachFormat")]
    attach_format: Option<String>,
    #[serde(rename = "secCode")]
    sec_code: Option<Vec<String>>,
}

fn validate_page_completeness(
    rows: &[SzseAnnouncementWire],
    page: u32,
    total: u32,
) -> Result<(), ExchangeError> {
    let expected_start = (page - 1).saturating_mul(PAGE_SIZE);
    if expected_start >= total {
        return Err(ExchangeError::Incomplete(
            "SZSE returned a page beyond declared total".into(),
        ));
    }
    let remaining = total - expected_start;
    let expected_rows = remaining.min(PAGE_SIZE) as usize;
    if rows.len() != expected_rows {
        return Err(ExchangeError::Incomplete(format!(
            "SZSE page {page} has {} rows, expected {expected_rows}",
            rows.len()
        )));
    }
    Ok(())
}

fn map_row(
    row: SzseAnnouncementWire,
    request: &InstrumentDateRangeRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<Announcement, ExchangeError> {
    let source_id = required(row.id, "SZSE id")?;
    if source_id.len() > 128 {
        return Err(ExchangeError::Schema("SZSE id is too long".into()));
    }
    let ann_id = row
        .ann_id
        .ok_or_else(|| ExchangeError::Schema("SZSE annId is missing".into()))?;
    let source_codes = row
        .sec_code
        .ok_or_else(|| ExchangeError::Schema("SZSE secCode is missing".into()))?;
    if source_codes.as_slice() != [request.instrument().code()] {
        return Err(ExchangeError::Schema(format!(
            "SZSE source identity {source_codes:?} does not exactly match requested {}",
            request.instrument().code()
        )));
    }
    let published = required(row.publish_time, "SZSE publishTime")?;
    let source_date = parse_source_date(&published)?;
    ensure_date_in_range(&source_date, request)?;
    let format = required(row.attach_format, "SZSE attachFormat")?;
    if !format.eq_ignore_ascii_case("pdf") {
        return Err(ExchangeError::Schema(format!(
            "SZSE attachment format {format:?} is not PDF"
        )));
    }
    let path = required(row.attach_path, "SZSE attachPath")?;
    let pdf_url = szse_pdf_url(&path, &source_date)?;
    let canonical_url = HttpsUrl::new(format!(
        "https://www.szse.cn/disclosure/listed/bulletinDetail/index.html?{ann_id}"
    ))?;
    let mut evidence = SourceEvidence::new(ProviderId::Szse, observed_at, batch_id)?;
    evidence = evidence.with_source_at(source_date.clone())?;
    Ok(Announcement {
        announcement_id: NonEmptyText::new(ann_id.to_string())?,
        instrument: request.instrument().clone(),
        instrument_name: None,
        category: None,
        title: NonEmptyText::new(required(row.title, "SZSE title")?)?,
        published_at: NonEmptyText::new(source_date)?,
        canonical_url,
        pdf_url: Some(pdf_url),
        evidence,
    })
}

fn validate_instrument(instrument: &InstrumentId) -> Result<(), ExchangeError> {
    if instrument.exchange() != Exchange::Shenzhen || instrument.asset_class() != AssetClass::Equity
    {
        return Err(ExchangeError::InvalidRequest(
            "SZSE announcements require a Shenzhen equity".into(),
        ));
    }
    let code = instrument.code();
    if code.len() != 6
        || !code.bytes().all(|byte| byte.is_ascii_digit())
        || !matches!(code.as_bytes()[0], b'0' | b'3')
    {
        return Err(ExchangeError::InvalidRequest(
            "SZSE equity code must be six digits beginning with 0 or 3".into(),
        ));
    }
    Ok(())
}

fn szse_pdf_url(path: &str, date: &str) -> Result<HttpsUrl, ExchangeError> {
    let expected_date = format!("/{date}/");
    if !path.starts_with("/disc/")
        || !path.contains(&expected_date)
        || !path.to_ascii_lowercase().ends_with(".pdf")
        || path.contains("..")
        || path.contains('\\')
        || path.contains('?')
        || path.contains('#')
    {
        return Err(ExchangeError::Schema(
            "SZSE PDF path does not match official date path".into(),
        ));
    }
    Ok(HttpsUrl::new(format!(
        "https://disc.static.szse.cn/download{path}"
    ))?)
}

fn required(value: Option<String>, field: &str) -> Result<String, ExchangeError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && !value.chars().any(char::is_control))
        .ok_or_else(|| ExchangeError::Schema(format!("{field} is missing or invalid")))
}

fn parse_source_date(published: &str) -> Result<String, ExchangeError> {
    let bytes = published
        .as_bytes()
        .get(..10)
        .ok_or_else(|| ExchangeError::Schema("SZSE publishTime lacks YYYY-MM-DD".into()))?;
    let is_date = bytes.iter().enumerate().all(|(index, byte)| match index {
        4 | 7 => *byte == b'-',
        _ => byte.is_ascii_digit(),
    });
    if !is_date {
        return Err(ExchangeError::Schema(
            "SZSE publishTime must begin with ASCII YYYY-MM-DD".into(),
        ));
    }
    let date = std::str::from_utf8(bytes)
        .map_err(|error| ExchangeError::Schema(format!("invalid SZSE source date: {error}")))?;
    magic_market_core::IsoDate::new(date.to_owned())
        .map_err(|_| ExchangeError::Schema(format!("invalid SZSE source date {date:?}")))?;
    Ok(date.to_owned())
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
