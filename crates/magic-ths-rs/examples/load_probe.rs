use magic_market_core::{
    verify_serial_load, AssetClass, ConsensusData, DataBatch, Exchange, InstrumentId,
    InstrumentSignalRequest, IsoDate, LimitPoolKind, LimitPoolRequest, LimitPools, PopularityData,
    PositiveU32, ProbeStatus, StrongStockReasons,
};
use magic_ths_rs::{ThsClient, ThsError};
use std::error::Error;
use std::fmt::Debug;
use std::time::{Duration, Instant};

const MIN_REQUESTS: u32 = 4;
const MAX_REQUESTS: u32 = 8;
const MIN_PACING_MS: u64 = 1_000;

fn main() -> Result<(), Box<dyn Error>> {
    let requests = env_u32("MAGIC_THS_LOAD_REQUESTS", MIN_REQUESTS)?;
    if !(MIN_REQUESTS..=MAX_REQUESTS).contains(&requests) {
        return Err(
            format!("MAGIC_THS_LOAD_REQUESTS must be in {MIN_REQUESTS}..={MAX_REQUESTS}").into(),
        );
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
    let consensus_instrument =
        equity(std::env::var("MAGIC_THS_CONSENSUS_CODE").unwrap_or_else(|_| "600519".into()))?;
    let strong_instrument =
        equity(std::env::var("MAGIC_THS_STRONG_CODE").unwrap_or_else(|_| "000815".into()))?;
    let trading_date = IsoDate::new(
        std::env::var("MAGIC_THS_TRADING_DATE").unwrap_or_else(|_| "2026-07-23".into()),
    )?;
    let small = PositiveU32::new(1)?;
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
        let operation = select_operation(attempt);
        println!("\n--- attempt={} operation={operation} ---", attempt + 1);
        let result = match operation {
            "consensus" => match client.consensus(std::slice::from_ref(&consensus_instrument)) {
                Ok(batch) => {
                    print_batch(batch);
                    Ok(())
                }
                Err(ThsError::VerifiedEmpty(empty)) => {
                    println!("verified_empty={empty:#?}");
                    Ok(())
                }
                Err(error) => Err(error),
            },
            "strong_stock_reasons" => {
                let request = InstrumentSignalRequest::new(strong_instrument.clone(), small)?
                    .with_trading_date(trading_date.clone());
                client.strong_stock_reasons(&request).map(print_batch)
            }
            "upper_limit_pool" => {
                let request =
                    LimitPoolRequest::new(LimitPoolKind::Upper, trading_date.clone(), small)?;
                client.limit_pool(&request).map(print_batch)
            }
            "popularity" => client.popularity(small).map(print_batch),
            _ => unreachable!("operation selector is exhaustive"),
        };
        match result {
            Ok(()) => {
                successes += 1;
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
        println!("load_probe_status={}", ProbeStatus::Failed);
        return Err(format!("{failures} load-probe attempts failed").into());
    }
    let snapshot = client.load_probe_snapshot()?;
    let status = verify_serial_load(&snapshot, Duration::from_millis(MIN_PACING_MS))?;
    println!("actual_request_starts={}", snapshot.request_starts());
    println!(
        "actual_minimum_start_gap_ms={}",
        snapshot.minimum_start_gap().unwrap_or_default().as_millis()
    );
    println!(
        "actual_maximum_concurrency={}",
        snapshot.maximum_concurrency()
    );
    println!("load_probe_status={status}");
    Ok(())
}

fn select_operation(attempt: u32) -> &'static str {
    [
        "consensus",
        "strong_stock_reasons",
        "upper_limit_pool",
        "popularity",
    ][attempt as usize % 4]
}

fn equity(code: String) -> Result<InstrumentId, Box<dyn Error>> {
    let exchange = match code.as_bytes().first().copied() {
        Some(b'6') => Exchange::Shanghai,
        Some(b'0') | Some(b'3') => Exchange::Shenzhen,
        Some(b'4') | Some(b'8') => Exchange::Beijing,
        Some(b'9') if code.starts_with("920") => Exchange::Beijing,
        _ => return Err(format!("unsupported or unverified A-share code family: {code}").into()),
    };
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
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

#[cfg(test)]
#[path = "../tests/unit/load_probe_tests.rs"]
mod tests;
