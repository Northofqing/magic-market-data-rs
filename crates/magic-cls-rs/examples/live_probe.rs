use magic_cls_rs::ClsClient;
use magic_market_core::{NewsProvider, PositiveU32};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let client = ClsClient::new()?;
    let capabilities = ClsClient::content_capabilities();
    println!("provider=cls-v1 capabilities={capabilities:?}");
    let batch = client.global_news(PositiveU32::new(5)?)?;
    println!(
        "source={} source_at={:?} fetched_at={} batch_id={:?} complete={} records={}",
        batch.provenance().source(),
        batch.provenance().source_at(),
        batch.provenance().fetched_at(),
        batch.provenance().batch_id(),
        batch.quality().is_complete(),
        batch.records().len()
    );
    for item in batch.records() {
        let instruments = item
            .instruments
            .iter()
            .map(|instrument| format!("{:?}:{}", instrument.exchange(), instrument.code()))
            .collect::<Vec<_>>();
        let topics = item
            .topics
            .iter()
            .map(|topic| topic.as_str())
            .collect::<Vec<_>>();
        println!(
            "item_id={} title={:?} summary={:?} content={:?} publisher={} canonical_url={} published_at={} instruments={instruments:?} topics={topics:?} language={} evidence_provider={:?} evidence_source_at={:?} evidence_observed_at={} evidence_batch_id={}",
            item.item_id,
            item.title.as_str(),
            item.summary.as_ref().map(|value| value.as_str()),
            item.content.as_ref().map(|value| value.as_str()),
            item.publisher,
            item.canonical_url,
            item.published_at,
            item.language,
            item.evidence.provider(),
            item.evidence.source_at(),
            item.evidence.observed_at(),
            item.evidence.batch_id()
        );
    }
    println!("live_probe_status=passed");
    Ok(())
}
