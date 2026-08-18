use std::error::Error;
use std::time::Duration;

use magic_market_composition::{
    production_operation_registry, T0EvidenceRecord, T0EvidenceRequest, SCHEMA_VERSION,
    T0_EVIDENCE_REQUEST_SCHEMA,
};
use magic_market_core::{AssetClass, Exchange, InstrumentId, PositiveU32};
use magic_market_service::{CanonicalPayload, Operation, QueryCommand};

const MAX_REQUESTS: u32 = 3;
const MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let requests = parse_positive_bounded("MAGIC_T0_EVIDENCE_REQUESTS", 2, MAX_REQUESTS)?;
    let timeout_seconds = parse_positive_bounded("MAGIC_T0_EVIDENCE_TIMEOUT_SECS", 15, 60)?;
    let daily_count = parse_positive_bounded("MAGIC_T0_EVIDENCE_DAILY_BARS", 20, 800)?;
    let five_minute_count = parse_positive_bounded("MAGIC_T0_EVIDENCE_5M_BARS", 20, 800)?;
    let request = T0EvidenceRequest::new(
        vec![InstrumentId::new(
            Exchange::Shanghai,
            "600396",
            AssetClass::Equity,
        )?],
        PositiveU32::new(daily_count)?,
        PositiveU32::new(five_minute_count)?,
    )?;
    let registry = production_operation_registry(
        Duration::from_secs(u64::from(timeout_seconds)),
        MAX_PAYLOAD_BYTES,
    )?;

    for sequence in 1..=requests {
        let command = QueryCommand::new(
            format!("t0-evidence-live-{sequence}"),
            Operation::T0Evidence,
            Some("Tdx".to_owned()),
            CanonicalPayload::new(
                T0_EVIDENCE_REQUEST_SCHEMA,
                SCHEMA_VERSION,
                serde_json::to_vec(&request)?,
                MAX_PAYLOAD_BYTES,
            )?,
        )?;
        let result = registry.execute(command)?;
        if !result.repository_admitted || !result.complete || result.records.len() != 1 {
            return Err(format!(
                "unexpected T0Evidence production result: admitted={} complete={} records={}",
                result.repository_admitted,
                result.complete,
                result.records.len()
            )
            .into());
        }
        let record: T0EvidenceRecord = serde_json::from_slice(result.records[0].data())?;
        println!(
            "t0_evidence sequence={sequence} instrument={} quote_status={:?} book_status={:?} daily_bars={} five_minute_bars={} evidence={} observed_at={} source_at={:?} admitted={} complete={} blocker={:?} digest={}",
            record.instrument().code(),
            record.quote().status(),
            record.order_book().status(),
            record.daily_bars().len(),
            record.five_minute_bars().len(),
            record.input_evidence().len(),
            result.observed_at,
            result.source_at,
            result.repository_admitted,
            result.complete,
            result.diagnostic_blocker,
            record.input_digest_sha256(),
        );
    }
    println!("t0_evidence_live_probe_status=admitted requests={requests}");
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
