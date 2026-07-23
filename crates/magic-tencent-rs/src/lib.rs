#![forbid(unsafe_code)]
//! Read-only supplemental adapter for Tencent's public quote endpoint.
//!
//! The endpoint is undocumented and has no project-visible SLA. This crate
//! consequently exposes only fields whose positions and units are checked by
//! fixtures and live probes.

mod bars;
mod minute;
mod trades;

use encoding_rs::GBK;
use magic_market_core::{
    AssetClass, Board, BookLevel, Capabilities, DataBatch, DataStatus, Exchange, FiniteNumber,
    InstrumentId, MarketStatistics, MarketStatisticsProvider, Money, OrderBook, OrderBooks, Price,
    PriceLimitRule, ProviderId, Quantity, Quote, Ratio, RatioUnit, RealtimeQuotes,
    SecurityMetadata, SecurityMetadataProvider, SourceEvidence,
};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const DEFAULT_ENDPOINT: &str = "https://qt.gtimg.cn/q=";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BATCH_SIZE: usize = 50;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Errors raised by the Tencent supplemental provider.
#[derive(Debug, Error)]
pub enum TencentError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("Tencent response decoding failed: {0}")]
    Decode(String),
    #[error("Tencent protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

/// Bounded byte transport used by the adapter and by deterministic fixtures.
pub trait SnapshotTransport: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, TencentError>;
}

#[derive(Clone)]
struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    fn new(timeout: Duration) -> Result<Self, TencentError> {
        if timeout.is_zero() {
            return Err(TencentError::InvalidRequest(
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

impl SnapshotTransport for HttpsTransport {
    fn get(&self, url: &str) -> Result<Vec<u8>, TencentError> {
        if !url.starts_with("https://") {
            return Err(TencentError::InvalidRequest(
                "Tencent endpoint must use HTTPS".into(),
            ));
        }
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|error| TencentError::Transport(error.to_string()))?;
        if response.status() != 200 {
            return Err(TencentError::Transport(format!(
                "unexpected HTTP status {}",
                response.status()
            )));
        }
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|error| TencentError::Transport(error.to_string()))?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(TencentError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        Ok(body)
    }
}

/// Read-only Tencent quote client. Clones share one connection-pooling agent.
#[derive(Clone)]
pub struct TencentClient {
    endpoint: String,
    pub(crate) transport: Arc<dyn SnapshotTransport>,
}

impl std::fmt::Debug for TencentClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TencentClient")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl TencentClient {
    /// Creates a client with bounded connect/read/write timeouts.
    pub fn new() -> Result<Self, TencentError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    /// Creates a client with a caller-selected positive timeout.
    pub fn with_timeout(timeout: Duration) -> Result<Self, TencentError> {
        Ok(Self {
            endpoint: DEFAULT_ENDPOINT.to_owned(),
            transport: Arc::new(HttpsTransport::new(timeout)?),
        })
    }

    /// Creates a client backed by an injected transport for deterministic use.
    pub fn with_transport(transport: impl SnapshotTransport + 'static) -> Self {
        Self {
            endpoint: DEFAULT_ENDPOINT.to_owned(),
            transport: Arc::new(transport),
        }
    }

    /// Reports only capabilities proven by the parser and live probe.
    pub const fn capabilities() -> Capabilities {
        Capabilities {
            quotes: true,
            bars: true,
            minute: true,
            trades: true,
            fundamentals: false,
            corporate_actions: false,
            blocks: false,
            money_flow: false,
            order_book: true,
            auction: false,
            security_metadata: true,
        }
    }

    fn snapshots(&self, instruments: &[InstrumentId]) -> Result<Vec<Snapshot>, TencentError> {
        let symbols = validate_instruments(instruments)?;
        let url = format!("{}{}", self.endpoint, symbols.join(","));
        let response = self.transport.get(&url)?;
        let parsed = parse_response(&response)?;
        order_snapshots(&symbols, parsed)
    }

    fn statistics_snapshots(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<Vec<Snapshot>, TencentError> {
        let symbols = validate_statistics_instruments(instruments)?;
        let url = format!("{}{}", self.endpoint, symbols.join(","));
        let response = self.transport.get(&url)?;
        let parsed = parse_statistics_response(&response)?;
        order_snapshots(&symbols, parsed)
    }
}

#[derive(Debug, Clone)]
struct SnapshotStatistics {
    turnover_rate: Option<f64>,
    trailing_pe: Option<f64>,
    static_pe: Option<f64>,
    pb: Option<f64>,
    total_market_cap_yi: Option<f64>,
    floating_market_cap_yi: Option<f64>,
    upper_limit: Option<f64>,
    lower_limit: Option<f64>,
    volume_ratio: Option<f64>,
}

#[derive(Debug, Clone)]
struct Snapshot {
    symbol: String,
    name: Option<String>,
    current: f64,
    previous_close: Option<f64>,
    open: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
    change_percent: Option<f64>,
    volume_lots: f64,
    amount_yuan: Option<f64>,
    bids: [(Option<f64>, Option<f64>); 5],
    asks: [(Option<f64>, Option<f64>); 5],
    source_at: Option<String>,
    statistics: Option<SnapshotStatistics>,
}

#[derive(Debug, Clone, Copy)]
enum SnapshotParseMode {
    Base,
    Statistics,
}

pub(crate) fn validate_instruments(
    instruments: &[InstrumentId],
) -> Result<Vec<String>, TencentError> {
    if instruments.is_empty() {
        return Err(TencentError::InvalidRequest(
            "instrument list must not be empty".into(),
        ));
    }
    if instruments.len() > MAX_BATCH_SIZE {
        return Err(TencentError::InvalidRequest(format!(
            "at most {MAX_BATCH_SIZE} instruments are accepted per request"
        )));
    }
    let mut seen = HashSet::with_capacity(instruments.len());
    instruments
        .iter()
        .map(|instrument| {
            if instrument.asset_class() != AssetClass::Equity {
                return Err(TencentError::Unsupported(format!(
                    "asset class {:?} has unverified field units",
                    instrument.asset_class()
                )));
            }
            let prefix = match instrument.exchange() {
                Exchange::Shanghai => "sh",
                Exchange::Shenzhen => "sz",
                Exchange::Beijing => "bj",
            };
            let code = instrument.code();
            if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(TencentError::InvalidRequest(format!(
                    "{} must be a six-digit A-share code",
                    code
                )));
            }
            let symbol = format!("{prefix}{code}");
            if !seen.insert(symbol.clone()) {
                return Err(TencentError::InvalidRequest(format!(
                    "duplicate instrument {symbol}"
                )));
            }
            Ok(symbol)
        })
        .collect()
}

fn validate_statistics_instruments(
    instruments: &[InstrumentId],
) -> Result<Vec<String>, TencentError> {
    if instruments.is_empty() {
        return Err(TencentError::InvalidRequest(
            "instrument list must not be empty".into(),
        ));
    }
    if instruments.len() > MAX_BATCH_SIZE {
        return Err(TencentError::InvalidRequest(format!(
            "at most {MAX_BATCH_SIZE} instruments are accepted per request"
        )));
    }
    let mut seen = HashSet::with_capacity(instruments.len());
    instruments
        .iter()
        .map(|instrument| {
            if !matches!(
                instrument.asset_class(),
                AssetClass::Equity | AssetClass::Index | AssetClass::Fund
            ) {
                return Err(TencentError::Unsupported(format!(
                    "asset class {:?} has no verified Tencent market statistics",
                    instrument.asset_class()
                )));
            }
            if instrument.exchange() == Exchange::Beijing
                && instrument.asset_class() != AssetClass::Equity
            {
                return Err(TencentError::Unsupported(
                    "Beijing index/fund market-statistics identity is unverified".into(),
                ));
            }
            let prefix = match instrument.exchange() {
                Exchange::Shanghai => "sh",
                Exchange::Shenzhen => "sz",
                Exchange::Beijing => "bj",
            };
            let code = instrument.code();
            if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(TencentError::InvalidRequest(format!(
                    "{code} must be a six-digit market code"
                )));
            }
            let symbol = format!("{prefix}{code}");
            if !seen.insert(symbol.clone()) {
                return Err(TencentError::InvalidRequest(format!(
                    "duplicate instrument {symbol}"
                )));
            }
            Ok(symbol)
        })
        .collect()
}

fn parse_response(bytes: &[u8]) -> Result<Vec<Snapshot>, TencentError> {
    parse_response_with_mode(bytes, SnapshotParseMode::Base)
}

fn parse_statistics_response(bytes: &[u8]) -> Result<Vec<Snapshot>, TencentError> {
    parse_response_with_mode(bytes, SnapshotParseMode::Statistics)
}

fn parse_response_with_mode(
    bytes: &[u8],
    mode: SnapshotParseMode,
) -> Result<Vec<Snapshot>, TencentError> {
    if bytes.is_empty() {
        return Err(TencentError::Protocol("empty response".into()));
    }
    let (decoded, _, had_errors) = GBK.decode(bytes);
    if had_errors {
        return Err(TencentError::Decode(
            "response contains invalid GBK byte sequences".into(),
        ));
    }
    let mut snapshots = Vec::new();
    for line in decoded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        snapshots.push(parse_line(line, mode)?);
    }
    if snapshots.is_empty() {
        return Err(TencentError::Protocol(
            "response did not contain any quote records".into(),
        ));
    }
    Ok(snapshots)
}

fn parse_line(line: &str, mode: SnapshotParseMode) -> Result<Snapshot, TencentError> {
    let (variable, encoded) = line.split_once("=\"").ok_or_else(|| {
        TencentError::Protocol("quote line is missing the opening delimiter".into())
    })?;
    let encoded = encoded.strip_suffix("\";").ok_or_else(|| {
        TencentError::Protocol("quote line is missing the closing delimiter".into())
    })?;
    let symbol = variable
        .strip_prefix("v_")
        .filter(|value| value.len() == 8)
        .ok_or_else(|| TencentError::Protocol("quote line has an invalid symbol key".into()))?
        .to_owned();
    let fields: Vec<_> = encoded.split('~').collect();
    if fields.len() <= 37 {
        return Err(TencentError::Protocol(format!(
            "{symbol} has only {} fields; at least 38 are required",
            fields.len()
        )));
    }
    let response_symbol = format!(
        "{}{}",
        match fields[0] {
            "1" => "sh",
            "51" => "sz",
            "62" => "bj",
            other => {
                return Err(TencentError::Protocol(format!(
                    "{symbol} has unverified market code {other}"
                )));
            }
        },
        fields[2]
    );
    if response_symbol != symbol {
        return Err(TencentError::Protocol(format!(
            "symbol key {symbol} contradicts payload {response_symbol}"
        )));
    }

    let mut bids = [(None, None); 5];
    let mut asks = [(None, None); 5];
    for level in 0..5 {
        bids[level] = (
            parse_optional_book_number(fields[9 + level * 2], "bid price")?,
            parse_optional_book_number(fields[10 + level * 2], "bid quantity")?,
        );
        asks[level] = (
            parse_optional_book_number(fields[19 + level * 2], "ask price")?,
            parse_optional_book_number(fields[20 + level * 2], "ask quantity")?,
        );
    }

    let current = parse_nonnegative(fields[3], "current price")?;
    let previous_close = parse_optional_nonnegative(fields[4], "previous close")?;
    let open = parse_optional_nonnegative(fields[5], "open")?;
    let high = parse_optional_nonnegative(fields[33], "high")?;
    let low = parse_optional_nonnegative(fields[34], "low")?;
    let change_percent = parse_optional_number(fields[32], "change percent")?;
    let volume_lots = parse_nonnegative(fields[6], "volume")?;
    validate_quote_shape(
        &symbol,
        current,
        previous_close,
        open,
        high,
        low,
        change_percent,
    )?;
    let statistics = match mode {
        SnapshotParseMode::Base => None,
        SnapshotParseMode::Statistics => parse_snapshot_statistics(&fields, &symbol)?,
    };
    Ok(Snapshot {
        symbol,
        name: optional_text(fields[1]),
        current,
        previous_close,
        open,
        high,
        low,
        change_percent,
        volume_lots,
        amount_yuan: parse_amount(fields[35], current, volume_lots)?,
        bids,
        asks,
        source_at: parse_optional_timestamp(fields[30])?,
        statistics,
    })
}

fn parse_snapshot_statistics(
    fields: &[&str],
    symbol: &str,
) -> Result<Option<SnapshotStatistics>, TencentError> {
    if fields.len() <= 52 {
        return Ok(None);
    }
    let turnover_rate = parse_optional_nonnegative_scalar(fields[38], "turnover rate")?;
    let total_market_cap_yi = parse_optional_nonnegative_scalar(fields[44], "total market cap yi")?;
    let floating_market_cap_yi =
        parse_optional_nonnegative_scalar(fields[45], "floating market cap yi")?;
    let volume_ratio = parse_optional_nonnegative_scalar(fields[49], "volume ratio")?;
    let upper_limit = parse_optional_limit_price(fields[47], "upper limit")?;
    let lower_limit = parse_optional_limit_price(fields[48], "lower limit")?;
    let trailing_pe = parse_optional_number(fields[39], "trailing PE")?;
    let static_pe = parse_optional_number(fields[52], "static PE")?;
    let pb = parse_optional_number(fields[46], "PB")?;
    if let (Some(upper), Some(lower)) = (upper_limit, lower_limit) {
        if upper < lower {
            return Err(TencentError::Protocol(format!(
                "{symbol} upper limit is below lower limit"
            )));
        }
    }
    if [
        turnover_rate,
        trailing_pe,
        static_pe,
        pb,
        total_market_cap_yi,
        floating_market_cap_yi,
        upper_limit,
        lower_limit,
        volume_ratio,
    ]
    .iter()
    .all(Option::is_none)
    {
        return Err(TencentError::Protocol(format!(
            "{symbol} market-statistics extension has no usable fields"
        )));
    }
    Ok(Some(SnapshotStatistics {
        turnover_rate,
        trailing_pe,
        static_pe,
        pb,
        total_market_cap_yi,
        floating_market_cap_yi,
        upper_limit,
        lower_limit,
        volume_ratio,
    }))
}

fn optional_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_number(value: &str, field: &'static str) -> Result<f64, TencentError> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| TencentError::Protocol(format!("{field} is not numeric: {value:?}")))?;
    if !parsed.is_finite() {
        return Err(TencentError::Protocol(format!("{field} must be finite")));
    }
    Ok(parsed)
}

fn parse_nonnegative(value: &str, field: &'static str) -> Result<f64, TencentError> {
    if value.trim().is_empty() {
        return Err(TencentError::Protocol(format!("{field} is missing")));
    }
    let parsed = parse_number(value, field)?;
    if parsed < 0.0 {
        return Err(TencentError::Protocol(format!(
            "{field} must be non-negative"
        )));
    }
    Ok(parsed)
}

fn parse_optional_number(value: &str, field: &'static str) -> Result<Option<f64>, TencentError> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        parse_number(value, field).map(Some)
    }
}

fn parse_optional_nonnegative(
    value: &str,
    field: &'static str,
) -> Result<Option<f64>, TencentError> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let parsed = parse_nonnegative(value, field)?;
    Ok((parsed > 0.0).then_some(parsed))
}

fn parse_optional_nonnegative_scalar(
    value: &str,
    field: &'static str,
) -> Result<Option<f64>, TencentError> {
    let value = parse_optional_number(value, field)?;
    if value.is_some_and(|number| number < 0.0) {
        return Err(TencentError::Protocol(format!(
            "{field} must be non-negative"
        )));
    }
    Ok(value)
}

fn parse_optional_limit_price(
    value: &str,
    field: &'static str,
) -> Result<Option<f64>, TencentError> {
    let Some(value) = parse_optional_number(value, field)? else {
        return Ok(None);
    };
    if value > 0.0 {
        return Ok(Some(value));
    }
    if value == 0.0 || value == -1.0 {
        return Ok(None);
    }
    Err(TencentError::Protocol(format!(
        "{field} has an unknown non-positive sentinel {value}"
    )))
}

fn parse_optional_book_number(
    value: &str,
    field: &'static str,
) -> Result<Option<f64>, TencentError> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        parse_nonnegative(value, field).map(Some)
    }
}

fn parse_amount(composite: &str, current: f64, volume: f64) -> Result<Option<f64>, TencentError> {
    if composite.trim().is_empty() {
        return Ok(None);
    }
    let fields: Vec<_> = composite.split('/').collect();
    if fields.len() != 3 {
        return Err(TencentError::Protocol(
            "price/volume/amount composite must contain three fields".into(),
        ));
    }
    let composite_price = parse_nonnegative(fields[0], "composite price")?;
    let composite_volume = parse_nonnegative(fields[1], "composite volume")?;
    if composite_price != current || composite_volume != volume {
        return Err(TencentError::Protocol(
            "price/volume/amount composite contradicts standalone fields".into(),
        ));
    }
    let amount = parse_nonnegative(fields[2], "amount")?;
    Ok(Some(amount))
}

#[allow(clippy::too_many_arguments)]
fn validate_quote_shape(
    symbol: &str,
    current: f64,
    previous_close: Option<f64>,
    open: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
    change_percent: Option<f64>,
) -> Result<(), TencentError> {
    if let (Some(high), Some(low)) = (high, low) {
        if high < low {
            return Err(TencentError::Protocol(format!(
                "{symbol} high price is below low price"
            )));
        }
        for (label, value) in [
            ("current", (current > 0.0).then_some(current)),
            ("open", open),
        ] {
            if value.is_some_and(|value| value < low || value > high) {
                return Err(TencentError::Protocol(format!(
                    "{symbol} {label} price is outside the daily range"
                )));
            }
        }
    }
    if let (Some(previous_close), Some(change_percent)) = (previous_close, change_percent) {
        if current > 0.0 {
            let expected = (current - previous_close) / previous_close * 100.0;
            if (expected - change_percent).abs() > 0.02 {
                return Err(TencentError::Protocol(format!(
                    "{symbol} change percent contradicts current and previous close"
                )));
            }
        }
    }
    Ok(())
}

fn parse_optional_timestamp(value: &str) -> Result<Option<String>, TencentError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() != 14 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(TencentError::Protocol(format!(
            "source timestamp must use YYYYMMDDHHMMSS: {value:?}"
        )));
    }
    let year = parse_timestamp_part(value, 0, 4)?;
    let month = parse_timestamp_part(value, 4, 6)?;
    let day = parse_timestamp_part(value, 6, 8)?;
    let hour = parse_timestamp_part(value, 8, 10)?;
    let minute = parse_timestamp_part(value, 10, 12)?;
    let second = parse_timestamp_part(value, 12, 14)?;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => 0,
    };
    if year == 0 || day == 0 || day > days || hour > 23 || minute > 59 || second > 59 {
        return Err(TencentError::Protocol(format!(
            "source timestamp is outside calendar/time bounds: {value:?}"
        )));
    }
    Ok(Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}+08:00"
    )))
}

fn parse_timestamp_part(value: &str, start: usize, end: usize) -> Result<u32, TencentError> {
    value[start..end]
        .parse::<u32>()
        .map_err(|_| TencentError::Protocol("source timestamp contains invalid digits".into()))
}

fn order_snapshots(
    symbols: &[String],
    snapshots: Vec<Snapshot>,
) -> Result<Vec<Snapshot>, TencentError> {
    if snapshots.len() != symbols.len() {
        return Err(TencentError::Protocol(format!(
            "cardinality mismatch: requested {}, received {}",
            symbols.len(),
            snapshots.len()
        )));
    }
    let mut indexed = HashMap::with_capacity(snapshots.len());
    for snapshot in snapshots {
        let symbol = snapshot.symbol.clone();
        if indexed.insert(symbol.clone(), snapshot).is_some() {
            return Err(TencentError::Protocol(format!(
                "duplicate response record {symbol}"
            )));
        }
    }
    let mut ordered = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        ordered.push(indexed.remove(symbol).ok_or_else(|| {
            TencentError::Protocol(format!("response omitted requested record {symbol}"))
        })?);
    }
    if !indexed.is_empty() {
        return Err(TencentError::Protocol(
            "response contained an unexpected record".into(),
        ));
    }
    Ok(ordered)
}

fn market_cap_yuan(value: Option<f64>) -> Result<Option<Money>, TencentError> {
    value
        .map(|value| {
            let yuan = value * 100_000_000.0;
            if !yuan.is_finite() {
                return Err(TencentError::Protocol(
                    "market capitalization overflows finite CNY range".into(),
                ));
            }
            Money::new(yuan).map_err(Into::into)
        })
        .transpose()
}

fn source_evidence(
    kind: &str,
    observed_at: &str,
    source_at: Option<&str>,
) -> Result<SourceEvidence, TencentError> {
    let batch_id = format!("tencent-web:{observed_at}:{kind}");
    let mut evidence = SourceEvidence::new(ProviderId::Tencent, observed_at, batch_id)?;
    if let Some(source_at) = source_at {
        evidence = evidence.with_source_at(source_at)?;
    }
    Ok(evidence)
}

pub(crate) fn now() -> Result<String, TencentError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| {
            TencentError::Protocol(format!("system clock precedes UNIX epoch: {error}"))
        })
}

fn optional_price(value: Option<f64>) -> Result<Option<Price>, TencentError> {
    value.map(Price::new).transpose().map_err(Into::into)
}

fn batch_provenance(
    kind: &str,
    observed_at: &str,
    snapshots: &[Snapshot],
) -> Result<magic_market_core::Provenance, TencentError> {
    let batch_id = format!("tencent-web:{observed_at}:{kind}");
    let mut provenance =
        magic_market_core::Provenance::new("tencent-web", observed_at)?.with_batch_id(batch_id)?;
    if snapshots
        .iter()
        .all(|snapshot| snapshot.source_at.is_some())
    {
        let oldest = snapshots
            .iter()
            .filter_map(|snapshot| snapshot.source_at.as_deref())
            .min()
            .ok_or_else(|| TencentError::Protocol("empty snapshot batch".into()))?;
        provenance = provenance.with_source_at(oldest)?;
    }
    Ok(provenance)
}

impl RealtimeQuotes for TencentClient {
    type Quote = Quote;
    type Error = TencentError;

    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let snapshots = self.snapshots(instruments)?;
        let observed_at = now()?;
        let batch_id = format!("tencent-web:{observed_at}:quote");
        let mut records = Vec::with_capacity(snapshots.len());
        let mut issues = Vec::new();
        for (instrument, snapshot) in instruments.iter().zip(&snapshots) {
            let price = Price::new(snapshot.current).map_err(|_| {
                TencentError::Protocol(format!("{} has no positive current price", snapshot.symbol))
            })?;
            let previous_close = optional_price(snapshot.previous_close)?;
            let open = optional_price(snapshot.open)?;
            let high = optional_price(snapshot.high)?;
            let low = optional_price(snapshot.low)?;
            let change_percent = snapshot
                .change_percent
                .map(|value| Ratio::new(value, RatioUnit::Percent))
                .transpose()?;
            let volume = Quantity::new(snapshot.volume_lots)?;
            let amount = snapshot.amount_yuan.map(Money::new).transpose()?;
            let complete = snapshot.name.is_some()
                && previous_close.is_some()
                && open.is_some()
                && high.is_some()
                && low.is_some()
                && change_percent.is_some()
                && amount.is_some()
                && snapshot.source_at.is_some();
            let status = if complete {
                DataStatus::Available
            } else {
                issues.push(format!(
                    "{}: one or more verified quote fields unavailable",
                    instrument.code()
                ));
                DataStatus::Unavailable
            };
            records.push(Quote::from_parts(
                instrument.clone(),
                snapshot.name.clone(),
                price,
                previous_close,
                open,
                high,
                low,
                change_percent,
                volume,
                amount,
                status,
                snapshot.source_at.clone(),
                observed_at.clone(),
                ProviderId::Tencent,
                batch_id.clone(),
            )?);
        }
        let provenance = batch_provenance("quote", &observed_at, &snapshots)?;
        Ok(DataBatch::best_effort(records, provenance, issues)?)
    }
}

impl MarketStatisticsProvider for TencentClient {
    type Error = TencentError;

    fn market_statistics(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<MarketStatistics>, Self::Error> {
        let snapshots = self.statistics_snapshots(instruments)?;
        let observed_at = now()?;
        let mut records = Vec::with_capacity(snapshots.len());
        for (instrument, snapshot) in instruments.iter().zip(&snapshots) {
            let statistics = snapshot.statistics.as_ref().ok_or_else(|| {
                TencentError::Protocol(format!(
                    "{} market-statistics response has fewer than 53 fields",
                    snapshot.symbol
                ))
            })?;
            let turnover_rate = statistics
                .turnover_rate
                .map(|value| Ratio::new(value, RatioUnit::Percent))
                .transpose()?;
            let trailing_pe = statistics.trailing_pe.map(FiniteNumber::new).transpose()?;
            let static_pe = statistics.static_pe.map(FiniteNumber::new).transpose()?;
            let pb = statistics.pb.map(FiniteNumber::new).transpose()?;
            let upper_limit = statistics.upper_limit.map(Price::new).transpose()?;
            let lower_limit = statistics.lower_limit.map(Price::new).transpose()?;
            let volume_ratio = statistics.volume_ratio.map(FiniteNumber::new).transpose()?;
            records.push(MarketStatistics::new(
                instrument.clone(),
                turnover_rate,
                trailing_pe,
                static_pe,
                pb,
                market_cap_yuan(statistics.total_market_cap_yi)?,
                market_cap_yuan(statistics.floating_market_cap_yi)?,
                upper_limit,
                lower_limit,
                volume_ratio,
                source_evidence(
                    "market-statistics",
                    &observed_at,
                    snapshot.source_at.as_deref(),
                )?,
            )?);
        }
        let provenance = batch_provenance("market-statistics", &observed_at, &snapshots)?;
        Ok(DataBatch::strict(records, provenance))
    }
}

fn normalize_level(
    instrument: &InstrumentId,
    side: &str,
    position: usize,
    pair: (Option<f64>, Option<f64>),
    issues: &mut Vec<String>,
) -> Result<BookLevel, TencentError> {
    let (price, quantity) = pair;
    match (price, quantity) {
        (Some(0.0), Some(0.0)) => return Ok(BookLevel::unavailable()),
        (Some(0.0), Some(quantity)) if quantity != 0.0 => {
            issues.push(format!(
                "{}: {side} level {position} has quantity without price",
                instrument.code()
            ));
            return Ok(BookLevel::unavailable());
        }
        (Some(price), Some(quantity)) if price > 0.0 => {
            return Ok(BookLevel::new(
                Some(Price::new(price)?),
                Some(Quantity::new(quantity)?),
            )?);
        }
        _ => {
            issues.push(format!(
                "{}: {side} level {position} is missing price or quantity",
                instrument.code()
            ));
        }
    }
    Ok(BookLevel::unavailable())
}

fn book_total(levels: &[BookLevel; 5]) -> Result<Option<Quantity>, TencentError> {
    let quantities: Vec<_> = levels.iter().filter_map(|level| level.quantity()).collect();
    if quantities.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Quantity::new(
            quantities.into_iter().map(Quantity::get).sum(),
        )?))
    }
}

impl OrderBooks for TencentClient {
    type Error = TencentError;

    fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        let snapshots = self.snapshots(instruments)?;
        let observed_at = now()?;
        let batch_id = format!("tencent-web:{observed_at}:order-book");
        let mut records = Vec::with_capacity(snapshots.len());
        let mut issues = Vec::new();
        for (instrument, snapshot) in instruments.iter().zip(&snapshots) {
            let mut bids = [BookLevel::unavailable(); 5];
            let mut asks = [BookLevel::unavailable(); 5];
            for level in 0..5 {
                bids[level] = normalize_level(
                    instrument,
                    "bid",
                    level + 1,
                    snapshot.bids[level],
                    &mut issues,
                )?;
                asks[level] = normalize_level(
                    instrument,
                    "ask",
                    level + 1,
                    snapshot.asks[level],
                    &mut issues,
                )?;
            }
            let total_bid_quantity = book_total(&bids)?;
            let total_ask_quantity = book_total(&asks)?;
            let complete = bids
                .iter()
                .chain(&asks)
                .all(|level| level.price().is_some())
                && snapshot.source_at.is_some();
            let status = if complete {
                DataStatus::Available
            } else {
                issues.push(format!(
                    "{}: one or more five-level book fields unavailable",
                    instrument.code()
                ));
                DataStatus::Unavailable
            };
            records.push(OrderBook::new(
                instrument.clone(),
                bids,
                asks,
                total_bid_quantity,
                total_ask_quantity,
                status,
                snapshot.source_at.clone(),
                observed_at.clone(),
                ProviderId::Tencent,
                batch_id.clone(),
            )?);
        }
        let provenance = batch_provenance("order-book", &observed_at, &snapshots)?;
        Ok(DataBatch::best_effort(records, provenance, issues)?)
    }
}

fn derived_board(instrument: &InstrumentId) -> Board {
    match instrument.exchange() {
        Exchange::Beijing => Board::Beijing,
        Exchange::Shanghai if instrument.code().starts_with("688") => Board::Star,
        Exchange::Shenzhen
            if instrument.code().starts_with("300") || instrument.code().starts_with("301") =>
        {
            Board::ChiNext
        }
        Exchange::Shanghai | Exchange::Shenzhen => Board::Main,
    }
}

fn st_flag(name: &str) -> bool {
    let uppercase = name.trim().to_ascii_uppercase();
    uppercase.starts_with("ST")
        || uppercase.starts_with("*ST")
        || uppercase.starts_with("S*ST")
        || uppercase.starts_with("SST")
}

impl SecurityMetadataProvider for TencentClient {
    type Error = TencentError;

    fn security_metadata(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        let snapshots = self.snapshots(instruments)?;
        let observed_at = now()?;
        let batch_id = format!("tencent-web:{observed_at}:security-metadata");
        let mut records = Vec::with_capacity(snapshots.len());
        let mut issues = Vec::with_capacity(snapshots.len() * 4);
        for (instrument, snapshot) in instruments.iter().zip(&snapshots) {
            let name = snapshot.name.clone();
            let is_st = name.as_deref().map(st_flag);
            issues.push(format!(
                "{}: board is derived from exchange/code because the Tencent snapshot has no board field",
                instrument.code()
            ));
            issues.push(format!("{}: listing date unavailable", instrument.code()));
            issues.push(format!(
                "{}: source-backed price-limit rule and version unavailable",
                instrument.code()
            ));
            records.push(SecurityMetadata::new(
                instrument.clone(),
                name,
                Some(derived_board(instrument)),
                is_st,
                None,
                PriceLimitRule::new(None, None)?,
                DataStatus::Unavailable,
                snapshot.source_at.clone(),
                observed_at.clone(),
                ProviderId::Tencent,
                batch_id.clone(),
            )?);
        }
        let provenance = batch_provenance("security-metadata", &observed_at, &snapshots)?;
        Ok(DataBatch::best_effort(records, provenance, issues)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SH_LINE: &str = "v_sh600396=\"1~华电辽能~600396~15.47~14.92~15.30~1775070~821130~950794~15.47~212~15.46~95~15.45~64~15.44~3~15.43~375~15.49~49~15.50~2721~15.51~241~15.52~450~15.53~86~~20260723094907~0.55~3.69~15.88~14.85~15.47/1775070/2729507908~1775070~272951~\";";
    const SZ_LINE: &str = "v_sz000001=\"51~平安银行~000001~11.22~11.08~11.10~1000~400~600~11.21~10~11.20~20~11.19~30~11.18~40~11.17~50~11.22~11~11.23~21~11.24~31~11.25~41~11.26~51~~20260723094908~0.14~1.26~11.30~11.01~11.22/1000/1122000~1000~112~\";";
    const BJ_LINE: &str = "v_bj920118=\"62~太湖远大~920118~16.91~16.53~16.44~3240~1810~1430~16.82~1~16.81~4~16.80~12~16.73~4~16.72~8~16.93~30~16.94~2~16.95~8~16.97~5~16.98~10~~20260723114602~0.38~2.30~16.99~16.38~16.91/3240/5425465~3240~542.55~\";";
    const SH_STATS_LINE: &str = "v_sh600396=\"1~华电辽能~600396~16.41~14.92~15.30~3417801~1658577~1759224~16.41~64092~16.40~721~16.39~176~16.38~35~16.37~50~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~~20260723161449~1.49~9.99~16.41~14.85~16.41/3417801/5352355411~3417801~535236~23.21~595.98~~16.41~14.85~10.46~241.67~241.67~15.75~16.41~13.43~1.29~65074~15.66~38.31\";";
    const INDEX_STATS_LINE: &str = "v_sh000001=\"1~上证指数~000001~3876.78~3867.03~3868.09~562122601~0~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~0.00~0~~20260723161420~9.75~0.25~3878.83~3851.71~3876.78/562122601/1025875517700~562122601~102587552~1.16~17.30~~3878.83~3851.71~0.70~602482.43~646317.73~0.00~-1~-1~0.87~0~3867.94~\";";
    const ETF_STATS_LINE: &str = "v_sh510050=\"1~上证50ETF华夏~510050~3.084~3.086~3.104~5858331~2212228~3646103~3.084~30395~3.083~53435~3.082~50356~3.081~21033~3.080~1619~3.085~17569~3.086~14211~3.087~18738~3.088~8069~3.089~20421~~20260723161435~-0.002~-0.06~3.107~3.074~3.084/5858331/1806609827~5858331~180661~7.84~~~3.107~3.074~1.07~230.53~230.53~0.00~3.395~2.777~0.43~77830~3.084~\";";

    #[derive(Clone)]
    struct FixtureTransport {
        response: Vec<u8>,
    }

    struct FailingTransport;
    impl SnapshotTransport for FailingTransport {
        fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
            Err(TencentError::Transport("fixture timeout".into()))
        }
    }
    impl SnapshotTransport for FixtureTransport {
        fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
            Ok(self.response.clone())
        }
    }

    fn encoded(lines: &str) -> Vec<u8> {
        let (bytes, _, had_errors) = GBK.encode(lines);
        assert!(!had_errors);
        bytes.into_owned()
    }

    fn replace_fields(line: &str, replacements: &[(usize, &str)]) -> String {
        let (variable, encoded) = line.split_once("=\"").unwrap();
        let encoded = encoded.strip_suffix("\";").unwrap();
        let mut fields: Vec<_> = encoded.split('~').map(str::to_owned).collect();
        for (index, value) in replacements {
            fields[*index] = (*value).to_owned();
        }
        format!("{variable}=\"{}\";", fields.join("~"))
    }

    fn sh() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
    }
    fn sz() -> InstrumentId {
        InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap()
    }
    fn bj() -> InstrumentId {
        InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap()
    }
    fn index() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).unwrap()
    }
    fn etf() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "510050", AssetClass::Fund).unwrap()
    }

    #[test]
    fn parses_verified_gbk_fields_and_units() {
        let snapshot = parse_response(&encoded(SH_LINE)).unwrap().remove(0);
        assert_eq!(snapshot.name.as_deref(), Some("华电辽能"));
        assert_eq!(
            snapshot.source_at.as_deref(),
            Some("2026-07-23T09:49:07+08:00")
        );
        assert_eq!(snapshot.volume_lots, 1_775_070.0);
        assert_eq!(snapshot.amount_yuan, Some(2_729_507_908.0));
        assert_eq!(snapshot.bids[0], (Some(15.47), Some(212.0)));
        assert_eq!(snapshot.asks[4], (Some(15.53), Some(86.0)));
    }

    #[test]
    fn quote_contract_reorders_response_and_keeps_evidence() {
        let response = encoded(&format!("{SZ_LINE}\n{SH_LINE}\n"));
        let client = TencentClient::with_transport(FixtureTransport { response });
        let batch = client.realtime_quotes(&[sh(), sz()]).unwrap();
        assert_eq!(batch.records().len(), 2);
        assert_eq!(batch.records()[0].instrument(), &sh());
        assert_eq!(batch.records()[0].name(), Some("华电辽能"));
        assert_eq!(batch.records()[0].status(), DataStatus::Available);
        assert_eq!(batch.records()[0].provider(), ProviderId::Tencent);
        assert_eq!(batch.records()[0].volume().get(), 1_775_070.0);
        assert_eq!(
            batch.records()[0].amount().map(Money::get),
            Some(2_729_507_908.0)
        );
        assert_eq!(batch.provenance().source(), "tencent-web");
        assert_eq!(
            batch.provenance().source_at(),
            Some("2026-07-23T09:49:07+08:00")
        );
        assert!(batch.quality().is_complete());
    }

    #[test]
    fn base_capabilities_ignore_malformed_statistics_extension() {
        let malformed = replace_fields(SH_STATS_LINE, &[(38, "not-a-number")]);
        let client = TencentClient::with_transport(FixtureTransport {
            response: encoded(&malformed),
        });

        assert!(client.realtime_quotes(&[sh()]).is_ok());
        assert!(client.order_books(&[sh()]).is_ok());
        assert!(client.security_metadata(&[sh()]).is_ok());
        assert!(matches!(
            client.market_statistics(&[sh()]),
            Err(TencentError::Protocol(message)) if message.contains("turnover rate")
        ));
    }

    #[test]
    fn market_statistics_parse_equity_index_and_etf_without_fabricating_fields() {
        let response = encoded(&format!(
            "{ETF_STATS_LINE}\n{SH_STATS_LINE}\n{INDEX_STATS_LINE}\n"
        ));
        let client = TencentClient::with_transport(FixtureTransport { response });
        let batch = client.market_statistics(&[sh(), index(), etf()]).unwrap();
        assert_eq!(batch.records().len(), 3);

        let equity = &batch.records()[0];
        assert_eq!(equity.instrument(), &sh());
        assert_eq!(equity.turnover_rate().map(Ratio::get), Some(23.21));
        assert_eq!(equity.trailing_pe().map(FiniteNumber::get), Some(595.98));
        assert_eq!(equity.static_pe().map(FiniteNumber::get), Some(38.31));
        assert_eq!(equity.pb().map(FiniteNumber::get), Some(15.75));
        assert_eq!(
            equity.total_market_cap().map(Money::get),
            Some(24_167_000_000.0)
        );
        assert_eq!(
            equity.floating_market_cap().map(Money::get),
            Some(24_167_000_000.0)
        );
        assert_eq!(equity.upper_limit().map(Price::get), Some(16.41));
        assert_eq!(equity.lower_limit().map(Price::get), Some(13.43));
        assert_eq!(equity.volume_ratio().map(FiniteNumber::get), Some(1.29));

        let index = &batch.records()[1];
        assert_eq!(index.instrument(), &self::index());
        assert_eq!(index.pb().map(FiniteNumber::get), Some(0.0));
        assert!(index.upper_limit().is_none());
        assert!(index.lower_limit().is_none());
        assert!(index.static_pe().is_none());

        let etf = &batch.records()[2];
        assert_eq!(etf.instrument(), &self::etf());
        assert!(etf.trailing_pe().is_none());
        assert_eq!(etf.pb().map(FiniteNumber::get), Some(0.0));
        assert_eq!(etf.upper_limit().map(Price::get), Some(3.395));
        assert!(batch.quality().is_complete());
        assert_eq!(batch.provenance().source(), "tencent-web");
    }

    #[test]
    fn market_statistics_require_extended_fields_and_explicit_supported_assets() {
        let client = TencentClient::with_transport(FixtureTransport {
            response: encoded(SH_LINE),
        });
        assert!(matches!(
            client.market_statistics(&[sh()]),
            Err(TencentError::Protocol(message)) if message.contains("market-statistics")
        ));

        let beijing_index =
            InstrumentId::new(Exchange::Beijing, "899050", AssetClass::Index).unwrap();
        assert!(matches!(
            client.market_statistics(&[beijing_index]),
            Err(TencentError::Unsupported(_))
        ));
    }

    #[test]
    fn market_statistics_reject_an_all_empty_extension() {
        let empty = replace_fields(
            SH_STATS_LINE,
            &[
                (38, ""),
                (39, ""),
                (44, ""),
                (45, ""),
                (46, ""),
                (47, ""),
                (48, ""),
                (49, ""),
                (52, ""),
            ],
        );
        let client = TencentClient::with_transport(FixtureTransport {
            response: encoded(&empty),
        });

        assert!(matches!(
            client.market_statistics(&[sh()]),
            Err(TencentError::Protocol(message))
                if message.contains("market-statistics") && message.contains("no usable")
        ));
    }

    #[test]
    fn order_book_contract_exposes_five_levels_and_exact_totals() {
        let client = TencentClient::with_transport(FixtureTransport {
            response: encoded(SH_LINE),
        });
        let batch = client.order_books(&[sh()]).unwrap();
        let book = &batch.records()[0];
        assert_eq!(book.status(), DataStatus::Available);
        assert_eq!(book.bids()[0].price().map(Price::get), Some(15.47));
        assert_eq!(book.total_bid_quantity().map(Quantity::get), Some(749.0));
        assert_eq!(book.total_ask_quantity().map(Quantity::get), Some(3547.0));
        assert!(batch.quality().is_complete());
    }

    #[test]
    fn missing_book_side_is_explicitly_unavailable() {
        let line = SH_LINE.replace("~15.49~49~", "~0~49~");
        let client = TencentClient::with_transport(FixtureTransport {
            response: encoded(&line),
        });
        let batch = client.order_books(&[sh()]).unwrap();
        assert_eq!(batch.records()[0].status(), DataStatus::Unavailable);
        assert!(batch.records()[0].asks()[0].price().is_none());
        assert!(!batch.quality().is_complete());
        assert!(batch
            .quality()
            .issues()
            .iter()
            .any(|issue| issue.contains("quantity without price")));
    }

    #[test]
    fn rejects_duplicates_and_cardinality_mismatch() {
        let client = TencentClient::with_transport(FixtureTransport {
            response: encoded(SH_LINE),
        });
        assert!(matches!(
            client.realtime_quotes(&[sh(), sh()]),
            Err(TencentError::InvalidRequest(_))
        ));
        assert!(matches!(
            client.realtime_quotes(&[sh(), sz()]),
            Err(TencentError::Protocol(_))
        ));
    }

    #[test]
    fn beijing_quote_book_and_metadata_use_verified_market_code() {
        let client = TencentClient::with_transport(FixtureTransport {
            response: encoded(BJ_LINE),
        });
        let quotes = client.realtime_quotes(&[bj()]).unwrap();
        assert_eq!(
            quotes.records()[0].instrument().exchange(),
            Exchange::Beijing
        );
        assert_eq!(quotes.records()[0].name(), Some("太湖远大"));
        let books = client.order_books(&[bj()]).unwrap();
        assert_eq!(
            books.records()[0].bids()[0].price().map(Price::get),
            Some(16.82)
        );
        let metadata = client.security_metadata(&[bj()]).unwrap();
        assert_eq!(metadata.records()[0].board(), Some(Board::Beijing));
        assert!(metadata.records()[0].listed_on().is_none());
        assert!(!metadata.quality().is_complete());
    }

    #[test]
    fn security_metadata_st_flag_uses_name_prefix_only() {
        assert!(st_flag("*ST示例"));
        assert!(st_flag("ST示例"));
        assert!(!st_flag("BEST示例"));
    }

    #[test]
    fn rejects_invalid_timestamp_and_invalid_gbk() {
        let bad_time = SH_LINE.replace("20260723094907", "20260230094907");
        assert!(matches!(
            parse_response(&encoded(&bad_time)),
            Err(TencentError::Protocol(_))
        ));
        assert!(matches!(
            parse_response(&[0x81]),
            Err(TencentError::Decode(_))
        ));
    }

    #[test]
    fn rejects_contradictory_composite_and_propagates_transport_failure() {
        let bad_composite = SH_LINE.replace("15.47/1775070/2729507908", "15.47/1775071/2729507908");
        assert!(matches!(
            parse_response(&encoded(&bad_composite)),
            Err(TencentError::Protocol(_))
        ));
        let client = TencentClient::with_transport(FailingTransport);
        assert!(matches!(
            client.realtime_quotes(&[sh()]),
            Err(TencentError::Transport(message)) if message == "fixture timeout"
        ));
    }

    #[test]
    fn rejects_contradictory_quote_price_shape() {
        let bad_range = SH_LINE.replace("~15.88~14.85~", "~14.00~16.00~");
        assert!(matches!(
            parse_response(&encoded(&bad_range)),
            Err(TencentError::Protocol(_))
        ));
        let bad_change = SH_LINE.replace("~0.55~3.69~", "~0.55~99.00~");
        assert!(matches!(
            parse_response(&encoded(&bad_change)),
            Err(TencentError::Protocol(_))
        ));
    }

    #[test]
    fn missing_numeric_fields_never_become_available_zeroes() {
        let missing_volume = SH_LINE.replace("~1775070~821130~", "~~821130~");
        assert!(matches!(
            parse_response(&encoded(&missing_volume)),
            Err(TencentError::Protocol(_))
        ));
        let missing_amount = SH_LINE.replace("15.47/1775070/2729507908", "15.47/1775070/");
        assert!(matches!(
            parse_response(&encoded(&missing_amount)),
            Err(TencentError::Protocol(_))
        ));

        let missing_bid_quantity = SH_LINE.replace("~15.47~212~", "~15.47~~");
        let client = TencentClient::with_transport(FixtureTransport {
            response: encoded(&missing_bid_quantity),
        });
        let batch = client.order_books(&[sh()]).unwrap();
        assert_eq!(batch.records()[0].status(), DataStatus::Unavailable);
        assert!(batch.records()[0].bids()[0].quantity().is_none());
        assert!(!batch.quality().is_complete());
    }

    #[test]
    fn incomplete_record_does_not_promote_another_records_source_time_to_batch() {
        let missing_time = SZ_LINE.replace("20260723094908", "");
        let response = encoded(&format!("{SH_LINE}\n{missing_time}\n"));
        let client = TencentClient::with_transport(FixtureTransport { response });
        let batch = client.realtime_quotes(&[sh(), sz()]).unwrap();
        assert_eq!(batch.records()[0].status(), DataStatus::Available);
        assert_eq!(batch.records()[1].status(), DataStatus::Unavailable);
        assert!(batch.provenance().source_at().is_none());
        assert!(!batch.quality().is_complete());
    }

    #[test]
    fn capabilities_do_not_claim_unverified_families() {
        let capabilities = TencentClient::capabilities();
        assert!(capabilities.quotes);
        assert!(capabilities.order_book);
        assert!(capabilities.bars);
        assert!(capabilities.minute);
        assert!(capabilities.trades);
        assert!(capabilities.security_metadata);
        assert!(!capabilities.money_flow);
        assert!(!capabilities.auction);
    }
}
