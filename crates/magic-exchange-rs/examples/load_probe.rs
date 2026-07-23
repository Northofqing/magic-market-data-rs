use magic_exchange_rs::{ExchangeError, HkexClient, SseClient, SzseClient};
use magic_market_core::{
    Announcement, Announcements, AssetClass, DataBatch, DragonTigerData, DragonTigerEntry,
    Exchange, InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest, IsoDate,
    NorthboundChannel, NorthboundDailyRequest, NorthboundDailyStat, NorthboundDailyStatistics,
    OrderBook, OrderBooks, PositiveU32, Quote, RealtimeQuotes,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::time::{Duration, Instant};

const MAX_REQUESTS: u32 = 8;
const MIN_PACING_MS: u64 = 1_000;

fn main() -> Result<(), Box<dyn Error>> {
    let requests = env_u32("MAGIC_EXCHANGE_LOAD_REQUESTS", 8)?;
    if requests == 0 || requests > MAX_REQUESTS {
        return Err(format!("MAGIC_EXCHANGE_LOAD_REQUESTS must be in 1..={MAX_REQUESTS}").into());
    }
    let concurrency = env_u32("MAGIC_EXCHANGE_LOAD_CONCURRENCY", 1)?;
    if concurrency != 1 {
        return Err("official exchange load probe requires concurrency=1".into());
    }
    let pacing_ms = u64::from(env_u32("MAGIC_EXCHANGE_LOAD_PACING_MS", 1_000)?);
    if pacing_ms < MIN_PACING_MS {
        return Err(format!("pacing must be at least {MIN_PACING_MS} ms").into());
    }
    let sse_code = std::env::var("MAGIC_EXCHANGE_SSE_CODE").unwrap_or_else(|_| "600396".into());
    let szse_code = std::env::var("MAGIC_EXCHANGE_SZSE_CODE").unwrap_or_else(|_| "000858".into());
    let sse_dragon_date =
        std::env::var("MAGIC_EXCHANGE_SSE_DRAGON_DATE").unwrap_or_else(|_| "2026-07-22".into());
    let szse_dragon_code =
        std::env::var("MAGIC_EXCHANGE_SZSE_DRAGON_CODE").unwrap_or_else(|_| "000603".into());
    let szse_dragon_date =
        std::env::var("MAGIC_EXCHANGE_SZSE_DRAGON_DATE").unwrap_or_else(|_| "2026-07-23".into());
    let hkex_date =
        std::env::var("MAGIC_EXCHANGE_HKEX_DATE").unwrap_or_else(|_| "2026-07-22".into());
    let sse_request = request(Exchange::Shanghai, sse_code)?;
    let szse_request = request(Exchange::Shenzhen, szse_code)?;
    let szse_instruments = [szse_request.instrument().clone()];
    let sse_dragon_request = signal_request(
        Exchange::Shanghai,
        sse_request.instrument().code(),
        &sse_dragon_date,
    )?;
    let szse_dragon_request =
        signal_request(Exchange::Shenzhen, &szse_dragon_code, &szse_dragon_date)?;
    let sse = SseClient::new()?;
    let szse = SzseClient::new()?;
    let hkex = HkexClient::new()?;
    let hkex_date = IsoDate::new(hkex_date)?;

    let mut latencies = Vec::with_capacity(requests as usize);
    let mut successes = 0_u32;
    let mut failures = 0_u32;
    let mut error_categories = BTreeMap::<&'static str, u32>::new();
    let mut previous_started: Option<Instant> = None;
    let mut minimum_start_gap: Option<Duration> = None;
    let mut operation_elapsed_total = Duration::ZERO;
    let mut pacing_wait_total = Duration::ZERO;
    let mut outcomes = Vec::<(
        u32,
        &'static str,
        &'static str,
        Result<AttemptResult, ExchangeError>,
    )>::new();
    let measurement_started = Instant::now();

    for attempt in 0..requests {
        pacing_wait_total += pace(previous_started, pacing_ms);
        let (provider, operation) = match attempt % 8 {
            0 => ("sse", "announcements"),
            1 => ("szse", "announcements"),
            2 => ("szse", "quotes"),
            3 => ("szse", "order_books"),
            4 => ("sse", "dragon_tiger_entries"),
            5 => ("szse", "dragon_tiger_entries"),
            6 => ("hkex", "sse_northbound_daily"),
            _ => ("hkex", "szse_northbound_daily"),
        };
        let started = Instant::now();
        if let Some(previous) = previous_started {
            let gap = started.duration_since(previous);
            minimum_start_gap = Some(minimum_start_gap.map_or(gap, |value| value.min(gap)));
        }
        previous_started = Some(started);
        let result = match attempt % 8 {
            0 => sse
                .announcements(&sse_request)
                .map(AttemptResult::Announcements),
            1 => szse
                .announcements(&szse_request)
                .map(AttemptResult::Announcements),
            2 => szse
                .realtime_quotes(&szse_instruments)
                .map(AttemptResult::Quotes),
            3 => szse
                .order_books(&szse_instruments)
                .map(AttemptResult::OrderBooks),
            4 => sse
                .dragon_tiger_entries(&sse_dragon_request)
                .map(AttemptResult::DragonTigerEntries),
            5 => szse
                .dragon_tiger_entries(&szse_dragon_request)
                .map(AttemptResult::DragonTigerEntries),
            6 => hkex
                .northbound_daily_statistics(&NorthboundDailyRequest::new(
                    hkex_date.clone(),
                    NorthboundChannel::Shanghai,
                ))
                .map(AttemptResult::Northbound),
            _ => hkex
                .northbound_daily_statistics(&NorthboundDailyRequest::new(
                    hkex_date.clone(),
                    NorthboundChannel::Shenzhen,
                ))
                .map(AttemptResult::Northbound),
        }
        .and_then(|result| result.validate(operation));
        let operation_elapsed = started.elapsed();
        operation_elapsed_total += operation_elapsed;
        latencies.push(operation_elapsed);
        match &result {
            Ok(_) => {
                successes += 1;
            }
            Err(error) => {
                failures += 1;
                *error_categories.entry(error_category(error)).or_default() += 1;
            }
        }
        outcomes.push((attempt + 1, provider, operation, result));
    }
    let measurement_elapsed = measurement_started.elapsed();

    for (attempt, provider, operation, result) in outcomes {
        println!("\n--- attempt={attempt} operation={operation} provider={provider} ---");
        match result {
            Ok(AttemptResult::Announcements(batch)) => print_batch(&batch),
            Ok(AttemptResult::Quotes(batch)) => print_batch(&batch),
            Ok(AttemptResult::OrderBooks(batch)) => print_batch(&batch),
            Ok(AttemptResult::DragonTigerEntries(batch)) => print_batch(&batch),
            Ok(AttemptResult::Northbound(batch)) => print_batch(&batch),
            Err(error) => println!("error={error}"),
        }
    }
    latencies.sort_unstable();
    let wall_elapsed_including_output = measurement_started.elapsed();
    println!("\n=== load_summary ===");
    println!("provider=official-exchanges");
    println!("concurrency={concurrency}");
    println!("pacing_ms={pacing_ms}");
    println!("attempts={requests}");
    println!("successes={successes}");
    println!("failures={failures}");
    println!("error_categories={error_categories:?}");
    println!(
        "measurement_elapsed_ms_excluding_output={}",
        measurement_elapsed.as_millis()
    );
    println!(
        "wall_elapsed_ms_including_attempt_output={}",
        wall_elapsed_including_output.as_millis()
    );
    println!(
        "operation_elapsed_total_ms={}",
        operation_elapsed_total.as_millis()
    );
    println!("pacing_wait_total_ms={}", pacing_wait_total.as_millis());
    println!(
        "attempt_throughput_per_second={:.4}",
        f64::from(requests) / measurement_elapsed.as_secs_f64()
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
        "minimum_attempt_start_gap_ms={}",
        minimum_start_gap.unwrap_or_default().as_millis()
    );
    if failures > 0 {
        return Err(format!("{failures} load-probe attempts failed").into());
    }
    println!("load_probe_status=passed");
    Ok(())
}

enum AttemptResult {
    Announcements(DataBatch<Announcement>),
    Quotes(DataBatch<Quote>),
    OrderBooks(DataBatch<OrderBook>),
    DragonTigerEntries(DataBatch<DragonTigerEntry>),
    Northbound(DataBatch<NorthboundDailyStat>),
}

impl AttemptResult {
    fn validate(self, operation: &str) -> Result<Self, ExchangeError> {
        let (complete, records, minimum, maximum) = match &self {
            Self::Announcements(batch) => {
                (batch.quality().is_complete(), batch.records().len(), 1, 1)
            }
            Self::Quotes(batch) => (batch.quality().is_complete(), batch.records().len(), 1, 1),
            Self::OrderBooks(batch) => (batch.quality().is_complete(), batch.records().len(), 1, 1),
            Self::DragonTigerEntries(batch) => {
                (batch.quality().is_complete(), batch.records().len(), 1, 20)
            }
            Self::Northbound(batch) => (batch.quality().is_complete(), batch.records().len(), 1, 1),
        };
        validate_acceptance(operation, complete, records, minimum, maximum)?;
        Ok(self)
    }
}

fn validate_acceptance(
    operation: &str,
    complete: bool,
    records: usize,
    minimum: usize,
    maximum: usize,
) -> Result<(), ExchangeError> {
    if !complete {
        return Err(ExchangeError::Incomplete(format!(
            "{operation} returned incomplete quality"
        )));
    }
    if !(minimum..=maximum).contains(&records) {
        return Err(ExchangeError::Incomplete(format!(
            "{operation} returned {records} records; expected {minimum}..={maximum}"
        )));
    }
    Ok(())
}

fn print_batch<T: std::fmt::Debug>(batch: &DataBatch<T>) {
    println!("records={}", batch.records().len());
    println!("provenance={:#?}", batch.provenance());
    for (index, record) in batch.records().iter().enumerate() {
        println!("record[{index}]={record:#?}");
    }
}

fn request(exchange: Exchange, code: String) -> Result<InstrumentDateRangeRequest, Box<dyn Error>> {
    Ok(InstrumentDateRangeRequest::new(
        InstrumentId::new(exchange, code, AssetClass::Equity)?,
        PositiveU32::new(1)?,
    )?)
}

fn signal_request(
    exchange: Exchange,
    code: &str,
    trading_date: &str,
) -> Result<InstrumentSignalRequest, Box<dyn Error>> {
    Ok(InstrumentSignalRequest::new(
        InstrumentId::new(exchange, code, AssetClass::Equity)?,
        PositiveU32::new(20)?,
    )?
    .with_trading_date(IsoDate::new(trading_date)?))
}

fn pace(previous_started: Option<Instant>, pacing_ms: u64) -> Duration {
    if let Some(previous) = previous_started {
        let elapsed = previous.elapsed();
        let pacing = Duration::from_millis(pacing_ms);
        if elapsed < pacing {
            let wait_started = Instant::now();
            std::thread::sleep(pacing - elapsed);
            return wait_started.elapsed();
        }
    }
    Duration::ZERO
}

fn error_category(error: &ExchangeError) -> &'static str {
    match error {
        ExchangeError::InvalidRequest(_) => "invalid_request",
        ExchangeError::Unsupported(_) => "unsupported",
        ExchangeError::Authentication(_) => "authentication",
        ExchangeError::RateLimited => "rate_limited",
        ExchangeError::Transport(_) => "transport",
        ExchangeError::HttpStatus(_) => "http_status",
        ExchangeError::Decode(_) => "decode",
        ExchangeError::Schema(_) => "schema",
        ExchangeError::Incomplete(_) => "incomplete",
        ExchangeError::Core(_) => "core",
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
    use super::validate_acceptance;

    #[test]
    fn acceptance_rejects_incomplete_quality_and_wrong_cardinality() {
        assert!(validate_acceptance("order_books", false, 1, 1, 1).is_err());
        assert!(validate_acceptance("order_books", true, 0, 1, 1).is_err());
        assert!(validate_acceptance("order_books", true, 2, 1, 1).is_err());
        assert!(validate_acceptance("order_books", true, 1, 1, 1).is_ok());
    }
}
