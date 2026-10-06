use super::*;
use magic_market_core::{InstrumentId, PositiveU32};
use serde_json::json;
use std::sync::Mutex;

type ObservedRequest = (String, Vec<(String, String)>, Vec<u8>);

const _: () = assert!(!MX_DAILY_FUND_FLOW_ADMITTED);
const _: () = assert!(!MX_OPENING_AUCTION_ADMITTED);
const _: () = assert!(MX_MARKET_BREADTH_ADMITTED);

#[derive(Clone)]
struct FixtureTransport {
    responses: Arc<Mutex<Vec<Vec<u8>>>>,
    requests: Arc<Mutex<Vec<ObservedRequest>>>,
}

impl FixtureTransport {
    fn new(responses: Vec<serde_json::Value>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(
                responses
                    .into_iter()
                    .rev()
                    .map(|value| serde_json::to_vec(&value).unwrap())
                    .collect(),
            )),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl EastmoneyTransport for FixtureTransport {
    fn get(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        unreachable!()
    }

    fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.requests.lock().unwrap().push((
            url.to_owned(),
            headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            body.to_vec(),
        ));
        self.responses
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| EastmoneyError::Transport("fixture exhausted".into()))
    }
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

#[test]
fn mx_client_can_share_the_public_clients_single_transport_lane() {
    let public = crate::EastmoneyClient::with_transport(FixtureTransport::new(Vec::new()));
    let mx = EastmoneyMxClient::with_client("mkt_test_key", &public).unwrap();
    assert!(Arc::ptr_eq(&public.transport, &mx.transport));
}

fn entity() -> serde_json::Value {
    json!({
        "entityType": "SEC",
        "entityTypeName": "A股",
        "className": "沪深京股票",
        "fullName": "华电辽能",
        "secuCode": "600396",
        "marketChar": ".SH"
    })
}

fn envelope(id: &str, tables: Vec<serde_json::Value>) -> serde_json::Value {
    json!({
        "success": true,
        "status": 0,
        "code": 0,
        "message": "ok",
        "requestId": id,
        "data": {
            "status": 0,
            "code": 0,
            "message": "OK",
            "data": {
                "protocolType": "SEARCH_DATA",
                "id": format!("payload-{id}"),
                "searchDataResultDTO": { "dataTableDTOList": tables }
            }
        }
    })
}

fn opening_auction_table(volume_unit: &str) -> serde_json::Value {
    json!({
        "code": "600396.SH",
        "entityName": "华电辽能(600396.SH)",
        "rawTable": {
            "100000000047336": ["2951900"],
            "100000000047337": ["53665542"],
            "headName": ["2026-08-14"]
        },
        "nameMap": {
            "100000000047336": "开盘集合竞价成交量",
            "100000000047337": "开盘集合竞价成交额",
            "headNameSub": "数据来源"
        },
        "field": {
            "returnCode": "100000000047336",
            "returnName": "开盘集合竞价成交量",
            "dateGranularity": "DAY",
            "unitName": volume_unit
        },
        "fieldSet": [
            {
                "returnCode": "100000000047336",
                "returnName": "开盘集合竞价成交量",
                "dateGranularity": "DAY",
                "unitName": volume_unit
            },
            {
                "returnCode": "100000000047337",
                "returnName": "开盘集合竞价成交额",
                "dateGranularity": "DAY",
                "unitName": "元"
            }
        ],
        "entityTagDTO": entity()
    })
}

#[test]
fn debug_redacts_key_and_admission_stays_false() {
    let client =
        EastmoneyMxClient::with_transport("mkt_secret_value", FixtureTransport::new(Vec::new()))
            .unwrap();
    let debug = format!("{client:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("mkt_secret_value"));
}

#[test]
fn opening_auction_preserves_observed_fields_and_nulls_unproved_fields() {
    let transport = FixtureTransport::new(vec![envelope(
        "auction-request",
        vec![opening_auction_table("股")],
    )]);
    let observed = transport.clone();
    let client = EastmoneyMxClient::with_transport("mkt_test_key", transport).unwrap();
    let batch = client
        .diagnose_opening_auction(&instrument(), &IsoDate::new("2026-08-14").unwrap())
        .unwrap();
    let record = &batch.records()[0];
    assert_eq!(record.matched_quantity_shares.unwrap().get(), 2_951_900.0);
    assert_eq!(record.matched_amount_cny.unwrap().get(), 53_665_542.0);
    assert!(record.matched_price.is_none());
    assert!(record.previous_close.is_none());
    assert!(record.change_percent.is_none());
    assert!(record.unmatched_bid_quantity_shares.is_none());
    assert!(record.unmatched_ask_quantity_shares.is_none());
    assert!(record.volume_ratio.is_none());
    assert_eq!(record.status, DataStatus::Available);
    assert!(batch.quality().is_complete());
    let requests = observed.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests.iter().all(|request| request.0 == ENDPOINT));
    assert!(requests.iter().all(|request| request
        .1
        .iter()
        .any(|(name, value)| name == "apikey" && value == "mkt_test_key")));
}

#[test]
fn opening_auction_rejects_unproved_unit() {
    let client = EastmoneyMxClient::with_transport(
        "mkt_test_key",
        FixtureTransport::new(vec![envelope(
            "auction-request",
            vec![opening_auction_table("手")],
        )]),
    )
    .unwrap();
    assert!(matches!(
        client.diagnose_opening_auction(&instrument(), &IsoDate::new("2026-08-14").unwrap()),
        Err(EastmoneyError::Protocol(_))
    ));
}

#[test]
fn opening_auction_rejects_two_tables_missing_field_set_and_wrong_amount_unit() {
    let table = opening_auction_table("股");
    for tables in [
        vec![table.clone(), table.clone()],
        {
            let mut missing = table.clone();
            missing.as_object_mut().unwrap().remove("fieldSet");
            vec![missing]
        },
        {
            let mut wrong_amount_unit = table;
            wrong_amount_unit["fieldSet"][1]["unitName"] = json!("万元");
            vec![wrong_amount_unit]
        },
    ] {
        let client = EastmoneyMxClient::with_transport(
            "mkt_test_key",
            FixtureTransport::new(vec![envelope("auction-request", tables)]),
        )
        .unwrap();
        assert!(matches!(
            client.diagnose_opening_auction(&instrument(), &IsoDate::new("2026-08-14").unwrap()),
            Err(EastmoneyError::Protocol(_))
        ));
    }
}

/// One `HQ` table carrying a single dated metric, as the provider returned
/// them on 2026-09-21: the volume table declares `股`, the amount table
/// declares no unit at all.
fn dated_hq_metric_table(
    label: &str,
    key: &str,
    unit: Option<&str>,
    value: &str,
    head_name: &str,
) -> serde_json::Value {
    let raw = serde_json::Map::from_iter([
        (key.to_owned(), json!([value])),
        ("headName".to_owned(), json!([head_name])),
    ]);
    let names = serde_json::Map::from_iter([
        (key.to_owned(), json!(label)),
        ("headNameSub".to_owned(), json!("数据来源")),
    ]);
    let field = json!({
        "returnCode": key,
        "returnName": label,
        "dateGranularity": "DAY",
        "unitName": unit
    });
    json!({
        "code": "600396.SH",
        "entityName": "华电辽能(600396.SH)",
        "dataTypeEnum": "HQ",
        "rawTable": raw,
        "nameMap": names,
        "field": field.clone(),
        "fieldSet": [field],
        "entityTagDTO": entity()
    })
}

fn auction_shape_failure(tables: Vec<serde_json::Value>) -> String {
    let client = EastmoneyMxClient::with_transport(
        "mkt_test_key",
        FixtureTransport::new(vec![envelope("auction-shape", tables)]),
    )
    .unwrap();
    match client.diagnose_opening_auction(&instrument(), &IsoDate::new("2026-09-21").unwrap()) {
        Err(EastmoneyError::Protocol(message)) => message,
        Err(other) => panic!("expected a protocol failure, got {other}"),
        Ok(_) => panic!("expected a protocol failure, got an admitted batch"),
    }
}

#[test]
fn opening_auction_shape_failure_names_the_observed_tables() {
    // The 2026-09-21 answer: two single-metric tables dated today, one of
    // them without a declared amount unit, plus a two-metric table whose
    // source date is the previous trading day.
    let split = auction_shape_failure(vec![
        dated_hq_metric_table(
            "开盘集合竞价成交量",
            "AUC_VOLUME_010000_AUC_VOLUME_99",
            Some("股"),
            "24100",
            "2026-09-21 11:39",
        ),
        opening_auction_table("股"),
        dated_hq_metric_table(
            "开盘集合竞价成交额",
            "JHJJCJE_f63_3",
            None,
            "30341900.00",
            "2026-09-21 11:39",
        ),
    ]);
    assert!(split.contains("3 tables"), "{split}");
    assert!(
        split.contains("HQ[开盘集合竞价成交量(股)] @2026-09-21 11:39"),
        "{split}"
    );
    assert!(
        split.contains("HQ[开盘集合竞价成交额(no unit)] @2026-09-21 11:39"),
        "{split}"
    );
    assert!(
        split.contains("untyped[开盘集合竞价成交量(股), 开盘集合竞价成交额(元)] @2026-08-14"),
        "{split}"
    );

    // A repeated table is a different fault, and the message says so.
    let duplicate = auction_shape_failure(vec![
        opening_auction_table("股"),
        opening_auction_table("股"),
    ]);
    assert!(duplicate.contains("2 tables"), "{duplicate}");
    assert!(!duplicate.contains("HQ["), "{duplicate}");
}

fn breadth_table(fields: &[(&str, &str, &str)], return_code: &str) -> serde_json::Value {
    let raw = fields
        .iter()
        .map(|(_, code, value)| ((*code).to_owned(), json!([value])))
        .chain(std::iter::once((
            "headName".to_owned(),
            json!(["2026-08-14"]),
        )))
        .collect::<serde_json::Map<_, _>>();
    let names = fields
        .iter()
        .map(|(label, code, _)| ((*code).to_owned(), json!(label)))
        .collect::<serde_json::Map<_, _>>();
    let field_set = fields
        .iter()
        .map(|(label, code, _)| {
            json!({
                "returnCode": code,
                "returnName": label,
                "dateGranularity": "DAY",
                "unitName": null
            })
        })
        .collect::<Vec<_>>();
    json!({
        "code": "001071",
        "entityName": "全部A股(板块)",
        "rawTable": raw,
        "nameMap": names,
        "field": {
            "returnCode": return_code,
            "returnName": fields.iter().find(|(_, code, _)| *code == return_code).unwrap().0,
            "dateGranularity": "DAY",
            "unitName": null
        },
        "fieldSet": field_set,
        "entityTagDTO": {
            "entityType": "BLOCK",
            "entityTypeName": "BLOCK",
            "className": "市场类(沪深京)",
            "fullName": "全部A股"
        }
    })
}

#[test]
fn breadth_proves_total_coverage_and_one_response_atomicity() {
    let transport = FixtureTransport::new(vec![envelope(
        "breadth-request",
        vec![
            breadth_table(
                &[
                    ("上市股票数量", "listed", "5544"),
                    ("上涨家数", "up", "2400"),
                    ("下跌家数", "down", "2970"),
                    ("平盘家数", "flat", "170"),
                ],
                "down",
            ),
            breadth_table(
                &[
                    ("涨停家数", "limit-up", "64"),
                    ("跌停家数", "limit-down", "13"),
                ],
                "limit-down",
            ),
        ],
    )]);
    let observed = transport.clone();
    let client = EastmoneyMxClient::with_transport("mkt_test_key", transport).unwrap();
    let batch = client
        .diagnose_market_breadth(&IsoDate::new("2026-08-14").unwrap())
        .unwrap();
    let record = &batch.records()[0];
    assert_eq!((record.up, record.down, record.flat), (2400, 2970, 170));
    assert_eq!((record.limit_up, record.limit_down), (64, 13));
    assert_eq!(record.valid, 5540);
    assert_eq!(record.listed_total, Some(5544));
    assert_eq!(record.coverage.unwrap().get(), 5540.0 / 5544.0);
    assert!(record.maximum_source_skew_millis.is_none());
    assert_eq!(record.status, DataStatus::Available);
    assert!(batch.quality().is_complete());
    assert_eq!(observed.requests.lock().unwrap().len(), 1);
}

#[test]
fn source_counts_accept_only_canonical_or_thousands_grouped_digits() {
    assert_eq!(parse_source_count("5544", "count").unwrap(), 5544);
    assert_eq!(parse_source_count("5,544", "count").unwrap(), 5544);
    assert_eq!(parse_source_count("5544.0", "count").unwrap(), 5544);
    assert_eq!(parse_source_count("5,544.00", "count").unwrap(), 5544);
    for invalid in ["", "55,44", ",544", "5,54a", "5544家", "5.544", "1.1"] {
        assert!(parse_source_count(invalid, "count").is_err(), "{invalid}");
    }
}

#[test]
fn breadth_rejects_wrong_table_count_missing_field_set_and_total_contradiction() {
    let directional = breadth_table(
        &[
            ("上市股票数量", "listed", "5544"),
            ("上涨家数", "up", "2400"),
            ("下跌家数", "down", "2970"),
            ("平盘家数", "flat", "170"),
        ],
        "down",
    );
    let limits = breadth_table(
        &[
            ("涨停家数", "limit-up", "64"),
            ("跌停家数", "limit-down", "13"),
        ],
        "limit-down",
    );
    let mut missing_field_set = limits.clone();
    missing_field_set
        .as_object_mut()
        .unwrap()
        .remove("fieldSet");
    let mut contradictory_total = directional.clone();
    contradictory_total["rawTable"]["listed"] = json!(["5539"]);

    for tables in [
        vec![directional.clone()],
        vec![directional.clone(), limits.clone(), limits.clone()],
        vec![directional.clone(), missing_field_set],
        vec![contradictory_total, limits],
    ] {
        let client = EastmoneyMxClient::with_transport(
            "mkt_test_key",
            FixtureTransport::new(vec![envelope("breadth-request", tables)]),
        )
        .unwrap();
        assert!(matches!(
            client.diagnose_market_breadth(&IsoDate::new("2026-08-14").unwrap()),
            Err(EastmoneyError::Protocol(_))
        ));
    }
}

#[test]
fn daily_fund_flow_is_bounded_and_reordered_oldest_first() {
    let labels = [
        ("(区间)主力净流入资金", "main", ["30", "20", "10"]),
        ("(区间)超大单净流入资金", "super", ["3", "2", "1"]),
        ("(区间)大单净流入资金", "large", ["6", "4", "2"]),
        ("(区间)中单净流入资金", "medium", ["-3", "-2", "-1"]),
        ("(区间)小单净流入资金", "small", ["-6", "-4", "-2"]),
    ];
    let raw = labels
        .iter()
        .map(|(_, code, values)| ((*code).to_owned(), json!(values)))
        .chain(std::iter::once((
            "headName".to_owned(),
            json!(["2026-08-14", "2026-08-13", "2026-08-12"]),
        )))
        .collect::<serde_json::Map<_, _>>();
    let names = labels
        .iter()
        .map(|(label, code, _)| ((*code).to_owned(), json!(label)))
        .collect::<serde_json::Map<_, _>>();
    let table = json!({
        "code": "600396.SH",
        "entityName": "华电辽能(600396.SH)",
        "rawTable": raw,
        "nameMap": names,
        "field": {
            "returnCode": "main",
            "returnName": "(区间)主力净流入资金",
            "dateGranularity": "DAY",
            "unitName": "元"
        },
        "entityTagDTO": entity()
    });
    let client = EastmoneyMxClient::with_transport(
        "mkt_test_key",
        FixtureTransport::new(vec![envelope("flow-request", vec![table])]),
    )
    .unwrap();
    let request = FundFlowRequest::new(
        FlowScope::Instrument(instrument()),
        FlowInterval::Day1,
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    let batch = client.diagnose_daily_fund_flow(&request).unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].period_at.as_str(), "2026-08-13");
    assert_eq!(batch.records()[1].period_at.as_str(), "2026-08-14");
    assert_eq!(batch.records()[1].main_net.unwrap().get(), 30.0);
    assert!(!batch.quality().is_complete());
}

#[test]
fn failed_outer_status_is_not_a_successful_empty_result() {
    let mut value = envelope("failed-request", Vec::new());
    value["status"] = json!(1001);
    let client =
        EastmoneyMxClient::with_transport("mkt_test_key", FixtureTransport::new(vec![value]))
            .unwrap();
    assert!(matches!(
        client.diagnose_market_breadth(&IsoDate::new("2026-08-14").unwrap()),
        Err(EastmoneyError::Protocol(_))
    ));
}
