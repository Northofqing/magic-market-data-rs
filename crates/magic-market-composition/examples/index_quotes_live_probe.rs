use std::error::Error;
use std::time::Duration;

use magic_market_composition::{
    production_operation_registry, INDEX_QUOTES_REQUEST_SCHEMA, SCHEMA_VERSION,
};
use magic_market_core::Quote;
use magic_market_service::{CanonicalPayload, Operation, QueryCommand};

const MAX_REQUESTS: u32 = 3;
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let requests = parse_positive_bounded("MAGIC_INDEX_QUOTES_PROBE_REQUESTS", 2, MAX_REQUESTS)?;
    let timeout_seconds = parse_positive_bounded("MAGIC_INDEX_QUOTES_TIMEOUT_SECS", 10, 60)?;
    let maximum_source_age_millis = std::env::var("MAGIC_INDEX_QUOTES_MAX_SOURCE_AGE_MILLIS")
        .ok()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(259_200_000);
    if maximum_source_age_millis == 0 {
        return Err("MAGIC_INDEX_QUOTES_MAX_SOURCE_AGE_MILLIS must be positive".into());
    }

    let registry = production_operation_registry(
        Duration::from_secs(u64::from(timeout_seconds)),
        MAX_PAYLOAD_BYTES,
    )?;
    let indices = serde_json::json!([
        {"exchange":"Shanghai","code":"000001","asset_class":"Index"},
        {"exchange":"Shenzhen","code":"399001","asset_class":"Index"},
        {"exchange":"Shenzhen","code":"399006","asset_class":"Index"},
        {"exchange":"Shanghai","code":"000300","asset_class":"Index"},
        {"exchange":"Shanghai","code":"000905","asset_class":"Index"},
        {"exchange":"Shanghai","code":"000852","asset_class":"Index"}
    ]);

    for sequence in 1..=requests {
        let data = serde_json::to_vec(&serde_json::json!({
            "indices": indices.clone(),
            "maximum_source_age_millis": maximum_source_age_millis
        }))?;
        let command = QueryCommand::new(
            format!("index-quotes-live-{sequence}"),
            Operation::IndexQuotes,
            Some("Tencent".to_owned()),
            CanonicalPayload::new(
                INDEX_QUOTES_REQUEST_SCHEMA,
                SCHEMA_VERSION,
                data,
                MAX_PAYLOAD_BYTES,
            )?,
        )?;
        let result = registry.execute(command)?;
        if !result.repository_admitted || !result.complete || result.records.len() != 6 {
            return Err(format!(
                "unexpected IndexQuotes result: admitted={} complete={} records={}",
                result.repository_admitted,
                result.complete,
                result.records.len()
            )
            .into());
        }
        println!(
            "index_quotes_probe sequence={sequence} provider={} records={} observed_at={} source_at={:?}",
            result.provider,
            result.records.len(),
            result.observed_at,
            result.source_at
        );
        for record in &result.records {
            let quote: Quote = serde_json::from_slice(record.data())?;
            println!(
                "index code={} exchange={:?} price={} previous_close={:?} volume_lots={} amount_yuan={:?} source_at={:?} status={:?}",
                quote.instrument().code(),
                quote.instrument().exchange(),
                quote.price().get(),
                quote.previous_close().map(|value| value.get()),
                quote.volume().get(),
                quote.amount().map(|value| value.get()),
                quote.source_at(),
                quote.status()
            );
        }
    }
    println!("index_quotes_live_probe_status=passed requests={requests}");
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
