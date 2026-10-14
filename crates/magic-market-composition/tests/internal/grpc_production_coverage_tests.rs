use magic_eastmoney_rs::EastmoneyTransport;
use magic_market_core::{AssetClass, Exchange, RatioUnit};

use super::*;

#[path = "grpc_eastmoney_capital_query_tests.rs"]
mod eastmoney_capital_public_contract;

#[path = "grpc_eastmoney_lockup_units_query_tests.rs"]
mod eastmoney_lockup_units_public_contract;

#[path = "grpc_eastmoney_board_flow_query_tests.rs"]
mod eastmoney_board_flow_public_contract;

#[path = "grpc_eastmoney_holder_count_query_tests.rs"]
mod eastmoney_holder_count_public_contract;

#[path = "grpc_current_auction_query_tests.rs"]
mod current_auction_public_contract;

#[path = "grpc_eastmoney_dragon_tiger_entries_query_tests.rs"]
mod dragon_tiger_entries_public_contract;

#[path = "grpc_worldbank_metadata_query_tests.rs"]
mod worldbank_metadata_public_contract;

#[path = "grpc_eastmoney_market_rankings_query_tests.rs"]
mod eastmoney_market_rankings_public_contract;

// Synthetic fixed-source-time rows use the documented f51..f57 minute field
// order already exercised by the provider's fund_flow_tests.rs. The expected
// CNY values below are independent literals, not computed from parser output.
// These fixtures are not live admission evidence or proof of consumer delivery.
const COMPLETE_MINUTE_FLOW: &[u8] = br#"{"rc":0,"data":{"code":"600396","market":1,"klines":[
    "2026-07-23 14:59,10.25,-1,2,3,6,0.5",
    "2026-07-23 15:00,100.5,-10,20,30,60,1.25"
]}}"#;

struct MinuteFundFlowFixture(&'static [u8]);

impl EastmoneyTransport for MinuteFundFlowFixture {
    fn get(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        _maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        if !url.starts_with("https://push2.eastmoney.com/api/qt/stock/fflow/kline/get?") {
            return Err(EastmoneyError::Protocol(
                "minute-flow fixture does not serve a daily or unrelated endpoint".into(),
            ));
        }
        Ok(self.0.to_vec())
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Protocol(
            "minute-flow fixture does not serve POST requests".into(),
        ))
    }
}

fn minute_flow_registry(body: &'static [u8]) -> OperationRegistry {
    let client = EastmoneyClient::with_transport(MinuteFundFlowFixture(body));
    let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
    // Execute the actual production registration; only its external HTTP
    // transport is replaced. The fixture is not a live admission witness.
    register_eastmoney_client(&mut registry, client, 4096).unwrap();
    registry
}

fn minute_flow_command() -> QueryCommand {
    let request = FundFlowRequest::new(
        FlowScope::Instrument(
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
        ),
        FlowInterval::Minute1,
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    QueryCommand::new(
        "offline-minute-flow-coverage",
        Operation::FundFlowSeries,
        Some("Eastmoney".into()),
        CanonicalPayload::new(
            FUND_FLOW_SERIES_REQUEST_SCHEMA,
            1,
            serde_json::to_vec(&request).unwrap(),
            4096,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn registered_minute_fund_flow_preserves_series_identity_tiers_and_source_times() {
    let result = minute_flow_registry(COMPLETE_MINUTE_FLOW)
        .execute(minute_flow_command())
        .unwrap();

    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.provider, "Eastmoney");
    assert_eq!(
        result.source_at.as_deref(),
        Some("2026-07-23T15:00:00+08:00")
    );
    assert_eq!(result.records.len(), 2);
    let points = result
        .records
        .iter()
        .map(|payload| {
            assert_eq!(payload.schema(), "magic.market.fund_flow_point");
            assert_eq!(payload.schema_version(), 1);
            serde_json::from_slice::<FundFlowPoint>(payload.data()).unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(points[0].period_at.as_str(), "2026-07-23 14:59");
    assert_eq!(points[0].main_net.unwrap().get(), 10.25);
    assert_eq!(
        points[0].evidence.source_at(),
        Some("2026-07-23T14:59:00+08:00")
    );
    let last = &points[1];
    assert_eq!(last.period_at.as_str(), "2026-07-23 15:00");
    assert_eq!(last.main_net.unwrap().get(), 100.5);
    assert_eq!(last.small_net.unwrap().get(), -10.0);
    assert_eq!(last.medium_net.unwrap().get(), 20.0);
    assert_eq!(last.large_net.unwrap().get(), 30.0);
    assert_eq!(last.super_large_net.unwrap().get(), 60.0);
    assert_eq!(last.main_ratio.unwrap().get(), 1.25);
    assert_eq!(last.main_ratio.unwrap().unit(), RatioUnit::Percent);
    for point in &points {
        assert_eq!(point.interval, FlowInterval::Minute1);
        assert_eq!(
            point.scope,
            FlowScope::Instrument(
                InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
            )
        );
        assert_eq!(point.evidence.provider(), ProviderId::Eastmoney);
        assert_eq!(point.evidence.batch_id(), result.batch_id);
        assert_eq!(point.evidence.observed_at(), result.observed_at);
    }
}

#[test]
fn registered_minute_fund_flow_rejects_a_missing_second_row_tier_atomically() {
    // The first row is valid. The second omits the super-large tier entirely,
    // so the caller must not receive the first row as partial complete data.
    const MISSING_TIER: &[u8] = br#"{"rc":0,"data":{"code":"600396","market":1,"klines":[
        "2026-07-23 14:59,10.25,-1,2,3,6,0.5",
        "2026-07-23 15:00,100.5,-10,20,30"
    ]}}"#;

    let error = minute_flow_registry(MISSING_TIER)
        .execute(minute_flow_command())
        .unwrap_err();
    assert!(
        matches!(error, ServiceError::FailedPrecondition(ref message)
            if message.contains("fund-flow row has 5 fields, expected at least 6")),
        "a malformed second source row must reject the entire public query: {error:?}"
    );
}

mod latest_day_money_flow_release {
    use super::*;
    use magic_market_core::EvidenceTimestamp;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Synthetic one-row daily response follows the f51..f65 source schema of
    // fund_flow_day1_native.jsonp. The admitted latest-day query requests lmt=1;
    // do not feed its two-row capture to that bounded query. This is offline
    // parser/projection evidence, not a live capture or admission witness.
    const LATEST_DAY_FLOW: &[u8] = br#"emProbe({"rc":0,"data":{"code":"300005","market":0,"klines":["2026-09-30,4930843.0,-67617987.0,62687144.0,176755.0,4754088.0,1.66,-22.78,21.12,0.06,1.60,10.70,0.75,0.00,0.00"]}});"#;

    struct LatestDailyFundFlowFixture;

    impl EastmoneyTransport for LatestDailyFundFlowFixture {
        fn get(
            &self,
            url: &str,
            _headers: &[(&str, &str)],
            maximum_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            let Some((endpoint, query)) = url.split_once('?') else {
                return Err(EastmoneyError::Protocol(
                    "daily-flow fixture requires a bounded daily query".into(),
                ));
            };
            if endpoint != "https://push2his.eastmoney.com/api/qt/stock/fflow/daykline/get" {
                return Err(EastmoneyError::Protocol(
                    "daily-flow fixture does not serve minute or unrelated endpoints".into(),
                ));
            }
            // Validate the external request scope without depending on query
            // parameter order or mocking the client/parser/projector internals.
            for (name, expected) in [
                ("secid", "0.300005"),
                ("klt", "101"),
                ("lmt", "1"),
                ("cb", "emProbe"),
            ] {
                let mut values = query
                    .split('&')
                    .filter_map(|entry| entry.split_once('='))
                    .filter(|(key, _)| *key == name)
                    .map(|(_, value)| value);
                if values.next() != Some(expected) || values.next().is_some() {
                    return Err(EastmoneyError::Protocol(format!(
                        "daily-flow fixture requires exactly one {name}={expected}"
                    )));
                }
            }
            if LATEST_DAY_FLOW.len() > maximum_bytes {
                return Err(EastmoneyError::Protocol(
                    "daily-flow fixture exceeds the supplied body limit".into(),
                ));
            }
            Ok(LATEST_DAY_FLOW.to_vec())
        }

        fn post_json(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _body: &[u8],
            _maximum_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            Err(EastmoneyError::Protocol(
                "daily-flow fixture does not serve POST requests".into(),
            ))
        }
    }

    #[test]
    fn registered_latest_daily_flow_preserves_source_evidence() {
        let client = EastmoneyClient::with_transport(LatestDailyFundFlowFixture);
        let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
        // Use the actual production registration with an external transport
        // fixture; the local registry does not change source admission.
        register_eastmoney_client(&mut registry, client, 4096).unwrap();
        let command = QueryCommand::new(
            "offline-latest-daily-flow-coverage",
            Operation::MoneyFlows,
            Some("Eastmoney".into()),
            CanonicalPayload::new(
                "magic.market.money_flows.request",
                1,
                serde_json::to_vec(&serde_json::json!({
                    "instruments": [{
                        "exchange": "Shenzhen",
                        "code": "300005",
                        "asset_class": "Equity"
                    }]
                }))
                .unwrap(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();

        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let result = registry.execute(command).unwrap();
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        assert!(result.repository_admitted);
        assert!(result.complete);
        assert_eq!(result.provider, "Eastmoney");
        assert_eq!(result.diagnostic_blocker, None);
        assert_eq!(result.source_at.as_deref(), Some("2026-09-30"));
        assert_eq!(result.records.len(), 1);
        assert!(!result.batch_id.trim().is_empty());
        assert!(EvidenceTimestamp::parse_instant(&result.observed_at).is_ok());
        let observed_millis = result
            .observed_at
            .strip_prefix("unix-ms:")
            .unwrap()
            .parse::<u128>()
            .unwrap();
        assert!(before <= observed_millis && observed_millis <= after);

        let payload = &result.records[0];
        assert_eq!(payload.schema(), "magic.market.money_flow");
        assert_eq!(payload.schema_version(), 1);
        let wire: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        // Source fields are independent literals. Only the runtime-generated
        // observation and opaque batch ID use the separately checked envelope.
        assert_eq!(
            wire,
            serde_json::json!({
                "instrument": {
                    "exchange": "Shenzhen",
                    "code": "300005",
                    "asset_class": "Equity"
                },
                "main_net": 4_930_843.0,
                "super_large_net": 4_754_088.0,
                "large_net": 176_755.0,
                "medium_net": 62_687_144.0,
                "small_net": -67_617_987.0,
                "status": "Available",
                "source_at": "2026-09-30",
                "observed_at": result.observed_at.as_str(),
                "provider": "Eastmoney",
                "batch_id": result.batch_id.as_str()
            })
        );
        let flow: MoneyFlow = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(
            flow.instrument(),
            &InstrumentId::new(Exchange::Shenzhen, "300005", AssetClass::Equity).unwrap()
        );
        assert_eq!(flow.main_net().unwrap().get(), 4_930_843.0);
        assert_eq!(flow.super_large_net().unwrap().get(), 4_754_088.0);
        assert_eq!(flow.large_net().unwrap().get(), 176_755.0);
        assert_eq!(flow.medium_net().unwrap().get(), 62_687_144.0);
        assert_eq!(flow.small_net().unwrap().get(), -67_617_987.0);
        assert_eq!(flow.status(), DataStatus::Available);
        assert_eq!(flow.provider(), ProviderId::Eastmoney);
        assert_eq!(flow.source_at(), Some("2026-09-30"));
        assert!(EvidenceTimestamp::parse_instant(flow.source_at().unwrap()).is_err());
        assert_eq!(flow.observed_at(), result.observed_at);
        assert_eq!(flow.batch_id(), result.batch_id);
    }
}

mod hithink_financial_v2_release {
    use super::*;
    use magic_hithink_rs::HithinkClient;
    use magic_market_core::{EvidenceTimestamp, FinancialStatement, StatementKind};
    use magic_market_transport::{
        HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
    };
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Synthetic source shape, not live admission evidence. H1 remains the
    // source fiscal label, not a claim about cumulative or single-quarter data.
    const HALF_YEAR_CASH_FLOW: &[u8] = br#"{
        "code": 0,
        "message": "success",
        "request_id": "offline-financial-v2",
        "data": {
            "timestamp": 1782748800000,
            "item": [{
                "thscode": "600519.SH",
                "ticker": "600519",
                "period": "quarterly",
                "period_end_ms": 1782748800000,
                "report_date_ms": 1787587200000,
                "fiscal_year": 2026,
                "fiscal_period": "H1",
                "currency": "CNY",
                "act_cash_flow_net": 0.0,
                "invest_cash_flow_net": null,
                "financing_cash_flow_net": -42.5,
                "cash_equivalents_net_addition": 12.75,
                "pay_dividends_profits_interest_cash": 100.0,
                "pay_fixed_assets_etc_cash": null
            }]
        }
    }"#;

    struct HalfYearFinancialFixture;

    impl HttpTransport for HalfYearFinancialFixture {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            if request.method() != HttpMethod::Get || !request.body().is_empty() {
                return Err(TransportError::InvalidRequest(
                    "financial fixture serves only bodyless GET requests".into(),
                ));
            }
            let Some((endpoint, query)) = request.url().split_once('?') else {
                return Err(TransportError::InvalidRequest(
                    "financial fixture requires a bounded source query".into(),
                ));
            };
            if endpoint != "https://fuyao.aicubes.cn/api/a-share/financials/cash-flow-statements" {
                return Err(TransportError::InvalidRequest(
                    "financial fixture does not serve unrelated source endpoints".into(),
                ));
            }
            for (name, expected) in [
                ("thscode", "600519.SH"),
                ("period", "quarterly"),
                ("limit", "20"),
            ] {
                let mut values = query
                    .split('&')
                    .filter_map(|entry| entry.split_once('='))
                    .filter(|(key, _)| *key == name)
                    .map(|(_, value)| value);
                if values.next() != Some(expected) || values.next().is_some() {
                    return Err(TransportError::InvalidRequest(format!(
                        "financial fixture requires exactly one {name}={expected}"
                    )));
                }
            }
            Ok(HttpResponse::new(
                200,
                request.url(),
                Some("application/json".into()),
                HALF_YEAR_CASH_FLOW.to_vec(),
            ))
        }
    }

    #[test]
    fn registered_hithink_v2_preserves_report_evidence() {
        // Non-secret placeholder for an offline transport; no environment key
        // is read and no network client is started.
        let client =
            HithinkClient::with_transport("offline-fixture-key", HalfYearFinancialFixture).unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
        register_hithink_client(&mut registry, Arc::new(client), 16384).unwrap();
        let command = QueryCommand::new(
            "offline-hithink-financial-v2-coverage",
            Operation::FinancialStatements,
            Some("HithinkFinance".into()),
            CanonicalPayload::new(
                "magic.market.financial_statements.request",
                2,
                serde_json::to_vec(&serde_json::json!({
                    "instruments": [{
                        "exchange": "Shanghai",
                        "code": "600519",
                        "asset_class": "Equity"
                    }],
                    "kind": "CashFlow"
                }))
                .unwrap(),
                16384,
            )
            .unwrap(),
        )
        .unwrap();

        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let result = registry.execute(command).unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let observed = EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
        let lower = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            before.as_secs(),
            before.subsec_nanos()
        ))
        .unwrap();
        let upper = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            after.as_secs(),
            after.subsec_nanos()
        ))
        .unwrap();
        assert!(lower <= observed && observed <= upper);

        assert!(result.repository_admitted);
        assert!(result.complete);
        assert_eq!(result.provider, "HithinkFinance");
        assert_eq!(result.diagnostic_blocker, None);
        assert_eq!(result.batch_id, "hithink-financials:offline-financial-v2");
        assert_eq!(result.source_at.as_deref(), Some("unix-ms:1787587200000"));
        assert_eq!(result.records.len(), 1);
        let payload = &result.records[0];
        assert_eq!(payload.schema(), "magic.market.financial_statement");
        assert_eq!(payload.schema_version(), 2);
        let wire: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        // Source-derived fields use independent literals, including zero/null.
        // Only the independently bounded runtime observation is dynamic.
        assert_eq!(
            wire,
            serde_json::json!({
                "instrument": {
                    "exchange": "Shanghai",
                    "code": "600519",
                    "asset_class": "Equity"
                },
                "kind": "CashFlow",
                "report_period": "2026-06-30",
                "fiscal_period": "H1",
                "announced_on": "2026-08-25",
                "currency": "CNY",
                "lines": [
                    {"key": "act_cash_flow_net", "source_label": "act_cash_flow_net", "value": 0.0, "unit": "CNY"},
                    {"key": "invest_cash_flow_net", "source_label": "invest_cash_flow_net", "value": null, "unit": "CNY"},
                    {"key": "financing_cash_flow_net", "source_label": "financing_cash_flow_net", "value": -42.5, "unit": "CNY"},
                    {"key": "cash_equivalents_net_addition", "source_label": "cash_equivalents_net_addition", "value": 12.75, "unit": "CNY"},
                    {"key": "pay_dividends_profits_interest_cash", "source_label": "pay_dividends_profits_interest_cash", "value": 100.0, "unit": "CNY"},
                    {"key": "pay_fixed_assets_etc_cash", "source_label": "pay_fixed_assets_etc_cash", "value": null, "unit": "CNY"}
                ],
                "evidence": {
                    "provider": "Tonghuashun",
                    "source_at": "unix-ms:1787587200000",
                    "observed_at": result.observed_at.as_str(),
                    "batch_id": "hithink-financials:offline-financial-v2"
                }
            })
        );
        let statement: FinancialStatement = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(statement.kind, StatementKind::CashFlow);
        assert_eq!(statement.report_period.as_str(), "2026-06-30");
        assert_eq!(statement.fiscal_period.as_ref().unwrap().as_str(), "H1");
        assert_eq!(
            statement.announced_on.as_ref().unwrap().as_str(),
            "2026-08-25"
        );
        assert_eq!(statement.currency.as_ref().unwrap().as_str(), "CNY");
        assert_eq!(statement.lines[0].value.unwrap().get(), 0.0);
        assert_eq!(statement.lines[1].value, None);
        assert_eq!(statement.evidence.provider(), ProviderId::Tonghuashun);
        assert_eq!(
            statement.evidence.source_at(),
            Some("unix-ms:1787587200000")
        );
        assert_eq!(
            statement.evidence.batch_id(),
            "hithink-financials:offline-financial-v2"
        );
        assert_eq!(statement.evidence.batch_id(), result.batch_id);
        assert_eq!(statement.evidence.observed_at(), result.observed_at);
    }
}

mod fred_date_only_schedule_release {
    use super::*;
    use magic_fred_rs::FredClient;
    use magic_market_core::{
        EconomicReleaseScheduleEntry, EconomicReleaseScheduleProvider,
        EconomicReleaseScheduleRequest, EvidenceTimestamp, IsoDate, PositiveU32,
    };
    use magic_market_transport::{
        HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
    };
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Synthetic native-envelope sample, not an actual release announcement,
    // live API capture or admission witness. Last-updated metadata is more
    // precise than the release date but is not its publication timestamp.
    const DATE_ONLY_SCHEDULE: &[u8] = br#"{
        "realtime_start": "2026-09-01",
        "realtime_end": "2026-09-30",
        "order_by": "release_date",
        "sort_order": "asc",
        "count": 1,
        "offset": 0,
        "limit": 1000,
        "release_dates": [{
            "release_id": 50,
            "release_name": "Offline date-only release",
            "date": "2026-09-04",
            "release_last_updated": "2026-09-01T13:30:00Z"
        }]
    }"#;

    struct DateOnlyScheduleFixture;

    impl HttpTransport for DateOnlyScheduleFixture {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            if request.method() != HttpMethod::Get || !request.body().is_empty() {
                return Err(TransportError::InvalidRequest(
                    "schedule fixture serves only bodyless GET requests".into(),
                ));
            }
            let Some((endpoint, query)) = request.url().split_once('?') else {
                return Err(TransportError::InvalidRequest(
                    "schedule fixture requires a bounded source query".into(),
                ));
            };
            if endpoint != "https://api.stlouisfed.org/fred/releases/dates" {
                return Err(TransportError::InvalidRequest(
                    "schedule fixture does not serve unrelated source endpoints".into(),
                ));
            }
            for (name, expected) in [
                ("file_type", "json"),
                ("realtime_start", "2026-09-01"),
                ("realtime_end", "2026-09-30"),
                ("limit", "1000"),
                ("offset", "0"),
                ("order_by", "release_date"),
                ("sort_order", "asc"),
                ("include_release_dates_with_no_data", "true"),
            ] {
                let mut values = query
                    .split('&')
                    .filter_map(|entry| entry.split_once('='))
                    .filter(|(key, _)| *key == name)
                    .map(|(_, value)| value);
                if values.next() != Some(expected) || values.next().is_some() {
                    return Err(TransportError::InvalidRequest(format!(
                        "schedule fixture requires exactly one {name}={expected}"
                    )));
                }
            }
            // Real Fred source handling still enforces EndpointPolicy URL,
            // MIME/body bounds, duplicate JSON keys and pagination identity.
            Ok(HttpResponse::new(
                200,
                request.url(),
                Some("application/json".into()),
                DATE_ONLY_SCHEDULE.to_vec(),
            ))
        }
    }

    #[test]
    fn registered_fred_schedule_keeps_release_dates_date_only() {
        // Non-secret placeholder only; no environment key or network transport.
        let client =
            FredClient::with_transport("offline-fixture-key", Arc::new(DateOnlyScheduleFixture))
                .unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
        // Existing production handler body and scope, with only the external
        // transport replaced. No registrar, hook or admission change is added.
        registry
            .register_handler(
                admitted(
                    Operation::EconomicReleaseSchedule,
                    "Fred",
                    "official FRED release dates for an inclusive range of at most 366 days; date-only schedule with no fabricated source_at",
                ),
                move |command| {
                    execute_typed(
                        command,
                        ECONOMIC_RELEASE_SCHEDULE_REQUEST_SCHEMA,
                        ECONOMIC_RELEASE_SCHEDULE_RECORD_SCHEMA,
                        "Fred",
                        4096,
                        |request: &EconomicReleaseScheduleRequest| {
                            client.economic_release_schedule(request)
                        },
                    )
                },
            )
            .unwrap();
        let request = EconomicReleaseScheduleRequest::new(
            IsoDate::new("2026-09-01").unwrap(),
            IsoDate::new("2026-09-30").unwrap(),
            PositiveU32::new(1).unwrap(),
        )
        .unwrap();
        let command = QueryCommand::new(
            "offline-fred-date-only-schedule-coverage",
            Operation::EconomicReleaseSchedule,
            Some("Fred".into()),
            CanonicalPayload::new(
                "magic.market.economic_release_schedule.request",
                1,
                serde_json::to_vec(&request).unwrap(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();

        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let result = registry.execute(command).unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let observed = EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
        let lower = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            before.as_secs(),
            before.subsec_nanos()
        ))
        .unwrap();
        let upper = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            after.as_secs(),
            after.subsec_nanos()
        ))
        .unwrap();
        assert!(lower <= observed && observed <= upper);

        assert!(result.repository_admitted);
        assert!(result.complete);
        assert_eq!(result.provider, "Fred");
        assert_eq!(result.diagnostic_blocker, None);
        assert_eq!(result.source_at, None);
        assert!(!result.batch_id.trim().is_empty());
        assert_eq!(result.records.len(), 1);
        let payload = &result.records[0];
        assert_eq!(
            payload.schema(),
            "magic.market.economic_release_schedule_entry"
        );
        assert_eq!(payload.schema_version(), 1);
        let wire: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        // Fixed source fields and absence of source_at are literal oracles.
        // Runtime observation is independently bounded; opaque batch identity
        // is checked for nonemptiness and record/query consistency, not format.
        assert_eq!(
            wire,
            serde_json::json!({
                "release_id": 50,
                "release_name": "Offline date-only release",
                "release_date": "2026-09-04",
                "release_last_updated": "2026-09-01T13:30:00Z",
                "evidence": {
                    "provider": "Fred",
                    "source_at": null,
                    "observed_at": result.observed_at.as_str(),
                    "batch_id": result.batch_id.as_str()
                }
            })
        );
        let entry: EconomicReleaseScheduleEntry = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(entry.release_id().get(), 50);
        assert_eq!(entry.release_name().as_str(), "Offline date-only release");
        assert_eq!(entry.release_date().as_str(), "2026-09-04");
        assert!(EvidenceTimestamp::parse_instant(entry.release_date().as_str()).is_err());
        assert_eq!(
            entry.release_last_updated().unwrap().as_str(),
            "2026-09-01T13:30:00Z"
        );
        assert_eq!(entry.evidence().provider(), ProviderId::Fred);
        assert_eq!(entry.evidence().source_at(), None);
        assert_eq!(entry.evidence().observed_at(), result.observed_at);
        assert_eq!(entry.evidence().batch_id(), result.batch_id);
    }
}

// Prepare-only. Append this complete nested module to the existing internal
// grpc_production_coverage_tests.rs after root review. Do not include! this file:
// fixture paths intentionally resolve relative to the integrated test file.
mod fred_economic_series_public_query_v5 {
    use super::*;
    use magic_fred_rs::FredClient;
    use magic_market_core::{EconomicObservation, EvidenceTimestamp, ProviderId};
    use magic_market_transport::{
        HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
    };
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Existing bounded native-envelope samples, not a live capture or admission
    // witness. These are the provider crate's public offline test resources.
    const METADATA: &[u8] = include_bytes!("../../../magic-fred-rs/tests/fixtures/series.json");
    const OBSERVATIONS: &[u8] =
        include_bytes!("../../../magic-fred-rs/tests/fixtures/observations.json");

    struct GdpSeriesFixture;

    impl HttpTransport for GdpSeriesFixture {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            if request.method() != HttpMethod::Get || !request.body().is_empty() {
                return Err(TransportError::InvalidRequest(
                    "series fixture serves only bodyless GET requests".into(),
                ));
            }
            let Some((endpoint, query)) = request.url().split_once('?') else {
                return Err(TransportError::InvalidRequest(
                    "series fixture requires an explicit bounded query".into(),
                ));
            };
            let common = [
                ("api_key", "offline-fixture-key"),
                ("file_type", "json"),
                ("series_id", "GDP"),
            ];
            let (body, required) = match endpoint {
                "https://api.stlouisfed.org/fred/series" => (METADATA, common.to_vec()),
                "https://api.stlouisfed.org/fred/series/observations" => {
                    let mut fields = common.to_vec();
                    fields.extend([
                        ("observation_start", "2025-01-01"),
                        ("observation_end", "2025-12-31"),
                        ("offset", "0"),
                        ("limit", "100000"),
                        ("sort_order", "asc"),
                    ]);
                    (OBSERVATIONS, fields)
                }
                _ => {
                    return Err(TransportError::InvalidRequest(
                        "series fixture does not serve unrelated endpoints".into(),
                    ));
                }
            };
            let supplied = query
                .split('&')
                .map(|field| {
                    field.split_once('=').ok_or_else(|| {
                        TransportError::InvalidRequest("series fixture query is malformed".into())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if supplied.len() != required.len()
                || required.iter().any(|expected| {
                    supplied.iter().filter(|actual| *actual == expected).count() != 1
                })
            {
                return Err(TransportError::InvalidRequest(
                    "series fixture requires exact non-duplicated query fields".into(),
                ));
            }
            // The real Fred source still validates endpoint, response URL,
            // MIME/body bounds, duplicate keys, series identity and pagination.
            Ok(HttpResponse::new(
                200,
                request.url(),
                Some("application/json".into()),
                body.to_vec(),
            ))
        }
    }

    #[test]
    fn registered_fred_series_preserves_missing_zero_revision_and_record_source_times() {
        // Non-secret placeholder: no environment, credential or live transport.
        let client =
            FredClient::with_transport("offline-fixture-key", Arc::new(GdpSeriesFixture)).unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline external transport only");
        // Existing shared production registrar is setup only. This does not
        // exercise the credential factory or register_fred's separate closure.
        register_economic_provider(
            &mut registry,
            client,
            "Fred",
            "admitted FRED economic series",
            4096,
        )
        .unwrap();
        // Exercise the checked external JSON request, not private request helpers.
        let data = br#"{
            "series": [{"provider":"Fred", "namespace":"fred", "code":"GDP"}],
            "start": {"Quarterly":{"year":2025,"quarter":1}},
            "end": {"Quarterly":{"year":2025,"quarter":4}},
            "max_rows": 4
        }"#;
        let command = QueryCommand::new(
            "offline-fred-series-v5",
            Operation::EconomicSeries,
            Some("Fred".into()),
            CanonicalPayload::new(
                "magic.market.economic_series.request",
                1,
                data.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();

        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let result = registry.execute(command).unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let observed = EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
        let lower = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            before.as_secs(),
            before.subsec_nanos()
        ))
        .unwrap();
        let upper = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            after.as_secs(),
            after.subsec_nanos()
        ))
        .unwrap();
        assert!(lower <= observed && observed <= upper);

        assert_eq!(result.provider, "Fred");
        assert!(result.complete);
        assert!(result.repository_admitted);
        assert_eq!(result.diagnostic_blocker, None);
        // fetch_series aggregates provenance without asserting one common native
        // source instant. Record source times must not be erased or copied here.
        assert_eq!(result.source_at, None);
        assert!(!result.batch_id.trim().is_empty());
        assert_eq!(result.records.len(), 4);

        let expected = [
            (1, Some(30142.8), "Present"),
            (2, None, "Missing"),
            (3, Some(0.0), "Present"),
            (4, Some(31000.0), "Present"),
        ]
        .into_iter()
        .map(|(quarter, value, status)| {
            serde_json::json!({
                "series": {"provider":"Fred", "namespace":"fred", "code":"GDP"},
                "name": "Gross Domestic Product",
                "region_code": null,
                "region_name": null,
                "period": {"Quarterly":{"year":2025,"quarter":quarter}},
                "value": value,
                "unit": "Billions of Dollars",
                "scale": null,
                "seasonal_adjustment": "Seasonally Adjusted Annual Rate",
                "status": status,
                "released_at": "2026-06-25T07:45:02-05:00",
                "revision": {
                    "kind": {"SourceDefined":"FRED realtime"},
                    "label": "2026-07-29"
                },
                "evidence": {
                    "provider": "Fred",
                    "source_at": "2026-06-25T07:45:02-05:00",
                    "observed_at": result.observed_at.as_str(),
                    "batch_id": result.batch_id.as_str()
                }
            })
        })
        .collect::<Vec<_>>();
        let mut actual = Vec::new();
        for payload in &result.records {
            assert_eq!(payload.schema(), "magic.market.economic_observation");
            assert_eq!(payload.schema_version(), 1);
            actual.push(serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap());
            let record: EconomicObservation = serde_json::from_slice(payload.data()).unwrap();
            assert_eq!(record.series().provider(), ProviderId::Fred);
            assert_eq!(
                record.evidence().source_at(),
                Some("2026-06-25T07:45:02-05:00")
            );
            assert_eq!(record.evidence().observed_at(), result.observed_at);
            assert_eq!(record.evidence().batch_id(), result.batch_id);
        }
        // Oracles above are fixed, independently worked values/labels, not a
        // second invocation of the parser or a derived snapshot of actual data.
        assert_eq!(actual, expected);
    }
}

// Prepare-only. Append this complete nested module to the existing internal
// grpc_production_coverage_tests.rs after root review, not through include!.
// The existing provider fixture path resolves relative to that integrated file.
mod xinhua_global_news_public_query {
    use super::*;
    use magic_market_core::{
        EvidenceTimestamp, HttpsUrl, InstrumentId, NonEmptyText, ProviderId, SourceEvidence,
    };
    use magic_market_transport::{
        HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
    };
    use magic_xinhua_rs::XinhuaClient;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Checked-in synthetic provider sample: not a live capture/admission witness.
    const LISTING: &[u8] = include_bytes!("../../../magic-xinhua-rs/tests/fixtures/news.html");

    struct OfficialListingFixture;

    impl HttpTransport for OfficialListingFixture {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            if request.method() != HttpMethod::Get
                || request.url() != "https://www.cnfin.com/news/index.html"
                || !request.body().is_empty()
            {
                return Err(TransportError::InvalidRequest(
                    "listing fixture serves only the fixed bodyless official GET".into(),
                ));
            }
            Ok(HttpResponse::new(
                200,
                request.url(),
                Some("text/html; charset=utf-8".into()),
                LISTING.to_vec(),
            ))
        }
    }

    // Caller-side typed wire model from the documented v2 record contract.
    // This is not the private production serializer or a new public contract.
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct PublicNewsRecordV2 {
        item_id: NonEmptyText,
        title: NonEmptyText,
        summary: Option<NonEmptyText>,
        content: Option<NonEmptyText>,
        publisher: NonEmptyText,
        url: HttpsUrl,
        published_at: NonEmptyText,
        instruments: Vec<InstrumentId>,
        topics: Vec<NonEmptyText>,
        language: NonEmptyText,
        evidence: SourceEvidence,
    }

    #[test]
    fn registered_xinhua_news_preserves_each_source_time_and_metadata_only_v2_record() {
        let client = XinhuaClient::with_transport(OfficialListingFixture).unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline external transport only");
        // Existing registrar used by production's Xinhua registration. Setup is
        // not the assertion seam; the only query entry below is public execute.
        register_global_news_provider(
            &mut registry,
            client,
            "XinhuaFinance",
            ProviderId::XinhuaFinance,
            "bounded first-party Xinhua Finance front-page metadata",
            4096,
        )
        .unwrap();
        let command = QueryCommand::new(
            "offline-xinhua-global-news-v2",
            Operation::GlobalNews,
            Some("XinhuaFinance".into()),
            CanonicalPayload::new(
                "magic.market.global_news.request",
                2,
                br#"{"limit":2}"#.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();

        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let result = registry.execute(command).unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let observed = EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
        let lower = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            before.as_secs(),
            before.subsec_nanos()
        ))
        .unwrap();
        let upper = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            after.as_secs(),
            after.subsec_nanos()
        ))
        .unwrap();
        assert!(lower <= observed && observed <= upper);

        assert_eq!(result.provider, "XinhuaFinance");
        assert!(result.repository_admitted);
        assert!(result.complete);
        assert_eq!(result.diagnostic_blocker, None);
        assert_eq!(result.source_at.as_deref(), Some("2026-07-29 10:31:05"));
        assert!(!result.batch_id.trim().is_empty());
        assert_eq!(result.records.len(), 2);

        // Independent literals read from the synthetic native rows and the
        // published v2 contract, not parser output or production serializer use.
        let rows = [
            (
                "4277771",
                "合成的公开财经标题 1",
                "https://www.cnfin.com/yw-lb/detail/20260729/4277771_1.html",
                "2026-07-29T10:31:05+08:00",
                "2026-07-29 10:31:05",
            ),
            (
                "4277770",
                "合成的公开财经标题 2",
                "https://www.cnfin.com/yw-lb/detail/20260729/4277770_1.html",
                "2026-07-29T10:30:05+08:00",
                "2026-07-29 10:30:05",
            ),
        ];
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        for (payload, (id, title, url, published_at, source_at)) in result.records.iter().zip(rows)
        {
            assert_eq!(payload.schema(), "magic.market.news_item");
            assert_eq!(payload.schema_version(), 2);
            let wire: PublicNewsRecordV2 = serde_json::from_slice(payload.data()).unwrap();
            assert_eq!(wire.item_id.as_str(), id);
            assert_eq!(wire.title.as_str(), title);
            assert_eq!(wire.publisher.as_str(), "新华财经");
            assert_eq!(wire.url.as_str(), url);
            assert_eq!(wire.published_at.as_str(), published_at);
            assert_eq!(wire.summary, None);
            assert_eq!(wire.content, None);
            assert!(wire.instruments.is_empty());
            assert_eq!(
                wire.topics
                    .iter()
                    .map(NonEmptyText::as_str)
                    .collect::<Vec<_>>(),
                vec!["要闻"]
            );
            assert_eq!(wire.language.as_str(), "zh-CN");
            assert_eq!(wire.evidence.provider(), ProviderId::XinhuaFinance);
            assert_eq!(wire.evidence.source_at(), Some(source_at));
            assert_eq!(wire.evidence.observed_at(), result.observed_at);
            assert_eq!(wire.evidence.batch_id(), result.batch_id);
            actual.push(serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap());
            expected.push(serde_json::json!({
                "item_id": id,
                "title": title,
                "summary": null,
                "content": null,
                "publisher": "新华财经",
                "url": url,
                "published_at": published_at,
                "instruments": [],
                "topics": ["要闻"],
                "language": "zh-CN",
                "evidence": {
                    "provider": "XinhuaFinance",
                    "source_at": source_at,
                    "observed_at": result.observed_at.as_str(),
                    "batch_id": result.batch_id.as_str()
                }
            }));
        }
        // Full wire equality catches extra/omitted fields and preserves the
        // older record's own time instead of copying the newest batch time.
        assert_eq!(actual, expected);
    }
}

mod cffex_public_planned_delivery_regression {
    use super::production_operation_registry;
    use magic_market_core::{
        EvidenceTimestamp, FuturesDeliveryMethod, FuturesProduct, HttpsUrl, IsoDate, NonEmptyText,
        ProviderId, SourceEvidence,
    };
    use magic_market_service::{CanonicalPayload, Operation, QueryCommand};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    // A caller-side model of the documented public v2 record. It does not use
    // the private projection function or introduce a production wire contract.
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct PublicPlannedDeliveryRecordV2 {
        product: FuturesProduct,
        contract_code: NonEmptyText,
        last_trading_date: Option<IsoDate>,
        delivery_date: IsoDate,
        method: FuturesDeliveryMethod,
        schedule_status: String,
        date_basis: String,
        rule_url: HttpsUrl,
        holiday_calendar_url: HttpsUrl,
        evidence: SourceEvidence,
    }

    #[test]
    fn public_cffex_delivery_query_preserves_the_conditional_2026_plan_and_observation_evidence() {
        // Use the public production factory and its ORIGINAL registered CFFEX
        // handler. The concrete CffexClient's formal trait reads the checked-in
        // 2026 schedule; it never calls the separate notice HTTP diagnostic.
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let command = QueryCommand::new(
            "offline-cffex-planned-delivery-v2",
            Operation::FuturesDelivery,
            Some("Cffex".into()),
            CanonicalPayload::new(
                "magic.market.futures_delivery.request",
                2,
                br#"{"year":2026,"month":2}"#.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();

        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let result = registry.execute(command).unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let observed = EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
        let lower = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            before.as_secs(),
            before.subsec_nanos()
        ))
        .unwrap();
        let upper = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            after.as_secs(),
            after.subsec_nanos()
        ))
        .unwrap();
        assert!(lower <= observed && observed <= upper);

        assert_eq!(result.provider, "Cffex");
        assert!(result.complete);
        assert!(result.repository_admitted);
        assert_eq!(result.diagnostic_blocker, None);
        assert_eq!(result.source_at, None);
        assert_eq!(
            result.batch_id,
            "cffex-equity-index-planned-delivery-2026-v2:02"
        );
        assert_eq!(result.records.len(), 4);

        // Independent literal dates/contract identities/rule URLs are grounded
        // in the reviewed 2026 plan and public v2 contract, not a runtime formula
        // or values read back from the provider/projection. February 24 is the
        // planned holiday-adjusted date, not a claim of actual settlement.
        let rows = [
            (
                FuturesProduct::If,
                "If",
                "IF2602",
                "https://www.cffex.com.cn/cn/hs300.html",
            ),
            (
                FuturesProduct::Ih,
                "Ih",
                "IH2602",
                "https://www.cffex.com.cn/cn/sz50gzqh.html",
            ),
            (
                FuturesProduct::Ic,
                "Ic",
                "IC2602",
                "https://www.cffex.com.cn/cn/zz500.html",
            ),
            (
                FuturesProduct::Im,
                "Im",
                "IM2602",
                "https://www.cffex.com.cn/zz1000/",
            ),
        ];
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        for (payload, (product, product_name, contract_code, rule_url)) in
            result.records.iter().zip(rows)
        {
            assert_eq!(payload.schema(), "magic.market.futures_delivery_event");
            assert_eq!(payload.schema_version(), 2);
            let wire: PublicPlannedDeliveryRecordV2 =
                serde_json::from_slice(payload.data()).unwrap();
            assert_eq!(wire.product, product);
            assert_eq!(wire.contract_code.as_str(), contract_code);
            assert_eq!(
                wire.last_trading_date.as_ref().map(IsoDate::as_str),
                Some("2026-02-24")
            );
            assert_eq!(wire.delivery_date.as_str(), "2026-02-24");
            assert_eq!(wire.method, FuturesDeliveryMethod::Cash);
            assert_eq!(wire.schedule_status, "Planned");
            assert_eq!(wire.date_basis, "CffexRuleAndPublishedHolidays");
            assert_eq!(wire.rule_url.as_str(), rule_url);
            assert_eq!(
                wire.holiday_calendar_url.as_str(),
                "https://www.gov.cn/gongbao/2025/issue_12406/material/gwygb202532.pdf"
            );
            assert_eq!(wire.evidence.provider(), ProviderId::Cffex);
            assert_eq!(wire.evidence.source_at(), None);
            assert_eq!(wire.evidence.observed_at(), result.observed_at);
            assert_eq!(
                wire.evidence.batch_id(),
                "cffex-equity-index-planned-delivery-2026-v2:02"
            );
            actual.push(serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap());
            expected.push(serde_json::json!({
                "product": product_name,
                "contract_code": contract_code,
                "last_trading_date": "2026-02-24",
                "delivery_date": "2026-02-24",
                "method": "Cash",
                "schedule_status": "Planned",
                "date_basis": "CffexRuleAndPublishedHolidays",
                "rule_url": rule_url,
                "holiday_calendar_url":
                    "https://www.gov.cn/gongbao/2025/issue_12406/material/gwygb202532.pdf",
                "evidence": {
                    "provider": "Cffex",
                    "source_at": null,
                    "observed_at": result.observed_at.as_str(),
                    "batch_id": "cffex-equity-index-planned-delivery-2026-v2:02"
                }
            }));
        }
        // Exact full-wire equality also rejects a legacy notice_url or any
        // invented monthly publication field; complete means four planned rows.
        assert_eq!(actual, expected);
    }
}

mod worldbank_missing_public_series_query {
    use super::*;
    use magic_market_core::{
        EconomicObservation, EconomicObservationStatus, EvidenceTimestamp, ProviderId,
    };
    use magic_market_transport::{
        HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
    };
    use magic_worldbank_rs::WorldBankClient;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    const INDICATOR: &[u8] =
        include_bytes!("../../../magic-worldbank-rs/tests/fixtures/indicator.json");
    const SERIES_METADATA: &[u8] =
        include_bytes!("../../../magic-worldbank-rs/tests/fixtures/series-metadata.json");
    // Synthetic native response for the exact admitted source/country/year.
    // This null is an offline boundary sample, not a live GDP availability claim.
    const MISSING_2024: &[u8] = br#"[
        {"page":1,"pages":1,"per_page":1000,"total":1,
         "sourceid":"2","lastupdated":"2026-07-13"},
        [{"indicator":{"id":"NY.GDP.MKTP.CD","value":"GDP (current US$)"},
          "country":{"id":"US","value":"United States"},
          "countryiso3code":"USA","date":"2024","value":null,
          "unit":"","obs_status":"","decimal":0}]
    ]"#;

    struct MissingAnnualGdpFixture;

    impl HttpTransport for MissingAnnualGdpFixture {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            if request.method() != HttpMethod::Get || !request.body().is_empty() {
                return Err(TransportError::InvalidRequest(
                    "WorldBank fixture requires bodyless GET".into(),
                ));
            }
            let body = match request.url() {
                "https://api.worldbank.org/v2/indicator/NY.GDP.MKTP.CD?format=json&source=2" => {
                    INDICATOR
                }
                "https://api.worldbank.org/v2/sources/2/series/NY.GDP.MKTP.CD/metadata?format=json" => {
                    SERIES_METADATA
                }
                "https://api.worldbank.org/v2/country/USA/indicator/NY.GDP.MKTP.CD?format=json&date=2024:2024&page=1&per_page=1000&source=2" => {
                    MISSING_2024
                }
                _ => {
                    return Err(TransportError::InvalidRequest(
                        "WorldBank fixture serves only the three fixed admitted-scope URLs".into(),
                    ));
                }
            };
            // The concrete client retains production endpoint/MIME/body bounds,
            // pagination, metadata checks and its one-second request gate.
            Ok(HttpResponse::new(
                200,
                request.url(),
                Some("application/json".into()),
                body.to_vec(),
            ))
        }
    }

    #[test]
    fn registered_worldbank_series_preserves_missing_annual_row_and_date_only_evidence() {
        let client = WorldBankClient::with_transport(Arc::new(MissingAnnualGdpFixture)).unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline external transport only");
        // This existing production registrar is setup, not the assertion seam.
        // Production WorldBank registration uses this same generic handler body.
        register_economic_provider(
            &mut registry,
            client,
            "WorldBank",
            "exact admitted World Bank source 2 USA annual GDP observation",
            4096,
        )
        .unwrap();
        let request = br#"{
            "series":[{"provider":"WorldBank","namespace":"source:2/country:USA",
                       "code":"NY.GDP.MKTP.CD"}],
            "start":{"Annual":{"year":2024}},
            "end":{"Annual":{"year":2024}},
            "max_rows":1
        }"#;
        let command = QueryCommand::new(
            "offline-worldbank-missing-annual-v1",
            Operation::EconomicSeries,
            Some("WorldBank".into()),
            CanonicalPayload::new(
                "magic.market.economic_series.request",
                1,
                request.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();

        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let result = registry.execute(command).unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let observed = EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
        let lower = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            before.as_secs(),
            before.subsec_nanos()
        ))
        .unwrap();
        let upper = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            after.as_secs(),
            after.subsec_nanos()
        ))
        .unwrap();
        assert!(lower <= observed && observed <= upper);
        assert_eq!(result.provider, "WorldBank");
        assert!(result.complete);
        assert!(result.repository_admitted);
        assert_eq!(result.diagnostic_blocker, None);
        // Current fetch_series reaggregates batch provenance without source_at.
        // It must not invent a common source instant from the row's source date.
        assert_eq!(result.source_at, None);
        assert_eq!(
            result.batch_id,
            format!("WorldBank:economic-series:{}", result.observed_at)
        );
        // A source null is a preserved Missing row, not zero or a dropped row.
        assert_eq!(result.records.len(), 1);
        let payload = &result.records[0];
        assert_eq!(payload.schema(), "magic.market.economic_observation");
        assert_eq!(payload.schema_version(), 1);
        let actual: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        // Independent literals: metadata supplies the unit, not the empty native
        // row unit or a heuristic based on the indicator name. Only runtime
        // observation/batch strings vary, and observation was bounded above.
        let expected = serde_json::json!({
            "series": {
                "provider":"WorldBank", "namespace":"source:2/country:USA",
                "code":"NY.GDP.MKTP.CD"
            },
            "name":"GDP (current US$)",
            "region_code":"USA",
            "region_name":"United States",
            "period":{"Annual":{"year":2024}},
            "value":null,
            "unit":"current US$",
            "scale":null,
            "seasonal_adjustment":null,
            "status":"Missing",
            "released_at":"2026-07-13",
            "revision":null,
            "evidence": {
                "provider":"WorldBank",
                "source_at":"2026-07-13",
                "observed_at":result.observed_at.as_str(),
                "batch_id":result.batch_id.as_str()
            }
        });
        assert_eq!(actual, expected);
        let record: EconomicObservation = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(record.series().provider(), ProviderId::WorldBank);
        assert_eq!(record.series().namespace(), "source:2/country:USA");
        assert_eq!(record.series().code(), "NY.GDP.MKTP.CD");
        assert_eq!(record.name(), "GDP (current US$)");
        assert_eq!(record.region_code(), Some("USA"));
        assert_eq!(record.region_name(), Some("United States"));
        assert_eq!(record.period().as_year(), Some(2024));
        assert_eq!(record.value(), None);
        assert_eq!(record.status(), &EconomicObservationStatus::Missing);
        assert_eq!(record.unit(), "current US$");
        assert_eq!(record.released_at(), Some("2026-07-13"));
        assert_eq!(record.evidence().provider(), ProviderId::WorldBank);
        assert_eq!(record.evidence().source_at(), Some("2026-07-13"));
        assert!(EvidenceTimestamp::parse_instant(record.evidence().source_at().unwrap()).is_err());
        assert_eq!(record.evidence().observed_at(), result.observed_at);
        assert_eq!(record.evidence().batch_id(), result.batch_id);
    }
}

mod registered_option_request_regressions {
    use crate::production_operation_registry;
    use magic_market_service::{CanonicalPayload, Operation, QueryCommand, ServiceError};
    use std::time::Duration;

    #[test]
    fn registered_option_greeks_reject_oversized_contract_batches() {
        // 51 unique eight-digit identities violate the public 50-contract
        // bound. The real registered handler must preserve the request error
        // before any source request, rather than trying a larger HTTP batch.
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let contracts: Vec<String> = (10_000_000..10_000_051)
            .map(|code| code.to_string())
            .collect();
        let body = serde_json::to_vec(&serde_json::json!({
            "kind": "greeks",
            "contracts": contracts,
        }))
        .unwrap();
        let command = QueryCommand::new(
            "option-greeks-oversized-public-regression",
            Operation::OptionData,
            Some("Sina".into()),
            CanonicalPayload::new("magic.market.option_data.request", 1, body, 4096).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            registry.execute(command),
            Err(ServiceError::InvalidRequest(message))
                if message == "Sina option requests accept at most 50 contracts"
        ));
    }

    #[test]
    fn registered_option_contracts_preserve_unsupported_underlying_errors() {
        // Both valid request envelopes reach the registered contract-discovery
        // handler. Unsupported underlyings are rejected before source I/O;
        // their reason and operation must not become a transport failure.
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let specimens = [
            (
                br#"{"kind":"contracts","underlying":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"expiry":null}"#.as_slice(),
                "Sina ETF options require a Shanghai fund underlying, got Shanghai Equity",
            ),
            (
                br#"{"kind":"contracts","underlying":{"exchange":"Shanghai","code":"510999","asset_class":"Fund"},"expiry":null}"#.as_slice(),
                "Sina ETF options do not support underlying 510999",
            ),
        ];
        for (body, expected_reason) in specimens {
            let command = QueryCommand::new(
                "option-unsupported-underlying-public-regression",
                Operation::OptionData,
                Some("Sina".into()),
                CanonicalPayload::new("magic.market.option_data.request", 1, body.to_vec(), 4096)
                    .unwrap(),
            )
            .unwrap();

            let result = registry.execute(command);
            assert!(matches!(
                result,
                Err(ServiceError::Unsupported { operation: Operation::OptionData, reason })
                    if reason == expected_reason
            ));
        }
    }

    #[test]
    fn registered_option_quotes_reject_empty_contracts_with_correct_schema() {
        // A valid envelope must reach the real registered Sina handler and
        // preserve its typed request error. This empty list is rejected by
        // the provider before any source transport is called; no live key,
        // endpoint override or synthetic handler replaces production wiring.
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let command = QueryCommand::new(
            "option-empty-contracts-public-regression",
            Operation::OptionData,
            Some("Sina".into()),
            CanonicalPayload::new(
                "magic.market.option_data.request",
                1,
                br#"{"kind":"quotes","contracts":[]}"#.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();

        let result = registry.execute(command);
        assert!(matches!(
            result,
            Err(ServiceError::InvalidRequest(message))
                if message == "option contract list must not be empty"
        ));
    }

    #[test]
    fn registered_query_routes_reject_non_object_requests_after_correct_envelope_validation() {
        // Wire specimens are independent literals. The earlier invalid-schema
        // matrix stops before deserialization; this matrix must reach each
        // actual callable registration's request decoder without issuing a
        // valid source request. Missing runtime credentials remain unavailable.
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let specimens = [
            (
                Operation::RealtimeQuotes,
                "magic.market.realtime_quotes.request",
                1,
            ),
            (
                Operation::HistoricalBars,
                "magic.market.historical_bars.request",
                1,
            ),
            (Operation::MinuteData, "magic.market.minute_data.request", 1),
            (Operation::OrderBooks, "magic.market.order_books.request", 1),
            (Operation::Trades, "magic.market.trades.request", 1),
            (
                Operation::SecurityMetadata,
                "magic.market.security_metadata.request",
                1,
            ),
            (
                Operation::MarketStatistics,
                "magic.market.market_statistics.request",
                1,
            ),
            (
                Operation::GlobalIndices,
                "magic.market.global_indices.request",
                1,
            ),
            (
                Operation::ForeignExchange,
                "magic.market.foreign_exchange.request",
                1,
            ),
            (
                Operation::ReferenceRates,
                "magic.market.reference_rates.request",
                1,
            ),
            (
                Operation::OfficialFxFixings,
                "magic.market.official_fx_fixings.request",
                1,
            ),
            (
                Operation::EconomicSeries,
                "magic.market.economic_series.request",
                1,
            ),
            (
                Operation::CompanyFilings,
                "magic.market.company_filings.request",
                1,
            ),
            (Operation::GlobalNews, "magic.market.global_news.request", 2),
            (
                Operation::InstrumentNews,
                "magic.market.instrument_news.request",
                2,
            ),
            (
                Operation::Announcements,
                "magic.market.announcements.request",
                1,
            ),
            (
                Operation::MarketAnnouncements,
                "magic.market.market_announcements.request",
                2,
            ),
            (
                Operation::InvestorQuestions,
                "magic.market.investor_questions.request",
                1,
            ),
            (
                Operation::PolicyDocuments,
                "magic.market.policy_documents.request",
                1,
            ),
            (
                Operation::SecurityProfiles,
                "magic.market.security_profiles.request",
                1,
            ),
            (
                Operation::FinancialStatements,
                "magic.market.financial_statements.request",
                2,
            ),
            (
                Operation::ResearchReports,
                "magic.market.research_reports.request",
                1,
            ),
            (
                Operation::ResearchDocuments,
                "magic.market.research_documents.request",
                1,
            ),
            (
                Operation::TargetPrices,
                "magic.market.target_prices.request",
                1,
            ),
            (Operation::BoardFlows, "magic.market.board_flows.request", 1),
            (Operation::MarginData, "magic.market.margin_data.request", 1),
            (
                Operation::BlockTrades,
                "magic.market.block_trades.request",
                1,
            ),
            (
                Operation::HolderCounts,
                "magic.market.holder_counts.request",
                1,
            ),
            (
                Operation::LockupEvents,
                "magic.market.lockup_events.request",
                1,
            ),
            (
                Operation::DividendPlans,
                "magic.market.dividend_plans.request",
                1,
            ),
            (Operation::LimitPools, "magic.market.limit_pools.request", 1),
            (
                Operation::DragonTiger,
                "magic.market.dragon_tiger.request",
                1,
            ),
            (Operation::Popularity, "magic.market.popularity.request", 1),
            (
                Operation::EconomicCalendar,
                "magic.market.economic_calendar.request",
                1,
            ),
            (
                Operation::NorthboundDaily,
                "magic.market.northbound_daily.request",
                1,
            ),
            (Operation::Consensus, "magic.market.consensus.request", 1),
            (
                Operation::StrongStockReasons,
                "magic.market.strong_stock_reasons.request",
                1,
            ),
            (
                Operation::SemanticSearch,
                "magic.market.semantic_search.request",
                1,
            ),
            (Operation::OptionData, "magic.market.option_data.request", 1),
            (
                Operation::CorporateActions,
                "magic.market.corporate_actions.request",
                1,
            ),
            (
                Operation::BoardDirectory,
                "magic.market.board_directory.request",
                1,
            ),
            (
                Operation::BoardConstituents,
                "magic.market.board_constituents.request",
                1,
            ),
            (
                Operation::BoardMemberships,
                "magic.market.board_memberships.request",
                1,
            ),
            (
                Operation::ConceptHits,
                "magic.market.concept_hits.request",
                1,
            ),
            (Operation::MoneyFlows, "magic.market.money_flows.request", 1),
            (
                Operation::FuturesDelivery,
                "magic.market.futures_delivery.request",
                2,
            ),
            (
                Operation::IndexQuotes,
                "magic.market.index_quotes.request",
                1,
            ),
            (
                Operation::IntradayShape,
                "magic.market.intraday_shape.request",
                1,
            ),
            (Operation::T0Evidence, "magic.market.t0_evidence.request", 2),
            (
                Operation::OutcomeDailyBars,
                "magic.market.outcome_daily_bars.request",
                1,
            ),
            (
                Operation::UpperLimitPoolReview,
                "magic.market.upper_limit_pool_review.request",
                1,
            ),
            (
                Operation::TechnicalBars,
                "magic.market.technical_bars.request",
                1,
            ),
            (
                Operation::FundFlowSeries,
                "magic.market.fund_flow_series.request",
                1,
            ),
            (
                Operation::PostCloseFlows,
                "magic.market.post_close_flows.request",
                1,
            ),
            (
                Operation::MarketRankings,
                "magic.market.market_rankings.request",
                1,
            ),
            (
                Operation::MarketBreadth,
                "magic.market.market_breadth.request",
                1,
            ),
        ];
        let capabilities = registry.capabilities();
        let mut checked = 0;
        for (operation, schema, version) in specimens {
            for capability in capabilities.iter().filter(|item| {
                item.operation == operation
                    && ((item.repository_admitted && item.runtime_available)
                        || item.diagnostic_available)
            }) {
                let command = QueryCommand::new(
                    "correct-envelope-non-object-public-regression",
                    operation,
                    Some(capability.provider.clone()),
                    CanonicalPayload::new(schema, version, b"null".to_vec(), 4096).unwrap(),
                )
                .unwrap()
                .with_unadmitted_access(true);
                let result = registry.execute(command);
                assert!(
                    matches!(&result, Err(ServiceError::InvalidRequest(message))
                        if message.starts_with("invalid ") && message.contains(" request:")),
                    "correct envelope did not reach request deserialization for {operation:?}/{}: {result:?}",
                    capability.provider,
                );
                checked += 1;
            }
        }
        assert!(
            checked >= 64,
            "callable request validation matrix was unexpectedly empty"
        );
    }
}

mod eastmoney_research_reports_public_contract {
    use super::*;
    use magic_eastmoney_rs::EastmoneyTransport;
    use magic_market_core::ResearchReport;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Synthetic reportapi-shaped specimens, not live reports or admission evidence.
    // The source order is intentionally older then newer; both dates stay date-only.
    const SOURCE_REPORTS: &str = r#"{
        "TotalPage":1,
        "data":[
            {"infoCode":"OFFLINE_REPORT_A","title":"Offline report A",
             "publishDate":"2026-07-23","orgSName":"Offline institution A",
             "orgCode":"8001","researcher":null,"author":["Analyst A","Analyst B"],
             "emRatingName":"Increase","stockCode":"600396","market":"SHANGHAI",
             "industryCode":"BK0428","industryName":"Power",
             "predictThisYearEps":"0","predictNextYearEps":"-0.5",
             "predictNextTwoYearEps":"-","indvAimPriceT":25.5,"indvAimPriceL":23.0},
            {"infoCode":"OFFLINE_REPORT_B","title":"Offline report B",
             "publishDate":"2026-07-24","orgName":"Offline institution B",
             "orgCode":null,"researcher":"Analyst C","author":null,
             "emRatingName":null,"stockCode":"600396","market":"SHANGHAI",
             "industryCode":null,"industryName":null,
             "predictThisYearEps":null,"predictNextYearEps":"--",
             "predictNextTwoYearEps":null,"indvAimPriceT":null,"indvAimPriceL":null}
        ]
    }"#;

    // Independent complete wire oracle. Only the two local runtime evidence
    // fields are filled below after observed_at is independently time-bounded.
    const EXPECTED_REPORTS: &str = r#"[
        {
            "report_id":"OFFLINE_REPORT_A",
            "scope":{"Instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}},
            "title":"Offline report A","organization":"Offline institution A",
            "organization_id":"8001","author":"Analyst A, Analyst B","rating":"Increase",
            "industry_code":"BK0428","industry_name":"Power","published_at":"2026-07-23",
            "canonical_url":"https://pdf.dfcfw.com/pdf/H3_OFFLINE_REPORT_A_1.pdf",
            "pdf_url":"https://pdf.dfcfw.com/pdf/H3_OFFLINE_REPORT_A_1.pdf",
            "estimates":[
                {"fiscal_year":2026,"eps":0.0,"eps_min":null,"eps_max":null,
                 "contributor_count":null,"revenue":null,"profit":null},
                {"fiscal_year":2027,"eps":-0.5,"eps_min":null,"eps_max":null,
                 "contributor_count":null,"revenue":null,"profit":null}
            ],
            "source_indv_aim_price_t":25.5,"source_indv_aim_price_l":23.0,
            "evidence":{"provider":"Eastmoney","source_at":"2026-07-23",
                        "observed_at":"$LOCAL_OBSERVATION","batch_id":"$LOCAL_BATCH"}
        },
        {
            "report_id":"OFFLINE_REPORT_B",
            "scope":{"Instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}},
            "title":"Offline report B","organization":"Offline institution B",
            "organization_id":null,"author":"Analyst C","rating":null,
            "industry_code":null,"industry_name":null,"published_at":"2026-07-24",
            "canonical_url":"https://pdf.dfcfw.com/pdf/H3_OFFLINE_REPORT_B_1.pdf",
            "pdf_url":"https://pdf.dfcfw.com/pdf/H3_OFFLINE_REPORT_B_1.pdf",
            "estimates":[],"source_indv_aim_price_t":null,"source_indv_aim_price_l":null,
            "evidence":{"provider":"Eastmoney","source_at":"2026-07-24",
                        "observed_at":"$LOCAL_OBSERVATION","batch_id":"$LOCAL_BATCH"}
        }
    ]"#;

    struct ReportSourceFixture(Vec<u8>);

    impl EastmoneyTransport for ReportSourceFixture {
        fn get(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            maximum_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            // Serve only the real external source boundary. No PDF/article/body
            // endpoint, fallback provider, credentials, or production handler mock.
            let expected_url = concat!(
                "https://reportapi.eastmoney.com/report/list?industryCode=%2A",
                "&pageSize=2&industry=%2A&rating=%2A&ratingChange=%2A",
                "&beginTime=2000-01-01&endTime=2099-12-31&pageNo=1",
                "&fields=&qType=0&orgCode=&code=600396&rcode="
            );
            if url != expected_url
                || !headers.contains(&("Accept", "application/json"))
                || !headers.contains(&("Referer", "https://data.eastmoney.com/"))
                || maximum_bytes != 4 * 1024 * 1024
            {
                return Err(EastmoneyError::Protocol(
                    "offline report fixture serves only the fixed bounded reportapi request".into(),
                ));
            }
            if self.0.len() > maximum_bytes {
                return Err(EastmoneyError::ResponseTooLarge {
                    limit: maximum_bytes,
                });
            }
            Ok(self.0.clone())
        }

        fn post_json(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _body: &[u8],
            _maximum_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            Err(EastmoneyError::Protocol(
                "offline report fixture does not serve POST".into(),
            ))
        }
    }

    fn report_registry(body: Vec<u8>) -> OperationRegistry {
        let client = EastmoneyClient::with_transport(ReportSourceFixture(body));
        let mut registry = OperationRegistry::all_unadmitted("offline external source only");
        // Use the actual production registrations with only the external
        // transport replaced. Public execute is still the assertion seam.
        register_eastmoney_client(&mut registry, client, 4096).unwrap();
        registry
    }

    fn report_command() -> QueryCommand {
        QueryCommand::new(
            "offline-eastmoney-research-report-v1",
            Operation::ResearchReports,
            Some("Eastmoney".into()),
            CanonicalPayload::new(
                "magic.market.research_reports.request",
                1,
                br#"{"scope":{"Instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}},"page":1,"page_size":2}"#.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn registered_research_reports_preserve_date_only_metadata_and_reject_a_conflicting_source_row()
    {
        let registry = report_registry(SOURCE_REPORTS.as_bytes().to_vec());
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let result = registry.execute(report_command()).unwrap();
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let observed_millis: u128 = result
            .observed_at
            .strip_prefix("unix-ms:")
            .unwrap()
            .parse()
            .unwrap();
        assert!(before <= observed_millis && observed_millis <= after);
        assert_eq!(result.provider, "Eastmoney");
        assert!(result.complete);
        assert!(result.repository_admitted);
        assert_eq!(result.diagnostic_blocker, None);
        assert_eq!(result.source_at.as_deref(), Some("2026-07-24"));
        assert_eq!(
            result.batch_id,
            format!("eastmoney-web:research-reports:{}", result.observed_at)
        );
        assert_eq!(result.records.len(), 2);

        let mut expected: serde_json::Value = serde_json::from_str(EXPECTED_REPORTS).unwrap();
        for row in expected.as_array_mut().unwrap() {
            row["evidence"]["observed_at"] = serde_json::Value::String(result.observed_at.clone());
            row["evidence"]["batch_id"] = serde_json::Value::String(result.batch_id.clone());
        }
        let mut actual = Vec::new();
        for payload in &result.records {
            assert_eq!(payload.schema(), "magic.market.research_report");
            assert_eq!(payload.schema_version(), 1);
            actual.push(serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap());
            let report: ResearchReport = serde_json::from_slice(payload.data()).unwrap();
            assert_eq!(report.evidence.provider(), ProviderId::Eastmoney);
            assert_eq!(report.evidence.batch_id(), result.batch_id);
            assert_eq!(report.evidence.observed_at(), result.observed_at);
            // Dates are not source instants, and local observation is never a
            // replacement for either row's explicit publication-date evidence.
            assert!(
                EvidenceTimestamp::parse_instant(report.evidence.source_at().unwrap()).is_err()
            );
        }
        assert_eq!(serde_json::Value::Array(actual), expected);

        // A valid first row cannot hide the second row's actual source identity.
        // Change one external native field only; preserve the original request.
        let mut conflicting: serde_json::Value = serde_json::from_str(SOURCE_REPORTS).unwrap();
        conflicting["data"][1]["stockCode"] = serde_json::json!("600519");
        let error = report_registry(serde_json::to_vec(&conflicting).unwrap())
            .execute(report_command())
            .unwrap_err();
        assert!(matches!(
            error,
            ServiceError::FailedPrecondition(ref message)
                if message == "Eastmoney protocol error: Eastmoney source instrument Shanghai.600519 does not match requested Shanghai.600396"
        ));
    }
}

mod hithink_corporate_actions_public_query {
    use super::*;
    use magic_hithink_rs::HithinkClient;
    use magic_market_core::{
        AssetClass, CorporateAction, CorporateActionCategory, CorporateActionStatus,
        CorporateActionTerms, EvidenceTimestamp, Exchange, ProviderId,
    };
    use magic_market_transport::{
        HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
    };
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Synthetic native envelope using the existing concrete provider test shape.
    // Fixed independent instants: 2025-06-27T00:00:00+08:00 = 1750953600000 ms;
    // 2026-06-26T00:00:00+08:00 = 1782403200000 ms. These are ex-dates, not
    // publication timestamps, and must not be promoted into source_at.
    const ADJUSTMENT_EVENTS: &[u8] = br#"{
        "code":0,"message":"success","request_id":"offline-corporate-actions-v1",
        "data": {
            "thscode":"600519.SH","ticker":"600519",
            "item":[
                {"ticker":"600519","ex_date_ms":1782403200000,
                 "dividend_per_share":30.0,"per_share_bonus":0.0},
                {"ticker":"600519","ex_date_ms":1750953600000,
                 "dividend_per_share":20.0,"per_share_bonus":0.1}
            ]
        }
    }"#;

    struct AdjustmentEventsFixture;

    impl HttpTransport for AdjustmentEventsFixture {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            if request.method() != HttpMethod::Get
                || request.url()
                    != "https://fuyao.aicubes.cn/api/a-share/corporate-actions/adjustment-factors?thscode=600519.SH&from=2025-01-01&to=2026-08-21"
                || !request.body().is_empty()
            {
                return Err(TransportError::InvalidRequest(
                    "adjustment fixture requires the fixed bodyless admitted-scope GET".into(),
                ));
            }
            // No production client, parser, request policy or registry is faked.
            Ok(HttpResponse::new(
                200,
                request.url(),
                Some("application/json".into()),
                ADJUSTMENT_EVENTS.to_vec(),
            ))
        }
    }

    #[test]
    fn registered_hithink_corporate_actions_preserve_terms_dates_and_absent_publication_time() {
        // Non-secret placeholder; no environment/factory/real credential path.
        let client =
            HithinkClient::with_transport("offline-fixture-key", AdjustmentEventsFixture).unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline external transport only");
        // Original shared production registrar is setup only. The sole assertion
        // entrance is the public registry execute call below, not a copied handler.
        register_hithink_client(&mut registry, Arc::new(client), 4096).unwrap();
        let request = br#"{
            "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
            "start":"2025-01-01","end":"2026-08-21"
        }"#;
        let command = QueryCommand::new(
            "offline-hithink-corporate-actions-public-v1",
            Operation::CorporateActions,
            Some("HithinkFinance".into()),
            CanonicalPayload::new(
                "magic.market.corporate_actions.request",
                1,
                request.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();

        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let result = registry.execute(command).unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let observed = EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
        let lower = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            before.as_secs(),
            before.subsec_nanos()
        ))
        .unwrap();
        let upper = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            after.as_secs(),
            after.subsec_nanos()
        ))
        .unwrap();
        assert!(lower <= observed && observed <= upper);
        assert_eq!(result.provider, "HithinkFinance");
        assert_eq!(result.batch_id, "offline-corporate-actions-v1");
        assert!(result.complete);
        assert!(result.repository_admitted);
        assert_eq!(result.diagnostic_blocker, None);
        // No native publication timestamp exists. Neither ex-date nor local
        // receipt clock is a substitute for source publication evidence.
        assert_eq!(result.source_at, None);
        assert_eq!(result.records.len(), 2);

        // Independently specified full wire records, ascending by effective day,
        // despite reversed native input. No production parser is invoked to
        // manufacture this oracle. Runtime observation is bounded independently.
        let expected = vec![
            serde_json::json!({
                "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
                "category":"Distribution",
                "effective_on":"2025-06-27",
                "record_on":null,
                "ex_on":"2025-06-27",
                "payable_on":null,
                "status":"Implemented",
                "terms":{"Distribution":{
                    "cash_per_share":20.0,"bonus_per_share":0.1,
                    "rights_per_share":null,"rights_price":null
                }},
                "evidence":{
                    "provider":"Tonghuashun","source_at":null,
                    "observed_at":result.observed_at.as_str(),
                    "batch_id":"offline-corporate-actions-v1"
                }
            }),
            serde_json::json!({
                "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
                "category":"Distribution",
                "effective_on":"2026-06-26",
                "record_on":null,
                "ex_on":"2026-06-26",
                "payable_on":null,
                "status":"Implemented",
                "terms":{"Distribution":{
                    "cash_per_share":30.0,"bonus_per_share":null,
                    "rights_per_share":null,"rights_price":null
                }},
                "evidence":{
                    "provider":"Tonghuashun","source_at":null,
                    "observed_at":result.observed_at.as_str(),
                    "batch_id":"offline-corporate-actions-v1"
                }
            }),
        ];
        let mut actual = Vec::new();
        for (payload, (day, cash, bonus)) in result
            .records
            .iter()
            .zip([("2025-06-27", 20.0, Some(0.1)), ("2026-06-26", 30.0, None)])
        {
            assert_eq!(payload.schema(), "magic.market.corporate_action");
            assert_eq!(payload.schema_version(), 1);
            actual.push(serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap());
            let record: CorporateAction = serde_json::from_slice(payload.data()).unwrap();
            assert_eq!(record.instrument().exchange(), Exchange::Shanghai);
            assert_eq!(record.instrument().code(), "600519");
            assert_eq!(record.instrument().asset_class(), AssetClass::Equity);
            assert_eq!(record.category(), CorporateActionCategory::Distribution);
            assert_eq!(record.status(), CorporateActionStatus::Implemented);
            assert!(record.is_implemented());
            assert_eq!(record.effective_on().as_str(), day);
            assert_eq!(record.ex_on().map(|date| date.as_str()), Some(day));
            assert_eq!(record.record_on(), None);
            assert_eq!(record.payable_on(), None);
            assert!(EvidenceTimestamp::parse_instant(record.effective_on().as_str()).is_err());
            match record.terms() {
                CorporateActionTerms::Distribution {
                    cash_per_share,
                    bonus_per_share,
                    rights_per_share,
                    rights_price,
                } => {
                    assert_eq!(cash_per_share.as_ref().unwrap().get(), cash);
                    assert_eq!(bonus_per_share.as_ref().map(|value| value.get()), bonus);
                    assert!(rights_per_share.is_none());
                    assert!(rights_price.is_none());
                }
                _ => panic!("expected source cash/bonus distribution terms"),
            }
            assert_eq!(record.evidence().provider(), ProviderId::Tonghuashun);
            assert_eq!(record.evidence().source_at(), None);
            assert_eq!(record.evidence().observed_at(), result.observed_at);
            assert_eq!(record.evidence().batch_id(), result.batch_id);
        }
        assert_eq!(actual, expected);
    }
}

mod hithink_valuations_public_query {
    use super::*;
    use magic_hithink_rs::HithinkClient;
    use magic_market_core::{
        AssetClass, EvidenceTimestamp, Exchange, MarketStatistics, ProviderId,
    };
    use magic_market_transport::{
        HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
    };
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Synthetic native sample, based on the checked-in valuation test shape.
    // Negative, real zero and explicit null are deliberately distinct. The
    // native timestamp is batch-only upstream context, not a per-metric instant.
    const VALUATION_SNAPSHOT: &[u8] = br#"{
        "code":0,"message":"success","request_id":"offline-valuations-v1",
        "data":{
            "timestamp":1787068800000,"total":1,
            "item":[{
                "thscode":"600519.SH","ticker":"600519","name":null,
                "pe_ttm":-2.0,"pe_mrq":0.0,"pb_mrq":null,
                "ps_ttm":8.1,"pcf_ttm":-3.0
            }]
        }
    }"#;

    struct ValuationSnapshotFixture;

    impl HttpTransport for ValuationSnapshotFixture {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            if request.method() != HttpMethod::Get
                || request.url()
                    != "https://fuyao.aicubes.cn/api/a-share/valuations/snapshot?thscodes=600519.SH"
                || !request.body().is_empty()
            {
                return Err(TransportError::InvalidRequest(
                    "valuation fixture requires the fixed bodyless single-equity GET".into(),
                ));
            }
            // Real client policy, envelope, identity and numeric validation stay.
            Ok(HttpResponse::new(
                200,
                request.url(),
                Some("application/json".into()),
                VALUATION_SNAPSHOT.to_vec(),
            ))
        }
    }

    #[test]
    fn registered_hithink_valuations_preserve_negative_zero_null_and_batch_only_time() {
        // This non-secret placeholder never reads an environment credential.
        let client =
            HithinkClient::with_transport("offline-fixture-key", ValuationSnapshotFixture).unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline external transport only");
        // Same original production registrar as setup, never a copied closure.
        register_hithink_client(&mut registry, Arc::new(client), 4096).unwrap();
        let request = br#"{
            "instruments":[{"exchange":"Shanghai","code":"600519","asset_class":"Equity"}]
        }"#;
        let command = QueryCommand::new(
            "offline-hithink-valuation-public-v1",
            Operation::MarketStatistics,
            Some("HithinkFinance".into()),
            CanonicalPayload::new(
                "magic.market.market_statistics.request",
                1,
                request.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();

        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let result = registry.execute(command).unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let observed = EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
        let lower = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            before.as_secs(),
            before.subsec_nanos()
        ))
        .unwrap();
        let upper = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            after.as_secs(),
            after.subsec_nanos()
        ))
        .unwrap();
        assert!(lower <= observed && observed <= upper);
        assert_eq!(result.provider, "HithinkFinance");
        assert_eq!(result.batch_id, "offline-valuations-v1");
        assert_eq!(result.source_at.as_deref(), Some("unix-ms:1787068800000"));
        assert!(result.complete);
        assert!(result.repository_admitted);
        assert_eq!(result.diagnostic_blocker, None);
        assert_eq!(result.records.len(), 1);
        let payload = &result.records[0];
        assert_eq!(payload.schema(), "magic.market.market_statistics");
        assert_eq!(payload.schema_version(), 1);
        let actual: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        // Independent complete literal wire record. PS/PCF are validated native
        // inputs, not turnover, market cap, volume ratio or other Core fields.
        let expected = serde_json::json!({
            "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
            "turnover_rate":null,
            "trailing_pe":-2.0,
            "static_pe":0.0,
            "pb":null,
            "total_market_cap":null,
            "floating_market_cap":null,
            "upper_limit":null,
            "lower_limit":null,
            "volume_ratio":null,
            "evidence":{
                "provider":"Tonghuashun",
                "source_at":null,
                "observed_at":result.observed_at.as_str(),
                "batch_id":"offline-valuations-v1"
            }
        });
        assert_eq!(actual, expected);
        let record: MarketStatistics = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(record.instrument().exchange(), Exchange::Shanghai);
        assert_eq!(record.instrument().code(), "600519");
        assert_eq!(record.instrument().asset_class(), AssetClass::Equity);
        assert_eq!(record.trailing_pe().unwrap().get(), -2.0);
        assert_eq!(record.static_pe().unwrap().get(), 0.0);
        assert_eq!(record.pb(), None);
        assert_eq!(record.turnover_rate(), None);
        assert_eq!(record.total_market_cap(), None);
        assert_eq!(record.floating_market_cap(), None);
        assert_eq!(record.upper_limit(), None);
        assert_eq!(record.lower_limit(), None);
        assert_eq!(record.volume_ratio(), None);
        assert_eq!(record.evidence().provider(), ProviderId::Tonghuashun);
        // Endpoint has no timestamp for an individual metric. Outer batch
        // source_at is not copied into the record or replaced by observation.
        assert_eq!(record.evidence().source_at(), None);
        assert_eq!(record.evidence().observed_at(), result.observed_at);
        assert_eq!(record.evidence().batch_id(), result.batch_id);
    }
}

#[path = "grpc_miaoxiang_query_tests.rs"]
mod miaoxiang_public_contract;

#[path = "grpc_sina_option_quote_query_tests.rs"]
mod sina_option_quote_public_contract;

mod hithink_financial_atomic_public_contract {
    use super::*;
    use magic_hithink_rs::HithinkClient;
    use magic_market_core::EvidenceTimestamp;
    use magic_market_transport::{
        HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
    };
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    const CASH_FLOW_SOURCE: &[u8] = br#"{
        "code": 0,
        "message": "success",
        "request_id": "offline-financial-atomic",
        "data": {
            "timestamp": 1782748800000,
            "item": [{
                "thscode": "600519.SH",
                "ticker": "600519",
                "period": "quarterly",
                "period_end_ms": 1782748800000,
                "report_date_ms": 1787587200000,
                "fiscal_year": 2026,
                "fiscal_period": "H1",
                "currency": "CNY",
                "act_cash_flow_net": 0.0,
                "invest_cash_flow_net": null,
                "financing_cash_flow_net": -42.5,
                "cash_equivalents_net_addition": 12.75,
                "pay_dividends_profits_interest_cash": 100.0,
                "pay_fixed_assets_etc_cash": null
            }]
        }
    }"#;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum SourceOutcome {
        CompleteCashFlowResponse,
        HttpStatus429,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SourceCall {
        method: HttpMethod,
        endpoint: String,
        query: BTreeMap<String, String>,
        headers: BTreeMap<String, String>,
        body_len: usize,
        outcome: SourceOutcome,
    }

    struct FinancialHttpFixture {
        calls: Arc<Mutex<Vec<SourceCall>>>,
    }

    impl HttpTransport for FinancialHttpFixture {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            assert_eq!(request.method(), HttpMethod::Get);
            assert!(request.body().is_empty());
            assert!(!request.url().contains('#'));
            let (endpoint, raw_query) = request.url().split_once('?').unwrap();
            assert_eq!(
                endpoint,
                "https://fuyao.aicubes.cn/api/a-share/financials/cash-flow-statements"
            );
            let mut query = BTreeMap::new();
            for pair in raw_query.split('&') {
                let (key, value) = pair.split_once('=').unwrap();
                assert!(query.insert(key.to_owned(), value.to_owned()).is_none());
            }
            assert_eq!(query.len(), 3);
            assert_eq!(query.get("period").map(String::as_str), Some("quarterly"));
            assert_eq!(query.get("limit").map(String::as_str), Some("20"));
            let outcome = match query.get("thscode").map(String::as_str) {
                Some("600519.SH") => SourceOutcome::CompleteCashFlowResponse,
                Some("000001.SZ") => SourceOutcome::HttpStatus429,
                other => panic!("unexpected offline financial source identity: {other:?}"),
            };
            let headers = request
                .headers()
                .iter()
                .cloned()
                .collect::<BTreeMap<_, _>>();
            assert_eq!(request.headers().len(), 3);
            assert_eq!(
                headers,
                BTreeMap::from([
                    ("Accept".to_owned(), "application/json".to_owned()),
                    ("User-Agent".to_owned(), "magic-hithink-rs/0.2".to_owned()),
                    ("X-api-key".to_owned(), "offline-fixture-key".to_owned()),
                ])
            );
            self.calls.lock().unwrap().push(SourceCall {
                method: request.method(),
                endpoint: endpoint.to_owned(),
                query,
                headers,
                body_len: request.body().len(),
                outcome: outcome.clone(),
            });
            match outcome {
                SourceOutcome::CompleteCashFlowResponse => Ok(HttpResponse::new(
                    200,
                    request.url(),
                    Some("application/json".into()),
                    CASH_FLOW_SOURCE.to_vec(),
                )),
                SourceOutcome::HttpStatus429 => Err(TransportError::HttpStatus { status: 429 }),
            }
        }
    }

    fn registry(calls: &Arc<Mutex<Vec<SourceCall>>>) -> OperationRegistry {
        // This non-secret placeholder is passed only to the offline HTTP boundary.
        // No environment, credentials or live transport are consulted.
        let client = HithinkClient::with_transport(
            "offline-fixture-key",
            FinancialHttpFixture {
                calls: Arc::clone(calls),
            },
        )
        .unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline financial source only");
        register_hithink_client(&mut registry, Arc::new(client), 16384).unwrap();
        registry
    }

    fn command(instruments: serde_json::Value) -> QueryCommand {
        QueryCommand::new(
            "offline-financial-atomic-contract",
            Operation::FinancialStatements,
            Some("HithinkFinance".into()),
            CanonicalPayload::new(
                "magic.market.financial_statements.request",
                2,
                serde_json::to_vec(&serde_json::json!({
                    "instruments": instruments,
                    "kind": "CashFlow"
                }))
                .unwrap(),
                16384,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn expected_call(symbol: &str, outcome: SourceOutcome) -> SourceCall {
        SourceCall {
            method: HttpMethod::Get,
            endpoint: "https://fuyao.aicubes.cn/api/a-share/financials/cash-flow-statements".into(),
            query: BTreeMap::from([
                ("thscode".into(), symbol.into()),
                ("period".into(), "quarterly".into()),
                ("limit".into(), "20".into()),
            ]),
            headers: BTreeMap::from([
                ("Accept".into(), "application/json".into()),
                ("User-Agent".into(), "magic-hithink-rs/0.2".into()),
                ("X-api-key".into(), "offline-fixture-key".into()),
            ]),
            body_len: 0,
            outcome,
        }
    }

    #[test]
    fn registered_hithink_financial_second_rate_limit_does_not_publish_prior_report() {
        // A minimal independent control proves the first native response is valid;
        // its trace is separate from the two-request rejection, not a recovery claim.
        let control_calls = Arc::new(Mutex::new(Vec::new()));
        let control_registry = registry(&control_calls);
        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let control = control_registry
            .execute(command(serde_json::json!([{
                "exchange": "Shanghai", "code": "600519", "asset_class": "Equity"
            }])))
            .unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        assert!(control.complete && control.repository_admitted);
        assert_eq!(control.provider, "HithinkFinance");
        assert_eq!(control.diagnostic_blocker, None);
        assert_eq!(
            control.batch_id,
            "hithink-financials:offline-financial-atomic"
        );
        assert_eq!(control.source_at.as_deref(), Some("unix-ms:1787587200000"));
        assert_eq!(control.records.len(), 1);
        let payload = &control.records[0];
        assert_eq!(payload.schema(), "magic.market.financial_statement");
        assert_eq!(payload.schema_version(), 2);
        let wire: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(wire["report_period"], "2026-06-30");
        assert_eq!(wire["announced_on"], "2026-08-25");
        assert_eq!(wire["fiscal_period"], "H1");
        assert_eq!(
            wire["lines"][0],
            serde_json::json!({
                "key": "act_cash_flow_net", "source_label": "act_cash_flow_net",
                "value": 0.0, "unit": "CNY"
            })
        );
        assert_eq!(
            wire["lines"][1],
            serde_json::json!({
                "key": "invest_cash_flow_net", "source_label": "invest_cash_flow_net",
                "value": null, "unit": "CNY"
            })
        );
        assert_eq!(
            wire["lines"][2],
            serde_json::json!({
                "key": "financing_cash_flow_net", "source_label": "financing_cash_flow_net",
                "value": -42.5, "unit": "CNY"
            })
        );
        let observed = EvidenceTimestamp::parse_instant(&control.observed_at).unwrap();
        let lower = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            before.as_secs(),
            before.subsec_nanos()
        ))
        .unwrap();
        let upper = EvidenceTimestamp::parse_instant(&format!(
            "{}.{:09}",
            after.as_secs(),
            after.subsec_nanos()
        ))
        .unwrap();
        assert!(lower <= observed && observed <= upper);
        assert_eq!(
            wire["evidence"],
            serde_json::json!({
                "provider": "Tonghuashun", "source_at": "unix-ms:1787587200000",
                "observed_at": control.observed_at,
                "batch_id": "hithink-financials:offline-financial-atomic"
            })
        );
        assert_eq!(
            control_calls.lock().unwrap().as_slice(),
            &[expected_call(
                "600519.SH",
                SourceOutcome::CompleteCashFlowResponse
            )]
        );

        let rejection_calls = Arc::new(Mutex::new(Vec::new()));
        let rejection_registry = registry(&rejection_calls);
        let error = rejection_registry
            .execute(command(serde_json::json!([
                {"exchange": "Shanghai", "code": "600519", "asset_class": "Equity"},
                {"exchange": "Shenzhen", "code": "000001", "asset_class": "Equity"}
            ])))
            .unwrap_err();
        // Err is the caller-visible result: neither an empty success nor the first
        // source's successful record is published by this two-instrument execution.
        assert_eq!(
            error,
            ServiceError::ProviderFailure {
                operation: Operation::FinancialStatements,
                provider: "HithinkFinance".into(),
                kind: ProviderFailureKind::RateLimited,
                provider_reason: "http_status=429".into()
            }
        );
        assert_eq!(
            rejection_calls.lock().unwrap().as_slice(),
            &[
                expected_call("600519.SH", SourceOutcome::CompleteCashFlowResponse),
                expected_call("000001.SZ", SourceOutcome::HttpStatus429)
            ]
        );
    }
}

#[path = "grpc_nbs_identity_query_tests.rs"]
mod nbs_identity_public_contract;

#[path = "grpc_pbc_regional_sheet_query_tests.rs"]
mod pbc_regional_sheet_public_contract;

#[path = "grpc_ths_consensus_query_tests.rs"]
mod ths_consensus_public_contract;

#[path = "grpc_ths_popularity_query_tests.rs"]
mod ths_popularity_public_contract;

#[path = "grpc_ths_strong_reasons_query_tests.rs"]
mod ths_strong_reasons_public_contract;

#[path = "grpc_ths_limit_pools_query_tests.rs"]
mod ths_upper_limit_pool_public_contract;

#[path = "grpc_baidu_technical_bars_query_tests.rs"]
mod baidu_technical_bars_public_contract;

#[path = "grpc_hkex_northbound_query_tests.rs"]
mod hkex_northbound_public_contract;

#[path = "grpc_sse_announcements_query_tests.rs"]
mod sse_announcements_public_contract;

#[path = "grpc_szse_order_books_query_tests.rs"]
mod szse_order_books_public_contract;

#[path = "grpc_szse_announcements_query_tests.rs"]
mod szse_announcements_public_contract;

#[path = "grpc_hithink_legacy_auction_query_tests.rs"]
mod hithink_legacy_auction_public_contract;

#[path = "grpc_hithink_popularity_query_tests.rs"]
mod hithink_popularity_public_contract;

#[path = "grpc_szse_dragon_tiger_seats_query_tests.rs"]
mod szse_dragon_tiger_seats_public_contract;

#[path = "grpc_tdx_board_memberships_query_tests.rs"]
mod tdx_board_memberships_public_contract;

#[path = "grpc_emquant_quote_scope_query_tests.rs"]
mod emquant_quotes_scope_public_contract;

#[path = "grpc_tdx_smart_quotes_query_tests.rs"]
mod tdx_smart_quotes_public_contract;

#[path = "grpc_hithink_instrument_request_rejection_tests.rs"]
mod hithink_instrument_request_public_contract;

#[path = "grpc_eastmoney_market_dragon_tiger_query_tests.rs"]
mod eastmoney_market_dragon_tiger_public_contract;

#[path = "grpc_sse_dragon_tiger_query_tests.rs"]
mod sse_dragon_tiger_public_contract;
