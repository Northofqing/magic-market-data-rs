use magic_market_core::{
    verify_admitted_newest_first_batch, PositiveU32, ProbeAdmissionPolicy, ProbeStatus, ProviderId,
};
use magic_xinhua_rs::{XinhuaClient, GLOBAL_NEWS_ADMITTED};
use std::error::Error;
use std::time::Duration;

const MAX_SOURCE_AGE: Duration = Duration::from_secs(72 * 60 * 60);

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
    let client = XinhuaClient::new()?;
    let capabilities = XinhuaClient::content_capabilities();
    println!("provider=xinhua-finance capabilities={capabilities:?}");
    let batch = client.probe_global_news(PositiveU32::new(13)?)?;
    let verified = verify_admitted_newest_first_batch(
        &batch,
        &ProbeAdmissionPolicy::new(ProviderId::XinhuaFinance)
            .with_max_source_age(MAX_SOURCE_AGE)?,
        |item| &item.evidence,
        |item| item.published_at.as_str(),
        |item| item.item_id.as_str().to_owned(),
    )?;
    if batch
        .records()
        .iter()
        .any(|item| item.summary.is_some() || item.content.is_some())
    {
        return Err("Xinhua Finance metadata probe exposed summary or content".into());
    }
    println!(
        "source={} source_at={:?} fetched_at={} batch_id={:?} complete={} records={} verification_status={verified}",
        batch.provenance().source(),
        batch.provenance().source_at(),
        batch.provenance().fetched_at(),
        batch.provenance().batch_id(),
        batch.quality().is_complete(),
        batch.records().len()
    );
    for item in batch.records() {
        println!(
            "provider=XinhuaFinance id={} title={} publisher={} url={} published_at={} source_at={:?} observed_at={} batch_id={}",
            item.item_id,
            item.title,
            item.publisher,
            item.canonical_url,
            item.published_at,
            item.evidence.source_at(),
            item.evidence.observed_at(),
            item.evidence.batch_id()
        );
    }
    Ok(declared_admission_status(GLOBAL_NEWS_ADMITTED, verified))
}

fn declared_admission_status(advertised: bool, verified: ProbeStatus) -> ProbeStatus {
    if advertised {
        verified
    } else {
        ProbeStatus::DiagnosticCompleteUnadmitted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_does_not_promote_an_unadvertised_capability() {
        assert_eq!(
            declared_admission_status(false, ProbeStatus::Admitted),
            ProbeStatus::DiagnosticCompleteUnadmitted
        );
        assert_eq!(
            declared_admission_status(true, ProbeStatus::Admitted),
            ProbeStatus::Admitted
        );
    }
}
