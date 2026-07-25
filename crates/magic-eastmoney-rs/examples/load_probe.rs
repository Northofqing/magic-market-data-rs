use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError};
use magic_market_core::{
    verify_serial_load, AssetClass, BlockTrades, BoardCategory, BoardFlows, DataBatch,
    DividendPlans, DragonTigerData, DragonTigerDiscovery, DragonTigerDiscoveryRequest, Exchange,
    FlowInterval, FlowScope, FundFlowRequest, FundFlowSeries, HolderCounts,
    InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest, IsoDate, LimitPoolKind,
    LimitPoolRequest, LimitPools, LockupEvents, MarginData, NewsProvider, NonEmptyText,
    PopularityData, PositiveU32, ProbeStatus, ReportScope, ResearchReports, ResearchRequest,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Debug;
use std::time::{Duration, Instant};

const MAX_HIGH_LEVEL_ATTEMPTS: u32 = 20;
const SUITE_ATTEMPTS: u32 = 19;
const MIN_PACING_MS: u64 = 1_000;

fn main() -> Result<(), Box<dyn Error>> {
    let operation =
        std::env::var("MAGIC_EASTMONEY_LOAD_OPERATION").unwrap_or_else(|_| "suite".into());
    let high_level_attempts = env_u32(
        "MAGIC_EASTMONEY_LOAD_REQUESTS",
        if operation == "suite" {
            SUITE_ATTEMPTS
        } else {
            1
        },
    )?;
    if high_level_attempts == 0 || high_level_attempts > MAX_HIGH_LEVEL_ATTEMPTS {
        return Err(format!(
            "MAGIC_EASTMONEY_LOAD_REQUESTS must be in 1..={MAX_HIGH_LEVEL_ATTEMPTS}"
        )
        .into());
    }
    if operation == "suite" && high_level_attempts < SUITE_ATTEMPTS {
        return Err(format!(
            "suite requires at least {SUITE_ATTEMPTS} requests to cover every advertised family"
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
    let diagnostic_mode = is_diagnostic_operation(&operation) || operation != "suite";
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
    let status = completion_status(diagnostic_mode, failures, diagnostic_failures);
    if !matches!(
        status,
        CompletionStatus::Failed(_) | CompletionStatus::DiagnosticFailed(_)
    ) {
        let snapshot = client.load_probe_snapshot()?;
        let pacing_status = verify_serial_load(&snapshot, Duration::from_millis(MIN_PACING_MS))?;
        println!("actual_request_starts={}", snapshot.request_starts());
        println!(
            "actual_minimum_start_gap_ms={}",
            snapshot.minimum_start_gap().unwrap_or_default().as_millis()
        );
        println!(
            "actual_maximum_concurrency={}",
            snapshot.maximum_concurrency()
        );
        println!("pacing_probe_status={pacing_status}");
    }
    match status {
        CompletionStatus::DiagnosticFailed(count) => {
            println!("load_probe_status=diagnostic_failed");
            Err(format!("{count} unadmitted diagnostic attempts failed").into())
        }
        CompletionStatus::DiagnosticCompleteUnadmitted => {
            println!("load_probe_status=diagnostic_complete_unadmitted");
            Ok(())
        }
        CompletionStatus::Failed(count) => {
            println!("load_probe_status={}", ProbeStatus::Failed);
            Err(format!("{count} load-probe attempts failed").into())
        }
        CompletionStatus::Admitted => {
            println!("load_probe_status={}", ProbeStatus::Admitted);
            Ok(())
        }
    }
}

fn select_operation(requested: &str, attempt: u32) -> Result<&str, Box<dyn Error>> {
    const SUITE: &[&str] = &[
        "research-instrument",
        "research-industry",
        "board-flow-industry",
        "board-flow-concept",
        "board-flow-region",
        "dragon-tiger-entries",
        "dragon-tiger-seats",
        "margin",
        "block-trades",
        "holder-counts",
        "dividends",
        "lockups",
        "limit-pool-upper",
        "limit-pool-broken",
        "limit-pool-lower",
        "limit-pool-previous-upper",
        "popularity",
        "dragon-tiger-discovery",
        "news",
    ];
    const ALL: &[&str] = &[
        "research-instrument",
        "research-industry",
        "fund-flow",
        "board-flow-industry",
        "board-flow-concept",
        "board-flow-region",
        "dragon-tiger-entries",
        "dragon-tiger-seats",
        "margin",
        "block-trades",
        "holder-counts",
        "dividends",
        "lockups",
        "limit-pool-upper",
        "limit-pool-broken",
        "limit-pool-lower",
        "limit-pool-previous-upper",
        "popularity",
        "dragon-tiger-discovery",
        "news",
    ];
    if requested == "suite" {
        return Ok(SUITE[attempt as usize % SUITE.len()]);
    }
    if ALL.contains(&requested) {
        Ok(requested)
    } else {
        Err(format!(
            "unsupported operation {requested}; expected suite or {}",
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
    Admitted,
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
        CompletionStatus::Admitted
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
        "research-instrument" => {
            let request = ResearchRequest::new(
                ReportScope::Instrument(report_instrument.clone()),
                PositiveU32::new(1)?,
                small,
            )?;
            print_batch(client.research_reports(&request)?);
        }
        "research-industry" => {
            let request = ResearchRequest::new(
                ReportScope::Industry(NonEmptyText::new("*")?),
                PositiveU32::new(1)?,
                PositiveU32::new(1)?,
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
        "board-flow-industry" => {
            print_batch(client.board_flows(BoardCategory::Industry, FlowInterval::Day1, small)?);
        }
        "board-flow-concept" => {
            print_batch(client.board_flows(BoardCategory::Concept, FlowInterval::Day1, small)?);
        }
        "board-flow-region" => {
            print_batch(client.board_flows(BoardCategory::Region, FlowInterval::Day1, small)?);
        }
        "dragon-tiger-entries" => {
            let request = InstrumentSignalRequest::new(instrument.clone(), PositiveU32::new(1)?)?;
            print_batch(client.dragon_tiger_entries(&request)?);
        }
        "dragon-tiger-seats" => {
            let request = InstrumentSignalRequest::new(instrument.clone(), PositiveU32::new(1)?)?;
            print_batch(client.dragon_tiger_seats(&request)?);
        }
        "margin" => {
            let request =
                InstrumentDateRangeRequest::new(instrument.clone(), PositiveU32::new(1)?)?;
            print_batch(client.margin_data(&request)?);
        }
        "block-trades" => {
            let request =
                InstrumentDateRangeRequest::new(instrument.clone(), PositiveU32::new(1)?)?;
            print_batch(client.block_trades(&request)?);
        }
        "holder-counts" => {
            let request =
                InstrumentDateRangeRequest::new(instrument.clone(), PositiveU32::new(1)?)?;
            print_batch(client.holder_counts(&request)?);
        }
        "dividends" => {
            let request =
                InstrumentDateRangeRequest::new(instrument.clone(), PositiveU32::new(1)?)?;
            print_batch(client.dividend_plans(&request)?);
        }
        "lockups" => {
            let request =
                InstrumentDateRangeRequest::new(instrument.clone(), PositiveU32::new(1)?)?;
            print_batch(client.lockup_events(&request)?);
        }
        "limit-pool-upper"
        | "limit-pool-broken"
        | "limit-pool-lower"
        | "limit-pool-previous-upper" => {
            let kind = match operation {
                "limit-pool-upper" => LimitPoolKind::Upper,
                "limit-pool-broken" => LimitPoolKind::Broken,
                "limit-pool-lower" => LimitPoolKind::Lower,
                "limit-pool-previous-upper" => LimitPoolKind::PreviousUpper,
                _ => unreachable!("matched limit-pool operation"),
            };
            let request = LimitPoolRequest::new(kind, pool_date.clone(), small)?;
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
#[path = "../tests/unit/load_probe_tests.rs"]
mod tests;
