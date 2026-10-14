use super::super::register_eastmoney_mx_client;
use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError, EastmoneyMxClient, EastmoneyTransport};
use magic_market_service::{CanonicalPayload, Operation, OperationRegistry, QueryCommand};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// Public synthetic fixture marker, never an environment/server credential.
const FIXTURE_API_KEY: &str = "mkt_public_offline_fixture_only";
const MONEY_FLOW_BODY: &str =
    r#"{"toolQuery":"查询600396.SH最近1个交易日的主力资金净流入、超大单、大单、中单、小单净流入"}"#;
const BREADTH_BODY: &str = r#"{"toolQuery":"查询2026-08-14全部A股上市总数、上涨家数、下跌家数、平盘家数、涨停家数和跌停家数"}"#;

// Literal native envelopes, independent of the SDK parser and wire mapper.
// Distinct outer request/payload identities make batch provenance observable.
const MONEY_FLOW_SOURCE: &str = r#"{
  "success":true,"status":0,"code":0,"message":"ok",
  "requestId":"offline-money-flow-latest",
  "data":{"status":0,"code":0,"message":"OK","data":{
    "protocolType":"SEARCH_DATA","id":"offline-money-flow-payload",
    "searchDataResultDTO":{"dataTableDTOList":[{
      "code":"600396.SH","entityName":"华电辽能(600396.SH)",
      "rawTable":{
        "main":["-125000","9999"],"super":["0","1"],
        "large":["25000","2"],"medium":["-150000","3"],"small":["0","4"],
        "headName":["2026-08-14","2026-08-13"]
      },
      "nameMap":{
        "main":"(区间)主力净流入资金","super":"(区间)超大单净流入资金",
        "large":"(区间)大单净流入资金","medium":"(区间)中单净流入资金",
        "small":"(区间)小单净流入资金"
      },
      "field":{"returnCode":"main","returnName":"(区间)主力净流入资金","dateGranularity":"DAY","unitName":"元"},
      "entityTagDTO":{"entityType":"SEC","entityTypeName":"A股","className":"沪深京股票","fullName":"华电辽能","secuCode":"600396","marketChar":".SH"}
    }]}
  }}
}"#;

const BREADTH_SOURCE: &str = r#"{
  "success":true,"status":0,"code":0,"message":"ok",
  "requestId":"offline-breadth-one",
  "data":{"status":0,"code":0,"message":"OK","data":{
    "protocolType":"SEARCH_DATA","id":"offline-breadth-payload",
    "searchDataResultDTO":{"dataTableDTOList":[
      {
        "code":"001071","entityName":"全部A股(板块)",
        "rawTable":{"listed":["4.0"],"up":["2"],"down":["1"],"flat":["1"],"headName":["2026-08-14"]},
        "nameMap":{"listed":"上市股票数量","up":"上涨家数","down":"下跌家数","flat":"平盘家数"},
        "field":{"returnCode":"down","returnName":"下跌家数","dateGranularity":"DAY","unitName":null},
        "fieldSet":[
          {"returnCode":"listed","returnName":"上市股票数量","dateGranularity":"DAY","unitName":null},
          {"returnCode":"up","returnName":"上涨家数","dateGranularity":"DAY","unitName":null},
          {"returnCode":"down","returnName":"下跌家数","dateGranularity":"DAY","unitName":null},
          {"returnCode":"flat","returnName":"平盘家数","dateGranularity":"DAY","unitName":null}
        ],
        "entityTagDTO":{"entityType":"BLOCK","entityTypeName":"BLOCK","className":"市场类(沪深京)","fullName":"全部A股"}
      },
      {
        "code":"001071","entityName":"全部A股(板块)",
        "rawTable":{"limit-up":["1"],"limit-down":["0"],"headName":["2026-08-14"]},
        "nameMap":{"limit-up":"涨停家数","limit-down":"跌停家数"},
        "field":{"returnCode":"limit-down","returnName":"跌停家数","dateGranularity":"DAY","unitName":null},
        "fieldSet":[
          {"returnCode":"limit-up","returnName":"涨停家数","dateGranularity":"DAY","unitName":null},
          {"returnCode":"limit-down","returnName":"跌停家数","dateGranularity":"DAY","unitName":null}
        ],
        "entityTagDTO":{"entityType":"BLOCK","entityTypeName":"BLOCK","className":"市场类(沪深京)","fullName":"全部A股"}
      }
    ]}
  }}
}"#;

#[derive(Debug, PartialEq, Eq)]
struct MxPostTrace {
    endpoint: String,
    headers: BTreeMap<String, String>,
    body: String,
    maximum_bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum MxHttpCall {
    Get,
    Post(MxPostTrace),
}

fn expected_mx_headers() -> BTreeMap<String, String> {
    [
        ("Accept", "application/json"),
        ("apikey", "mkt_public_offline_fixture_only"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect()
}

// Only the external HTTP boundary is replaced. The concrete SDK creates and
// parses its own request/response; no client/provider/parser/registrar is mocked.
struct MxFixture {
    calls: Arc<Mutex<Vec<MxHttpCall>>>,
    expected_body: &'static str,
    response: &'static str,
}

impl EastmoneyTransport for MxFixture {
    fn get(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.calls.lock().unwrap().push(MxHttpCall::Get);
        Err(EastmoneyError::Protocol(
            "Miaoxiang fixture does not serve GET".into(),
        ))
    }

    fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        let mismatch = || EastmoneyError::Protocol("Miaoxiang fixture request mismatch".into());
        if url != "https://mkapi2.dfcfs.com/finskillshub/api/claw/query"
            || maximum_bytes != 4_194_304
            || body != self.expected_body.as_bytes()
        {
            return Err(mismatch());
        }
        let mut actual_headers = BTreeMap::new();
        for (key, value) in headers {
            if actual_headers
                .insert((*key).to_owned(), (*value).to_owned())
                .is_some()
            {
                return Err(mismatch());
            }
        }
        if actual_headers != expected_mx_headers() {
            return Err(mismatch());
        }
        let mut calls = self.calls.lock().unwrap();
        if !calls.is_empty() {
            return Err(EastmoneyError::Protocol(
                "Miaoxiang fixture permits exactly one HTTP call".into(),
            ));
        }
        calls.push(MxHttpCall::Post(MxPostTrace {
            endpoint: url.to_owned(),
            headers: actual_headers,
            body: String::from_utf8(body.to_vec()).map_err(|_| mismatch())?,
            maximum_bytes,
        }));
        Ok(self.response.as_bytes().to_vec())
    }
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn assert_bracketed_observation(observed: &str, before: u128, after: u128) {
    let suffix = observed.strip_prefix("unix-ms:").unwrap();
    assert!(!suffix.is_empty());
    assert!(suffix.bytes().all(|byte| byte.is_ascii_digit()));
    let millis = suffix.parse::<u128>().unwrap();
    assert!(before <= millis && millis <= after);
}

#[test]
fn registered_miaoxiang_latest_money_flows_preserve_signed_cny_without_admission() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let client = EastmoneyClient::with_transport(MxFixture {
        calls: Arc::clone(&calls),
        expected_body: MONEY_FLOW_BODY,
        response: MONEY_FLOW_SOURCE,
    });
    let mx = EastmoneyMxClient::with_client(FIXTURE_API_KEY, &client).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
    // The proposed real production registrar is setup only; Execute is the entry.
    register_eastmoney_mx_client(&mut registry, &mx, 4096).unwrap();
    let command = QueryCommand::new(
        "offline-miaoxiang-latest-money-flows",
        Operation::MoneyFlows,
        Some("EastmoneyMiaoxiang".into()),
        CanonicalPayload::new(
            "magic.market.money_flows.request",
            1,
            br#"{"instruments":[{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}]}"#
                .to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap()
    .with_unadmitted_access(true);
    let before = unix_millis();
    let result = registry.execute(command).unwrap();
    let after = unix_millis();

    assert_eq!(result.provider, "EastmoneyMiaoxiang");
    assert!(!result.repository_admitted);
    assert!(!result.complete);
    assert_eq!(
        result.diagnostic_blocker.as_deref(),
        Some("source methodology and serial live stability remain repository-unadmitted")
    );
    assert_eq!(result.source_at.as_deref(), Some("2026-08-14"));
    assert_eq!(
        result.batch_id,
        "eastmoney-mx:daily-fund-flow:offline-money-flow-latest"
    );
    assert_bracketed_observation(&result.observed_at, before, after);
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.money_flow");
    assert_eq!(payload.schema_version(), 1);
    let actual_wire: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    // Only the independently bracketed observation is dynamic. All business
    // values/date/status/provider/batch identity are independently literal.
    let expected_wire = serde_json::json!({
        "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
        "main_net":-125000.0,
        "super_large_net":0.0,
        "large_net":25000.0,
        "medium_net":-150000.0,
        "small_net":0.0,
        "status":"Available",
        "source_at":"2026-08-14",
        "observed_at":result.observed_at.as_str(),
        "provider":"Eastmoney",
        "batch_id":"eastmoney-mx:daily-fund-flow:offline-money-flow-latest"
    });
    assert_eq!(actual_wire, expected_wire);
    assert_eq!(
        *calls.lock().unwrap(),
        vec![MxHttpCall::Post(MxPostTrace {
            endpoint: "https://mkapi2.dfcfs.com/finskillshub/api/claw/query".into(),
            headers: expected_mx_headers(),
            body: r#"{"toolQuery":"查询600396.SH最近1个交易日的主力资金净流入、超大单、大单、中单、小单净流入"}"#.into(),
            maximum_bytes: 4_194_304,
        })]
    );
}

#[test]
fn registered_miaoxiang_breadth_preserves_counts_and_unknown_source_skew() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let client = EastmoneyClient::with_transport(MxFixture {
        calls: Arc::clone(&calls),
        expected_body: BREADTH_BODY,
        response: BREADTH_SOURCE,
    });
    let mx = EastmoneyMxClient::with_client(FIXTURE_API_KEY, &client).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
    register_eastmoney_mx_client(&mut registry, &mx, 4096).unwrap();
    let command = QueryCommand::new(
        "offline-miaoxiang-breadth",
        Operation::MarketBreadth,
        Some("EastmoneyMiaoxiang".into()),
        CanonicalPayload::new(
            "magic.market.market_breadth.request",
            1,
            br#"{"source_date":"2026-08-14"}"#.to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap();
    // No unadmitted opt-in: this is the existing admitted Breadth registration.
    let before = unix_millis();
    let result = registry.execute(command).unwrap();
    let after = unix_millis();

    assert_eq!(result.provider, "EastmoneyMiaoxiang");
    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at.as_deref(), Some("2026-08-14"));
    assert_eq!(
        result.batch_id,
        "eastmoney-mx:market-breadth:offline-breadth-one"
    );
    assert_bracketed_observation(&result.observed_at, before, after);
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.market_breadth_diagnostic");
    assert_eq!(payload.schema_version(), 1);
    let actual_wire: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    let expected_wire = serde_json::json!({
        "universe":"all_a_shares",
        "source_date":"2026-08-14",
        "source_session":"PostClose",
        "listed_total":4,
        "valid":4,
        "up":2,
        "down":1,
        "flat":1,
        "limit_up":1,
        "limit_down":0,
        "coverage":{"value":1.0,"unit":"Decimal"},
        "maximum_source_skew_millis":null,
        "status":"Available",
        "evidence":{
            "provider":"Eastmoney",
            "source_at":"2026-08-14",
            "observed_at":result.observed_at.as_str(),
            "batch_id":"eastmoney-mx:market-breadth:offline-breadth-one"
        }
    });
    assert_eq!(actual_wire, expected_wire);
    assert_eq!(
        *calls.lock().unwrap(),
        vec![MxHttpCall::Post(MxPostTrace {
            endpoint: "https://mkapi2.dfcfs.com/finskillshub/api/claw/query".into(),
            headers: expected_mx_headers(),
            body: r#"{"toolQuery":"查询2026-08-14全部A股上市总数、上涨家数、下跌家数、平盘家数、涨停家数和跌停家数"}"#.into(),
            maximum_bytes: 4_194_304,
        })]
    );
}

#[test]
fn registered_miaoxiang_fund_flow_series_preserves_oldest_first_dates_and_signed_cny_without_admission(
) {
    const BODY: &str = r#"{"toolQuery":"查询600396.SH最近2个交易日的主力资金净流入、超大单、大单、中单、小单净流入"}"#;
    // Native rows are newest-first; identities and all business values are literal.
    const SOURCE: &str = r#"{
      "success":true,"status":0,"code":0,"message":"ok",
      "requestId":"offline-series-two",
      "data":{"status":0,"code":0,"message":"OK","data":{
        "protocolType":"SEARCH_DATA","id":"offline-series-payload",
        "searchDataResultDTO":{"dataTableDTOList":[{
          "code":"600396.SH","entityName":"华电辽能(600396.SH)",
          "rawTable":{
            "main":["-125000","0"],"super":["0","100000"],
            "large":["-125000","-100000"],"medium":["25000","-250"],
            "small":["100000","250"],
            "headName":["2026-08-14","2026-08-13"]
          },
          "nameMap":{
            "main":"(区间)主力净流入资金","super":"(区间)超大单净流入资金",
            "large":"(区间)大单净流入资金","medium":"(区间)中单净流入资金",
            "small":"(区间)小单净流入资金"
          },
          "field":{"returnCode":"main","returnName":"(区间)主力净流入资金","dateGranularity":"DAY","unitName":"元"},
          "entityTagDTO":{"entityType":"SEC","entityTypeName":"A股","className":"沪深京股票","fullName":"华电辽能","secuCode":"600396","marketChar":".SH"}
        }]}
      }}
    }"#;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let client = EastmoneyClient::with_transport(MxFixture {
        calls: Arc::clone(&calls),
        expected_body: BODY,
        response: SOURCE,
    });
    let mx = EastmoneyMxClient::with_client(FIXTURE_API_KEY, &client).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
    // Actual concrete production registrar is setup only; Execute is the entry.
    register_eastmoney_mx_client(&mut registry, &mx, 4096).unwrap();
    let command = QueryCommand::new(
        "offline-miaoxiang-fund-flow-series",
        Operation::FundFlowSeries,
        Some("EastmoneyMiaoxiang".into()),
        CanonicalPayload::new(
            "magic.market.fund_flow_series.request",
            1,
            br#"{"scope":{"Instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}},"interval":"Day1","limit":2}"#.to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap()
    .with_unadmitted_access(true);
    let before = unix_millis();
    let result = registry.execute(command).unwrap();
    let after = unix_millis();

    assert_eq!(result.provider, "EastmoneyMiaoxiang");
    assert!(!result.repository_admitted);
    assert!(!result.complete);
    assert_eq!(
        result.diagnostic_blocker.as_deref(),
        Some("natural-language result cardinality and serial live stability remain repository-unadmitted")
    );
    assert_eq!(result.source_at.as_deref(), Some("2026-08-14"));
    assert_eq!(
        result.batch_id,
        "eastmoney-mx:daily-fund-flow:offline-series-two"
    );
    assert_bracketed_observation(&result.observed_at, before, after);
    assert_eq!(result.records.len(), 2);
    for payload in &result.records {
        assert_eq!(payload.schema(), "magic.market.fund_flow_point");
        assert_eq!(payload.schema_version(), 1);
    }
    let actual_wire: Vec<serde_json::Value> = result
        .records
        .iter()
        .map(|payload| serde_json::from_slice(payload.data()).unwrap())
        .collect();
    // Full literal wire oracle: older date first, null ratio, signed/zero CNY,
    // and distinct per-row source dates. Only observation is bracketed/dynamic.
    let expected_wire = vec![
        serde_json::json!({
            "scope":{"Instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}},
            "interval":"Day1",
            "period_at":"2026-08-13",
            "main_net":0.0,
            "main_ratio":null,
            "super_large_net":100000.0,
            "large_net":-100000.0,
            "medium_net":-250.0,
            "small_net":250.0,
            "evidence":{
                "provider":"Eastmoney",
                "source_at":"2026-08-13",
                "observed_at":result.observed_at.as_str(),
                "batch_id":"eastmoney-mx:daily-fund-flow:offline-series-two"
            }
        }),
        serde_json::json!({
            "scope":{"Instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}},
            "interval":"Day1",
            "period_at":"2026-08-14",
            "main_net":-125000.0,
            "main_ratio":null,
            "super_large_net":0.0,
            "large_net":-125000.0,
            "medium_net":25000.0,
            "small_net":100000.0,
            "evidence":{
                "provider":"Eastmoney",
                "source_at":"2026-08-14",
                "observed_at":result.observed_at.as_str(),
                "batch_id":"eastmoney-mx:daily-fund-flow:offline-series-two"
            }
        }),
    ];
    assert_eq!(actual_wire, expected_wire);
    assert_eq!(
        *calls.lock().unwrap(),
        vec![MxHttpCall::Post(MxPostTrace {
            endpoint: "https://mkapi2.dfcfs.com/finskillshub/api/claw/query".into(),
            headers: expected_mx_headers(),
            body: r#"{"toolQuery":"查询600396.SH最近2个交易日的主力资金净流入、超大单、大单、中单、小单净流入"}"#.into(),
            maximum_bytes: 4_194_304,
        })]
    );
}

#[test]
fn registered_miaoxiang_breadth_rejects_limits_above_directional_counts() {
    // Independent native envelopes: the source grammar is valid in both.
    // The failure changes only the limit-up count and valid response identities.
    const EQUAL_LIMIT_SOURCE: &str = r#"{
      "success":true,"status":0,"code":0,"message":"ok",
      "requestId":"offline-breadth-subset-boundary",
      "data":{"status":0,"code":0,"message":"OK","data":{
        "protocolType":"SEARCH_DATA","id":"offline-breadth-subset-boundary-payload",
        "searchDataResultDTO":{"dataTableDTOList":[
          {
            "code":"001071","entityName":"全部A股(板块)",
            "rawTable":{"listed":["4.0"],"up":["2"],"down":["1"],"flat":["1"],"headName":["2026-08-14"]},
            "nameMap":{"listed":"上市股票数量","up":"上涨家数","down":"下跌家数","flat":"平盘家数"},
            "field":{"returnCode":"down","returnName":"下跌家数","dateGranularity":"DAY","unitName":null},
            "fieldSet":[
              {"returnCode":"listed","returnName":"上市股票数量","dateGranularity":"DAY","unitName":null},
              {"returnCode":"up","returnName":"上涨家数","dateGranularity":"DAY","unitName":null},
              {"returnCode":"down","returnName":"下跌家数","dateGranularity":"DAY","unitName":null},
              {"returnCode":"flat","returnName":"平盘家数","dateGranularity":"DAY","unitName":null}
            ],
            "entityTagDTO":{"entityType":"BLOCK","entityTypeName":"BLOCK","className":"市场类(沪深京)","fullName":"全部A股"}
          },
          {
            "code":"001071","entityName":"全部A股(板块)",
            "rawTable":{"limit-up":["2"],"limit-down":["0"],"headName":["2026-08-14"]},
            "nameMap":{"limit-up":"涨停家数","limit-down":"跌停家数"},
            "field":{"returnCode":"limit-down","returnName":"跌停家数","dateGranularity":"DAY","unitName":null},
            "fieldSet":[
              {"returnCode":"limit-up","returnName":"涨停家数","dateGranularity":"DAY","unitName":null},
              {"returnCode":"limit-down","returnName":"跌停家数","dateGranularity":"DAY","unitName":null}
            ],
            "entityTagDTO":{"entityType":"BLOCK","entityTypeName":"BLOCK","className":"市场类(沪深京)","fullName":"全部A股"}
          }
        ]}
      }}
    }"#;
    const INVALID_SUBSET_SOURCE: &str = r#"{
      "success":true,"status":0,"code":0,"message":"ok",
      "requestId":"offline-breadth-subset-error",
      "data":{"status":0,"code":0,"message":"OK","data":{
        "protocolType":"SEARCH_DATA","id":"offline-breadth-subset-error-payload",
        "searchDataResultDTO":{"dataTableDTOList":[
          {
            "code":"001071","entityName":"全部A股(板块)",
            "rawTable":{"listed":["4.0"],"up":["2"],"down":["1"],"flat":["1"],"headName":["2026-08-14"]},
            "nameMap":{"listed":"上市股票数量","up":"上涨家数","down":"下跌家数","flat":"平盘家数"},
            "field":{"returnCode":"down","returnName":"下跌家数","dateGranularity":"DAY","unitName":null},
            "fieldSet":[
              {"returnCode":"listed","returnName":"上市股票数量","dateGranularity":"DAY","unitName":null},
              {"returnCode":"up","returnName":"上涨家数","dateGranularity":"DAY","unitName":null},
              {"returnCode":"down","returnName":"下跌家数","dateGranularity":"DAY","unitName":null},
              {"returnCode":"flat","returnName":"平盘家数","dateGranularity":"DAY","unitName":null}
            ],
            "entityTagDTO":{"entityType":"BLOCK","entityTypeName":"BLOCK","className":"市场类(沪深京)","fullName":"全部A股"}
          },
          {
            "code":"001071","entityName":"全部A股(板块)",
            "rawTable":{"limit-up":["3"],"limit-down":["0"],"headName":["2026-08-14"]},
            "nameMap":{"limit-up":"涨停家数","limit-down":"跌停家数"},
            "field":{"returnCode":"limit-down","returnName":"跌停家数","dateGranularity":"DAY","unitName":null},
            "fieldSet":[
              {"returnCode":"limit-up","returnName":"涨停家数","dateGranularity":"DAY","unitName":null},
              {"returnCode":"limit-down","returnName":"跌停家数","dateGranularity":"DAY","unitName":null}
            ],
            "entityTagDTO":{"entityType":"BLOCK","entityTypeName":"BLOCK","className":"市场类(沪深京)","fullName":"全部A股"}
          }
        ]}
      }}
    }"#;

    // Setup only: both executions use the same real SDK and production registrar.
    let registry_for = |response: &'static str, calls: Arc<Mutex<Vec<MxHttpCall>>>| {
        let client = EastmoneyClient::with_transport(MxFixture {
            calls,
            expected_body: BREADTH_BODY,
            response,
        });
        let mx = EastmoneyMxClient::with_client(FIXTURE_API_KEY, &client).unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
        register_eastmoney_mx_client(&mut registry, &mx, 4096).unwrap();
        registry
    };
    let command_for = |request_id: &str| {
        QueryCommand::new(
            request_id,
            Operation::MarketBreadth,
            Some("EastmoneyMiaoxiang".into()),
            CanonicalPayload::new(
                "magic.market.market_breadth.request",
                1,
                br#"{"source_date":"2026-08-14"}"#.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap()
    };

    // Equality is valid; no unadmitted opt-in or private validator is used.
    let control_calls = Arc::new(Mutex::new(Vec::new()));
    let control_registry = registry_for(EQUAL_LIMIT_SOURCE, Arc::clone(&control_calls));
    let before = unix_millis();
    let control = control_registry
        .execute(command_for("offline-miaoxiang-breadth-subset-control"))
        .unwrap();
    let after = unix_millis();
    assert_eq!(control.provider, "EastmoneyMiaoxiang");
    assert!(control.repository_admitted);
    assert!(control.complete);
    assert_eq!(control.diagnostic_blocker, None);
    assert_eq!(control.source_at.as_deref(), Some("2026-08-14"));
    assert_eq!(
        control.batch_id,
        "eastmoney-mx:market-breadth:offline-breadth-subset-boundary"
    );
    assert_bracketed_observation(&control.observed_at, before, after);
    assert_eq!(control.records.len(), 1);
    assert_eq!(
        control.records[0].schema(),
        "magic.market.market_breadth_diagnostic"
    );
    assert_eq!(control.records[0].schema_version(), 1);
    let control_wire: serde_json::Value =
        serde_json::from_slice(control.records[0].data()).unwrap();
    assert_eq!(
        control_wire,
        serde_json::json!({
            "universe":"all_a_shares",
            "source_date":"2026-08-14",
            "source_session":"PostClose",
            "listed_total":4,
            "valid":4,
            "up":2,
            "down":1,
            "flat":1,
            "limit_up":2,
            "limit_down":0,
            "coverage":{"value":1.0,"unit":"Decimal"},
            "maximum_source_skew_millis":null,
            "status":"Available",
            "evidence":{
                "provider":"Eastmoney",
                "source_at":"2026-08-14",
                "observed_at":control.observed_at.as_str(),
                "batch_id":"eastmoney-mx:market-breadth:offline-breadth-subset-boundary"
            }
        })
    );

    let rejected_calls = Arc::new(Mutex::new(Vec::new()));
    let rejected_registry = registry_for(INVALID_SUBSET_SOURCE, Arc::clone(&rejected_calls));
    match rejected_registry.execute(command_for("offline-miaoxiang-breadth-subset-rejection")) {
        Err(magic_market_service::ServiceError::FailedPrecondition(reason)) => assert_eq!(
            reason,
            "Eastmoney protocol error: Miaoxiang limit counts contradict directional counts"
        ),
        Err(other) => panic!("expected the directional-subset source rejection, got {other}"),
        Ok(_) => panic!("contradictory source counts must not publish a partial or empty result"),
    }

    // Complete external trace for each independent fixture; no GET/fallback/retry.
    for calls in [control_calls, rejected_calls] {
        assert_eq!(
            *calls.lock().unwrap(),
            vec![MxHttpCall::Post(MxPostTrace {
                endpoint: "https://mkapi2.dfcfs.com/finskillshub/api/claw/query".into(),
                headers: expected_mx_headers(),
                body: r#"{"toolQuery":"查询2026-08-14全部A股上市总数、上涨家数、下跌家数、平盘家数、涨停家数和跌停家数"}"#.into(),
                maximum_bytes: 4_194_304,
            })]
        );
    }
}

#[derive(Debug, PartialEq, Eq)]
enum MxIntegrityAttempt {
    Get {
        endpoint: String,
        headers: Vec<(String, String)>,
        maximum_bytes: usize,
    },
    Post {
        endpoint: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
        maximum_bytes: usize,
    },
}

// Owns only external response bytes and records every external attempt before
// delegating to the unchanged guard. The existing success trace stays separate.
struct MxIntegrityFixture {
    request_guard: MxFixture,
    response: Vec<u8>,
    attempts: Arc<Mutex<Vec<MxIntegrityAttempt>>>,
}

impl EastmoneyTransport for MxIntegrityFixture {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.attempts.lock().unwrap().push(MxIntegrityAttempt::Get {
            endpoint: url.to_owned(),
            headers: headers
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
            maximum_bytes,
        });
        self.request_guard.get(url, headers, maximum_bytes)
    }

    fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.attempts
            .lock()
            .unwrap()
            .push(MxIntegrityAttempt::Post {
                endpoint: url.to_owned(),
                headers: headers
                    .iter()
                    .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                    .collect(),
                body: body.to_vec(),
                maximum_bytes,
            });
        self.request_guard
            .post_json(url, headers, body, maximum_bytes)?;
        Ok(self.response.clone())
    }
}

#[test]
fn registered_miaoxiang_raw_response_evidence_is_required_before_atomic_publication() {
    use magic_market_service::ServiceError;
    use serde_json::{json, Value};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Family {
        Flow,
        Breadth,
        Auction,
    }

    const FLOW_BODY: &str = r#"{"toolQuery":"查询600396.SH最近2个交易日的主力资金净流入、超大单、大单、中单、小单净流入"}"#;
    const AUCTION_BODY: &str =
        r#"{"toolQuery":"查询600396.SH在2026-08-14的开盘集合竞价成交量和开盘集合竞价成交额"}"#;
    const FLOW_SOURCE: &str = r#"{
      "success":true,"status":0,"code":0,"message":"ok",
      "requestId":"matrix-flow-control",
      "data":{"status":0,"code":0,"message":"OK","data":{
        "protocolType":"SEARCH_DATA","id":"matrix-flow-payload",
        "searchDataResultDTO":{"dataTableDTOList":[{
          "code":"600396.SH","entityName":"华电辽能(600396.SH)",
          "rawTable":{
            "main":["-125000","0","9999"],"super":["0","100000","7"],
            "large":["-125000","-100000","8"],"medium":["25000","-250","9"],
            "small":["100000","250","10"],
            "headName":["2026-08-14","2026-08-13","2026-08-12"]
          },
          "nameMap":{
            "main":"(区间)主力净流入资金","super":"(区间)超大单净流入资金",
            "large":"(区间)大单净流入资金","medium":"(区间)中单净流入资金",
            "small":"(区间)小单净流入资金"
          },
          "field":{"returnCode":"main","returnName":"(区间)主力净流入资金","dateGranularity":"DAY","unitName":"元"},
          "entityTagDTO":{"entityType":"SEC","entityTypeName":"A股","className":"沪深京股票","fullName":"华电辽能","secuCode":"600396","marketChar":".SH"}
        }]}
      }}
    }"#;
    // Exact representable integer boundary and a zero monetary amount. Every
    // unsupported Level-2 observation stays null in the independent wire oracle.
    const AUCTION_SOURCE: &str = r#"{
      "success":true,"status":0,"code":0,"message":"ok",
      "requestId":"matrix-auction-control",
      "data":{"status":0,"code":0,"message":"OK","data":{
        "protocolType":"SEARCH_DATA","id":"matrix-auction-payload",
        "searchDataResultDTO":{"dataTableDTOList":[{
          "code":"600396.SH","entityName":"华电辽能(600396.SH)",
          "rawTable":{"vol":["9007199254740992"],"amt":["0"],"headName":["2026-08-14"]},
          "nameMap":{"vol":"开盘集合竞价成交量","amt":"开盘集合竞价成交额"},
          "field":{"returnCode":"vol","returnName":"开盘集合竞价成交量","dateGranularity":"DAY","unitName":"股"},
          "fieldSet":[
            {"returnCode":"vol","returnName":"开盘集合竞价成交量","dateGranularity":"DAY","unitName":"股"},
            {"returnCode":"amt","returnName":"开盘集合竞价成交额","dateGranularity":"DAY","unitName":"元"}
          ],
          "entityTagDTO":{"entityType":"SEC","entityTypeName":"A股","className":"沪深京股票","fullName":"华电辽能","secuCode":"600396","marketChar":".SH"}
        }]}
      }}
    }"#;

    let seed = |family| -> Value {
        serde_json::from_str(match family {
            Family::Flow => FLOW_SOURCE,
            Family::Breadth => BREADTH_SOURCE,
            Family::Auction => AUCTION_SOURCE,
        })
        .unwrap()
    };
    let expected_body = |family| match family {
        Family::Flow => FLOW_BODY,
        Family::Breadth => BREADTH_BODY,
        Family::Auction => AUCTION_BODY,
    };
    let execute = |family, source: &Value| {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let client = EastmoneyClient::with_transport(MxIntegrityFixture {
            request_guard: MxFixture {
                calls: Arc::clone(&calls),
                expected_body: expected_body(family),
                // Only the guard's request checks/trace are used; this reply is
                // never passed to the real SDK. The owned response is above.
                response: "",
            },
            response: serde_json::to_vec(source).unwrap(),
            attempts: Arc::clone(&attempts),
        });
        let mx = EastmoneyMxClient::with_client(FIXTURE_API_KEY, &client).unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
        // Actual registrar is setup only. Public Execute is the sole business seam.
        register_eastmoney_mx_client(&mut registry, &mx, 4096).unwrap();
        let (operation, schema, wire): (_, _, &[u8]) = match family {
            Family::Flow => (
                Operation::FundFlowSeries,
                "magic.market.fund_flow_series.request",
                br#"{"scope":{"Instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}},"interval":"Day1","limit":2}"#,
            ),
            Family::Breadth => (
                Operation::MarketBreadth,
                "magic.market.market_breadth.request",
                br#"{"source_date":"2026-08-14"}"#,
            ),
            Family::Auction => (
                Operation::Auctions,
                "magic.market.auctions.request",
                br#"{"instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-08-14"}"#,
            ),
        };
        let command = QueryCommand::new(
            "miaoxiang-response-integrity-matrix",
            operation,
            Some("EastmoneyMiaoxiang".into()),
            CanonicalPayload::new(schema, 1, wire.to_vec(), 4096).unwrap(),
        )
        .unwrap();
        let command = if family == Family::Breadth {
            command
        } else {
            command.with_unadmitted_access(true)
        };
        let before = unix_millis();
        let result = registry.execute(command);
        let after = unix_millis();
        (result, calls, attempts, before, after)
    };
    let assert_one_post = |family, calls: &Arc<Mutex<Vec<MxHttpCall>>>| {
        assert_eq!(
            *calls.lock().unwrap(),
            vec![MxHttpCall::Post(MxPostTrace {
                endpoint: "https://mkapi2.dfcfs.com/finskillshub/api/claw/query".into(),
                headers: expected_mx_headers(),
                body: expected_body(family).into(),
                maximum_bytes: 4_194_304,
            })]
        );
    };

    let assert_one_attempt = |family, attempts: &Arc<Mutex<Vec<MxIntegrityAttempt>>>| {
        assert_eq!(
            *attempts.lock().unwrap(),
            vec![MxIntegrityAttempt::Post {
                endpoint: "https://mkapi2.dfcfs.com/finskillshub/api/claw/query".into(),
                headers: vec![
                    ("Accept".into(), "application/json".into()),
                    ("apikey".into(), "mkt_public_offline_fixture_only".into()),
                ],
                body: expected_body(family).as_bytes().to_vec(),
                maximum_bytes: 4_194_304,
            }]
        );
    };

    // Independent, same-wiring valid controls run before the defect matrix.
    // Only observed_at is dynamic and independently bracketed; no business
    // expected value is read from the SDK's result or raw-response parser.
    for family in [Family::Flow, Family::Breadth, Family::Auction] {
        let (result, calls, attempts, before, after) = execute(family, &seed(family));
        let result = result.unwrap();
        assert_one_post(family, &calls);
        assert_one_attempt(family, &attempts);
        assert_eq!(result.provider, "EastmoneyMiaoxiang");
        assert_eq!(result.source_at.as_deref(), Some("2026-08-14"));
        assert_bracketed_observation(&result.observed_at, before, after);
        let (schema, batch, blocker) = match family {
            Family::Flow => (
                "magic.market.fund_flow_point",
                "eastmoney-mx:daily-fund-flow:matrix-flow-control",
                Some("natural-language result cardinality and serial live stability remain repository-unadmitted"),
            ),
            Family::Breadth => (
                "magic.market.market_breadth_diagnostic",
                "eastmoney-mx:market-breadth:offline-breadth-one",
                None,
            ),
            Family::Auction => (
                "magic.market.opening_auction_diagnostic",
                "eastmoney-mx:opening-auction:matrix-auction-control",
                Some("natural-language answer cardinality is not stable within one session; the admitted one-table shape was not reproduced on 2026-09-21"),
            ),
        };
        assert_eq!(result.batch_id, batch);
        assert_eq!(result.diagnostic_blocker.as_deref(), blocker);
        assert_eq!(result.repository_admitted, family == Family::Breadth);
        assert_eq!(result.complete, family == Family::Breadth);
        for record in &result.records {
            assert_eq!(record.schema(), schema);
            assert_eq!(record.schema_version(), 1);
        }
        let wire: Vec<Value> = result
            .records
            .iter()
            .map(|record| serde_json::from_slice(record.data()).unwrap())
            .collect();
        let expected = match family {
            Family::Flow => vec![
                json!({
                    "scope":{"Instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}},
                    "interval":"Day1","period_at":"2026-08-13","main_net":0.0,"main_ratio":null,
                    "super_large_net":100000.0,"large_net":-100000.0,"medium_net":-250.0,"small_net":250.0,
                    "evidence":{"provider":"Eastmoney","source_at":"2026-08-13",
                        "observed_at":result.observed_at.as_str(),"batch_id":"eastmoney-mx:daily-fund-flow:matrix-flow-control"}
                }),
                json!({
                    "scope":{"Instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}},
                    "interval":"Day1","period_at":"2026-08-14","main_net":-125000.0,"main_ratio":null,
                    "super_large_net":0.0,"large_net":-125000.0,"medium_net":25000.0,"small_net":100000.0,
                    "evidence":{"provider":"Eastmoney","source_at":"2026-08-14",
                        "observed_at":result.observed_at.as_str(),"batch_id":"eastmoney-mx:daily-fund-flow:matrix-flow-control"}
                }),
            ],
            Family::Breadth => vec![json!({
                "universe":"all_a_shares","source_date":"2026-08-14","source_session":"PostClose",
                "listed_total":4,"valid":4,"up":2,"down":1,"flat":1,"limit_up":1,"limit_down":0,
                "coverage":{"value":1.0,"unit":"Decimal"},"maximum_source_skew_millis":null,"status":"Available",
                "evidence":{"provider":"Eastmoney","source_at":"2026-08-14",
                    "observed_at":result.observed_at.as_str(),"batch_id":"eastmoney-mx:market-breadth:offline-breadth-one"}
            })],
            Family::Auction => vec![json!({
                "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
                "name":"华电辽能","trading_date":"2026-08-14",
                "matched_price":null,"previous_close":null,"change_percent":null,
                "matched_quantity_shares":9007199254740992.0,"matched_amount_cny":0.0,
                "unmatched_bid_quantity_shares":null,"unmatched_ask_quantity_shares":null,"volume_ratio":null,
                "status":"Available","evidence":{"provider":"Eastmoney","source_at":"2026-08-14",
                    "observed_at":result.observed_at.as_str(),"batch_id":"eastmoney-mx:opening-auction:matrix-auction-control"}
            })],
        };
        assert_eq!(wire, expected, "same-wiring {family:?} control");
    }

    // One logical source defect per row; the full reason identifies the actual
    // rejection stage, so an early malformed envelope cannot satisfy a later
    // row/field oracle. Count changes keep all dated columns aligned.
    type Case = (&'static str, Family, fn(&mut Value), &'static str);
    let cases: [Case; 25] = [
        (
            "inner status", Family::Flow,
            |v| v["data"]["status"] = json!(1),
            "Eastmoney protocol error: Miaoxiang inner status failed: status=1, code=0, message=\"OK\"",
        ),
        (
            "protocol", Family::Flow,
            |v| v["data"]["data"]["protocolType"] = json!("TEST_OTHER"),
            "Eastmoney protocol error: Miaoxiang protocol type \"TEST_OTHER\" is not SEARCH_DATA",
        ),
        (
            "outer identity", Family::Flow,
            |v| v["requestId"] = json!("/"),
            "Eastmoney protocol error: Miaoxiang outer requestId is invalid",
        ),
        (
            "payload identity", Family::Flow,
            |v| v["data"]["data"]["id"] = json!("/"),
            "Eastmoney protocol error: Miaoxiang payload id is invalid",
        ),
        (
            "no matching table", Family::Flow,
            |v| { v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["nameMap"].as_object_mut().unwrap().remove("main"); },
            "Eastmoney protocol error: Miaoxiang fund-flow returned 0 matching tables, expected exactly 1",
        ),
        (
            "two matching tables", Family::Flow,
            |v| {
                let tables = v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"].as_array_mut().unwrap();
                tables.push(tables[0].clone());
            },
            "Eastmoney protocol error: Miaoxiang fund-flow returned 2 matching tables, expected exactly 1",
        ),
        (
            "too few aligned rows", Family::Flow,
            |v| {
                for column in v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"].as_object_mut().unwrap().values_mut() {
                    column.as_array_mut().unwrap().truncate(1);
                }
            },
            "Eastmoney protocol error: Miaoxiang fund-flow returned 1 dates for requested limit 2",
        ),
        (
            "too many aligned rows", Family::Flow,
            |v| {
                for (key, column) in v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"].as_object_mut().unwrap() {
                    if key == "headName" {
                        *column = json!(["2026-08-14","2026-08-13","2026-08-12","2026-08-11","2026-08-10"]);
                    } else {
                        column.as_array_mut().unwrap().extend([json!("0"), json!("0")]);
                    }
                }
            },
            "Eastmoney protocol error: Miaoxiang fund-flow returned 5 dates for requested limit 2",
        ),
        (
            "equal dates", Family::Flow,
            |v| v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"]["headName"][1] = json!("2026-08-14"),
            "Eastmoney protocol error: Miaoxiang fund-flow dates must be unique newest-first",
        ),
        (
            "duplicate label", Family::Flow,
            |v| {
                let table = &mut v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0];
                table["nameMap"]["extra"] = json!("(区间)主力净流入资金");
                table["rawTable"]["extra"] = table["rawTable"]["main"].clone();
            },
            "Eastmoney protocol error: Miaoxiang table has 2 keys for \"(区间)主力净流入资金\", expected exactly 1",
        ),
        (
            "missing column", Family::Flow,
            |v| { v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"].as_object_mut().unwrap().remove("large"); },
            "Eastmoney protocol error: Miaoxiang raw table has no column \"large\"",
        ),
        (
            "full received column length", Family::Flow,
            |v| { v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"]["large"].as_array_mut().unwrap().pop(); },
            "Eastmoney protocol error: Miaoxiang \"(区间)大单净流入资金\" has 2 values, expected 3",
        ),
        (
            "primary unit", Family::Flow,
            |v| v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["field"]["unitName"] = json!("万元"),
            "Eastmoney protocol error: Miaoxiang field metadata does not prove \"(区间)主力净流入资金\"/元/DAY",
        ),
        (
            "security identity", Family::Flow,
            |v| v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["entityTagDTO"]["secuCode"] = json!("000001"),
            "Eastmoney protocol error: Miaoxiang security identity does not match requested 600396.SH",
        ),
        (
            "second selected value after valid first", Family::Flow,
            |v| v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"]["main"][1] = json!("not-money"),
            "Eastmoney protocol error: Miaoxiang main net is invalid: invalid float literal",
        ),
        (
            "exact all-A universe", Family::Breadth,
            |v| v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["entityTagDTO"]["className"] = json!("沪深"),
            "Eastmoney protocol error: Miaoxiang breadth universe is not the exact all-A-share aggregate",
        ),
        (
            "total fieldSet cardinality", Family::Breadth,
            |v| { v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["fieldSet"].as_array_mut().unwrap().remove(1); },
            "Eastmoney protocol error: Miaoxiang breadth fieldSet has 5 entries, expected exactly 6",
        ),
        (
            "duplicate matching fieldSet", Family::Breadth,
            |v| {
                let fields = v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["fieldSet"].as_array_mut().unwrap();
                fields[1] = fields[0].clone();
            },
            "Eastmoney protocol error: Miaoxiang fieldSet has 2 entries for \"上市股票数量\", expected exactly 1",
        ),
        (
            "same label in two tables", Family::Breadth,
            |v| {
                let table = &mut v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][1];
                table["nameMap"]["listed-mirror"] = json!("上市股票数量");
                table["rawTable"]["listed-mirror"] = json!(["4.0"]);
            },
            "Eastmoney protocol error: Miaoxiang returned 2 tables for \"上市股票数量\", expected exactly 1: untyped[下跌家数(no unit), 平盘家数(no unit), 上市股票数量(no unit), 上涨家数(no unit)] @2026-08-14; untyped[跌停家数(no unit), 涨停家数(no unit), 上市股票数量(no unit)] @2026-08-14",
        ),
        (
            "missing auction scalar", Family::Auction,
            |v| { v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"].as_object_mut().unwrap().remove("vol"); },
            "Eastmoney protocol error: Miaoxiang raw table has no column \"vol\"",
        ),
        (
            "multiple auction scalars", Family::Auction,
            |v| v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"]["vol"] = json!(["12","13"]),
            "Eastmoney protocol error: Miaoxiang \"开盘集合竞价成交量\" has 2 values, expected exactly 1",
        ),
        (
            "missing auction date", Family::Auction,
            |v| v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"]["headName"] = json!([]),
            "Eastmoney protocol error: Miaoxiang table has 0 dates, expected exactly 1",
        ),
        (
            "wrong auction date", Family::Auction,
            |v| v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"]["headName"][0] = json!("2026-08-13"),
            "Eastmoney protocol error: Miaoxiang source date \"2026-08-13\" does not match requested 2026-08-14",
        ),
        (
            "non-digit auction volume", Family::Auction,
            |v| v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"]["vol"][0] = json!("-1"),
            "Eastmoney protocol error: Miaoxiang opening auction volume is not a non-negative integer",
        ),
        (
            "above exact integer boundary", Family::Auction,
            |v| v["data"]["data"]["searchDataResultDTO"]["dataTableDTOList"][0]["rawTable"]["vol"][0] = json!("9007199254740993"),
            "Eastmoney protocol error: Miaoxiang opening auction volume exceeds exact f64 integer range",
        ),
    ];

    for (name, family, defect, expected_reason) in cases {
        let mut source = seed(family);
        defect(&mut source);
        let (result, calls, attempts, _, _) = execute(family, &source);
        assert_one_post(family, &calls);
        assert_one_attempt(family, &attempts);
        match result {
            Err(ServiceError::FailedPrecondition(reason)) => {
                assert_eq!(reason, expected_reason, "{name}")
            }
            Err(other) => panic!("{name}: wrong public failure stage: {other:?}"),
            Ok(result) => {
                panic!("{name}: source defect published a partial/empty/success result: {result:?}")
            }
        }
    }
}
