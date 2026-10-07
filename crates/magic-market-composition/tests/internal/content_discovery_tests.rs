use super::*;
use magic_market_core::{AssetClass, Exchange};
use magic_market_service::{Capability, QueryResult};
use std::collections::BTreeMap;

#[test]
fn disclosure_discovery_inspects_valid_incomplete_source_prefix() {
    let original = announcement("关于持股 5% 以上股东减持股份的公告");
    let mut prefix = result("Cninfo", vec![original.clone()]);
    prefix.complete = false;
    let gateway = FixtureGateway {
        capabilities: vec![capability(Operation::MarketAnnouncements, "Cninfo")],
        responses: BTreeMap::from([(
            (Operation::MarketAnnouncements, "Cninfo".into()),
            Ok(prefix),
        )]),
    };
    let query = DisclosureQuery::market_day(
        IsoDate::new("2026-09-30").unwrap(),
        DisclosureKind::ShareholderReduction,
    )
    .unwrap();

    let page = discover_disclosures(&gateway, &query).unwrap();
    assert_eq!(page.status, DiscoveryStatus::AllSourcesInspected);
    assert_eq!(page.candidates.len(), 1);
    assert_eq!(page.candidates[0].record, original);
    assert!(matches!(
        &page.sources[0],
        SourceOutcome::Inspected { provider, records: 1, source_complete: false }
            if provider == "Cninfo"
    ));
    assert!(matches!(
        page.scope,
        DiscoveryScope::AnnouncementRange {
            source_limit: 300,
            ..
        }
    ));
}

#[test]
fn ownership_candidate_kinds_do_not_claim_execution_or_confuse_actions() {
    for (title, expected) in [
        (
            "关于控股股东增持公司股份计划的公告",
            Some(DisclosureKind::ShareholderIncrease),
        ),
        (
            "关于董事增持公司股份计划实施完成的公告",
            Some(DisclosureKind::ShareholderIncrease),
        ),
        (
            "关于向特定对象发行股票获得注册批复的公告",
            Some(DisclosureKind::EquityIssuance),
        ),
        (
            "关于发行股份购买资产的预案",
            Some(DisclosureKind::EquityIssuance),
        ),
        (
            "关于控股股东变更的提示性公告",
            Some(DisclosureKind::ControlChange),
        ),
        (
            "关于公司控制权变更的公告",
            Some(DisclosureKind::ControlChange),
        ),
        (
            "关于股东股份协议转让的公告",
            Some(DisclosureKind::ShareTransfer),
        ),
        (
            "关于控股股东股份解除质押的公告",
            Some(DisclosureKind::SharePledge),
        ),
        (
            "关于控股股东所持股份解除冻结的公告",
            Some(DisclosureKind::ShareFreeze),
        ),
        (
            "关于回购公司股份方案的公告",
            Some(DisclosureKind::ShareRepurchase),
        ),
        (
            "关于限制性股票激励计划的公告",
            Some(DisclosureKind::EquityIncentive),
        ),
        (
            "关于员工持股计划的公告",
            Some(DisclosureKind::EmployeeOwnership),
        ),
        (
            "关于回购股份减持计划的公告",
            Some(DisclosureKind::RepurchasedShareReduction),
        ),
        ("关于回购注销限制性股票的公告", None),
        ("关于增持计划的公告", None),
        ("关于股东增持债券的公告", None),
        ("关于减持股份的公告", None),
        ("关于控制权变更事项不会发生变化的说明", None),
        ("关于股份协议转让暨控制权变更的公告", None),
        ("关于股东股份质押及司法冻结的公告", None),
    ] {
        assert_eq!(classify_disclosure(title), expected, "{title}");
    }
}

#[test]
fn disclosure_discovery_keeps_mixed_titles_unclassified() {
    let gateway = FixtureGateway {
        capabilities: vec![capability(Operation::MarketAnnouncements, "Cninfo")],
        responses: BTreeMap::from([(
            (Operation::MarketAnnouncements, "Cninfo".into()),
            Ok(result(
                "Cninfo",
                vec![
                    announcement("2025年年度报告及员工持股计划的公告"),
                    announcement("2026年半年度报告及股东减持股份计划的公告"),
                    announcement("2025年业绩预告及股东减持股份计划的公告"),
                    announcement("关于回购股份减持及员工持股计划的公告"),
                    announcement("2026年半年度报告及2025年年度报告的补充公告"),
                ],
            )),
        )]),
    };
    for kind in [
        DisclosureKind::AnnualReport,
        DisclosureKind::HalfYearReport,
        DisclosureKind::EarningsForecast,
        DisclosureKind::RepurchasedShareReduction,
        DisclosureKind::ShareholderReduction,
        DisclosureKind::EmployeeOwnership,
    ] {
        let query = DisclosureQuery::market_day(IsoDate::new("2026-09-30").unwrap(), kind).unwrap();
        let page = discover_disclosures(&gateway, &query).unwrap();
        assert!(
            page.candidates.is_empty(),
            "mixed title classified as {kind:?}"
        );
        assert!(matches!(
            page.sources[0],
            SourceOutcome::Inspected {
                records: 5,
                source_complete: true,
                ..
            }
        ));
    }
}

#[test]
fn disclosure_discovery_keeps_empty_window_source_quality_explicit() {
    for source_complete in [false, true] {
        let mut window = result("Cninfo", vec![]);
        window.complete = source_complete;
        let gateway = FixtureGateway {
            capabilities: vec![capability(Operation::MarketAnnouncements, "Cninfo")],
            responses: BTreeMap::from([(
                (Operation::MarketAnnouncements, "Cninfo".into()),
                Ok(window),
            )]),
        };
        let query = DisclosureQuery::market_day(
            IsoDate::new("2026-09-30").unwrap(),
            DisclosureKind::AnnualReport,
        )
        .unwrap();
        let page = discover_disclosures(&gateway, &query).unwrap();
        assert!(page.candidates.is_empty());
        assert_eq!(
            page.sources,
            vec![SourceOutcome::Inspected {
                provider: "Cninfo".into(),
                records: 0,
                source_complete,
            }]
        );
    }
}

#[test]
fn disclosure_discovery_inspects_incomplete_issuer_range() {
    let original = announcement("2025年半年度报告");
    let mut prefix = result("Cninfo", vec![original.clone()]);
    prefix.complete = false;
    let gateway = FixtureGateway {
        capabilities: vec![capability(Operation::Announcements, "Cninfo")],
        responses: BTreeMap::from([((Operation::Announcements, "Cninfo".into()), Ok(prefix))]),
    };
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let query = DisclosureQuery::instrument_range(
        instrument.clone(),
        IsoDate::new("2025-01-01").unwrap(),
        IsoDate::new("2025-12-31").unwrap(),
        DisclosureKind::HalfYearReport,
    )
    .unwrap();
    let page = discover_disclosures(&gateway, &query).unwrap();
    assert_eq!(page.candidates.len(), 1);
    assert_eq!(page.candidates[0].record, original);
    assert_eq!(
        page.scope,
        DiscoveryScope::InstrumentAnnouncementRange {
            instrument,
            start: IsoDate::new("2025-01-01").unwrap(),
            end: IsoDate::new("2025-12-31").unwrap(),
            source_limit: 200,
        }
    );
    assert!(matches!(
        page.sources[0],
        SourceOutcome::Inspected {
            records: 1,
            source_complete: false,
            ..
        }
    ));
}

#[test]
fn disclosure_discovery_rejects_conflicting_unadmitted_and_oversized_prefixes() {
    let original = announcement("2025年年度报告");
    let mut unadmitted = result("Cninfo", vec![original.clone()]);
    unadmitted.repository_admitted = false;
    let mut oversized = result("Cninfo", vec![original.clone(); 301]);
    oversized.complete = false;
    let query = DisclosureQuery::market_day(
        IsoDate::new("2026-09-30").unwrap(),
        DisclosureKind::AnnualReport,
    )
    .unwrap();
    for invalid in [result("cninfo", vec![original]), unadmitted, oversized] {
        let gateway = FixtureGateway {
            capabilities: vec![capability(Operation::MarketAnnouncements, "Cninfo")],
            responses: BTreeMap::from([(
                (Operation::MarketAnnouncements, "Cninfo".into()),
                Ok(invalid),
            )]),
        };
        assert_eq!(
            discover_disclosures(&gateway, &query),
            Err(ServiceError::FailedPrecondition(
                "announcement result lacks admitted bounded Cninfo identity".into()
            ))
        );
    }
}

#[test]
fn disclosure_discovery_rejects_malformed_incomplete_prefix_atomically() {
    let mut prefix = result(
        "Cninfo",
        vec![
            announcement("2025年年度报告"),
            record("wrong.schema", 1, json!({"title": "2025年年度报告"})),
        ],
    );
    prefix.complete = false;
    let gateway = FixtureGateway {
        capabilities: vec![capability(Operation::MarketAnnouncements, "Cninfo")],
        responses: BTreeMap::from([(
            (Operation::MarketAnnouncements, "Cninfo".into()),
            Ok(prefix),
        )]),
    };
    let query = DisclosureQuery::market_day(
        IsoDate::new("2026-09-30").unwrap(),
        DisclosureKind::AnnualReport,
    )
    .unwrap();
    assert_eq!(
        discover_disclosures(&gateway, &query),
        Err(ServiceError::FailedPrecondition(
            "content record schema does not match the admitted contract".into()
        ))
    );
}

#[test]
fn disclosure_discovery_keeps_typed_acquisition_failure_instead_of_empty_hits() {
    let expected = ServiceError::FailedPrecondition("native source acquisition failed".into());
    let gateway = FixtureGateway {
        capabilities: vec![capability(Operation::MarketAnnouncements, "Cninfo")],
        responses: BTreeMap::from([(
            (Operation::MarketAnnouncements, "Cninfo".into()),
            Err(expected.clone()),
        )]),
    };
    let query = DisclosureQuery::market_day(
        IsoDate::new("2026-09-30").unwrap(),
        DisclosureKind::AnnualReport,
    )
    .unwrap();
    assert_eq!(discover_disclosures(&gateway, &query), Err(expected));
}

#[test]
fn shareholder_increase_discovery_preserves_plan_and_result_originals() {
    let plan = announcement("关于控股股东增持公司股份计划的公告");
    let completed = announcement("关于控股股东增持公司股份计划完成的公告");
    let gateway = FixtureGateway {
        capabilities: vec![capability(Operation::MarketAnnouncements, "Cninfo")],
        responses: BTreeMap::from([(
            (Operation::MarketAnnouncements, "Cninfo".into()),
            Ok(result(
                "Cninfo",
                vec![
                    plan.clone(),
                    completed.clone(),
                    announcement("关于回购公司股份的公告"),
                ],
            )),
        )]),
    };
    let query = DisclosureQuery::market_day(
        IsoDate::new("2026-09-30").unwrap(),
        DisclosureKind::ShareholderIncrease,
    )
    .unwrap();
    let page = discover_disclosures(&gateway, &query).unwrap();
    assert_eq!(page.status, DiscoveryStatus::AllSourcesInspected);
    assert_eq!(page.candidates.len(), 2);
    assert_eq!(page.candidates[0].record, plan);
    assert_eq!(page.candidates[1].record, completed);
}

struct FixtureGateway {
    capabilities: Vec<Capability>,
    responses: BTreeMap<(Operation, String), Result<QueryResult, ServiceError>>,
}

impl BlockingQueryGateway for FixtureGateway {
    fn capabilities(&self) -> Vec<Capability> {
        self.capabilities.clone()
    }

    fn execute(&self, command: QueryCommand) -> Result<QueryResult, ServiceError> {
        self.responses
            .get(&(
                command.operation(),
                command.preferred_provider().unwrap_or_default().to_owned(),
            ))
            .expect("fixture response")
            .clone()
    }
}

fn capability(operation: Operation, provider: &str) -> Capability {
    Capability {
        operation,
        repository_admitted: true,
        runtime_available: true,
        provider: provider.into(),
        exact_scope: "fixture".into(),
        blocker: None,
        diagnostic_available: false,
    }
}

fn record(schema: &str, version: u32, value: Value) -> CanonicalPayload {
    CanonicalPayload::new(schema, version, serde_json::to_vec(&value).unwrap(), 4096).unwrap()
}

fn result(provider: &str, records: Vec<CanonicalPayload>) -> QueryResult {
    QueryResult {
        provider: provider.into(),
        batch_id: format!("{provider}-fixture"),
        complete: true,
        observed_at: "2026-09-25T00:00:00Z".into(),
        source_at: None,
        records,
        repository_admitted: true,
        diagnostic_blocker: None,
    }
}

fn news(title: &str) -> CanonicalPayload {
    record(
        GLOBAL_NEWS_RECORD_SCHEMA,
        NEWS_SCHEMA_VERSION,
        json!({
            "title": title,
            "summary": "",
            "content": null,
            "url": "https://example.test/news",
            "evidence": {"provider": "WallstreetCn", "batch_id": "fixture"}
        }),
    )
}

fn announcement(title: &str) -> CanonicalPayload {
    record(
        ANNOUNCEMENTS_RECORD_SCHEMA,
        SCHEMA_VERSION,
        json!({
            "title": title,
            "canonical_url": "https://www.cninfo.com.cn/announcement/1",
            "announcement_id": title,
            "evidence": {"provider": "Cninfo", "batch_id": "fixture"}
        }),
    )
}

#[test]
fn entity_groups_filter_bounded_news_and_preserve_source_failures() {
    let mut responses = BTreeMap::new();
    responses.insert(
        (Operation::GlobalNews, "WallstreetCn".into()),
        Ok(result(
            "WallstreetCn",
            vec![
                news("NVIDIA Vera Rubin 平台发布"),
                news("NVIDIA PCB 供应链新闻"),
                news("Meta 推出 Muse Spark 模型"),
                news("Meta 推出 Muse 个人智能体"),
            ],
        )),
    );
    responses.insert(
        (Operation::GlobalNews, "Eastmoney".into()),
        Err(ServiceError::FailedPrecondition(
            "unadmitted article host".into(),
        )),
    );
    let gateway = FixtureGateway {
        capabilities: vec![
            capability(Operation::GlobalNews, "Eastmoney"),
            capability(Operation::GlobalNews, "WallstreetCn"),
        ],
        responses,
    };
    let rubin = EntityQuery::new(vec![
        vec!["NVIDIA".into(), "英伟达".into()],
        vec!["Rubin".into(), "鲁宾".into()],
    ])
    .unwrap();
    let page = search_recent_news(&gateway, &rubin).unwrap();
    assert_eq!(page.candidates.len(), 1);
    assert_eq!(page.candidates[0].title, "NVIDIA Vera Rubin 平台发布");
    assert_eq!(
        page.candidates[0].record.schema(),
        GLOBAL_NEWS_RECORD_SCHEMA
    );
    assert!(matches!(
        page.scope,
        DiscoveryScope::LatestPerSource { limit: 20 }
    ));
    assert_eq!(page.status, DiscoveryStatus::PartialSourceFailure);
    assert!(matches!(page.sources[0], SourceOutcome::Inspected { .. }));
    assert!(matches!(page.sources[1], SourceOutcome::Failed { .. }));

    let muse_spark =
        EntityQuery::new(vec![vec!["Meta".into()], vec!["Muse Spark".into()]]).unwrap();
    let page = search_recent_news(&gateway, &muse_spark).unwrap();
    assert_eq!(page.candidates.len(), 1);
    assert_eq!(page.candidates[0].title, "Meta 推出 Muse Spark 模型");

    let muse_agent = EntityQuery::new(vec![
        vec!["Meta".into()],
        vec!["Muse".into()],
        vec!["个人智能体".into(), "agent".into()],
    ])
    .unwrap();
    let page = search_recent_news(&gateway, &muse_agent).unwrap();
    assert_eq!(page.candidates.len(), 1);
    assert_eq!(page.candidates[0].title, "Meta 推出 Muse 个人智能体");
}

#[test]
fn disclosure_candidates_keep_shareholder_and_repurchase_reductions_distinct() {
    let mut responses = BTreeMap::new();
    responses.insert(
        (Operation::MarketAnnouncements, "Cninfo".into()),
        Ok(result(
            "Cninfo",
            vec![
                announcement("2025 年年度报告"),
                announcement("2025 年半年度报告"),
                announcement("关于持股 5% 以上股东减持股份的公告"),
                announcement("关于回购股份减持的公告"),
                announcement("关于减持股份的公告"),
            ],
        )),
    );
    let gateway = FixtureGateway {
        capabilities: vec![capability(Operation::MarketAnnouncements, "Cninfo")],
        responses,
    };
    let day = IsoDate::new("2025-08-29").unwrap();
    for (kind, expected) in [
        (DisclosureKind::AnnualReport, "2025 年年度报告"),
        (DisclosureKind::HalfYearReport, "2025 年半年度报告"),
        (
            DisclosureKind::ShareholderReduction,
            "关于持股 5% 以上股东减持股份的公告",
        ),
        (
            DisclosureKind::RepurchasedShareReduction,
            "关于回购股份减持的公告",
        ),
    ] {
        let query = DisclosureQuery::market_day(day.clone(), kind).unwrap();
        let page = discover_disclosures(&gateway, &query).unwrap();
        assert_eq!(page.candidates.len(), 1);
        assert_eq!(page.candidates[0].title, expected);
        assert_eq!(
            page.candidates[0].record.schema(),
            ANNOUNCEMENTS_RECORD_SCHEMA
        );
    }
    assert_eq!(classify_disclosure("关于减持股份的公告"), None);
}

#[test]
fn issuer_discovery_uses_instrument_announcements_for_a_bounded_year() {
    let mut responses = BTreeMap::new();
    responses.insert(
        (Operation::Announcements, "Cninfo".into()),
        Ok(result("Cninfo", vec![announcement("2025 年年度报告")])),
    );
    let gateway = FixtureGateway {
        capabilities: vec![capability(Operation::Announcements, "Cninfo")],
        responses,
    };
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let query = DisclosureQuery::instrument_range(
        instrument.clone(),
        IsoDate::new("2025-01-01").unwrap(),
        IsoDate::new("2025-12-31").unwrap(),
        DisclosureKind::AnnualReport,
    )
    .unwrap();
    let page = discover_disclosures(&gateway, &query).unwrap();
    assert_eq!(page.candidates.len(), 1);
    assert!(matches!(
        page.scope,
        DiscoveryScope::InstrumentAnnouncementRange {
            source_limit: 200,
            ..
        }
    ));
}

#[test]
fn disclosure_titles_do_not_turn_denials_or_dividend_record_dates_into_events() {
    for title in [
        "关于本次交易不会导致实际控制人变更的公告",
        "关于本次交易不导致控制权变更的公告",
        "关于2025年度利润分配股权登记日的公告",
        "关于分配股票股利的公告",
    ] {
        assert_eq!(classify_disclosure(title), None, "{title}");
    }
    assert_eq!(
        classify_disclosure("2026年配股发行方案"),
        Some(DisclosureKind::EquityIssuance)
    );
}

#[test]
fn discovery_rejects_invalid_query_and_malformed_source_without_partial_hits() {
    assert!(EntityQuery::new(vec![]).is_err());
    assert!(EntityQuery::new(vec![vec![" ".into()]]).is_err());
    assert!(!contains_bounded_alias("metadata update", "meta"));
    assert!(contains_bounded_alias("meta muse spark", "meta"));

    let mut responses = BTreeMap::new();
    responses.insert(
        (Operation::GlobalNews, "WallstreetCn".into()),
        Ok(result(
            "WallstreetCn",
            vec![
                news("NVIDIA Rubin"),
                record("wrong.schema", 2, json!({"title":"NVIDIA Rubin"})),
            ],
        )),
    );
    let gateway = FixtureGateway {
        capabilities: vec![capability(Operation::GlobalNews, "WallstreetCn")],
        responses,
    };
    let query = EntityQuery::new(vec![vec!["Rubin".into()]]).unwrap();
    let page = search_recent_news(&gateway, &query).unwrap();
    assert!(page.candidates.is_empty());
    assert_eq!(page.status, DiscoveryStatus::NoSourceSucceeded);
    assert!(matches!(page.sources[0], SourceOutcome::Failed { .. }));
}
