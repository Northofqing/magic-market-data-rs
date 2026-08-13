use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError};
use magic_market_core::{
    verify_admitted_batch, verify_verified_empty, AssetClass, BlockTrades, BoardCategory,
    BoardFlows, DataBatch, DividendPlans, DragonTigerData, DragonTigerDiscovery,
    DragonTigerDiscoveryRequest, DragonTigerEntry, Exchange, FlowInterval, FlowScope,
    FundFlowRequest, FundFlowSeries, HolderCounts, InstrumentDateRangeRequest, InstrumentId,
    InstrumentSignalRequest, IsoDate, LimitPoolKind, LimitPoolRequest, LimitPools, LockupEvents,
    MarginData, MarketDragonTigerData, MarketDragonTigerRequest, MarketRankingEntry,
    MarketRankingKind, MarketRankings, NewsProvider, PopularityData, PositiveU32,
    PostCloseFlowRequest, ProbeAdmissionPolicy, ProbeStatus, ProviderId, ProviderTopNRankingEntry,
    ProviderTopNRankings, ReportScope, ResearchReports, ResearchRequest, SourceEvidence,
    TargetPriceConsensus, TargetPriceData, TargetPriceRequest,
};
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt::Debug;
use time::{OffsetDateTime, UtcOffset};

fn main() -> Result<(), Box<dyn Error>> {
    let client = EastmoneyClient::new()?;
    let mut failures = Vec::new();
    if std::env::var("MAGIC_EASTMONEY_LIVE_OPERATION").as_deref() == Ok("provider-topn-rankings") {
        let trading_date = IsoDate::new(required_env("MAGIC_EASTMONEY_TOPN_DATE")?)?;
        let limit = PositiveU32::new(env("MAGIC_EASTMONEY_TOPN_LIMIT", "20").parse::<u32>()?)?;
        let kinds = match env("MAGIC_EASTMONEY_RANKING_KIND", "all").as_str() {
            "all" => vec![
                MarketRankingKind::VolumeRatio,
                MarketRankingKind::MainNetInflow,
            ],
            "volume-ratio" => vec![MarketRankingKind::VolumeRatio],
            "main-net-inflow" => vec![MarketRankingKind::MainNetInflow],
            other => {
                return Err(format!(
                    "MAGIC_EASTMONEY_RANKING_KIND must be all, volume-ratio, or main-net-inflow; got {other:?}"
                )
                .into())
            }
        };
        for kind in kinds {
            let request = EastmoneyClient::provider_top_n_a_share_request(
                kind.clone(),
                trading_date.clone(),
                limit,
            )?;
            let acquisition_started_at = china_now()?;
            let result = client.provider_top_n_rankings(&request);
            probe_provider_top_n(&kind, &acquisition_started_at, result, &mut failures);
        }
        return print_summary(&failures);
    }
    if std::env::var("MAGIC_EASTMONEY_LIVE_OPERATION").as_deref() == Ok("market-rankings") {
        let limit = PositiveU32::new(20)?;
        let kinds = match env("MAGIC_EASTMONEY_RANKING_KIND", "all").as_str() {
            "all" => vec![
                MarketRankingKind::VolumeRatio,
                MarketRankingKind::MainNetInflow,
            ],
            "volume-ratio" => vec![MarketRankingKind::VolumeRatio],
            "main-net-inflow" => vec![MarketRankingKind::MainNetInflow],
            other => {
                return Err(format!(
                    "MAGIC_EASTMONEY_RANKING_KIND must be all, volume-ratio, or main-net-inflow; got {other:?}"
                )
                .into())
            }
        };
        for kind in kinds {
            probe_market_ranking(&kind, client.market_rankings(&kind, limit), &mut failures);
        }
        return print_summary(&failures);
    }
    if std::env::var("MAGIC_EASTMONEY_LIVE_OPERATION").as_deref() == Ok("target-price") {
        let target = instrument(
            Exchange::Shanghai,
            env("MAGIC_EASTMONEY_TARGET_CODE", "600519"),
        )?;
        let request = TargetPriceRequest::new(
            target,
            IsoDate::new(env("MAGIC_EASTMONEY_TARGET_FROM", "2026-01-01"))?,
            IsoDate::new(env("MAGIC_EASTMONEY_TARGET_THROUGH", "2026-07-27"))?,
        )?;
        probe_target_price(client.target_price_consensus(&request), &mut failures);
        return print_summary(&failures);
    }
    if std::env::var("MAGIC_EASTMONEY_LIVE_OPERATION").as_deref() == Ok("post-close-ranking") {
        let request = PostCloseFlowRequest::new(
            IsoDate::new(required_env("MAGIC_EASTMONEY_POST_CLOSE_DATE")?)?,
            PositiveU32::new(env("MAGIC_EASTMONEY_POST_CLOSE_LIMIT", "20").parse::<u32>()?)?,
        )?;
        probe_unadmitted_batch(
            "capital.post_close_ranking",
            client.diagnose_post_close_flows(&request),
        );
        println!("\n=== diagnostic_summary ===");
        println!("diagnostic_probe_status=unadmitted");
        return Ok(());
    }
    let pool_date = IsoDate::new(required_env("MAGIC_EASTMONEY_POOL_DATE")?)?;
    let dragon_tiger_date = IsoDate::new(required_env("MAGIC_EASTMONEY_DRAGON_TIGER_DATE")?)?;
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
        probe_limit_pool(
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

fn probe_limit_pool(
    label: &str,
    result: Result<DataBatch<magic_market_core::LimitPoolEntry>, EastmoneyError>,
    policy: &ProbeAdmissionPolicy,
    evidence_of: impl Fn(&magic_market_core::LimitPoolEntry) -> &SourceEvidence,
    identity_of: impl Fn(&magic_market_core::LimitPoolEntry) -> String,
    failures: &mut Vec<String>,
) {
    match result {
        Err(EastmoneyError::VerifiedEmpty(empty)) => match verify_verified_empty(&empty, policy) {
            Ok(status) => {
                println!("family={label} status={status}");
                println!("request_identity={}", empty.request_identity());
                println!("reason={}", empty.reason());
            }
            Err(error) => {
                let failure = format!("{label}: verified-empty admission rejected: {error}");
                println!("family={label} status={}", ProbeStatus::Failed);
                println!("error={error}");
                failures.push(failure);
            }
        },
        other => probe_batch(label, other, policy, evidence_of, identity_of, failures),
    }
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

fn probe_market_ranking<E: std::fmt::Display>(
    kind: &MarketRankingKind,
    result: Result<DataBatch<MarketRankingEntry>, E>,
    failures: &mut Vec<String>,
) {
    let label = format!("market_rankings.{kind:?}");
    println!("\n=== {label} ===");
    println!("admitted=false pending_current_live_review");
    match result {
        Ok(batch) => {
            println!("status={}", ProbeStatus::DiagnosticCompleteUnadmitted);
            println!("records={}", batch.records().len());
            if let Some(first) = batch.records().first() {
                println!("universe={}", first.universe());
                println!("universe_size={}", first.universe_size().get());
                println!("covered_count={}", first.covered_count().get());
                println!("coverage={}", first.coverage_ratio().get());
                println!("max_source_skew_millis={}", first.max_source_skew_millis());
                println!("source_date={}", first.source_date());
                println!("source_session={:?}", first.source_session());
            }
            for record in batch.records() {
                let code = record
                    .instrument()
                    .map(instrument_identity)
                    .unwrap_or_else(|| "<no-code>".into());
                println!(
                    "rank={} stock={} name={} value={} unit={:?} source_at={:?}",
                    record.rank().get(),
                    code,
                    record.label(),
                    record.value().get(),
                    record.unit(),
                    record.evidence().source_at()
                );
            }
        }
        Err(error) => {
            println!("status={}", ProbeStatus::Failed);
            println!("error={error}");
            failures.push(format!("{label}: {error}"));
        }
    }
}

fn probe_provider_top_n<E: std::fmt::Display>(
    kind: &MarketRankingKind,
    acquisition_started_at: &str,
    result: Result<DataBatch<ProviderTopNRankingEntry>, E>,
    failures: &mut Vec<String>,
) {
    let label = format!("provider_top_n_rankings.{kind:?}");
    println!("\n=== {label} ===");
    println!("capability_admitted=true");
    println!("acquisition_started_at={acquisition_started_at}");
    match result {
        Ok(batch) => {
            println!("status={}", ProbeStatus::Admitted);
            println!("records={}", batch.records().len());
            println!("batch_observed_at={}", batch.provenance().fetched_at());
            println!("batch_source_at={:?}", batch.provenance().source_at());
            if let Some(first) = batch.records().first() {
                println!(
                    "first_record_observed_at={} filter_identity={} provider_declared_total={} inspected_row_count={} latest_trading_date={}",
                    first.evidence().observed_at(),
                    first.filter_identity(),
                    first.provider_declared_total().get(),
                    first.inspected_row_count().get(),
                    first.latest_trading_date()
                );
            }
            for record in batch.records() {
                println!(
                    "source_order_ordinal={} stock={} name={} value={} unit={:?} latest_trading_date={} source_at={:?}",
                    record.source_order_ordinal().get(),
                    instrument_identity(record.instrument()),
                    record.label(),
                    record.value().get(),
                    record.unit(),
                    record.latest_trading_date(),
                    record.evidence().source_at()
                );
            }
        }
        Err(error) => {
            println!("status={}", ProbeStatus::Failed);
            println!("error={error}");
            failures.push(format!("{label}: {error}"));
        }
    }
}

fn china_now() -> Result<String, Box<dyn Error>> {
    let china_offset = UtcOffset::from_hms(8, 0, 0)?;
    Ok(OffsetDateTime::now_utc()
        .to_offset(china_offset)
        .format(&time::format_description::well_known::Rfc3339)?)
}

fn probe_target_price(
    result: Result<DataBatch<TargetPriceConsensus>, EastmoneyError>,
    failures: &mut Vec<String>,
) {
    println!("\n=== research.target_price ===");
    match result {
        Ok(batch) if batch.records().len() == 1 => {
            let value = &batch.records()[0];
            println!("status={}", ProbeStatus::Admitted);
            println!(
                "stock={} name={} samples={} contributors={} observation_period={}..{} low={} mean={} high={} mean_semantics=arithmetic_mean_of_report_range_midpoints source_at={:?} observed_at={} batch_id={} input_evidence={}",
                instrument_identity(value.instrument()),
                value.instrument_name(),
                value.sample_count().get(),
                value.contributor_count().get(),
                value.observation_start(),
                value.observation_end(),
                value.low().get(),
                value.mean().get(),
                value.high().get(),
                value.evidence().source_at(),
                value.evidence().observed_at(),
                value.evidence().batch_id(),
                value.input_evidence().len(),
            );
            for observation in value.observations() {
                println!(
                    "report={} institution={} published_on={} source_indvAimPriceT={} source_indvAimPriceL={} normalized_low={} normalized_high={}",
                    observation.report_id(),
                    observation.institution_name(),
                    observation.published_on(),
                    observation.source_indv_aim_price_t().get(),
                    observation.source_indv_aim_price_l().get(),
                    observation.normalized_low().get(),
                    observation.normalized_high().get(),
                );
            }
        }
        Ok(batch) => {
            let failure = format!(
                "research.target_price: expected one aggregate, received {}",
                batch.records().len()
            );
            println!("status={}", ProbeStatus::Failed);
            println!("error={failure}");
            failures.push(failure);
        }
        Err(EastmoneyError::VerifiedEmpty(empty)) => {
            match verify_verified_empty(&empty, &ProbeAdmissionPolicy::new(ProviderId::Eastmoney)) {
                Ok(status) => {
                    println!("status={status}");
                    println!("request_identity={}", empty.request_identity());
                    println!("reason={}", empty.reason());
                    println!("observed_at={}", empty.evidence().observed_at());
                    println!("batch_id={}", empty.evidence().batch_id());
                }
                Err(error) => {
                    println!("status={}", ProbeStatus::Failed);
                    println!("error={error}");
                    failures.push(format!(
                        "research.target_price: verified-empty admission rejected: {error}"
                    ));
                }
            }
        }
        Err(error) => {
            println!("status={}", ProbeStatus::Failed);
            println!("error={error}");
            failures.push(format!("research.target_price: {error}"));
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
