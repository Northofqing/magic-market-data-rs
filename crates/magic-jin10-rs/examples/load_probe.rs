use magic_jin10_rs::{Jin10Client, Jin10Error};
use magic_market_core::{
    verify_admitted_newest_first_batch, verify_verified_empty, EconomicReleaseObservationsProvider,
    EconomicReleaseObservationsRequest, NewsProvider, PositiveU32, ProbeAdmissionPolicy,
    ProviderId,
};
use std::error::Error;
use std::time::{Duration, Instant};

const MAX_REQUESTS: usize = 3;
const MIN_INTERVAL: Duration = Duration::from_secs(1);

fn validate_load(requests: usize) -> Result<(), String> {
    if requests == 0 || requests > MAX_REQUESTS {
        return Err(format!(
            "MAGIC_JIN10_LOAD_REQUESTS must be between 1 and {MAX_REQUESTS}"
        ));
    }
    Ok(())
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    let rank = (sorted.len() * percentile).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn main() -> Result<(), Box<dyn Error>> {
    let requests = std::env::var("MAGIC_JIN10_LOAD_REQUESTS")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(2);
    validate_load(requests)?;
    let release_observations =
        std::env::var("MAGIC_JIN10_LOAD_RELEASE_OBSERVATIONS").as_deref() == Ok("1");
    let client = Jin10Client::new()?;
    let started = Instant::now();
    let mut latencies = Vec::with_capacity(requests);
    let mut records = 0_usize;
    let mut successes = 0_usize;
    let mut errors = Vec::new();
    for index in 0..requests {
        let request_started = Instant::now();
        let result = if release_observations {
            let request = EconomicReleaseObservationsRequest::new(PositiveU32::new(20)?)?;
            match client.economic_release_observations(&request) {
                Ok(batch) => verify_admitted_newest_first_batch(
                    &batch,
                    &ProbeAdmissionPolicy::new(ProviderId::Jin10).require_source_at(),
                    |event| &event.evidence,
                    |event| event.released_at.as_str(),
                    |event| event.event_id.as_str().to_owned(),
                )
                .map(|_| batch.records().len())
                .map_err(|error| error.to_string()),
                Err(Jin10Error::VerifiedEmpty(empty)) => {
                    verify_verified_empty(&empty, &ProbeAdmissionPolicy::new(ProviderId::Jin10))
                        .map(|_| 0)
                        .map_err(|error| error.to_string())
                }
                Err(error) => Err(error.to_string()),
            }
        } else {
            client
                .global_news(PositiveU32::new(10)?)
                .map(|batch| batch.records().len())
                .map_err(|error| error.to_string())
        };
        match result {
            Ok(record_count) => {
                successes += 1;
                records += record_count;
            }
            Err(error) => errors.push(format!("request_{}={error}", index + 1)),
        }
        latencies.push(request_started.elapsed().as_micros());
    }
    latencies.sort_unstable();
    let elapsed = started.elapsed();
    let failures = errors.len();
    let rps = requests as f64 / elapsed.as_secs_f64();
    println!(
        "provider=jin10-flash-v1 operation={} requests={requests} concurrency=1 min_interval_ms={} successes={successes} failures={failures} records={records} elapsed_seconds={:.3} requests_per_second={rps:.3} latency_us_p50={} latency_us_p95={} latency_us_p99={} latency_us_max={}",
        if release_observations { "economic_release_observations" } else { "global_news" },
        MIN_INTERVAL.as_millis(),
        elapsed.as_secs_f64(),
        percentile(&latencies, 50),
        percentile(&latencies, 95),
        percentile(&latencies, 99),
        latencies[latencies.len() - 1]
    );
    for error in &errors {
        println!("load_probe_error={error}");
    }
    if failures != 0 {
        return Err(format!("{failures} of {requests} requests failed").into());
    }
    println!("load_probe_status=passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_probe_is_client_paced_and_hard_bounded() {
        assert_eq!(MIN_INTERVAL, Duration::from_secs(1));
        assert!(validate_load(1).is_ok());
        assert!(validate_load(3).is_ok());
        assert!(validate_load(0).is_err());
        assert!(validate_load(4).is_err());
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 50), 3);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 95), 5);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 99), 5);
    }
}
