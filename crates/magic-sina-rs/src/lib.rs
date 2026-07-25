#![forbid(unsafe_code)]
//! Read-only supplemental adapter for Sina's public market-data endpoints.
//!
//! Sina's public-web endpoints do not publish a project-visible SLA. This
//! crate therefore advertises only response families covered by strict
//! deterministic parsers and real probes.

mod bars;
mod financials;
mod minute;
mod news;
mod options;

use encoding_rs::GB18030;
use magic_market_core::{
    AssetClass, Board, BookLevel, Capabilities, DataBatch, DataStatus, Exchange, InstrumentId,
    Money, OrderBook, OrderBooks, Price, PriceLimitRule, ProviderId, Quantity, Quote, Ratio,
    RatioUnit, RealtimeQuotes, SecurityMetadata, SecurityMetadataProvider,
};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const DEFAULT_ENDPOINT: &str = "https://hq.sinajs.cn/list=";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BATCH_SIZE: usize = 50;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const SHARES_PER_LOT: f64 = 100.0;

/// Errors raised by the Sina supplemental provider.
#[derive(Debug, Error)]
pub enum SinaError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("Sina response decoding failed: {0}")]
    Decode(String),
    #[error("Sina protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

/// One bounded HTTP document returned with the metadata needed by strict
/// presentation-page adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentResponse {
    status: u16,
    content_type: String,
    body: Vec<u8>,
    observed_unix_seconds: u64,
}

impl DocumentResponse {
    pub fn new(
        status: u16,
        content_type: impl Into<String>,
        body: Vec<u8>,
        observed_unix_seconds: u64,
    ) -> Self {
        Self {
            status,
            content_type: content_type.into(),
            body,
            observed_unix_seconds,
        }
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn observed_unix_seconds(&self) -> u64 {
        self.observed_unix_seconds
    }
}

/// Bounded byte transport used by the adapter and deterministic fixtures.
pub trait SnapshotTransport: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, SinaError>;

    fn get_with_referer(&self, url: &str, _referer: &str) -> Result<Vec<u8>, SinaError> {
        self.get(url)
    }

    fn get_document(&self, _url: &str) -> Result<DocumentResponse, SinaError> {
        Err(SinaError::Unsupported(
            "transport does not expose HTTP document metadata".into(),
        ))
    }
}

#[derive(Clone)]
struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    fn new(timeout: Duration) -> Result<Self, SinaError> {
        if timeout.is_zero() {
            return Err(SinaError::InvalidRequest(
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

    fn request_document(&self, url: &str, referer: &str) -> Result<DocumentResponse, SinaError> {
        if !url.starts_with("https://") {
            return Err(SinaError::InvalidRequest(
                "Sina endpoint must use HTTPS".into(),
            ));
        }
        if !referer.starts_with("https://") {
            return Err(SinaError::InvalidRequest(
                "Sina Referer must use HTTPS".into(),
            ));
        }
        let response = self
            .agent
            .get(url)
            .set("Referer", referer)
            .set("User-Agent", "magic-sina-rs/0.2")
            .call()
            .map_err(|error| SinaError::Transport(error.to_string()))?;
        if response.status() != 200 {
            return Err(SinaError::Transport(format!(
                "unexpected HTTP status {}",
                response.status()
            )));
        }
        let status = response.status();
        let content_type = response
            .header("Content-Type")
            .unwrap_or_default()
            .to_owned();
        let observed_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                SinaError::Protocol(format!("system clock precedes UNIX epoch: {error}"))
            })?
            .as_secs();
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|error| SinaError::Transport(error.to_string()))?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(SinaError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        Ok(DocumentResponse::new(
            status,
            content_type,
            body,
            observed_unix_seconds,
        ))
    }

    fn request(&self, url: &str, referer: &str) -> Result<Vec<u8>, SinaError> {
        Ok(self.request_document(url, referer)?.body)
    }
}

impl SnapshotTransport for HttpsTransport {
    fn get(&self, url: &str) -> Result<Vec<u8>, SinaError> {
        self.request(url, "https://finance.sina.com.cn/")
    }

    fn get_with_referer(&self, url: &str, referer: &str) -> Result<Vec<u8>, SinaError> {
        self.request(url, referer)
    }

    fn get_document(&self, url: &str) -> Result<DocumentResponse, SinaError> {
        self.request_document(url, "https://finance.sina.com.cn/")
    }
}

/// Read-only Sina market-data client. Clones share one pooled HTTPS agent.
#[derive(Clone)]
pub struct SinaClient {
    endpoint: String,
    pub(crate) transport: Arc<dyn SnapshotTransport>,
}

impl std::fmt::Debug for SinaClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SinaClient")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl SinaClient {
    /// Creates a client with bounded connect/read/write timeouts.
    pub fn new() -> Result<Self, SinaError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    /// Creates a client with a caller-selected positive timeout.
    pub fn with_timeout(timeout: Duration) -> Result<Self, SinaError> {
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

    /// Reports only capabilities proved by parsers and real probes.
    pub const fn capabilities() -> Capabilities {
        Capabilities {
            quotes: true,
            bars: true,
            minute: true,
            trades: false,
            fundamentals: true,
            corporate_actions: false,
            blocks: false,
            money_flow: false,
            order_book: true,
            auction: false,
            security_metadata: true,
        }
    }

    fn snapshots(&self, instruments: &[InstrumentId]) -> Result<Vec<Snapshot>, SinaError> {
        let symbols = validate_instruments(instruments)?;
        let response = self
            .transport
            .get(&format!("{}{}", self.endpoint, symbols.join(",")))?;
        order_snapshots(instruments, parse_response(&response)?)
    }
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
}

pub(crate) fn validate_instruments(instruments: &[InstrumentId]) -> Result<Vec<String>, SinaError> {
    if instruments.is_empty() {
        return Err(SinaError::InvalidRequest(
            "instrument list must not be empty".into(),
        ));
    }
    if instruments.len() > MAX_BATCH_SIZE {
        return Err(SinaError::InvalidRequest(format!(
            "at most {MAX_BATCH_SIZE} instruments are accepted per request"
        )));
    }
    let mut seen = HashSet::with_capacity(instruments.len());
    instruments
        .iter()
        .map(|instrument| {
            if instrument.asset_class() != AssetClass::Equity {
                return Err(SinaError::Unsupported(format!(
                    "asset class {:?} has unverified Sina field units",
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
                return Err(SinaError::InvalidRequest(format!(
                    "{code} must be a six-digit A-share code"
                )));
            }
            let symbol = format!("{prefix}{code}");
            if !seen.insert(symbol.clone()) {
                return Err(SinaError::InvalidRequest(format!(
                    "duplicate instrument {symbol}"
                )));
            }
            Ok(symbol)
        })
        .collect()
}

fn parse_response(bytes: &[u8]) -> Result<Vec<Snapshot>, SinaError> {
    if bytes.is_empty() {
        return Err(SinaError::Protocol("empty response".into()));
    }
    let (decoded, _, had_errors) = GB18030.decode(bytes);
    if had_errors {
        return Err(SinaError::Decode(
            "response contains invalid GB18030 byte sequences".into(),
        ));
    }
    let mut snapshots = Vec::new();
    for line in decoded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        snapshots.push(parse_line(line)?);
    }
    if snapshots.is_empty() {
        return Err(SinaError::Protocol(
            "response did not contain any quote records".into(),
        ));
    }
    Ok(snapshots)
}

fn parse_line(line: &str) -> Result<Snapshot, SinaError> {
    let (variable, encoded) = line
        .split_once("=\"")
        .ok_or_else(|| SinaError::Protocol("quote line is missing the opening delimiter".into()))?;
    let encoded = encoded
        .strip_suffix("\";")
        .ok_or_else(|| SinaError::Protocol("quote line is missing the closing delimiter".into()))?;
    let symbol = variable
        .strip_prefix("var hq_str_")
        .filter(|value| {
            value.len() == 8
                && matches!(&value[0..2], "sh" | "sz" | "bj")
                && value[2..].bytes().all(|byte| byte.is_ascii_digit())
        })
        .ok_or_else(|| SinaError::Protocol("quote line has an invalid symbol key".into()))?
        .to_owned();
    let fields: Vec<_> = encoded.split(',').collect();
    if fields.len() < 33 {
        return Err(SinaError::Protocol(format!(
            "{symbol} has only {} fields; at least 33 are required",
            fields.len()
        )));
    }
    if fields[32].trim().is_empty() {
        return Err(SinaError::Protocol(format!(
            "{symbol} has no market-status field"
        )));
    }

    let mut bids = [(None, None); 5];
    let mut asks = [(None, None); 5];
    for level in 0..5 {
        bids[level] = parse_book_pair(fields[11 + level * 2], fields[10 + level * 2], "bid")?;
        asks[level] = parse_book_pair(fields[21 + level * 2], fields[20 + level * 2], "ask")?;
    }
    validate_top_of_book("bid", parse_nonnegative(fields[6], "best bid")?, bids[0].0)?;
    validate_top_of_book("ask", parse_nonnegative(fields[7], "best ask")?, asks[0].0)?;

    let current = parse_positive(fields[3], "current price")?;
    let previous_close = parse_optional_positive(fields[2], "previous close")?;
    let open = parse_optional_positive(fields[1], "open")?;
    let high = parse_optional_positive(fields[4], "high")?;
    let low = parse_optional_positive(fields[5], "low")?;
    validate_quote_shape(&symbol, current, open, high, low)?;
    let change_percent = previous_close.map(|value| (current - value) / value * 100.0);
    let source_at = parse_optional_timestamp(fields[30], fields[31])?;
    Ok(Snapshot {
        symbol,
        name: optional_text(fields[0]),
        current,
        previous_close,
        open,
        high,
        low,
        change_percent,
        volume_lots: shares_to_lots(parse_nonnegative(fields[8], "volume shares")?)?,
        amount_yuan: parse_optional_nonnegative(fields[9], "amount yuan")?,
        bids,
        asks,
        source_at,
    })
}

fn optional_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_number(value: &str, field: &'static str) -> Result<f64, SinaError> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| SinaError::Protocol(format!("{field} is not numeric: {value:?}")))?;
    if !parsed.is_finite() {
        return Err(SinaError::Protocol(format!("{field} must be finite")));
    }
    Ok(parsed)
}

fn parse_nonnegative(value: &str, field: &'static str) -> Result<f64, SinaError> {
    if value.trim().is_empty() {
        return Err(SinaError::Protocol(format!("{field} is missing")));
    }
    let parsed = parse_number(value, field)?;
    if parsed < 0.0 {
        return Err(SinaError::Protocol(format!("{field} must be non-negative")));
    }
    Ok(parsed)
}

fn parse_positive(value: &str, field: &'static str) -> Result<f64, SinaError> {
    let parsed = parse_nonnegative(value, field)?;
    if parsed <= 0.0 {
        return Err(SinaError::Protocol(format!("{field} must be positive")));
    }
    Ok(parsed)
}

fn parse_optional_nonnegative(value: &str, field: &'static str) -> Result<Option<f64>, SinaError> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        parse_nonnegative(value, field).map(Some)
    }
}

fn parse_optional_positive(value: &str, field: &'static str) -> Result<Option<f64>, SinaError> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let parsed = parse_nonnegative(value, field)?;
    Ok((parsed > 0.0).then_some(parsed))
}

fn parse_book_pair(
    price: &str,
    quantity_shares: &str,
    side: &'static str,
) -> Result<(Option<f64>, Option<f64>), SinaError> {
    let price = parse_optional_nonnegative(price, "book price")?;
    let quantity = parse_optional_nonnegative(quantity_shares, "book quantity shares")?
        .map(shares_to_lots)
        .transpose()?;
    if price.is_none() != quantity.is_none() {
        return Err(SinaError::Protocol(format!(
            "{side} book price and quantity presence differ"
        )));
    }
    Ok((price, quantity))
}

pub(crate) fn shares_to_lots(shares: f64) -> Result<f64, SinaError> {
    if !shares.is_finite() || shares < 0.0 {
        return Err(SinaError::Protocol(
            "share quantity must be finite and non-negative".into(),
        ));
    }
    Ok(shares / SHARES_PER_LOT)
}

fn validate_top_of_book(
    side: &'static str,
    summary: f64,
    level_price: Option<f64>,
) -> Result<(), SinaError> {
    if level_price.is_some_and(|price| (summary - price).abs() > 0.000_001) {
        return Err(SinaError::Protocol(format!(
            "{side} summary price contradicts level one"
        )));
    }
    Ok(())
}

fn validate_quote_shape(
    symbol: &str,
    current: f64,
    open: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
) -> Result<(), SinaError> {
    if let (Some(high), Some(low)) = (high, low) {
        if high < low {
            return Err(SinaError::Protocol(format!(
                "{symbol} high price is below low price"
            )));
        }
        for (label, value) in [("current", Some(current)), ("open", open)] {
            if value.is_some_and(|price| price < low || price > high) {
                return Err(SinaError::Protocol(format!(
                    "{symbol} {label} price is outside the daily range"
                )));
            }
        }
    }
    Ok(())
}

fn parse_optional_timestamp(date: &str, time: &str) -> Result<Option<String>, SinaError> {
    let date = date.trim();
    let time = time.trim();
    if date.is_empty() && time.is_empty() {
        return Ok(None);
    }
    if !valid_date(date) || !valid_time(time) {
        return Err(SinaError::Protocol(format!(
            "source timestamp is outside calendar/time bounds: {date:?} {time:?}"
        )));
    }
    Ok(Some(format!("{date}T{time}+08:00")))
}

pub(crate) fn valid_date(value: &str) -> bool {
    if value.len() != 10
        || value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || !value
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }
    let year = value[0..4].parse::<u32>().unwrap_or(0);
    let month = value[5..7].parse::<u32>().unwrap_or(0);
    let day = value[8..10].parse::<u32>().unwrap_or(0);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    year >= 1900 && day > 0 && day <= max_day
}

pub(crate) fn valid_time(value: &str) -> bool {
    if value.len() != 8
        || value.as_bytes()[2] != b':'
        || value.as_bytes()[5] != b':'
        || !value
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 2 || index == 5 || byte.is_ascii_digit())
    {
        return false;
    }
    value[0..2].parse::<u8>().is_ok_and(|hour| hour < 24)
        && value[3..5].parse::<u8>().is_ok_and(|minute| minute < 60)
        && value[6..8].parse::<u8>().is_ok_and(|second| second < 60)
}

fn order_snapshots(
    instruments: &[InstrumentId],
    snapshots: Vec<Snapshot>,
) -> Result<Vec<Snapshot>, SinaError> {
    let symbols = validate_instruments(instruments)?;
    if snapshots.len() != symbols.len() {
        return Err(SinaError::Protocol(format!(
            "cardinality mismatch: requested {}, received {}",
            symbols.len(),
            snapshots.len()
        )));
    }
    let mut indexed = HashMap::with_capacity(snapshots.len());
    for snapshot in snapshots {
        let symbol = snapshot.symbol.clone();
        if indexed.insert(symbol.clone(), snapshot).is_some() {
            return Err(SinaError::Protocol(format!(
                "duplicate response record {symbol}"
            )));
        }
    }
    let mut ordered = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        ordered.push(indexed.remove(&symbol).ok_or_else(|| {
            SinaError::Protocol(format!("response omitted requested record {symbol}"))
        })?);
    }
    if !indexed.is_empty() {
        return Err(SinaError::Protocol(
            "response contained an unexpected record".into(),
        ));
    }
    Ok(ordered)
}

pub(crate) fn now() -> Result<String, SinaError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| SinaError::Protocol(format!("system clock precedes UNIX epoch: {error}")))
}

fn optional_price(value: Option<f64>) -> Result<Option<Price>, SinaError> {
    value.map(Price::new).transpose().map_err(Into::into)
}

fn batch_provenance(
    kind: &str,
    observed_at: &str,
    snapshots: &[Snapshot],
) -> Result<magic_market_core::Provenance, SinaError> {
    let batch_id = format!("sina-web:{observed_at}:{kind}");
    let mut provenance =
        magic_market_core::Provenance::new("sina-web", observed_at)?.with_batch_id(batch_id)?;
    if snapshots
        .iter()
        .all(|snapshot| snapshot.source_at.is_some())
    {
        let oldest = snapshots
            .iter()
            .filter_map(|snapshot| snapshot.source_at.as_deref())
            .min()
            .ok_or_else(|| SinaError::Protocol("empty snapshot batch".into()))?;
        provenance = provenance.with_source_at(oldest)?;
    }
    Ok(provenance)
}

impl RealtimeQuotes for SinaClient {
    type Quote = Quote;
    type Error = SinaError;

    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let snapshots = self.snapshots(instruments)?;
        let observed_at = now()?;
        let batch_id = format!("sina-web:{observed_at}:quote");
        let mut records = Vec::with_capacity(snapshots.len());
        let mut issues = Vec::new();
        for (instrument, snapshot) in instruments.iter().zip(&snapshots) {
            let previous_close = optional_price(snapshot.previous_close)?;
            let open = optional_price(snapshot.open)?;
            let high = optional_price(snapshot.high)?;
            let low = optional_price(snapshot.low)?;
            let change_percent = snapshot
                .change_percent
                .map(|value| Ratio::new(value, RatioUnit::Percent))
                .transpose()?;
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
                Price::new(snapshot.current)?,
                previous_close,
                open,
                high,
                low,
                change_percent,
                Quantity::new(snapshot.volume_lots)?,
                amount,
                status,
                snapshot.source_at.clone(),
                observed_at.clone(),
                ProviderId::Sina,
                batch_id.clone(),
            )?);
        }
        let provenance = batch_provenance("quote", &observed_at, &snapshots)?;
        Ok(DataBatch::best_effort(records, provenance, issues)?)
    }
}

fn normalize_level(
    instrument: &InstrumentId,
    side: &str,
    position: usize,
    pair: (Option<f64>, Option<f64>),
    issues: &mut Vec<String>,
) -> Result<BookLevel, SinaError> {
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
        (Some(price), Some(0.0)) if price > 0.0 => {
            issues.push(format!(
                "{}: {side} level {position} has price without quantity",
                instrument.code()
            ));
            return Ok(BookLevel::unavailable());
        }
        (Some(price), Some(quantity)) if price > 0.0 && quantity > 0.0 => {
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

fn book_total(levels: &[BookLevel; 5]) -> Result<Option<Quantity>, SinaError> {
    let quantities: Vec<_> = levels.iter().filter_map(|level| level.quantity()).collect();
    if quantities.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Quantity::new(
            quantities.into_iter().map(Quantity::get).sum(),
        )?))
    }
}

impl OrderBooks for SinaClient {
    type Error = SinaError;

    fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        let snapshots = self.snapshots(instruments)?;
        let observed_at = now()?;
        let batch_id = format!("sina-web:{observed_at}:order-book");
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
                ProviderId::Sina,
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

impl SecurityMetadataProvider for SinaClient {
    type Error = SinaError;

    fn security_metadata(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        let snapshots = self.snapshots(instruments)?;
        let observed_at = now()?;
        let batch_id = format!("sina-web:{observed_at}:security-metadata");
        let mut records = Vec::with_capacity(snapshots.len());
        let mut issues = Vec::with_capacity(snapshots.len() * 4);
        for (instrument, snapshot) in instruments.iter().zip(&snapshots) {
            let name = snapshot.name.clone();
            let is_st = name.as_deref().map(st_flag);
            issues.push(format!(
                "{}: board is derived from exchange/code because the Sina snapshot has no board field",
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
                ProviderId::Sina,
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
    use encoding_rs::GB18030;
    use magic_market_core::{
        AssetClass, Board, DataStatus, Exchange, Money, Price, ProviderId, Quantity,
    };

    const SH_LINE: &str = "var hq_str_sh600396=\"华电辽能,15.300,14.920,16.410,16.410,14.850,16.410,0.000,341780059,5352355411.000,6409200,16.410,72100,16.400,17600,16.390,3500,16.380,5000,16.370,0,0.000,0,0.000,0,0.000,0,0.000,0,0.000,2026-07-23,15:34:59,00,D|402000|6596820.00\";";
    const SZ_LINE: &str = "var hq_str_sz000001=\"平安银行,10.920,10.980,11.080,11.120,10.900,11.070,11.080,109574268,1210838024.380,238200,11.070,173100,11.060,441300,11.050,88100,11.040,163594,11.030,15464,11.080,689371,11.090,2364064,11.100,1244300,11.110,2108997,11.120,2026-07-23,15:36:00,00,D|21253|235483.240\";";
    const BJ_LINE: &str = "var hq_str_bj920118=\"太湖远大,16.440,16.530,17.260,17.360,16.380,17.250,17.260,588716,9976849.670,75313,17.250,378,17.240,1000,17.230,500,17.150,2500,17.020,2038,17.260,7183,17.280,3002,17.290,1113,17.300,1200,17.320,2026-07-23,15:30:02,00,33.2261,0.0000,0,8300000,B,T\";";

    #[derive(Clone)]
    struct FixtureTransport {
        response: Vec<u8>,
    }

    impl SnapshotTransport for FixtureTransport {
        fn get(&self, _url: &str) -> Result<Vec<u8>, SinaError> {
            Ok(self.response.clone())
        }
    }

    struct FailingTransport;

    impl SnapshotTransport for FailingTransport {
        fn get(&self, _url: &str) -> Result<Vec<u8>, SinaError> {
            Err(SinaError::Transport("fixture timeout".into()))
        }
    }

    fn encoded(value: &str) -> Vec<u8> {
        let (bytes, _, had_errors) = GB18030.encode(value);
        assert!(!had_errors);
        bytes.into_owned()
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

    #[test]
    fn parses_verified_gb18030_fields_and_share_units() {
        let snapshot = parse_response(&encoded(SH_LINE)).unwrap().remove(0);
        assert_eq!(snapshot.name.as_deref(), Some("华电辽能"));
        assert_eq!(
            snapshot.source_at.as_deref(),
            Some("2026-07-23T15:34:59+08:00")
        );
        assert_eq!(snapshot.volume_lots, 3_417_800.59);
        assert_eq!(snapshot.amount_yuan, Some(5_352_355_411.0));
        assert_eq!(snapshot.bids[0], (Some(16.41), Some(64_092.0)));
        assert_eq!(snapshot.asks[0], (Some(0.0), Some(0.0)));
    }

    #[test]
    fn quote_contract_reorders_records_and_keeps_evidence() {
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded(&format!("{SZ_LINE}\n{SH_LINE}\n")),
        });
        let batch = client.realtime_quotes(&[sh(), sz()]).unwrap();
        assert_eq!(batch.records().len(), 2);
        assert_eq!(batch.records()[0].instrument(), &sh());
        assert_eq!(batch.records()[0].name(), Some("华电辽能"));
        assert_eq!(batch.records()[0].status(), DataStatus::Available);
        assert_eq!(batch.records()[0].provider(), ProviderId::Sina);
        assert_eq!(batch.records()[0].volume().get(), 3_417_800.59);
        assert_eq!(
            batch.records()[0].amount().map(Money::get),
            Some(5_352_355_411.0)
        );
        assert_eq!(batch.provenance().source(), "sina-web");
        assert_eq!(
            batch.provenance().source_at(),
            Some("2026-07-23T15:34:59+08:00")
        );
        assert!(batch.quality().is_complete());
    }

    #[test]
    fn order_book_exposes_five_levels_in_lots() {
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded(SZ_LINE),
        });
        let batch = client.order_books(&[sz()]).unwrap();
        let book = &batch.records()[0];
        assert_eq!(book.status(), DataStatus::Available);
        assert_eq!(book.bids()[0].price().map(Price::get), Some(11.07));
        assert_eq!(book.bids()[0].quantity().map(Quantity::get), Some(2_382.0));
        assert_eq!(
            book.total_bid_quantity().map(Quantity::get),
            Some(11_042.94)
        );
        assert!(batch.quality().is_complete());
    }

    #[test]
    fn limit_up_empty_ask_side_is_explicitly_unavailable() {
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded(SH_LINE),
        });
        let batch = client.order_books(&[sh()]).unwrap();
        assert_eq!(batch.records()[0].status(), DataStatus::Unavailable);
        assert!(batch.records()[0].asks()[0].price().is_none());
        assert!(!batch.quality().is_complete());
    }

    #[test]
    fn rejects_duplicate_requests_and_cardinality_mismatch() {
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded(SH_LINE),
        });
        assert!(matches!(
            client.realtime_quotes(&[sh(), sh()]),
            Err(SinaError::InvalidRequest(_))
        ));
        assert!(matches!(
            client.realtime_quotes(&[sh(), sz()]),
            Err(SinaError::Protocol(_))
        ));
    }

    #[test]
    fn supports_verified_beijing_key_and_partial_metadata() {
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded(BJ_LINE),
        });
        let quotes = client.realtime_quotes(&[bj()]).unwrap();
        assert_eq!(quotes.records()[0].name(), Some("太湖远大"));
        let books = client.order_books(&[bj()]).unwrap();
        assert_eq!(
            books.records()[0].bids()[0].quantity().map(Quantity::get),
            Some(753.13)
        );
        let metadata = client.security_metadata(&[bj()]).unwrap();
        assert_eq!(metadata.records()[0].board(), Some(Board::Beijing));
        assert!(metadata.records()[0].listed_on().is_none());
        assert!(!metadata.quality().is_complete());
    }

    #[test]
    fn rejects_invalid_encoding_timestamp_and_price_shape() {
        assert!(matches!(parse_response(&[0x81]), Err(SinaError::Decode(_))));
        let bad_time = SH_LINE.replace("2026-07-23,15:34:59", "2026-02-30,15:34:59");
        assert!(matches!(
            parse_response(&encoded(&bad_time)),
            Err(SinaError::Protocol(_))
        ));
        let bad_range = SH_LINE.replace(",16.410,14.850,", ",14.000,16.000,");
        assert!(matches!(
            parse_response(&encoded(&bad_range)),
            Err(SinaError::Protocol(_))
        ));
    }

    #[test]
    fn rejects_empty_short_duplicate_and_unexpected_records() {
        assert!(matches!(parse_response(&[]), Err(SinaError::Protocol(_))));
        let short = "var hq_str_sh600396=\"华电辽能,1\";";
        assert!(matches!(
            parse_response(&encoded(short)),
            Err(SinaError::Protocol(_))
        ));
        let duplicate = format!("{SH_LINE}\n{SH_LINE}\n");
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded(&duplicate),
        });
        assert!(matches!(
            client.realtime_quotes(&[sh()]),
            Err(SinaError::Protocol(_))
        ));
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded(&format!("{SH_LINE}\n{SZ_LINE}\n")),
        });
        assert!(matches!(
            client.realtime_quotes(&[sh()]),
            Err(SinaError::Protocol(_))
        ));
    }

    #[test]
    fn rejects_quantity_without_price_and_propagates_transport_failure() {
        let bad_book = SZ_LINE
            .replacen(",11.070,11.080,", ",0.000,11.080,", 1)
            .replace(",238200,11.070,", ",238200,0.000,");
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded(&bad_book),
        });
        let batch = client.order_books(&[sz()]).unwrap();
        assert_eq!(batch.records()[0].status(), DataStatus::Unavailable);
        assert!(batch.records()[0].bids()[0].price().is_none());
        assert!(batch
            .quality()
            .issues()
            .iter()
            .any(|issue| issue.contains("quantity without price")));

        let client = SinaClient::with_transport(FailingTransport);
        assert!(matches!(
            client.realtime_quotes(&[sh()]),
            Err(SinaError::Transport(message)) if message == "fixture timeout"
        ));
    }

    #[test]
    fn st_flag_uses_only_verified_name_prefixes() {
        assert!(st_flag("*ST示例"));
        assert!(st_flag("ST示例"));
        assert!(!st_flag("BEST示例"));
    }
}
