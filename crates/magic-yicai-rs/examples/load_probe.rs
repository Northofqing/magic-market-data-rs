use magic_market_core::{
    verify_admitted_batch, verify_serial_load, PositiveU32, ProbeAdmissionPolicy, ProbeStatus,
    ProviderId,
};
use magic_yicai_rs::{YicaiClient, GLOBAL_NEWS_ADMITTED};
use std::error::Error;
use std::time::Duration;

const REQUESTS: u32 = 3;
const MIN_INTERVAL: Duration = Duration::from_secs(1);
const MAX_SOURCE_AGE: Duration = Duration::from_secs(72 * 60 * 60);

fn main() -> Result<(), Box<dyn Error>> {
    match run_probe() {
        Ok(status) => {
            println!("load_probe_status={status}");
            Ok(())
        }
        Err(error) => {
            println!("load_probe_status={}", ProbeStatus::Failed);
            Err(error)
        }
    }
}

fn run_probe() -> Result<ProbeStatus, Box<dyn Error>> {
    let client = YicaiClient::new()?;
    let policy =
        ProbeAdmissionPolicy::new(ProviderId::Yicai).with_max_source_age(MAX_SOURCE_AGE)?;
    let mut records = 0;
    for call in 1..=REQUESTS {
        let batch = client.probe_global_news(PositiveU32::new(50)?)?;
        verify_admitted_batch(
            &batch,
            &policy,
            |item| &item.evidence,
            |item| item.item_id.as_str().to_owned(),
        )?;
        records += batch.records().len();
        println!("call={call} records={}", batch.records().len());
    }
    let snapshot = client.load_probe_snapshot()?;
    let status = verify_serial_load(&snapshot, MIN_INTERVAL)?;
    println!(
        "provider=yicai requests={} records={} actual_request_starts={} actual_minimum_start_gap_ms={} observed_maximum_concurrency={}",
        REQUESTS,
        records,
        snapshot.request_starts(),
        snapshot
            .minimum_start_gap()
            .ok_or("multiple Yicai requests did not record a start gap")?
            .as_millis(),
        snapshot.maximum_concurrency()
    );
    Ok(declared_admission_status(GLOBAL_NEWS_ADMITTED, status))
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
    fn probe_is_hard_bounded_and_requires_production_pacing() {
        assert_eq!(REQUESTS, 3);
        assert_eq!(MIN_INTERVAL, Duration::from_secs(1));
        assert_eq!(MAX_SOURCE_AGE, Duration::from_secs(72 * 60 * 60));
    }

    #[test]
    fn load_verification_does_not_promote_an_unadvertised_capability() {
        assert_eq!(
            declared_admission_status(false, ProbeStatus::Admitted),
            ProbeStatus::DiagnosticCompleteUnadmitted
        );
    }
}
