use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError};
use magic_market_core::{
    AssetClass, BoardCategory, BoardFlows, DataBatch, DragonTigerDiscovery,
    DragonTigerDiscoveryRequest, Exchange, FlowInterval, FlowScope, FundFlowRequest,
    FundFlowSeries, InstrumentId, IsoDate, LimitPoolKind, LimitPoolRequest, LimitPools,
    NewsProvider, PopularityData, PositiveU32, ReportScope, ResearchReports, ResearchRequest,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Debug;
use std::time::{Duration, Instant};

const MAX_HIGH_LEVEL_ATTEMPTS: u32 = 3;
const MIN_PACING_MS: u64 = 1_000;

fn main() -> Result<(), Box<dyn Error>> {
    let high_level_attempts = env_u32("MAGIC_EASTMONEY_LOAD_REQUESTS", 3)?;
    if high_level_attempts == 0 || high_level_attempts > MAX_HIGH_LEVEL_ATTEMPTS {
        return Err(format!(
            "MAGIC_EASTMONEY_LOAD_REQUESTS must be in 1..={MAX_HIGH_LEVEL_ATTEMPTS}"
        )
        .into());
    }
    let concurrency = env_u32("MAGIC_EASTMONEY_LOAD_CONCURRENCY", 1)?;
    if concurrency != 1 {
        return Err("Eastmoney public-web load probe requires concurrency=1".into());
    }
    let pacing_ms = u64::from(env_u32("MAGIC_EASTMONEY_LOAD_PACING_MS", 1_000)?);
    if pacing_ms < MIN_PACING_MS {
        return Err(format!("pacing must be at least {MIN_PACING_MS} ms").into());
    }
    let operation =
        std::env::var("MAGIC_EASTMONEY_LOAD_OPERATION").unwrap_or_else(|_| "mixed".into());
    let diagnostic_mode = is_diagnostic_operation(&operation);
    let client = EastmoneyClient::new()?;
    let instrument = InstrumentId::new(
        Exchange::Shanghai,
        std::env::var("MAGIC_EASTMONEY_CODE").unwrap_or_else(|_| "600396".into()),
        AssetClass::Equity,
    )?;
    let report_instrument = InstrumentId::new(
        Exchange::Shanghai,
        std::env::var("MAGIC_EASTMONEY_REPORT_CODE").unwrap_or_else(|_| "688017".into()),
        AssetClass::Equity,
    )?;
    let pool_date = magic_market_core::IsoDate::new(
        std::env::var("MAGIC_EASTMONEY_POOL_DATE").unwrap_or_else(|_| "2026-07-23".into()),
    )?;
    let dragon_date = IsoDate::new(
        std::env::var("MAGIC_EASTMONEY_DRAGON_DATE").unwrap_or_else(|_| "2026-07-24".into()),
    )?;

    let total_started = Instant::now();
    let mut latencies = Vec::with_capacity(high_level_attempts as usize);
    let mut successes = 0_u32;
    let mut failures = 0_u32;
    let mut diagnostic_successes = 0_u32;
    let mut diagnostic_failures = 0_u32;
    let mut previous_started: Option<Instant> = None;
    let mut minimum_start_gap: Option<Duration> = None;
    let mut limiter_waits = Vec::with_capacity(high_level_attempts as usize);
    let mut error_categories = BTreeMap::<&'static str, u32>::new();
    for attempt in 0..high_level_attempts {
        let mut limiter_wait = Duration::ZERO;
        if let Some(previous) = previous_started {
            let elapsed = previous.elapsed();
            let pacing = Duration::from_millis(pacing_ms);
            if elapsed < pacing {
                let wait_started = Instant::now();
                std::thread::sleep(pacing - elapsed);
                limiter_wait = wait_started.elapsed();
            }
        }
        limiter_waits.push(limiter_wait);
        let started = Instant::now();
        if let Some(previous) = previous_started {
            let gap = started.duration_since(previous);
            minimum_start_gap = Some(minimum_start_gap.map_or(gap, |value| value.min(gap)));
        }
        previous_started = Some(started);
        let selected = select_operation(&operation, attempt)?;
        println!("\n--- attempt={} operation={} ---", attempt + 1, selected);
        println!("limiter_wait_ms={}", limiter_wait.as_millis());
        println!("admitted={}", !diagnostic_mode);
        match run_operation(
            &client,
            &instrument,
            &report_instrument,
            &pool_date,
            &dragon_date,
            selected,
        ) {
            Ok(()) if diagnostic_mode => {
                diagnostic_successes += 1;
                println!("diagnostic_status=diagnostic_complete_unadmitted");
                println!("error_category=none");
            }
            Ok(()) => {
                successes += 1;
                println!("status=success");
                println!("error_category=none");
            }
            Err(error) if diagnostic_mode => {
                diagnostic_failures += 1;
                *error_categories.entry(error.category()).or_default() += 1;
                println!("diagnostic_status=diagnostic_failed");
                println!("error_category={}", error.category());
                println!("error={error}");
            }
            Err(error) => {
                failures += 1;
                *error_categories.entry(error.category()).or_default() += 1;
                println!("status=failed");
                println!("error_category={}", error.category());
                println!("error={error}");
            }
        }
        latencies.push(started.elapsed());
    }
    latencies.sort_unstable();
    let elapsed = total_started.elapsed();
    println!("\n=== load_summary ===");
    println!("provider=eastmoney-web");
    println!("concurrency={concurrency}");
    println!("pacing_ms={pacing_ms}");
    println!("high_level_attempts={high_level_attempts}");
    println!("admitted_successful_attempts={successes}");
    println!("admitted_failed_attempts={failures}");
    println!("diagnostic_complete_unadmitted_attempts={diagnostic_successes}");
    println!("diagnostic_failed_attempts={diagnostic_failures}");
    println!("total_elapsed_ms={}", elapsed.as_millis());
    println!(
        "attempts_per_second={:.4}",
        f64::from(high_level_attempts) / elapsed.as_secs_f64()
    );
    println!(
        "attempt_latency_min_ms={}",
        percentile(&latencies, 0).as_millis()
    );
    println!(
        "attempt_latency_p50_ms={}",
        percentile(&latencies, 50).as_millis()
    );
    println!(
        "attempt_latency_p95_ms={}",
        percentile(&latencies, 95).as_millis()
    );
    println!(
        "attempt_latency_p99_ms={}",
        percentile(&latencies, 99).as_millis()
    );
    println!(
        "attempt_latency_max_ms={}",
        percentile(&latencies, 100).as_millis()
    );
    println!(
        "limiter_wait_total_ms={}",
        limiter_waits.iter().sum::<Duration>().as_millis()
    );
    println!(
        "limiter_wait_p95_ms={}",
        percentile(&limiter_waits, 95).as_millis()
    );
    println!(
        "minimum_attempt_start_gap_ms={}",
        minimum_start_gap.unwrap_or_default().as_millis()
    );
    println!("error_categories={error_categories:?}");
    match completion_status(diagnostic_mode, failures, diagnostic_failures) {
        CompletionStatus::DiagnosticFailed(count) => {
            println!("load_probe_status=diagnostic_failed");
            Err(format!("{count} unadmitted diagnostic attempts failed").into())
        }
        CompletionStatus::DiagnosticCompleteUnadmitted => {
            println!("load_probe_status=diagnostic_complete_unadmitted");
            Ok(())
        }
        CompletionStatus::Failed(count) => {
            Err(format!("{count} load-probe attempts failed").into())
        }
        CompletionStatus::Passed => {
            println!("load_probe_status=passed");
            Ok(())
        }
    }
}

fn select_operation(requested: &str, attempt: u32) -> Result<&str, Box<dyn Error>> {
    const MIXED: &[&str] = &["research", "board-flow", "limit-pool", "popularity", "news"];
    const ALL: &[&str] = &[
        "research",
        "fund-flow",
        "board-flow",
        "limit-pool",
        "popularity",
        "dragon-tiger-discovery",
        "news",
    ];
    if requested == "mixed" {
        return Ok(MIXED[attempt as usize % MIXED.len()]);
    }
    if ALL.contains(&requested) {
        Ok(requested)
    } else {
        Err(format!(
            "unsupported operation {requested}; expected mixed or {}",
            ALL.join(",")
        )
        .into())
    }
}

fn is_diagnostic_operation(operation: &str) -> bool {
    operation == "fund-flow"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionStatus {
    Passed,
    Failed(u32),
    DiagnosticCompleteUnadmitted,
    DiagnosticFailed(u32),
}

fn completion_status(
    diagnostic_mode: bool,
    admitted_failures: u32,
    diagnostic_failures: u32,
) -> CompletionStatus {
    if diagnostic_mode {
        if diagnostic_failures > 0 {
            CompletionStatus::DiagnosticFailed(diagnostic_failures)
        } else {
            CompletionStatus::DiagnosticCompleteUnadmitted
        }
    } else if admitted_failures > 0 {
        CompletionStatus::Failed(admitted_failures)
    } else {
        CompletionStatus::Passed
    }
}

fn run_operation(
    client: &EastmoneyClient,
    instrument: &InstrumentId,
    report_instrument: &InstrumentId,
    pool_date: &magic_market_core::IsoDate,
    dragon_date: &IsoDate,
    operation: &str,
) -> Result<(), EastmoneyError> {
    let small = PositiveU32::new(3)?;
    match operation {
        "research" => {
            let request = ResearchRequest::new(
                ReportScope::Instrument(report_instrument.clone()),
                PositiveU32::new(1)?,
                small,
            )?;
            print_batch(client.research_reports(&request)?);
        }
        "fund-flow" => {
            let request = FundFlowRequest::new(
                FlowScope::Instrument(instrument.clone()),
                FlowInterval::Minute1,
                small,
            )?;
            print_batch(client.fund_flow_series(&request)?);
        }
        "board-flow" => {
            print_batch(client.board_flows(BoardCategory::Industry, FlowInterval::Day1, small)?);
        }
        "limit-pool" => {
            let request = LimitPoolRequest::new(LimitPoolKind::Upper, pool_date.clone(), small)?;
            print_batch(client.limit_pool(&request)?);
        }
        "popularity" => print_batch(client.popularity(small)?),
        "dragon-tiger-discovery" => {
            let request =
                DragonTigerDiscoveryRequest::new(dragon_date.clone(), PositiveU32::new(10_000)?)?;
            print_batch(client.discover_dragon_tiger(&request)?);
        }
        "news" => {
            print_batch(client.global_news(small)?);
        }
        _ => {
            return Err(EastmoneyError::InvalidRequest(format!(
                "unsupported operation {operation}"
            )))
        }
    }
    Ok(())
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
mod tests {
    use super::{completion_status, is_diagnostic_operation, select_operation, CompletionStatus};

    #[test]
    fn mixed_rotates_only_admitted_operations() {
        let selected = (0..10)
            .map(|attempt| select_operation("mixed", attempt).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            selected,
            vec![
                "research",
                "board-flow",
                "limit-pool",
                "popularity",
                "news",
                "research",
                "board-flow",
                "limit-pool",
                "popularity",
                "news"
            ]
        );
        assert!(!selected.contains(&"fund-flow"));
        assert!(selected.contains(&"news"));
    }

    #[test]
    fn unadmitted_operations_and_failure_statuses_are_explicit() {
        assert!(is_diagnostic_operation("fund-flow"));
        assert!(!is_diagnostic_operation("news"));
        assert!(!is_diagnostic_operation("research"));
        assert_eq!(
            completion_status(true, 0, 1),
            CompletionStatus::DiagnosticFailed(1)
        );
        assert_eq!(
            completion_status(true, 0, 0),
            CompletionStatus::DiagnosticCompleteUnadmitted
        );
        assert_eq!(completion_status(false, 2, 0), CompletionStatus::Failed(2));
        assert_eq!(completion_status(false, 0, 0), CompletionStatus::Passed);
    }

    #[test]
    fn invalid_operation_message_lists_every_explicit_diagnostic() {
        let error = select_operation("unknown", 0).unwrap_err().to_string();
        assert!(error.contains("fund-flow"));
        assert!(error.contains("news"));
    }
}
