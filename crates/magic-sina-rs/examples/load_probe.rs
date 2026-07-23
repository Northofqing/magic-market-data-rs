use magic_market_core::{
    AssetClass, BarInterval, BarsRequest, Exchange, FinancialStatements, HistoricalBars,
    InstrumentId, MinuteData, MinuteDataRequest, NonEmptyText, OptionData, RealtimeQuotes,
    StatementKind,
};
use magic_sina_rs::SinaClient;
use std::error::Error;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

const MAX_REQUESTS: usize = 40;
const MAX_CONCURRENCY: usize = 4;
const MAX_OPTION_CONTRACTS: usize = 50;
const MAX_OPTION_SAMPLE_CONTRACTS: usize = 10;

#[derive(Clone, Copy)]
enum Operation {
    Quotes,
    Bars,
    Minute,
    Financial,
    Options,
    Mixed,
}

impl Operation {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "quotes" => Ok(Self::Quotes),
            "bars" => Ok(Self::Bars),
            "minute" => Ok(Self::Minute),
            "financial" => Ok(Self::Financial),
            "options" => Ok(Self::Options),
            "mixed" => Ok(Self::Mixed),
            _ => Err("operation must be one of quotes, bars, minute, financial, options, mixed"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Quotes => "quotes",
            Self::Bars => "bars",
            Self::Minute => "minute",
            Self::Financial => "financial",
            Self::Options => "options",
            Self::Mixed => "mixed",
        }
    }

    const fn for_request(self, index: usize) -> Self {
        match self {
            Self::Mixed => match index % 4 {
                0 => Self::Quotes,
                1 => Self::Bars,
                2 => Self::Minute,
                _ => Self::Financial,
            },
            value => value,
        }
    }

    const fn requires_option_contracts(self) -> bool {
        matches!(self, Self::Options)
    }
}

fn parse_option_contracts(value: &str) -> Result<Vec<NonEmptyText>, String> {
    let contracts = value
        .split(',')
        .map(|contract| NonEmptyText::new(contract).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    if contracts.is_empty() || contracts.len() > MAX_OPTION_CONTRACTS {
        return Err(format!(
            "MAGIC_SINA_OPTION_CONTRACTS must contain between 1 and {MAX_OPTION_CONTRACTS} contracts"
        ));
    }
    for (index, contract) in contracts.iter().enumerate() {
        if contracts[..index]
            .iter()
            .any(|previous| previous == contract)
        {
            return Err(format!(
                "MAGIC_SINA_OPTION_CONTRACTS contains duplicate contract {}",
                contract.as_str()
            ));
        }
    }
    Ok(contracts)
}

fn resolve_option_contracts<Discover>(
    provided: Option<&str>,
    sample_contracts: usize,
    discover: Discover,
) -> Result<Vec<NonEmptyText>, String>
where
    Discover: FnOnce() -> Result<Vec<NonEmptyText>, String>,
{
    if sample_contracts == 0 || sample_contracts > MAX_OPTION_SAMPLE_CONTRACTS {
        return Err(format!(
            "MAGIC_SINA_OPTION_SAMPLE_CONTRACTS must be between 1 and {MAX_OPTION_SAMPLE_CONTRACTS}"
        ));
    }
    if let Some(provided) = provided {
        return parse_option_contracts(provided);
    }
    let discovered = discover()?;
    if discovered.is_empty() {
        return Err("option discovery returned no current contracts".into());
    }
    Ok(discovered.into_iter().take(sample_contracts).collect())
}

fn execute(
    client: &SinaClient,
    operation: Operation,
    index: usize,
    instruments: &[InstrumentId],
    primary: &InstrumentId,
    option_contracts: &[NonEmptyText],
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
        Operation::Financial => client
            .financial_statements(std::slice::from_ref(primary), StatementKind::Income)
            .map(|batch| batch.records().len())
            .map_err(|error| error.to_string()),
        Operation::Options => client
            .option_quotes(option_contracts)
            .and_then(|quotes| {
                client
                    .option_greeks(option_contracts)
                    .map(|greeks| quotes.records().len() + greeks.records().len())
            })
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
    let requests = std::env::var("MAGIC_SINA_LOAD_REQUESTS")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(20);
    let concurrency = std::env::var("MAGIC_SINA_LOAD_CONCURRENCY")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(4);
    let operation = Operation::parse(
        &std::env::var("MAGIC_SINA_LOAD_OPERATION").unwrap_or_else(|_| "mixed".to_owned()),
    )?;
    validate_load(requests, concurrency)?;
    let instruments = Arc::new(vec![
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)?,
        InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity)?,
    ]);
    let primary = Arc::new(instruments[0].clone());
    let client = SinaClient::with_timeout(Duration::from_secs(10))?;
    let option_contracts = if operation.requires_option_contracts() {
        let option_underlying_code =
            std::env::var("MAGIC_SINA_OPTION_UNDERLYING").unwrap_or_else(|_| "510050".to_owned());
        let option_underlying =
            InstrumentId::new(Exchange::Shanghai, option_underlying_code, AssetClass::Fund)?;
        let sample_contracts = std::env::var("MAGIC_SINA_OPTION_SAMPLE_CONTRACTS")
            .ok()
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(2);
        let provided = std::env::var("MAGIC_SINA_OPTION_CONTRACTS").ok();
        let contracts = resolve_option_contracts(
            provided.as_deref(),
            sample_contracts,
            || -> Result<Vec<NonEmptyText>, String> {
                client
                    .option_contracts(&option_underlying, None)
                    .map_err(|error| error.to_string())?
                    .records()
                    .iter()
                    .map(|contract| {
                        NonEmptyText::new(contract.contract_code.as_str())
                            .map_err(|error| error.to_string())
                    })
                    .collect()
            },
        )?;
        println!(
            "option_load_contracts source={} underlying={} count={}",
            if provided.is_some() {
                "environment"
            } else {
                "discovery"
            },
            option_underlying.code(),
            contracts.len()
        );
        Arc::new(contracts)
    } else {
        Arc::new(Vec::new())
    };
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let mut workers = Vec::with_capacity(concurrency);
    for worker in 0..concurrency {
        let sender = sender.clone();
        let client = client.clone();
        let instruments = Arc::clone(&instruments);
        let primary = Arc::clone(&primary);
        let option_contracts = Arc::clone(&option_contracts);
        workers.push(
            std::thread::Builder::new()
                .name(format!("sina-load-{worker}"))
                .spawn(move || {
                    for index in (worker..requests).step_by(concurrency) {
                        let request_started = Instant::now();
                        let result = execute(
                            &client,
                            operation,
                            index,
                            &instruments,
                            primary.as_ref(),
                            &option_contracts,
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
            Err(error) => errors.push(error),
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
        "provider=sina-web operation={} requests={} concurrency={} successes={} failures={} records={} elapsed_seconds={elapsed:.3} requests_per_second={:.2} latency_us_p50={} latency_us_p95={} latency_us_max={}",
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
        assert_eq!(Operation::parse("options").unwrap().label(), "options");
        assert!(Operation::parse("unknown").is_err());
        let mixed = Operation::Mixed;
        assert_eq!(mixed.for_request(0).label(), "quotes");
        assert_eq!(mixed.for_request(1).label(), "bars");
        assert_eq!(mixed.for_request(2).label(), "minute");
        assert_eq!(mixed.for_request(3).label(), "financial");
        assert_eq!(mixed.for_request(4).label(), "quotes");
        assert!(Operation::Options.requires_option_contracts());
        assert!(!Operation::Mixed.requires_option_contracts());
    }

    #[test]
    fn option_contracts_are_discovered_once_without_an_environment_override() {
        let contracts = resolve_option_contracts(None, 2, || {
            ["10000001", "10000002", "10000003"]
                .into_iter()
                .map(NonEmptyText::new)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())
        })
        .unwrap();

        assert_eq!(contracts.len(), 2);
        assert_eq!(contracts[0].as_str(), "10000001");
        assert_eq!(contracts[1].as_str(), "10000002");
    }

    #[test]
    fn explicit_option_contracts_bypass_discovery_and_reject_duplicates() {
        let contracts = resolve_option_contracts(Some("10000001,10000002"), 2, || {
            Err("discovery must not run".into())
        })
        .unwrap();
        assert_eq!(contracts.len(), 2);
        assert!(resolve_option_contracts(Some("10000001,10000001"), 2, || {
            Err("discovery must not run".into())
        })
        .is_err());
        assert!(resolve_option_contracts(None, 0, || Ok(Vec::new())).is_err());
        assert!(resolve_option_contracts(None, 2, || Ok(Vec::new())).is_err());
    }
}
