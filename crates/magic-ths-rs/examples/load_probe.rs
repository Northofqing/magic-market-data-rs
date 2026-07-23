use magic_market_core::{DataBatch, PopularityData, PositiveU32};
use magic_ths_rs::ThsClient;
use std::error::Error;
use std::fmt::Debug;
use std::time::{Duration, Instant};

const MAX_REQUESTS: u32 = 5;
const MIN_PACING_MS: u64 = 1_000;

fn main() -> Result<(), Box<dyn Error>> {
    let requests = env_u32("MAGIC_THS_LOAD_REQUESTS", 3)?;
    if requests == 0 || requests > MAX_REQUESTS {
        return Err(format!("MAGIC_THS_LOAD_REQUESTS must be in 1..={MAX_REQUESTS}").into());
    }
    let concurrency = env_u32("MAGIC_THS_LOAD_CONCURRENCY", 1)?;
    if concurrency != 1 {
        return Err("Tonghuashun public-data load probe requires concurrency=1".into());
    }
    let pacing_ms = u64::from(env_u32("MAGIC_THS_LOAD_PACING_MS", 1_000)?);
    if pacing_ms < MIN_PACING_MS {
        return Err(format!("pacing must be at least {MIN_PACING_MS} ms").into());
    }

    let client = ThsClient::new()?;
    let mut latencies = Vec::with_capacity(requests as usize);
    let mut successes = 0_u32;
    let mut failures = 0_u32;
    let mut previous_started: Option<Instant> = None;
    let mut minimum_start_gap: Option<Duration> = None;
    let total_started = Instant::now();

    for attempt in 0..requests {
        pace(previous_started, pacing_ms);
        let started = Instant::now();
        if let Some(previous) = previous_started {
            let gap = started.duration_since(previous);
            minimum_start_gap = Some(minimum_start_gap.map_or(gap, |value| value.min(gap)));
        }
        previous_started = Some(started);
        println!("\n--- attempt={} operation=popularity ---", attempt + 1);
        match client.popularity(PositiveU32::new(1)?) {
            Ok(batch) => {
                successes += 1;
                print_batch(batch);
            }
            Err(error) => {
                failures += 1;
                println!("error={error}");
            }
        }
        latencies.push(started.elapsed());
    }

    latencies.sort_unstable();
    let elapsed = total_started.elapsed();
    println!("\n=== load_summary ===");
    println!("provider=tonghuashun");
    println!("concurrency={concurrency}");
    println!("pacing_ms={pacing_ms}");
    println!("requests={requests}");
    println!("successes={successes}");
    println!("failures={failures}");
    println!("elapsed_ms={}", elapsed.as_millis());
    println!(
        "throughput_requests_per_second={:.4}",
        f64::from(requests) / elapsed.as_secs_f64()
    );
    println!("latency_min_ms={}", percentile(&latencies, 0).as_millis());
    println!("latency_p50_ms={}", percentile(&latencies, 50).as_millis());
    println!("latency_p95_ms={}", percentile(&latencies, 95).as_millis());
    println!("latency_max_ms={}", percentile(&latencies, 100).as_millis());
    println!(
        "minimum_attempt_start_gap_ms={}",
        minimum_start_gap.unwrap_or_default().as_millis()
    );
    if failures > 0 {
        return Err(format!("{failures} load-probe attempts failed").into());
    }
    Ok(())
}

fn pace(previous_started: Option<Instant>, pacing_ms: u64) {
    if let Some(previous) = previous_started {
        let elapsed = previous.elapsed();
        let pacing = Duration::from_millis(pacing_ms);
        if elapsed < pacing {
            std::thread::sleep(pacing - elapsed);
        }
    }
}

fn print_batch<T: Debug>(batch: DataBatch<T>) {
    println!("records={}", batch.records().len());
    println!("provenance={:#?}", batch.provenance());
    println!("quality={:#?}", batch.quality());
    for (index, record) in batch.records().iter().enumerate() {
        println!("record[{index}]={record:#?}");
    }
}

fn env_u32(name: &str, default: u32) -> Result<u32, Box<dyn Error>> {
    Ok(std::env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .parse::<u32>()?)
}

fn percentile(values: &[Duration], percentile: usize) -> Duration {
    if values.is_empty() {
        return Duration::ZERO;
    }
    let last = values.len() - 1;
    let index = (last * percentile).div_ceil(100);
    values[index.min(last)]
}
