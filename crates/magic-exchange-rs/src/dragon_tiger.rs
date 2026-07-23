use crate::transport::{HttpMethod, HttpRequest};
use crate::{ExchangeError, SseClient, SzseClient};
use magic_market_core::{
    AssetClass, DataBatch, DragonTigerData, DragonTigerEntry, DragonTigerSeat, DragonTigerSide,
    Exchange, InstrumentId, InstrumentSignalRequest, IsoDate, Money, NonEmptyText, PositiveU32,
    Provenance, ProviderId, SourceEvidence,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use url::Url;

const SSE_ENDPOINT: &str = "https://query.sse.com.cn/infodisplay/showTradePublicFile.do";
const SSE_CALLBACK: &str = "magicExchange";
const SZSE_REPORT_ENDPOINT: &str = "https://www.szse.cn/api/report/ShowReport/data";
const SZSE_LIST_CATALOG: &str = "1842_xxpl_after";
const SZSE_DETAIL_CATALOG: &str = "1842_detal";
const SZSE_PAGE_SIZE: u32 = 10;
const SZSE_MAX_PAGES: u32 = 50;
const SZSE_MAX_RECORDS: u32 = SZSE_PAGE_SIZE * SZSE_MAX_PAGES;
pub const MAX_DRAGON_TIGER_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_DRAGON_TIGER_RECORDS: u32 = 500;
const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; magic-exchange-rs/0.2; read-only official-data probe)";
const SSE_REFERER: &str = "https://www.sse.com.cn/disclosure/diclosure/public/dailydata/";
const SZSE_REFERER: &str = "https://www.szse.cn/disclosure/deal/public/index.html";

#[derive(Debug, Error)]
pub enum DragonTigerParseError {
    #[error("invalid official dragon-tiger request: {0}")]
    InvalidRequest(String),
    #[error("official dragon-tiger response decoding failed: {0}")]
    Decode(String),
    #[error("official dragon-tiger schema drift: {0}")]
    Schema(String),
    #[error("official dragon-tiger response is incomplete: {0}")]
    Incomplete(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

impl From<DragonTigerParseError> for ExchangeError {
    fn from(error: DragonTigerParseError) -> Self {
        match error {
            DragonTigerParseError::InvalidRequest(message) => Self::InvalidRequest(message),
            DragonTigerParseError::Decode(message) => Self::Decode(message),
            DragonTigerParseError::Schema(message) => Self::Schema(message),
            DragonTigerParseError::Incomplete(message) => Self::Incomplete(message),
            DragonTigerParseError::Core(error) => Self::Core(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialDragonTigerRequest {
    instrument: InstrumentId,
    trading_date: IsoDate,
}

impl OfficialDragonTigerRequest {
    pub fn new(
        instrument: InstrumentId,
        trading_date: IsoDate,
    ) -> Result<Self, DragonTigerParseError> {
        validate_equity_identity(&instrument)?;
        Ok(Self {
            instrument,
            trading_date,
        })
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn trading_date(&self) -> &IsoDate {
        &self.trading_date
    }

    pub fn sse_url(&self) -> Result<String, DragonTigerParseError> {
        if self.instrument.exchange() != Exchange::Shanghai {
            return Err(DragonTigerParseError::InvalidRequest(
                "SSE query requires a Shanghai equity".into(),
            ));
        }
        let mut url = Url::parse(SSE_ENDPOINT)
            .map_err(|error| DragonTigerParseError::InvalidRequest(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("jsonCallBack", SSE_CALLBACK)
            .append_pair("isPagination", "false")
            .append_pair("dateTx", self.trading_date.as_str());
        Ok(url.into())
    }

    pub fn szse_list_url(&self, page: u32) -> Result<String, DragonTigerParseError> {
        if self.instrument.exchange() != Exchange::Shenzhen {
            return Err(DragonTigerParseError::InvalidRequest(
                "SZSE query requires a Shenzhen equity".into(),
            ));
        }
        if !(1..=SZSE_MAX_PAGES).contains(&page) {
            return Err(DragonTigerParseError::InvalidRequest(format!(
                "SZSE page must be between 1 and {SZSE_MAX_PAGES}"
            )));
        }
        let mut url = Url::parse(SZSE_REPORT_ENDPOINT)
            .map_err(|error| DragonTigerParseError::InvalidRequest(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("SHOWTYPE", "JSON")
            .append_pair("CATALOGID", SZSE_LIST_CATALOG)
            .append_pair("TABKEY", "tab1")
            .append_pair("PAGENO", &page.to_string())
            .append_pair("tab1PAGESIZE", &SZSE_PAGE_SIZE.to_string())
            .append_pair("txtDMorJC", self.instrument.code())
            .append_pair("txtStart", self.trading_date.as_str())
            .append_pair("txtEnd", self.trading_date.as_str());
        Ok(url.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SzseDragonTigerDetailKey {
    trading_date: IsoDate,
    instrument_code: String,
    indicator_code: String,
}

impl SzseDragonTigerDetailKey {
    pub fn new(
        trading_date: IsoDate,
        instrument_code: impl Into<String>,
        indicator_code: impl Into<String>,
    ) -> Result<Self, DragonTigerParseError> {
        let instrument_code = instrument_code.into();
        if !is_six_ascii_digits(&instrument_code)
            || !matches!(instrument_code.as_bytes().first(), Some(b'0' | b'3'))
        {
            return Err(DragonTigerParseError::InvalidRequest(
                "SZSE detail code must be a six-digit Shenzhen equity code".into(),
            ));
        }
        let indicator_code = indicator_code.into();
        if indicator_code.len() != 4 || !indicator_code.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(DragonTigerParseError::InvalidRequest(
                "SZSE indicator code must contain exactly four ASCII digits".into(),
            ));
        }
        Ok(Self {
            trading_date,
            instrument_code,
            indicator_code,
        })
    }

    pub fn trading_date(&self) -> &IsoDate {
        &self.trading_date
    }

    pub fn instrument_code(&self) -> &str {
        &self.instrument_code
    }

    pub fn indicator_code(&self) -> &str {
        &self.indicator_code
    }

    pub fn entry_id(&self) -> String {
        format!(
            "szse:{}:{}:{}",
            self.trading_date.as_str(),
            self.instrument_code,
            self.indicator_code
        )
    }

    pub fn url(&self) -> Result<String, DragonTigerParseError> {
        let mut url = Url::parse(SZSE_REPORT_ENDPOINT)
            .map_err(|error| DragonTigerParseError::InvalidRequest(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("SHOWTYPE", "JSON")
            .append_pair("CATALOGID", SZSE_DETAIL_CATALOG)
            .append_pair("TABKEY", "tab1,tab2")
            .append_pair("DQRQ", self.trading_date.as_str())
            .append_pair("ZQDM", &self.instrument_code)
            .append_pair("ZBDM", &self.indicator_code);
        Ok(url.into())
    }
}

#[derive(Debug)]
pub struct ParsedDragonTiger {
    pub entries: Vec<DragonTigerEntry>,
    pub seats: Vec<DragonTigerSeat>,
}

#[derive(Debug)]
pub struct SzseDragonTigerListItem {
    pub entry: DragonTigerEntry,
    pub detail_key: SzseDragonTigerDetailKey,
}

#[derive(Debug)]
pub struct SzseDragonTigerListPage {
    pub page_no: u32,
    pub page_count: u32,
    pub record_count: u32,
    pub items: Vec<SzseDragonTigerListItem>,
}

impl DragonTigerData for SseClient {
    type Error = ExchangeError;

    fn dragon_tiger_entries(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerEntry>, Self::Error> {
        let official = official_request(request, Exchange::Shanghai)?;
        let batch_id = batch_id(ProviderId::Sse, "dragon-tiger-entries", &official)?;
        let mut parsed = fetch_sse(self, &official, &batch_id)?;
        parsed.entries.truncate(request.limit().get() as usize);
        strict_batch(
            parsed.entries,
            "sse-official",
            official.trading_date(),
            &batch_id,
        )
    }

    fn dragon_tiger_seats(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerSeat>, Self::Error> {
        let entry_limit = complete_entry_limit(request)?;
        let official = official_request(request, Exchange::Shanghai)?;
        let batch_id = batch_id(ProviderId::Sse, "dragon-tiger-seats", &official)?;
        let parsed = fetch_sse(self, &official, &batch_id)?;
        let selected = parsed
            .entries
            .iter()
            .take(entry_limit)
            .map(|entry| entry.entry_id().as_str().to_owned())
            .collect::<HashSet<_>>();
        let seats = parsed
            .seats
            .into_iter()
            .filter(|seat| selected.contains(seat.entry_id().as_str()))
            .collect::<Vec<_>>();
        ensure_complete_seat_groups(&seats, selected.len())?;
        strict_batch(seats, "sse-official", official.trading_date(), &batch_id)
    }
}

impl DragonTigerData for SzseClient {
    type Error = ExchangeError;

    fn dragon_tiger_entries(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerEntry>, Self::Error> {
        let official = official_request(request, Exchange::Shenzhen)?;
        let batch_id = batch_id(ProviderId::Szse, "dragon-tiger-entries", &official)?;
        let mut items = fetch_all_szse_entries(self, &official, &batch_id)?;
        items.truncate(request.limit().get() as usize);
        let entries = items.into_iter().map(|item| item.entry).collect();
        strict_batch(entries, "szse-official", official.trading_date(), &batch_id)
    }

    fn dragon_tiger_seats(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerSeat>, Self::Error> {
        let entry_limit = complete_entry_limit(request)?;
        let official = official_request(request, Exchange::Shenzhen)?;
        let batch_id = batch_id(ProviderId::Szse, "dragon-tiger-seats", &official)?;
        let items = fetch_all_szse_entries(self, &official, &batch_id)?;
        let selected = items.into_iter().take(entry_limit).collect::<Vec<_>>();
        let mut seats = Vec::with_capacity(selected.len() * 10);
        let mut identities = HashSet::with_capacity(selected.len() * 10);
        for item in &selected {
            let response = self.execute_dragon_tiger(HttpRequest {
                method: HttpMethod::Get,
                url: item.detail_key.url()?,
                headers: official_headers(SZSE_REFERER, "application/json"),
                body: Vec::new(),
            })?;
            let observed_at = observed_at()?;
            for seat in parse_szse_detail_response(
                &response.body,
                &item.detail_key,
                &observed_at,
                &batch_id,
            )? {
                let side = match seat.side() {
                    DragonTigerSide::Buy => "buy",
                    DragonTigerSide::Sell => "sell",
                };
                let identity = format!("{}:{side}:{}", seat.entry_id().as_str(), seat.rank().get());
                if !identities.insert(identity.clone()) {
                    return Err(ExchangeError::Schema(format!(
                        "duplicate SZSE dragon-tiger seat identity {identity}"
                    )));
                }
                seats.push(seat);
            }
        }
        ensure_complete_seat_groups(&seats, selected.len())?;
        strict_batch(seats, "szse-official", official.trading_date(), &batch_id)
    }
}

fn official_request(
    request: &InstrumentSignalRequest,
    expected_exchange: Exchange,
) -> Result<OfficialDragonTigerRequest, ExchangeError> {
    if request.limit().get() > MAX_DRAGON_TIGER_RECORDS {
        return Err(ExchangeError::InvalidRequest(format!(
            "official dragon-tiger limit must be at most {MAX_DRAGON_TIGER_RECORDS}"
        )));
    }
    if request.instrument().exchange() != expected_exchange {
        return Err(ExchangeError::InvalidRequest(format!(
            "official dragon-tiger Provider requires {expected_exchange:?} instrument"
        )));
    }
    let trading_date = request.trading_date().cloned().ok_or_else(|| {
        ExchangeError::InvalidRequest(
            "official dragon-tiger request requires an explicit trading date".into(),
        )
    })?;
    OfficialDragonTigerRequest::new(request.instrument().clone(), trading_date)
        .map_err(ExchangeError::from)
}

fn complete_entry_limit(request: &InstrumentSignalRequest) -> Result<usize, ExchangeError> {
    if request.limit().get() < 10 {
        return Err(ExchangeError::InvalidRequest(
            "dragon-tiger seat limit must be at least 10 to preserve complete buy-five/sell-five groups"
                .into(),
        ));
    }
    Ok((request.limit().get() / 10) as usize)
}

fn fetch_sse(
    client: &SseClient,
    request: &OfficialDragonTigerRequest,
    batch_id: &str,
) -> Result<ParsedDragonTiger, ExchangeError> {
    let response = client.execute_dragon_tiger(HttpRequest {
        method: HttpMethod::Get,
        url: request.sse_url()?,
        headers: official_headers(SSE_REFERER, "application/json, text/javascript;q=0.9"),
        body: Vec::new(),
    })?;
    let observed_at = observed_at()?;
    parse_sse_response(&response.body, request, &observed_at, batch_id).map_err(ExchangeError::from)
}

fn fetch_all_szse_entries(
    client: &SzseClient,
    request: &OfficialDragonTigerRequest,
    batch_id: &str,
) -> Result<Vec<SzseDragonTigerListItem>, ExchangeError> {
    let mut page_no = 1_u32;
    let mut expected_page_count = None;
    let mut expected_record_count = None;
    let mut identities = HashSet::new();
    let mut items = Vec::new();
    loop {
        let response = client.execute_dragon_tiger(HttpRequest {
            method: HttpMethod::Get,
            url: request.szse_list_url(page_no)?,
            headers: official_headers(SZSE_REFERER, "application/json"),
            body: Vec::new(),
        })?;
        let observed_at = observed_at()?;
        let page =
            parse_szse_list_response(&response.body, request, page_no, &observed_at, batch_id)?;
        if expected_page_count.is_some_and(|expected| expected != page.page_count)
            || expected_record_count.is_some_and(|expected| expected != page.record_count)
        {
            return Err(ExchangeError::Incomplete(
                "SZSE dragon-tiger pagination totals changed between pages".into(),
            ));
        }
        expected_page_count = Some(page.page_count);
        expected_record_count = Some(page.record_count);
        for item in page.items {
            let identity = item.entry.entry_id().as_str().to_owned();
            if !identities.insert(identity.clone()) {
                return Err(ExchangeError::Schema(format!(
                    "duplicate SZSE dragon-tiger entry identity {identity} across pages"
                )));
            }
            items.push(item);
        }
        if page_no == page.page_count {
            break;
        }
        page_no = page_no
            .checked_add(1)
            .ok_or_else(|| ExchangeError::Incomplete("SZSE page overflow".into()))?;
    }
    if items.len() != expected_record_count.unwrap_or_default() as usize {
        return Err(ExchangeError::Incomplete(format!(
            "SZSE dragon-tiger returned {} entries, expected {}",
            items.len(),
            expected_record_count.unwrap_or_default()
        )));
    }
    Ok(items)
}

fn ensure_complete_seat_groups(
    seats: &[DragonTigerSeat],
    expected_entries: usize,
) -> Result<(), ExchangeError> {
    let expected_seats = expected_entries
        .checked_mul(10)
        .ok_or_else(|| ExchangeError::Incomplete("dragon-tiger seat count overflow".into()))?;
    if seats.len() != expected_seats || seats.is_empty() {
        return Err(ExchangeError::Incomplete(format!(
            "dragon-tiger seats contain {} records, expected {expected_seats} complete records",
            seats.len()
        )));
    }
    Ok(())
}

fn strict_batch<T>(
    records: Vec<T>,
    source: &str,
    trading_date: &IsoDate,
    batch_id: &str,
) -> Result<DataBatch<T>, ExchangeError> {
    if records.is_empty() {
        return Err(ExchangeError::Incomplete(
            "official dragon-tiger batch is empty".into(),
        ));
    }
    let fetched_at = observed_at()?;
    let provenance = Provenance::new(source, fetched_at)?
        .with_source_at(trading_date.as_str())?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn batch_id(
    provider: ProviderId,
    operation: &str,
    request: &OfficialDragonTigerRequest,
) -> Result<String, ExchangeError> {
    Ok(format!(
        "{provider:?}:{}:{}:{operation}:{}",
        request.trading_date().as_str(),
        request.instrument().code(),
        observed_at()?
    ))
}

fn official_headers(referer: &str, accept: &str) -> Vec<(String, String)> {
    vec![
        ("User-Agent".into(), USER_AGENT.into()),
        ("Accept".into(), accept.into()),
        ("Referer".into(), referer.into()),
    ]
}

fn observed_at() -> Result<String, ExchangeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| ExchangeError::Transport(format!("system clock error: {error}")))
}

pub fn parse_sse_response(
    body: &[u8],
    request: &OfficialDragonTigerRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<ParsedDragonTiger, DragonTigerParseError> {
    validate_body_size(body)?;
    if request.instrument.exchange() != Exchange::Shanghai {
        return Err(DragonTigerParseError::InvalidRequest(
            "SSE parser requires a Shanghai equity".into(),
        ));
    }
    let text = std::str::from_utf8(body)
        .map_err(|error| DragonTigerParseError::Decode(error.to_string()))?;
    let prefix = format!("{SSE_CALLBACK}(");
    let json = text
        .trim()
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| {
            DragonTigerParseError::Decode(
                "SSE response is not the expected magicExchange JSONP".into(),
            )
        })?;
    let document: SseDocument = serde_json::from_str(json)
        .map_err(|error| DragonTigerParseError::Decode(error.to_string()))?;
    if !document.action_errors.is_empty()
        || !document.action_messages.is_empty()
        || !document.field_errors.is_empty()
    {
        return Err(DragonTigerParseError::Schema(
            "SSE response reports action or field errors".into(),
        ));
    }
    let source_date = IsoDate::new(document.date_tx)?;
    if &source_date != request.trading_date() {
        return Err(DragonTigerParseError::Schema(format!(
            "SSE source date {} does not match requested {}",
            source_date.as_str(),
            request.trading_date().as_str()
        )));
    }
    let date_line = format!(
        "交易日期:{}年{}月{}日",
        &source_date.as_str()[..4],
        &source_date.as_str()[5..7],
        &source_date.as_str()[8..]
    );
    let date_line_count = document
        .file_contents
        .iter()
        .filter(|line| line.trim() == date_line)
        .count();
    if date_line_count != 1 {
        return Err(DragonTigerParseError::Schema(
            "SSE text must contain exactly one matching trading-date line".into(),
        ));
    }
    parse_sse_lines(&document.file_contents, request, observed_at, batch_id)
}

pub fn parse_szse_list_response(
    body: &[u8],
    request: &OfficialDragonTigerRequest,
    expected_page: u32,
    observed_at: &str,
    batch_id: &str,
) -> Result<SzseDragonTigerListPage, DragonTigerParseError> {
    validate_body_size(body)?;
    if request.instrument.exchange() != Exchange::Shenzhen {
        return Err(DragonTigerParseError::InvalidRequest(
            "SZSE parser requires a Shenzhen equity".into(),
        ));
    }
    if !(1..=SZSE_MAX_PAGES).contains(&expected_page) {
        return Err(DragonTigerParseError::InvalidRequest(format!(
            "SZSE page must be between 1 and {SZSE_MAX_PAGES}"
        )));
    }
    let reports: Vec<SzseListReport> = serde_json::from_slice(body)
        .map_err(|error| DragonTigerParseError::Decode(error.to_string()))?;
    if reports.len() != 1 {
        return Err(DragonTigerParseError::Schema(
            "SZSE list response must contain exactly one report".into(),
        ));
    }
    let report = reports
        .into_iter()
        .next()
        .ok_or_else(|| DragonTigerParseError::Schema("SZSE list report is absent".into()))?;
    if report.error.is_some() {
        return Err(DragonTigerParseError::Schema(
            "SZSE list report contains an error field".into(),
        ));
    }
    let metadata = report.metadata;
    require_equal(
        metadata.catalog_id.as_deref(),
        SZSE_LIST_CATALOG,
        "SZSE list catalogid",
    )?;
    require_equal(metadata.tab_key.as_deref(), "tab1", "SZSE list tabkey")?;
    let page_size = required_u32(metadata.page_size, "SZSE list pagesize")?;
    if page_size != SZSE_PAGE_SIZE {
        return Err(DragonTigerParseError::Schema(format!(
            "SZSE list pagesize {page_size} is not fixed {SZSE_PAGE_SIZE}"
        )));
    }
    let page_no = required_u32(metadata.page_no, "SZSE list pageno")?;
    if page_no != expected_page {
        return Err(DragonTigerParseError::Schema(format!(
            "SZSE source page {page_no} does not match requested page {expected_page}"
        )));
    }
    let page_count = required_u32(metadata.page_count, "SZSE list pagecount")?;
    let record_count = required_u32(metadata.record_count, "SZSE list recordcount")?;
    if record_count == 0 || record_count > SZSE_MAX_RECORDS {
        return Err(DragonTigerParseError::Incomplete(format!(
            "SZSE recordcount must be between 1 and {SZSE_MAX_RECORDS}"
        )));
    }
    let expected_page_count = record_count.div_ceil(SZSE_PAGE_SIZE);
    if page_count != expected_page_count || !(1..=SZSE_MAX_PAGES).contains(&page_count) {
        return Err(DragonTigerParseError::Incomplete(
            "SZSE pagecount does not match bounded recordcount".into(),
        ));
    }
    if page_no > page_count {
        return Err(DragonTigerParseError::Incomplete(
            "SZSE returned a page beyond pagecount".into(),
        ));
    }
    validate_condition(&metadata.conditions, "txtDMorJC", request.instrument.code())?;
    validate_condition(
        &metadata.conditions,
        "txtStart",
        request.trading_date.as_str(),
    )?;
    validate_condition(
        &metadata.conditions,
        "txtEnd",
        request.trading_date.as_str(),
    )?;
    validate_list_columns(&metadata.columns)?;
    let expected_rows =
        (record_count - ((page_no - 1) * SZSE_PAGE_SIZE)).min(SZSE_PAGE_SIZE) as usize;
    if report.data.len() != expected_rows {
        return Err(DragonTigerParseError::Incomplete(format!(
            "SZSE page {page_no} has {} rows, expected {expected_rows}",
            report.data.len()
        )));
    }

    let mut seen = HashSet::new();
    let mut items = Vec::with_capacity(report.data.len());
    for row in report.data {
        let source_date = required_text(row.trading_date, "SZSE list dqrq")?;
        if source_date != request.trading_date.as_str() {
            return Err(DragonTigerParseError::Schema(format!(
                "SZSE row date {source_date} does not match requested {}",
                request.trading_date.as_str()
            )));
        }
        let source_code = required_text(row.instrument_code, "SZSE list zqdm")?;
        if source_code != request.instrument.code() {
            return Err(DragonTigerParseError::Schema(format!(
                "SZSE row code {source_code} does not match requested {}",
                request.instrument.code()
            )));
        }
        let _name = NonEmptyText::new(required_text(row.instrument_name, "SZSE list zqjc")?)?;
        let reason = NonEmptyText::new(required_text(row.reason, "SZSE list plyy")?)?;
        let detail_key =
            parse_szse_detail_link(&required_text(row.detail_link, "SZSE list bz")?, request)?;
        let entry_id = detail_key.entry_id();
        if !seen.insert(entry_id.clone()) {
            return Err(DragonTigerParseError::Schema(format!(
                "duplicate SZSE entry identity {entry_id}"
            )));
        }
        let evidence = evidence(
            ProviderId::Szse,
            request.trading_date.as_str(),
            observed_at,
            batch_id,
        )?;
        items.push(SzseDragonTigerListItem {
            entry: DragonTigerEntry::new(
                NonEmptyText::new(entry_id)?,
                request.instrument.clone(),
                request.trading_date.clone(),
                Some(reason),
                None,
                None,
                None,
                None,
                evidence,
            )?,
            detail_key,
        });
    }
    Ok(SzseDragonTigerListPage {
        page_no,
        page_count,
        record_count,
        items,
    })
}

pub fn parse_szse_detail_response(
    body: &[u8],
    expected: &SzseDragonTigerDetailKey,
    observed_at: &str,
    batch_id: &str,
) -> Result<Vec<DragonTigerSeat>, DragonTigerParseError> {
    validate_body_size(body)?;
    let reports: Vec<SzseDetailReport> = serde_json::from_slice(body)
        .map_err(|error| DragonTigerParseError::Decode(error.to_string()))?;
    if reports.len() != 2 {
        return Err(DragonTigerParseError::Schema(
            "SZSE detail response must contain exactly tab1 and tab2".into(),
        ));
    }
    let mut by_tab = HashMap::new();
    for report in reports {
        if report.error.is_some() {
            return Err(DragonTigerParseError::Schema(
                "SZSE detail report contains an error field".into(),
            ));
        }
        require_equal(
            report.metadata.catalog_id.as_deref(),
            SZSE_DETAIL_CATALOG,
            "SZSE detail catalogid",
        )?;
        let tab = required_text(report.metadata.tab_key.clone(), "SZSE detail tabkey")?;
        if by_tab.insert(tab.clone(), report).is_some() {
            return Err(DragonTigerParseError::Schema(format!(
                "duplicate SZSE detail tab {tab}"
            )));
        }
    }
    let summary = by_tab
        .remove("tab1")
        .ok_or_else(|| DragonTigerParseError::Schema("SZSE detail tab1 is missing".into()))?;
    let seats = by_tab
        .remove("tab2")
        .ok_or_else(|| DragonTigerParseError::Schema("SZSE detail tab2 is missing".into()))?;
    if !by_tab.is_empty() {
        return Err(DragonTigerParseError::Schema(
            "SZSE detail contains an unexpected tab".into(),
        ));
    }
    validate_detail_conditions(&summary.metadata.conditions, expected)?;
    validate_detail_conditions(&seats.metadata.conditions, expected)?;
    validate_summary_columns(&summary.metadata.columns)?;
    validate_seat_columns(&seats.metadata.columns)?;
    validate_szse_summary(&summary.data, expected)?;
    map_szse_seats(&seats.data, expected, observed_at, batch_id)
}

#[derive(Debug, Deserialize)]
struct SseDocument {
    #[serde(rename = "actionErrors")]
    action_errors: Vec<Value>,
    #[serde(rename = "actionMessages")]
    action_messages: Vec<Value>,
    #[serde(rename = "fieldErrors")]
    field_errors: HashMap<String, Value>,
    #[serde(rename = "dateTx")]
    date_tx: String,
    #[serde(rename = "fileContents")]
    file_contents: Vec<String>,
}

#[derive(Debug)]
struct SseBlock {
    reason: String,
    entry_number: u32,
    buy: Vec<SseSeat>,
    sell: Vec<SseSeat>,
    mode: SseMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SseMode {
    AwaitBuy,
    Buy,
    Sell,
}

#[derive(Debug)]
struct SseSeat {
    rank: u32,
    name: String,
    amount: f64,
}

fn parse_sse_lines(
    lines: &[String],
    request: &OfficialDragonTigerRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<ParsedDragonTiger, DragonTigerParseError> {
    let mut current_reason: Option<String> = None;
    let mut current: Option<SseBlock> = None;
    let mut entries = Vec::new();
    let mut seats = Vec::new();
    let mut entry_number = 0_u32;

    for line in lines {
        if let Some(reason) = parse_sse_reason_heading(line) {
            if let Some(block) = current.take() {
                finish_sse_block(
                    block,
                    request,
                    observed_at,
                    batch_id,
                    &mut entries,
                    &mut seats,
                )?;
            }
            current_reason = Some(reason);
            continue;
        }
        if let Some(code) = parse_sse_detail_code(line)? {
            if let Some(block) = current.take() {
                finish_sse_block(
                    block,
                    request,
                    observed_at,
                    batch_id,
                    &mut entries,
                    &mut seats,
                )?;
            }
            if code == request.instrument.code() {
                let reason = current_reason.clone().ok_or_else(|| {
                    DragonTigerParseError::Schema(
                        "SSE instrument detail appears before a disclosure reason".into(),
                    )
                })?;
                entry_number = entry_number
                    .checked_add(1)
                    .ok_or_else(|| DragonTigerParseError::Schema("SSE entry overflow".into()))?;
                current = Some(SseBlock {
                    reason,
                    entry_number,
                    buy: Vec::new(),
                    sell: Vec::new(),
                    mode: SseMode::AwaitBuy,
                });
            }
            continue;
        }
        let Some(block) = current.as_mut() else {
            continue;
        };
        let trimmed = line.trim();
        if trimmed.starts_with("买入营业部名称:") {
            if trimmed != "买入营业部名称:                                                                      累计买入金额(元):"
            {
                return Err(DragonTigerParseError::Schema(
                    "SSE buy header or yuan unit changed".into(),
                ));
            }
            if block.mode != SseMode::AwaitBuy {
                return Err(DragonTigerParseError::Schema(
                    "SSE buy header is out of order or duplicated".into(),
                ));
            }
            block.mode = SseMode::Buy;
            continue;
        }
        if trimmed.starts_with("卖出营业部名称:") {
            if trimmed != "卖出营业部名称:                                                                      累计卖出金额(元):"
            {
                return Err(DragonTigerParseError::Schema(
                    "SSE sell header or yuan unit changed".into(),
                ));
            }
            if block.mode != SseMode::Buy || block.buy.len() != 5 {
                return Err(DragonTigerParseError::Incomplete(
                    "SSE sell section starts before five complete buy seats".into(),
                ));
            }
            block.mode = SseMode::Sell;
            continue;
        }
        if trimmed.starts_with('(') {
            let seat = parse_sse_seat_line(trimmed)?;
            let target = match block.mode {
                SseMode::Buy => &mut block.buy,
                SseMode::Sell => &mut block.sell,
                SseMode::AwaitBuy => {
                    return Err(DragonTigerParseError::Schema(
                        "SSE seat row appears before the buy header".into(),
                    ));
                }
            };
            let expected_rank = u32::try_from(target.len())
                .map_err(|_| DragonTigerParseError::Schema("SSE rank overflow".into()))?
                + 1;
            if seat.rank != expected_rank || seat.rank > 5 {
                return Err(DragonTigerParseError::Schema(format!(
                    "SSE seat rank {} does not match expected {expected_rank}",
                    seat.rank
                )));
            }
            target.push(seat);
        }
    }
    if let Some(block) = current {
        finish_sse_block(
            block,
            request,
            observed_at,
            batch_id,
            &mut entries,
            &mut seats,
        )?;
    }
    if entries.is_empty() {
        return Err(DragonTigerParseError::Incomplete(format!(
            "SSE returned no complete official dragon-tiger entry for {}",
            request.instrument.code()
        )));
    }
    Ok(ParsedDragonTiger { entries, seats })
}

fn parse_sse_reason_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let (number, reason) = trimmed.split_once('、')?;
    if number.is_empty()
        || !number
            .chars()
            .all(|value| "零一二三四五六七八九十百".contains(value))
    {
        return None;
    }
    let reason = reason
        .strip_suffix(':')
        .or_else(|| reason.strip_suffix('：'))
        .unwrap_or(reason)
        .trim();
    (!reason.is_empty()).then(|| reason.to_owned())
}

fn parse_sse_detail_code(line: &str) -> Result<Option<String>, DragonTigerParseError> {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix("证券代码:") else {
        return Ok(None);
    };
    let code = rest
        .split_whitespace()
        .next()
        .ok_or_else(|| DragonTigerParseError::Schema("SSE detail code is absent".into()))?;
    if !is_six_ascii_digits(code) {
        return Err(DragonTigerParseError::Schema(format!(
            "SSE detail code {code:?} is invalid"
        )));
    }
    Ok(Some(code.to_owned()))
}

fn parse_sse_seat_line(line: &str) -> Result<SseSeat, DragonTigerParseError> {
    let close = line
        .find(')')
        .ok_or_else(|| DragonTigerParseError::Schema("SSE seat rank is malformed".into()))?;
    let rank_text = line
        .strip_prefix('(')
        .and_then(|value| value.get(..close.saturating_sub(1)))
        .ok_or_else(|| DragonTigerParseError::Schema("SSE seat rank is malformed".into()))?;
    let rank = rank_text
        .parse::<u32>()
        .map_err(|_| DragonTigerParseError::Schema("SSE seat rank is not numeric".into()))?;
    let rest = line
        .get(close + 1..)
        .ok_or_else(|| DragonTigerParseError::Schema("SSE seat row is malformed".into()))?
        .trim();
    let split = rest
        .rfind(char::is_whitespace)
        .ok_or_else(|| DragonTigerParseError::Schema("SSE seat amount is absent".into()))?;
    let name = rest[..split].trim();
    let amount = rest[split..].trim();
    if name.is_empty() || amount.is_empty() {
        return Err(DragonTigerParseError::Schema(
            "SSE seat name or amount is absent".into(),
        ));
    }
    Ok(SseSeat {
        rank,
        name: name.to_owned(),
        amount: parse_yuan_number(amount, "SSE seat amount")?,
    })
}

fn finish_sse_block(
    block: SseBlock,
    request: &OfficialDragonTigerRequest,
    observed_at: &str,
    batch_id: &str,
    entries: &mut Vec<DragonTigerEntry>,
    seats: &mut Vec<DragonTigerSeat>,
) -> Result<(), DragonTigerParseError> {
    if block.mode != SseMode::Sell || block.buy.len() != 5 || block.sell.len() != 5 {
        return Err(DragonTigerParseError::Incomplete(format!(
            "SSE entry {} has {} buy and {} sell seats; exactly five of each are required",
            block.entry_number,
            block.buy.len(),
            block.sell.len()
        )));
    }
    let entry_id = format!(
        "sse:{}:{}:{}",
        request.trading_date.as_str(),
        request.instrument.code(),
        block.entry_number
    );
    let source_at = request.trading_date.as_str();
    let entry_evidence = evidence(ProviderId::Sse, source_at, observed_at, batch_id)?;
    entries.push(DragonTigerEntry::new(
        NonEmptyText::new(entry_id.clone())?,
        request.instrument.clone(),
        request.trading_date.clone(),
        Some(NonEmptyText::new(block.reason)?),
        None,
        None,
        None,
        None,
        entry_evidence,
    )?);
    for (side, side_rows) in [
        (DragonTigerSide::Buy, block.buy),
        (DragonTigerSide::Sell, block.sell),
    ] {
        for row in side_rows {
            let amount = Money::new(row.amount)?;
            let (buy_amount, sell_amount) = match side {
                DragonTigerSide::Buy => (Some(amount), None),
                DragonTigerSide::Sell => (None, Some(amount)),
            };
            seats.push(DragonTigerSeat::new(
                NonEmptyText::new(entry_id.clone())?,
                request.instrument.clone(),
                request.trading_date.clone(),
                side,
                PositiveU32::new(row.rank)?,
                NonEmptyText::new(row.name)?,
                amount,
                buy_amount,
                sell_amount,
                None,
                evidence(ProviderId::Sse, source_at, observed_at, batch_id)?,
            )?);
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct SzseListReport {
    metadata: SzseListMetadata,
    data: Vec<SzseListRow>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct SzseListMetadata {
    #[serde(rename = "catalogid")]
    catalog_id: Option<String>,
    #[serde(rename = "tabkey")]
    tab_key: Option<String>,
    #[serde(rename = "pagesize")]
    page_size: Option<i64>,
    #[serde(rename = "pageno")]
    page_no: Option<i64>,
    #[serde(rename = "pagecount")]
    page_count: Option<i64>,
    #[serde(rename = "recordcount")]
    record_count: Option<i64>,
    #[serde(default)]
    conditions: Vec<SzseCondition>,
    #[serde(rename = "cols", default)]
    columns: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct SzseCondition {
    name: Option<String>,
    #[serde(rename = "defaultValue")]
    default_value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SzseListRow {
    #[serde(rename = "dqrq")]
    trading_date: Option<String>,
    #[serde(rename = "zqdm")]
    instrument_code: Option<String>,
    #[serde(rename = "zqjc")]
    instrument_name: Option<String>,
    #[serde(rename = "plyy")]
    reason: Option<String>,
    #[serde(rename = "bz")]
    detail_link: Option<String>,
}

fn parse_szse_detail_link(
    html: &str,
    request: &OfficialDragonTigerRequest,
) -> Result<SzseDragonTigerDetailKey, DragonTigerParseError> {
    let marker = "a-param='";
    if html.matches(marker).count() != 1 {
        return Err(DragonTigerParseError::Schema(
            "SZSE detail link must contain exactly one a-param".into(),
        ));
    }
    let value = html
        .split_once(marker)
        .and_then(|(_, suffix)| suffix.split_once('\'').map(|(value, _)| value))
        .ok_or_else(|| DragonTigerParseError::Schema("SZSE detail a-param is malformed".into()))?;
    if !value.starts_with("/ShowReport/data?") {
        return Err(DragonTigerParseError::Schema(
            "SZSE detail a-param path changed".into(),
        ));
    }
    let url = Url::parse(&format!("https://www.szse.cn/api/report{value}"))
        .map_err(|error| DragonTigerParseError::Schema(error.to_string()))?;
    if url.scheme() != "https"
        || url.host_str() != Some("www.szse.cn")
        || url.path() != "/api/report/ShowReport/data"
    {
        return Err(DragonTigerParseError::Schema(
            "SZSE detail link is not the official report path".into(),
        ));
    }
    let query = strict_query_map(&url)?;
    require_query(&query, "SHOWTYPE", "JSON")?;
    require_query(&query, "CATALOGID", SZSE_DETAIL_CATALOG)?;
    require_query(&query, "TABKEY", "tab1,tab2")?;
    require_query(&query, "DQRQ", request.trading_date.as_str())?;
    require_query(&query, "ZQDM", request.instrument.code())?;
    let indicator_code = query
        .get("ZBDM")
        .ok_or_else(|| DragonTigerParseError::Schema("SZSE ZBDM is missing".into()))?;
    if query.len() != 6 {
        return Err(DragonTigerParseError::Schema(
            "SZSE detail link contains unexpected query fields".into(),
        ));
    }
    SzseDragonTigerDetailKey::new(
        request.trading_date.clone(),
        request.instrument.code(),
        indicator_code,
    )
}

fn strict_query_map(url: &Url) -> Result<HashMap<String, String>, DragonTigerParseError> {
    let mut result = HashMap::new();
    for (name, value) in url.query_pairs() {
        if result
            .insert(name.into_owned(), value.into_owned())
            .is_some()
        {
            return Err(DragonTigerParseError::Schema(
                "official URL contains a duplicate query field".into(),
            ));
        }
    }
    Ok(result)
}

fn require_query(
    query: &HashMap<String, String>,
    name: &str,
    expected: &str,
) -> Result<(), DragonTigerParseError> {
    if query.get(name).map(String::as_str) != Some(expected) {
        return Err(DragonTigerParseError::Schema(format!(
            "SZSE detail query {name} does not equal {expected}"
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct SzseDetailReport {
    metadata: SzseDetailMetadata,
    data: Vec<Value>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct SzseDetailMetadata {
    #[serde(rename = "catalogid")]
    catalog_id: Option<String>,
    #[serde(rename = "tabkey")]
    tab_key: Option<String>,
    #[serde(default)]
    conditions: Vec<SzseCondition>,
    #[serde(rename = "cols", default)]
    columns: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct SzseSummaryRow {
    #[serde(rename = "dqrq")]
    trading_date: Option<String>,
    #[serde(rename = "zqjc")]
    instrument_name_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SzseSeatRow {
    #[serde(rename = "mmlb")]
    side_rank: Option<String>,
    #[serde(rename = "zsmc")]
    seat_name: Option<String>,
    #[serde(rename = "mrje")]
    buy_amount: Option<String>,
    #[serde(rename = "mcje")]
    sell_amount: Option<String>,
}

fn validate_detail_conditions(
    conditions: &[SzseCondition],
    expected: &SzseDragonTigerDetailKey,
) -> Result<(), DragonTigerParseError> {
    validate_condition(conditions, "DQRQ", expected.trading_date.as_str())?;
    validate_condition(conditions, "ZQDM", &expected.instrument_code)?;
    validate_condition(conditions, "ZBDM", &expected.indicator_code)
}

fn validate_szse_summary(
    rows: &[Value],
    expected: &SzseDragonTigerDetailKey,
) -> Result<(), DragonTigerParseError> {
    if rows.len() != 1 {
        return Err(DragonTigerParseError::Incomplete(
            "SZSE detail summary must contain exactly one row".into(),
        ));
    }
    let row: SzseSummaryRow = serde_json::from_value(rows[0].clone())
        .map_err(|error| DragonTigerParseError::Decode(error.to_string()))?;
    let source_date = required_text(row.trading_date, "SZSE detail dqrq")?;
    if source_date != expected.trading_date.as_str() {
        return Err(DragonTigerParseError::Schema(format!(
            "SZSE detail date {source_date} does not match requested {}",
            expected.trading_date.as_str()
        )));
    }
    let name_code = required_text(row.instrument_name_code, "SZSE detail zqjc")?;
    let suffix = format!("&nbsp;({})", expected.instrument_code);
    let name = name_code.strip_suffix(&suffix).ok_or_else(|| {
        DragonTigerParseError::Schema(
            "SZSE detail zqjc does not end with the exact requested code".into(),
        )
    })?;
    if name.trim().is_empty() {
        return Err(DragonTigerParseError::Schema(
            "SZSE detail instrument name is empty".into(),
        ));
    }
    Ok(())
}

fn map_szse_seats(
    rows: &[Value],
    expected: &SzseDragonTigerDetailKey,
    observed_at: &str,
    batch_id: &str,
) -> Result<Vec<DragonTigerSeat>, DragonTigerParseError> {
    if rows.len() != 10 {
        return Err(DragonTigerParseError::Incomplete(format!(
            "SZSE detail has {} seat rows; exactly ten are required",
            rows.len()
        )));
    }
    let mut mapped = Vec::with_capacity(rows.len());
    let mut seen = HashSet::new();
    for value in rows {
        let row: SzseSeatRow = serde_json::from_value(value.clone())
            .map_err(|error| DragonTigerParseError::Decode(error.to_string()))?;
        let side_rank = required_text(row.side_rank, "SZSE detail mmlb")?;
        let (side, rank_text) = if let Some(rank) = side_rank.strip_prefix('买') {
            (DragonTigerSide::Buy, rank)
        } else if let Some(rank) = side_rank.strip_prefix('卖') {
            (DragonTigerSide::Sell, rank)
        } else {
            return Err(DragonTigerParseError::Schema(format!(
                "SZSE side/rank {side_rank:?} is not 买1..买5 or 卖1..卖5"
            )));
        };
        let rank = rank_text
            .parse::<u32>()
            .map_err(|_| DragonTigerParseError::Schema("SZSE rank is not numeric".into()))?;
        let side_key = match side {
            DragonTigerSide::Buy => 0_u8,
            DragonTigerSide::Sell => 1_u8,
        };
        if !(1..=5).contains(&rank) || !seen.insert((side_key, rank)) {
            return Err(DragonTigerParseError::Schema(format!(
                "SZSE side/rank {side_rank:?} is duplicate or out of range"
            )));
        }
        let buy_value = parse_yuan_number(
            &required_text(row.buy_amount, "SZSE detail mrje")?,
            "SZSE buy amount",
        )?;
        let sell_value = parse_yuan_number(
            &required_text(row.sell_amount, "SZSE detail mcje")?,
            "SZSE sell amount",
        )?;
        let buy_amount = Money::new(buy_value)?;
        let sell_amount = Money::new(sell_value)?;
        mapped.push(DragonTigerSeat::new(
            NonEmptyText::new(expected.entry_id())?,
            InstrumentId::new(
                Exchange::Shenzhen,
                expected.instrument_code.clone(),
                AssetClass::Equity,
            )?,
            expected.trading_date.clone(),
            side,
            PositiveU32::new(rank)?,
            NonEmptyText::new(required_text(row.seat_name, "SZSE detail zsmc")?)?,
            match side {
                DragonTigerSide::Buy => buy_amount,
                DragonTigerSide::Sell => sell_amount,
            },
            Some(buy_amount),
            Some(sell_amount),
            Some(Money::new(buy_value - sell_value)?),
            evidence(
                ProviderId::Szse,
                expected.trading_date.as_str(),
                observed_at,
                batch_id,
            )?,
        )?);
    }
    for side in [DragonTigerSide::Buy, DragonTigerSide::Sell] {
        let side_key = match side {
            DragonTigerSide::Buy => 0_u8,
            DragonTigerSide::Sell => 1_u8,
        };
        for rank in 1..=5 {
            if !seen.contains(&(side_key, rank)) {
                return Err(DragonTigerParseError::Incomplete(
                    "SZSE detail does not contain complete ranks one through five".into(),
                ));
            }
        }
    }
    mapped.sort_by_key(|seat| {
        (
            match seat.side() {
                DragonTigerSide::Buy => 0_u8,
                DragonTigerSide::Sell => 1_u8,
            },
            seat.rank().get(),
        )
    });
    Ok(mapped)
}

fn validate_condition(
    conditions: &[SzseCondition],
    name: &str,
    expected: &str,
) -> Result<(), DragonTigerParseError> {
    let matches = conditions
        .iter()
        .filter(|condition| condition.name.as_deref() == Some(name))
        .collect::<Vec<_>>();
    if matches.len() != 1 || matches[0].default_value.as_deref() != Some(expected) {
        return Err(DragonTigerParseError::Schema(format!(
            "SZSE condition {name} does not exactly equal {expected}"
        )));
    }
    Ok(())
}

fn validate_list_columns(columns: &HashMap<String, String>) -> Result<(), DragonTigerParseError> {
    for (name, expected) in [
        ("dqrq", "公告日期"),
        ("zqdm", "证券代码"),
        ("zqjc", "证券简称"),
        ("cjje", "成交金额<br>(亿元)"),
        ("cjsl", "成交量<br>(万股/万份)"),
        ("plyy", "披露原因"),
        ("bz", "备注"),
    ] {
        require_column(columns, name, expected)?;
    }
    Ok(())
}

fn validate_summary_columns(
    columns: &HashMap<String, String>,
) -> Result<(), DragonTigerParseError> {
    for (name, expected) in [
        ("dqrq", "公告日期"),
        ("zqjc", "[none]"),
        ("cjsl", "成 交 量"),
        ("cjje", "成交金额"),
        ("plyy", "[none]"),
    ] {
        require_column(columns, name, expected)?;
    }
    Ok(())
}

fn validate_seat_columns(columns: &HashMap<String, String>) -> Result<(), DragonTigerParseError> {
    for (name, expected) in [
        ("mmlb", "买/卖"),
        ("zsmc", "会员营业部名称"),
        ("mrje", "买入金额<br>（元）"),
        ("mcje", "卖出金额<br>（元）"),
    ] {
        require_column(columns, name, expected)?;
    }
    Ok(())
}

fn require_column(
    columns: &HashMap<String, String>,
    name: &str,
    expected: &str,
) -> Result<(), DragonTigerParseError> {
    if columns.get(name).map(String::as_str) != Some(expected) {
        return Err(DragonTigerParseError::Schema(format!(
            "SZSE column {name} changed from {expected:?}"
        )));
    }
    Ok(())
}

fn require_equal(
    actual: Option<&str>,
    expected: &str,
    field: &str,
) -> Result<(), DragonTigerParseError> {
    if actual != Some(expected) {
        return Err(DragonTigerParseError::Schema(format!(
            "{field} does not equal {expected}"
        )));
    }
    Ok(())
}

fn required_u32(value: Option<i64>, field: &str) -> Result<u32, DragonTigerParseError> {
    let value =
        value.ok_or_else(|| DragonTigerParseError::Schema(format!("{field} is missing")))?;
    u32::try_from(value)
        .map_err(|_| DragonTigerParseError::Schema(format!("{field} is outside u32 range")))
}

fn required_text(value: Option<String>, field: &str) -> Result<String, DragonTigerParseError> {
    let value =
        value.ok_or_else(|| DragonTigerParseError::Schema(format!("{field} is missing")))?;
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return Err(DragonTigerParseError::Schema(format!(
            "{field} is empty or contains control characters"
        )));
    }
    Ok(trimmed.to_owned())
}

fn parse_yuan_number(value: &str, field: &str) -> Result<f64, DragonTigerParseError> {
    let mut decimal_parts = value.split('.');
    let integer = decimal_parts.next().unwrap_or_default();
    let fraction = decimal_parts.next();
    if decimal_parts.next().is_some()
        || integer.is_empty()
        || fraction.is_some_and(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()))
        || !valid_grouped_integer(integer)
    {
        return Err(DragonTigerParseError::Schema(format!(
            "{field} is not a plain non-negative yuan number"
        )));
    }
    let compact = value.replace(',', "");
    let amount = compact.parse::<f64>().map_err(|_| {
        DragonTigerParseError::Schema(format!("{field} cannot be parsed as a finite number"))
    })?;
    if !amount.is_finite() || amount < 0.0 {
        return Err(DragonTigerParseError::Schema(format!(
            "{field} is not finite and non-negative"
        )));
    }
    Ok(amount)
}

fn valid_grouped_integer(value: &str) -> bool {
    if !value.contains(',') {
        return value.bytes().all(|byte| byte.is_ascii_digit());
    }
    let groups = value.split(',').collect::<Vec<_>>();
    let Some(first) = groups.first() else {
        return false;
    };
    (1..=3).contains(&first.len())
        && first.bytes().all(|byte| byte.is_ascii_digit())
        && groups.len() > 1
        && groups[1..]
            .iter()
            .all(|group| group.len() == 3 && group.bytes().all(|byte| byte.is_ascii_digit()))
}

fn evidence(
    provider: ProviderId,
    source_at: &str,
    observed_at: &str,
    batch_id: &str,
) -> Result<SourceEvidence, DragonTigerParseError> {
    Ok(SourceEvidence::new(provider, observed_at, batch_id)?.with_source_at(source_at)?)
}

fn validate_body_size(body: &[u8]) -> Result<(), DragonTigerParseError> {
    if body.len() > MAX_DRAGON_TIGER_RESPONSE_BYTES {
        return Err(DragonTigerParseError::Incomplete(format!(
            "official response exceeds {MAX_DRAGON_TIGER_RESPONSE_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_equity_identity(instrument: &InstrumentId) -> Result<(), DragonTigerParseError> {
    if instrument.asset_class() != AssetClass::Equity || !is_six_ascii_digits(instrument.code()) {
        return Err(DragonTigerParseError::InvalidRequest(
            "official dragon-tiger request requires a six-digit equity code".into(),
        ));
    }
    let first = instrument.code().as_bytes().first().copied();
    let valid = match instrument.exchange() {
        Exchange::Shanghai => first == Some(b'6'),
        Exchange::Shenzhen => matches!(first, Some(b'0' | b'3')),
        Exchange::Beijing => false,
    };
    if !valid {
        return Err(DragonTigerParseError::InvalidRequest(
            "instrument code prefix does not match SSE/SZSE venue".into(),
        ));
    }
    Ok(())
}

fn is_six_ascii_digits(value: &str) -> bool {
    value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_digit())
}
