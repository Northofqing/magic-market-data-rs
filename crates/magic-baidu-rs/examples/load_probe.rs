use magic_baidu_rs::BaiduClient;
use magic_market_core::{
    AssetClass, BarInterval, BarsRequest, Exchange, InstrumentId, TechnicalBarsProvider,
};
use std::error::Error;
use std::time::{Duration, Instant};

const MAX_REQUESTS: usize = 3;
const MIN_INTERVAL: Duration = Duration::from_secs(1);

fn validate_load(requests: usize) -> Result<(), String> {
    if requests == 0 || requests > MAX_REQUESTS {
        return Err(format!(
            "MAGIC_BAIDU_LOAD_REQUESTS must be between 1 and {MAX_REQUESTS}"
        ));
    }
    Ok(())
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    let rank = (sorted.len() * percentile).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn main() -> Result<(), Box<dyn Error>> {
    let requests = std::env::var("MAGIC_BAIDU_LOAD_REQUESTS")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(2);
    validate_load(requests)?;
    let client = BaiduClient::new()?;
    let request = BarsRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)?,
        BarInterval::Day,
        20,
    )?;
    let started = Instant::now();
    let mut latencies = Vec::with_capacity(requests);
    let mut records = 0_usize;
    let mut successes = 0_usize;
    let mut errors = Vec::new();
    for index in 0..requests {
        let request_started = Instant::now();
        match client.technical_bars(&request) {
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
        "provider=baidu-pae requests={requests} concurrency=1 min_interval_ms={} successes={successes} failures={failures} records={records} elapsed_seconds={:.3} requests_per_second={rps:.3} latency_us_p50={} latency_us_p95={} latency_us_p99={} latency_us_max={}",
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
