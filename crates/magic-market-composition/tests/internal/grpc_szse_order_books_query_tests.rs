use super::register_szse_order_books_client;
use magic_exchange_rs::{
    ExchangeError, ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, SzseClient, SzseConfig,
};
use magic_market_core::{DataStatus, OrderBook, ProviderId, SourcedRecord};
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, QueryResult, ServiceError,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAXIMUM_PAYLOAD_BYTES: usize = 16 * 1024;
const MAXIMUM_SOURCE_BYTES: usize = 1024 * 1024;
const SZSE_URL: &str = "https://www.szse.cn/api/market/ssjjhq/getTimeData?marketId=1&code=000858";

// Byte-exact existing SDK raw fixture, including final LF; no normalized DTO generates this wire.
const NORMAL_JSON: &str = r#"{
  "datetime": "2026-07-23 15:30",
  "code": "0",
  "message": "成功",
  "data": {
    "code": "000858",
    "name": "五 粮 液",
    "groupId": 1,
    "close": "74.75",
    "open": "74.00",
    "now": "74.84",
    "high": "74.95",
    "low": "73.32",
    "volume": 370321,
    "amount": 2746776997.16,
    "delta": "0.09",
    "deltaPercent": "0.12",
    "marketTime": "2026-07-23 15:30:00",
    "tradingPhaseCode1": "00",
    "tradingPhaseCode2": "00",
    "sellbuy5": [
      {"price": "74.84", "volume": 506},
      {"price": "74.85", "volume": 317},
      {"price": "74.86", "volume": 178},
      {"price": "74.87", "volume": 152},
      {"price": "74.88", "volume": 86},
      {"price": "74.83", "volume": 122},
      {"price": "74.82", "volume": 112},
      {"price": "74.81", "volume": 65},
      {"price": "74.80", "volume": 103},
      {"price": "74.79", "volume": 73}
    ]
  }
}
"#;
// Only the final bid price and volume become null: source absence, not a protocol failure.
const MISSING_TAIL_JSON: &str = r#"{
  "datetime": "2026-07-23 15:30",
  "code": "0",
  "message": "成功",
  "data": {
    "code": "000858",
    "name": "五 粮 液",
    "groupId": 1,
    "close": "74.75",
    "open": "74.00",
    "now": "74.84",
    "high": "74.95",
    "low": "73.32",
    "volume": 370321,
    "amount": 2746776997.16,
    "delta": "0.09",
    "deltaPercent": "0.12",
    "marketTime": "2026-07-23 15:30:00",
    "tradingPhaseCode1": "00",
    "tradingPhaseCode2": "00",
    "sellbuy5": [
      {"price": "74.84", "volume": 506},
      {"price": "74.85", "volume": 317},
      {"price": "74.86", "volume": 178},
      {"price": "74.87", "volume": 152},
      {"price": "74.88", "volume": 86},
      {"price": "74.83", "volume": 122},
      {"price": "74.82", "volume": 112},
      {"price": "74.81", "volume": 65},
      {"price": "74.80", "volume": 103},
      {"price": null, "volume": null}
    ]
  }
}
"#;
// Only the final bid price becomes null; volume73 and all earlier fields remain literal.
const INVALID_TAIL_JSON: &str = r#"{
  "datetime": "2026-07-23 15:30",
  "code": "0",
  "message": "成功",
  "data": {
    "code": "000858",
    "name": "五 粮 液",
    "groupId": 1,
    "close": "74.75",
    "open": "74.00",
    "now": "74.84",
    "high": "74.95",
    "low": "73.32",
    "volume": 370321,
    "amount": 2746776997.16,
    "delta": "0.09",
    "deltaPercent": "0.12",
    "marketTime": "2026-07-23 15:30:00",
    "tradingPhaseCode1": "00",
    "tradingPhaseCode2": "00",
    "sellbuy5": [
      {"price": "74.84", "volume": 506},
      {"price": "74.85", "volume": 317},
      {"price": "74.86", "volume": 178},
      {"price": "74.87", "volume": 152},
      {"price": "74.88", "volume": 86},
      {"price": "74.83", "volume": 122},
      {"price": "74.82", "volume": 112},
      {"price": "74.81", "volume": 65},
      {"price": "74.80", "volume": 103},
      {"price": null, "volume": 73}
    ]
  }
}
"#;

fn expected_attempt() -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: SZSE_URL.to_owned(),
        headers: vec![
            (
                "User-Agent".to_owned(),
                "Mozilla/5.0 (compatible; magic-exchange-rs/0.2; read-only official-data probe)"
                    .to_owned(),
            ),
            ("Accept".to_owned(), "application/json".to_owned()),
        ],
        body: Vec::new(),
    }
}

#[derive(Clone)]
struct OrderBooksFixture {
    body: Vec<u8>,
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl OrderBooksFixture {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn assert_one_attempt(&self) {
        assert_eq!(
            self.attempts.lock().unwrap().as_slice(),
            &[expected_attempt()]
        );
    }
}

impl ExchangeTransport for OrderBooksFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        // Every external attempt precedes every fixture guard; Vec retains order and duplicates.
        self.attempts
            .lock()
            .map_err(|_| ExchangeError::Transport("offline SZSE trace poisoned".to_owned()))?
            .push(request.clone());
        if request != &expected_attempt() {
            return Err(ExchangeError::Transport(
                "unexpected offline SZSE method, raw URL, ordered headers or body".to_owned(),
            ));
        }
        if self.body.len() > MAXIMUM_SOURCE_BYTES {
            return Err(ExchangeError::Transport(
                "offline SZSE fixture exceeds parser response bound".to_owned(),
            ));
        }
        Ok(HttpResponse {
            status: 200,
            final_url: SZSE_URL.to_owned(),
            content_type: Some("application/json;charset=UTF-8".to_owned()),
            body: self.body.clone(),
        })
    }
}

fn registry(fixture: OrderBooksFixture) -> OperationRegistry {
    let config = SzseConfig::default();
    assert_eq!(config.timeout, Duration::from_secs(15));
    assert_eq!(config.minimum_interval, Duration::from_secs(1));
    assert_eq!(config.max_pages, 10);
    // Fresh real clients retain default pacing. No HttpsTransport is created or executed here.
    let client = SzseClient::with_transport(config, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline SZSE order-book contract");
    // Registrar is concrete setup only; all observations cross public Execute.
    register_szse_order_books_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::OrderBooks,
        Some("Szse".to_owned()),
        CanonicalPayload::new(
            "magic.market.order_books.request",
            1,
            br#"{"instruments":[{"exchange":"Shenzhen","code":"000858","asset_class":"Equity"}]}"#
                .to_vec(),
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
    u128::from(seconds_value)
        .checked_mul(1_000_000_000)
        .and_then(|base| base.checked_add(fraction.parse::<u128>().unwrap()))
        .unwrap()
}

fn assert_success(
    result: &QueryResult,
    before: u128,
    after: u128,
    expected_bids: serde_json::Value,
    expected_total_bid: f64,
    expected_status: DataStatus,
    expected_complete: bool,
) {
    assert_eq!(result.provider, "Szse");
    assert_eq!(result.complete, expected_complete);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(
        result.source_at.as_deref(),
        Some("2026-07-23T15:30:00+08:00")
    );
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.order_book");
    assert_eq!(payload.schema_version(), 1);
    let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    let typed: OrderBook = serde_json::from_slice(payload.data()).unwrap();

    let batch_clock = result
        .batch_id
        .strip_prefix("szse-official:")
        .unwrap()
        .strip_suffix(":order-book")
        .unwrap();
    let batch_nanos = epoch_nanos(batch_clock);
    let record_nanos = epoch_nanos(typed.observed_at());
    let fetched_nanos = epoch_nanos(&result.observed_at);
    // Initial identity, post-response observation and final fetch are distinct sampled clocks.
    assert!(before <= batch_nanos);
    assert!(batch_nanos <= record_nanos);
    assert!(record_nanos <= fetched_nanos);
    assert!(fetched_nanos <= after);
    assert_eq!(typed.instrument().code(), "000858");
    assert_eq!(typed.provider_id(), ProviderId::Szse);
    assert_eq!(typed.evidence_batch_id(), result.batch_id);
    assert_eq!(
        typed.evidence_source_at(),
        Some("2026-07-23T15:30:00+08:00")
    );
    assert_eq!(typed.evidence_observed_at(), Some(typed.observed_at()));
    assert_eq!(typed.evidence_status(), Some(expected_status));
    assert_eq!(typed.status(), expected_status);
    assert_eq!(
        typed.total_bid_quantity().unwrap().get(),
        expected_total_bid
    );
    assert_eq!(typed.total_ask_quantity().unwrap().get(), 1239.0);

    // All market fields/units are independent literals. Only bounded dynamic evidence is echoed.
    let expected = serde_json::json!({
        "instrument": {"exchange":"Shenzhen","code":"000858","asset_class":"Equity"},
        "bids": expected_bids,
        "asks": [
            {"price":74.84,"quantity":506.0},
            {"price":74.85,"quantity":317.0},
            {"price":74.86,"quantity":178.0},
            {"price":74.87,"quantity":152.0},
            {"price":74.88,"quantity":86.0}
        ],
        "total_bid_quantity": expected_total_bid,
        "total_ask_quantity": 1239.0,
        "status": expected_status,
        "source_at": "2026-07-23T15:30:00+08:00",
        "observed_at": typed.observed_at(),
        "provider": "Szse",
        "batch_id": result.batch_id.clone()
    });
    assert_eq!(row.as_object().unwrap().len(), 10);
    assert_eq!(row["instrument"].as_object().unwrap().len(), 3);
    for side in ["bids", "asks"] {
        let levels = row[side].as_array().unwrap();
        assert_eq!(levels.len(), 5);
        for level in levels {
            assert_eq!(level.as_object().unwrap().len(), 2);
        }
    }
    assert_eq!(row, expected);
    assert_eq!(serde_json::to_value(&typed).unwrap(), expected);
}

#[test]
fn registered_szse_order_books_preserves_lots_and_missing_tail_but_rejects_half_present_final_level(
) {
    for (body, bytes, digest) in [
        (
            NORMAL_JSON,
            896,
            "7e8ae9b3dd221f96316a84e7a738ceaa65964dcd58eb1fc519719212833a90c7",
        ),
        (
            MISSING_TAIL_JSON,
            895,
            "232932e16503646ca1d6895e96d43458378e4c8690cc0d215ba95d70c4b03e4d",
        ),
        (
            INVALID_TAIL_JSON,
            893,
            "81d33c3a6c3d83447205843b97a904eff36834d73fee62aa345f833494475db9",
        ),
    ] {
        assert_eq!(body.len(), bytes);
        assert_eq!(format!("{:x}", Sha256::digest(body.as_bytes())), digest);
    }
    let tail = r#"{"price": "74.79", "volume": 73}"#;
    assert_eq!(NORMAL_JSON.matches(tail).count(), 1);
    assert_eq!(
        MISSING_TAIL_JSON,
        NORMAL_JSON.replace(tail, r#"{"price": null, "volume": null}"#)
    );
    assert_eq!(
        INVALID_TAIL_JSON,
        NORMAL_JSON.replace(tail, r#"{"price": null, "volume": 73}"#)
    );

    let complete_fixture = OrderBooksFixture::new(NORMAL_JSON.as_bytes());
    let complete_registry = registry(complete_fixture.clone());
    let before = clock_nanos();
    let complete = complete_registry
        .execute(command("szse-order-book-complete-control"))
        .unwrap();
    let after = clock_nanos();
    complete_fixture.assert_one_attempt();
    assert_success(
        &complete,
        before,
        after,
        serde_json::json!([
            {"price":74.83,"quantity":122.0},
            {"price":74.82,"quantity":112.0},
            {"price":74.81,"quantity":65.0},
            {"price":74.80,"quantity":103.0},
            {"price":74.79,"quantity":73.0}
        ]),
        475.0,
        DataStatus::Available,
        true,
    );

    let missing_fixture = OrderBooksFixture::new(MISSING_TAIL_JSON.as_bytes());
    let missing_registry = registry(missing_fixture.clone());
    let before = clock_nanos();
    let missing = missing_registry
        .execute(command("szse-order-book-valid-missing-tail"))
        .unwrap();
    let after = clock_nanos();
    missing_fixture.assert_one_attempt();
    assert_success(
        &missing,
        before,
        after,
        serde_json::json!([
            {"price":74.83,"quantity":122.0},
            {"price":74.82,"quantity":112.0},
            {"price":74.81,"quantity":65.0},
            {"price":74.80,"quantity":103.0},
            {"price":null,"quantity":null}
        ]),
        402.0,
        DataStatus::Unavailable,
        false,
    );

    let rejected_fixture = OrderBooksFixture::new(INVALID_TAIL_JSON.as_bytes());
    let rejected_registry = registry(rejected_fixture.clone());
    let error = rejected_registry
        .execute(command("szse-order-book-half-present-final-bid"))
        .unwrap_err();
    rejected_fixture.assert_one_attempt();
    assert_eq!(
        error,
        ServiceError::FailedPrecondition(
            "official exchange schema drift: SZSE book price and volume must be present or absent together"
                .to_owned()
        )
    );
}
