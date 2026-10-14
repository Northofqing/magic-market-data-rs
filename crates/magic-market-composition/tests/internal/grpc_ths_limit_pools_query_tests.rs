use super::register_ths_limit_pools_client;
use magic_market_core::LimitPoolEntry;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use magic_ths_rs::{
    HttpMethod, HttpRequest, HttpResponse, ThsClient, ThsConfig, ThsError, ThsTransport,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAXIMUM_PAYLOAD_BYTES: usize = 16 * 1024;
const LIMIT_URL: &str = "https://data.10jqka.com.cn/dataapi/limit_up/limit_up_pool?page=1&limit=1&field=199112%2C10%2C9001%2C330323%2C330324%2C330325%2C9002%2C330329%2C133971%2C133970%2C1968584%2C3475914%2C9003%2C9004&filter=HS%2CGEM2STAR&order_field=330324&order_type=0&date=20260722";

// Exact pinned external fixture bytes; no provider record is fabricated.
const NORMAL_JSON: &str = r#"{
  "status_code": 0,
  "data": {
    "info": [
      {
        "code": "300779",
        "name": "惠城环保",
        "latest": 69.12,
        "change_rate": 20.0,
        "reason_type": "废塑料化学循环+固废处理+稀土永磁",
        "limit_up_type": "换手板",
        "limit_up_suc_rate": 1.0,
        "open_num": 8,
        "order_amount": 18125752,
        "high_days": "首板",
        "first_limit_up_time": "1784686821",
        "is_again_limit": 1
      }
    ]
  }
}
"#;

fn expected_request() -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: LIMIT_URL.to_owned(),
        headers: vec![
            (
                "User-Agent".to_owned(),
                "Mozilla/5.0 (compatible; magic-ths-rs/0.2; read-only public-data probe)"
                    .to_owned(),
            ),
            ("Accept".to_owned(), "application/json".to_owned()),
            (
                "Referer".to_owned(),
                "https://data.10jqka.com.cn/".to_owned(),
            ),
        ],
    }
}

#[derive(Clone)]
struct LimitPoolJsonFixture {
    body: Vec<u8>,
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl LimitPoolJsonFixture {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ThsTransport for LimitPoolJsonFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ThsError> {
        // Capture every external entry before method/URL/headers/body guards.
        self.attempts
            .lock()
            .map_err(|_| ThsError::Transport("offline upper-limit trace poisoned".to_owned()))?
            .push(request.clone());
        if request != &expected_request() {
            return Err(ThsError::Transport(
                "unexpected offline upper-limit method, endpoint or ordered headers".to_owned(),
            ));
        }
        if self.body.len() > 4 * 1024 * 1024 {
            return Err(ThsError::Transport(
                "offline upper-limit source exceeds response body bound".to_owned(),
            ));
        }
        Ok(HttpResponse {
            status: 200,
            final_url: LIMIT_URL.to_owned(),
            content_type: Some("application/json; charset=UTF-8".to_owned()),
            body: self.body.clone(),
        })
    }
}

fn registry(fixture: LimitPoolJsonFixture) -> OperationRegistry {
    let config = ThsConfig::default();
    assert_eq!(config.minimum_interval, Duration::from_secs(1));
    assert_eq!(config.timeout, Duration::from_secs(15));
    let client = ThsClient::with_transport(config, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline upper-limit public contract");
    // Actual concrete production registrar is setup only, never the test entry.
    register_ths_limit_pools_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::LimitPools,
        Some("Tonghuashun".to_owned()),
        CanonicalPayload::new(
            "magic.market.limit_pools.request",
            1,
            br#"{"kind":"Upper","trading_date":"2026-07-22","limit":1}"#.to_vec(),
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

#[test]
fn registered_upper_limit_pool_rejects_negative_sealed_amount_without_publishing_rows() {
    assert_eq!(NORMAL_JSON.len(), 495);
    assert_eq!(
        format!("{:x}", Sha256::digest(NORMAL_JSON.as_bytes())),
        "1f6abbfb157bb127e521f6357ee501d68d671d6222ba677db56690e9b898d167"
    );
    assert_eq!(
        NORMAL_JSON.matches(r#""order_amount": 18125752"#).count(),
        1
    );
    let malformed = NORMAL_JSON.replace(r#""order_amount": 18125752"#, r#""order_amount": -1"#);
    assert_eq!(malformed.len(), 489);
    assert_eq!(
        format!("{:x}", Sha256::digest(malformed.as_bytes())),
        "24bf1f2cecea1780414c345c82aadc9ac218716c2e865691e2cb42381b505cff"
    );

    let normal_fixture = LimitPoolJsonFixture::new(NORMAL_JSON.as_bytes());
    let normal_registry = registry(normal_fixture.clone());
    let before = clock_nanos();
    let result = normal_registry
        .execute(command("ths-upper-limit-complete-control"))
        .unwrap();
    let after = clock_nanos();
    assert_eq!(result.provider, "Tonghuashun");
    assert!(result.complete);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-22"));
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.records.len(), 1);
    assert_eq!(
        normal_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );

    // Independent epoch-nanosecond grammar and actual public-call time bracket.
    let (seconds, fraction) = result.observed_at.split_once('.').unwrap();
    assert!(!seconds.is_empty());
    assert!(seconds.bytes().all(|byte| byte.is_ascii_digit()));
    assert_eq!(fraction.len(), 9);
    assert!(fraction.bytes().all(|byte| byte.is_ascii_digit()));
    let observed_nanos = seconds
        .parse::<u128>()
        .unwrap()
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(fraction.parse::<u128>().unwrap()))
        .unwrap();
    assert!(before <= observed_nanos && observed_nanos <= after);
    let expected_batch_id = format!("ths:{}:upper-limit:2026-07-22", result.observed_at);
    assert_eq!(result.batch_id, expected_batch_id);

    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.limit_pool_entry");
    assert_eq!(payload.schema_version(), 1);
    let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(row.as_object().unwrap().len(), 18);
    assert_eq!(row["instrument"].as_object().unwrap().len(), 3);
    assert_eq!(row["change"].as_object().unwrap().len(), 2);
    assert_eq!(row["evidence"].as_object().unwrap().len(), 4);
    // Literal source facts: amounts stay yuan, Percent does not become Decimal,
    // first-seal seconds are independently converted; absent fields stay null.
    let expected = serde_json::json!({
        "kind": "Upper",
        "instrument": {"exchange":"Shenzhen", "code":"300779", "asset_class":"Equity"},
        "trading_date": "2026-07-22",
        "price": 69.12,
        "change": {"value":20.0, "unit":"Percent"},
        "volume": null,
        "turnover": null,
        "sealed_amount": 18125752.0,
        "first_seal_at": "2026-07-22T10:20:21+08:00",
        "last_seal_at": null,
        "break_count": 8,
        "streak": 1,
        "industry": null,
        "board_name": null,
        "seal_state": "换手板",
        "reseal_count": null,
        "reason": "废塑料化学循环+固废处理+稀土永磁",
        "evidence": {
            "provider": "Tonghuashun",
            "source_at": "2026-07-22",
            "observed_at": result.observed_at.clone(),
            "batch_id": expected_batch_id.clone()
        }
    });
    assert_eq!(row, expected);
    let typed: LimitPoolEntry = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), expected);

    let rejected_fixture = LimitPoolJsonFixture::new(malformed.as_bytes());
    let rejected_registry = registry(rejected_fixture.clone());
    let error = rejected_registry
        .execute(command("ths-upper-limit-negative-source-sealed-amount"))
        .unwrap_err();
    assert_eq!(
        error,
        ServiceError::FailedPrecondition(
            "Tonghuashun schema drift: limit reveal order_amount must be non-negative".to_owned(),
        )
    );
    assert_eq!(
        rejected_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );
}
