use std::error::Error;
use std::time::Duration;

use magic_market_composition::{
    production_operation_registry, UpperLimitPoolReviewRecord, UpperLimitPoolReviewRequest,
    SCHEMA_VERSION, UPPER_LIMIT_POOL_REVIEW_REQUEST_SCHEMA,
};
use magic_market_core::{IsoDate, PositiveU32};
use magic_market_service::{CanonicalPayload, Operation, QueryCommand};

const MAX_REQUESTS: u32 = 3;
const MAX_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let requests = parse_positive_bounded("MAGIC_UPPER_LIMIT_REVIEW_REQUESTS", 2, MAX_REQUESTS)?;
    let timeout_seconds = parse_positive_bounded("MAGIC_UPPER_LIMIT_REVIEW_TIMEOUT_SECS", 15, 60)?;
    let per_pool_limit = parse_positive_bounded("MAGIC_UPPER_LIMIT_REVIEW_LIMIT", 1_000, 1_000)?;
    let trading_date = IsoDate::new(
        std::env::var("MAGIC_UPPER_LIMIT_REVIEW_DATE").unwrap_or_else(|_| "2026-08-14".to_owned()),
    )?;
    let request =
        UpperLimitPoolReviewRequest::new(trading_date, PositiveU32::new(per_pool_limit)?)?;
    let registry = production_operation_registry(
        Duration::from_secs(u64::from(timeout_seconds)),
        MAX_PAYLOAD_BYTES,
    )?;

    for sequence in 1..=requests {
        let command = QueryCommand::new(
            format!("upper-limit-pool-review-live-{sequence}"),
            Operation::UpperLimitPoolReview,
            Some("Eastmoney".to_owned()),
            CanonicalPayload::new(
                UPPER_LIMIT_POOL_REVIEW_REQUEST_SCHEMA,
                SCHEMA_VERSION,
                serde_json::to_vec(&request)?,
                MAX_PAYLOAD_BYTES,
            )?,
        )?;
        let result = registry.execute(command)?;
        if !result.repository_admitted || !result.complete || result.records.len() != 1 {
            return Err(format!(
                "unexpected UpperLimitPoolReview result: admitted={} complete={} records={}",
                result.repository_admitted,
                result.complete,
                result.records.len()
            )
            .into());
        }
        let record: UpperLimitPoolReviewRecord = serde_json::from_slice(result.records[0].data())?;
        println!(
            "upper_limit_pool_review sequence={sequence} date={} upper={} broken={} lower={} previous_upper={} maximum_streak={:?} evidence={} digest={}",
            record.trading_date().as_str(),
            record.upper().len(),
            record.broken().len(),
            record.lower().len(),
            record.previous_upper().len(),
            record.maximum_streak(),
            record.input_evidence().len(),
            record.input_digest_sha256(),
        );
    }
    println!("upper_limit_pool_review_live_probe_status=passed requests={requests}");
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
