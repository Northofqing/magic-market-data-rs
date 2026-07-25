use crate::transport::{
    validate_endpoint, validate_minimum_interval, validate_request, validate_response,
    validate_timeout, ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, HttpsTransport,
    RequestGate,
};
use crate::{ExchangeError, ProviderCapabilities};
use magic_market_core::{
    AssetClass, Capabilities, CapitalCapabilities, ContentCapabilities, DataBatch, Exchange,
    InstrumentId, Money, NonEmptyText, NorthboundChannel, NorthboundDailyRequest,
    NorthboundDailyStat, NorthboundDailyStatistics, NorthboundQuotaBalance, NorthboundTopTurnover,
    PositiveU32, Provenance, ProviderId, Quantity, SignalCapabilities, SourceEvidence,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BASE_ENDPOINT: &str = "https://www.hkex.com.hk/eng/csm/DailyStat/";
const HOST: &str = "www.hkex.com.hk";
const BASE_PATH: &str = "/eng/csm/DailyStat/";
const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; magic-exchange-rs/0.2; read-only official-data probe)";
const SUMMARY_SCHEMA: [&str; 4] = ["Total Turnover", "Total Trade Count", "DQB", "ETF Turnover"];
const TOP_SCHEMA: [&str; 4] = ["Rank", "Stock Code", "Stock Name", "Total Turnover"];
const QUOTA_UNAVAILABLE_SENTINEL: &str = "999,999,999";

#[derive(Debug, Clone)]
pub struct HkexConfig {
    pub base_endpoint: String,
    pub timeout: Duration,
    pub minimum_interval: Duration,
}

impl Default for HkexConfig {
    fn default() -> Self {
        Self {
            base_endpoint: BASE_ENDPOINT.into(),
            timeout: Duration::from_secs(15),
            minimum_interval: Duration::from_secs(1),
        }
    }
}

impl HkexConfig {
    fn validate(&self) -> Result<(), ExchangeError> {
        validate_endpoint(&self.base_endpoint, HOST, BASE_PATH)?;
        validate_timeout(self.timeout)?;
        validate_minimum_interval(self.minimum_interval)
    }
}

#[derive(Clone)]
pub struct HkexClient {
    config: HkexConfig,
    transport: Arc<dyn ExchangeTransport>,
    gate: Arc<RequestGate>,
}

impl std::fmt::Debug for HkexClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HkexClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl HkexClient {
    pub fn new() -> Result<Self, ExchangeError> {
        Self::with_config(HkexConfig::default())
    }

    pub fn with_config(config: HkexConfig) -> Result<Self, ExchangeError> {
        config.validate()?;
        let transport = HttpsTransport::new(config.timeout)?;
        Self::from_parts(config, Arc::new(transport))
    }

    pub fn with_transport(
        config: HkexConfig,
        transport: impl ExchangeTransport + 'static,
    ) -> Result<Self, ExchangeError> {
        config.validate()?;
        Self::from_parts(config, Arc::new(transport))
    }

    fn from_parts(
        config: HkexConfig,
        transport: Arc<dyn ExchangeTransport>,
    ) -> Result<Self, ExchangeError> {
        Ok(Self {
            gate: Arc::new(RequestGate::new(config.minimum_interval)),
            config,
            transport,
        })
    }

    pub const fn provider_id() -> ProviderId {
        ProviderId::Hkex
    }

    pub const fn capabilities() -> ProviderCapabilities {
        ProviderCapabilities {
            provider: ProviderId::Hkex,
            market: Capabilities::new(),
            content: ContentCapabilities {
                instrument_news: false,
                global_news: false,
                announcements: false,
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
                northbound_daily_statistics: true,
            },
            signals: SignalCapabilities {
                board_memberships: false,
                strong_stock_reasons: false,
                dragon_tiger: false,
                market_rankings: false,
                popularity: false,
                concept_hits: false,
            },
        }
    }

    fn execute(
        &self,
        request: HttpRequest,
        expected_path: &str,
    ) -> Result<HttpResponse, ExchangeError> {
        validate_request(&request, HttpMethod::Get, HOST, expected_path)?;
        let response = self.gate.execute(|| self.transport.execute(&request))?;
        validate_response(&request, &response, &["javascript"])?;
        Ok(response)
    }

    fn fetch(&self, request: &NorthboundDailyRequest) -> Result<DailyDocument, ExchangeError> {
        let compact_date = request.trading_date().as_str().replace('-', "");
        let file_name = format!("data_tab_daily_{compact_date}e.js");
        let expected_path = format!("{BASE_PATH}{file_name}");
        let url = format!("{}{file_name}", self.config.base_endpoint);
        let response = self.execute(
            HttpRequest {
                method: HttpMethod::Get,
                url,
                headers: vec![
                    ("User-Agent".into(), USER_AGENT.into()),
                    ("Accept".into(), "application/javascript".into()),
                ],
                body: Vec::new(),
            },
            &expected_path,
        )?;
        parse_document(&response.body)
    }
}

impl NorthboundDailyStatistics for HkexClient {
    type Error = ExchangeError;

    fn northbound_daily_statistics(
        &self,
        request: &NorthboundDailyRequest,
    ) -> Result<DataBatch<NorthboundDailyStat>, Self::Error> {
        let document = self.fetch(request)?;
        let market = match request.channel() {
            NorthboundChannel::Shanghai => "SSE Northbound",
            NorthboundChannel::Shenzhen => "SZSE Northbound",
        };
        let matching = document
            .0
            .iter()
            .filter(|entry| entry.market == market)
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(ExchangeError::Incomplete(format!(
                "HKEX response must contain exactly one {market} record"
            )));
        }
        let entry = matching[0];
        if entry.date != request.trading_date().as_str() {
            return Err(ExchangeError::Schema(format!(
                "HKEX source date {} does not match request {}",
                entry.date,
                request.trading_date().as_str()
            )));
        }
        if entry.trading_day != 1 {
            return Err(ExchangeError::Incomplete(format!(
                "HKEX marks {} as a non-trading day for {market}",
                request.trading_date().as_str()
            )));
        }
        let summary = unique_table(entry, 1, "tradingTable")?;
        require_schema(summary, &SUMMARY_SCHEMA)?;
        if summary.rows.len() != 4 {
            return Err(ExchangeError::Incomplete(
                "HKEX northbound summary must contain exactly four rows".into(),
            ));
        }
        let total_turnover = parse_summary_money(single_cell(&summary.rows[0])?)?;
        let total_trade_count = parse_count(single_cell(&summary.rows[1])?)?;
        let quota_raw = single_cell(&summary.rows[2])?;
        let quota_balance = if quota_raw == QUOTA_UNAVAILABLE_SENTINEL {
            NorthboundQuotaBalance::Unavailable
        } else {
            NorthboundQuotaBalance::Amount(parse_summary_money(quota_raw)?)
        };
        let etf_turnover = parse_summary_money(single_cell(&summary.rows[3])?)?;

        let top_table = unique_table(entry, 2, "top10Table")?;
        require_schema(top_table, &TOP_SCHEMA)?;
        if top_table.rows.len() != 10 {
            return Err(ExchangeError::Incomplete(
                "HKEX northbound Top 10 must contain exactly ten rows".into(),
            ));
        }
        let mut top_turnover = Vec::with_capacity(10);
        let mut instruments = HashSet::with_capacity(10);
        for (index, row) in top_table.rows.iter().enumerate() {
            let cells = row_cells(row)?;
            if cells.len() != 4 {
                return Err(ExchangeError::Schema(
                    "HKEX northbound Top 10 row must contain four cells".into(),
                ));
            }
            let rank = parse_u32(&cells[0], "rank")?;
            let expected_rank = u32::try_from(index + 1)
                .map_err(|_| ExchangeError::Schema("HKEX rank exceeds u32".into()))?;
            if rank != expected_rank {
                return Err(ExchangeError::Schema(
                    "HKEX northbound ranks must be ordered 1 through 10".into(),
                ));
            }
            let instrument = parse_instrument(request.channel(), &cells[1])?;
            if !instruments.insert(instrument.clone()) {
                return Err(ExchangeError::Schema(
                    "HKEX northbound Top 10 contains a duplicate instrument".into(),
                ));
            }
            top_turnover.push(NorthboundTopTurnover::new(
                PositiveU32::new(rank)?,
                instrument,
                NonEmptyText::new(&cells[2])?,
                parse_money(&cells[3])?,
            )?);
        }

        let observed_at = observed_at()?;
        let batch_id = format!(
            "hkex-northbound-{}-{}",
            request.trading_date().as_str(),
            match request.channel() {
                NorthboundChannel::Shanghai => "sse",
                NorthboundChannel::Shenzhen => "szse",
            }
        );
        let evidence = SourceEvidence::new(ProviderId::Hkex, &observed_at, &batch_id)?
            .with_source_at(request.trading_date().as_str())?;
        let record = NorthboundDailyStat::new(
            request.trading_date().clone(),
            request.channel(),
            total_turnover,
            total_trade_count,
            quota_balance,
            etf_turnover,
            top_turnover,
            evidence,
        )?;
        let provenance = Provenance::new("hkex-official", observed_at)?
            .with_source_at(request.trading_date().as_str())?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::strict(vec![record], provenance))
    }
}

#[derive(Debug, Deserialize)]
struct DailyDocument(Vec<DailyEntry>);

#[derive(Debug, Deserialize)]
struct DailyEntry {
    date: String,
    market: String,
    #[serde(rename = "tradingDay")]
    trading_day: u8,
    content: Vec<DailyContent>,
}

#[derive(Debug, Deserialize)]
struct DailyContent {
    style: u8,
    table: DailyTable,
}

#[derive(Debug, Deserialize)]
struct DailyTable {
    classname: String,
    schema: Vec<Vec<String>>,
    #[serde(rename = "tr")]
    rows: Vec<DailyRow>,
}

#[derive(Debug, Deserialize)]
struct DailyRow {
    #[serde(rename = "td")]
    cells: Vec<Vec<String>>,
}

fn parse_document(body: &[u8]) -> Result<DailyDocument, ExchangeError> {
    let text = std::str::from_utf8(body)
        .map_err(|error| ExchangeError::Decode(format!("HKEX JavaScript is not UTF-8: {error}")))?;
    let trimmed = text.trim();
    let payload = trimmed
        .strip_prefix("tabData")
        .and_then(|value| value.trim_start().strip_prefix('='))
        .ok_or_else(|| {
            ExchangeError::Decode("HKEX JavaScript is missing tabData assignment".into())
        })?
        .trim();
    let payload = payload.strip_suffix(';').unwrap_or(payload).trim();
    let entries = serde_json::from_str(payload)
        .map_err(|error| ExchangeError::Decode(format!("invalid HKEX tabData JSON: {error}")))?;
    Ok(DailyDocument(entries))
}

fn unique_table<'a>(
    entry: &'a DailyEntry,
    style: u8,
    classname: &str,
) -> Result<&'a DailyTable, ExchangeError> {
    let matching = entry
        .content
        .iter()
        .filter(|content| content.style == style && content.table.classname == classname)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(ExchangeError::Schema(format!(
            "HKEX {classname} table must occur exactly once"
        )));
    }
    Ok(&matching[0].table)
}

fn require_schema(table: &DailyTable, expected: &[&str]) -> Result<(), ExchangeError> {
    if table.schema.len() != 1
        || table.schema[0].len() != expected.len()
        || !table.schema[0]
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())
    {
        return Err(ExchangeError::Schema(format!(
            "HKEX {} schema changed",
            table.classname
        )));
    }
    Ok(())
}

fn row_cells(row: &DailyRow) -> Result<&[String], ExchangeError> {
    if row.cells.len() != 1 {
        return Err(ExchangeError::Schema(
            "HKEX table row must contain exactly one cell group".into(),
        ));
    }
    Ok(&row.cells[0])
}

fn single_cell(row: &DailyRow) -> Result<&str, ExchangeError> {
    let cells = row_cells(row)?;
    if cells.len() != 1 {
        return Err(ExchangeError::Schema(
            "HKEX summary row must contain exactly one value".into(),
        ));
    }
    Ok(&cells[0])
}

fn validate_grouped_number(value: &str, allow_decimal: bool) -> Result<String, ExchangeError> {
    if value.is_empty() || value.trim() != value || value.starts_with('-') || value.starts_with('+')
    {
        return Err(ExchangeError::Schema(format!(
            "HKEX numeric value {value:?} is invalid"
        )));
    }
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || fraction.is_some_and(|digits| {
            !allow_decimal || digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return Err(ExchangeError::Schema(format!(
            "HKEX numeric value {value:?} is invalid"
        )));
    }
    let groups = integer.split(',').collect::<Vec<_>>();
    if groups.is_empty()
        || groups[0].is_empty()
        || groups[0].len() > 3
        || !groups[0].bytes().all(|byte| byte.is_ascii_digit())
        || groups
            .iter()
            .skip(1)
            .any(|group| group.len() != 3 || !group.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(ExchangeError::Schema(format!(
            "HKEX numeric grouping {value:?} is invalid"
        )));
    }
    Ok(value.replace(',', ""))
}

fn parse_money(value: &str) -> Result<Money, ExchangeError> {
    let normalized = validate_grouped_number(value, true)?;
    let parsed = normalized
        .parse::<f64>()
        .map_err(|error| ExchangeError::Schema(format!("invalid HKEX money {value:?}: {error}")))?;
    if parsed < 0.0 {
        return Err(ExchangeError::Schema(
            "HKEX money must be non-negative".into(),
        ));
    }
    Money::new(parsed).map_err(ExchangeError::from)
}

fn parse_summary_money(value: &str) -> Result<Money, ExchangeError> {
    let millions = parse_money(value)?.get();
    Money::new(millions * 1_000_000.0).map_err(ExchangeError::from)
}

fn parse_count(value: &str) -> Result<Quantity, ExchangeError> {
    let normalized = validate_grouped_number(value, false)?;
    let parsed = normalized
        .parse::<u64>()
        .map_err(|error| ExchangeError::Schema(format!("invalid HKEX count {value:?}: {error}")))?;
    if parsed > (1_u64 << 53) {
        return Err(ExchangeError::Schema(
            "HKEX count exceeds the exact integer range of the Core quantity type".into(),
        ));
    }
    Quantity::new(parsed as f64).map_err(ExchangeError::from)
}

fn parse_u32(value: &str, field: &str) -> Result<u32, ExchangeError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ExchangeError::Schema(format!(
            "HKEX {field} must contain only ASCII digits"
        )));
    }
    value
        .parse()
        .map_err(|error| ExchangeError::Schema(format!("invalid HKEX {field}: {error}")))
}

fn parse_instrument(
    channel: NorthboundChannel,
    source_code: &str,
) -> Result<InstrumentId, ExchangeError> {
    if source_code.is_empty()
        || source_code.len() > 6
        || !source_code.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ExchangeError::Schema(format!(
            "invalid HKEX northbound stock code {source_code:?}"
        )));
    }
    let code = match channel {
        NorthboundChannel::Shanghai => {
            if source_code.len() != 6 || !matches!(source_code.as_bytes()[0], b'5' | b'6') {
                return Err(ExchangeError::Schema(format!(
                    "invalid SSE northbound stock code {source_code:?}"
                )));
            }
            source_code.to_owned()
        }
        NorthboundChannel::Shenzhen => format!("{source_code:0>6}"),
    };
    let (exchange, asset_class) = match channel {
        NorthboundChannel::Shanghai => (
            Exchange::Shanghai,
            if code.starts_with('5') {
                AssetClass::Fund
            } else {
                AssetClass::Equity
            },
        ),
        NorthboundChannel::Shenzhen => {
            if !matches!(code.as_bytes()[0], b'0' | b'1' | b'3') {
                return Err(ExchangeError::Schema(format!(
                    "invalid SZSE northbound stock code {source_code:?}"
                )));
            }
            (
                Exchange::Shenzhen,
                if code.starts_with('1') {
                    AssetClass::Fund
                } else {
                    AssetClass::Equity
                },
            )
        }
    };
    InstrumentId::new(exchange, code, asset_class).map_err(ExchangeError::from)
}

fn observed_at() -> Result<String, ExchangeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("unix:{}", duration.as_millis()))
        .map_err(|error| ExchangeError::Transport(format!("system clock error: {error}")))
}
