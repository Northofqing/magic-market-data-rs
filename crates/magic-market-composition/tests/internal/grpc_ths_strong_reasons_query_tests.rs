use super::register_ths_strong_reasons_client;
use magic_market_core::StrongStockReason;
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
const STRONG_URL: &str =
    "https://zx.10jqka.com.cn/event/api/getharden/date/2026-07-22/orderby/date/orderway/desc/charset/GBK/";

// Pinned external source first row plus one legal, nonmatching source row.
const NORMAL_JSON: &str = r#"{
  "errocode": 0,
  "errormsg": "",
  "data": [
    {
      "code": "000815",
      "name": "美利云",
      "reason": "算力租赁+东数西算+中国诚通旗下",
      "close": 16.13,
      "zhangfu": 10.027,
      "huanshou": 12.97,
      "chengjiaoe": 144790,
      "chengjiaoliang": 901929,
      "ddejingliang": 0,
      "market": 33
    },
    {
      "code": "600396",
      "reason": "煤电"
    }
  ]
}
"#;
// Only the later source row's code changes type. The successful first row is unchanged.
const MALFORMED_TAIL_JSON: &str = r#"{
  "errocode": 0,
  "errormsg": "",
  "data": [
    {
      "code": "000815",
      "name": "美利云",
      "reason": "算力租赁+东数西算+中国诚通旗下",
      "close": 16.13,
      "zhangfu": 10.027,
      "huanshou": 12.97,
      "chengjiaoe": 144790,
      "chengjiaoliang": 901929,
      "ddejingliang": 0,
      "market": 33
    },
    {
      "code": {"value":"600396"},
      "reason": "煤电"
    }
  ]
}
"#;

fn expected_request() -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: STRONG_URL.to_owned(),
        headers: vec![
            (
                "User-Agent".to_owned(),
                "Mozilla/5.0 (compatible; magic-ths-rs/0.2; read-only public-data probe)"
                    .to_owned(),
            ),
            ("Accept".to_owned(), "application/json".to_owned()),
            ("Referer".to_owned(), "https://zx.10jqka.com.cn/".to_owned()),
        ],
    }
}

#[derive(Clone)]
struct StrongJsonFixture {
    body: Vec<u8>,
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl StrongJsonFixture {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ThsTransport for StrongJsonFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ThsError> {
        // Lossless all-entry trace before any fixture guard; only external bytes vary.
        self.attempts
            .lock()
            .map_err(|_| ThsError::Transport("offline strong-reasons trace poisoned".to_owned()))?
            .push(request.clone());
        if request != &expected_request() {
            return Err(ThsError::Transport(
                "unexpected offline strong-reasons method, endpoint or ordered headers".to_owned(),
            ));
        }
        if self.body.len() > 4 * 1024 * 1024 {
            return Err(ThsError::Transport(
                "offline strong-reasons body exceeds source bound".to_owned(),
            ));
        }
        Ok(HttpResponse {
            status: 200,
            final_url: STRONG_URL.to_owned(),
            content_type: Some("application/json; charset=UTF-8".to_owned()),
            body: self.body.clone(),
        })
    }
}

fn registry(fixture: StrongJsonFixture) -> OperationRegistry {
    let config = ThsConfig::default();
    assert_eq!(config.minimum_interval, Duration::from_secs(1));
    assert_eq!(config.timeout, Duration::from_secs(15));
    let client = ThsClient::with_transport(config, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline strong-reasons public contract");
    // Concrete private registrar is setup only; public Execute observes all behavior.
    register_ths_strong_reasons_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::StrongStockReasons,
        Some("Tonghuashun".to_owned()),
        CanonicalPayload::new(
            "magic.market.strong_stock_reasons.request",
            1,
            br#"{"instrument":{"exchange":"Shenzhen","code":"000815","asset_class":"Equity"},"trading_date":"2026-07-22","limit":1}"#.to_vec(),
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
fn registered_strong_reasons_rejects_malformed_tail_without_publishing_successful_prefix() {
    assert_eq!(NORMAL_JSON.len(), 420);
    assert_eq!(
        format!("{:x}", Sha256::digest(NORMAL_JSON.as_bytes())),
        "78c39b7c004e59abc91c5d3ae11e846c048c0bd5f850e913597f52c045b0d59b"
    );
    assert_eq!(MALFORMED_TAIL_JSON.len(), 430);
    assert_eq!(
        format!("{:x}", Sha256::digest(MALFORMED_TAIL_JSON.as_bytes())),
        "eb5a493d5d2403aabb6661e09a5df9ab0ae796871e107c72bcdbc6af5c94d042"
    );
    assert_eq!(
        MALFORMED_TAIL_JSON,
        NORMAL_JSON.replace(r#""code": "600396""#, r#""code": {"value":"600396"}"#,)
    );

    let normal_fixture = StrongJsonFixture::new(NORMAL_JSON.as_bytes());
    let normal_registry = registry(normal_fixture.clone());
    let before = clock_nanos();
    let result = normal_registry
        .execute(command("ths-strong-reasons-complete-control"))
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

    // Independent epoch-nanosecond syntax, overflow and actual time-window oracle.
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
    let expected_batch_id = format!("ths:{}:strong-stock:2026-07-22", result.observed_at);
    assert_eq!(result.batch_id, expected_batch_id);

    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.strong_stock_reason");
    assert_eq!(payload.schema_version(), 1);
    let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(row.as_object().unwrap().len(), 6);
    assert_eq!(row["instrument"].as_object().unwrap().len(), 3);
    assert_eq!(row["subjects"].as_array().unwrap().len(), 3);
    assert_eq!(row["evidence"].as_object().unwrap().len(), 4);
    let expected = serde_json::json!({
        "instrument": {"exchange":"Shenzhen", "code":"000815", "asset_class":"Equity"},
        "trading_date": "2026-07-22",
        "reason": "算力租赁+东数西算+中国诚通旗下",
        "subjects": ["算力租赁", "东数西算", "中国诚通旗下"],
        "limit_state": null,
        "evidence": {
            "provider": "Tonghuashun",
            "source_at": "2026-07-22",
            "observed_at": result.observed_at.clone(),
            "batch_id": expected_batch_id.clone()
        }
    });
    assert_eq!(row, expected);
    let typed: StrongStockReason = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), expected);

    let rejected_fixture = StrongJsonFixture::new(MALFORMED_TAIL_JSON.as_bytes());
    let rejected_registry = registry(rejected_fixture.clone());
    let error = rejected_registry
        .execute(command("ths-strong-reasons-malformed-later-source-row"))
        .unwrap_err();
    assert_eq!(
        error,
        ServiceError::FailedPrecondition(
            "Tonghuashun schema drift: strong-stock code must be a string when present".to_owned(),
        )
    );
    assert_eq!(
        rejected_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );
}
