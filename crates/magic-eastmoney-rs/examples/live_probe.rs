use magic_eastmoney_rs::EastmoneyClient;
use magic_market_core::{
    verify_admitted_batch, AssetClass, BlockTrades, BoardCategory, BoardFlows, DataBatch,
    DividendPlans, DragonTigerData, DragonTigerDiscovery, DragonTigerDiscoveryRequest,
    DragonTigerEntry, Exchange, FlowInterval, FlowScope, FundFlowRequest, FundFlowSeries,
    HolderCounts, InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest, IsoDate,
    LimitPoolKind, LimitPoolRequest, LimitPools, LockupEvents, MarginData, MarketDragonTigerData,
    MarketDragonTigerRequest, NewsProvider, PopularityData, PositiveU32, ProbeAdmissionPolicy,
    ProbeStatus, ProviderId, ReportScope, ResearchReports, ResearchRequest, SourceEvidence,
};
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt::Debug;

fn main() -> Result<(), Box<dyn Error>> {
    let pool_date = IsoDate::new(required_env("MAGIC_EASTMONEY_POOL_DATE")?)?;
    let dragon_tiger_date = IsoDate::new(required_env("MAGIC_EASTMONEY_DRAGON_TIGER_DATE")?)?;
    let client = EastmoneyClient::new()?;
    let mut failures = Vec::new();
    let primary = instrument(Exchange::Shanghai, env("MAGIC_EASTMONEY_CODE", "600396"))?;
    let reference = instrument(
        Exchange::Shanghai,
        env("MAGIC_EASTMONEY_REFERENCE", "600519"),
    )?;
    let event_sample = instrument(
        Exchange::Shenzhen,
        env("MAGIC_EASTMONEY_EVENT_CODE", "002475"),
    )?;
    let report_sample = instrument(
        Exchange::Shanghai,
        env("MAGIC_EASTMONEY_REPORT_CODE", "688017"),
    )?;
    let policy = ProbeAdmissionPolicy::new(ProviderId::Eastmoney);
    let source_policy = policy.require_source_at();
    let single = PositiveU32::new(1)?;
    let small = PositiveU32::new(3)?;

    println!("provider=eastmoney-web");
    println!(
        "research_capabilities={:#?}",
        EastmoneyClient::research_capabilities()
    );
    println!(
        "capital_capabilities={:#?}",
        EastmoneyClient::capital_capabilities()
    );
    println!(
        "signal_capabilities={:#?}",
        EastmoneyClient::signal_capabilities()
    );
    println!(
        "limit_pool_capabilities={:#?}",
        EastmoneyClient::limit_pool_capabilities()
    );
    println!(
        "content_capabilities={:#?}",
        EastmoneyClient::content_capabilities()
    );
    println!(
        "market_discovery_capabilities={:#?}",
        EastmoneyClient::market_discovery_capabilities()
    );

    if std::env::var("MAGIC_EASTMONEY_LIVE_OPERATION").as_deref() == Ok("global-news") {
        probe_batch(
            "content.global_news",
            client.global_news(PositiveU32::new(5)?),
            &source_policy,
            |record| &record.evidence,
            |record| record.item_id.as_str().to_owned(),
            &mut failures,
        );
        return print_summary(&failures);
    }

    let dragon_date =
        IsoDate::new(std::env::var("MAGIC_EASTMONEY_DRAGON_DATE").map_err(|_| {
            "MAGIC_EASTMONEY_DRAGON_DATE=YYYY-MM-DD is required for the discovery live probe"
        })?)?;
    let dragon_request = DragonTigerDiscoveryRequest::new(dragon_date, PositiveU32::new(10_000)?)?;
    probe_dragon_discovery(client.discover_dragon_tiger(&dragon_request), &mut failures);
    if std::env::var("MAGIC_EASTMONEY_LIVE_OPERATION").as_deref() == Ok("dragon-tiger-discovery") {
        return print_summary(&failures);
    }

    let report = ResearchRequest::new(
        ReportScope::Instrument(report_sample),
        PositiveU32::new(1)?,
        single,
    )?;
    probe_batch(
        "research.instrument",
        client.research_reports(&report),
        &source_policy,
        |record| &record.evidence,
        |record| record.report_id.as_str().to_owned(),
        &mut failures,
    );
    let industry = ResearchRequest::new(
        ReportScope::Industry(magic_market_core::NonEmptyText::new(env(
            "MAGIC_EASTMONEY_INDUSTRY",
            "*",
        ))?),
        PositiveU32::new(1)?,
        single,
    )?;
    probe_batch(
        "research.industry",
        client.research_reports(&industry),
        &source_policy,
        |record| &record.evidence,
        |record| record.report_id.as_str().to_owned(),
        &mut failures,
    );

    for interval in [FlowInterval::Minute1, FlowInterval::Day1] {
        let request = FundFlowRequest::new(
            FlowScope::Instrument(primary.clone()),
            interval,
            PositiveU32::new(5)?,
        )?;
        probe_unadmitted_batch(
            &format!("fund_flow.{interval:?}"),
            client.fund_flow_series(&request),
        );
    }
    for category in [
        BoardCategory::Industry,
        BoardCategory::Concept,
        BoardCategory::Region,
    ] {
        probe_batch(
            &format!("board_flow.{category:?}"),
            client.board_flows(category, FlowInterval::Day1, small),
            &source_policy,
            |record| &record.evidence,
            |record| {
                format!(
                    "{:?}:{}:{:?}",
                    record.category, record.board_code, record.interval
                )
            },
            &mut failures,
        );
    }

    let signal = InstrumentSignalRequest::new(event_sample.clone(), single)?;
    probe_batch(
        "dragon_tiger.entries",
        client.dragon_tiger_entries(&signal),
        &source_policy,
        |record| record.evidence(),
        |record| record.entry_id().as_str().to_owned(),
        &mut failures,
    );
    probe_batch(
        "dragon_tiger.seats",
        client.dragon_tiger_seats(&InstrumentSignalRequest::new(
            event_sample.clone(),
            PositiveU32::new(10)?,
        )?),
        &source_policy,
        |record| record.evidence(),
        |record| {
            format!(
                "{}:{:?}:{}:{}",
                record.entry_id(),
                record.side(),
                record.rank().get(),
                record.seat_name()
            )
        },
        &mut failures,
    );
    probe_batch(
        "dragon_tiger.market",
        client.market_dragon_tiger(&MarketDragonTigerRequest::new(dragon_tiger_date, small)?),
        &source_policy,
        |record| record.entry().evidence(),
        |record| {
            format!(
                "{}:seats={}",
                record.entry().entry_id(),
                record.seats().len()
            )
        },
        &mut failures,
    );

    let reference_request = InstrumentDateRangeRequest::new(reference, single)?;
    probe_batch(
        "capital.margin",
        client.margin_data(&reference_request),
        &source_policy,
        |record| &record.evidence,
        |record| {
            format!(
                "{}:{}",
                instrument_identity(&record.instrument),
                record.trading_date
            )
        },
        &mut failures,
    );
    probe_batch(
        "capital.block_trades",
        client.block_trades(&reference_request),
        &source_policy,
        |record| &record.evidence,
        |record| {
            format!(
                "{}:{}:{:?}:{}:{}:{:?}:{:?}",
                instrument_identity(&record.instrument),
                record.trading_date,
                record.traded_at,
                record.price.get(),
                record.volume.get(),
                record.buyer,
                record.seller
            )
        },
        &mut failures,
    );
    probe_batch(
        "capital.holder_counts",
        client.holder_counts(&reference_request),
        &source_policy,
        |record| &record.evidence,
        |record| {
            format!(
                "{}:{}",
                instrument_identity(&record.instrument),
                record.report_date
            )
        },
        &mut failures,
    );
    probe_batch(
        "capital.dividends",
        client.dividend_plans(&reference_request),
        &source_policy,
        |record| &record.evidence,
        |record| {
            format!(
                "{}:{}:{}:{:?}",
                instrument_identity(&record.instrument),
                record.report_date,
                record.state,
                record.ex_dividend_date
            )
        },
        &mut failures,
    );
    let event_request = InstrumentDateRangeRequest::new(event_sample, single)?;
    probe_batch(
        "capital.lockups",
        client.lockup_events(&event_request),
        &source_policy,
        |record| &record.evidence,
        |record| {
            format!(
                "{}:{}:{}",
                instrument_identity(&record.instrument),
                record.listing_date,
                record.share_type
            )
        },
        &mut failures,
    );

    for kind in [
        LimitPoolKind::Upper,
        LimitPoolKind::Broken,
        LimitPoolKind::Lower,
        LimitPoolKind::PreviousUpper,
    ] {
        let request = complete_limit_pool_request(kind, pool_date.clone())?;
        probe_batch(
            &format!("limit_pool.{kind:?}"),
            client.limit_pool(&request),
            &source_policy,
            |record| &record.evidence,
            |record| {
                format!(
                    "{:?}:{}:{}",
                    record.kind,
                    instrument_identity(&record.instrument),
                    record.trading_date
                )
            },
            &mut failures,
        );
    }

    probe_batch(
        "popularity",
        client.popularity(PositiveU32::new(5)?),
        &policy,
        |record| &record.evidence,
        |record| instrument_identity(&record.instrument),
        &mut failures,
    );
    probe_batch(
        "content.global_news",
        client.global_news(PositiveU32::new(5)?),
        &source_policy,
        |record| &record.evidence,
        |record| record.item_id.as_str().to_owned(),
        &mut failures,
    );
    let news_request = InstrumentDateRangeRequest::new(primary, PositiveU32::new(5)?)?;
    probe_unadmitted_batch(
        "content.instrument_news",
        client.instrument_news(&news_request),
    );
    print_summary(&failures)
}

fn print_summary(failures: &[String]) -> Result<(), Box<dyn Error>> {
    println!("\n=== probe_summary ===");
    println!("failures={}", failures.len());
    for failure in failures {
        println!("{failure}");
    }
    if !failures.is_empty() {
        return Err(format!("{} live-probe families failed", failures.len()).into());
    }
    println!("live_probe_status={}", ProbeStatus::Admitted);
    Ok(())
}

fn probe_dragon_discovery<E: std::fmt::Display>(
    result: Result<DataBatch<DragonTigerEntry>, E>,
    failures: &mut Vec<String>,
) {
    println!("\n=== dragon_tiger.discovery ===");
    match result {
        Ok(batch) => {
            let mut exchanges = BTreeMap::<String, usize>::new();
            let unique_ids = batch
                .records()
                .iter()
                .map(|record| {
                    *exchanges
                        .entry(format!("{:?}", record.instrument().exchange()))
                        .or_default() += 1;
                    record.entry_id().as_str()
                })
                .collect::<HashSet<_>>()
                .len();
            println!("records={}", batch.records().len());
            println!("exchange_distribution={exchanges:?}");
            println!("unique_entry_ids={unique_ids}");
            println!("provenance={:#?}", batch.provenance());
            println!("quality={:#?}", batch.quality());
            if batch.records().is_empty() || unique_ids != batch.records().len() {
                failures.push(
                    "dragon_tiger.discovery: empty result or duplicate entry identity".into(),
                );
            }
        }
        Err(error) => {
            println!("error={error}");
            failures.push(format!("dragon_tiger.discovery: {error}"));
        }
    }
}

fn probe_unadmitted_batch<T: Debug, E: std::fmt::Display>(
    label: &str,
    result: Result<DataBatch<T>, E>,
) {
    println!("\n=== {label} ===");
    println!("admitted=false");
    match result {
        Ok(batch) => {
            println!("status={}", ProbeStatus::DiagnosticCompleteUnadmitted);
            print_batch(label, &batch);
        }
        Err(error) => {
            println!("status={}", ProbeStatus::Failed);
            println!("error={error}");
        }
    }
}

fn instrument(exchange: Exchange, code: String) -> Result<InstrumentId, Box<dyn Error>> {
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
}

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn required_env(name: &str) -> Result<String, std::io::Error> {
    let value = std::env::var(name).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{name} is required and must identify the source trading session"),
        )
    })?;
    if value.trim().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{name} must not be empty"),
        ));
    }
    Ok(value)
}

fn complete_limit_pool_request(
    kind: LimitPoolKind,
    trading_date: IsoDate,
) -> Result<LimitPoolRequest, magic_market_core::CoreError> {
    LimitPoolRequest::new(kind, trading_date, PositiveU32::new(1_000)?)
}

fn instrument_identity(instrument: &InstrumentId) -> String {
    let suffix = match instrument.exchange() {
        Exchange::Shanghai => "SH",
        Exchange::Shenzhen => "SZ",
        Exchange::Beijing => "BJ",
    };
    format!("{}.{suffix}", instrument.code())
}

fn print_batch<T: Debug>(label: &str, batch: &DataBatch<T>) {
    println!("\n=== {label} ===");
    println!("records={}", batch.records().len());
    println!("provenance={:#?}", batch.provenance());
    println!("quality={:#?}", batch.quality());
    for (index, record) in batch.records().iter().enumerate() {
        println!("record[{index}]={record:#?}");
    }
}

fn probe_batch<T: Debug, E: std::fmt::Display>(
    label: &str,
    result: Result<DataBatch<T>, E>,
    policy: &ProbeAdmissionPolicy,
    evidence_of: impl Fn(&T) -> &SourceEvidence,
    identity_of: impl Fn(&T) -> String,
    failures: &mut Vec<String>,
) {
    match result {
        Ok(batch) => match verify_admitted_batch(&batch, policy, evidence_of, identity_of) {
            Ok(status) => {
                println!("family={label} status={status}");
                print_batch(label, &batch);
            }
            Err(error) => {
                let failure = format!("{label}: admission rejected: {error}");
                println!("\n=== {label} ===");
                println!("family={label} status={}", ProbeStatus::Failed);
                println!("error={error}");
                failures.push(failure);
            }
        },
        Err(error) => {
            let failure = format!("{label}: {error}");
            println!("\n=== {label} ===");
            println!("family={label} status={}", ProbeStatus::Failed);
            println!("error={error}");
            failures.push(failure);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_limit_pool_probe_requests_the_complete_source_page() {
        let request = complete_limit_pool_request(
            LimitPoolKind::PreviousUpper,
            IsoDate::new("2026-07-23").unwrap(),
        )
        .unwrap();
        assert_eq!(request.limit().get(), 1_000);
    }
}
