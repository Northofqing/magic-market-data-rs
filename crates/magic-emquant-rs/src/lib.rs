#![forbid(unsafe_code)]
//! Read-only Eastmoney/Choice EMQuant adapter using the audited C++ bridge.

use magic_market_core::{
    Adjustment, AuctionSnapshot, Auctions, Bar, BarInterval, BarsRequest, BookLevel, Capabilities,
    DataBatch, DataStatus, HistoricalBars, InstrumentId, Money, MoneyFlow, MoneyFlows, OrderBook,
    OrderBooks, Price, ProviderId, Quantity, Quote, Ratio, RatioUnit, RealtimeQuotes,
    SecurityMetadata, SecurityMetadataProvider, Trade, Trades, TradesRequest,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_BRIDGE_TIMEOUT: Duration = Duration::from_secs(30);
const BRIDGE_ENV: &str = "MAGIC_EMQUANT_BRIDGE";

#[cfg(windows)]
const BRIDGE_FILENAME: &str = "emquant-snapshot.exe";
#[cfg(not(windows))]
const BRIDGE_FILENAME: &str = "emquant-snapshot";

/// Failures emitted by the local bridge or strict result normalization.
#[derive(Debug, thiserror::Error)]
pub enum EmQuantError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("EMQuant bridge failed: {0}")]
    Bridge(String),
    #[error("invalid EMQuant response: {0}")]
    InvalidResponse(String),
    #[error("unsupported EMQuant capability: {0}")]
    Unsupported(String),
    #[error("Core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Debug, Deserialize)]
struct BridgeResponse {
    records: Vec<BridgeRecord>,
}

#[derive(Debug, Deserialize)]
struct BridgeRecord {
    date: String,
    code: String,
    values: HashMap<String, Value>,
}

/// Read-only provider backed by `tools/emquant/snapshot_bridge.cpp`.
pub struct EmQuantClient {
    bridge: PathBuf,
    timeout: Duration,
}

impl EmQuantClient {
    /// Discovers the bridge built in this workspace. `MAGIC_EMQUANT_BRIDGE`
    /// remains an optional override for deployments that keep executables in
    /// a separate, managed directory.
    pub fn discover() -> Result<Self, EmQuantError> {
        Self::new(discover_bridge_path()?)
    }

    /// Uses an already-built bridge executable. Credentials remain in the
    /// caller's environment and are never accepted as Rust API arguments.
    pub fn new(bridge: impl Into<PathBuf>) -> Result<Self, EmQuantError> {
        let bridge = bridge.into();
        if !bridge.is_file() {
            return Err(EmQuantError::InvalidRequest(format!(
                "bridge executable not found: {}",
                bridge.display()
            )));
        }
        Ok(Self {
            bridge,
            timeout: DEFAULT_BRIDGE_TIMEOUT,
        })
    }

    pub fn bridge_path(&self) -> &Path {
        &self.bridge
    }

    /// Overrides the default 30-second bridge timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, EmQuantError> {
        if timeout.is_zero() {
            return Err(EmQuantError::InvalidRequest(
                "bridge timeout must be positive".into(),
            ));
        }
        self.timeout = timeout;
        Ok(self)
    }

    /// Capabilities implemented by this adapter. Actual availability still
    /// depends on the caller's EMQuant product permissions.
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            quotes: true,
            bars: true,
            minute: true,
            money_flow: true,
            order_book: true,
            ..Capabilities::new()
        }
    }

    fn execute(&self, arguments: &[&str]) -> Result<BridgeResponse, EmQuantError> {
        let mut child = Command::new(&self.bridge)
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| EmQuantError::Bridge(error.to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| EmQuantError::Bridge("unable to capture bridge stdout".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| EmQuantError::Bridge("unable to capture bridge stderr".into()))?;
        let stdout_reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut stdout = stdout;
            stdout.read_to_end(&mut bytes).map(|_| bytes)
        });
        let stderr_reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut stderr = stderr;
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        });
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| EmQuantError::Bridge(error.to_string()))?
            {
                break status;
            }
            if started.elapsed() >= self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(EmQuantError::Bridge(format!(
                    "timed out after {} ms",
                    self.timeout.as_millis()
                )));
            }
            thread::sleep(Duration::from_millis(10));
        };
        let stdout = stdout_reader
            .join()
            .map_err(|_| EmQuantError::Bridge("stdout reader panicked".into()))?
            .map_err(|error| EmQuantError::Bridge(error.to_string()))?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| EmQuantError::Bridge("stderr reader panicked".into()))?
            .map_err(|error| EmQuantError::Bridge(error.to_string()))?;
        if !status.success() {
            let message = String::from_utf8_lossy(&stderr).trim().to_owned();
            return Err(EmQuantError::Bridge(if message.is_empty() {
                format!("exit status {status}")
            } else {
                message
            }));
        }
        serde_json::from_slice(&stdout)
            .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))
    }

    fn snapshot(&self, codes: &str, indicators: &str) -> Result<BridgeResponse, EmQuantError> {
        self.execute(&[codes, indicators])
    }

    fn history(
        &self,
        method: &str,
        code: &str,
        indicators: &str,
        start: &str,
        end: &str,
        options: &str,
    ) -> Result<BridgeResponse, EmQuantError> {
        self.execute(&["--history", method, code, indicators, start, end, options])
    }

    fn section(
        &self,
        codes: &str,
        indicators: &str,
        options: &str,
    ) -> Result<BridgeResponse, EmQuantError> {
        self.execute(&["--section", "css", codes, indicators, options])
    }
}

/// Returns the fixed workspace build location used by
/// `tools/emquant/build_snapshot_bridge.sh`.
pub fn workspace_bridge_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/emquant")
        .join(BRIDGE_FILENAME)
}

/// Finds an explicitly configured bridge or the bridge built under this
/// project's `target/emquant` directory.
pub fn discover_bridge_path() -> Result<PathBuf, EmQuantError> {
    if let Some(configured) = std::env::var_os(BRIDGE_ENV).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Ok(path);
        }
        return Err(EmQuantError::InvalidRequest(format!(
            "{BRIDGE_ENV} does not point to a bridge executable: {}",
            path.display()
        )));
    }

    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("target/emquant").join(BRIDGE_FILENAME));
    }
    candidates.push(workspace_bridge_path());
    if let Some(path) = candidates.iter().find(|path| path.is_file()) {
        return Ok(path.clone());
    }

    let searched = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(EmQuantError::InvalidRequest(format!(
        "EMQuant bridge is not built; run tools/emquant/build_snapshot_bridge.sh /path/to/EMQuantAPI_CPP_Mac (searched: {searched})"
    )))
}

fn source_code(instrument: &InstrumentId) -> Result<String, EmQuantError> {
    let suffix = match instrument.exchange() {
        magic_market_core::Exchange::Shanghai => "SH",
        magic_market_core::Exchange::Shenzhen => "SZ",
        magic_market_core::Exchange::Beijing => {
            return Err(EmQuantError::Unsupported(
                "beijing exchange code suffix is not verified by the bundled SDK documentation"
                    .into(),
            ));
        }
    };
    Ok(format!("{}.{}", instrument.code(), suffix))
}

fn required_number(values: &HashMap<String, Value>, field: &str) -> Result<f64, EmQuantError> {
    values
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| EmQuantError::InvalidResponse(format!("missing numeric field {field}")))
}

fn optional_number(
    values: &HashMap<String, Value>,
    field: &str,
) -> Result<Option<f64>, EmQuantError> {
    match values.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_f64()
            .map(Some)
            .ok_or_else(|| EmQuantError::InvalidResponse(format!("invalid numeric field {field}"))),
    }
}

fn optional_price(
    values: &HashMap<String, Value>,
    field: &str,
) -> Result<Option<Price>, EmQuantError> {
    match optional_number(values, field)? {
        None | Some(0.0) => Ok(None),
        Some(value) if value > 0.0 => Price::new(value)
            .map(Some)
            .map_err(|error| EmQuantError::InvalidResponse(error.to_string())),
        Some(_) => Err(EmQuantError::InvalidResponse(format!(
            "{field} must be positive when present"
        ))),
    }
}

fn optional_nonnegative_money(
    values: &HashMap<String, Value>,
    field: &str,
) -> Result<Option<Money>, EmQuantError> {
    match optional_number(values, field)? {
        None => Ok(None),
        Some(value) if value >= 0.0 => Money::new(value)
            .map(Some)
            .map_err(|error| EmQuantError::InvalidResponse(error.to_string())),
        Some(_) => Err(EmQuantError::InvalidResponse(format!(
            "{field} must be non-negative"
        ))),
    }
}

fn net_money(
    values: &HashMap<String, Value>,
    inflow: &str,
    outflow: &str,
) -> Result<Option<Money>, EmQuantError> {
    match (
        optional_number(values, inflow)?,
        optional_number(values, outflow)?,
    ) {
        (Some(inflow), Some(outflow)) => Money::new(inflow - outflow)
            .map(Some)
            .map_err(|error| EmQuantError::InvalidResponse(error.to_string())),
        _ => Ok(None),
    }
}

fn valid_iso_date(value: &str) -> bool {
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
    let year: u16 = value[0..4].parse().unwrap_or(0);
    let month: u8 = value[5..7].parse().unwrap_or(0);
    let day: u8 = value[8..10].parse().unwrap_or(0);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    year >= 1900 && day >= 1 && day <= max_day
}

fn valid_clock_time(value: &str) -> bool {
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
    let hour: u8 = value[0..2].parse().unwrap_or(24);
    let minute: u8 = value[3..5].parse().unwrap_or(60);
    let second: u8 = value[6..8].parse().unwrap_or(60);
    hour < 24 && minute < 60 && second < 60
}

fn source_date(record: &BridgeRecord) -> Option<String> {
    let raw_date = record.date.split_whitespace().next()?;
    let date = normalize_date(raw_date);
    valid_iso_date(&date).then_some(date)
}

fn realtime_source_timestamp(record: &BridgeRecord) -> Option<String> {
    let date = source_date(record)?;
    let time = value_text(&record.values, "TIME")
        .or_else(|| record.date.split_whitespace().nth(1).map(str::to_owned))
        .map(|value| normalize_time(&value))?;
    valid_clock_time(&time).then(|| format!("{date} {time}"))
}

fn daily_source_timestamp(record: &BridgeRecord) -> Option<String> {
    source_date(record)
}

fn observed_epoch() -> Result<String, EmQuantError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .map_err(|error| {
            EmQuantError::InvalidResponse(format!("system clock is before UNIX epoch: {error}"))
        })
}

fn daily_period(interval: BarInterval) -> Option<u8> {
    match interval {
        BarInterval::Day => Some(1),
        BarInterval::Week => Some(2),
        BarInterval::Month => Some(3),
        BarInterval::Year => Some(4),
        BarInterval::Minute1
        | BarInterval::Minute5
        | BarInterval::Minute15
        | BarInterval::Minute30
        | BarInterval::Hour1 => None,
    }
}

fn minute_width(interval: BarInterval) -> Option<u16> {
    match interval {
        BarInterval::Minute1 => Some(1),
        BarInterval::Minute5 => Some(5),
        BarInterval::Minute15 => Some(15),
        BarInterval::Minute30 => Some(30),
        BarInterval::Hour1 => Some(60),
        BarInterval::Day | BarInterval::Week | BarInterval::Month | BarInterval::Year => None,
    }
}

fn date_from_epoch_days(days: i64) -> String {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn default_date_range(request: &BarsRequest) -> Result<(String, String), EmQuantError> {
    match (request.start(), request.end()) {
        (Some(start), Some(end)) => return Ok((start.to_owned(), end.to_owned())),
        (Some(_), None) | (None, Some(_)) => {
            return Err(EmQuantError::InvalidRequest(
                "bar start and end must be supplied together".into(),
            ));
        }
        (None, None) => {}
    }
    let today_days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| EmQuantError::InvalidRequest(error.to_string()))?
        .as_secs()
        / 86_400;
    let count = i64::from(request.limit());
    let lookback_days = match request.interval() {
        BarInterval::Day => count * 2 + 30,
        BarInterval::Week => count * 8 + 30,
        BarInterval::Month => count * 32 + 31,
        BarInterval::Year => count * 367 + 366,
        interval => {
            let minutes = i64::from(minute_width(interval).ok_or_else(|| {
                EmQuantError::Unsupported("unsupported EMQuant bar interval".into())
            })?);
            (count * minutes + 239) / 240 + 7
        }
    };
    let end = date_from_epoch_days(today_days as i64);
    let calculated_start = date_from_epoch_days(today_days as i64 - lookback_days);
    let start = if calculated_start.as_str() < "1990-01-01" {
        "1990-01-01".to_owned()
    } else {
        calculated_start
    };
    Ok((start, end))
}

fn value_text(values: &HashMap<String, Value>, field: &str) -> Option<String> {
    values.get(field).and_then(|value| match value {
        Value::String(value) => Some(value.trim().to_owned()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    })
}

fn normalize_date(value: &str) -> String {
    let value = value.trim();
    if value.len() == 8 && value.bytes().all(|byte| byte.is_ascii_digit()) {
        format!("{}-{}-{}", &value[0..4], &value[4..6], &value[6..8])
    } else {
        let normalized = value.replace('/', "-");
        let mut fields = normalized.split('-');
        match (
            fields.next().and_then(|value| value.parse::<u16>().ok()),
            fields.next().and_then(|value| value.parse::<u8>().ok()),
            fields.next().and_then(|value| value.parse::<u8>().ok()),
            fields.next(),
        ) {
            (Some(year), Some(month), Some(day), None) => {
                format!("{year:04}-{month:02}-{day:02}")
            }
            _ => normalized,
        }
    }
}

fn normalize_time(value: &str) -> String {
    let value = value.trim();
    if value.contains(':') {
        return value.to_owned();
    }
    let digits = value.split('.').next().unwrap_or(value);
    let digits = if digits.len() > 6 {
        &digits[..6]
    } else {
        digits
    };
    if digits.bytes().all(|byte| byte.is_ascii_digit()) {
        let padded = format!("{digits:0>6}");
        format!("{}:{}:{}", &padded[0..2], &padded[2..4], &padded[4..6])
    } else {
        value.to_owned()
    }
}

fn record_bar_at(record: &BridgeRecord, minute: bool) -> Result<String, EmQuantError> {
    if !minute {
        if record.date.is_empty() {
            return Err(EmQuantError::InvalidResponse(
                "history response contained an empty date".into(),
            ));
        }
        return Ok(normalize_date(&record.date));
    }
    let date = value_text(&record.values, "DATE")
        .map(|value| normalize_date(&value))
        .or_else(|| {
            record
                .date
                .split_whitespace()
                .next()
                .filter(|value| !value.is_empty())
                .map(normalize_date)
        })
        .ok_or_else(|| EmQuantError::InvalidResponse("minute bar is missing DATE".into()))?;
    if record.date.contains(' ') && !record.values.contains_key("TIME") {
        return Ok(record.date.clone());
    }
    let time = value_text(&record.values, "TIME")
        .map(|value| normalize_time(&value))
        .ok_or_else(|| EmQuantError::InvalidResponse("minute bar is missing TIME".into()))?;
    Ok(format!("{date} {time}"))
}

fn minute_bucket(timestamp: &str, width: u16) -> Result<(String, u16), EmQuantError> {
    let (date, time) = timestamp.split_once(' ').ok_or_else(|| {
        EmQuantError::InvalidResponse(format!("invalid minute timestamp {timestamp}"))
    })?;
    let mut fields = time.split(':');
    let hour: u16 = fields
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| EmQuantError::InvalidResponse(format!("invalid time {time}")))?;
    let minute: u16 = fields
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| EmQuantError::InvalidResponse(format!("invalid time {time}")))?;
    if hour > 23 || minute > 59 {
        return Err(EmQuantError::InvalidResponse(format!(
            "invalid time {time}"
        )));
    }
    let minute_of_day = hour * 60 + minute;
    Ok((format!("{date}:{}", minute_of_day / width), minute_of_day))
}

fn aggregate_group(
    group: &[Bar],
    interval: BarInterval,
    batch_id: &str,
) -> Result<Bar, EmQuantError> {
    let first = group
        .first()
        .ok_or_else(|| EmQuantError::InvalidResponse("empty minute aggregation".into()))?;
    let last = group.last().expect("non-empty group");
    let high = group
        .iter()
        .map(|bar| bar.high().get())
        .fold(f64::NEG_INFINITY, f64::max);
    let low = group
        .iter()
        .map(|bar| bar.low().get())
        .fold(f64::INFINITY, f64::min);
    let volume = group.iter().map(|bar| bar.volume().get()).sum();
    let amount = if group.iter().all(|bar| bar.amount().is_some()) {
        Some(
            Money::new(group.iter().filter_map(Bar::amount).map(Money::get).sum())
                .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?,
        )
    } else {
        None
    };
    let mut bar = Bar::new(
        first.instrument().clone(),
        interval,
        first.bar_start().to_owned(),
        last.bar_end().to_owned(),
        first.open(),
        Price::new(high).map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?,
        Price::new(low).map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?,
        last.close(),
        Quantity::new(volume).map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?,
        amount,
        Adjustment::Unadjusted,
        ProviderId::Eastmoney,
        batch_id,
    )
    .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
    if let Some(source_at) = last.source_at() {
        bar = bar.with_source_at(source_at)?;
    }
    Ok(bar)
}

fn aggregate_minute_bars(
    bars: Vec<Bar>,
    interval: BarInterval,
    width: u16,
    batch_id: &str,
) -> Result<Vec<Bar>, EmQuantError> {
    if width == 1 {
        return Ok(bars);
    }
    let mut result = Vec::new();
    let mut group = Vec::new();
    let mut group_key = None;
    let mut previous_minute = None;
    for bar in bars {
        let (key, minute) = minute_bucket(bar.bar_start(), width)?;
        if group_key.as_ref().is_some_and(|current| current != &key) {
            result.push(aggregate_group(&group, interval, batch_id)?);
            group.clear();
            previous_minute = None;
        }
        if previous_minute.is_some_and(|previous| minute != previous + 1) {
            return Err(EmQuantError::InvalidResponse(format!(
                "minute gap inside aggregation bucket at {}",
                bar.bar_start()
            )));
        }
        group_key = Some(key);
        previous_minute = Some(minute);
        group.push(bar);
    }
    if !group.is_empty() {
        result.push(aggregate_group(&group, interval, batch_id)?);
    }
    Ok(result)
}

fn book_level(
    values: &HashMap<String, Value>,
    side: &str,
    level: u8,
) -> Result<BookLevel, EmQuantError> {
    let price_field = format!("{side}PRICE{level}");
    let volume_field = format!("{side}VOLUME{level}");
    let price = optional_number(values, &price_field)?
        .filter(|value| *value > 0.0)
        .map(Price::new)
        .transpose()
        .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
    let quantity = optional_number(values, &volume_field)?
        .map(Quantity::new)
        .transpose()
        .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
    Ok(BookLevel::new(price, quantity)?)
}

fn book_depth(levels: &[BookLevel; 5]) -> Result<Option<Quantity>, EmQuantError> {
    let mut found = false;
    let total =
        levels
            .iter()
            .filter_map(|level| level.quantity())
            .fold(0.0, |accumulator, quantity| {
                found = true;
                accumulator + quantity.get()
            });
    if found {
        Quantity::new(total)
            .map(Some)
            .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))
    } else {
        Ok(None)
    }
}

impl RealtimeQuotes for EmQuantClient {
    type Quote = Quote;
    type Error = EmQuantError;

    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        if instruments.is_empty() {
            return Err(EmQuantError::InvalidRequest(
                "quote request is empty".into(),
            ));
        }
        let mut seen = HashSet::new();
        let codes: Vec<String> = instruments
            .iter()
            .map(source_code)
            .collect::<Result<_, _>>()?;
        if codes.iter().any(|code| !seen.insert(code.clone())) {
            return Err(EmQuantError::InvalidRequest(
                "EMQuant rejects duplicate security codes".into(),
            ));
        }
        let indicators = "TIME,NAME,NOW,PRECLOSE,OPEN,HIGH,LOW,PCTCHANGE,VOLUME,AMOUNT";
        let response = self.snapshot(&codes.join(","), indicators)?;
        if response.records.len() != instruments.len() {
            return Err(EmQuantError::InvalidResponse(format!(
                "quote cardinality mismatch: requested {}, received {}",
                instruments.len(),
                response.records.len()
            )));
        }
        let mut by_code: HashMap<String, BridgeRecord> = response
            .records
            .into_iter()
            .map(|record| (record.code.clone(), record))
            .collect();
        let observed_at = observed_epoch()?;
        let batch_id = format!("eastmoney:{observed_at}:quote");
        let mut quotes = Vec::with_capacity(instruments.len());
        let mut issues = Vec::new();
        let mut source_at = None;
        for (instrument, code) in instruments.iter().zip(codes) {
            let record = by_code.remove(&code).ok_or_else(|| {
                EmQuantError::InvalidResponse(format!("missing requested code {code}"))
            })?;
            let record_source_at = realtime_source_timestamp(&record);
            if source_at.is_none() {
                source_at.clone_from(&record_source_at);
            }
            let price = Price::new(required_number(&record.values, "NOW")?)
                .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
            let volume = Quantity::new(required_number(&record.values, "VOLUME")?)
                .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
            let name = value_text(&record.values, "NAME").filter(|value| !value.is_empty());
            let previous_close = optional_price(&record.values, "PRECLOSE")?;
            let open = optional_price(&record.values, "OPEN")?;
            let high = optional_price(&record.values, "HIGH")?;
            let low = optional_price(&record.values, "LOW")?;
            let change_percent = optional_number(&record.values, "PCTCHANGE")?
                .map(|value| Ratio::new(value, RatioUnit::Percent))
                .transpose()
                .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
            let amount = optional_nonnegative_money(&record.values, "AMOUNT")?;
            let complete = name.is_some()
                && previous_close.is_some()
                && open.is_some()
                && high.is_some()
                && low.is_some()
                && change_percent.is_some()
                && amount.is_some()
                && record_source_at.is_some();
            if !complete {
                issues.push(format!(
                    "{code}: one or more normalized quote fields unavailable"
                ));
            }
            quotes.push(Quote::from_parts(
                instrument.clone(),
                name,
                price,
                previous_close,
                open,
                high,
                low,
                change_percent,
                volume,
                amount,
                if complete {
                    DataStatus::Available
                } else {
                    DataStatus::Unavailable
                },
                record_source_at,
                observed_at.clone(),
                ProviderId::Eastmoney,
                batch_id.clone(),
            )?);
        }
        if !by_code.is_empty() {
            return Err(EmQuantError::InvalidResponse(
                "response contained unexpected security codes".into(),
            ));
        }
        let mut provenance = magic_market_core::Provenance::new("eastmoney-emquant", observed_at)?
            .with_batch_id(batch_id)?;
        if let Some(value) = source_at {
            provenance = provenance.with_source_at(value)?;
        }
        Ok(DataBatch::best_effort(quotes, provenance, issues)?)
    }
}

impl HistoricalBars for EmQuantClient {
    type Bar = Bar;
    type Error = EmQuantError;

    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Bar>, Self::Error> {
        let (start, end) = default_date_range(request)?;
        let code = source_code(request.instrument())?;
        let minute = minute_width(request.interval());
        let (method, indicators, options) = if let Some(period) = daily_period(request.interval()) {
            (
                "csd",
                "OPEN,HIGH,LOW,CLOSE,VOLUME,AMOUNT",
                format!("Period={period},AdjustFlag=1,Order=1"),
            )
        } else if minute.is_some() {
            (
                "chmc",
                "DATE,TIME,OPEN,HIGH,LOW,CLOSE,VOLUME,AMOUNT",
                String::new(),
            )
        } else {
            return Err(EmQuantError::Unsupported(
                "unsupported EMQuant bar interval".into(),
            ));
        };
        let response = self.history(method, &code, indicators, &start, &end, &options)?;
        if response.records.is_empty() {
            return Err(EmQuantError::InvalidResponse(
                "history response contained no bars".into(),
            ));
        }
        let observed_at = observed_epoch()?;
        let batch_id = format!("eastmoney:{observed_at}:bars");
        let mut bars = Vec::with_capacity(response.records.len());
        let mut previous_at: Option<String> = None;
        for record in response.records {
            if record.code != code {
                return Err(EmQuantError::InvalidResponse(format!(
                    "history response contained unexpected code {}",
                    record.code
                )));
            }
            let bar_at = record_bar_at(&record, minute.is_some())?;
            if previous_at
                .as_ref()
                .is_some_and(|previous| bar_at <= *previous)
            {
                return Err(EmQuantError::InvalidResponse(format!(
                    "history timestamps are duplicated or out of order at {bar_at}"
                )));
            }
            previous_at = Some(bar_at.clone());
            let price = |field| {
                Price::new(required_number(&record.values, field)?)
                    .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))
            };
            let volume = Quantity::new(required_number(&record.values, "VOLUME")?)
                .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
            let amount = optional_number(&record.values, "AMOUNT")?
                .map(Money::new)
                .transpose()
                .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
            let bar = Bar::new(
                request.instrument().clone(),
                if minute.is_some() {
                    BarInterval::Minute1
                } else {
                    request.interval()
                },
                bar_at.clone(),
                bar_at.clone(),
                price("OPEN")?,
                price("HIGH")?,
                price("LOW")?,
                price("CLOSE")?,
                volume,
                amount,
                Adjustment::Unadjusted,
                ProviderId::Eastmoney,
                batch_id.clone(),
            )
            .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?
            .with_source_at(bar_at)?;
            bars.push(bar);
        }
        if let Some(width) = minute {
            bars = aggregate_minute_bars(bars, request.interval(), width, &batch_id)?;
        }
        let keep = usize::from(request.limit());
        if bars.len() > keep {
            bars = bars.split_off(bars.len() - keep);
        }
        let source_at = bars
            .last()
            .and_then(|bar| bar.source_at().map(str::to_owned));
        let mut provenance =
            magic_market_core::Provenance::new(format!("eastmoney-emquant-{method}"), observed_at)?
                .with_batch_id(batch_id)?;
        if let Some(source_at) = source_at {
            provenance = provenance.with_source_at(source_at)?;
        }
        Ok(DataBatch::strict(bars, provenance))
    }
}

impl MoneyFlows for EmQuantClient {
    type Error = EmQuantError;

    fn money_flows(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<MoneyFlow>, Self::Error> {
        if instruments.is_empty() {
            return Err(EmQuantError::InvalidRequest(
                "money-flow request is empty".into(),
            ));
        }
        let mut seen = HashSet::new();
        let codes: Vec<String> = instruments
            .iter()
            .map(source_code)
            .collect::<Result<_, _>>()?;
        if codes.iter().any(|code| !seen.insert(code.clone())) {
            return Err(EmQuantError::InvalidRequest(
                "EMQuant rejects duplicate security codes".into(),
            ));
        }
        let indicators = "SUPERINFLOW,SUPEROUTFLOW,BIGINFLOW,BIGOUTFLOW,MIDINFLOW,MIDOUTFLOW,SMALLINFLOW,SMALLOUTFLOW";
        let response = self.section(&codes.join(","), indicators, "")?;
        if response.records.len() != instruments.len() {
            return Err(EmQuantError::InvalidResponse(format!(
                "money-flow cardinality mismatch: requested {}, received {}",
                instruments.len(),
                response.records.len()
            )));
        }
        let mut by_code = HashMap::new();
        for record in response.records {
            let code = record.code.clone();
            if by_code.insert(code.clone(), record).is_some() {
                return Err(EmQuantError::InvalidResponse(format!(
                    "duplicate money-flow code {code}"
                )));
            }
        }
        let observed_at = observed_epoch()?;
        let batch_id = format!("eastmoney:{observed_at}:money-flow");
        let mut flows = Vec::with_capacity(instruments.len());
        let mut issues = Vec::new();
        let mut batch_source_at = None;
        for (instrument, code) in instruments.iter().zip(codes) {
            let record = by_code.remove(&code).ok_or_else(|| {
                EmQuantError::InvalidResponse(format!("missing requested code {code}"))
            })?;
            let source_at = daily_source_timestamp(&record);
            if batch_source_at.is_none() {
                batch_source_at.clone_from(&source_at);
            }
            let super_large_net = net_money(&record.values, "SUPERINFLOW", "SUPEROUTFLOW")?;
            let large_net = net_money(&record.values, "BIGINFLOW", "BIGOUTFLOW")?;
            let medium_net = net_money(&record.values, "MIDINFLOW", "MIDOUTFLOW")?;
            let small_net = net_money(&record.values, "SMALLINFLOW", "SMALLOUTFLOW")?;
            let main_net = match (super_large_net, large_net) {
                (Some(super_large), Some(large)) => Some(
                    Money::new(super_large.get() + large.get())
                        .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?,
                ),
                _ => None,
            };
            let complete = [main_net, super_large_net, large_net, medium_net, small_net]
                .iter()
                .all(Option::is_some)
                && source_at.is_some();
            if !complete {
                issues.push(format!("{code}: one or more money-flow fields unavailable"));
            }
            flows.push(MoneyFlow::new(
                instrument.clone(),
                main_net,
                super_large_net,
                large_net,
                medium_net,
                small_net,
                if complete {
                    DataStatus::Available
                } else {
                    DataStatus::Unavailable
                },
                source_at,
                observed_at.clone(),
                ProviderId::Eastmoney,
                batch_id.clone(),
            )?);
        }
        if !by_code.is_empty() {
            return Err(EmQuantError::InvalidResponse(
                "money-flow response contained unexpected security codes".into(),
            ));
        }
        let mut provenance =
            magic_market_core::Provenance::new("eastmoney-emquant-css-money-flow", observed_at)?
                .with_batch_id(batch_id)?;
        if let Some(source_at) = batch_source_at {
            provenance = provenance.with_source_at(source_at)?;
        }
        Ok(DataBatch::best_effort(flows, provenance, issues)?)
    }
}

impl Auctions for EmQuantClient {
    type Error = EmQuantError;

    fn auction_snapshots(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<AuctionSnapshot>, Self::Error> {
        Err(EmQuantError::Unsupported(
            "opening-auction matched and unmatched fields are not verified in EMQuant".into(),
        ))
    }
}

impl Trades for EmQuantClient {
    type Error = EmQuantError;

    fn trades(&self, _request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        Err(EmQuantError::Unsupported(
            "executed-trade fields and pagination are not verified in EMQuant".into(),
        ))
    }
}

impl SecurityMetadataProvider for EmQuantClient {
    type Error = EmQuantError;

    fn security_metadata(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        Err(EmQuantError::Unsupported(
            "security master fields and their source timestamps are not verified in EMQuant".into(),
        ))
    }
}

impl OrderBooks for EmQuantClient {
    type Error = EmQuantError;

    fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        if instruments.is_empty() {
            return Err(EmQuantError::InvalidRequest(
                "order-book request is empty".into(),
            ));
        }
        let mut seen = HashSet::new();
        let codes: Vec<String> = instruments
            .iter()
            .map(source_code)
            .collect::<Result<_, _>>()?;
        if codes.iter().any(|code| !seen.insert(code.clone())) {
            return Err(EmQuantError::InvalidRequest(
                "EMQuant rejects duplicate security codes".into(),
            ));
        }
        let indicators = "TIME,BUYPRICE1,BUYVOLUME1,SELLPRICE1,SELLVOLUME1,BUYPRICE2,BUYVOLUME2,SELLPRICE2,SELLVOLUME2,BUYPRICE3,BUYVOLUME3,SELLPRICE3,SELLVOLUME3,BUYPRICE4,BUYVOLUME4,SELLPRICE4,SELLVOLUME4,BUYPRICE5,BUYVOLUME5,SELLPRICE5,SELLVOLUME5";
        let response = self.snapshot(&codes.join(","), indicators)?;
        if response.records.len() != instruments.len() {
            return Err(EmQuantError::InvalidResponse(format!(
                "order-book cardinality mismatch: requested {}, received {}",
                instruments.len(),
                response.records.len()
            )));
        }
        let mut by_code: HashMap<String, BridgeRecord> = response
            .records
            .into_iter()
            .map(|record| (record.code.clone(), record))
            .collect();
        let observed_at = observed_epoch()?;
        let batch_id = format!("eastmoney:{observed_at}:order-book");
        let mut books = Vec::with_capacity(instruments.len());
        let mut issues = Vec::new();
        let mut batch_source_at = None;
        for (instrument, code) in instruments.iter().zip(codes) {
            let record = by_code.remove(&code).ok_or_else(|| {
                EmQuantError::InvalidResponse(format!("missing requested code {code}"))
            })?;
            let bids = [
                book_level(&record.values, "BUY", 1)?,
                book_level(&record.values, "BUY", 2)?,
                book_level(&record.values, "BUY", 3)?,
                book_level(&record.values, "BUY", 4)?,
                book_level(&record.values, "BUY", 5)?,
            ];
            let asks = [
                book_level(&record.values, "SELL", 1)?,
                book_level(&record.values, "SELL", 2)?,
                book_level(&record.values, "SELL", 3)?,
                book_level(&record.values, "SELL", 4)?,
                book_level(&record.values, "SELL", 5)?,
            ];
            let available = bids
                .iter()
                .chain(&asks)
                .all(|level| level.price().is_some());
            let total_bid_quantity = book_depth(&bids)?;
            let total_ask_quantity = book_depth(&asks)?;
            let source_at = realtime_source_timestamp(&record);
            if batch_source_at.is_none() {
                batch_source_at.clone_from(&source_at);
            }
            if !available || source_at.is_none() {
                issues.push(format!(
                    "{code}: one or more normalized order-book fields unavailable"
                ));
            }
            books.push(OrderBook::new(
                instrument.clone(),
                bids,
                asks,
                total_bid_quantity,
                total_ask_quantity,
                if available && source_at.is_some() {
                    DataStatus::Available
                } else {
                    DataStatus::Unavailable
                },
                source_at,
                observed_at.clone(),
                ProviderId::Eastmoney,
                batch_id.clone(),
            )?);
        }
        if !by_code.is_empty() {
            return Err(EmQuantError::InvalidResponse(
                "response contained unexpected security codes".into(),
            ));
        }
        let mut provenance = magic_market_core::Provenance::new("eastmoney-emquant", observed_at)?
            .with_batch_id(batch_id)?;
        if let Some(value) = batch_source_at {
            provenance = provenance.with_source_at(value)?;
        }
        Ok(DataBatch::best_effort(books, provenance, issues)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_snapshot_shape_without_credentials() {
        let response: BridgeResponse = serde_json::from_str(
            r#"{"records":[{"date":"2026-07-22","code":"600519.SH","values":{"TIME":"10:00:00","NOW":1300.0,"VOLUME":12.0,"AMOUNT":15600.0}}]}"#,
        )
        .unwrap();
        assert_eq!(response.records.len(), 1);
        assert_eq!(
            required_number(&response.records[0].values, "NOW").unwrap(),
            1300.0
        );
        assert_eq!(
            realtime_source_timestamp(&response.records[0]).as_deref(),
            Some("2026-07-22 10:00:00")
        );
    }

    #[test]
    fn source_evidence_requires_valid_family_specific_timestamps() {
        let record = |date: &str, time: Option<Value>| BridgeRecord {
            date: date.into(),
            code: "600519.SH".into(),
            values: time
                .map(|value| HashMap::from([("TIME".into(), value)]))
                .unwrap_or_default(),
        };

        assert_eq!(
            realtime_source_timestamp(&record("20260722", Some(Value::String("100001".into()))))
                .as_deref(),
            Some("2026-07-22 10:00:01")
        );
        assert!(realtime_source_timestamp(&record("2026-07-22", None)).is_none());
        assert!(realtime_source_timestamp(&record(
            "2026-02-30",
            Some(Value::String("10:00:00".into()))
        ))
        .is_none());
        assert!(realtime_source_timestamp(&record(
            "2026-07-22",
            Some(Value::String("25:00:00".into()))
        ))
        .is_none());
        assert_eq!(
            daily_source_timestamp(&record("2026/07/22", None)).as_deref(),
            Some("2026-07-22")
        );
        assert!(
            daily_source_timestamp(&record("", Some(Value::String("10:00:00".into())))).is_none()
        );
    }

    #[test]
    fn converts_missing_and_present_book_levels_explicitly() {
        let values: HashMap<String, Value> = serde_json::from_str(
            r#"{"BUYPRICE1":1300.0,"BUYVOLUME1":12.0,"SELLPRICE1":null,"SELLVOLUME1":null}"#,
        )
        .unwrap();
        let bid = book_level(&values, "BUY", 1).unwrap();
        let ask = book_level(&values, "SELL", 1).unwrap();
        assert_eq!(bid.price().map(Price::get), Some(1300.0));
        assert_eq!(bid.quantity().map(Quantity::get), Some(12.0));
        assert!(ask.price().is_none());
        assert!(ask.quantity().is_none());
    }

    #[test]
    fn converts_epoch_day_zero_to_unix_epoch_date() {
        assert_eq!(date_from_epoch_days(0), "1970-01-01");
    }

    #[test]
    fn workspace_bridge_uses_the_builder_output_location() {
        assert!(workspace_bridge_path()
            .ends_with(Path::new("target").join("emquant").join(BRIDGE_FILENAME)));
    }

    #[test]
    fn executed_trades_are_explicitly_unsupported() {
        let client = EmQuantClient::new(std::env::current_exe().unwrap()).unwrap();
        let instrument = InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            "600519",
            magic_market_core::AssetClass::Equity,
        )
        .unwrap();
        let request = TradesRequest::new(instrument, 20).unwrap();
        assert!(matches!(
            client.trades(&request),
            Err(EmQuantError::Unsupported(_))
        ));
    }
}
