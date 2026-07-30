use magic_eastmoney_rs::EastmoneyClient;
use magic_market_composition::EastmoneyProviderTopNRankingRouter;
use magic_market_core::{
    IsoDate, MarketRankingKind, PositiveU32, ProviderId, ProviderTopNRankingRequest,
};
use std::env;
use std::error::Error;
use time::{OffsetDateTime, UtcOffset};

struct ProbePlan {
    trading_date: IsoDate,
    limit: PositiveU32,
}

impl ProbePlan {
    fn from_values(
        date: Option<&str>,
        limit: Option<&str>,
        kind: Option<&str>,
    ) -> Result<Self, Box<dyn Error>> {
        if kind.unwrap_or("all") != "all" {
            return Err("MAGIC_COMPOSITION_TOPN_KIND must be exactly all".into());
        }
        let trading_date = IsoDate::new(match date {
            Some(value) => value.to_owned(),
            None => current_china_date()?,
        })?;
        let limit = PositiveU32::new(limit.unwrap_or("20").parse::<u32>()?)?;
        if limit.get() > ProviderTopNRankingRequest::MAX_SINGLE_PAGE_LIMIT {
            return Err(format!(
                "MAGIC_COMPOSITION_TOPN_LIMIT {} exceeds the proved single-page cap {}",
                limit.get(),
                ProviderTopNRankingRequest::MAX_SINGLE_PAGE_LIMIT
            )
            .into());
        }
        Ok(Self {
            trading_date,
            limit,
        })
    }
}

fn current_china_date() -> Result<String, Box<dyn Error>> {
    let china_offset = UtcOffset::from_hms(8, 0, 0)?;
    Ok(OffsetDateTime::now_utc()
        .to_offset(china_offset)
        .date()
        .to_string())
}

fn probe_kinds() -> [MarketRankingKind; 2] {
    [
        MarketRankingKind::VolumeRatio,
        MarketRankingKind::MainNetInflow,
    ]
}

#[allow(clippy::too_many_arguments)]
fn format_success(
    kind: &MarketRankingKind,
    provider: ProviderId,
    source: &str,
    observed_at: &str,
    source_at: Option<&str>,
    records: usize,
    provider_declared_total: u32,
) -> String {
    format!(
        "kind={kind:?}\nstatus=admitted\nprovider={provider:?}\nsource={source}\n\
         observed_at={observed_at}\nsource_at={}\nrecords={records}\n\
         provider_declared_total={provider_declared_total}",
        source_at.unwrap_or("None")
    )
}

fn format_failure(
    kind: &MarketRankingKind,
    expected_provider: ProviderId,
    expected_source: &str,
    error: &dyn Error,
) -> String {
    format!(
        "kind={kind:?}\nstatus=failed\nexpected_provider={expected_provider:?}\n\
         expected_source={expected_source}\n\
         observed_at=None\nsource_at=None\nrecords=0\nprovider_declared_total=None\nerror={error}"
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let date = env::var("MAGIC_COMPOSITION_TOPN_DATE").ok();
    let limit = env::var("MAGIC_COMPOSITION_TOPN_LIMIT").ok();
    let kind = env::var("MAGIC_COMPOSITION_TOPN_KIND").ok();
    let plan = ProbePlan::from_values(date.as_deref(), limit.as_deref(), kind.as_deref())?;

    let router = EastmoneyProviderTopNRankingRouter::new()?;
    let provider = router
        .provider_ids()
        .first()
        .copied()
        .ok_or("production composition route has no provider")?;
    let source = router.expected_source().as_str();
    let mut failures = 0_usize;

    for kind in probe_kinds() {
        println!("\n=== composition_provider_top_n.{kind:?} ===");
        let request = EastmoneyClient::provider_top_n_a_share_request(
            kind.clone(),
            plan.trading_date.clone(),
            plan.limit,
        )?;
        match router.route(&request) {
            Ok(outcome) => {
                let batch = outcome.batch();
                if let Some(first) = batch.records().first() {
                    println!(
                        "{}",
                        format_success(
                            &kind,
                            outcome.selected_provider(),
                            batch.provenance().source(),
                            batch.provenance().fetched_at(),
                            batch.provenance().source_at(),
                            batch.records().len(),
                            first.provider_declared_total().get(),
                        )
                    );
                } else {
                    failures += 1;
                    let error = std::io::Error::other(
                        "production composition route admitted an empty Top-N batch",
                    );
                    println!("{}", format_failure(&kind, provider, source, &error));
                }
            }
            Err(error) => {
                failures += 1;
                println!("{}", format_failure(&kind, provider, source, &error));
            }
        }
    }

    println!("\nfailures={failures}");
    if failures > 0 {
        return Err(std::io::Error::other(format!(
            "production composition provider Top-N probe failed for {failures} metric(s)"
        ))
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_plan_accepts_only_the_bounded_all_metric_plan() {
        let plan = ProbePlan::from_values(Some("2026-07-29"), Some("20"), Some("all")).unwrap();
        assert_eq!(plan.trading_date.as_str(), "2026-07-29");
        assert_eq!(plan.limit.get(), 20);

        assert!(ProbePlan::from_values(Some("2026-07-29"), Some("101"), Some("all")).is_err());
        assert!(
            ProbePlan::from_values(Some("2026-07-29"), Some("20"), Some("volume-ratio")).is_err()
        );
    }

    #[test]
    fn probe_plan_rejects_invalid_date_and_non_positive_limit() {
        assert!(ProbePlan::from_values(Some("2026-02-30"), Some("20"), Some("all")).is_err());
        assert!(ProbePlan::from_values(Some("2026-07-29"), Some("0"), Some("all")).is_err());
    }

    #[test]
    fn probe_routes_each_admitted_metric_once_and_renders_audit_fields() {
        assert_eq!(
            probe_kinds(),
            [
                MarketRankingKind::VolumeRatio,
                MarketRankingKind::MainNetInflow,
            ]
        );

        let rendered = format_success(
            &MarketRankingKind::VolumeRatio,
            ProviderId::Eastmoney,
            "eastmoney-web",
            "2026-07-29T22:25:25+08:00",
            None,
            20,
            5_542,
        );
        for expected in [
            "status=admitted",
            "provider=Eastmoney",
            "source=eastmoney-web",
            "observed_at=2026-07-29T22:25:25+08:00",
            "source_at=None",
            "records=20",
            "provider_declared_total=5542",
        ] {
            assert!(rendered.contains(expected), "missing {expected:?}");
        }
    }

    #[test]
    fn failure_output_does_not_present_expected_identity_as_selected_evidence() {
        let error = std::io::Error::other("transport unavailable");
        let rendered = format_failure(
            &MarketRankingKind::MainNetInflow,
            ProviderId::Eastmoney,
            "eastmoney-web",
            &error,
        );
        for expected in [
            "status=failed",
            "expected_provider=Eastmoney",
            "expected_source=eastmoney-web",
            "observed_at=None",
            "source_at=None",
            "records=0",
            "provider_declared_total=None",
        ] {
            assert!(rendered.contains(expected), "missing {expected:?}");
        }
        assert!(!rendered.contains("\nprovider="));
        assert!(!rendered.contains("\nsource="));
    }
}
