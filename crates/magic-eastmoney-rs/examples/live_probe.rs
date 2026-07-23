use magic_eastmoney_rs::EastmoneyClient;
use magic_market_core::{
    AssetClass, BlockTrades, BoardCategory, BoardFlows, DataBatch, DividendPlans, DragonTigerData,
    Exchange, FlowInterval, FlowScope, FundFlowRequest, FundFlowSeries, HolderCounts,
    InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest, IsoDate, LimitPoolKind,
    LimitPoolRequest, LimitPools, LockupEvents, MarginData, NewsProvider, PopularityData,
    PositiveU32, ReportScope, ResearchReports, ResearchRequest,
};
use std::error::Error;
use std::fmt::Debug;

fn main() -> Result<(), Box<dyn Error>> {
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

    let report = ResearchRequest::new(
        ReportScope::Instrument(report_sample),
        PositiveU32::new(1)?,
        small,
    )?;
    probe_batch(
        "research.instrument",
        client.research_reports(&report),
        &mut failures,
    );
    let industry = ResearchRequest::new(
        ReportScope::Industry(magic_market_core::NonEmptyText::new(env(
            "MAGIC_EASTMONEY_INDUSTRY",
            "*",
        ))?),
        PositiveU32::new(1)?,
        small,
    )?;
    probe_batch(
        "research.industry",
        client.research_reports(&industry),
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
            &mut failures,
        );
    }

    let signal = InstrumentSignalRequest::new(event_sample.clone(), small)?;
    probe_batch(
        "dragon_tiger.entries",
        client.dragon_tiger_entries(&signal),
        &mut failures,
    );
    probe_batch(
        "dragon_tiger.seats",
        client.dragon_tiger_seats(&signal),
        &mut failures,
    );

    let reference_request = InstrumentDateRangeRequest::new(reference, small)?;
    probe_batch(
        "capital.margin",
        client.margin_data(&reference_request),
        &mut failures,
    );
    probe_batch(
        "capital.block_trades",
        client.block_trades(&reference_request),
        &mut failures,
    );
    probe_batch(
        "capital.holder_counts",
        client.holder_counts(&reference_request),
        &mut failures,
    );
    probe_batch(
        "capital.dividends",
        client.dividend_plans(&reference_request),
        &mut failures,
    );
    let event_request = InstrumentDateRangeRequest::new(event_sample, small)?;
    probe_batch(
        "capital.lockups",
        client.lockup_events(&event_request),
        &mut failures,
    );

    let pool_date = IsoDate::new(env("MAGIC_EASTMONEY_POOL_DATE", "2026-07-23"))?;
    for kind in [
        LimitPoolKind::Upper,
        LimitPoolKind::Broken,
        LimitPoolKind::Lower,
        LimitPoolKind::PreviousUpper,
    ] {
        let request = LimitPoolRequest::new(kind, pool_date.clone(), small)?;
        probe_batch(
            &format!("limit_pool.{kind:?}"),
            client.limit_pool(&request),
            &mut failures,
        );
    }

    probe_batch(
        "popularity",
        client.popularity(PositiveU32::new(5)?),
        &mut failures,
    );
    let news_request = InstrumentDateRangeRequest::new(primary, PositiveU32::new(5)?)?;
    probe_unadmitted_batch(
        "content.instrument_news",
        client.instrument_news(&news_request),
    );
    println!("\n=== probe_summary ===");
    println!("failures={}", failures.len());
    for failure in &failures {
        println!("{failure}");
    }
    if !failures.is_empty() {
        return Err(format!("{} live-probe families failed", failures.len()).into());
    }
    println!("live_probe_status=passed");
    Ok(())
}

fn probe_unadmitted_batch<T: Debug, E: std::fmt::Display>(
    label: &str,
    result: Result<DataBatch<T>, E>,
) {
    println!("\n=== {label} ===");
    println!("admitted=false");
    match result {
        Ok(batch) => {
            println!("diagnostic_status=success_not_admitted");
            print_batch(label, &batch);
        }
        Err(error) => {
            println!("diagnostic_status=expected_failure");
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
    failures: &mut Vec<String>,
) {
    match result {
        Ok(batch) => print_batch(label, &batch),
        Err(error) => {
            let failure = format!("{label}: {error}");
            println!("\n=== {label} ===");
            println!("error={error}");
            failures.push(failure);
        }
    }
}
