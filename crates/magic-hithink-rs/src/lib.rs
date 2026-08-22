//! Bounded client for the official HITHINK Fuyao Financial API.
//!
//! The provider deliberately exposes only source contracts that can be mapped
//! without inventing values or record timestamps.

use magic_market_core::{
    Adjustment, AssetClass, Bar, BarInterval, BarsRequest, DataBatch, Exchange, FiniteNumber,
    HistoricalBars, InstrumentId, IsoDate, LimitPoolEntry, LimitPoolKind, LimitPoolRequest,
    LimitPools, LoadProbeSnapshot, MarketStatistics, MarketStatisticsProvider, Money, NonEmptyText,
    PopularityData, PopularityRank, PositiveU32, Price, ProbeRequestTracker, Provenance,
    ProviderId, Quantity, Ratio, RatioUnit, SourceEvidence,
};
use magic_market_transport::{
    EndpointPolicy, HttpMethod, HttpRequest, HttpResponse, HttpTransport, MediaType, RequestGate,
    ReqwestTransport, TransportError,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};
use url::Url;

const BASE_URL: &str = "https://fuyao.aicubes.cn";
const HOST: &str = "fuyao.aicubes.cn";
const HISTORICAL_PATH: &str = "/api/a-share/prices/historical";
const VALUATIONS_PATH: &str = "/api/a-share/valuations/snapshot";
const LIMIT_UP_PATH: &str = "/api/a-share/special-data/limit-up-pool";
const LIMIT_DOWN_PATH: &str = "/api/a-share/special-data/limit-down-pool";
const LIMIT_BREAK_PATH: &str = "/api/a-share/special-data/limit-break-pool";
const HOT_STOCK_PATH: &str = "/api/a-share/special-data/hot-stock-list";
const EXACT_PATHS: [&str; 6] = [
    HISTORICAL_PATH,
    VALUATIONS_PATH,
    LIMIT_UP_PATH,
    LIMIT_DOWN_PATH,
    LIMIT_BREAK_PATH,
    HOT_STOCK_PATH,
];
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_INTERVAL: Duration = Duration::from_millis(500);
const TEST_REQUEST_INTERVAL: Duration = Duration::from_millis(1);
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_HISTORICAL_ROWS: usize = 3_000;
const MAX_LIMIT_POOL_PAGES: u32 = 5;
const LIMIT_POOL_PAGE_SIZE: u32 = 200;
const MAX_POPULARITY_ROWS: usize = 500;
const MAX_POPULARITY_LIMIT: u32 = 100;
const MAX_VALUATION_INSTRUMENTS: usize = 100;
const MAX_TEN_YEAR_DAYS: i64 = 3_653;

/// Production admission is enabled only after deterministic, live, and serial probes pass.
pub const HISTORICAL_BARS_ADMITTED: bool = true;
/// Production admission is enabled only after deterministic, live, and serial probes pass.
pub const MARKET_STATISTICS_ADMITTED: bool = true;
/// Production admission is enabled only after deterministic, live, and serial probes pass.
pub const LIMIT_POOLS_ADMITTED: bool = true;
/// Production admission is enabled only after deterministic, live, and serial probes pass.
pub const POPULARITY_ADMITTED: bool = true;

#[derive(Debug, Error)]
pub enum HithinkError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HITHINK authentication failed with code {code}; request_id={request_id}")]
    Authentication { code: i64, request_id: String },
    #[error("HITHINK rate limit reached; request_id={request_id}")]
    RateLimited { request_id: String },
    #[error("HITHINK business failure code {code}; request_id={request_id}")]
    Business { code: i64, request_id: String },
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("HITHINK response decoding failed: {0}")]
    Decode(String),
    #[error("HITHINK protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Clone)]
pub struct HithinkClient {
    api_key: Arc<str>,
    transport: Arc<dyn HttpTransport>,
    policy: EndpointPolicy,
    gate: Arc<RequestGate>,
    request_probe: Arc<Mutex<ProbeRequestTracker>>,
}

impl std::fmt::Debug for HithinkClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HithinkClient")
            .field("api_key", &"[REDACTED]")
            .field("base_url", &BASE_URL)
            .finish_non_exhaustive()
    }
}

impl HithinkClient {
    /// Reads the official Fuyao API key from `HITHINK_FINANCE_API_KEY`.
    pub fn from_env() -> Result<Self, HithinkError> {
        let api_key = std::env::var("HITHINK_FINANCE_API_KEY").map_err(|_| {
            HithinkError::InvalidRequest("set HITHINK_FINANCE_API_KEY to an authorized key".into())
        })?;
        Self::new(api_key)
    }

    pub fn new(api_key: impl Into<String>) -> Result<Self, HithinkError> {
        Self::with_timeout(api_key, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        api_key: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, HithinkError> {
        let api_key = validate_api_key(api_key.into())?;
        let policy = endpoint_policy(timeout)?;
        let transport = Arc::new(ReqwestTransport::new(policy.clone())?);
        Self::from_parts(Arc::from(api_key), transport, policy, REQUEST_INTERVAL)
    }

    pub fn with_transport(
        api_key: impl Into<String>,
        transport: impl HttpTransport + 'static,
    ) -> Result<Self, HithinkError> {
        let api_key = validate_api_key(api_key.into())?;
        let policy = endpoint_policy(DEFAULT_TIMEOUT)?;
        Self::from_parts(
            Arc::from(api_key),
            Arc::new(transport),
            policy,
            TEST_REQUEST_INTERVAL,
        )
    }

    fn from_parts(
        api_key: Arc<str>,
        transport: Arc<dyn HttpTransport>,
        policy: EndpointPolicy,
        interval: Duration,
    ) -> Result<Self, HithinkError> {
        Ok(Self {
            api_key,
            transport,
            policy,
            gate: Arc::new(RequestGate::new(interval)?),
            request_probe: Arc::new(Mutex::new(ProbeRequestTracker::default())),
        })
    }

    pub fn load_probe_snapshot(&self) -> Result<LoadProbeSnapshot, HithinkError> {
        self.request_probe
            .lock()
            .map(|probe| probe.snapshot())
            .map_err(|_| tracker_error("request tracker lock poisoned"))
    }

    /// Diagnostic path used before the capability is promoted into routing.
    pub fn probe_historical_bars(
        &self,
        request: &BarsRequest,
    ) -> Result<DataBatch<Bar>, HithinkError> {
        validate_historical_request(request)?;
        let start = request.start().ok_or_else(|| {
            HithinkError::InvalidRequest("explicit start date is required".into())
        })?;
        let end = request
            .end()
            .ok_or_else(|| HithinkError::InvalidRequest("explicit end date is required".into()))?;
        let start_date = parse_date(start)?;
        let end_date = parse_date(end)?;
        let thscode = instrument_to_thscode(request.instrument())?;
        let query = [
            ("thscode", thscode),
            ("interval", "1d".to_owned()),
            (
                "start",
                shanghai_millis(start_date, Time::MIDNIGHT)?.to_string(),
            ),
            (
                "end",
                shanghai_millis(
                    end_date,
                    Time::from_hms_milli(23, 59, 59, 999)
                        .map_err(|error| HithinkError::InvalidRequest(error.to_string()))?,
                )?
                .to_string(),
            ),
            ("adjust", "none".to_owned()),
            ("offset", "0".to_owned()),
        ];
        let response: Success<HistoricalData> =
            self.get(HISTORICAL_PATH, query.iter().map(pair_ref))?;
        normalize_historical(request, response, start_date, end_date)
    }

    /// Diagnostic path used before the capability is promoted into routing.
    pub fn probe_market_statistics(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<MarketStatistics>, HithinkError> {
        if instruments.is_empty() || instruments.len() > MAX_VALUATION_INSTRUMENTS {
            return Err(HithinkError::InvalidRequest(format!(
                "valuation request must contain 1..={MAX_VALUATION_INSTRUMENTS} instruments"
            )));
        }
        let mut seen = HashSet::with_capacity(instruments.len());
        let mut thscodes = Vec::with_capacity(instruments.len());
        for instrument in instruments {
            let thscode = instrument_to_thscode(instrument)?;
            if !seen.insert(thscode.clone()) {
                return Err(HithinkError::InvalidRequest(
                    "valuation instruments must be unique".into(),
                ));
            }
            thscodes.push(thscode);
        }
        let joined = thscodes.join(",");
        let response: Success<ValuationData> =
            self.get(VALUATIONS_PATH, [("thscodes", joined)].iter().map(pair_ref))?;
        normalize_valuations(instruments, &thscodes, response)
    }

    /// Diagnostic path used before the capability is promoted into routing.
    pub fn probe_limit_pool(
        &self,
        request: &LimitPoolRequest,
    ) -> Result<DataBatch<LimitPoolEntry>, HithinkError> {
        let (path, sort_field) = match request.kind() {
            LimitPoolKind::Upper => (LIMIT_UP_PATH, "limit_up_time"),
            LimitPoolKind::Lower => (LIMIT_DOWN_PATH, "last_limit_time"),
            LimitPoolKind::Broken => (LIMIT_BREAK_PATH, "price_change_ratio_pct"),
            LimitPoolKind::PreviousUpper => {
                return Err(HithinkError::Unsupported(
                    "Fuyao does not expose an exact previous-upper pool".into(),
                ));
            }
        };
        let date = parse_date(request.trading_date().as_str())?;
        let date_ms = shanghai_millis(date, Time::MIDNIGHT)?;
        let observed_at = now()?;
        let mut pages = Vec::new();
        let mut expected: Option<PaginationContract> = None;
        for page in 1..=MAX_LIMIT_POOL_PAGES {
            let query = [
                ("date_ms", date_ms.to_string()),
                ("page", page.to_string()),
                ("size", LIMIT_POOL_PAGE_SIZE.to_string()),
                ("sort_field", sort_field.to_owned()),
                ("sort_dir", "desc".to_owned()),
            ];
            let response: Success<LimitPoolData> = self.get(path, query.iter().map(pair_ref))?;
            validate_pagination(page, &response.data, &mut expected)?;
            let last = response.data.pagination.pages <= page;
            pages.push(response);
            if last {
                break;
            }
        }
        let expected =
            expected.ok_or_else(|| HithinkError::Protocol("missing pagination".into()))?;
        if expected.pages > MAX_LIMIT_POOL_PAGES {
            return Err(HithinkError::Protocol(format!(
                "limit pool requires {} pages, exceeding bound {MAX_LIMIT_POOL_PAGES}",
                expected.pages
            )));
        }
        normalize_limit_pool(request, &observed_at, pages, expected)
    }

    /// Diagnostic path used before the capability is promoted into routing.
    pub fn probe_popularity(
        &self,
        limit: PositiveU32,
    ) -> Result<DataBatch<PopularityRank>, HithinkError> {
        if limit.get() > MAX_POPULARITY_LIMIT {
            return Err(HithinkError::InvalidRequest(format!(
                "popularity limit must be at most {MAX_POPULARITY_LIMIT}"
            )));
        }
        let response: Success<PopularityDataResponse> = self.get(
            HOT_STOCK_PATH,
            [("period", "day".to_owned())].iter().map(pair_ref),
        )?;
        normalize_popularity(limit, response)
    }

    fn get<'a, T: DeserializeOwned>(
        &self,
        path: &str,
        query: impl Iterator<Item = (&'a str, &'a str)>,
    ) -> Result<Success<T>, HithinkError> {
        if !EXACT_PATHS.contains(&path) {
            return Err(HithinkError::InvalidRequest(
                "endpoint is not in the closed HITHINK contract".into(),
            ));
        }
        let mut url = Url::parse(BASE_URL)
            .map_err(|_| HithinkError::InvalidRequest("invalid fixed base URL".into()))?;
        url.set_path(path);
        url.query_pairs_mut().extend_pairs(query);
        let request = HttpRequest::new(
            HttpMethod::Get,
            url.to_string(),
            vec![
                ("Accept".into(), "application/json".into()),
                ("X-api-key".into(), self.api_key.to_string()),
                ("User-Agent".into(), "magic-hithink-rs/0.2".into()),
            ],
            Vec::new(),
        )?;
        self.execute_json(&request)
    }

    fn execute_json<T: DeserializeOwned>(
        &self,
        request: &HttpRequest,
    ) -> Result<Success<T>, HithinkError> {
        ensure_exact_endpoint(request.url())?;
        self.policy.validate_request(request)?;
        self.gate.wait_for_turn()?;
        self.request_probe
            .lock()
            .map_err(|_| tracker_error("request tracker lock poisoned"))?
            .request_started();
        let response = self.transport.execute(request);
        self.request_probe
            .lock()
            .map_err(|_| tracker_error("request tracker lock poisoned"))?
            .request_finished()
            .map_err(|error| tracker_error(&error.to_string()))?;
        let response = self.policy.validate_response_for(request, response?)?;
        parse_envelope(response)
    }
}

impl HistoricalBars for HithinkClient {
    type Bar = Bar;
    type Error = HithinkError;

    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Bar>, Self::Error> {
        if HISTORICAL_BARS_ADMITTED {
            self.probe_historical_bars(request)
        } else {
            Err(HithinkError::Unsupported(
                "HITHINK historical bars await production admission".into(),
            ))
        }
    }
}

impl MarketStatisticsProvider for HithinkClient {
    type Error = HithinkError;

    fn market_statistics(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<MarketStatistics>, Self::Error> {
        if MARKET_STATISTICS_ADMITTED {
            self.probe_market_statistics(instruments)
        } else {
            Err(HithinkError::Unsupported(
                "HITHINK market statistics await production admission".into(),
            ))
        }
    }
}

impl LimitPools for HithinkClient {
    type Error = HithinkError;

    fn limit_pool(
        &self,
        request: &LimitPoolRequest,
    ) -> Result<DataBatch<LimitPoolEntry>, Self::Error> {
        if LIMIT_POOLS_ADMITTED {
            self.probe_limit_pool(request)
        } else {
            Err(HithinkError::Unsupported(
                "HITHINK limit pools await production admission".into(),
            ))
        }
    }
}

impl PopularityData for HithinkClient {
    type Error = HithinkError;

    fn popularity(&self, limit: PositiveU32) -> Result<DataBatch<PopularityRank>, Self::Error> {
        if POPULARITY_ADMITTED {
            self.probe_popularity(limit)
        } else {
            Err(HithinkError::Unsupported(
                "HITHINK popularity awaits production admission".into(),
            ))
        }
    }
}

#[derive(Debug)]
struct Success<T> {
    request_id: String,
    data: T,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    code: i64,
    message: String,
    request_id: String,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalData {
    thscode: String,
    interval: String,
    adjust: String,
    timestamp: i64,
    item: Vec<HistoricalItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalItem {
    date_ms: i64,
    open_price: f64,
    high_price: f64,
    low_price: f64,
    close_price: f64,
    volume: f64,
    turnover: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValuationData {
    timestamp: Option<i64>,
    total: usize,
    item: Vec<ValuationItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValuationItem {
    thscode: String,
    ticker: String,
    name: Option<String>,
    pe_ttm: Option<f64>,
    pe_mrq: Option<f64>,
    pb_mrq: Option<f64>,
    ps_ttm: Option<f64>,
    pcf_ttm: Option<f64>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pagination {
    total: u32,
    pages: u32,
    size: u32,
    page: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitPoolData {
    timestamp: i64,
    pagination: Pagination,
    item: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitUpItem {
    thscode: String,
    ticker: String,
    name: String,
    is_st: bool,
    is_new: bool,
    last_price: f64,
    price_change_ratio_pct: f64,
    limit_up_time: String,
    limit_up_reason: String,
    continue_day_text: String,
    continue_day_cnt: u32,
    seal_money: f64,
    max_seal_money: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitDownItem {
    thscode: String,
    ticker: String,
    name: String,
    last_price: f64,
    price_change_ratio_pct: f64,
    first_limit_time: String,
    last_limit_time: String,
    turnover_ratio_pct: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitBreakItem {
    thscode: String,
    ticker: String,
    name: String,
    last_price: f64,
    price_change_ratio_pct: f64,
    open_times: u32,
    turnover_ratio_pct: f64,
    turnover: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PopularityDataResponse {
    timestamp: i64,
    item: Vec<PopularityItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PopularityItem {
    thscode: String,
    ticker: String,
    name: String,
    rank: u32,
    #[serde(deserialize_with = "finite_number_or_string")]
    heat: f64,
    rank_change: i32,
    rank_trend: String,
}

#[derive(Debug, Clone, Copy)]
struct PaginationContract {
    timestamp: i64,
    total: u32,
    pages: u32,
    size: u32,
}

fn endpoint_policy(timeout: Duration) -> Result<EndpointPolicy, HithinkError> {
    Ok(EndpointPolicy::new(
        HOST,
        EXACT_PATHS.iter().map(|path| (*path).to_owned()).collect(),
        [
            "thscode",
            "thscodes",
            "interval",
            "start",
            "end",
            "adjust",
            "offset",
            "date_ms",
            "page",
            "size",
            "sort_field",
            "sort_dir",
            "period",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        vec![MediaType::Json],
        MAX_RESPONSE_BYTES,
        timeout,
    )?)
}

fn pair_ref(pair: &(impl AsRef<str>, String)) -> (&str, &str) {
    (pair.0.as_ref(), pair.1.as_str())
}

fn ensure_exact_endpoint(url: &str) -> Result<(), HithinkError> {
    let parsed = Url::parse(url)
        .map_err(|_| HithinkError::InvalidRequest("request URL is invalid".into()))?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some(HOST)
        || parsed.port().is_some()
        || !EXACT_PATHS.contains(&parsed.path())
    {
        return Err(HithinkError::InvalidRequest(
            "request URL is outside the closed HITHINK endpoint contract".into(),
        ));
    }
    Ok(())
}

fn validate_api_key(value: String) -> Result<String, HithinkError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 1_024 || trimmed.chars().any(char::is_control) {
        return Err(HithinkError::InvalidRequest(
            "HITHINK API key is empty or malformed".into(),
        ));
    }
    Ok(trimmed.to_owned())
}

fn parse_envelope<T: DeserializeOwned>(response: HttpResponse) -> Result<Success<T>, HithinkError> {
    let envelope: Envelope<T> = serde_json::from_slice(response.body())
        .map_err(|error| HithinkError::Decode(error.to_string()))?;
    validate_safe_text("request_id", &envelope.request_id)?;
    validate_safe_text("message", &envelope.message)?;
    if envelope.code == 0 {
        let data = envelope
            .data
            .ok_or_else(|| HithinkError::Protocol("successful envelope has null data".into()))?;
        return Ok(Success {
            request_id: envelope.request_id,
            data,
        });
    }
    if envelope.data.is_some() {
        return Err(HithinkError::Protocol(
            "failed envelope must have null data".into(),
        ));
    }
    match envelope.code {
        2001 | 2003 => Err(HithinkError::Authentication {
            code: envelope.code,
            request_id: envelope.request_id,
        }),
        4001 => Err(HithinkError::RateLimited {
            request_id: envelope.request_id,
        }),
        code => Err(HithinkError::Business {
            code,
            request_id: envelope.request_id,
        }),
    }
}

fn validate_safe_text(field: &str, value: &str) -> Result<(), HithinkError> {
    if value.trim().is_empty() || value.len() > 1_024 || value.chars().any(char::is_control) {
        return Err(HithinkError::Protocol(format!(
            "{field} is empty or malformed"
        )));
    }
    Ok(())
}

fn validate_historical_request(request: &BarsRequest) -> Result<(), HithinkError> {
    if request.interval() != BarInterval::Day {
        return Err(HithinkError::Unsupported(
            "Fuyao historical prices support only Day bars".into(),
        ));
    }
    let start = request
        .start()
        .ok_or_else(|| HithinkError::InvalidRequest("explicit start date is required".into()))?;
    let end = request
        .end()
        .ok_or_else(|| HithinkError::InvalidRequest("explicit end date is required".into()))?;
    let days = (parse_date(end)? - parse_date(start)?).whole_days();
    if days > MAX_TEN_YEAR_DAYS {
        return Err(HithinkError::InvalidRequest(
            "historical date range must not exceed ten years".into(),
        ));
    }
    instrument_to_thscode(request.instrument())?;
    Ok(())
}

fn normalize_historical(
    request: &BarsRequest,
    response: Success<HistoricalData>,
    start: Date,
    end: Date,
) -> Result<DataBatch<Bar>, HithinkError> {
    let expected_thscode = instrument_to_thscode(request.instrument())?;
    if response.data.thscode != expected_thscode
        || response.data.interval != "1d"
        || response.data.adjust != "none"
    {
        return Err(HithinkError::Protocol(
            "historical response context contradicts the request".into(),
        ));
    }
    if response.data.timestamp <= 0 {
        return Err(HithinkError::Protocol(
            "historical timestamp must be positive".into(),
        ));
    }
    if response.data.item.len() > MAX_HISTORICAL_ROWS {
        return Err(HithinkError::Protocol(format!(
            "historical result exceeds {MAX_HISTORICAL_ROWS} rows"
        )));
    }
    let observed_at = now()?;
    let batch_id = response.request_id;
    let mut seen = HashSet::with_capacity(response.data.item.len());
    let mut rows = Vec::with_capacity(response.data.item.len());
    for item in response.data.item {
        let date = shanghai_midnight_date(item.date_ms, "historical date_ms")?;
        if date < start || date > end || !seen.insert(date) {
            return Err(HithinkError::Protocol(
                "historical rows are duplicated or outside the requested range".into(),
            ));
        }
        let date_text = date.to_string();
        let mut bar = Bar::new(
            request.instrument().clone(),
            BarInterval::Day,
            date_text.clone(),
            date_text.clone(),
            Price::new(item.open_price)?,
            Price::new(item.high_price)?,
            Price::new(item.low_price)?,
            Price::new(item.close_price)?,
            Quantity::new(shares_to_lots(item.volume)?)?,
            Some(nonnegative_money(item.turnover, "historical turnover")?),
            Adjustment::Unadjusted,
            ProviderId::Tonghuashun,
            batch_id.clone(),
        )?;
        bar = bar
            .with_source_at(date_text)?
            .with_observed_at(observed_at.clone())?;
        rows.push(bar);
    }
    rows.sort_by(|left, right| left.bar_start().cmp(right.bar_start()));
    if let Some(latest) = rows.last() {
        let upstream_latest = shanghai_date(response.data.timestamp, "historical timestamp")?;
        if upstream_latest.to_string() != latest.bar_start() {
            return Err(HithinkError::Protocol(
                "historical timestamp does not identify the latest returned bar".into(),
            ));
        }
    }
    let keep = usize::from(request.limit());
    if rows.len() > keep {
        rows.drain(..rows.len() - keep);
    }
    let provenance = batch_provenance(
        source_millis(response.data.timestamp)?,
        observed_at,
        batch_id,
    )?;
    Ok(DataBatch::strict(rows, provenance))
}

fn normalize_valuations(
    instruments: &[InstrumentId],
    thscodes: &[String],
    response: Success<ValuationData>,
) -> Result<DataBatch<MarketStatistics>, HithinkError> {
    if response.data.total != instruments.len() || response.data.item.len() != instruments.len() {
        return Err(HithinkError::Protocol(
            "valuation response does not contain exactly one row per requested instrument".into(),
        ));
    }
    let observed_at = now()?;
    let batch_id = response.request_id;
    let mut records = Vec::with_capacity(instruments.len());
    for ((instrument, expected), item) in instruments.iter().zip(thscodes).zip(response.data.item) {
        validate_identity(expected, instrument.code(), &item.thscode, &item.ticker)?;
        if let Some(name) = item.name.as_deref() {
            validate_safe_text("valuation name", name)?;
        }
        // These fields are deliberately validated even though frozen v1 has no slots for them.
        let _ps_ttm = optional_finite(item.ps_ttm)?;
        let _pcf_ttm = optional_finite(item.pcf_ttm)?;
        let evidence = SourceEvidence::new(
            ProviderId::Tonghuashun,
            observed_at.clone(),
            batch_id.clone(),
        )?;
        records.push(MarketStatistics::new(
            instrument.clone(),
            None,
            optional_finite(item.pe_ttm)?,
            optional_finite(item.pe_mrq)?,
            optional_finite(item.pb_mrq)?,
            None,
            None,
            None,
            None,
            None,
            evidence,
        )?);
    }
    let provenance = match response.data.timestamp {
        Some(timestamp) => {
            if timestamp <= 0 {
                return Err(HithinkError::Protocol(
                    "valuation timestamp must be positive when present".into(),
                ));
            }
            batch_provenance(source_millis(timestamp)?, observed_at, batch_id)?
        }
        None => Provenance::new("HithinkFinance", observed_at)?.with_batch_id(batch_id)?,
    };
    Ok(DataBatch::strict(records, provenance))
}

fn validate_pagination(
    requested_page: u32,
    data: &LimitPoolData,
    expected: &mut Option<PaginationContract>,
) -> Result<(), HithinkError> {
    let pagination = data.pagination;
    if pagination.size == 0 {
        return Err(HithinkError::Protocol(
            "limit-pool page size must be positive".into(),
        ));
    }
    let declared_pages = if pagination.total == 0 {
        pagination.pages
    } else {
        pagination.total.div_ceil(pagination.size)
    };
    if data.timestamp <= 0
        || pagination.page != requested_page
        || pagination.size != LIMIT_POOL_PAGE_SIZE
        || pagination.pages > MAX_LIMIT_POOL_PAGES
        || (pagination.total > 0 && pagination.pages != declared_pages)
        || data.item.len() > LIMIT_POOL_PAGE_SIZE as usize
    {
        return Err(HithinkError::Protocol(
            "limit-pool pagination metadata is invalid".into(),
        ));
    }
    if pagination.total == 0 {
        if !data.item.is_empty() || !(pagination.pages == 0 || pagination.pages == 1) {
            return Err(HithinkError::Protocol(
                "empty limit pool has contradictory pagination".into(),
            ));
        }
    } else if pagination.pages == 0 || data.item.is_empty() {
        return Err(HithinkError::Protocol(
            "non-empty limit pool has contradictory pagination".into(),
        ));
    } else {
        let consumed_before = (requested_page - 1)
            .checked_mul(pagination.size)
            .ok_or_else(|| HithinkError::Protocol("pagination arithmetic overflow".into()))?;
        let expected_rows = pagination
            .total
            .saturating_sub(consumed_before)
            .min(pagination.size);
        if data.item.len() != expected_rows as usize {
            return Err(HithinkError::Protocol(
                "limit-pool page cardinality contradicts pagination".into(),
            ));
        }
    }
    let contract = PaginationContract {
        timestamp: data.timestamp,
        total: pagination.total,
        pages: pagination.pages,
        size: pagination.size,
    };
    match expected {
        Some(value)
            if value.timestamp != contract.timestamp
                || value.total != contract.total
                || value.pages != contract.pages
                || value.size != contract.size =>
        {
            Err(HithinkError::Protocol(
                "limit-pool pagination changed during traversal".into(),
            ))
        }
        Some(_) => Ok(()),
        None => {
            *expected = Some(contract);
            Ok(())
        }
    }
}

fn normalize_limit_pool(
    request: &LimitPoolRequest,
    observed_at: &str,
    pages: Vec<Success<LimitPoolData>>,
    contract: PaginationContract,
) -> Result<DataBatch<LimitPoolEntry>, HithinkError> {
    let batch_id = composed_request_id(&pages)?;
    let total_rows: usize = pages.iter().map(|page| page.data.item.len()).sum();
    if total_rows != contract.total as usize {
        return Err(HithinkError::Protocol(
            "limit-pool page traversal did not produce the declared total".into(),
        ));
    }
    let mut records = Vec::with_capacity(total_rows);
    let mut identities = HashSet::with_capacity(total_rows);
    for value in pages.into_iter().flat_map(|page| page.data.item) {
        let entry = match request.kind() {
            LimitPoolKind::Upper => normalize_limit_up(
                request,
                observed_at,
                &batch_id,
                serde_json::from_value(value)
                    .map_err(|error| HithinkError::Decode(error.to_string()))?,
            )?,
            LimitPoolKind::Lower => normalize_limit_down(
                request,
                observed_at,
                &batch_id,
                serde_json::from_value(value)
                    .map_err(|error| HithinkError::Decode(error.to_string()))?,
            )?,
            LimitPoolKind::Broken => normalize_limit_break(
                request,
                observed_at,
                &batch_id,
                serde_json::from_value(value)
                    .map_err(|error| HithinkError::Decode(error.to_string()))?,
            )?,
            LimitPoolKind::PreviousUpper => unreachable!("validated before transport"),
        };
        if !identities.insert(entry.instrument.clone()) {
            return Err(HithinkError::Protocol(
                "limit pool contains duplicate instruments".into(),
            ));
        }
        records.push(entry);
    }
    records.truncate(request.limit().get() as usize);
    let provenance = batch_provenance(
        source_millis(contract.timestamp)?,
        observed_at.to_owned(),
        batch_id,
    )?;
    Ok(DataBatch::strict(records, provenance))
}

fn composed_request_id(pages: &[Success<LimitPoolData>]) -> Result<String, HithinkError> {
    let first = pages
        .first()
        .ok_or_else(|| HithinkError::Protocol("limit pool returned no pages".into()))?;
    if pages.len() == 1 {
        return Ok(first.request_id.clone());
    }
    let mut batch_id = String::from("hithink-pages:");
    for (index, page) in pages.iter().enumerate() {
        if index > 0 {
            batch_id.push(',');
        }
        batch_id.push_str(&page.request_id);
    }
    validate_safe_text("composed batch_id", &batch_id)?;
    Ok(batch_id)
}

fn normalize_limit_up(
    request: &LimitPoolRequest,
    observed_at: &str,
    batch_id: &str,
    item: LimitUpItem,
) -> Result<LimitPoolEntry, HithinkError> {
    let instrument = parse_thscode(&item.thscode, &item.ticker)?;
    validate_safe_text("limit-up name", &item.name)?;
    validate_clock_text("limit_up_time", &item.limit_up_time)?;
    validate_safe_text("continue_day_text", &item.continue_day_text)?;
    let _max_seal_money = nonnegative_money(item.max_seal_money, "max_seal_money")?;
    let _source_flags = (item.is_st, item.is_new);
    Ok(LimitPoolEntry {
        kind: LimitPoolKind::Upper,
        instrument,
        trading_date: request.trading_date().clone(),
        price: Price::new(item.last_price)?,
        change: Ratio::new(item.price_change_ratio_pct, RatioUnit::Percent)?,
        volume: None,
        turnover: None,
        sealed_amount: Some(nonnegative_money(item.seal_money, "seal_money")?),
        first_seal_at: Some(NonEmptyText::new(item.limit_up_time)?),
        last_seal_at: None,
        break_count: None,
        streak: Some(PositiveU32::new(item.continue_day_cnt)?),
        industry: None,
        board_name: None,
        seal_state: None,
        reseal_count: None,
        reason: Some(NonEmptyText::new(item.limit_up_reason)?),
        evidence: record_evidence(request.trading_date(), observed_at, batch_id)?,
    })
}

fn normalize_limit_down(
    request: &LimitPoolRequest,
    observed_at: &str,
    batch_id: &str,
    item: LimitDownItem,
) -> Result<LimitPoolEntry, HithinkError> {
    let instrument = parse_thscode(&item.thscode, &item.ticker)?;
    validate_safe_text("limit-down name", &item.name)?;
    validate_clock_text("first_limit_time", &item.first_limit_time)?;
    validate_clock_text("last_limit_time", &item.last_limit_time)?;
    Ok(LimitPoolEntry {
        kind: LimitPoolKind::Lower,
        instrument,
        trading_date: request.trading_date().clone(),
        price: Price::new(item.last_price)?,
        change: Ratio::new(item.price_change_ratio_pct, RatioUnit::Percent)?,
        volume: None,
        turnover: Some(Ratio::new(item.turnover_ratio_pct, RatioUnit::Percent)?),
        sealed_amount: None,
        first_seal_at: Some(NonEmptyText::new(item.first_limit_time)?),
        last_seal_at: Some(NonEmptyText::new(item.last_limit_time)?),
        break_count: None,
        streak: None,
        industry: None,
        board_name: None,
        seal_state: None,
        reseal_count: None,
        reason: None,
        evidence: record_evidence(request.trading_date(), observed_at, batch_id)?,
    })
}

fn normalize_limit_break(
    request: &LimitPoolRequest,
    observed_at: &str,
    batch_id: &str,
    item: LimitBreakItem,
) -> Result<LimitPoolEntry, HithinkError> {
    let instrument = parse_thscode(&item.thscode, &item.ticker)?;
    validate_safe_text("limit-break name", &item.name)?;
    let _turnover_amount = nonnegative_money(item.turnover, "turnover")?;
    Ok(LimitPoolEntry {
        kind: LimitPoolKind::Broken,
        instrument,
        trading_date: request.trading_date().clone(),
        price: Price::new(item.last_price)?,
        change: Ratio::new(item.price_change_ratio_pct, RatioUnit::Percent)?,
        volume: None,
        turnover: Some(Ratio::new(item.turnover_ratio_pct, RatioUnit::Percent)?),
        sealed_amount: None,
        first_seal_at: None,
        last_seal_at: None,
        break_count: Some(item.open_times),
        streak: None,
        industry: None,
        board_name: None,
        seal_state: None,
        reseal_count: None,
        reason: None,
        evidence: record_evidence(request.trading_date(), observed_at, batch_id)?,
    })
}

fn normalize_popularity(
    limit: PositiveU32,
    response: Success<PopularityDataResponse>,
) -> Result<DataBatch<PopularityRank>, HithinkError> {
    if response.data.timestamp <= 0 || response.data.item.len() > MAX_POPULARITY_ROWS {
        return Err(HithinkError::Protocol(
            "popularity response timestamp or size is invalid".into(),
        ));
    }
    let observed_at = now()?;
    let source_at = source_millis(response.data.timestamp)?;
    let batch_id = response.request_id;
    let mut instruments = HashSet::with_capacity(response.data.item.len());
    let mut ranks = HashSet::with_capacity(response.data.item.len());
    let mut records = Vec::with_capacity(response.data.item.len());
    for item in response.data.item {
        let instrument = parse_thscode(&item.thscode, &item.ticker)?;
        if !instruments.insert(instrument.clone()) || !ranks.insert(item.rank) {
            return Err(HithinkError::Protocol(
                "popularity response contains duplicate instruments or ranks".into(),
            ));
        }
        validate_safe_text("popularity name", &item.name)?;
        validate_safe_text("rank_trend", &item.rank_trend)?;
        let evidence = SourceEvidence::new(
            ProviderId::Tonghuashun,
            observed_at.clone(),
            batch_id.clone(),
        )?
        .with_source_at(source_at.clone())?;
        records.push(PopularityRank {
            instrument,
            rank: PositiveU32::new(item.rank)?,
            price: None,
            name: Some(NonEmptyText::new(item.name)?),
            rank_change: Some(FiniteNumber::new(f64::from(item.rank_change))?),
            return_ratio: None,
            heat: Some(FiniteNumber::new(item.heat)?),
            concepts: Vec::new(),
            tag: None,
            quote_evidence: None,
            evidence,
        });
    }
    records.sort_by_key(|item| item.rank);
    records.truncate(limit.get() as usize);
    let provenance = batch_provenance(source_at, observed_at, batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn instrument_to_thscode(instrument: &InstrumentId) -> Result<String, HithinkError> {
    if instrument.asset_class() != AssetClass::Equity
        || instrument.code().len() != 6
        || !instrument.code().bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(HithinkError::Unsupported(
            "Fuyao provider accepts six-digit A-share equities only".into(),
        ));
    }
    let suffix = match instrument.exchange() {
        Exchange::Shanghai => "SH",
        Exchange::Shenzhen => "SZ",
        Exchange::Beijing => "BJ",
    };
    Ok(format!("{}.{suffix}", instrument.code()))
}

fn parse_thscode(thscode: &str, ticker: &str) -> Result<InstrumentId, HithinkError> {
    if ticker.len() != 6
        || !ticker.bytes().all(|byte| byte.is_ascii_digit())
        || thscode.len() != 9
        || thscode.as_bytes().get(6) != Some(&b'.')
        || !thscode.starts_with(ticker)
    {
        return Err(HithinkError::Protocol(
            "response contains a malformed or contradictory thscode".into(),
        ));
    }
    let exchange = match &thscode[7..] {
        "SH" => Exchange::Shanghai,
        "SZ" => Exchange::Shenzhen,
        "BJ" => Exchange::Beijing,
        _ => {
            return Err(HithinkError::Protocol(
                "response thscode has an unsupported exchange".into(),
            ));
        }
    };
    Ok(InstrumentId::new(exchange, ticker, AssetClass::Equity)?)
}

fn validate_identity(
    expected_thscode: &str,
    expected_ticker: &str,
    actual_thscode: &str,
    actual_ticker: &str,
) -> Result<(), HithinkError> {
    if expected_thscode != actual_thscode || expected_ticker != actual_ticker {
        return Err(HithinkError::Protocol(
            "response instrument identity contradicts the request".into(),
        ));
    }
    parse_thscode(actual_thscode, actual_ticker)?;
    Ok(())
}

fn optional_finite(value: Option<f64>) -> Result<Option<FiniteNumber>, HithinkError> {
    value.map(FiniteNumber::new).transpose().map_err(Into::into)
}

fn finite_number_or_string<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;
    let value = serde_json::Value::deserialize(deserializer)?;
    let number = match value {
        serde_json::Value::Number(value) => value
            .as_f64()
            .ok_or_else(|| D::Error::custom("numeric value is outside f64 range"))?,
        serde_json::Value::String(value)
            if !value.is_empty()
                && value.len() <= 64
                && value == value.trim()
                && !value.chars().any(char::is_control) =>
        {
            value
                .parse::<f64>()
                .map_err(|_| D::Error::custom("numeric string is invalid"))?
        }
        _ => {
            return Err(D::Error::custom(
                "expected a number or strict numeric string",
            ))
        }
    };
    if !number.is_finite() {
        return Err(D::Error::custom("numeric value must be finite"));
    }
    Ok(number)
}

fn nonnegative_money(value: f64, field: &str) -> Result<Money, HithinkError> {
    if !value.is_finite() || value < 0.0 {
        return Err(HithinkError::Protocol(format!(
            "{field} must be finite and non-negative"
        )));
    }
    Ok(Money::new(value)?)
}

fn shares_to_lots(shares: f64) -> Result<f64, HithinkError> {
    if !shares.is_finite() || shares < 0.0 {
        return Err(HithinkError::Protocol(
            "historical volume must be finite and non-negative shares".into(),
        ));
    }
    Ok(shares / 100.0)
}

fn record_evidence(
    date: &IsoDate,
    observed_at: &str,
    batch_id: &str,
) -> Result<SourceEvidence, HithinkError> {
    Ok(
        SourceEvidence::new(ProviderId::Tonghuashun, observed_at, batch_id)?
            .with_source_at(date.as_str())?,
    )
}

fn batch_provenance(
    source_at: String,
    observed_at: String,
    batch_id: String,
) -> Result<Provenance, HithinkError> {
    Ok(Provenance::new("HithinkFinance", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?)
}

fn parse_date(value: &str) -> Result<Date, HithinkError> {
    let mut parts = value.split('-');
    let year = parts.next().and_then(|part| part.parse::<i32>().ok());
    let month = parts.next().and_then(|part| part.parse::<u8>().ok());
    let day = parts.next().and_then(|part| part.parse::<u8>().ok());
    if parts.next().is_some() {
        return Err(HithinkError::InvalidRequest("invalid ISO date".into()));
    }
    let (year, month, day) = match (year, month, day) {
        (Some(year), Some(month), Some(day)) => (year, month, day),
        _ => return Err(HithinkError::InvalidRequest("invalid ISO date".into())),
    };
    let month = Month::try_from(month)
        .map_err(|_| HithinkError::InvalidRequest("invalid ISO date".into()))?;
    Date::from_calendar_date(year, month, day)
        .map_err(|_| HithinkError::InvalidRequest("invalid ISO date".into()))
}

fn shanghai_offset() -> Result<UtcOffset, HithinkError> {
    UtcOffset::from_hms(8, 0, 0)
        .map_err(|_| HithinkError::Protocol("invalid fixed Shanghai offset".into()))
}

fn shanghai_millis(date: Date, time: Time) -> Result<i64, HithinkError> {
    let seconds = PrimitiveDateTime::new(date, time)
        .assume_offset(shanghai_offset()?)
        .unix_timestamp();
    seconds
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(i64::from(time.millisecond())))
        .ok_or_else(|| HithinkError::InvalidRequest("date is outside timestamp range".into()))
}

fn shanghai_date(timestamp_ms: i64, field: &str) -> Result<Date, HithinkError> {
    if timestamp_ms <= 0 {
        return Err(HithinkError::Protocol(format!(
            "{field} must be a positive unix-millisecond timestamp"
        )));
    }
    let seconds = timestamp_ms.div_euclid(1_000);
    let timestamp = OffsetDateTime::from_unix_timestamp(seconds)
        .map_err(|_| HithinkError::Protocol(format!("{field} is outside timestamp range")))?;
    Ok(timestamp.to_offset(shanghai_offset()?).date())
}

fn shanghai_midnight_date(timestamp_ms: i64, field: &str) -> Result<Date, HithinkError> {
    let date = shanghai_date(timestamp_ms, field)?;
    if timestamp_ms != shanghai_millis(date, Time::MIDNIGHT)? {
        return Err(HithinkError::Protocol(format!(
            "{field} is not Asia/Shanghai midnight"
        )));
    }
    Ok(date)
}

fn validate_clock_text(field: &str, value: &str) -> Result<(), HithinkError> {
    let bytes = value.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || ![bytes[0], bytes[1], bytes[3], bytes[4]]
            .into_iter()
            .all(|byte| byte.is_ascii_digit())
    {
        return Err(HithinkError::Protocol(format!("{field} must use HH:mm")));
    }
    let hour = value[..2].parse::<u8>().unwrap_or(u8::MAX);
    let minute = value[3..].parse::<u8>().unwrap_or(u8::MAX);
    if hour > 23 || minute > 59 {
        return Err(HithinkError::Protocol(format!(
            "{field} must use a valid HH:mm"
        )));
    }
    Ok(())
}

fn source_millis(value: i64) -> Result<String, HithinkError> {
    if value <= 0 {
        return Err(HithinkError::Protocol(
            "source timestamp must be positive".into(),
        ));
    }
    Ok(format!("unix-ms:{value}"))
}

fn now() -> Result<String, HithinkError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HithinkError::Protocol("system clock is before unix epoch".into()))?;
    Ok(format!(
        "{}.{:09}",
        elapsed.as_secs(),
        elapsed.subsec_nanos()
    ))
}

fn tracker_error(message: &str) -> HithinkError {
    HithinkError::Transport(TransportError::Internal(message.into()))
}

#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
