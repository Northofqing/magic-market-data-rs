#![forbid(unsafe_code)]

use magic_tdx_local_rs::{
    SourceExchange, TqInstrument, TqLoopbackClient, TqLoopbackError, TqLoopbackLimits,
};
use serde::Serialize;
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Serialize)]
struct DiagnosticReport {
    mode: &'static str,
    status: &'static str,
    latency_micros: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cumulative_volume: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason_code: Option<&'static str>,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let parsed = parse_arguments(&arguments);
    let (instrument, limits, request_id, bridge_sequence, observed_at_utc) = match parsed {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    let started = Instant::now();
    let result = TqLoopbackClient::new(limits).poll_price_volume(
        request_id,
        bridge_sequence,
        &instrument,
        observed_at_utc,
    );
    let latency_micros = started.elapsed().as_micros();
    let (report, exit) = match result {
        Ok(observation) => (
            DiagnosticReport {
                mode: "diagnostic",
                status: "observed",
                latency_micros,
                price: observation.price.map(|value| value.value),
                cumulative_volume: observation.cumulative_volume.map(|value| value.value),
                reason_code: None,
            },
            ExitCode::SUCCESS,
        ),
        Err(error) => (
            DiagnosticReport {
                mode: "diagnostic",
                status: "unavailable",
                latency_micros,
                price: None,
                cumulative_volume: None,
                reason_code: Some(reason_code(&error)),
            },
            ExitCode::from(3),
        ),
    };
    match serde_json::to_string(&report) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("unable to encode diagnostic report: {error}");
            return ExitCode::from(1);
        }
    }
    exit
}

type ParsedArguments = (TqInstrument, TqLoopbackLimits, u64, u64, String);

fn parse_arguments(arguments: &[String]) -> Result<ParsedArguments, &'static str> {
    let [exchange, code, connect_ms, read_ms, write_ms, request_bytes, response_bytes, request_id, bridge_sequence, observed_at_utc] =
        arguments
    else {
        return Err(
            "usage: tq_loopback_probe <SH|SZ|BJ> <six-digit-code> <connect-ms> <read-ms> <write-ms> <request-bytes> <response-bytes> <request-id> <bridge-sequence> <observed-at-utc>",
        );
    };
    let exchange = match exchange.as_str() {
        "SH" => SourceExchange::Shanghai,
        "SZ" => SourceExchange::Shenzhen,
        "BJ" => SourceExchange::Beijing,
        _ => return Err("exchange must be exactly SH, SZ or BJ"),
    };
    let instrument = TqInstrument::new(exchange, code.clone())
        .map_err(|_| "instrument code must be exactly six ASCII digits")?;
    let limits = TqLoopbackLimits::new(
        parse_positive_millis(connect_ms)?,
        parse_positive_millis(read_ms)?,
        parse_positive_millis(write_ms)?,
        parse_positive_usize(request_bytes)?,
        parse_positive_usize(response_bytes)?,
    )
    .map_err(|_| "all timeout and body limits must be positive and bounded")?;
    let request_id = parse_positive_u64(request_id)?;
    let bridge_sequence = parse_positive_u64(bridge_sequence)?;
    if observed_at_utc.is_empty()
        || observed_at_utc.trim() != observed_at_utc
        || observed_at_utc.chars().any(char::is_control)
    {
        return Err("observed-at-utc must be unpadded non-control text");
    }
    Ok((
        instrument,
        limits,
        request_id,
        bridge_sequence,
        observed_at_utc.clone(),
    ))
}

fn parse_positive_millis(value: &str) -> Result<Duration, &'static str> {
    Ok(Duration::from_millis(parse_positive_u64(value)?))
}

fn parse_positive_u64(value: &str) -> Result<u64, &'static str> {
    let value = value
        .parse::<u64>()
        .map_err(|_| "numeric arguments must be unsigned decimal integers")?;
    if value == 0 {
        return Err("numeric arguments must be greater than zero");
    }
    Ok(value)
}

fn parse_positive_usize(value: &str) -> Result<usize, &'static str> {
    let value = value
        .parse::<usize>()
        .map_err(|_| "body limits must be positive decimal integers")?;
    if value == 0 || value == usize::MAX {
        return Err("body limits must be positive and bounded");
    }
    Ok(value)
}

fn reason_code(error: &TqLoopbackError) -> &'static str {
    match error {
        TqLoopbackError::InvalidLimits { .. } => "invalid_limits",
        TqLoopbackError::InvalidRequest { .. } => "invalid_request",
        TqLoopbackError::EncodeJson(_) => "request_encoding",
        TqLoopbackError::RequestTooLarge { .. } => "request_too_large",
        TqLoopbackError::Connect => "connect",
        TqLoopbackError::Timeout => "timeout",
        TqLoopbackError::Transport { .. } => "transport",
        TqLoopbackError::HttpStatus { .. } => "http_status",
        TqLoopbackError::MissingContentType => "missing_content_type",
        TqLoopbackError::InvalidContentType => "invalid_content_type",
        TqLoopbackError::ResponseTooLarge { .. } => "response_too_large",
        TqLoopbackError::Read => "response_read",
        TqLoopbackError::Json(_) => "invalid_json",
        TqLoopbackError::Rpc { .. } => "rpc_error",
        TqLoopbackError::CorrelationMismatch { .. } => "correlation_mismatch",
        TqLoopbackError::InstrumentIdentity { .. } => "instrument_identity",
        TqLoopbackError::Schema { .. } => "schema_mismatch",
        TqLoopbackError::Synchronization => "synchronization",
    }
}
