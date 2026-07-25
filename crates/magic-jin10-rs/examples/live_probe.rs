use magic_jin10_rs::Jin10Client;
use magic_market_core::{
    EconomicCalendarProvider, EconomicCalendarRequest, NewsProvider, PositiveU32,
};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let client = Jin10Client::new()?;
    let capabilities = Jin10Client::content_capabilities();
    println!("provider=jin10-flash-v1 capabilities={capabilities:?}");
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
        let topics = item
            .topics
            .iter()
            .map(|topic| topic.as_str())
            .collect::<Vec<_>>();
        println!(
            "item_id={} title={:?} content={:?} publisher={} canonical_url={} published_at={} instruments={} topics={topics:?} language={} evidence_provider={:?} evidence_source_at={:?} evidence_observed_at={} evidence_batch_id={}",
            item.item_id,
            item.title.as_str(),
            item.content.as_ref().map(|value| value.as_str()),
            item.publisher,
            item.canonical_url,
            item.published_at,
            item.instruments.len(),
            item.language,
            item.evidence.provider(),
            item.evidence.source_at(),
            item.evidence.observed_at(),
            item.evidence.batch_id()
        );
    }
    if std::env::var("MAGIC_JIN10_LIVE_INCLUDE_CALENDAR").as_deref() == Ok("1") {
        let calendar_request = EconomicCalendarRequest::new(PositiveU32::new(10)?)?;
        let calendar = client.economic_calendar(&calendar_request)?;
        println!(
            "economic_calendar source={} source_at={:?} fetched_at={} batch_id={:?} complete={} records={}",
            calendar.provenance().source(),
            calendar.provenance().source_at(),
            calendar.provenance().fetched_at(),
            calendar.provenance().batch_id(),
            calendar.quality().is_complete(),
            calendar.records().len()
        );
        for event in calendar.records() {
            println!(
                "event_id={} country={} name={} period={:?} scheduled_at={} released_at={} previous={:?} consensus={:?} actual={:?} unit={:?} importance={} impact={:?}",
                event.event_id,
                event.country,
                event.name,
                event.period.as_ref().map(|value| value.as_str()),
                event.scheduled_at,
                event.released_at,
                event.previous.as_ref().map(|value| value.as_str()),
                event.consensus.as_ref().map(|value| value.as_str()),
                event.actual.as_ref().map(|value| value.as_str()),
                event.unit.as_ref().map(|value| value.as_str()),
                event.importance.get(),
                event.impact.as_ref().map(|value| value.as_str())
            );
        }
    }
    println!("live_probe_status=passed");
    Ok(())
}
