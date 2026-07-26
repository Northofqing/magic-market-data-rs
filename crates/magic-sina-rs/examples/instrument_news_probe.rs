use magic_market_core::{
    AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, NewsProvider, PositiveU32,
    ProviderId,
};
use magic_sina_rs::SinaClient;
use std::error::Error;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    let timeout = std::env::var("MAGIC_SINA_TIMEOUT_SECS")
        .ok()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(10);
    let client = SinaClient::with_timeout(Duration::from_secs(timeout))?;
    let instruments = [
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)?,
        InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity)?,
    ];

    for instrument in instruments {
        let request = InstrumentDateRangeRequest::new(instrument.clone(), PositiveU32::new(3)?)?;
        let batch = client.instrument_news(&request)?;
        if batch.records().is_empty() {
            return Err(format!(
                "{}.{:?} returned an ordinary empty live batch",
                instrument.code(),
                instrument.exchange()
            )
            .into());
        }
        if !batch.quality().is_complete() || !batch.quality().issues().is_empty() {
            return Err(format!(
                "{}.{:?} returned incomplete news quality",
                instrument.code(),
                instrument.exchange()
            )
            .into());
        }
        let source_at = batch
            .provenance()
            .source_at()
            .ok_or("instrument-news provenance source_at is missing")?;
        let batch_id = batch
            .provenance()
            .batch_id()
            .ok_or("instrument-news provenance batch_id is missing")?;
        println!(
            "instrument={}.{:?} records={} source={} source_at={} observed_at={} batch_id={}",
            instrument.code(),
            instrument.exchange(),
            batch.records().len(),
            batch.provenance().source(),
            source_at,
            batch.provenance().fetched_at(),
            batch_id
        );
        for item in batch.records() {
            if item.instruments != vec![instrument.clone()]
                || item.evidence.provider() != ProviderId::Sina
                || item.evidence.batch_id() != batch_id
                || item.evidence.source_at() != Some(item.published_at.as_str())
            {
                return Err("instrument-news record evidence mismatch".into());
            }
            println!(
                "  published_at={} canonical_url={}",
                item.published_at.as_str(),
                item.canonical_url.as_str()
            );
        }
    }

    println!("instrument_news_live_probe_status=passed");
    Ok(())
}
