use super::register_szse_dragon_tiger_client;
use magic_exchange_rs::{
    ExchangeError, ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, SzseClient, SzseConfig,
};
use magic_market_core::{DragonTigerSeat, ProviderId};
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, QueryResult, ServiceError,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAXIMUM_PAYLOAD_BYTES: usize = 64 * 1024;
const MAXIMUM_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const LIST_URL: &str = "https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1842_xxpl_after&TABKEY=tab1&PAGENO=1&tab1PAGESIZE=10&txtDMorJC=000603&txtStart=2026-07-23&txtEnd=2026-07-23";
const DETAIL_0901_URL: &str = "https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1842_detal&TABKEY=tab1%2Ctab2&DQRQ=2026-07-23&ZQDM=000603&ZBDM=0901";
const DETAIL_1001_URL: &str = "https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1842_detal&TABKEY=tab1%2Ctab2&DQRQ=2026-07-23&ZQDM=000603&ZBDM=1001";
const LIST_JSON: &[u8] =
    include_bytes!("../../../magic-exchange-rs/fixtures/dragon_tiger_szse_list_000603.json");
const DETAIL_0901_JSON: &[u8] =
    include_bytes!("../../../magic-exchange-rs/fixtures/dragon_tiger_szse_detail_000603_0901.json");
// Literal external second-group sample: only the two ZBDM metadata values differ from 0901.
const DETAIL_1001_JSON: &str = r#"[{
  "metadata": {
    "catalogid":"1842_detal",
    "tabkey":"tab1",
    "conditions":[
      {"name":"DQRQ","defaultValue":"2026-07-23"},
      {"name":"ZQDM","defaultValue":"000603"},
      {"name":"ZBDM","defaultValue":"1001"}
    ],
    "cols":{
      "dqrq":"公告日期",
      "zqjc":"[none]",
      "cjsl":"成 交 量",
      "cjje":"成交金额",
      "plyy":"[none]"
    }
  },
  "data":[{
    "dqrq":"2026-07-23",
    "ycqj":"无",
    "zqjc":"盛达资源&nbsp;(000603)",
    "cjsl":"69,170,347 份/股",
    "cjje":"1,656,097,547 元",
    "plyy":"日价格涨幅偏离值达到9.07%"
  }],
  "error":null
},{
  "metadata":{
    "catalogid":"1842_detal",
    "tabkey":"tab2",
    "conditions":[
      {"name":"DQRQ","defaultValue":"2026-07-23"},
      {"name":"ZQDM","defaultValue":"000603"},
      {"name":"ZBDM","defaultValue":"1001"}
    ],
    "cols":{
      "mmlb":"买/卖",
      "zsmc":"会员营业部名称",
      "mrje":"买入金额<br>（元）",
      "mcje":"卖出金额<br>（元）"
    }
  },
  "data":[
    {"mmlb":"买1","zsmc":"深股通专用","mrje":"136,235,364","mcje":"185,329,440"},
    {"mmlb":"买2","zsmc":"国泰海通证券股份有限公司北京知春路证券营业部","mrje":"112,304,399","mcje":"1,403,337"},
    {"mmlb":"买3","zsmc":"机构专用","mrje":"37,589,828","mcje":"0"},
    {"mmlb":"买4","zsmc":"机构专用","mrje":"27,014,122","mcje":"27,977,745"},
    {"mmlb":"买5","zsmc":"机构专用","mrje":"26,201,294","mcje":"33,471,072"},
    {"mmlb":"卖1","zsmc":"深股通专用","mrje":"136,235,364","mcje":"185,329,440"},
    {"mmlb":"卖2","zsmc":"机构专用","mrje":"25,722,905","mcje":"51,712,871"},
    {"mmlb":"卖3","zsmc":"机构专用","mrje":"26,201,294","mcje":"33,471,072"},
    {"mmlb":"卖4","zsmc":"招商证券股份有限公司宁波丽江东路证券营业部","mrje":"48,330","mcje":"32,281,355"},
    {"mmlb":"卖5","zsmc":"机构专用","mrje":"27,014,122","mcje":"27,977,745"}
  ],
  "error":null
}]
"#;

fn expected_attempt(url: &str) -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: url.to_owned(),
        headers: vec![
            ("User-Agent".to_owned(), "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36".to_owned()),
            ("Accept".to_owned(), "application/json".to_owned()),
            ("Accept-Language".to_owned(), "zh-CN,zh;q=0.9,en;q=0.8".to_owned()),
            ("Referer".to_owned(), "https://www.szse.cn/disclosure/deal/public/index.html".to_owned()),
        ],
        body: Vec::new(),
    }
}

#[derive(Clone)]
struct SeatsFixture {
    detail_1001: Vec<u8>,
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl SeatsFixture {
    fn new(detail_1001: &[u8]) -> Self {
        Self {
            detail_1001: detail_1001.to_vec(),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn assert_full_trace(&self) {
        assert_eq!(
            self.attempts.lock().unwrap().as_slice(),
            &[
                expected_attempt(LIST_URL),
                expected_attempt(DETAIL_0901_URL),
                expected_attempt(DETAIL_1001_URL),
            ]
        );
    }
}

impl ExchangeTransport for SeatsFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        // Every external entry is retained before any fixture guard, including rejected attempts.
        self.attempts
            .lock()
            .map_err(|_| ExchangeError::Transport("offline SZSE seats trace poisoned".to_owned()))?
            .push(request.clone());
        let body = match request.url.as_str() {
            LIST_URL => LIST_JSON,
            DETAIL_0901_URL => DETAIL_0901_JSON,
            DETAIL_1001_URL => self.detail_1001.as_slice(),
            _ => {
                return Err(ExchangeError::Transport(
                    "unexpected offline SZSE seats URL".to_owned(),
                ))
            }
        };
        if request != &expected_attempt(&request.url) {
            return Err(ExchangeError::Transport(
                "unexpected offline SZSE seats method, ordered headers or body".to_owned(),
            ));
        }
        if body.len() > MAXIMUM_SOURCE_BYTES {
            return Err(ExchangeError::Transport(
                "offline SZSE seats response exceeds 8 MiB".to_owned(),
            ));
        }
        Ok(HttpResponse {
            status: 200,
            final_url: request.url.clone(),
            content_type: Some("application/json;charset=UTF-8".to_owned()),
            body: body.to_vec(),
        })
    }
}

fn registry(fixture: SeatsFixture) -> OperationRegistry {
    let config = SzseConfig::default();
    assert_eq!(config.timeout, Duration::from_secs(15));
    assert_eq!(config.minimum_interval, Duration::from_secs(1));
    assert_eq!(config.max_pages, 10);
    let client = SzseClient::with_transport(config, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline SZSE complete-seat contract");
    // The actual concrete registrar is setup only; observations use public Execute.
    register_szse_dragon_tiger_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::DragonTiger,
        Some("Szse".to_owned()),
        CanonicalPayload::new(
            "magic.market.dragon_tiger.request", 1,
            br#"{"kind":"seats","request":{"instrument":{"exchange":"Shenzhen","code":"000603","asset_class":"Equity"},"trading_date":"2026-07-23","limit":20}}"#.to_vec(),
            MAXIMUM_PAYLOAD_BYTES,
        ).unwrap(),
    ).unwrap();
    assert!(!command.allows_unadmitted());
    command
}

fn clock_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn epoch_nanos(value: &str) -> u128 {
    let (seconds, fraction) = value.split_once('.').unwrap();
    assert!(!seconds.is_empty());
    assert!(seconds.bytes().all(|byte| byte.is_ascii_digit()));
    assert_eq!(fraction.len(), 9);
    assert!(fraction.bytes().all(|byte| byte.is_ascii_digit()));
    let seconds_value = seconds.parse::<u64>().unwrap();
    assert_eq!(seconds_value.to_string(), seconds);
    u128::from(seconds_value)
        .checked_mul(1_000_000_000)
        .and_then(|base| base.checked_add(fraction.parse::<u128>().unwrap()))
        .unwrap()
}

fn expected_rows(
    first_observed: &str,
    second_observed: &str,
    batch_id: &str,
) -> Vec<serde_json::Value> {
    let instrument =
        serde_json::json!({"exchange":"Shenzhen","code":"000603","asset_class":"Equity"});
    let first_evidence = serde_json::json!({"provider":"Szse","source_at":"2026-07-23","observed_at":first_observed,"batch_id":batch_id});
    let second_evidence = serde_json::json!({"provider":"Szse","source_at":"2026-07-23","observed_at":second_observed,"batch_id":batch_id});
    // Market fields and all twenty identities/numeric expectations are independent source literals.
    vec![
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:0901","instrument":instrument,"trading_date":"2026-07-23","side":"Buy","rank":1,"seat_name":"深股通专用","amount":136235364.0,"buy_amount":136235364.0,"sell_amount":185329440.0,"net_amount":-49094076.0,"evidence":first_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:0901","instrument":instrument,"trading_date":"2026-07-23","side":"Buy","rank":2,"seat_name":"国泰海通证券股份有限公司北京知春路证券营业部","amount":112304399.0,"buy_amount":112304399.0,"sell_amount":1403337.0,"net_amount":110901062.0,"evidence":first_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:0901","instrument":instrument,"trading_date":"2026-07-23","side":"Buy","rank":3,"seat_name":"机构专用","amount":37589828.0,"buy_amount":37589828.0,"sell_amount":0.0,"net_amount":37589828.0,"evidence":first_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:0901","instrument":instrument,"trading_date":"2026-07-23","side":"Buy","rank":4,"seat_name":"机构专用","amount":27014122.0,"buy_amount":27014122.0,"sell_amount":27977745.0,"net_amount":-963623.0,"evidence":first_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:0901","instrument":instrument,"trading_date":"2026-07-23","side":"Buy","rank":5,"seat_name":"机构专用","amount":26201294.0,"buy_amount":26201294.0,"sell_amount":33471072.0,"net_amount":-7269778.0,"evidence":first_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:0901","instrument":instrument,"trading_date":"2026-07-23","side":"Sell","rank":1,"seat_name":"深股通专用","amount":185329440.0,"buy_amount":136235364.0,"sell_amount":185329440.0,"net_amount":-49094076.0,"evidence":first_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:0901","instrument":instrument,"trading_date":"2026-07-23","side":"Sell","rank":2,"seat_name":"机构专用","amount":51712871.0,"buy_amount":25722905.0,"sell_amount":51712871.0,"net_amount":-25989966.0,"evidence":first_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:0901","instrument":instrument,"trading_date":"2026-07-23","side":"Sell","rank":3,"seat_name":"机构专用","amount":33471072.0,"buy_amount":26201294.0,"sell_amount":33471072.0,"net_amount":-7269778.0,"evidence":first_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:0901","instrument":instrument,"trading_date":"2026-07-23","side":"Sell","rank":4,"seat_name":"招商证券股份有限公司宁波丽江东路证券营业部","amount":32281355.0,"buy_amount":48330.0,"sell_amount":32281355.0,"net_amount":-32233025.0,"evidence":first_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:0901","instrument":instrument,"trading_date":"2026-07-23","side":"Sell","rank":5,"seat_name":"机构专用","amount":27977745.0,"buy_amount":27014122.0,"sell_amount":27977745.0,"net_amount":-963623.0,"evidence":first_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:1001","instrument":instrument,"trading_date":"2026-07-23","side":"Buy","rank":1,"seat_name":"深股通专用","amount":136235364.0,"buy_amount":136235364.0,"sell_amount":185329440.0,"net_amount":-49094076.0,"evidence":second_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:1001","instrument":instrument,"trading_date":"2026-07-23","side":"Buy","rank":2,"seat_name":"国泰海通证券股份有限公司北京知春路证券营业部","amount":112304399.0,"buy_amount":112304399.0,"sell_amount":1403337.0,"net_amount":110901062.0,"evidence":second_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:1001","instrument":instrument,"trading_date":"2026-07-23","side":"Buy","rank":3,"seat_name":"机构专用","amount":37589828.0,"buy_amount":37589828.0,"sell_amount":0.0,"net_amount":37589828.0,"evidence":second_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:1001","instrument":instrument,"trading_date":"2026-07-23","side":"Buy","rank":4,"seat_name":"机构专用","amount":27014122.0,"buy_amount":27014122.0,"sell_amount":27977745.0,"net_amount":-963623.0,"evidence":second_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:1001","instrument":instrument,"trading_date":"2026-07-23","side":"Buy","rank":5,"seat_name":"机构专用","amount":26201294.0,"buy_amount":26201294.0,"sell_amount":33471072.0,"net_amount":-7269778.0,"evidence":second_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:1001","instrument":instrument,"trading_date":"2026-07-23","side":"Sell","rank":1,"seat_name":"深股通专用","amount":185329440.0,"buy_amount":136235364.0,"sell_amount":185329440.0,"net_amount":-49094076.0,"evidence":second_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:1001","instrument":instrument,"trading_date":"2026-07-23","side":"Sell","rank":2,"seat_name":"机构专用","amount":51712871.0,"buy_amount":25722905.0,"sell_amount":51712871.0,"net_amount":-25989966.0,"evidence":second_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:1001","instrument":instrument,"trading_date":"2026-07-23","side":"Sell","rank":3,"seat_name":"机构专用","amount":33471072.0,"buy_amount":26201294.0,"sell_amount":33471072.0,"net_amount":-7269778.0,"evidence":second_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:1001","instrument":instrument,"trading_date":"2026-07-23","side":"Sell","rank":4,"seat_name":"招商证券股份有限公司宁波丽江东路证券营业部","amount":32281355.0,"buy_amount":48330.0,"sell_amount":32281355.0,"net_amount":-32233025.0,"evidence":second_evidence}),
        serde_json::json!({"entry_id":"szse:2026-07-23:000603:1001","instrument":instrument,"trading_date":"2026-07-23","side":"Sell","rank":5,"seat_name":"机构专用","amount":27977745.0,"buy_amount":27014122.0,"sell_amount":27977745.0,"net_amount":-963623.0,"evidence":second_evidence}),
    ]
}

fn assert_complete_result(result: &QueryResult, before: u128, after: u128) {
    assert_eq!(result.provider, "Szse");
    assert!(result.complete);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
    assert_eq!(result.records.len(), 20);
    let batch_clock = result
        .batch_id
        .strip_prefix("Szse:2026-07-23:000603:dragon-tiger-seats:")
        .unwrap();
    let batch_nanos = epoch_nanos(batch_clock);
    let fetched_nanos = epoch_nanos(&result.observed_at);
    let typed = result
        .records
        .iter()
        .map(|payload| {
            assert_eq!(payload.schema(), "magic.market.dragon_tiger_seat");
            assert_eq!(payload.schema_version(), 1);
            serde_json::from_slice::<DragonTigerSeat>(payload.data()).unwrap()
        })
        .collect::<Vec<_>>();
    let first_observed = typed[0].evidence().observed_at();
    let second_observed = typed[10].evidence().observed_at();
    let first_nanos = epoch_nanos(first_observed);
    let second_nanos = epoch_nanos(second_observed);
    // Batch identity precedes each separately sampled group observation; final fetch is later.
    assert!(before <= batch_nanos);
    assert!(batch_nanos <= first_nanos);
    assert!(first_nanos <= second_nanos);
    assert!(second_nanos <= fetched_nanos);
    assert!(fetched_nanos <= after);
    let expected = expected_rows(first_observed, second_observed, &result.batch_id);
    for (index, ((payload, seat), expected)) in
        result.records.iter().zip(&typed).zip(&expected).enumerate()
    {
        let evidence = seat.evidence();
        assert_eq!(evidence.provider(), ProviderId::Szse);
        assert_eq!(evidence.source_at(), Some("2026-07-23"));
        assert_eq!(evidence.batch_id(), result.batch_id);
        assert_eq!(
            evidence.observed_at(),
            if index < 10 {
                first_observed
            } else {
                second_observed
            }
        );
        let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(row.as_object().unwrap().len(), 11);
        assert_eq!(row["instrument"].as_object().unwrap().len(), 3);
        assert_eq!(row["evidence"].as_object().unwrap().len(), 4);
        assert_eq!(&row, expected);
        assert_eq!(&serde_json::to_value(seat).unwrap(), expected);
    }
}

#[test]
fn registered_szse_seats_rejects_late_duplicate_rank_without_publishing_first_group() {
    assert_eq!(LIST_JSON.len(), 1395);
    assert_eq!(
        format!("{:x}", Sha256::digest(LIST_JSON)),
        "86774ef07592d129bedf630bfc7068a722b01dc7f2f35d91c673d6a15d8d0a3f"
    );
    assert_eq!(DETAIL_0901_JSON.len(), 1996);
    assert_eq!(
        format!("{:x}", Sha256::digest(DETAIL_0901_JSON)),
        "20a7cc1cc4a7479d6da8769af27c0058089044639863253cc4483b64839a480e"
    );
    let original = std::str::from_utf8(DETAIL_0901_JSON).unwrap();
    let metadata = r#""name":"ZBDM","defaultValue":"0901""#;
    assert_eq!(original.matches(metadata).count(), 2);
    assert_eq!(
        DETAIL_1001_JSON,
        original.replace(metadata, r#""name":"ZBDM","defaultValue":"1001""#)
    );
    assert_eq!(DETAIL_1001_JSON.len(), 1996);
    let last_rank = r#""mmlb":"卖5""#;
    assert_eq!(DETAIL_1001_JSON.matches(last_rank).count(), 1);
    let invalid = DETAIL_1001_JSON.replace(last_rank, r#""mmlb":"卖4""#);
    assert_eq!(invalid.len(), 1996);

    let normal_fixture = SeatsFixture::new(DETAIL_1001_JSON.as_bytes());
    let normal_registry = registry(normal_fixture.clone());
    let before = clock_nanos();
    let result = normal_registry
        .execute(command("szse-seats-two-complete-groups-control"))
        .unwrap();
    let after = clock_nanos();
    normal_fixture.assert_full_trace();
    assert_complete_result(&result, before, after);

    let rejected_fixture = SeatsFixture::new(invalid.as_bytes());
    let rejected_registry = registry(rejected_fixture.clone());
    let error = rejected_registry
        .execute(command("szse-seats-second-group-duplicate-last-rank"))
        .unwrap_err();
    rejected_fixture.assert_full_trace();
    assert_eq!(
        error,
        ServiceError::FailedPrecondition(
            "official exchange schema drift: SZSE side/rank \"卖4\" is duplicate or out of range"
                .to_owned()
        )
    );
}
