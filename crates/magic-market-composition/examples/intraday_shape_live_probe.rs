use std::error::Error;
use std::time::Duration;

use magic_market_composition::{
    production_operation_registry, IntradayShapeRecord, IntradayShapeRequest,
    INTRADAY_SHAPE_REQUEST_SCHEMA, SCHEMA_VERSION,
};
use magic_market_core::{AssetClass, Exchange, InstrumentId, IsoDate, PositiveU32};
use magic_market_service::{CanonicalPayload, Operation, QueryCommand};

const MAX_REQUESTS: u32 = 3;
const MAX_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let requests = parse_positive_bounded("MAGIC_INTRADAY_SHAPE_REQUESTS", 2, MAX_REQUESTS)?;
    let timeout_seconds = parse_positive_bounded("MAGIC_INTRADAY_SHAPE_TIMEOUT_SECS", 15, 60)?;
    let maximum_points = parse_positive_bounded("MAGIC_INTRADAY_SHAPE_MAX_POINTS", 800, 800)?;
    let trading_date = IsoDate::new(
        std::env::var("MAGIC_INTRADAY_SHAPE_DATE").unwrap_or_else(|_| "2026-08-14".to_owned()),
    )?;
    let request = IntradayShapeRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)?,
        Some(trading_date),
        PositiveU32::new(maximum_points)?,
    )?;
    let registry = production_operation_registry(
        Duration::from_secs(u64::from(timeout_seconds)),
        MAX_PAYLOAD_BYTES,
    )?;

    for sequence in 1..=requests {
        let command = QueryCommand::new(
            format!("intraday-shape-live-{sequence}"),
            Operation::IntradayShape,
            Some("LocalAnalysis".to_owned()),
            CanonicalPayload::new(
                INTRADAY_SHAPE_REQUEST_SCHEMA,
                SCHEMA_VERSION,
                serde_json::to_vec(&request)?,
                MAX_PAYLOAD_BYTES,
            )?,
        )?;
        let result = registry.execute(command)?;
        if !result.repository_admitted || !result.complete || result.records.len() != 1 {
            return Err(format!(
                "unexpected IntradayShape result: admitted={} complete={} records={}",
                result.repository_admitted,
                result.complete,
                result.records.len()
            )
            .into());
        }
        let record: IntradayShapeRecord = serde_json::from_slice(result.records[0].data())?;
        println!(
            "intraday_shape sequence={sequence} instrument={} date={} points={} first={} last={} open={} high={} low={} latest={} vwap={:?} volume_lots={:?} amount_cny={:?} up={} down={} flat={} digest={}",
            record.instrument().code(),
            record.trading_date().as_str(),
            record.point_count().get(),
            record.first_at(),
            record.last_at(),
            record.open().get(),
            record.high().get(),
            record.low().get(),
            record.latest().get(),
            record.vwap().map(|value| value.get()),
            record.cumulative_volume().map(|value| value.get()),
            record.cumulative_amount().map(|value| value.get()),
            record.up_points(),
            record.down_points(),
            record.flat_points(),
            record.input_digest_sha256(),
        );
    }
    println!("intraday_shape_live_probe_status=passed requests={requests}");
    Ok(())
}

fn parse_positive_bounded(name: &str, default: u32, maximum: u32) -> Result<u32, Box<dyn Error>> {
    let value = std::env::var(name)
        .ok()
        .map(|value| value.parse::<u32>())
        .transpose()?
        .unwrap_or(default);
    if value == 0 || value > maximum {
        return Err(format!("{name} must be in 1..={maximum}").into());
    }
    Ok(value)
}
