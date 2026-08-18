//! Bounded synchronous access to the vendor-documented TQ loopback endpoint.
//!
//! This module exposes no endpoint override, background thread or free-form
//! method string. Endpoint availability is runtime evidence, not admission.

use crate::protocol::{
    DecimalObservation, ObservationUnit, SourceExchange, SourceInstrument, SourceObservation,
    PROTOCOL_VERSION, SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as StdError;
use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;
use thiserror::Error;

/// The only production TQ HTTP origin, port and path.
pub const TQ_LOOPBACK_ENDPOINT: &str = "http://127.0.0.1:17709/";

/// Closed read-only method allowlist for the initial polling slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TqReadMethod {
    EquityUniverse,
    PriceVolume,
    MarketSnapshot,
}

impl TqReadMethod {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::EquityUniverse => "get_stock_list",
            Self::PriceVolume => "get_pricevol",
            Self::MarketSnapshot => "get_market_snapshot",
        }
    }
}

const MARKET_SNAPSHOT_FIELDS: [&str; 7] =
    ["Amount", "Now", "Volume", "LastClose", "Open", "Max", "Min"];

// The exact decimal converter below uses a checked u128 coefficient. This is
// a representation limit, not a source or deployment throughput default.
const MAX_EXACT_DECIMAL_DIGITS: usize = 39;

/// Exact TQ instrument identity. No code-prefix exchange inference is used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TqInstrument {
    instrument: SourceInstrument,
}

impl TqInstrument {
    pub fn new(exchange: SourceExchange, code: impl Into<String>) -> Result<Self, TqLoopbackError> {
        let code = code.into();
        if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(TqLoopbackError::InvalidRequest {
                reason: "instrument code must be exactly six ASCII digits",
            });
        }
        Ok(Self {
            instrument: SourceInstrument { exchange, code },
        })
    }

    pub fn source_instrument(&self) -> &SourceInstrument {
        &self.instrument
    }

    fn wire_symbol(&self) -> String {
        let suffix = match self.instrument.exchange {
            SourceExchange::Shanghai => "SH",
            SourceExchange::Shenzhen => "SZ",
            SourceExchange::Beijing => "BJ",
        };
        format!("{}.{suffix}", self.instrument.code)
    }
}

/// One validated single-instrument TQ market snapshot.
///
/// `previous_close` is retained separately from the v1 [`SourceObservation`]
/// so the response-backed diagnostic field cannot be confused with the
/// independently admitted current-price family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TqMarketSnapshot {
    observation: SourceObservation,
    prices: TqSnapshotPrices,
}

impl TqMarketSnapshot {
    pub fn new(
        observation: SourceObservation,
        prices: TqSnapshotPrices,
    ) -> Result<Self, TqLoopbackError> {
        observation
            .validate()
            .map_err(|_| TqLoopbackError::Schema {
                reason: "mapped market-snapshot observation failed protocol validation",
            })?;
        for (field, value) in [
            ("LastClose", &prices.previous_close),
            ("Open", &prices.open),
            ("Max", &prices.high),
            ("Min", &prices.low),
        ] {
            if value.unit != ObservationUnit::CnyPerShare {
                return Err(TqLoopbackError::Schema {
                    reason: "market-snapshot OHLC field has an invalid unit",
                });
            }
            validate_decimal(field, &value.value, ObservationUnit::CnyPerShare)?;
        }
        Ok(Self {
            observation,
            prices,
        })
    }

    pub fn observation(&self) -> &SourceObservation {
        &self.observation
    }

    pub fn previous_close(&self) -> &DecimalObservation {
        &self.prices.previous_close
    }

    pub fn prices(&self) -> &TqSnapshotPrices {
        &self.prices
    }

    pub fn into_parts(self) -> (SourceObservation, TqSnapshotPrices) {
        (self.observation, self.prices)
    }
}

/// Response-backed previous-close and OHLC snapshot fields. They remain
/// independently unadmitted even when the containing amount event is admitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TqSnapshotPrices {
    pub previous_close: DecimalObservation,
    pub open: DecimalObservation,
    pub high: DecimalObservation,
    pub low: DecimalObservation,
}

/// Bounded evidence returned after a complete all-A-share identity check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TqEquityUniverseEvidence {
    instrument_count: usize,
}

impl TqEquityUniverseEvidence {
    pub const fn instrument_count(self) -> usize {
        self.instrument_count
    }
}

/// Required positive transport limits. There is intentionally no Default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TqLoopbackLimits {
    connect_timeout: Duration,
    read_timeout: Duration,
    write_timeout: Duration,
    max_request_bytes: usize,
    max_response_bytes: usize,
}

impl TqLoopbackLimits {
    pub fn new(
        connect_timeout: Duration,
        read_timeout: Duration,
        write_timeout: Duration,
        max_request_bytes: usize,
        max_response_bytes: usize,
    ) -> Result<Self, TqLoopbackError> {
        if connect_timeout.is_zero() {
            return Err(TqLoopbackError::InvalidLimits {
                field: "connect_timeout",
            });
        }
        if read_timeout.is_zero() {
            return Err(TqLoopbackError::InvalidLimits {
                field: "read_timeout",
            });
        }
        if write_timeout.is_zero() {
            return Err(TqLoopbackError::InvalidLimits {
                field: "write_timeout",
            });
        }
        if max_request_bytes == 0 || max_request_bytes == usize::MAX {
            return Err(TqLoopbackError::InvalidLimits {
                field: "max_request_bytes",
            });
        }
        if max_response_bytes == 0 || max_response_bytes == usize::MAX {
            return Err(TqLoopbackError::InvalidLimits {
                field: "max_response_bytes",
            });
        }
        Ok(Self {
            connect_timeout,
            read_timeout,
            write_timeout,
            max_request_bytes,
            max_response_bytes,
        })
    }
}

/// One-request-at-a-time synchronous TQ loopback primitive.
pub struct TqLoopbackClient {
    agent: ureq::Agent,
    limits: TqLoopbackLimits,
    endpoint: &'static str,
    request_lock: Mutex<()>,
}

impl TqLoopbackClient {
    /// Creates a client pinned to TQ_LOOPBACK_ENDPOINT.
    pub fn new(limits: TqLoopbackLimits) -> Self {
        Self::at_endpoint(limits, TQ_LOOPBACK_ENDPOINT)
    }

    /// Validates every requested equity against one complete vendor-defined
    /// all-A-share universe before any market observation is accepted.
    pub fn validate_equity_watchlist(
        &self,
        request_id: u64,
        instruments: &[TqInstrument],
    ) -> Result<TqEquityUniverseEvidence, TqLoopbackError> {
        if request_id == 0 {
            return Err(TqLoopbackError::InvalidRequest {
                reason: "request id must be greater than zero",
            });
        }
        if instruments.is_empty() {
            return Err(TqLoopbackError::InvalidRequest {
                reason: "equity watchlist must be non-empty",
            });
        }
        let request = TqRequest {
            id: request_id,
            method: TqReadMethod::EquityUniverse.wire_name(),
            params: EquityUniverseParams {
                market: "5",
                list_type: 0,
            },
        };
        let response = self.execute(&request)?;
        parse_equity_universe_response(&response, request_id, instruments)
    }

    /// Performs exactly one synchronous price/volume request for one exact
    /// instrument and maps only the fields whose semantics are proven.
    pub fn poll_price_volume(
        &self,
        request_id: u64,
        bridge_sequence: u64,
        instrument: &TqInstrument,
        observed_at_utc: impl Into<String>,
    ) -> Result<SourceObservation, TqLoopbackError> {
        let observations = self.poll_price_volumes(
            request_id,
            bridge_sequence,
            std::slice::from_ref(instrument),
            observed_at_utc,
        )?;
        observations
            .into_iter()
            .next()
            .ok_or(TqLoopbackError::Schema {
                reason: "single-instrument price-volume response was empty",
            })
    }

    /// Performs one bounded single-flight request for the complete requested
    /// watchlist. The response identity set must match exactly; observations
    /// retain request order, one local observation time and checked consecutive
    /// bridge sequences.
    pub fn poll_price_volumes(
        &self,
        request_id: u64,
        first_bridge_sequence: u64,
        instruments: &[TqInstrument],
        observed_at_utc: impl Into<String>,
    ) -> Result<Vec<SourceObservation>, TqLoopbackError> {
        if instruments.is_empty() {
            return Err(TqLoopbackError::InvalidRequest {
                reason: "price-volume watchlist must be non-empty",
            });
        }
        let symbols = instruments
            .iter()
            .map(TqInstrument::wire_symbol)
            .collect::<Vec<_>>();
        if symbols.iter().collect::<BTreeSet<_>>().len() != symbols.len() {
            return Err(TqLoopbackError::InvalidRequest {
                reason: "price-volume watchlist must not contain duplicates",
            });
        }
        validate_poll_request(request_id, first_bridge_sequence)?;
        let observed_at_utc = validate_observation_time(observed_at_utc.into())?;
        let request = TqRequest {
            id: request_id,
            method: TqReadMethod::PriceVolume.wire_name(),
            params: PriceVolumeParams {
                stock_list: symbols,
            },
        };
        let response = self.execute(&request)?;
        parse_price_volume_responses(
            &response,
            request_id,
            first_bridge_sequence,
            instruments,
            observed_at_utc,
        )
    }

    /// Performs exactly one synchronous market-snapshot request for one exact
    /// instrument. TQ reports `Amount` in ten-thousand CNY units, so this path
    /// converts it exactly to CNY with checked decimal arithmetic.
    pub fn poll_market_snapshot(
        &self,
        request_id: u64,
        bridge_sequence: u64,
        instrument: &TqInstrument,
        observed_at_utc: impl Into<String>,
    ) -> Result<TqMarketSnapshot, TqLoopbackError> {
        validate_poll_request(request_id, bridge_sequence)?;
        let observed_at_utc = validate_observation_time(observed_at_utc.into())?;
        let request = TqRequest {
            id: request_id,
            method: TqReadMethod::MarketSnapshot.wire_name(),
            params: MarketSnapshotParams {
                stock_code: instrument.wire_symbol(),
                field_list: MARKET_SNAPSHOT_FIELDS,
            },
        };
        let response = self.execute(&request)?;
        parse_market_snapshot_response(
            &response,
            request_id,
            bridge_sequence,
            instrument,
            observed_at_utc,
        )
    }

    fn at_endpoint(limits: TqLoopbackLimits, endpoint: &'static str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .try_proxy_from_env(false)
            .redirects(0)
            .timeout_connect(limits.connect_timeout)
            .timeout_read(limits.read_timeout)
            .timeout_write(limits.write_timeout)
            .build();
        Self {
            agent,
            limits,
            endpoint,
            request_lock: Mutex::new(()),
        }
    }

    fn execute<P: Serialize>(&self, request: &TqRequest<P>) -> Result<Vec<u8>, TqLoopbackError> {
        let _guard = self
            .request_lock
            .lock()
            .map_err(|_| TqLoopbackError::Synchronization)?;
        let encoded = serde_json::to_vec(request).map_err(TqLoopbackError::EncodeJson)?;
        if encoded.len() > self.limits.max_request_bytes {
            return Err(TqLoopbackError::RequestTooLarge {
                actual: encoded.len(),
                maximum: self.limits.max_request_bytes,
            });
        }

        let response = match self
            .agent
            .post(self.endpoint)
            .set("Content-Type", "application/json")
            .send_bytes(&encoded)
        {
            Ok(response) => response,
            Err(ureq::Error::Status(status, _)) => {
                return Err(TqLoopbackError::HttpStatus { status });
            }
            Err(ureq::Error::Transport(error)) => return Err(map_transport_error(&error)),
        };
        if response.status() != 200 {
            return Err(TqLoopbackError::HttpStatus {
                status: response.status(),
            });
        }
        let content_type = response
            .header("Content-Type")
            .ok_or(TqLoopbackError::MissingContentType)?;
        if !is_json_content_type(content_type) {
            return Err(TqLoopbackError::InvalidContentType);
        }

        let read_limit = self.limits.max_response_bytes + 1;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(
                u64::try_from(read_limit).map_err(|_| TqLoopbackError::InvalidLimits {
                    field: "max_response_bytes",
                })?,
            )
            .read_to_end(&mut bytes)
            .map_err(map_read_error)?;
        if bytes.len() > self.limits.max_response_bytes {
            return Err(TqLoopbackError::ResponseTooLarge {
                maximum: self.limits.max_response_bytes,
            });
        }
        Ok(bytes)
    }
}

#[derive(Serialize)]
struct TqRequest<P> {
    id: u64,
    method: &'static str,
    params: P,
}

#[derive(Serialize)]
struct PriceVolumeParams {
    stock_list: Vec<String>,
}

#[derive(Serialize)]
struct EquityUniverseParams {
    market: &'static str,
    list_type: u8,
}

#[derive(Serialize)]
struct MarketSnapshotParams {
    stock_code: String,
    field_list: [&'static str; 7],
}

/// The vendor may add bounded diagnostic envelope members. Only `id` and
/// `result` participate in this client's correlation and schema contract.
#[derive(Deserialize)]
struct TqResponseEnvelope {
    id: u64,
    result: serde_json::Value,
}

/// Required price-volume RPC members. Unmapped result members are ignored so
/// vendor additions do not widen the normalized observation contract.
#[derive(Deserialize)]
struct PriceVolumeResult {
    #[serde(rename = "ErrorId")]
    error_id: String,
    #[serde(rename = "Error", default)]
    error: Option<String>,
    #[serde(rename = "Value", default)]
    value: Option<BTreeMap<String, PriceVolumeRow>>,
}

/// Required fields for the one requested instrument. Unmapped quote fields
/// remain bounded by the response-body limit and are deliberately ignored.
#[derive(Deserialize)]
struct PriceVolumeRow {
    #[serde(rename = "LastClose")]
    last_close: String,
    #[serde(rename = "Now")]
    now: String,
    #[serde(rename = "Volume")]
    volume: String,
}

/// TQ may add unrelated snapshot fields even when a minimal `field_list` was
/// requested. The required fields below stay strict, while Serde deliberately
/// ignores additional result members for bounded forward compatibility.
#[derive(Deserialize)]
struct MarketSnapshotResult {
    #[serde(rename = "ErrorId")]
    error_id: String,
    #[serde(rename = "Error", default)]
    error: Option<String>,
    #[serde(rename = "Amount")]
    amount: String,
    #[serde(rename = "LastClose")]
    last_close: String,
    #[serde(rename = "Now")]
    now: String,
    #[serde(rename = "Volume")]
    volume: String,
    #[serde(rename = "Open")]
    open: String,
    #[serde(rename = "Max")]
    high: String,
    #[serde(rename = "Min")]
    low: String,
}

#[derive(Deserialize)]
struct RpcResultHeader {
    #[serde(rename = "ErrorId")]
    error_id: String,
    #[serde(rename = "Error", default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct EquityUniverseResult {
    #[serde(rename = "ErrorId")]
    error_id: String,
    #[serde(rename = "Error", default)]
    error: Option<String>,
    #[serde(rename = "Value", default)]
    value: Option<Vec<String>>,
}

fn parse_equity_universe_response(
    bytes: &[u8],
    expected_id: u64,
    requested: &[TqInstrument],
) -> Result<TqEquityUniverseEvidence, TqLoopbackError> {
    let json: serde_json::Value = serde_json::from_slice(bytes).map_err(TqLoopbackError::Json)?;
    let envelope: TqResponseEnvelope =
        serde_json::from_value(json).map_err(|_| TqLoopbackError::Schema {
            reason: "equity-universe response does not match the exact id/result schema",
        })?;
    if envelope.id != expected_id {
        return Err(TqLoopbackError::CorrelationMismatch {
            expected: expected_id,
            actual: envelope.id,
        });
    }
    let result: EquityUniverseResult =
        serde_json::from_value(envelope.result).map_err(|_| TqLoopbackError::Schema {
            reason: "equity-universe result does not match the exact schema",
        })?;
    if result.error_id != "0" {
        return Err(TqLoopbackError::Rpc {
            error_id: result.error_id,
            message: result.error,
        });
    }
    let rows = result.value.ok_or(TqLoopbackError::Schema {
        reason: "equity-universe Value must be an array",
    })?;
    if rows.is_empty() {
        return Err(TqLoopbackError::Schema {
            reason: "equity-universe Value must be non-empty",
        });
    }
    let mut universe = BTreeSet::new();
    for identity in rows {
        if !is_canonical_equity_identity(&identity) {
            return Err(TqLoopbackError::Schema {
                reason: "equity-universe identity must be canonical dddddd.SH|SZ|BJ",
            });
        }
        if !universe.insert(identity) {
            return Err(TqLoopbackError::Schema {
                reason: "equity-universe contains a duplicate identity",
            });
        }
    }
    for instrument in requested {
        let identity = instrument.wire_symbol();
        if !universe.contains(&identity) {
            return Err(TqLoopbackError::InstrumentIdentity {
                instrument: identity,
            });
        }
    }
    Ok(TqEquityUniverseEvidence {
        instrument_count: universe.len(),
    })
}

fn is_canonical_equity_identity(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 9
        && bytes[..6].iter().all(u8::is_ascii_digit)
        && matches!(&bytes[6..], b".SH" | b".SZ" | b".BJ")
}

fn validate_poll_request(request_id: u64, bridge_sequence: u64) -> Result<(), TqLoopbackError> {
    if request_id == 0 {
        return Err(TqLoopbackError::InvalidRequest {
            reason: "request id must be greater than zero",
        });
    }
    if bridge_sequence == 0 {
        return Err(TqLoopbackError::InvalidRequest {
            reason: "bridge sequence must be greater than zero",
        });
    }
    Ok(())
}

fn validate_observation_time(value: String) -> Result<String, TqLoopbackError> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(TqLoopbackError::InvalidRequest {
            reason: "observation time must be unpadded non-control text",
        });
    }
    Ok(value)
}

#[cfg(test)]
fn parse_price_volume_response(
    bytes: &[u8],
    expected_id: u64,
    bridge_sequence: u64,
    expected_instrument: &TqInstrument,
    observed_at_utc: String,
) -> Result<SourceObservation, TqLoopbackError> {
    parse_price_volume_responses(
        bytes,
        expected_id,
        bridge_sequence,
        std::slice::from_ref(expected_instrument),
        observed_at_utc,
    )?
    .into_iter()
    .next()
    .ok_or(TqLoopbackError::Schema {
        reason: "single-instrument price-volume response was empty",
    })
}

fn parse_price_volume_responses(
    bytes: &[u8],
    expected_id: u64,
    first_bridge_sequence: u64,
    expected_instruments: &[TqInstrument],
    observed_at_utc: String,
) -> Result<Vec<SourceObservation>, TqLoopbackError> {
    let json: serde_json::Value = serde_json::from_slice(bytes).map_err(TqLoopbackError::Json)?;
    let envelope: TqResponseEnvelope =
        serde_json::from_value(json).map_err(|_| TqLoopbackError::Schema {
            reason: "response envelope does not match the exact id/result schema",
        })?;
    if envelope.id != expected_id {
        return Err(TqLoopbackError::CorrelationMismatch {
            expected: expected_id,
            actual: envelope.id,
        });
    }
    let result: PriceVolumeResult =
        serde_json::from_value(envelope.result).map_err(|_| TqLoopbackError::Schema {
            reason: "price-volume result does not match the exact schema",
        })?;
    if result.error_id != "0" {
        return Err(TqLoopbackError::Rpc {
            error_id: result.error_id,
            message: result.error,
        });
    }
    let mut values = result.value.ok_or(TqLoopbackError::Schema {
        reason: "successful price-volume result is missing Value",
    })?;
    if values.len() != expected_instruments.len() {
        return Err(TqLoopbackError::Schema {
            reason: "price-volume result cardinality does not match the request",
        });
    }
    let mut observations = Vec::with_capacity(expected_instruments.len());
    for (index, expected_instrument) in expected_instruments.iter().enumerate() {
        let expected_symbol = expected_instrument.wire_symbol();
        let row = values
            .remove(&expected_symbol)
            .ok_or(TqLoopbackError::Schema {
                reason: "price-volume result identity set does not match the request",
            })?;
        validate_decimal("LastClose", &row.last_close, ObservationUnit::CnyPerShare)?;
        validate_decimal("Now", &row.now, ObservationUnit::CnyPerShare)?;
        validate_decimal("Volume", &row.volume, ObservationUnit::Lot)?;
        let sequence_offset =
            u64::try_from(index).map_err(|_| TqLoopbackError::InvalidRequest {
                reason: "price-volume watchlist length is not representable",
            })?;
        let bridge_sequence = first_bridge_sequence.checked_add(sequence_offset).ok_or(
            TqLoopbackError::InvalidRequest {
                reason: "price-volume bridge sequence range is exhausted",
            },
        )?;
        let observation = SourceObservation {
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
            bridge_sequence,
            instrument: expected_instrument.instrument.clone(),
            observed_at_utc: observed_at_utc.clone(),
            source_timestamp: None,
            price: Some(DecimalObservation {
                value: row.now,
                unit: ObservationUnit::CnyPerShare,
            }),
            cumulative_amount: None,
            cumulative_volume: Some(DecimalObservation {
                value: row.volume,
                unit: ObservationUnit::Lot,
            }),
            source_record_count: None,
        };
        observation
            .validate()
            .map_err(|_| TqLoopbackError::Schema {
                reason: "mapped observation failed protocol validation",
            })?;
        observations.push(observation);
    }
    debug_assert!(values.is_empty());
    Ok(observations)
}

fn parse_market_snapshot_response(
    bytes: &[u8],
    expected_id: u64,
    bridge_sequence: u64,
    expected_instrument: &TqInstrument,
    observed_at_utc: String,
) -> Result<TqMarketSnapshot, TqLoopbackError> {
    let json: serde_json::Value = serde_json::from_slice(bytes).map_err(TqLoopbackError::Json)?;
    let envelope: TqResponseEnvelope =
        serde_json::from_value(json).map_err(|_| TqLoopbackError::Schema {
            reason: "response envelope does not match the exact id/result schema",
        })?;
    if envelope.id != expected_id {
        return Err(TqLoopbackError::CorrelationMismatch {
            expected: expected_id,
            actual: envelope.id,
        });
    }

    let header: RpcResultHeader =
        serde_json::from_value(envelope.result.clone()).map_err(|_| TqLoopbackError::Schema {
            reason: "market-snapshot result is missing a string ErrorId",
        })?;
    if header.error_id != "0" {
        return Err(TqLoopbackError::Rpc {
            error_id: header.error_id,
            message: header.error,
        });
    }
    let result: MarketSnapshotResult =
        serde_json::from_value(envelope.result).map_err(|_| TqLoopbackError::Schema {
            reason: "successful market-snapshot result is missing a required string field",
        })?;
    debug_assert_eq!(result.error_id, "0");
    let _rpc_message = result.error;

    validate_decimal(
        "LastClose",
        &result.last_close,
        ObservationUnit::CnyPerShare,
    )?;
    validate_decimal("Now", &result.now, ObservationUnit::CnyPerShare)?;
    validate_decimal("Volume", &result.volume, ObservationUnit::Lot)?;
    validate_decimal("Open", &result.open, ObservationUnit::CnyPerShare)?;
    validate_decimal("Max", &result.high, ObservationUnit::CnyPerShare)?;
    validate_decimal("Min", &result.low, ObservationUnit::CnyPerShare)?;
    let amount_cny = ten_thousand_cny_to_cny(&result.amount)?;

    let observation = SourceObservation {
        protocol_version: PROTOCOL_VERSION,
        schema_version: SCHEMA_VERSION,
        bridge_sequence,
        // The single-symbol response has no independent instrument member;
        // identity is therefore correlated only to the validated request.
        instrument: expected_instrument.instrument.clone(),
        observed_at_utc,
        source_timestamp: None,
        price: Some(DecimalObservation {
            value: result.now,
            unit: ObservationUnit::CnyPerShare,
        }),
        cumulative_amount: Some(DecimalObservation {
            value: amount_cny,
            unit: ObservationUnit::Cny,
        }),
        cumulative_volume: Some(DecimalObservation {
            value: result.volume,
            unit: ObservationUnit::Lot,
        }),
        source_record_count: None,
    };
    TqMarketSnapshot::new(
        observation,
        TqSnapshotPrices {
            previous_close: DecimalObservation {
                value: result.last_close,
                unit: ObservationUnit::CnyPerShare,
            },
            open: DecimalObservation {
                value: result.open,
                unit: ObservationUnit::CnyPerShare,
            },
            high: DecimalObservation {
                value: result.high,
                unit: ObservationUnit::CnyPerShare,
            },
            low: DecimalObservation {
                value: result.low,
                unit: ObservationUnit::CnyPerShare,
            },
        },
    )
}

/// Converts exact canonical decimal text from ten-thousand CNY to CNY by
/// moving the decimal point four places. The checked u128 coefficient rejects
/// values outside this crate's exact representation instead of rounding.
fn ten_thousand_cny_to_cny(value: &str) -> Result<String, TqLoopbackError> {
    let (integer, fraction) = match value.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (value, None),
    };
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
        || fraction.is_some_and(|fraction| {
            fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return Err(TqLoopbackError::Schema {
            reason: "market-snapshot Amount is not canonical non-negative decimal text",
        });
    }
    let fraction = fraction.unwrap_or_default();
    let digit_count = integer
        .len()
        .checked_add(fraction.len())
        .ok_or(TqLoopbackError::Schema {
            reason: "market-snapshot Amount exceeds exact decimal capacity",
        })?;
    if digit_count > MAX_EXACT_DECIMAL_DIGITS {
        return Err(TqLoopbackError::Schema {
            reason: "market-snapshot Amount exceeds exact decimal capacity",
        });
    }

    let mut coefficient = 0_u128;
    for digit in integer.bytes().chain(fraction.bytes()) {
        coefficient = coefficient
            .checked_mul(10)
            .and_then(|current| current.checked_add(u128::from(digit - b'0')))
            .ok_or(TqLoopbackError::Schema {
                reason: "market-snapshot Amount exceeds exact decimal capacity",
            })?;
    }

    let mut scale = fraction.len();
    if scale >= 4 {
        scale -= 4;
    } else {
        for _ in 0..(4 - scale) {
            coefficient = coefficient.checked_mul(10).ok_or(TqLoopbackError::Schema {
                reason: "market-snapshot Amount overflows exact CNY conversion",
            })?;
        }
        scale = 0;
    }
    Ok(format_canonical_decimal(coefficient, scale))
}

fn format_canonical_decimal(mut coefficient: u128, mut scale: usize) -> String {
    while scale > 0 && coefficient.is_multiple_of(10) {
        coefficient /= 10;
        scale -= 1;
    }
    let digits = coefficient.to_string();
    if scale == 0 {
        return digits;
    }
    if digits.len() > scale {
        let split = digits.len() - scale;
        return format!("{}.{}", &digits[..split], &digits[split..]);
    }
    format!("0.{}{}", "0".repeat(scale - digits.len()), digits)
}

fn validate_decimal(
    field: &'static str,
    value: &str,
    unit: ObservationUnit,
) -> Result<(), TqLoopbackError> {
    DecimalObservation {
        value: value.to_owned(),
        unit,
    }
    .validate(field)
    .map_err(|_| TqLoopbackError::Schema {
        reason: "TQ result contains an invalid decimal",
    })
}

fn is_json_content_type(content_type: &str) -> bool {
    let mut parts = content_type.split(';').map(str::trim);
    if !parts
        .next()
        .is_some_and(|value| value.eq_ignore_ascii_case("application/json"))
    {
        return false;
    }
    let mut saw_charset = false;
    for parameter in parts {
        if saw_charset || !parameter.eq_ignore_ascii_case("charset=utf-8") {
            return false;
        }
        saw_charset = true;
    }
    true
}

fn map_transport_error(error: &ureq::Transport) -> TqLoopbackError {
    if matches!(
        error.kind(),
        ureq::ErrorKind::ConnectionFailed | ureq::ErrorKind::Dns
    ) {
        return TqLoopbackError::Connect;
    }
    if error_chain_is_connection_failure(error) {
        return TqLoopbackError::Connect;
    }
    if error_chain_is_timeout(error) {
        return TqLoopbackError::Timeout;
    }
    TqLoopbackError::Transport {
        kind: error.kind().to_string(),
    }
}

fn map_read_error(error: std::io::Error) -> TqLoopbackError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ) {
        TqLoopbackError::Timeout
    } else {
        TqLoopbackError::Read
    }
}

fn error_chain_is_timeout(error: &(dyn StdError + 'static)) -> bool {
    error_chain_has_io_kind(error, |kind| {
        matches!(
            kind,
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        )
    })
}

fn error_chain_is_connection_failure(error: &(dyn StdError + 'static)) -> bool {
    error_chain_has_io_kind(error, |kind| {
        matches!(
            kind,
            std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::NotConnected
                | std::io::ErrorKind::AddrNotAvailable
        )
    })
}

fn error_chain_has_io_kind(
    error: &(dyn StdError + 'static),
    predicate: impl Fn(std::io::ErrorKind) -> bool,
) -> bool {
    let mut current = Some(error);
    while let Some(source) = current {
        if source
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_error| predicate(io_error.kind()))
        {
            return true;
        }
        current = source.source();
    }
    false
}

/// Stable machine-readable classification for loopback failures.
///
/// This type lets callers preserve failure taxonomy without parsing display
/// text or serializing source errors embedded in [`TqLoopbackError`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TqLoopbackErrorCategory {
    InvalidLimits,
    InvalidRequest,
    RequestEncoding,
    RequestTooLarge,
    Connect,
    Timeout,
    Transport,
    HttpStatus,
    MissingContentType,
    InvalidContentType,
    ResponseTooLarge,
    Read,
    InvalidJson,
    Rpc,
    CorrelationMismatch,
    InstrumentIdentity,
    Schema,
    Synchronization,
}

/// Typed loopback transport, RPC and schema failures.
#[derive(Debug, Error)]
pub enum TqLoopbackError {
    #[error("invalid positive loopback limit: {field}")]
    InvalidLimits { field: &'static str },
    #[error("invalid read-only TQ request: {reason}")]
    InvalidRequest { reason: &'static str },
    #[error("unable to encode the bounded TQ request: {0}")]
    EncodeJson(#[source] serde_json::Error),
    #[error("TQ request size {actual} exceeds maximum {maximum}")]
    RequestTooLarge { actual: usize, maximum: usize },
    #[error("unable to connect to the TQ loopback endpoint")]
    Connect,
    #[error("TQ loopback operation timed out")]
    Timeout,
    #[error("TQ loopback transport failed: {kind}")]
    Transport { kind: String },
    #[error("TQ loopback returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("TQ loopback response is missing Content-Type")]
    MissingContentType,
    #[error("TQ loopback response Content-Type is not strict JSON")]
    InvalidContentType,
    #[error("TQ loopback response exceeds maximum {maximum}")]
    ResponseTooLarge { maximum: usize },
    #[error("unable to read the TQ loopback response")]
    Read,
    #[error("TQ loopback response is not JSON: {0}")]
    Json(#[source] serde_json::Error),
    #[error("TQ RPC failed with ErrorId={error_id}")]
    Rpc {
        error_id: String,
        message: Option<String>,
    },
    #[error("TQ response id mismatch: expected {expected}, received {actual}")]
    CorrelationMismatch { expected: u64, actual: u64 },
    #[error("requested instrument is absent from the complete TQ A-share universe: {instrument}")]
    InstrumentIdentity { instrument: String },
    #[error("TQ response schema mismatch: {reason}")]
    Schema { reason: &'static str },
    #[error("TQ request serialization lock is poisoned")]
    Synchronization,
}

impl TqLoopbackError {
    /// Returns the stable structural category for this failure.
    pub const fn category(&self) -> TqLoopbackErrorCategory {
        match self {
            Self::InvalidLimits { .. } => TqLoopbackErrorCategory::InvalidLimits,
            Self::InvalidRequest { .. } => TqLoopbackErrorCategory::InvalidRequest,
            Self::EncodeJson(_) => TqLoopbackErrorCategory::RequestEncoding,
            Self::RequestTooLarge { .. } => TqLoopbackErrorCategory::RequestTooLarge,
            Self::Connect => TqLoopbackErrorCategory::Connect,
            Self::Timeout => TqLoopbackErrorCategory::Timeout,
            Self::Transport { .. } => TqLoopbackErrorCategory::Transport,
            Self::HttpStatus { .. } => TqLoopbackErrorCategory::HttpStatus,
            Self::MissingContentType => TqLoopbackErrorCategory::MissingContentType,
            Self::InvalidContentType => TqLoopbackErrorCategory::InvalidContentType,
            Self::ResponseTooLarge { .. } => TqLoopbackErrorCategory::ResponseTooLarge,
            Self::Read => TqLoopbackErrorCategory::Read,
            Self::Json(_) => TqLoopbackErrorCategory::InvalidJson,
            Self::Rpc { .. } => TqLoopbackErrorCategory::Rpc,
            Self::CorrelationMismatch { .. } => TqLoopbackErrorCategory::CorrelationMismatch,
            Self::InstrumentIdentity { .. } => TqLoopbackErrorCategory::InstrumentIdentity,
            Self::Schema { .. } => TqLoopbackErrorCategory::Schema,
            Self::Synchronization => TqLoopbackErrorCategory::Synchronization,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    const RESPONSE: &str = concat!(
        r#"{"id":7,"result":{"ErrorId":"0","Value":{"600396.SH":{"#,
        r#""LastClose":"16.80","Now":"17.18","Volume":"2543527"}}}}"#
    );
    const PRICE_VOLUME_WITH_VENDOR_ADDITIONS: &str = r#"{"id":7,"trace_id":"vendor-diagnostic","result":{"ErrorId":"0","ServerVersion":"future","Value":{"600396.SH":{"LastClose":"16.80","Now":"17.18","Volume":"2543527","Open":"16.76","Bid1":"17.17"}}}}"#;
    const MARKET_SNAPSHOT_RESPONSE: &str =
        include_str!("../tests/fixtures/tq_market_snapshot_success.json");

    fn limits(response_bytes: usize) -> TqLoopbackLimits {
        TqLoopbackLimits::new(
            Duration::from_millis(100),
            Duration::from_millis(100),
            Duration::from_millis(100),
            1024,
            response_bytes,
        )
        .unwrap()
    }

    fn instrument() -> TqInstrument {
        TqInstrument::new(SourceExchange::Shanghai, "600396").unwrap()
    }

    fn spawn_server(
        status: &'static str,
        headers: &'static str,
        body: &'static str,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            write!(
                stream,
                "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            stream.flush().unwrap();
            request
        });
        (format!("http://{address}/"), handle)
    }

    fn read_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        let header_end = loop {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0);
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(position) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                break position + 4;
            }
        };
        let header = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
        let content_length = header
            .lines()
            .find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("Content-Length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
            })
            .unwrap();
        while bytes.len() - header_end < content_length {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0);
            bytes.extend_from_slice(&buffer[..read]);
        }
        String::from_utf8(bytes).unwrap()
    }

    fn test_client(endpoint: String, limits: TqLoopbackLimits) -> TqLoopbackClient {
        let endpoint: &'static str = Box::leak(endpoint.into_boxed_str());
        TqLoopbackClient::at_endpoint(limits, endpoint)
    }

    #[test]
    fn endpoint_and_method_allowlist_are_exact() {
        assert_eq!(TQ_LOOPBACK_ENDPOINT, "http://127.0.0.1:17709/");
        assert_eq!(TqReadMethod::EquityUniverse.wire_name(), "get_stock_list");
        assert_eq!(TqReadMethod::PriceVolume.wire_name(), "get_pricevol");
        assert_eq!(
            TqReadMethod::MarketSnapshot.wire_name(),
            "get_market_snapshot"
        );
    }

    #[test]
    fn equity_universe_request_is_exact_and_watchlist_must_be_present() {
        let body =
            r#"{"id":7,"result":{"ErrorId":"0","Value":["000001.SZ","600396.SH","920118.BJ"]}}"#;
        let (endpoint, server) = spawn_server(
            "200 OK",
            "Content-Type: application/json; charset=utf-8\r\n",
            body,
        );
        let watchlist = [
            TqInstrument::new(SourceExchange::Shanghai, "600396").unwrap(),
            TqInstrument::new(SourceExchange::Shenzhen, "000001").unwrap(),
        ];
        let evidence = test_client(endpoint, limits(4096))
            .validate_equity_watchlist(7, &watchlist)
            .unwrap();
        assert_eq!(evidence.instrument_count(), 3);
        let request = server.join().unwrap();
        let value: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(value["method"], "get_stock_list");
        assert_eq!(
            value["params"],
            serde_json::json!({"market":"5","list_type":0})
        );

        assert!(matches!(
            parse_equity_universe_response(
                body.as_bytes(),
                7,
                &[TqInstrument::new(SourceExchange::Shanghai, "999999").unwrap()]
            ),
            Err(TqLoopbackError::InstrumentIdentity { instrument }) if instrument == "999999.SH"
        ));
    }

    #[test]
    fn equity_universe_rejects_empty_duplicate_malformed_rpc_and_id() {
        let requested = [instrument()];
        for body in [
            r#"{"id":7,"result":{"ErrorId":"0","Value":[]}}"#,
            r#"{"id":7,"result":{"ErrorId":"0","Value":["600396.SH","600396.SH"]}}"#,
            r#"{"id":7,"result":{"ErrorId":"0","Value":["600396.US"]}}"#,
            r#"{"id":7,"result":{"ErrorId":"0","Value":["60039.SH"]}}"#,
            r#"{"id":7,"result":{"ErrorId":"0","Value":null}}"#,
        ] {
            assert!(matches!(
                parse_equity_universe_response(body.as_bytes(), 7, &requested),
                Err(TqLoopbackError::Schema { .. })
            ));
        }
        assert!(matches!(
            parse_equity_universe_response(
                br#"{"id":8,"result":{"ErrorId":"0","Value":["600396.SH"]}}"#,
                7,
                &requested
            ),
            Err(TqLoopbackError::CorrelationMismatch { .. })
        ));
        assert!(matches!(
            parse_equity_universe_response(
                br#"{"id":7,"result":{"ErrorId":"20","Error":"unavailable"}}"#,
                7,
                &requested
            ),
            Err(TqLoopbackError::Rpc { .. })
        ));
    }

    #[test]
    fn market_snapshot_request_is_minimal_and_maps_explicit_units() {
        let (endpoint, server) = spawn_server(
            "200 OK",
            "Content-Type: application/json; charset=utf-8\r\n",
            MARKET_SNAPSHOT_RESPONSE,
        );
        let requested_instrument = instrument();
        let snapshot = test_client(endpoint, limits(4096))
            .poll_market_snapshot(7, 1, &requested_instrument, "2026-08-13T01:02:03Z")
            .unwrap();
        assert_eq!(
            snapshot.previous_close(),
            &DecimalObservation {
                value: "17.18".into(),
                unit: ObservationUnit::CnyPerShare,
            }
        );
        assert_eq!(snapshot.prices().open.value, "16.80");
        assert_eq!(snapshot.prices().high.value, "17.66");
        assert_eq!(snapshot.prices().low.value, "16.80");
        let (observation, prices) = snapshot.into_parts();
        assert_eq!(prices.previous_close.value, "17.18");

        assert_eq!(
            observation.instrument,
            requested_instrument.source_instrument().clone()
        );
        assert_eq!(
            observation.price.unwrap(),
            DecimalObservation {
                value: "17.62".into(),
                unit: ObservationUnit::CnyPerShare,
            }
        );
        assert_eq!(
            observation.cumulative_amount.unwrap(),
            DecimalObservation {
                value: "1273546500".into(),
                unit: ObservationUnit::Cny,
            }
        );
        assert_eq!(
            observation.cumulative_volume.unwrap(),
            DecimalObservation {
                value: "735536".into(),
                unit: ObservationUnit::Lot,
            }
        );
        assert!(observation.source_timestamp.is_none());
        assert!(observation.source_record_count.is_none());

        let request = server.join().unwrap();
        let body = request.split_once("\r\n\r\n").unwrap().1;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(value["id"], 7);
        assert_eq!(value["method"], "get_market_snapshot");
        assert_eq!(value["params"]["stock_code"], "600396.SH");
        assert_eq!(
            value["params"]["field_list"],
            serde_json::json!(["Amount", "Now", "Volume", "LastClose", "Open", "Max", "Min"])
        );
        assert_eq!(value["params"].as_object().unwrap().len(), 2);
        assert_eq!(value.as_object().unwrap().len(), 3);
    }

    #[test]
    fn ten_thousand_cny_conversion_is_exact_and_checked() {
        assert_eq!(ten_thousand_cny_to_cny("12").unwrap(), "120000");
        assert_eq!(ten_thousand_cny_to_cny("12.3456").unwrap(), "123456");
        assert_eq!(ten_thousand_cny_to_cny("12.34567").unwrap(), "123456.7");
        assert_eq!(ten_thousand_cny_to_cny("0.00001").unwrap(), "0.1");
        assert_eq!(ten_thousand_cny_to_cny("0.0000").unwrap(), "0");

        let overflow = u128::MAX.to_string();
        assert!(matches!(
            ten_thousand_cny_to_cny(&overflow),
            Err(TqLoopbackError::Schema {
                reason: "market-snapshot Amount overflows exact CNY conversion"
            })
        ));
        assert!(matches!(
            ten_thousand_cny_to_cny(&"1".repeat(MAX_EXACT_DECIMAL_DIGITS + 1)),
            Err(TqLoopbackError::Schema {
                reason: "market-snapshot Amount exceeds exact decimal capacity"
            })
        ));
        for invalid in ["", "-1", "+1", ".1", "1.", "01", "1e3", "1.2.3"] {
            assert!(matches!(
                ten_thousand_cny_to_cny(invalid),
                Err(TqLoopbackError::Schema {
                    reason: "market-snapshot Amount is not canonical non-negative decimal text"
                })
            ));
        }
    }

    #[test]
    fn market_snapshot_rpc_schema_and_id_fail_closed() {
        let parse = |body: &str| {
            parse_market_snapshot_response(
                body.as_bytes(),
                7,
                1,
                &instrument(),
                "2026-08-13T01:02:03Z".into(),
            )
        };
        assert!(matches!(
            parse(
                r#"{"id":8,"result":{"ErrorId":"0","Amount":"1","Now":"1","Volume":"1","LastClose":"1"}}"#
            ),
            Err(TqLoopbackError::CorrelationMismatch {
                expected: 7,
                actual: 8
            })
        ));
        assert!(matches!(
            parse(r#"{"id":7,"result":{"ErrorId":"20","Error":"unavailable"}}"#),
            Err(TqLoopbackError::Rpc { error_id, .. }) if error_id == "20"
        ));
        assert!(matches!(
            parse(r#"{"id":7,"result":{"ErrorId":"0","Now":"1","Volume":"1","LastClose":"1"}}"#),
            Err(TqLoopbackError::Schema { .. })
        ));
        assert!(matches!(
            parse(
                r#"{"id":7,"result":{"ErrorId":0,"Amount":"1","Now":"1","Volume":"1","LastClose":"1"}}"#
            ),
            Err(TqLoopbackError::Schema { .. })
        ));
        assert!(matches!(
            parse(
                r#"{"id":7,"result":{"ErrorId":"0","Amount":"1","Now":"NaN","Volume":"1","LastClose":"1"}}"#
            ),
            Err(TqLoopbackError::Schema { .. })
        ));
    }

    #[test]
    fn price_volume_accepts_bounded_unmapped_envelope_result_and_row_fields() {
        let observation = parse_price_volume_response(
            PRICE_VOLUME_WITH_VENDOR_ADDITIONS.as_bytes(),
            7,
            1,
            &instrument(),
            "2026-08-13T01:02:03Z".into(),
        )
        .unwrap();
        assert_eq!(observation.price.unwrap().value, "17.18");
        assert_eq!(observation.cumulative_volume.unwrap().value, "2543527");
    }

    #[test]
    fn price_volume_still_requires_mapped_fields_and_exact_one_instrument() {
        let parse = |body: &str| {
            parse_price_volume_response(
                body.as_bytes(),
                7,
                1,
                &instrument(),
                "2026-08-13T01:02:03Z".into(),
            )
        };
        for missing in [
            r#"{"id":7,"result":{"Value":{"600396.SH":{"LastClose":"1","Now":"1","Volume":"1"}}}}"#,
            r#"{"id":7,"result":{"ErrorId":"0"}}"#,
            r#"{"id":7,"result":{"ErrorId":"0","Value":{"600396.SH":{"Now":"1","Volume":"1"}}}}"#,
            r#"{"id":7,"result":{"ErrorId":"0","Value":{"600396.SH":{"LastClose":"1","Volume":"1"}}}}"#,
            r#"{"id":7,"result":{"ErrorId":"0","Value":{"600396.SH":{"LastClose":"1","Now":"1"}}}}"#,
        ] {
            assert!(matches!(
                parse(missing),
                Err(TqLoopbackError::Schema { .. })
            ));
        }

        assert!(matches!(
            parse(
                r#"{"id":7,"result":{"ErrorId":"0","Value":{"000001.SZ":{"LastClose":"1","Now":"1","Volume":"1"}}}}"#
            ),
            Err(TqLoopbackError::Schema { .. })
        ));
        assert!(matches!(
            parse(
                r#"{"id":7,"result":{"ErrorId":"0","Value":{"600396.SH":{"LastClose":"1","Now":"1","Volume":"1"},"000001.SZ":{"LastClose":"1","Now":"1","Volume":"1"}}}}"#
            ),
            Err(TqLoopbackError::Schema { .. })
        ));
    }

    #[test]
    fn error_category_is_stable_and_serializable_without_display_parsing() {
        assert_eq!(
            TqLoopbackError::Timeout.category(),
            TqLoopbackErrorCategory::Timeout
        );
        assert_eq!(
            TqLoopbackError::Schema { reason: "fixture" }.category(),
            TqLoopbackErrorCategory::Schema
        );
        assert_eq!(
            serde_json::to_string(&TqLoopbackErrorCategory::CorrelationMismatch).unwrap(),
            r#""correlation_mismatch""#
        );
    }

    #[test]
    fn batch_price_volume_is_one_request_with_exact_identity_set_and_request_order() {
        let body = r#"{"id":7,"result":{"ErrorId":"0","Value":{"000001.SZ":{"LastClose":"10.00","Now":"10.10","Volume":"200"},"600396.SH":{"LastClose":"17.00","Now":"17.18","Volume":"100"}}}}"#;
        let (endpoint, server) = spawn_server(
            "200 OK",
            "Content-Type: application/json; charset=utf-8\r\n",
            body,
        );
        let instruments = [
            TqInstrument::new(SourceExchange::Shanghai, "600396").unwrap(),
            TqInstrument::new(SourceExchange::Shenzhen, "000001").unwrap(),
        ];
        let observations = test_client(endpoint, limits(4096))
            .poll_price_volumes(7, 9, &instruments, "2026-08-13T01:02:03Z")
            .unwrap();
        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].instrument.code, "600396");
        assert_eq!(observations[0].bridge_sequence, 9);
        assert_eq!(observations[0].price.as_ref().unwrap().value, "17.18");
        assert_eq!(observations[1].instrument.code, "000001");
        assert_eq!(observations[1].bridge_sequence, 10);
        assert_eq!(observations[1].price.as_ref().unwrap().value, "10.10");
        assert!(observations
            .iter()
            .all(|observation| observation.observed_at_utc == "2026-08-13T01:02:03Z"));

        let request = server.join().unwrap();
        let value: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(value["method"], "get_pricevol");
        assert_eq!(
            value["params"]["stock_list"],
            serde_json::json!(["600396.SH", "000001.SZ"])
        );
    }

    #[test]
    fn batch_price_volume_rejects_empty_duplicate_and_sequence_exhaustion() {
        let client = test_client("http://127.0.0.1:9/".into(), limits(4096));
        let instrument = instrument();
        assert!(matches!(
            client.poll_price_volumes(7, 1, &[], "2026-08-13T01:02:03Z"),
            Err(TqLoopbackError::InvalidRequest { .. })
        ));
        assert!(matches!(
            client.poll_price_volumes(
                7,
                1,
                &[instrument.clone(), instrument],
                "2026-08-13T01:02:03Z"
            ),
            Err(TqLoopbackError::InvalidRequest { .. })
        ));

        let body = r#"{"id":7,"result":{"ErrorId":"0","Value":{"000001.SZ":{"LastClose":"10","Now":"10","Volume":"2"},"600396.SH":{"LastClose":"17","Now":"17","Volume":"1"}}}}"#;
        let (endpoint, server) = spawn_server("200 OK", "Content-Type: application/json\r\n", body);
        let instruments = [
            TqInstrument::new(SourceExchange::Shanghai, "600396").unwrap(),
            TqInstrument::new(SourceExchange::Shenzhen, "000001").unwrap(),
        ];
        assert!(matches!(
            test_client(endpoint, limits(4096)).poll_price_volumes(
                7,
                u64::MAX,
                &instruments,
                "2026-08-13T01:02:03Z"
            ),
            Err(TqLoopbackError::InvalidRequest { .. })
        ));
        server.join().unwrap();
    }

    #[test]
    fn request_uses_post_root_host_json_and_exact_read_only_shape() {
        let (endpoint, server) = spawn_server(
            "200 OK",
            "Content-Type: application/json; charset=utf-8\r\n",
            RESPONSE,
        );
        let expected_host = endpoint
            .strip_prefix("http://")
            .unwrap()
            .trim_end_matches('/')
            .to_owned();
        let observation = test_client(endpoint, limits(4096))
            .poll_price_volume(7, 1, &instrument(), "2026-08-13T01:02:03Z")
            .unwrap();
        assert_eq!(observation.price.unwrap().value, "17.18");
        assert_eq!(observation.cumulative_volume.unwrap().value, "2543527");
        assert!(observation.cumulative_amount.is_none());
        assert!(observation.source_record_count.is_none());
        assert!(observation.source_timestamp.is_none());

        let request = server.join().unwrap();
        assert!(request.starts_with("POST / HTTP/1.1\r\n"));
        assert!(request
            .lines()
            .any(|line| line.eq_ignore_ascii_case(&format!("Host: {expected_host}"))));
        assert!(request
            .lines()
            .any(|line| line.eq_ignore_ascii_case("Content-Type: application/json")));
        let body = request.split_once("\r\n\r\n").unwrap().1;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(value["id"], 7);
        assert_eq!(value["method"], "get_pricevol");
        assert_eq!(value["params"]["stock_list"][0], "600396.SH");
        assert_eq!(value.as_object().unwrap().len(), 3);
    }

    #[test]
    fn redirect_is_not_followed() {
        let (endpoint, server) = spawn_server(
            "302 Found",
            "Location: http://127.0.0.1:9/redirected\r\nContent-Type: application/json\r\n",
            "{}",
        );
        let error = test_client(endpoint, limits(1024))
            .poll_price_volume(7, 1, &instrument(), "2026-08-13T01:02:03Z")
            .unwrap_err();
        assert!(matches!(error, TqLoopbackError::HttpStatus { status: 302 }));
        server.join().unwrap();
    }

    #[test]
    fn request_and_response_bounds_fail_closed() {
        let client = test_client(
            "http://127.0.0.1:9/".into(),
            TqLoopbackLimits {
                max_request_bytes: 1,
                ..limits(1024)
            },
        );
        assert!(matches!(
            client.poll_price_volume(7, 1, &instrument(), "2026-08-13T01:02:03Z"),
            Err(TqLoopbackError::RequestTooLarge { .. })
        ));

        let (endpoint, server) =
            spawn_server("200 OK", "Content-Type: application/json\r\n", RESPONSE);
        assert!(matches!(
            test_client(endpoint, limits(8)).poll_price_volume(
                7,
                1,
                &instrument(),
                "2026-08-13T01:02:03Z"
            ),
            Err(TqLoopbackError::ResponseTooLarge { maximum: 8 })
        ));
        server.join().unwrap();
    }

    #[test]
    fn timeout_is_typed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(100));
        });
        let short = TqLoopbackLimits::new(
            Duration::from_millis(20),
            Duration::from_millis(20),
            Duration::from_millis(20),
            1024,
            1024,
        )
        .unwrap();
        assert!(matches!(
            test_client(endpoint, short).poll_price_volume(
                7,
                1,
                &instrument(),
                "2026-08-13T01:02:03Z"
            ),
            Err(TqLoopbackError::Timeout)
        ));
        server.join().unwrap();
    }

    #[test]
    fn content_type_json_rpc_id_and_schema_failures_are_distinct() {
        let cases = [
            ("", RESPONSE, "missing_content_type"),
            ("Content-Type: text/plain\r\n", RESPONSE, "content_type"),
            ("Content-Type: application/json\r\n", "not json", "json"),
            (
                "Content-Type: application/json\r\n",
                r#"{"id":8,"result":{"ErrorId":"0","Value":{}}}"#,
                "id",
            ),
            (
                "Content-Type: application/json\r\n",
                r#"{"id":7,"result":{"ErrorId":"20","Error":"unavailable"}}"#,
                "rpc",
            ),
            (
                "Content-Type: application/json\r\n",
                r#"{"id":7,"result":{"ErrorId":"0","Value":{"600396.SH":{"LastClose":"16.80","Now":"NaN","Volume":"1"}}}}"#,
                "schema",
            ),
        ];
        for (headers, body, expected) in cases {
            let (endpoint, server) = spawn_server("200 OK", headers, body);
            let error = test_client(endpoint, limits(4096))
                .poll_price_volume(7, 1, &instrument(), "2026-08-13T01:02:03Z")
                .unwrap_err();
            match expected {
                "missing_content_type" => {
                    assert!(matches!(error, TqLoopbackError::MissingContentType))
                }
                "content_type" => assert!(matches!(error, TqLoopbackError::InvalidContentType)),
                "json" => assert!(matches!(error, TqLoopbackError::Json(_))),
                "id" => assert!(matches!(error, TqLoopbackError::CorrelationMismatch { .. })),
                "rpc" => assert!(matches!(error, TqLoopbackError::Rpc { .. })),
                "schema" => assert!(matches!(error, TqLoopbackError::Schema { .. })),
                _ => unreachable!(),
            }
            server.join().unwrap();
        }
    }

    #[test]
    fn limits_and_instrument_validation_have_no_implicit_defaults() {
        assert!(TqLoopbackLimits::new(
            Duration::ZERO,
            Duration::from_millis(1),
            Duration::from_millis(1),
            1,
            1,
        )
        .is_err());
        assert!(TqInstrument::new(SourceExchange::Shanghai, "60039").is_err());
        assert!(TqInstrument::new(SourceExchange::Shanghai, "60039A").is_err());
        let client = test_client("http://127.0.0.1:9/".into(), limits(1024));
        assert!(matches!(
            client.poll_price_volume(0, 1, &instrument(), "2026-08-13T01:02:03Z"),
            Err(TqLoopbackError::InvalidRequest { .. })
        ));
        assert!(matches!(
            client.poll_price_volume(1, 0, &instrument(), "2026-08-13T01:02:03Z"),
            Err(TqLoopbackError::InvalidRequest { .. })
        ));
        assert!(matches!(
            client.poll_market_snapshot(0, 1, &instrument(), "2026-08-13T01:02:03Z"),
            Err(TqLoopbackError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn unavailable_listener_is_a_typed_connect_failure() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/", listener.local_addr().unwrap());
        drop(listener);
        let result = test_client(endpoint, limits(1024)).poll_price_volume(
            7,
            1,
            &instrument(),
            "2026-08-13T01:02:03Z",
        );
        assert!(
            matches!(result, Err(TqLoopbackError::Connect)),
            "{result:?}"
        );
    }
}
