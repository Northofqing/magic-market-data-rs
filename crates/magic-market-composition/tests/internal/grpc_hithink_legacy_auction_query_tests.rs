use super::*;
use magic_market_transport::{
    HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Mutex;

const OFFLINE_KEY: &str = "offline-legacy-auction-fixture";
const BLOCKER: &str = "Fuyao current auction snapshots omit the exact trading date, provider source time and directional unmatched bid/ask quantities; separate benchmark and calendar dates are not bound to snapshot records";
const RESPONSE: &str = r#"{
  "code":0,"message":"success","request_id":"offline-legacy-auction-source",
  "data":{"timestamp":1787386686058,"auction_phase":"closed","data_status":"final","total":2,
    "item":[
      {"thscode":"600396.SH","ticker":"600396","name":"离线样本一",
       "auction_price":10.25,"pre_close_price":10.0,"auction_pct":2.5,
       "auction_volume":12.5,"auction_amount":12812.5,"auction_unmatched":25.0,
       "auction_turnover_pct":0.012,"auction_volume_ratio":1.25,
       "auction_yesterday_ratio_pct":101.0,"float_market_cap":1000000.0,
       "last_price":10.25,"open_price":10.25},
      {"thscode":"000001.SZ","ticker":"000001","name":"离线样本二",
       "auction_price":0.0,"pre_close_price":12.2,"auction_pct":0.0,
       "auction_volume":0.0,"auction_amount":0.0,"auction_unmatched":-321.0,
       "auction_turnover_pct":0.0,"auction_volume_ratio":null,
       "auction_yesterday_ratio_pct":null,"float_market_cap":null,
       "last_price":0.0,"open_price":0.0}
    ]}
}"#;

#[derive(Clone)]
struct FinalAuctionFixture {
    body: Arc<Vec<u8>>,
    attempts: Arc<Mutex<Vec<Value>>>,
}

impl FinalAuctionFixture {
    fn new(body: Vec<u8>) -> Self {
        Self {
            body: Arc::new(body),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn attempts(&self) -> Vec<Value> {
        self.attempts.lock().unwrap().clone()
    }
}

impl HttpTransport for FinalAuctionFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        // Capture the whole external request even if the closed fixture rejects it.
        self.attempts.lock().unwrap().push(json!({
            "method": format!("{:?}", request.method()),
            "url": request.url(),
            "headers": request.headers(),
            "body": request.body()
        }));
        let mut headers = BTreeMap::new();
        for (name, value) in request.headers() {
            if headers
                .insert(name.to_ascii_lowercase(), value.as_str())
                .is_some()
            {
                return Err(TransportError::InvalidRequest(
                    "duplicate fixture header".into(),
                ));
            }
        }
        if request.method() != HttpMethod::Get
            || request.url() != "https://fuyao.aicubes.cn/api/a-share/auction/snapshot?thscodes=600396.SH%2C000001.SZ&stage=final"
            || !request.body().is_empty()
            || headers != BTreeMap::from([
                ("accept".into(), "application/json"),
                ("user-agent".into(), "magic-hithink-rs/0.2"),
                ("x-api-key".into(), OFFLINE_KEY),
            ])
        {
            return Err(TransportError::InvalidRequest("unexpected offline auction request".into()));
        }
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            self.body.as_ref().clone(),
        ))
    }
}

fn registry(fixture: FinalAuctionFixture) -> OperationRegistry {
    let client = HithinkClient::with_transport(OFFLINE_KEY, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline external response only");
    register_hithink_client(&mut registry, Arc::new(client), 16_384).unwrap();
    registry
}

fn command(allow_unadmitted: bool) -> QueryCommand {
    QueryCommand::new(
        "offline-legacy-auction-public",
        Operation::Auctions,
        Some("HithinkFinance".into()),
        CanonicalPayload::new(
            "magic.market.hithink_current_auctions.request",
            1,
            serde_json::to_vec(&json!({
                "instruments": [
                    {"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
                    {"exchange":"Shenzhen","code":"000001","asset_class":"Equity"}
                ]
            }))
            .unwrap(),
            16_384,
        )
        .unwrap(),
    )
    .unwrap()
    .with_unadmitted_access(allow_unadmitted)
}

fn assert_single_source_request(fixture: &FinalAuctionFixture) {
    let attempts = fixture.attempts();
    assert_eq!(
        attempts.len(),
        1,
        "unexpected external requests: {attempts:?}"
    );
    assert_eq!(attempts[0]["method"], "Get");
    assert_eq!(
        attempts[0]["url"],
        "https://fuyao.aicubes.cn/api/a-share/auction/snapshot?thscodes=600396.SH%2C000001.SZ&stage=final"
    );
    assert_eq!(attempts[0]["body"], json!([]));
    let mut headers: Vec<(String, String)> =
        serde_json::from_value(attempts[0]["headers"].clone()).unwrap();
    headers.sort();
    assert_eq!(
        headers,
        vec![
            ("Accept".into(), "application/json".into()),
            ("User-Agent".into(), "magic-hithink-rs/0.2".into()),
            ("X-api-key".into(), OFFLINE_KEY.into()),
        ]
    );
}

#[test]
fn registered_legacy_auction_keeps_diagnostic_evidence_and_rejects_wrong_tail_identity() {
    let fixture = FinalAuctionFixture::new(RESPONSE.as_bytes().to_vec());
    let registry = registry(fixture.clone());

    assert!(matches!(
        registry.execute(command(false)),
        Err(ServiceError::Unsupported { operation: Operation::Auctions, reason })
            if reason == BLOCKER
    ));
    assert!(
        fixture.attempts().is_empty(),
        "default access must not reach the source"
    );

    let result = registry.execute(command(true));
    assert_single_source_request(&fixture);
    let result = result.unwrap();
    let records: Vec<Value> = result
        .records
        .iter()
        .map(|record| {
            assert_eq!(
                record.schema(),
                "magic.market.hithink_current_auction_snapshot"
            );
            assert_eq!(record.schema_version(), 1);
            serde_json::from_slice(record.data()).unwrap()
        })
        .collect();
    assert_eq!(
        json!({
            "provider":result.provider,
            "batch_id":result.batch_id,
            "complete":result.complete,
            "observed_at":result.observed_at,
            "source_at":result.source_at,
            "repository_admitted":result.repository_admitted,
            "diagnostic_blocker":result.diagnostic_blocker,
            "records":records
        }),
        json!({
            "provider":"HithinkFinance",
            "batch_id":"offline-legacy-auction-source",
            "complete":false,
            "observed_at":"unix-ms:1787386686058",
            "source_at":null,
            "repository_admitted":false,
            "diagnostic_blocker":BLOCKER,
            "records":[
                {
                    "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
                    "name":"离线样本一","matched_price":10.25,"previous_close":10.0,
                    "change_percent":{"value":2.5,"unit":"Percent"},
                    "matched_quantity":1250.0,"matched_amount":12812.5,
                    "unmatched_bid_quantity":null,"unmatched_ask_quantity":null,
                    "volume_ratio":{"value":1.25,"unit":"Decimal"},"status":"Unavailable",
                    "source_at":null,"observed_at":"unix-ms:1787386686058",
                    "provider":"Tonghuashun","batch_id":"offline-legacy-auction-source"
                },
                {
                    "instrument":{"exchange":"Shenzhen","code":"000001","asset_class":"Equity"},
                    "name":"离线样本二","matched_price":null,"previous_close":12.2,
                    "change_percent":{"value":0.0,"unit":"Percent"},
                    "matched_quantity":0.0,"matched_amount":0.0,
                    "unmatched_bid_quantity":null,"unmatched_ask_quantity":null,
                    "volume_ratio":null,"status":"Unavailable",
                    "source_at":null,"observed_at":"unix-ms:1787386686058",
                    "provider":"Tonghuashun","batch_id":"offline-legacy-auction-source"
                }
            ]
        })
    );

    // Change only the second raw identity; the successful first row is not returned.
    assert_eq!(RESPONSE.matches(r#""ticker":"000001""#).count(), 1);
    let wrong_tail = RESPONSE.replacen(r#""ticker":"000001""#, r#""ticker":"000002""#, 1);
    let fixture = FinalAuctionFixture::new(wrong_tail.into_bytes());
    let result = self::registry(fixture.clone()).execute(command(true));
    assert_single_source_request(&fixture);
    assert!(
        matches!(
            result,
            Err(ServiceError::ProviderFailure {
                operation: Operation::Auctions,
                provider,
                kind: ProviderFailureKind::ResponseInvalid,
                provider_reason,
            }) if provider == "HithinkFinance" && provider_reason == "category=protocol"
        ),
        "wrong source identity must reject the entire diagnostic query"
    );
}
