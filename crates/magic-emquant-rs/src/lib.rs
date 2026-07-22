#![forbid(unsafe_code)]
//! Read-only Eastmoney/Choice EMQuant adapter using the audited C++ bridge.

use magic_market_core::{
    BookLevel, Capabilities, DataBatch, DataStatus, InstrumentId, Money, OrderBook, OrderBooks,
    Price, ProviderId, Quantity, Quote, RealtimeQuotes,
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

/// Failures emitted by the local bridge or strict result normalization.
#[derive(Debug, thiserror::Error)]
pub enum EmQuantError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("EMQuant bridge failed: {0}")]
    Bridge(String),
    #[error("invalid EMQuant response: {0}")]
    InvalidResponse(String),
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
            order_book: true,
            ..Capabilities::new()
        }
    }

    fn snapshot(&self, codes: &str, indicators: &str) -> Result<BridgeResponse, EmQuantError> {
        let mut child = Command::new(&self.bridge)
            .arg(codes)
            .arg(indicators)
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
}

fn source_code(instrument: &InstrumentId) -> String {
    let suffix = match instrument.exchange() {
        magic_market_core::Exchange::Shanghai => "SH",
        magic_market_core::Exchange::Shenzhen => "SZ",
    };
    format!("{}.{}", instrument.code(), suffix)
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

fn source_timestamp(record: &BridgeRecord) -> Option<String> {
    let time = record.values.get("TIME").and_then(|value| match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    });
    match (record.date.is_empty(), time) {
        (false, Some(time)) => Some(format!("{} {}", record.date, time)),
        (false, None) => Some(record.date.clone()),
        (true, Some(time)) => Some(time),
        (true, None) => None,
    }
}

fn observed_epoch() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or_else(
            |_| "unknown".to_owned(),
            |value| value.as_secs().to_string(),
        )
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
    Ok(BookLevel { price, quantity })
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
        let codes: Vec<String> = instruments.iter().map(source_code).collect();
        if codes.iter().any(|code| !seen.insert(code.clone())) {
            return Err(EmQuantError::InvalidRequest(
                "EMQuant rejects duplicate security codes".into(),
            ));
        }
        let response = self.snapshot(&codes.join(","), "TIME,NOW,VOLUME,AMOUNT")?;
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
        let observed_at = observed_epoch();
        let batch_id = format!("eastmoney:{observed_at}");
        let mut quotes = Vec::with_capacity(instruments.len());
        let mut source_at = None;
        for (instrument, code) in instruments.iter().zip(codes) {
            let record = by_code.remove(&code).ok_or_else(|| {
                EmQuantError::InvalidResponse(format!("missing requested code {code}"))
            })?;
            let record_source_at = source_timestamp(&record);
            if source_at.is_none() {
                source_at.clone_from(&record_source_at);
            }
            let price = Price::new(required_number(&record.values, "NOW")?)
                .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
            let volume = Quantity::new(required_number(&record.values, "VOLUME")?)
                .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
            let amount = optional_number(&record.values, "AMOUNT")?
                .map(Money::new)
                .transpose()
                .map_err(|error| EmQuantError::InvalidResponse(error.to_string()))?;
            let mut quote = Quote::new(
                instrument.clone(),
                price,
                volume,
                amount,
                observed_at.clone(),
                ProviderId::Eastmoney,
                batch_id.clone(),
            );
            if let Some(value) = record_source_at {
                quote = quote.with_source_at(value);
            }
            quotes.push(quote);
        }
        if !by_code.is_empty() {
            return Err(EmQuantError::InvalidResponse(
                "response contained unexpected security codes".into(),
            ));
        }
        let mut provenance = magic_market_core::Provenance::new("eastmoney-emquant", observed_at)
            .with_batch_id(batch_id);
        if let Some(value) = source_at {
            provenance = provenance.with_source_at(value);
        }
        Ok(DataBatch::strict(quotes, provenance))
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
        let codes: Vec<String> = instruments.iter().map(source_code).collect();
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
        let observed_at = observed_epoch();
        let batch_id = format!("eastmoney:{observed_at}");
        let mut books = Vec::with_capacity(instruments.len());
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
            let available = bids.iter().chain(&asks).any(|level| level.price.is_some());
            if batch_source_at.is_none() {
                batch_source_at = source_timestamp(&record);
            }
            books.push(OrderBook {
                instrument: instrument.clone(),
                bids,
                asks,
                status: if available {
                    DataStatus::Available
                } else {
                    DataStatus::Unavailable
                },
            });
        }
        if !by_code.is_empty() {
            return Err(EmQuantError::InvalidResponse(
                "response contained unexpected security codes".into(),
            ));
        }
        let mut provenance = magic_market_core::Provenance::new("eastmoney-emquant", observed_at)
            .with_batch_id(batch_id);
        if let Some(value) = batch_source_at {
            provenance = provenance.with_source_at(value);
        }
        Ok(DataBatch::strict(books, provenance))
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
            source_timestamp(&response.records[0]).as_deref(),
            Some("2026-07-22 10:00:00")
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
        assert_eq!(bid.price.map(Price::get), Some(1300.0));
        assert_eq!(bid.quantity.map(Quantity::get), Some(12.0));
        assert!(ask.price.is_none());
        assert!(ask.quantity.is_none());
    }
}
