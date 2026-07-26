use magic_eastmoney_rs::EastmoneyClient;
use magic_market_core::{
    verify_admitted_batch, Exchange, IsoDate, MarketDragonTigerData, MarketDragonTigerRequest,
    PositiveU32, ProbeAdmissionPolicy, ProviderId,
};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let trading_date = IsoDate::new(required_env("MAGIC_EASTMONEY_DRAGON_TIGER_DATE")?)?;
    let limit = std::env::var("MAGIC_EASTMONEY_DRAGON_TIGER_LIMIT")
        .unwrap_or_else(|_| "5".to_owned())
        .parse::<u32>()?;
    let request = MarketDragonTigerRequest::new(trading_date, PositiveU32::new(limit)?)?;
    let batch = EastmoneyClient::new()?.market_dragon_tiger(&request)?;
    let status = verify_admitted_batch(
        &batch,
        &ProbeAdmissionPolicy::new(ProviderId::Eastmoney).require_source_at(),
        |record| record.entry().evidence(),
        |record| record.entry().entry_id().as_str().to_owned(),
    )?;

    println!("family=dragon_tiger.market status={status}");
    println!("provider={:?}", ProviderId::Eastmoney);
    println!("source={}", batch.provenance().source());
    println!(
        "source_at={}",
        batch.provenance().source_at().unwrap_or("<missing>")
    );
    println!(
        "batch_id={}",
        batch.provenance().batch_id().unwrap_or("<missing>")
    );
    println!("records={}", batch.records().len());
    for (index, disclosure) in batch.records().iter().enumerate() {
        let entry = disclosure.entry();
        let exchange = match entry.instrument().exchange() {
            Exchange::Shanghai => "SH",
            Exchange::Shenzhen => "SZ",
            Exchange::Beijing => "BJ",
        };
        println!(
            "record[{index}]={}.{} entry_id={} seats={} net_amount={:?}",
            entry.instrument().code(),
            exchange,
            entry.entry_id(),
            disclosure.seats().len(),
            entry.net_amount().map(|amount| amount.get())
        );
    }
    Ok(())
}

fn required_env(name: &str) -> Result<String, std::io::Error> {
    let value = std::env::var(name).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{name} is required and must identify the source trading session"),
        )
    })?;
    if value.trim().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{name} must not be empty"),
        ));
    }
    Ok(value)
}
