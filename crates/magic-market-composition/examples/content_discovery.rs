//! Bounded candidate search over existing admitted news and Cninfo disclosures.
//!
//! Examples:
//! cargo run -p magic-market-composition --example content_discovery -- news "NVIDIA|英伟达" "Rubin|鲁宾"
//! cargo run -p magic-market-composition --example content_discovery -- issuer SH 600519 2025-01-01 2025-12-31 annual
//! cargo run -p magic-market-composition --example content_discovery -- market-day 2025-08-29 shareholder-reduction

use std::{env, error::Error, time::Duration};

use magic_market_composition::{
    discover_disclosures, production_operation_registry, search_recent_news, DisclosureKind,
    DisclosureQuery, DiscoveryStatus, EntityQuery, SourceOutcome,
};
use magic_market_core::{AssetClass, Exchange, InstrumentId, IsoDate};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mode = args.next().ok_or_else(usage)?;
    match mode.as_str() {
        "news" => {
            let groups = args
                .map(|group| group.split('|').map(str::to_owned).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            let query = EntityQuery::new(groups)?;
            let registry = production_operation_registry(Duration::from_secs(10), 4 * 1024 * 1024)?;
            let page = search_recent_news(&registry, &query)?;
            show_sources(&page.sources);
            eprintln!("Status: {:?}", page.status);
            if page.status == DiscoveryStatus::NoSourceSucceeded {
                return Err("all admitted news source windows failed".into());
            }
            for candidate in page.candidates {
                println!(
                    "{} | {} | {}",
                    candidate.provider, candidate.title, candidate.url
                );
            }
            eprintln!(
                "Scope: latest 20 records per inspected source; no historical-completeness claim."
            );
        }
        "issuer" => {
            let exchange = match args.next().as_deref() {
                Some("SH") => Exchange::Shanghai,
                Some("SZ") => Exchange::Shenzhen,
                Some("BJ") => Exchange::Beijing,
                _ => return Err(usage().into()),
            };
            let code = args.next().ok_or_else(usage)?;
            let start = IsoDate::new(args.next().ok_or_else(usage)?)?;
            let end = IsoDate::new(args.next().ok_or_else(usage)?)?;
            let kind = disclosure_kind(&args.next().ok_or_else(usage)?)?;
            if args.next().is_some() {
                return Err(usage().into());
            }
            let instrument = InstrumentId::new(exchange, code, AssetClass::Equity)?;
            let query = DisclosureQuery::instrument_range(instrument, start, end, kind)?;
            run_disclosures(&query)?;
        }
        "market-day" => {
            let day = IsoDate::new(args.next().ok_or_else(usage)?)?;
            let kind = disclosure_kind(&args.next().ok_or_else(usage)?)?;
            if args.next().is_some() {
                return Err(usage().into());
            }
            let query = DisclosureQuery::market_day(day, kind)?;
            run_disclosures(&query)?;
        }
        _ => return Err(usage().into()),
    }
    Ok(())
}

fn run_disclosures(query: &DisclosureQuery) -> Result<(), Box<dyn Error>> {
    let registry = production_operation_registry(Duration::from_secs(10), 4 * 1024 * 1024)?;
    let page = discover_disclosures(&registry, query)?;
    show_sources(&page.sources);
    for candidate in page.candidates {
        println!(
            "{:?} | {} | {}",
            candidate.kind, candidate.title, candidate.url
        );
    }
    eprintln!("Scope: bounded Cninfo source window; title labels are candidates, not verified event details.");
    Ok(())
}

fn show_sources(sources: &[SourceOutcome]) {
    for source in sources {
        match source {
            SourceOutcome::Inspected { provider, records } => {
                eprintln!("{provider}: inspected {records} source records");
            }
            SourceOutcome::Failed { provider, .. } => {
                eprintln!("{provider}: failed; no result from this source");
            }
        }
    }
}

fn disclosure_kind(value: &str) -> Result<DisclosureKind, String> {
    match value {
        "annual" => Ok(DisclosureKind::AnnualReport),
        "half-year" => Ok(DisclosureKind::HalfYearReport),
        "shareholder-reduction" => Ok(DisclosureKind::ShareholderReduction),
        "repurchased-share-reduction" => Ok(DisclosureKind::RepurchasedShareReduction),
        "earnings-forecast" => Ok(DisclosureKind::EarningsForecast),
        "shareholder-increase" => Ok(DisclosureKind::ShareholderIncrease),
        "equity-issuance" => Ok(DisclosureKind::EquityIssuance),
        "control-change" => Ok(DisclosureKind::ControlChange),
        "share-transfer" => Ok(DisclosureKind::ShareTransfer),
        "share-pledge" => Ok(DisclosureKind::SharePledge),
        "share-freeze" => Ok(DisclosureKind::ShareFreeze),
        "share-repurchase" => Ok(DisclosureKind::ShareRepurchase),
        "equity-incentive" => Ok(DisclosureKind::EquityIncentive),
        "employee-ownership" => Ok(DisclosureKind::EmployeeOwnership),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: content_discovery news ALIAS[|ALIAS]... | issuer SH|SZ|BJ CODE START END KIND | market-day DATE KIND; KIND: annual, half-year, earnings-forecast, shareholder-reduction, repurchased-share-reduction, shareholder-increase, equity-issuance, control-change, share-transfer, share-pledge, share-freeze, share-repurchase, equity-incentive, employee-ownership".into()
}
