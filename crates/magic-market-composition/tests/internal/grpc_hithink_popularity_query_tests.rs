use super::register_hithink_client;
use magic_hithink_rs::HithinkClient;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, ProviderFailureKind, QueryCommand, ServiceError,
};
use magic_market_transport::{
    HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MAXIMUM_PAYLOAD_BYTES: usize = 16 * 1024;
const OFFLINE_KEY: &str = "offline-popularity-fixture-key";
const POPULARITY_URL: &str =
    "https://fuyao.aicubes.cn/api/a-share/special-data/hot-stock-list?period=day";

// External source bytes, based on the existing native Fuyao fixture grammar.
const NORMAL_JSON: &str = r#"{
  "code":0,"message":"success","request_id":"hot-public-control",
  "data":{"timestamp":1787302800000,"item":[
    {"thscode":"600519.SH","ticker":"600519","name":"贵州茅台","rank":1,"heat":99.0,"rank_change":2,"rank_trend":"up"},
    {"thscode":"920344.BJ","ticker":"920344","name":"北交二","rank":2,"heat":"88.0","rank_change":-1,"rank_trend":"down"}
  ]}
}
"#;

fn expected_request() -> HttpRequest {
    HttpRequest::new(
        HttpMethod::Get,
        POPULARITY_URL,
        vec![
            ("Accept".to_owned(), "application/json".to_owned()),
            ("X-api-key".to_owned(), OFFLINE_KEY.to_owned()),
            ("User-Agent".to_owned(), "magic-hithink-rs/0.2".to_owned()),
        ],
        Vec::new(),
    )
    .unwrap()
}

#[derive(Clone)]
struct PopularityFixture {
    body: Vec<u8>,
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl PopularityFixture {
    fn new(body: Vec<u8>) -> Self {
        Self {
            body,
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl HttpTransport for PopularityFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        // Record every entry before any fixture guard, including rejected attempts.
        // HttpRequest preserves the method, complete URL, ordered headers and body.
        self.attempts
            .lock()
            .map_err(|_| {
                TransportError::InvalidRequest("offline popularity trace poisoned".into())
            })?
            .push(request.clone());
        if request != &expected_request() {
            return Err(TransportError::InvalidRequest(
                "unexpected offline popularity method, URL, ordered headers or body".into(),
            ));
        }
        if self.body.len() > 4 * 1024 * 1024 {
            return Err(TransportError::InvalidRequest(
                "offline popularity response exceeds 4 MiB".into(),
            ));
        }
        Ok(HttpResponse::new(
            200,
            POPULARITY_URL,
            Some("application/json; charset=UTF-8".to_owned()),
            self.body.clone(),
        ))
    }
}

fn registry(fixture: PopularityFixture) -> OperationRegistry {
    let client = HithinkClient::with_transport(OFFLINE_KEY, fixture).unwrap();
    let mut registry =
        OperationRegistry::all_unadmitted("offline Fuyao popularity public contract");
    // Private real-client registrar is setup only, not the assertion entry point.
    register_hithink_client(&mut registry, Arc::new(client), MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str, payload: &[u8]) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::Popularity,
        Some("HithinkFinance".to_owned()),
        CanonicalPayload::new(
            "magic.market.popularity.request",
            1,
            payload.to_vec(),
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
fn registered_hithink_popularity_rejects_duplicate_tail_rank_before_return_limit() {
    let normal_fixture = PopularityFixture::new(NORMAL_JSON.as_bytes().to_vec());
    let normal_registry = registry(normal_fixture.clone());
    let before = clock_nanos();
    let result = normal_registry
        .execute(command(
            "hithink-popularity-normal-control",
            br#"{"limit":2}"#,
        ))
        .unwrap();
    let after = clock_nanos();
    assert_eq!(
        normal_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );

    // An independent canonical wall-clock oracle, not a copied SDK formatter.
    let (seconds, fraction) = result.observed_at.split_once('.').unwrap();
    assert!(!seconds.is_empty());
    assert!(seconds.bytes().all(|byte| byte.is_ascii_digit()));
    let seconds_value = seconds.parse::<u64>().unwrap();
    assert_eq!(seconds_value.to_string(), seconds);
    assert_eq!(fraction.len(), 9);
    assert!(fraction.bytes().all(|byte| byte.is_ascii_digit()));
    let nanos_value = fraction.parse::<u32>().unwrap();
    assert!(nanos_value < 1_000_000_000);
    let observed_nanos = u128::from(seconds_value)
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(u128::from(nanos_value)))
        .unwrap();
    assert!(before <= after);
    assert!(before <= observed_nanos && observed_nanos <= after);

    let records: Vec<Value> = result
        .records
        .iter()
        .map(|payload| {
            assert_eq!(payload.schema(), "magic.market.popularity_rank");
            assert_eq!(payload.schema_version(), 1);
            let row: Value = serde_json::from_slice(payload.data()).unwrap();
            assert_eq!(row.as_object().unwrap().len(), 11);
            assert_eq!(row["instrument"].as_object().unwrap().len(), 3);
            assert_eq!(row["evidence"].as_object().unwrap().len(), 4);
            row
        })
        .collect();
    let observed_at = result.observed_at.clone();
    assert_eq!(
        json!({
            "provider": result.provider,
            "batch_id": result.batch_id,
            "complete": result.complete,
            "observed_at": result.observed_at,
            "source_at": result.source_at,
            "repository_admitted": result.repository_admitted,
            "diagnostic_blocker": result.diagnostic_blocker,
            "records": records
        }),
        json!({
            "provider": "HithinkFinance",
            "batch_id": "hot-public-control",
            "complete": true,
            "observed_at": observed_at.clone(),
            "source_at": "unix-ms:1787302800000",
            "repository_admitted": true,
            "diagnostic_blocker": null,
            "records": [
                {
                    "instrument": {"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
                    "rank": 1,
                    "price": null,
                    "name": "贵州茅台",
                    "rank_change": 2.0,
                    "return_ratio": null,
                    "heat": 99.0,
                    "concepts": [],
                    "tag": null,
                    "quote_evidence": null,
                    "evidence": {
                        "provider": "Tonghuashun",
                        "source_at": "unix-ms:1787302800000",
                        "observed_at": observed_at.clone(),
                        "batch_id": "hot-public-control"
                    }
                },
                {
                    "instrument": {"exchange":"Beijing","code":"920344","asset_class":"Equity"},
                    "rank": 2,
                    "price": null,
                    "name": "北交二",
                    "rank_change": -1.0,
                    "return_ratio": null,
                    "heat": 88.0,
                    "concepts": [],
                    "tag": null,
                    "quote_evidence": null,
                    "evidence": {
                        "provider": "Tonghuashun",
                        "source_at": "unix-ms:1787302800000",
                        "observed_at": observed_at,
                        "batch_id": "hot-public-control"
                    }
                }
            ]
        })
    );

    // The valid second source identity remains intact; only rank 2 becomes rank 1.
    // With limit 1, that tail still invalidates the entire public query.
    assert_eq!(NORMAL_JSON.matches(r#""rank":2"#).count(), 1);
    let duplicate_tail = NORMAL_JSON.replacen(r#""rank":2"#, r#""rank":1"#, 1);
    let malformed_fixture = PopularityFixture::new(duplicate_tail.into_bytes());
    let error = registry(malformed_fixture.clone())
        .execute(command(
            "hithink-popularity-duplicate-tail",
            br#"{"limit":1}"#,
        ))
        .unwrap_err();
    assert_eq!(
        malformed_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );
    assert_eq!(
        error,
        ServiceError::ProviderFailure {
            operation: Operation::Popularity,
            provider: "HithinkFinance".to_owned(),
            kind: ProviderFailureKind::ResponseInvalid,
            provider_reason: "category=protocol".to_owned(),
        }
    );
}
