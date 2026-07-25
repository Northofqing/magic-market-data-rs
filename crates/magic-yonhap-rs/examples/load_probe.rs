use magic_market_core::PositiveU32;
use magic_yonhap_rs::YonhapClient;
use std::error::Error;
use std::time::{Duration, Instant};

const MAX_REQUESTS: usize = 3;
const MIN_INTERVAL: Duration = Duration::from_secs(1);

fn validate_load(requests: usize) -> Result<(), String> {
    if (1..=MAX_REQUESTS).contains(&requests) {
        Ok(())
    } else {
        Err(format!(
            "MAGIC_YONHAP_LOAD_REQUESTS must be between 1 and {MAX_REQUESTS}"
        ))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let requests = std::env::var("MAGIC_YONHAP_LOAD_REQUESTS")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(2);
    validate_load(requests)?;

    let client = YonhapClient::new()?;
    let started = Instant::now();
    let mut records = 0_usize;
    for index in 0..requests {
        let request_started = Instant::now();
        println!(
            "request={} event=start elapsed_ms={}",
            index + 1,
            started.elapsed().as_millis()
        );
        let batch = client.probe_global_news(PositiveU32::new(10)?)?;
        records += batch.records().len();
        println!(
            "request={} event=complete latency_ms={} records={}",
            index + 1,
            request_started.elapsed().as_millis(),
            batch.records().len()
        );
    }
    let elapsed = started.elapsed();
    let minimum_elapsed = MIN_INTERVAL * requests.saturating_sub(1) as u32;
    if elapsed < minimum_elapsed {
        return Err(format!(
            "shared client pacing was shorter than {:?}: {:?}",
            minimum_elapsed, elapsed
        )
        .into());
    }
    println!(
        "provider=yonhap-cn-rss-v1 requests={requests} concurrency=1 min_interval_ms={} records={records} elapsed_seconds={:.3}",
        MIN_INTERVAL.as_millis(),
        elapsed.as_secs_f64()
    );
    println!("load_probe_status=passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_probe_is_serial_and_hard_bounded() {
        assert_eq!(MIN_INTERVAL, Duration::from_secs(1));
        assert!(validate_load(1).is_ok());
        assert!(validate_load(3).is_ok());
        assert!(validate_load(0).is_err());
        assert!(validate_load(4).is_err());
    }
}
