use super::register_ths_popularity_client;
use magic_market_core::{PopularityRank, RatioUnit};
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
const POPULARITY_URL: &str =
    "https://dq.10jqka.com.cn/fuyao/hot_list_data/out/hot_list/v1/stock?stock_type=a&type=hour&list_type=normal";

// Complete checked-in external JSON bytes, not normalized provider records.
const NORMAL_JSON: &str = r#"{
  "status_code": 0,
  "data": {
    "stock_list": [
      {
        "order": 1,
        "code": "001309",
        "name": "德明利",
        "rate": "411579.0",
        "rise_and_fall": -4.8556,
        "hot_rank_chg": 0,
        "tag": {
          "concept_tag": ["存储芯片", "中芯国际概念"],
          "popularity_tag": "持续上榜"
        }
      }
    ]
  }
}
"#;
// The only source defect is concept_tag: array -> object; all other bytes stay fixed.
const WRONG_CONCEPT_TAG_JSON: &str = r#"{
  "status_code": 0,
  "data": {
    "stock_list": [
      {
        "order": 1,
        "code": "001309",
        "name": "德明利",
        "rate": "411579.0",
        "rise_and_fall": -4.8556,
        "hot_rank_chg": 0,
        "tag": {
          "concept_tag": {"name":"存储芯片"},
          "popularity_tag": "持续上榜"
        }
      }
    ]
  }
}
"#;

fn expected_request() -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: POPULARITY_URL.to_owned(),
        headers: vec![
            (
                "User-Agent".to_owned(),
                "Mozilla/5.0 (compatible; magic-ths-rs/0.2; read-only public-data probe)"
                    .to_owned(),
            ),
            ("Accept".to_owned(), "application/json".to_owned()),
            ("Referer".to_owned(), "https://dq.10jqka.com.cn/".to_owned()),
        ],
    }
}

#[derive(Clone)]
struct PopularityJsonFixture {
    body: Vec<u8>,
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl PopularityJsonFixture {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ThsTransport for PopularityJsonFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ThsError> {
        // All entries are traced before endpoint/header/response-body guards.
        self.attempts
            .lock()
            .map_err(|_| ThsError::Transport("offline popularity trace poisoned".to_owned()))?
            .push(request.clone());
        if request != &expected_request() {
            return Err(ThsError::Transport(
                "unexpected offline popularity method, endpoint or ordered headers".to_owned(),
            ));
        }
        if self.body.len() > 4 * 1024 * 1024 {
            return Err(ThsError::Transport(
                "offline popularity source exceeds response body bound".to_owned(),
            ));
        }
        Ok(HttpResponse {
            status: 200,
            final_url: POPULARITY_URL.to_owned(),
            content_type: Some("application/json; charset=UTF-8".to_owned()),
            body: self.body.clone(),
        })
    }
}

fn registry(fixture: PopularityJsonFixture) -> OperationRegistry {
    let config = ThsConfig::default();
    assert_eq!(config.minimum_interval, Duration::from_secs(1));
    assert_eq!(config.timeout, Duration::from_secs(15));
    let client = ThsClient::with_transport(config, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline popularity public contract");
    // Private concrete registrar is setup only; all observations use public Execute.
    register_ths_popularity_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::Popularity,
        Some("Tonghuashun".to_owned()),
        CanonicalPayload::new(
            "magic.market.popularity.request",
            1,
            br#"{"limit":1}"#.to_vec(),
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
fn registered_popularity_rejects_wrong_typed_source_concepts_without_publishing_rows() {
    assert_eq!(NORMAL_JSON.len(), 381);
    assert_eq!(
        format!("{:x}", Sha256::digest(NORMAL_JSON.as_bytes())),
        "869ed832e54ebffc7c88c1f59a64da66ebf7c32f42761d2cf98236a9e7712248"
    );
    assert_eq!(WRONG_CONCEPT_TAG_JSON.len(), 366);
    assert_eq!(
        format!("{:x}", Sha256::digest(WRONG_CONCEPT_TAG_JSON.as_bytes())),
        "74c3b3f9da18424a575c3c634ec4953d34f385ac47a594fa38260f23732954cc"
    );
    assert_eq!(
        WRONG_CONCEPT_TAG_JSON,
        NORMAL_JSON.replace(
            r#""concept_tag": ["存储芯片", "中芯国际概念"]"#,
            r#""concept_tag": {"name":"存储芯片"}"#,
        )
    );

    let normal_fixture = PopularityJsonFixture::new(NORMAL_JSON.as_bytes());
    let normal_registry = registry(normal_fixture.clone());
    let before = clock_nanos();
    let result = normal_registry
        .execute(command("ths-popularity-normal-control"))
        .unwrap();
    let after = clock_nanos();
    assert_eq!(result.provider, "Tonghuashun");
    assert!(result.complete);
    assert_eq!(result.source_at, None);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.records.len(), 1);
    assert_eq!(
        normal_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );

    // Independent syntax/numeric/time-window checks, not equality alone.
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
    let expected_batch_id = format!("ths:{}:popularity-hour", result.observed_at);
    assert_eq!(result.batch_id, expected_batch_id);

    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.popularity_rank");
    assert_eq!(payload.schema_version(), 1);
    let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(row.as_object().unwrap().len(), 11);
    assert_eq!(row["return_ratio"].as_object().unwrap().len(), 2);
    assert_eq!(row["evidence"].as_object().unwrap().len(), 4);
    let expected = serde_json::json!({
        "instrument": {"exchange":"Shenzhen", "code":"001309", "asset_class":"Equity"},
        "rank": 1,
        "price": null,
        "name": "德明利",
        "rank_change": 0.0,
        "return_ratio": {"value": -4.8556, "unit": "Percent"},
        "heat": 411579.0,
        "concepts": ["存储芯片", "中芯国际概念"],
        "tag": "持续上榜",
        "quote_evidence": null,
        "evidence": {
            "provider": "Tonghuashun",
            "source_at": null,
            "observed_at": result.observed_at.clone(),
            "batch_id": expected_batch_id.clone()
        }
    });
    assert_eq!(row, expected);
    let typed: PopularityRank = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(typed.return_ratio.unwrap().unit(), RatioUnit::Percent);
    assert_eq!(typed.return_ratio.unwrap().get(), -4.8556);
    assert_eq!(serde_json::to_value(&typed).unwrap(), expected);

    let malformed_fixture = PopularityJsonFixture::new(WRONG_CONCEPT_TAG_JSON.as_bytes());
    let malformed_registry = registry(malformed_fixture.clone());
    let error = malformed_registry
        .execute(command("ths-popularity-invalid-source-concepts"))
        .unwrap_err();
    assert_eq!(
        error,
        ServiceError::FailedPrecondition(
            "Tonghuashun schema drift: popularity concept_tag must be an array when present"
                .to_owned(),
        )
    );
    assert_eq!(
        malformed_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );
}
