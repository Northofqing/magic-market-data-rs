#![forbid(unsafe_code)]
//! Bounded read-only adapter for verified Tonghuashun public HTTPS endpoints.

mod transport;

pub use transport::{HttpMethod, HttpRequest, HttpResponse, ThsTransport};

use encoding_rs::GBK;
use magic_market_core::{
    AssetClass, ConsensusData, ConsensusSnapshot, DataBatch, EarningsEstimate, Exchange,
    FiniteNumber, InstrumentId, InstrumentSignalRequest, LimitPoolCapabilities, LimitPoolEntry,
    LimitPoolKind, LimitPoolRequest, LimitPools, LoadProbeSnapshot, Money, NonEmptyText,
    PopularityData, PopularityRank, PositiveU32, Price, ProbeRequestTracker, Provenance,
    ProviderId, Ratio, RatioUnit, ResearchCapabilities, SignalCapabilities, SourceEvidence,
    StrongStockReason, StrongStockReasons, VerifiedEmpty,
};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use transport::HttpsTransport;
use url::Url;

const USER_AGENT: &str = "Mozilla/5.0 (compatible; magic-ths-rs/0.2; read-only public-data probe)";
const DEFAULT_CONSENSUS_ORIGIN: &str = "https://basic.10jqka.com.cn";
const DEFAULT_STRONG_ORIGIN: &str = "https://zx.10jqka.com.cn";
const DEFAULT_LIMIT_URL: &str = "https://data.10jqka.com.cn/dataapi/limit_up/limit_up_pool";
const DEFAULT_POPULARITY_URL: &str =
    "https://dq.10jqka.com.cn/fuyao/hot_list_data/out/hot_list/v1/stock";
const ALLOWED_HOSTS: [&str; 4] = [
    "basic.10jqka.com.cn",
    "zx.10jqka.com.cn",
    "data.10jqka.com.cn",
    "dq.10jqka.com.cn",
];
const MAX_CONSENSUS_INSTRUMENTS: usize = 20;
const MAX_STRONG_LIMIT: u32 = 200;
const MAX_LIMIT_POOL: u32 = 200;
const MAX_POPULARITY: u32 = 100;
pub(crate) const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ThsError {
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
    #[error("Tonghuashun response decoding failed: {0}")]
    Decode(String),
    #[error("Tonghuashun schema drift: {0}")]
    Schema(String),
    #[error("Tonghuashun response is incomplete: {0}")]
    Incomplete(String),
    #[error("Tonghuashun source verified an empty result: {0}")]
    VerifiedEmpty(Box<VerifiedEmpty>),
    #[error("probe admission evidence failed: {0}")]
    ProbeAdmission(#[from] magic_market_core::ProbeAdmissionError),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Debug, Clone)]
pub struct ThsConfig {
    pub consensus_origin: String,
    pub strong_origin: String,
    pub limit_url: String,
    pub popularity_url: String,
    pub timeout: Duration,
    pub minimum_interval: Duration,
}

impl Default for ThsConfig {
    fn default() -> Self {
        Self {
            consensus_origin: DEFAULT_CONSENSUS_ORIGIN.into(),
            strong_origin: DEFAULT_STRONG_ORIGIN.into(),
            limit_url: DEFAULT_LIMIT_URL.into(),
            popularity_url: DEFAULT_POPULARITY_URL.into(),
            timeout: Duration::from_secs(15),
            minimum_interval: Duration::from_secs(1),
        }
    }
}

impl ThsConfig {
    fn validate(&self) -> Result<(), ThsError> {
        for endpoint in [
            &self.consensus_origin,
            &self.strong_origin,
            &self.limit_url,
            &self.popularity_url,
        ] {
            validate_url(endpoint)?;
        }
        if self.timeout.is_zero() || self.timeout > Duration::from_secs(60) {
            return Err(ThsError::InvalidRequest(
                "timeout must be positive and at most 60 seconds".into(),
            ));
        }
        if self.minimum_interval < Duration::from_secs(1) {
            return Err(ThsError::InvalidRequest(
                "minimum request interval must be at least one second".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThsCapabilities {
    pub research: ResearchCapabilities,
    pub signals: SignalCapabilities,
    pub limit_pools: LimitPoolCapabilities,
}

#[derive(Clone)]
pub struct ThsClient {
    config: ThsConfig,
    transport: Arc<dyn ThsTransport>,
    pacing_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
    request_probe: Arc<Mutex<ProbeRequestTracker>>,
}

impl std::fmt::Debug for ThsClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ThsClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl ThsClient {
    pub fn new() -> Result<Self, ThsError> {
        Self::with_config(ThsConfig::default())
    }

    pub fn with_config(config: ThsConfig) -> Result<Self, ThsError> {
        config.validate()?;
        let transport = HttpsTransport::new(config.timeout)?;
        Ok(Self::from_parts(
            config.minimum_interval,
            config,
            Arc::new(transport),
        ))
    }

    pub fn with_transport(
        config: ThsConfig,
        transport: impl ThsTransport + 'static,
    ) -> Result<Self, ThsError> {
        config.validate()?;
        Ok(Self::from_parts(
            config.minimum_interval,
            config,
            Arc::new(transport),
        ))
    }

    fn from_parts(interval: Duration, config: ThsConfig, transport: Arc<dyn ThsTransport>) -> Self {
        Self {
            config,
            transport,
            pacing_interval: interval,
            request_gate: Arc::new(Mutex::new(None)),
            request_probe: Arc::new(Mutex::new(ProbeRequestTracker::default())),
        }
    }

    #[cfg(test)]
    fn with_test_transport(transport: impl ThsTransport + 'static) -> Self {
        Self::from_parts(Duration::ZERO, ThsConfig::default(), Arc::new(transport))
    }

    pub const fn capabilities() -> ThsCapabilities {
        ThsCapabilities {
            research: ResearchCapabilities {
                reports: false,
                consensus: true,
                semantic_search: false,
                pdf_download: false,
                document_body: false,
            },
            signals: SignalCapabilities {
                board_memberships: false,
                strong_stock_reasons: true,
                dragon_tiger: false,
                market_rankings: false,
                popularity: true,
                concept_hits: false,
            },
            limit_pools: LimitPoolCapabilities {
                upper: true,
                broken: false,
                lower: false,
                previous_upper: false,
                reasons: true,
            },
        }
    }

    pub fn load_probe_snapshot(&self) -> Result<LoadProbeSnapshot, ThsError> {
        self.request_probe
            .lock()
            .map(|probe| probe.snapshot())
            .map_err(|_| ThsError::Transport("request probe mutex poisoned".into()))
    }

    fn execute(&self, request: HttpRequest) -> Result<HttpResponse, ThsError> {
        validate_request(&request)?;
        let mut last_started = self
            .request_gate
            .lock()
            .map_err(|_| ThsError::Transport("request limiter mutex poisoned".into()))?;
        if let Some(previous) = *last_started {
            let elapsed = previous.elapsed();
            if elapsed < self.pacing_interval {
                thread::sleep(self.pacing_interval - elapsed);
            }
        }
        *last_started = Some(Instant::now());
        self.request_probe
            .lock()
            .map_err(|_| ThsError::Transport("request probe mutex poisoned".into()))?
            .request_started();
        let response = self.transport.execute(&request);
        self.request_probe
            .lock()
            .map_err(|_| ThsError::Transport("request probe mutex poisoned".into()))?
            .request_finished()
            .map_err(|error| ThsError::Transport(error.to_string()))?;
        drop(last_started);
        let response = response?;
        validate_response(&request, &response)?;
        Ok(response)
    }

    fn consensus_html(&self, instrument: &InstrumentId) -> Result<String, ThsError> {
        validate_equity(instrument)?;
        let url = format!(
            "{}/{}/worth.html",
            self.config.consensus_origin.trim_end_matches('/'),
            instrument.code()
        );
        let response = self.execute(HttpRequest {
            method: HttpMethod::Get,
            url,
            headers: vec![
                ("User-Agent".into(), USER_AGENT.into()),
                ("Accept".into(), "text/html".into()),
                ("Referer".into(), "https://basic.10jqka.com.cn/".into()),
            ],
        })?;
        ensure_html(&response)?;
        let (decoded, _, had_errors) = GBK.decode(&response.body);
        if had_errors {
            return Err(ThsError::Decode(
                "consensus HTML contains invalid GB18030 bytes".into(),
            ));
        }
        let html = decoded.into_owned();
        if !html.contains(instrument.code()) {
            return Err(ThsError::Schema(format!(
                "consensus page does not contain requested code {}",
                instrument.code()
            )));
        }
        Ok(html)
    }

    fn strong_json(&self, trading_date: &str) -> Result<Value, ThsError> {
        let url = format!(
            "{}/event/api/getharden/date/{trading_date}/orderby/date/orderway/desc/charset/GBK/",
            self.config.strong_origin.trim_end_matches('/')
        );
        self.get_json(url, "https://zx.10jqka.com.cn/")
    }

    fn limit_json(&self, request: &LimitPoolRequest) -> Result<Value, ThsError> {
        let mut url = Url::parse(&self.config.limit_url)
            .map_err(|error| ThsError::InvalidRequest(error.to_string()))?;
        let date = request.trading_date().as_str().replace('-', "");
        url.query_pairs_mut()
            .append_pair("page", "1")
            .append_pair("limit", &request.limit().get().to_string())
            .append_pair(
                "field",
                "199112,10,9001,330323,330324,330325,9002,330329,133971,133970,1968584,3475914,9003,9004",
            )
            .append_pair("filter", "HS,GEM2STAR")
            .append_pair("order_field", "330324")
            .append_pair("order_type", "0")
            .append_pair("date", &date);
        self.get_json(url.to_string(), "https://data.10jqka.com.cn/")
    }

    fn popularity_json(&self) -> Result<Value, ThsError> {
        let mut url = Url::parse(&self.config.popularity_url)
            .map_err(|error| ThsError::InvalidRequest(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("stock_type", "a")
            .append_pair("type", "hour")
            .append_pair("list_type", "normal");
        self.get_json(url.to_string(), "https://dq.10jqka.com.cn/")
    }

    fn get_json(&self, url: String, referer: &str) -> Result<Value, ThsError> {
        let response = self.execute(HttpRequest {
            method: HttpMethod::Get,
            url,
            headers: vec![
                ("User-Agent".into(), USER_AGENT.into()),
                ("Accept".into(), "application/json".into()),
                ("Referer".into(), referer.into()),
            ],
        })?;
        ensure_json(&response)?;
        serde_json::from_slice(&response.body).map_err(|error| ThsError::Decode(error.to_string()))
    }
}

impl ConsensusData for ThsClient {
    type Error = ThsError;

    fn consensus(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<ConsensusSnapshot>, Self::Error> {
        validate_instrument_batch(instruments)?;
        let mut parsed = Vec::with_capacity(instruments.len());
        let mut empty_instruments = Vec::new();
        let mut source_dates = Vec::new();
        for instrument in instruments {
            let html = self.consensus_html(instrument)?;
            let source_date = extract_as_of_date(&html);
            if let Some(source_date) = &source_date {
                source_dates.push(source_date.clone());
            }
            let estimates = match parse_consensus_table(&html)? {
                Some(estimates) => estimates,
                None if html.contains("暂无机构做出业绩预测") => {
                    empty_instruments.push(instrument.clone());
                    continue;
                }
                None => {
                    return Err(ThsError::Schema(format!(
                        "{} consensus EPS table is missing",
                        instrument.code()
                    )));
                }
            };
            let contributor_count = common_contributor_count(&estimates);
            parsed.push((
                instrument.clone(),
                estimates,
                contributor_count,
                source_date,
            ));
        }
        let observed_at = now()?;
        let batch_id = format!("ths:{observed_at}:consensus");
        let source_at = source_dates.iter().min().map(String::as_str);
        let provenance = provenance("tonghuashun", &observed_at, &batch_id, source_at)?;
        if !empty_instruments.is_empty() {
            if !parsed.is_empty() {
                let codes = empty_instruments
                    .iter()
                    .map(InstrumentId::code)
                    .collect::<Vec<_>>()
                    .join(",");
                return Err(ThsError::Incomplete(format!(
                    "atomic consensus batch rejected because source reports no current consensus for {codes}"
                )));
            }
            let request_identity = empty_instruments
                .iter()
                .map(instrument_identity)
                .collect::<Vec<_>>()
                .join(",");
            let mut evidence =
                SourceEvidence::new(ProviderId::Tonghuashun, &observed_at, &batch_id)?;
            if let Some(source_at) = source_at {
                evidence = evidence.with_source_at(source_at)?;
            }
            let empty = VerifiedEmpty::new(
                "consensus",
                request_identity,
                "source explicitly reports no current institutional consensus",
                evidence,
                provenance,
            )?;
            return Err(ThsError::VerifiedEmpty(Box::new(empty)));
        }
        let mut records = Vec::with_capacity(parsed.len());
        for (instrument, estimates, contributor_count, source_date) in parsed {
            let mut evidence =
                SourceEvidence::new(ProviderId::Tonghuashun, &observed_at, &batch_id)?;
            if let Some(source_date) = source_date {
                evidence = evidence.with_source_at(source_date)?;
            }
            records.push(ConsensusSnapshot {
                instrument,
                estimates,
                contributor_count,
                evidence,
            });
        }
        Ok(DataBatch::strict(records, provenance))
    }
}

impl StrongStockReasons for ThsClient {
    type Error = ThsError;

    fn strong_stock_reasons(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<StrongStockReason>, Self::Error> {
        validate_equity(request.instrument())?;
        if request.limit().get() > MAX_STRONG_LIMIT {
            return Err(ThsError::InvalidRequest(format!(
                "strong-stock limit must be at most {MAX_STRONG_LIMIT}"
            )));
        }
        let trading_date = request.trading_date().ok_or_else(|| {
            ThsError::InvalidRequest("strong-stock request requires a trading date".into())
        })?;
        let document = self.strong_json(trading_date.as_str())?;
        require_status(&document, "errocode")?;
        let rows = document
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| ThsError::Schema("strong-stock data array is missing".into()))?;
        if rows.len() > 500 {
            return Err(ThsError::Schema(
                "strong-stock response exceeds 500 rows".into(),
            ));
        }
        let observed_at = now()?;
        let batch_id = format!("ths:{observed_at}:strong-stock:{}", trading_date.as_str());
        let mut records = Vec::new();
        for row in rows {
            if required_string(row.get("code"), "strong-stock code")? != request.instrument().code()
            {
                continue;
            }
            let reason = required_string(row.get("reason"), "strong-stock reason")?;
            let subjects = split_subjects(&reason)?;
            let mut evidence =
                SourceEvidence::new(ProviderId::Tonghuashun, &observed_at, &batch_id)?;
            evidence = evidence.with_source_at(trading_date.as_str())?;
            records.push(StrongStockReason {
                instrument: request.instrument().clone(),
                trading_date: trading_date.clone(),
                reason: NonEmptyText::new(reason)?,
                subjects,
                limit_state: None,
                evidence,
            });
        }
        if records.len() > 1 {
            return Err(ThsError::Schema(format!(
                "strong-stock response contains duplicate code {}",
                request.instrument().code()
            )));
        }
        if records.is_empty() {
            return Err(ThsError::Incomplete(format!(
                "strong-stock response has no exact match for {} on {}",
                request.instrument().code(),
                trading_date.as_str()
            )));
        }
        let provenance = provenance(
            "tonghuashun",
            &observed_at,
            &batch_id,
            Some(trading_date.as_str()),
        )?;
        Ok(DataBatch::strict(records, provenance))
    }
}

impl LimitPools for ThsClient {
    type Error = ThsError;

    fn limit_pool(
        &self,
        request: &LimitPoolRequest,
    ) -> Result<DataBatch<LimitPoolEntry>, Self::Error> {
        if request.kind() != LimitPoolKind::Upper {
            return Err(ThsError::Unsupported(
                "Tonghuashun limit reveal supports only the upper-limit pool".into(),
            ));
        }
        if request.limit().get() > MAX_LIMIT_POOL {
            return Err(ThsError::InvalidRequest(format!(
                "Tonghuashun limit-pool limit must be at most {MAX_LIMIT_POOL}"
            )));
        }
        let document = self.limit_json(request)?;
        require_status(&document, "status_code")?;
        let rows = document
            .pointer("/data/info")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ThsError::Incomplete(format!(
                    "no upper-limit pool for {}",
                    request.trading_date()
                ))
            })?;
        if rows.len() > MAX_LIMIT_POOL as usize {
            return Err(ThsError::Schema(format!(
                "limit reveal contains {} rows, above the verified bound",
                rows.len()
            )));
        }
        if rows.is_empty() {
            return Err(ThsError::Incomplete(format!(
                "upper-limit pool is empty for {}",
                request.trading_date().as_str()
            )));
        }
        let observed_at = now()?;
        let batch_id = format!(
            "ths:{observed_at}:upper-limit:{}",
            request.trading_date().as_str()
        );
        let mut records = Vec::with_capacity(rows.len());
        let mut seen = HashSet::new();
        for row in rows.iter().take(request.limit().get() as usize) {
            let code = required_string(row.get("code"), "limit reveal code")?;
            if !seen.insert(code.clone()) {
                return Err(ThsError::Schema(format!(
                    "limit reveal contains duplicate code {code}"
                )));
            }
            let instrument = equity_from_code(&code)?;
            let price = required_f64(row.get("latest"), "limit reveal latest")?;
            let change = required_f64(row.get("change_rate"), "limit reveal change_rate")?;
            let sealed_amount = optional_f64(row.get("order_amount"), "order_amount")?
                .map(Money::new)
                .transpose()?;
            if sealed_amount.is_some_and(|amount| amount.get() < 0.0) {
                return Err(ThsError::Schema(
                    "limit reveal order_amount must be non-negative".into(),
                ));
            }
            let break_count = optional_u64(row.get("open_num"), "open_num")?
                .map(|value| {
                    u32::try_from(value)
                        .map_err(|_| ThsError::Schema("open_num exceeds u32".into()))
                })
                .transpose()?;
            let high_days = optional_string(row.get("high_days"), "high_days")?;
            let first_seal_at =
                optional_i64(row.get("first_limit_up_time"), "first_limit_up_time")?
                    .map(unix_seconds_to_china_iso)
                    .transpose()?
                    .map(NonEmptyText::new)
                    .transpose()?;
            let reason = optional_nonempty(row.get("reason_type"), "reason_type")?;
            let seal_state = optional_nonempty(row.get("limit_up_type"), "limit_up_type")?;
            let streak = high_days
                .as_deref()
                .and_then(parse_streak)
                .map(PositiveU32::new)
                .transpose()?;
            let mut evidence =
                SourceEvidence::new(ProviderId::Tonghuashun, &observed_at, &batch_id)?;
            evidence = evidence.with_source_at(request.trading_date().as_str())?;
            records.push(LimitPoolEntry {
                kind: LimitPoolKind::Upper,
                instrument,
                trading_date: request.trading_date().clone(),
                price: Price::new(price)?,
                change: Ratio::new(change, RatioUnit::Percent)?,
                volume: None,
                turnover: None,
                sealed_amount,
                first_seal_at,
                last_seal_at: None,
                break_count,
                streak,
                industry: None,
                board_name: None,
                seal_state,
                // The source exposes only a boolean is_again_limit flag, not a count.
                reseal_count: None,
                reason,
                evidence,
            });
        }
        let provenance = provenance(
            "tonghuashun",
            &observed_at,
            &batch_id,
            Some(request.trading_date().as_str()),
        )?;
        Ok(DataBatch::strict(records, provenance))
    }
}

impl PopularityData for ThsClient {
    type Error = ThsError;

    fn popularity(&self, limit: PositiveU32) -> Result<DataBatch<PopularityRank>, Self::Error> {
        if limit.get() > MAX_POPULARITY {
            return Err(ThsError::InvalidRequest(format!(
                "popularity limit must be at most {MAX_POPULARITY}"
            )));
        }
        let document = self.popularity_json()?;
        require_status(&document, "status_code")?;
        let rows = document
            .pointer("/data/stock_list")
            .and_then(Value::as_array)
            .ok_or_else(|| ThsError::Schema("popularity stock_list is missing".into()))?;
        if rows.len() > MAX_POPULARITY as usize {
            return Err(ThsError::Schema(format!(
                "popularity response contains {} rows, above the verified bound",
                rows.len()
            )));
        }
        if rows.is_empty() {
            return Err(ThsError::Incomplete(
                "popularity response contains no ranked stocks".into(),
            ));
        }
        let observed_at = now()?;
        let batch_id = format!("ths:{observed_at}:popularity-hour");
        let evidence = SourceEvidence::new(ProviderId::Tonghuashun, &observed_at, &batch_id)?;
        let mut records = Vec::with_capacity(limit.get() as usize);
        let mut seen = HashSet::new();
        for row in rows.iter().take(limit.get() as usize) {
            let code = required_string(row.get("code"), "popularity code")?;
            if !seen.insert(code.clone()) {
                return Err(ThsError::Schema(format!(
                    "popularity response contains duplicate code {code}"
                )));
            }
            let rank = required_u64(row.get("order"), "popularity order")?;
            let rank = u32::try_from(rank)
                .map_err(|_| ThsError::Schema("popularity order exceeds u32".into()))?;
            let tag = optional_object(row.get("tag"), "popularity tag")?;
            let concepts = optional_array(
                tag.and_then(|tag| tag.get("concept_tag")),
                "popularity concept_tag",
            )?
            .into_iter()
            .flatten()
            .map(|value| {
                NonEmptyText::new(required_string(Some(value), "popularity concept_tag")?)
                    .map_err(Into::into)
            })
            .collect::<Result<Vec<_>, ThsError>>()?;
            records.push(PopularityRank {
                instrument: equity_from_code(&code)?,
                rank: PositiveU32::new(rank)?,
                price: None,
                name: optional_nonempty(row.get("name"), "popularity name")?,
                rank_change: optional_f64(row.get("hot_rank_chg"), "hot_rank_chg")?
                    .map(FiniteNumber::new)
                    .transpose()?,
                return_ratio: optional_f64(row.get("rise_and_fall"), "rise_and_fall")?
                    .map(|value| Ratio::new(value, RatioUnit::Percent))
                    .transpose()?,
                heat: optional_f64(row.get("rate"), "rate")?
                    .map(FiniteNumber::new)
                    .transpose()?,
                concepts,
                tag: optional_nonempty(
                    tag.and_then(|tag| tag.get("popularity_tag")),
                    "popularity popularity_tag",
                )?,
                quote_evidence: None,
                evidence: evidence.clone(),
            });
        }
        let provenance = provenance("tonghuashun", &observed_at, &batch_id, None)?;
        Ok(DataBatch::strict(records, provenance))
    }
}

fn parse_consensus_table(html: &str) -> Result<Option<Vec<EarningsEstimate>>, ThsError> {
    let Some(marker) = html.find("汇总--预测年报每股收益") else {
        return Ok(None);
    };
    let table_start = html[..marker]
        .rfind("<table")
        .ok_or_else(|| ThsError::Schema("EPS caption has no enclosing table".into()))?;
    let table_end = html[marker..]
        .find("</table>")
        .map(|offset| marker + offset + "</table>".len())
        .ok_or_else(|| ThsError::Schema("EPS table has no closing tag".into()))?;
    let rows = extract_rows(&html[table_start..table_end])?;
    if rows.len() < 2 {
        return Err(ThsError::Schema(
            "EPS table has no header and data rows".into(),
        ));
    }
    let headers = &rows[0];
    let year_index = header_index(headers, "年度")?;
    let count_index = header_index(headers, "预测机构数")?;
    let minimum_index = header_index(headers, "最小值")?;
    let mean_index = header_index(headers, "均值")?;
    let maximum_index = header_index(headers, "最大值")?;
    let max_index = [
        year_index,
        count_index,
        minimum_index,
        mean_index,
        maximum_index,
    ]
    .into_iter()
    .max()
    .unwrap_or_default();
    let mut estimates = Vec::with_capacity(rows.len() - 1);
    for row in &rows[1..] {
        if row.len() <= max_index {
            return Err(ThsError::Schema(
                "EPS table row is shorter than its named headers".into(),
            ));
        }
        let year = row[year_index].parse::<u32>().map_err(|_| {
            ThsError::Schema(format!("invalid EPS fiscal year {:?}", row[year_index]))
        })?;
        let count = row[count_index].parse::<u32>().map_err(|_| {
            ThsError::Schema(format!(
                "invalid EPS contributor count {:?}",
                row[count_index]
            ))
        })?;
        let minimum = parse_html_optional_number(&row[minimum_index], "EPS minimum")?;
        let mean = parse_html_optional_number(&row[mean_index], "EPS mean")?;
        let maximum = parse_html_optional_number(&row[maximum_index], "EPS maximum")?;
        if minimum.is_none() && mean.is_none() && maximum.is_none() {
            return Err(ThsError::Schema(format!(
                "EPS row {year} contains no estimate values"
            )));
        }
        if let (Some(minimum), Some(mean)) = (minimum, mean) {
            if mean < minimum {
                return Err(ThsError::Schema(format!(
                    "EPS mean is below minimum for {year}"
                )));
            }
        }
        if let (Some(mean), Some(maximum)) = (mean, maximum) {
            if mean > maximum {
                return Err(ThsError::Schema(format!(
                    "EPS mean is above maximum for {year}"
                )));
            }
        }
        let estimate = EarningsEstimate::new(
            PositiveU32::new(year)?,
            mean.map(FiniteNumber::new).transpose()?,
            minimum.map(FiniteNumber::new).transpose()?,
            maximum.map(FiniteNumber::new).transpose()?,
            Some(PositiveU32::new(count)?),
            None,
            None,
        )?;
        estimates.push(estimate);
    }
    Ok(Some(estimates))
}

fn extract_rows(table: &str) -> Result<Vec<Vec<String>>, ThsError> {
    let mut rows = Vec::new();
    let mut remaining = table;
    while let Some(start) = remaining.find("<tr") {
        let after_start = &remaining[start..];
        let open_end = after_start
            .find('>')
            .ok_or_else(|| ThsError::Schema("table row opening tag is malformed".into()))?;
        let content = &after_start[open_end + 1..];
        let end = content
            .find("</tr>")
            .ok_or_else(|| ThsError::Schema("table row closing tag is missing".into()))?;
        rows.push(extract_cells(&content[..end])?);
        remaining = &content[end + "</tr>".len()..];
    }
    Ok(rows)
}

fn extract_cells(row: &str) -> Result<Vec<String>, ThsError> {
    let mut cells = Vec::new();
    let mut remaining = row;
    loop {
        let th = remaining.find("<th").map(|index| (index, "th"));
        let td = remaining.find("<td").map(|index| (index, "td"));
        let Some((start, tag)) = [th, td].into_iter().flatten().min_by_key(|entry| entry.0) else {
            break;
        };
        let after_start = &remaining[start..];
        let open_end = after_start
            .find('>')
            .ok_or_else(|| ThsError::Schema("table cell opening tag is malformed".into()))?;
        let content = &after_start[open_end + 1..];
        let closing = format!("</{tag}>");
        let end = content
            .find(&closing)
            .ok_or_else(|| ThsError::Schema("table cell closing tag is missing".into()))?;
        cells.push(strip_html(&content[..end]));
        remaining = &content[end + closing.len()..];
    }
    Ok(cells)
}

fn strip_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
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

fn header_index(headers: &[String], name: &str) -> Result<usize, ThsError> {
    let matches: Vec<_> = headers
        .iter()
        .enumerate()
        .filter(|(_, header)| header.as_str() == name)
        .map(|(index, _)| index)
        .collect();
    if matches.len() != 1 {
        return Err(ThsError::Schema(format!(
            "EPS table requires exactly one {name:?} header"
        )));
    }
    Ok(matches[0])
}

fn common_contributor_count(estimates: &[EarningsEstimate]) -> Option<PositiveU32> {
    let first = estimates.first()?.contributor_count();
    estimates
        .iter()
        .all(|estimate| estimate.contributor_count() == first)
        .then_some(first)
        .flatten()
}

fn parse_html_optional_number(value: &str, field: &str) -> Result<Option<f64>, ThsError> {
    let value = value.trim();
    if value.is_empty() || value == "--" || value == "-" {
        return Ok(None);
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .map(Some)
        .ok_or_else(|| ThsError::Schema(format!("{field} is not finite numeric data: {value:?}")))
}

fn extract_as_of_date(html: &str) -> Option<String> {
    let marker = html.find("截至")? + "截至".len();
    let candidate = html.get(marker..marker + 10)?;
    magic_market_core::IsoDate::new(candidate)
        .ok()
        .map(|date| date.as_str().to_owned())
}

fn require_status(document: &Value, field: &str) -> Result<(), ThsError> {
    let status = required_i64(document.get(field), field)?;
    if status == 0 {
        Ok(())
    } else {
        let message = match optional_string(document.get("errormsg"), "errormsg")? {
            Some(message) => Some(message),
            None => optional_string(document.get("message"), "message")?,
        }
        .unwrap_or_else(|| "no upstream message".into());
        Err(ThsError::Schema(format!("{field}={status}: {message}")))
    }
}

fn required_string(value: Option<&Value>, field: &str) -> Result<String, ThsError> {
    optional_string(value, field)?.ok_or_else(|| ThsError::Schema(format!("{field} is missing")))
}

fn optional_string(value: Option<&Value>, field: &str) -> Result<Option<String>, ThsError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => {
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            Ok((!text.is_empty()).then_some(text))
        }
        Some(_) => Err(ThsError::Schema(format!(
            "{field} must be a string when present"
        ))),
    }
}

fn optional_nonempty(value: Option<&Value>, field: &str) -> Result<Option<NonEmptyText>, ThsError> {
    optional_string(value, field)?
        .map(NonEmptyText::new)
        .transpose()
        .map_err(Into::into)
}

fn optional_object<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<Option<&'a serde_json::Map<String, Value>>, ThsError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(object)) => Ok(Some(object)),
        Some(_) => Err(ThsError::Schema(format!(
            "{field} must be an object when present"
        ))),
    }
}

fn optional_array<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<Option<&'a Vec<Value>>, ThsError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(array)) => Ok(Some(array)),
        Some(_) => Err(ThsError::Schema(format!(
            "{field} must be an array when present"
        ))),
    }
}

fn required_f64(value: Option<&Value>, field: &str) -> Result<f64, ThsError> {
    optional_f64(value, field)?.ok_or_else(|| ThsError::Schema(format!("{field} is missing")))
}

fn optional_f64(value: Option<&Value>, field: &str) -> Result<Option<f64>, ThsError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let parsed = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) if text.trim().is_empty() || text.trim() == "--" => return Ok(None),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
    .filter(|number| number.is_finite())
    .ok_or_else(|| ThsError::Schema(format!("{field} is not finite numeric data")))?;
    Ok(Some(parsed))
}

fn required_u64(value: Option<&Value>, field: &str) -> Result<u64, ThsError> {
    optional_u64(value, field)?.ok_or_else(|| ThsError::Schema(format!("{field} is missing")))
}

fn optional_u64(value: Option<&Value>, field: &str) -> Result<Option<u64>, ThsError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) if text.trim().is_empty() => return Ok(None),
        Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
    .map(Some)
    .ok_or_else(|| ThsError::Schema(format!("{field} is not a non-negative integer")))
}

fn required_i64(value: Option<&Value>, field: &str) -> Result<i64, ThsError> {
    optional_i64(value, field)?.ok_or_else(|| ThsError::Schema(format!("{field} is missing")))
}

fn optional_i64(value: Option<&Value>, field: &str) -> Result<Option<i64>, ThsError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) if text.trim().is_empty() => return Ok(None),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
    .map(Some)
    .ok_or_else(|| ThsError::Schema(format!("{field} is not an integer")))
}

fn split_subjects(reason: &str) -> Result<Vec<NonEmptyText>, ThsError> {
    reason
        .split('+')
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
        .map(|subject| NonEmptyText::new(subject).map_err(Into::into))
        .collect()
}

fn parse_streak(value: &str) -> Option<u32> {
    if value == "首板" {
        return Some(1);
    }
    if let Some(index) = value.rfind('板') {
        let prefix = &value[..index];
        let digits: String = prefix
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        return digits.parse().ok();
    }
    None
}

fn validate_instrument_batch(instruments: &[InstrumentId]) -> Result<(), ThsError> {
    if instruments.is_empty() {
        return Err(ThsError::InvalidRequest(
            "consensus instrument list must not be empty".into(),
        ));
    }
    if instruments.len() > MAX_CONSENSUS_INSTRUMENTS {
        return Err(ThsError::InvalidRequest(format!(
            "consensus accepts at most {MAX_CONSENSUS_INSTRUMENTS} instruments"
        )));
    }
    let mut seen = HashSet::new();
    for instrument in instruments {
        validate_equity(instrument)?;
        let identity = (instrument.exchange(), instrument.code());
        if !seen.insert(identity) {
            return Err(ThsError::InvalidRequest(format!(
                "duplicate consensus instrument {}",
                instrument.code()
            )));
        }
    }
    Ok(())
}

fn validate_equity(instrument: &InstrumentId) -> Result<(), ThsError> {
    if instrument.asset_class() != AssetClass::Equity {
        return Err(ThsError::Unsupported(format!(
            "Tonghuashun family supports equities, not {:?}",
            instrument.asset_class()
        )));
    }
    let code = instrument.code();
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ThsError::InvalidRequest(
            "Tonghuashun stock code must contain exactly six digits".into(),
        ));
    }
    let expected_exchange = match code.as_bytes()[0] {
        b'6' => Exchange::Shanghai,
        b'0' | b'3' => Exchange::Shenzhen,
        b'4' | b'8' => Exchange::Beijing,
        b'9' if code.starts_with("920") => Exchange::Beijing,
        prefix => {
            return Err(ThsError::Unsupported(format!(
                "Tonghuashun stock-code prefix {:?} has no verified exchange mapping",
                char::from(prefix)
            )));
        }
    };
    if instrument.exchange() != expected_exchange {
        return Err(ThsError::InvalidRequest(format!(
            "Tonghuashun code {code} implies {expected_exchange:?} exchange, not {:?}",
            instrument.exchange()
        )));
    }
    Ok(())
}

fn equity_from_code(code: &str) -> Result<InstrumentId, ThsError> {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ThsError::Schema(format!(
            "source returned invalid stock code {code:?}"
        )));
    }
    let exchange = match code.as_bytes()[0] {
        b'6' => Exchange::Shanghai,
        b'0' | b'3' => Exchange::Shenzhen,
        b'4' | b'8' => Exchange::Beijing,
        b'9' if code.starts_with("920") => Exchange::Beijing,
        _ => {
            return Err(ThsError::Schema(format!(
                "source returned stock code with unsupported venue prefix {code}"
            )));
        }
    };
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
}

fn instrument_identity(instrument: &InstrumentId) -> String {
    let suffix = match instrument.exchange() {
        Exchange::Shanghai => "SH",
        Exchange::Shenzhen => "SZ",
        Exchange::Beijing => "BJ",
    };
    format!("{}.{suffix}", instrument.code())
}

fn validate_request(request: &HttpRequest) -> Result<(), ThsError> {
    validate_url(&request.url)
}

fn validate_url(value: &str) -> Result<(), ThsError> {
    let parsed = Url::parse(value).map_err(|error| ThsError::InvalidRequest(error.to_string()))?;
    if parsed.scheme() != "https"
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.port_or_known_default() != Some(443)
    {
        return Err(ThsError::InvalidRequest(
            "Tonghuashun endpoints must use credential-free HTTPS on port 443".into(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| ThsError::InvalidRequest("endpoint host is missing".into()))?;
    if !ALLOWED_HOSTS.contains(&host) {
        return Err(ThsError::InvalidRequest(format!(
            "Tonghuashun host {host} is not allowlisted"
        )));
    }
    Ok(())
}

fn validate_response(request: &HttpRequest, response: &HttpResponse) -> Result<(), ThsError> {
    if response.body.len() > MAX_RESPONSE_BYTES {
        return Err(ThsError::Incomplete(format!(
            "response body exceeds {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    validate_url(&response.final_url)?;
    let expected =
        Url::parse(&request.url).map_err(|error| ThsError::InvalidRequest(error.to_string()))?;
    let actual =
        Url::parse(&response.final_url).map_err(|error| ThsError::Schema(error.to_string()))?;
    if expected != actual {
        return Err(ThsError::Schema(
            "redirected or final response URL does not match the request".into(),
        ));
    }
    match response.status {
        200..=299 => Ok(()),
        401 | 403 => Err(ThsError::Authentication(response.status)),
        429 => Err(ThsError::RateLimited),
        status => Err(ThsError::HttpStatus(status)),
    }
}

fn ensure_json(response: &HttpResponse) -> Result<(), ThsError> {
    if response
        .content_type
        .as_deref()
        .is_some_and(|value| value.to_ascii_lowercase().contains("html"))
    {
        return Err(ThsError::Schema(
            "JSON endpoint returned an HTML/login document".into(),
        ));
    }
    let first = response
        .body
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace());
    if first != Some(b'{') {
        return Err(ThsError::Schema(
            "successful response is not a JSON object".into(),
        ));
    }
    Ok(())
}

fn ensure_html(response: &HttpResponse) -> Result<(), ThsError> {
    if response
        .content_type
        .as_deref()
        .is_some_and(|value| !value.to_ascii_lowercase().contains("html"))
    {
        return Err(ThsError::Schema(format!(
            "expected HTML but received {:?}",
            response.content_type
        )));
    }
    if !response.body.windows(5).any(|window| window == b"<html")
        && !response.body.windows(6).any(|window| window == b"<table")
    {
        return Err(ThsError::Schema(
            "successful consensus response is not HTML".into(),
        ));
    }
    Ok(())
}

fn provenance(
    source: &str,
    observed_at: &str,
    batch_id: &str,
    source_at: Option<&str>,
) -> Result<Provenance, ThsError> {
    let mut provenance =
        Provenance::new(source, observed_at.to_owned())?.with_batch_id(batch_id.to_owned())?;
    if let Some(source_at) = source_at {
        provenance = provenance.with_source_at(source_at.to_owned())?;
    }
    Ok(provenance)
}

fn now() -> Result<String, ThsError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| ThsError::Transport(format!("system clock error: {error}")))
}

fn unix_seconds_to_china_iso(seconds: i64) -> Result<String, ThsError> {
    let local = seconds
        .checked_add(8 * 60 * 60)
        .ok_or_else(|| ThsError::Schema("source timestamp overflow".into()))?;
    let days = local.div_euclid(86_400);
    let day_seconds = local.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days)
        .ok_or_else(|| ThsError::Schema("source timestamp is outside supported years".into()))?;
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    let second = day_seconds % 60;
    Ok(format!(
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
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
