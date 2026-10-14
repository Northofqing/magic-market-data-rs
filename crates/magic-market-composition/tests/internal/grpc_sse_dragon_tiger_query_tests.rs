use super::register_sse_dragon_tiger_client;
use magic_exchange_rs::{
    ExchangeError, ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, SseClient, SseConfig,
};
use magic_market_core::{DragonTigerEntry, DragonTigerSeat, ProviderId};
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, QueryResult, ServiceError,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAXIMUM_PAYLOAD_BYTES: usize = 64 * 1024;
const PAGE_URL: &str = "https://query.sse.com.cn/infodisplay/showTradePublicFile.do?jsonCallBack=magicExchange&isPagination=false&dateTx=2026-07-22";
const PAGE: &[u8] =
    include_bytes!("../../../magic-exchange-rs/fixtures/dragon_tiger_sse_600396.jsonp");

fn expected_request() -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: PAGE_URL.to_owned(),
        headers: vec![
            (
                "User-Agent".to_owned(),
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
                    .to_owned(),
            ),
            (
                "Accept".to_owned(),
                "application/json, text/javascript;q=0.9".to_owned(),
            ),
            ("Accept-Language".to_owned(), "zh-CN,zh;q=0.9,en;q=0.8".to_owned()),
            (
                "Referer".to_owned(),
                "https://www.sse.com.cn/disclosure/diclosure/public/dailydata/".to_owned(),
            ),
            ("X-Requested-With".to_owned(), "XMLHttpRequest".to_owned()),
        ],
        body: Vec::new(),
    }
}

#[derive(Clone)]
struct OfficialPageFixture {
    body: Vec<u8>,
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl OfficialPageFixture {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn assert_one_external_request(&self) {
        assert_eq!(
            self.attempts.lock().unwrap().as_slice(),
            &[expected_request()]
        );
    }
}

impl ExchangeTransport for OfficialPageFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        self.attempts
            .lock()
            .map_err(|_| ExchangeError::Transport("offline SSE trace poisoned".to_owned()))?
            .push(request.clone());
        if request != &expected_request() {
            return Err(ExchangeError::Transport(
                "unexpected offline SSE method, URL, ordered headers or body".to_owned(),
            ));
        }
        if self.body.len() > 8 * 1024 * 1024 {
            return Err(ExchangeError::Transport(
                "offline SSE response exceeds 8 MiB".to_owned(),
            ));
        }
        Ok(HttpResponse {
            status: 200,
            final_url: PAGE_URL.to_owned(),
            content_type: Some("application/javascript".to_owned()),
            body: self.body.clone(),
        })
    }
}

fn registry(fixture: OfficialPageFixture) -> OperationRegistry {
    let config = SseConfig::default();
    assert_eq!(config.timeout, Duration::from_secs(15));
    assert_eq!(config.minimum_interval, Duration::from_secs(1));
    assert_eq!(config.max_pages, 10);
    let client = SseClient::with_transport(config, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline SSE dragon-tiger contract");
    // Setup uses the exact same concrete registrar as production. Assertions
    // observe only public Query execution and the external request seam.
    register_sse_dragon_tiger_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(kind: &str) -> QueryCommand {
    let limit = match kind {
        "entries" => 1,
        "seats" => 10,
        _ => panic!("unsupported test request kind"),
    };
    let command = QueryCommand::new(
        "sse-dragon-tiger-public-contract",
        Operation::DragonTiger,
        Some("Sse".to_owned()),
        CanonicalPayload::new(
            "magic.market.dragon_tiger.request",
            1,
            serde_json::to_vec(&serde_json::json!({
                "kind":kind,
                "request":{
                    "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
                    "trading_date":"2026-07-22","limit":limit
                }
            }))
            .unwrap(),
            MAXIMUM_PAYLOAD_BYTES,
        )
        .unwrap(),
    )
    .unwrap();
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
    u128::from(seconds_value) * 1_000_000_000 + fraction.parse::<u128>().unwrap()
}

fn complete_projected_rows(
    result: &QueryResult,
    kind: &str,
    before: u128,
    after: u128,
) -> Vec<serde_json::Value> {
    assert_eq!(result.provider, "Sse");
    assert!(result.complete);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-22"));
    assert_eq!(result.records.len(), if kind == "entries" { 1 } else { 10 });
    let prefix = format!("Sse:2026-07-22:600396:dragon-tiger-{kind}:");
    let batch_time = epoch_nanos(result.batch_id.strip_prefix(&prefix).unwrap());
    let returned_time = epoch_nanos(&result.observed_at);
    assert!(before <= batch_time && batch_time <= returned_time && returned_time <= after);

    let schema = if kind == "entries" {
        "magic.market.dragon_tiger_entry"
    } else {
        "magic.market.dragon_tiger_seat"
    };
    let mut record_time = None;
    result
        .records
        .iter()
        .map(|payload| {
            assert_eq!(payload.schema(), schema);
            assert_eq!(payload.schema_version(), 1);
            let mut row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
            let observed = row["evidence"]["observed_at"].as_str().unwrap();
            let observed_nanos = epoch_nanos(observed);
            assert!(batch_time <= observed_nanos && observed_nanos <= returned_time);
            match &record_time {
                Some(first) => assert_eq!(observed, first),
                None => record_time = Some(observed.to_owned()),
            }
            assert_eq!(row["evidence"]["batch_id"], result.batch_id);
            if kind == "entries" {
                let typed: DragonTigerEntry = serde_json::from_slice(payload.data()).unwrap();
                assert_eq!(typed.evidence().provider(), ProviderId::Sse);
            } else {
                let typed: DragonTigerSeat = serde_json::from_slice(payload.data()).unwrap();
                assert_eq!(typed.evidence().provider(), ProviderId::Sse);
            }
            // Only nondeterministic evidence is projected, after it is checked
            // against the actual request window and returned batch identity.
            row["evidence"]["observed_at"] = serde_json::json!("<observed-at>");
            row["evidence"]["batch_id"] = serde_json::json!("<batch-id>");
            row
        })
        .collect()
}

#[test]
fn registered_sse_dragon_tiger_preserves_official_entries_and_complete_seats_and_rejects_late_rank()
{
    // The oracle uses the independent source fixture's exact yuan values;
    // entry summary totals, name, opposite-side amounts and net stay absent.
    let expected_entry: serde_json::Value = serde_json::from_str(r#"{
        "entry_id":"sse:2026-07-22:600396:1",
        "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
        "instrument_name":null,"trading_date":"2026-07-22",
        "reason":"有价格涨跌幅限制的日收盘价格涨幅偏离值达到7%的前五只证券",
        "buy_amount":null,"sell_amount":null,"net_amount":null,"turnover_rate":null,
        "evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}
    }"#).unwrap();
    let expected_seats: Vec<serde_json::Value> = serde_json::from_str(r#"[
        {"entry_id":"sse:2026-07-22:600396:1","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-22","side":"Buy","rank":1,"seat_name":"沪股通专用","amount":204844204.54,"buy_amount":204844204.54,"sell_amount":null,"net_amount":null,"evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}},
        {"entry_id":"sse:2026-07-22:600396:1","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-22","side":"Buy","rank":2,"seat_name":"开源证券股份有限公司西安太华路证券营业部","amount":105063836.0,"buy_amount":105063836.0,"sell_amount":null,"net_amount":null,"evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}},
        {"entry_id":"sse:2026-07-22:600396:1","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-22","side":"Buy","rank":3,"seat_name":"国泰海通证券股份有限公司总部","amount":90889949.0,"buy_amount":90889949.0,"sell_amount":null,"net_amount":null,"evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}},
        {"entry_id":"sse:2026-07-22:600396:1","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-22","side":"Buy","rank":4,"seat_name":"国新证券股份有限公司北京分公司","amount":87790025.0,"buy_amount":87790025.0,"sell_amount":null,"net_amount":null,"evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}},
        {"entry_id":"sse:2026-07-22:600396:1","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-22","side":"Buy","rank":5,"seat_name":"国泰海通证券股份有限公司湛江万豪世家证券营业部","amount":77178105.0,"buy_amount":77178105.0,"sell_amount":null,"net_amount":null,"evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}},
        {"entry_id":"sse:2026-07-22:600396:1","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-22","side":"Sell","rank":1,"seat_name":"沪股通专用","amount":81243566.0,"buy_amount":null,"sell_amount":81243566.0,"net_amount":null,"evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}},
        {"entry_id":"sse:2026-07-22:600396:1","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-22","side":"Sell","rank":2,"seat_name":"国信证券股份有限公司浙江互联网分公司","amount":76512065.0,"buy_amount":null,"sell_amount":76512065.0,"net_amount":null,"evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}},
        {"entry_id":"sse:2026-07-22:600396:1","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-22","side":"Sell","rank":3,"seat_name":"中信证券股份有限公司上海分公司","amount":48672096.62,"buy_amount":null,"sell_amount":48672096.62,"net_amount":null,"evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}},
        {"entry_id":"sse:2026-07-22:600396:1","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-22","side":"Sell","rank":4,"seat_name":"东方财富证券股份有限公司山南香曲东路证券营业部","amount":42104028.0,"buy_amount":null,"sell_amount":42104028.0,"net_amount":null,"evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}},
        {"entry_id":"sse:2026-07-22:600396:1","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-22","side":"Sell","rank":5,"seat_name":"东方财富证券股份有限公司拉萨东城区江苏大道证券营业部","amount":33992533.0,"buy_amount":null,"sell_amount":33992533.0,"net_amount":null,"evidence":{"provider":"Sse","source_at":"2026-07-22","observed_at":"<observed-at>","batch_id":"<batch-id>"}}
    ]"#).unwrap();

    for (kind, expected) in [("entries", vec![expected_entry]), ("seats", expected_seats)] {
        let fixture = OfficialPageFixture::new(PAGE);
        let registry = registry(fixture.clone());
        let before = clock_nanos();
        let result = registry.execute(command(kind)).unwrap();
        let after = clock_nanos();
        fixture.assert_one_external_request();
        assert_eq!(
            complete_projected_rows(&result, kind, before, after),
            expected
        );
    }

    let text = std::str::from_utf8(PAGE).unwrap();
    let final_sell = "  (5) 东方财富证券股份有限公司拉萨东城区江苏大道证券营业部                                       33992533.00";
    assert_eq!(text.matches(final_sell).count(), 1);
    let late_wrong_rank = final_sell.replacen("(5)", "(4)", 1);
    let corrupted = text.replacen(final_sell, &late_wrong_rank, 1);
    for kind in ["entries", "seats"] {
        let fixture = OfficialPageFixture::new(corrupted.as_bytes());
        let result = registry(fixture.clone()).execute(command(kind));
        fixture.assert_one_external_request();
        match result {
            Err(ServiceError::FailedPrecondition(reason)) => {
                assert_eq!(
                    reason,
                    "official exchange schema drift: SSE seat rank 4 does not match expected 5"
                );
            }
            other => panic!("late invalid rank must reject the entire {kind} query: {other:?}"),
        }
    }
}
