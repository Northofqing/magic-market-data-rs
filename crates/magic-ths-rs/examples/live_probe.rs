use magic_market_core::{
    AssetClass, ConsensusData, DataBatch, Exchange, InstrumentId, InstrumentSignalRequest, IsoDate,
    LimitPoolKind, LimitPoolRequest, LimitPools, PopularityData, PositiveU32, StrongStockReasons,
};
use magic_ths_rs::ThsClient;
use std::error::Error;
use std::fmt::Debug;

fn main() -> Result<(), Box<dyn Error>> {
    let client = ThsClient::new()?;
    let consensus_instrument =
        equity(std::env::var("MAGIC_THS_CONSENSUS_CODE").unwrap_or_else(|_| "600519".into()))?;
    let strong_instrument =
        equity(std::env::var("MAGIC_THS_STRONG_CODE").unwrap_or_else(|_| "000815".into()))?;
    let trading_date = IsoDate::new(
        std::env::var("MAGIC_THS_TRADING_DATE").unwrap_or_else(|_| "2026-07-22".into()),
    )?;
    let small = PositiveU32::new(3)?;

    println!("provider=tonghuashun");
    println!("capabilities={:#?}", ThsClient::capabilities());

    print_batch("consensus", &client.consensus(&[consensus_instrument])?);
    let strong_request = InstrumentSignalRequest::new(strong_instrument, small)?
        .with_trading_date(trading_date.clone());
    print_batch(
        "strong_stock_reasons",
        &client.strong_stock_reasons(&strong_request)?,
    );
    let pool_request = LimitPoolRequest::new(LimitPoolKind::Upper, trading_date, small)?;
    print_batch("upper_limit_pool", &client.limit_pool(&pool_request)?);
    print_batch("popularity", &client.popularity(small)?);
    Ok(())
}

fn equity(code: String) -> Result<InstrumentId, Box<dyn Error>> {
    let exchange = match code.as_bytes().first().copied() {
        Some(b'6') => Exchange::Shanghai,
        Some(b'0') | Some(b'3') => Exchange::Shenzhen,
        Some(b'4') | Some(b'8') => Exchange::Beijing,
        Some(b'9') if code.starts_with("920") => Exchange::Beijing,
        _ => return Err(format!("unsupported or unverified A-share code family: {code}").into()),
    };
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
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
