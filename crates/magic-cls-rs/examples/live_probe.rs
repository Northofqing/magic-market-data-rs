use magic_cls_rs::ClsClient;
use magic_market_core::{
    verify_admitted_batch, NewsProvider, PositiveU32, ProbeAdmissionPolicy, ProbeStatus, ProviderId,
};
use std::error::Error;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    match run_probe() {
        Ok(status) => {
            println!("admitted={}", status.satisfies_capability());
            println!("live_probe_status={status}");
            Ok(())
        }
        Err(error) => {
            println!("live_probe_status={}", ProbeStatus::Failed);
            Err(error)
        }
    }
}

fn run_probe() -> Result<ProbeStatus, Box<dyn Error>> {
    let client = ClsClient::new()?;
    let capabilities = ClsClient::content_capabilities();
    println!("provider=cls-v1 capabilities={capabilities:?}");
    let batch = client.global_news(PositiveU32::new(1)?)?;
    let verified = verify_admitted_batch(
        &batch,
        &ProbeAdmissionPolicy::new(ProviderId::Cailianpress)
            .require_source_at()
            .with_max_source_age(Duration::from_secs(24 * 60 * 60))?,
        |record| &record.evidence,
        |record| record.item_id.as_str().to_owned(),
    )?;
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
    Ok(declared_admission_status(
        capabilities.global_news,
        verified,
    ))
}

fn declared_admission_status(
    capability_advertised: bool,
    verified_status: ProbeStatus,
) -> ProbeStatus {
    if capability_advertised {
        verified_status
    } else {
        ProbeStatus::DiagnosticCompleteUnadmitted
    }
}

#[cfg(test)]
#[path = "../tests/unit/live_probe_tests.rs"]
mod tests;
