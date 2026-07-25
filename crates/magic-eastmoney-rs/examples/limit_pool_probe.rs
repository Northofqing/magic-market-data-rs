use magic_eastmoney_rs::EastmoneyClient;
use magic_market_core::{IsoDate, LimitPoolKind, LimitPoolRequest, LimitPools, PositiveU32};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let trading_date = args.next().ok_or("usage: limit_pool_probe YYYY-MM-DD")?;
    if args.next().is_some() {
        return Err("usage: limit_pool_probe YYYY-MM-DD".into());
    }
    let request = LimitPoolRequest::new(
        LimitPoolKind::Upper,
        IsoDate::new(trading_date)?,
        PositiveU32::new(1_000)?,
    )?;
    let batch = EastmoneyClient::new()?.limit_pool(&request)?;
    println!(
        "state={} source={} source_at={} observed_at={} batch_id={} records={} issues={}",
        if batch.quality().is_complete() {
            if batch.records().is_empty() {
                "verified_empty"
            } else {
                "available"
            }
        } else {
            "incomplete"
        },
        batch.provenance().source(),
        batch.provenance().source_at().unwrap_or("absent"),
        batch.provenance().fetched_at(),
        batch.provenance().batch_id().unwrap_or("absent"),
        batch.records().len(),
        batch.quality().issues().join("; "),
    );
    if !batch.quality().is_complete() {
        return Err("BR-019 whole-market limit pool is incomplete".into());
    }
    Ok(())
}
