use magic_jin10_rs::{Jin10Client, Jin10Error};
use magic_market_core::{
    verify_admitted_newest_first_batch, verify_verified_empty, EconomicCalendarProvider,
    EconomicCalendarRequest, EconomicReleaseObservationsProvider,
    EconomicReleaseObservationsRequest, NewsProvider, PositiveU32, ProbeAdmissionPolicy,
    ProviderId,
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
    if std::env::var("MAGIC_JIN10_LIVE_INCLUDE_RELEASE_OBSERVATIONS").as_deref() == Ok("1") {
        let request = EconomicReleaseObservationsRequest::new(PositiveU32::new(20)?)?;
        match client.economic_release_observations(&request) {
            Ok(observations) => {
                let status = verify_admitted_newest_first_batch(
                    &observations,
                    &ProbeAdmissionPolicy::new(ProviderId::Jin10).require_source_at(),
                    |event| &event.evidence,
                    |event| event.released_at.as_str(),
                    |event| event.event_id.as_str().to_owned(),
                )?;
                println!(
                    "economic_release_observations status={} source={} source_at={:?} fetched_at={} batch_id={:?} complete={} records={}",
                    status,
                    observations.provenance().source(),
                    observations.provenance().source_at(),
                    observations.provenance().fetched_at(),
                    observations.provenance().batch_id(),
                    observations.quality().is_complete(),
                    observations.records().len()
                );
                for event in observations.records() {
                    println!(
                        "event_id={} country={} name={} period={:?} scheduled_at={} released_at={} previous={:?} consensus={:?} actual={:?} unit={:?} importance={} impact={:?} evidence_source_at={:?} evidence_observed_at={} evidence_batch_id={}",
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
                        event.impact.as_ref().map(|value| value.as_str()),
                        event.evidence.source_at(),
                        event.evidence.observed_at(),
                        event.evidence.batch_id(),
                    );
                }
            }
            Err(Jin10Error::VerifiedEmpty(empty)) => {
                let status =
                    verify_verified_empty(&empty, &ProbeAdmissionPolicy::new(ProviderId::Jin10))?;
                println!(
                    "economic_release_observations status={} source={} source_at={:?} fetched_at={} batch_id={:?} complete=true records=0 request_identity={} reason={}",
                    status,
                    empty.provenance().source(),
                    empty.provenance().source_at(),
                    empty.provenance().fetched_at(),
                    empty.provenance().batch_id(),
                    empty.request_identity(),
                    empty.reason(),
                );
            }
            Err(error) => return Err(error.into()),
        }
    }
    println!("live_probe_status=passed");
    Ok(())
}
