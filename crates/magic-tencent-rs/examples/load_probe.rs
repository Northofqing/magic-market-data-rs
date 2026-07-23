use magic_market_core::{AssetClass, Exchange, InstrumentId, RealtimeQuotes};
use magic_tencent_rs::TencentClient;
use std::error::Error;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

const MAX_REQUESTS: usize = 100;
const MAX_CONCURRENCY: usize = 8;

fn percentile(sorted: &[u128], numerator: usize, denominator: usize) -> u128 {
    let index = (sorted.len() - 1) * numerator / denominator;
    sorted[index]
}

fn validate_load(requests: usize, concurrency: usize) -> Result<(), &'static str> {
    if requests == 0 || concurrency == 0 || concurrency > requests {
        return Err(
            "requests and concurrency must be positive; concurrency cannot exceed requests",
        );
    }
    if requests > MAX_REQUESTS || concurrency > MAX_CONCURRENCY {
        return Err("load probe exceeds its hard request/concurrency safety limit");
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let requests = std::env::var("MAGIC_TENCENT_LOAD_REQUESTS")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(20);
    let concurrency = std::env::var("MAGIC_TENCENT_LOAD_CONCURRENCY")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(4);
    validate_load(requests, concurrency)?;
    let instruments = Arc::new(vec![
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)?,
        InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity)?,
    ]);
    let client = TencentClient::with_timeout(Duration::from_secs(10))?;
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let mut workers = Vec::with_capacity(concurrency);
    for worker in 0..concurrency {
        let sender = sender.clone();
        let client = client.clone();
        let instruments = Arc::clone(&instruments);
        workers.push(
            std::thread::Builder::new()
                .name(format!("tencent-load-{worker}"))
                .spawn(move || {
                    for _ in (worker..requests).step_by(concurrency) {
                        let request_started = Instant::now();
                        let result = client
                            .realtime_quotes(&instruments)
                            .map(|batch| batch.records().len());
                        let _ = sender.send((request_started.elapsed(), result));
                    }
                })?,
        );
    }
    drop(sender);
    let mut latencies = Vec::with_capacity(requests);
    let mut successes = 0_usize;
    let mut records = 0_usize;
    let mut errors = Vec::new();
    for (latency, result) in receiver {
        latencies.push(latency.as_micros());
        match result {
            Ok(count) => {
                successes += 1;
                records += count;
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    for worker in workers {
        if worker.join().is_err() {
            return Err("a load-probe worker panicked".into());
        }
    }
    if latencies.len() != requests {
        return Err(format!(
            "worker completion mismatch: expected {requests}, received {}",
            latencies.len()
        )
        .into());
    }
    latencies.sort_unstable();
    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "provider=tencent-web requests={} concurrency={} successes={} failures={} records={} elapsed_seconds={elapsed:.3} requests_per_second={:.2} latency_us_p50={} latency_us_p95={} latency_us_max={}",
        requests,
        concurrency,
        successes,
        errors.len(),
        records,
        requests as f64 / elapsed,
        percentile(&latencies, 50, 100),
        percentile(&latencies, 95, 100),
        latencies[latencies.len() - 1]
    );
    if !errors.is_empty() {
        eprintln!("load_probe_status=failed errors={}", errors.join(" | "));
        std::process::exit(1);
    }
    println!("load_probe_status=passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_configuration_is_hard_bounded() {
        assert!(validate_load(20, 4).is_ok());
        assert!(validate_load(0, 1).is_err());
        assert!(validate_load(20, 21).is_err());
        assert!(validate_load(MAX_REQUESTS + 1, 4).is_err());
        assert!(validate_load(20, MAX_CONCURRENCY + 1).is_err());
    }
}
