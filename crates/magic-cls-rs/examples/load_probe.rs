use magic_cls_rs::ClsClient;
use magic_market_core::{verify_serial_load, NewsProvider, PositiveU32, ProbeStatus};
use std::error::Error;
use std::time::{Duration, Instant};

const MAX_REQUESTS: usize = 3;
const MIN_INTERVAL: Duration = Duration::from_secs(1);

fn validate_load(requests: usize) -> Result<(), String> {
    if requests == 0 || requests > MAX_REQUESTS {
        return Err(format!(
            "MAGIC_CLS_LOAD_REQUESTS must be between 1 and {MAX_REQUESTS}"
        ));
    }
    Ok(())
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    let rank = (sorted.len() * percentile).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn main() -> Result<(), Box<dyn Error>> {
    let requests = std::env::var("MAGIC_CLS_LOAD_REQUESTS")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(2);
    validate_load(requests)?;
    let client = ClsClient::new()?;
    let started = Instant::now();
    let mut latencies = Vec::with_capacity(requests);
    let mut records = 0_usize;
    let mut successes = 0_usize;
    let mut errors = Vec::new();
    for index in 0..requests {
        let request_started = Instant::now();
        match client.global_news(PositiveU32::new(10)?) {
            Ok(batch) => {
                successes += 1;
                records += batch.records().len();
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
        "provider=cls-v1 requests={requests} concurrency=1 min_interval_ms={} successes={successes} failures={failures} records={records} elapsed_seconds={:.3} requests_per_second={rps:.3} latency_us_p50={} latency_us_p95={} latency_us_p99={} latency_us_max={}",
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
        println!("load_probe_status={}", ProbeStatus::Failed);
        return Err(format!("{failures} of {requests} requests failed").into());
    }
    let snapshot = client.load_probe_snapshot()?;
    let status = verify_serial_load(&snapshot, MIN_INTERVAL)?;
    println!("actual_request_starts={}", snapshot.request_starts());
    println!(
        "actual_minimum_start_gap_ms={}",
        snapshot.minimum_start_gap().unwrap_or_default().as_millis()
    );
    println!(
        "observed_maximum_concurrency={}",
        snapshot.maximum_concurrency()
    );
    println!("load_probe_status={status}");
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/load_probe_tests.rs"]
mod tests;
