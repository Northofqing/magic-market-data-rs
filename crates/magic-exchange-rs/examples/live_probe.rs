use magic_exchange_rs::{SseClient, SzseClient};
use magic_market_core::{
    Announcements, AssetClass, DataBatch, Exchange, InstrumentDateRangeRequest, InstrumentId,
    PositiveU32,
};
use std::error::Error;
use std::fmt::Debug;

fn main() -> Result<(), Box<dyn Error>> {
    let limit = env_u32("MAGIC_EXCHANGE_LIVE_LIMIT", 3)?;
    if limit == 0 || limit > 20 {
        return Err("MAGIC_EXCHANGE_LIVE_LIMIT must be in 1..=20".into());
    }
    let sse_code = std::env::var("MAGIC_EXCHANGE_SSE_CODE").unwrap_or_else(|_| "600396".into());
    let szse_code = std::env::var("MAGIC_EXCHANGE_SZSE_CODE").unwrap_or_else(|_| "000858".into());
    let sse_request = request(Exchange::Shanghai, sse_code, limit)?;
    let szse_request = request(Exchange::Shenzhen, szse_code, limit)?;
    let sse = SseClient::new()?;
    let szse = SzseClient::new()?;

    println!("provider=sse-official");
    println!("capabilities={:#?}", SseClient::capabilities());
    let sse_batch = sse.announcements(&sse_request)?;
    print_batch("sse_announcements", &sse_batch);

    println!("\nprovider=szse-official");
    println!("capabilities={:#?}", SzseClient::capabilities());
    let szse_batch = szse.announcements(&szse_request)?;
    print_batch("szse_announcements", &szse_batch);

    println!("\nlive_probe_status=passed");
    Ok(())
}

fn request(
    exchange: Exchange,
    code: String,
    limit: u32,
) -> Result<InstrumentDateRangeRequest, Box<dyn Error>> {
    Ok(InstrumentDateRangeRequest::new(
        InstrumentId::new(exchange, code, AssetClass::Equity)?,
        PositiveU32::new(limit)?,
    )?)
}

fn print_batch<T: Debug>(label: &str, batch: &DataBatch<T>) {
    println!("\n=== {label} ===");
    println!("records={}", batch.records().len());
    println!("provenance={:#?}", batch.provenance());
    println!("quality={:#?}", batch.quality());
    for (index, record) in batch.records().iter().enumerate() {
        println!("record[{index}]={record:#?}");
    }
}

fn env_u32(name: &str, default: u32) -> Result<u32, Box<dyn Error>> {
    Ok(std::env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .parse::<u32>()?)
}
