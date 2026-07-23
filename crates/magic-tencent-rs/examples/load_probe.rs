use magic_market_core::{
    AssetClass, BarInterval, BarsRequest, Exchange, HistoricalBars, InstrumentId,
    MarketStatisticsProvider, MinuteData, MinuteDataRequest, RealtimeQuotes, Trades, TradesRequest,
};
use magic_tencent_rs::TencentClient;
use std::error::Error;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

const MAX_REQUESTS: usize = 100;
const MAX_CONCURRENCY: usize = 8;

#[derive(Clone, Copy)]
enum Operation {
    Quotes,
    Bars,
    Minute,
    Trades,
    Statistics,
    Mixed,
}

impl Operation {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "quotes" => Ok(Self::Quotes),
            "bars" => Ok(Self::Bars),
            "minute" => Ok(Self::Minute),
            "trades" => Ok(Self::Trades),
            "statistics" => Ok(Self::Statistics),
            "mixed" => Ok(Self::Mixed),
            _ => Err("operation must be one of quotes, bars, minute, trades, statistics, mixed"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Quotes => "quotes",
            Self::Bars => "bars",
            Self::Minute => "minute",
            Self::Trades => "trades",
            Self::Statistics => "statistics",
            Self::Mixed => "mixed",
        }
    }

    const fn for_request(self, index: usize) -> Self {
        match self {
            Self::Mixed => match index % 5 {
                0 => Self::Quotes,
                1 => Self::Bars,
                2 => Self::Minute,
                3 => Self::Trades,
                _ => Self::Statistics,
            },
            value => value,
        }
    }
}

fn execute(
    client: &TencentClient,
    operation: Operation,
    index: usize,
    instruments: &[InstrumentId],
    statistics_instruments: &[InstrumentId],
    primary: &InstrumentId,
) -> Result<usize, String> {
    match operation.for_request(index) {
        Operation::Quotes => client
            .realtime_quotes(instruments)
            .map(|batch| batch.records().len())
            .map_err(|error| error.to_string()),
        Operation::Bars => BarsRequest::new(primary.clone(), BarInterval::Day, 5)
            .map_err(|error| error.to_string())
            .and_then(|request| {
                client
                    .historical_bars(&request)
                    .map(|batch| batch.records().len())
                    .map_err(|error| error.to_string())
            }),
        Operation::Minute => client
            .minute_data(&MinuteDataRequest::new(primary.clone()))
            .map(|batch| batch.records().len())
            .map_err(|error| error.to_string()),
        Operation::Trades => TradesRequest::new(primary.clone(), 20)
            .map_err(|error| error.to_string())
            .and_then(|request| {
                client
                    .trades(&request)
                    .map(|batch| batch.records().len())
                    .map_err(|error| error.to_string())
            }),
        Operation::Statistics => client
            .market_statistics(statistics_instruments)
            .map(|batch| batch.records().len())
            .map_err(|error| error.to_string()),
        Operation::Mixed => unreachable!("mixed is resolved before dispatch"),
    }
}

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
    let operation = Operation::parse(
        &std::env::var("MAGIC_TENCENT_LOAD_OPERATION").unwrap_or_else(|_| "mixed".to_owned()),
    )?;
    validate_load(requests, concurrency)?;
    let instruments = Arc::new(vec![
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)?,
        InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity)?,
    ]);
    let primary = Arc::new(instruments[0].clone());
    let statistics_instruments = Arc::new(vec![
        instruments[0].clone(),
        InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index)?,
        InstrumentId::new(Exchange::Shanghai, "510050", AssetClass::Fund)?,
    ]);
    let client = TencentClient::with_timeout(Duration::from_secs(10))?;
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let mut workers = Vec::with_capacity(concurrency);
    for worker in 0..concurrency {
        let sender = sender.clone();
        let client = client.clone();
        let instruments = Arc::clone(&instruments);
        let statistics_instruments = Arc::clone(&statistics_instruments);
        let primary = Arc::clone(&primary);
        workers.push(
            std::thread::Builder::new()
                .name(format!("tencent-load-{worker}"))
                .spawn(move || {
                    for index in (worker..requests).step_by(concurrency) {
                        let request_started = Instant::now();
                        let result = execute(
                            &client,
                            operation,
                            index,
                            &instruments,
                            &statistics_instruments,
                            primary.as_ref(),
                        );
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
        "provider=tencent-web operation={} requests={} concurrency={} successes={} failures={} records={} elapsed_seconds={elapsed:.3} requests_per_second={:.2} latency_us_p50={} latency_us_p95={} latency_us_max={}",
        operation.label(),
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

    #[test]
    fn operation_selector_is_explicit_and_mixed_is_deterministic() {
        assert_eq!(Operation::parse("quotes").unwrap().label(), "quotes");
        assert_eq!(Operation::parse("mixed").unwrap().label(), "mixed");
        assert!(Operation::parse("unknown").is_err());
        let mixed = Operation::Mixed;
        assert_eq!(mixed.for_request(0).label(), "quotes");
        assert_eq!(mixed.for_request(1).label(), "bars");
        assert_eq!(mixed.for_request(2).label(), "minute");
        assert_eq!(mixed.for_request(3).label(), "trades");
        assert_eq!(mixed.for_request(4).label(), "statistics");
        assert_eq!(mixed.for_request(5).label(), "quotes");
    }
}
