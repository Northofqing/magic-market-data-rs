use std::error::Error;
use std::time::Duration;

use magic_market_composition::{
    production_operation_registry, OutcomeDailyBarsRecord, OutcomeDailyBarsRequest,
    OUTCOME_DAILY_BARS_REQUEST_SCHEMA, SCHEMA_VERSION,
};
use magic_market_core::{AssetClass, Exchange, InstrumentId, IsoDate, PositiveU32};
use magic_market_service::{CanonicalPayload, Operation, QueryCommand};

const MAX_REQUESTS: u32 = 3;
const MAX_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let requests = parse_positive_bounded("MAGIC_OUTCOME_BARS_REQUESTS", 2, MAX_REQUESTS)?;
    let timeout_seconds = parse_positive_bounded("MAGIC_OUTCOME_BARS_TIMEOUT_SECS", 15, 60)?;
    let limit = parse_positive_bounded("MAGIC_OUTCOME_BARS_LIMIT", 20, 800)?;
    let through = IsoDate::new(
        std::env::var("MAGIC_OUTCOME_BARS_THROUGH").unwrap_or_else(|_| "2026-08-14".to_owned()),
    )?;
    let due_at = std::env::var("MAGIC_OUTCOME_BARS_DUE_AT")
        .unwrap_or_else(|_| format!("{}T15:30:00+08:00", through.as_str()));
    let request = OutcomeDailyBarsRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)?,
        through,
        PositiveU32::new(limit)?,
        due_at,
    )?;
    let registry = production_operation_registry(
        Duration::from_secs(u64::from(timeout_seconds)),
        MAX_PAYLOAD_BYTES,
    )?;

    for sequence in 1..=requests {
        let command = QueryCommand::new(
            format!("outcome-daily-bars-live-{sequence}"),
            Operation::OutcomeDailyBars,
            Some("Tdx".to_owned()),
            CanonicalPayload::new(
                OUTCOME_DAILY_BARS_REQUEST_SCHEMA,
                SCHEMA_VERSION,
                serde_json::to_vec(&request)?,
                MAX_PAYLOAD_BYTES,
            )?,
        )?;
        let result = registry.execute(command)?;
        if !result.repository_admitted || !result.complete || result.records.len() != 1 {
            return Err(format!(
                "unexpected OutcomeDailyBars result: admitted={} complete={} records={}",
                result.repository_admitted,
                result.complete,
                result.records.len()
            )
            .into());
        }
        let record: OutcomeDailyBarsRecord = serde_json::from_slice(result.records[0].data())?;
        let first = record
            .bars()
            .first()
            .ok_or("OutcomeDailyBars record is empty")?;
        let last = record
            .bars()
            .last()
            .ok_or("OutcomeDailyBars record is empty")?;
        println!(
            "outcome_daily_bars sequence={sequence} instrument={} through={} due_at={} bars={} first={} last={} evidence={} digest={}",
            record.instrument().code(),
            record.requested_through().as_str(),
            record.outcome_due_at(),
            record.bars().len(),
            first.bar_start(),
            last.bar_end(),
            record.input_evidence().len(),
            record.input_digest_sha256(),
        );
    }
    println!("outcome_daily_bars_live_probe_status=passed requests={requests}");
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
