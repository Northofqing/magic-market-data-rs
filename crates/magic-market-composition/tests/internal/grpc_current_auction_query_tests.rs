use super::*;
use magic_market_transport::{
    HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const OFFLINE_KEY: &str = "offline-public-auction-fixture";
const REQUEST: &str = include_str!("fixtures/current_auction_request.json");
const RESPONSE: &[u8] = include_bytes!("fixtures/current_auction_response.json");
const ORACLE: &str = include_str!("fixtures/current_auction_oracle.json");

struct CurrentAuctionFixture;

impl HttpTransport for CurrentAuctionFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let reject =
            || TransportError::InvalidRequest("offline auction fixture rejected request".into());
        if request.method() != HttpMethod::Get || !request.body().is_empty() {
            return Err(reject());
        }
        let (endpoint, query) = request.url().split_once('?').ok_or_else(reject)?;
        if endpoint != "https://fuyao.aicubes.cn/api/a-share/auction/snapshot"
            || query.contains('#')
        {
            return Err(reject());
        }
        let mut pairs = BTreeMap::new();
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').ok_or_else(reject)?;
            if !matches!(key, "thscodes" | "stage") {
                return Err(reject());
            }
            // Only the known instrument-list separator is decoded. This is
            // a closed offline fixture, not a replacement URL-policy parser.
            let value = value.replace("%2C", ",").replace("%2c", ",");
            if pairs.insert(key, value).is_some() {
                return Err(reject());
            }
        }
        if pairs.len() != 2
            || pairs.get("thscodes").map(String::as_str) != Some("600396.SH,000001.SZ")
            || pairs.get("stage").map(String::as_str) != Some("live")
        {
            return Err(reject());
        }
        let mut headers = BTreeMap::new();
        for (name, value) in request.headers() {
            if headers
                .insert(name.to_ascii_lowercase(), value.as_str())
                .is_some()
            {
                return Err(reject());
            }
        }
        if headers.len() != 3
            || headers.get("accept").copied() != Some("application/json")
            || headers.get("x-api-key").copied() != Some(OFFLINE_KEY)
            || headers.get("user-agent").copied() != Some("magic-hithink-rs/0.2")
        {
            return Err(reject());
        }
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            RESPONSE.to_vec(),
        ))
    }
}

#[test]
fn registered_current_auction_preserves_trade_and_no_trade_observations() {
    // Exercise the real registrar and public query, substituting only the
    // external HTTP response. These samples are not live admission evidence.
    let client = HithinkClient::with_transport(OFFLINE_KEY, CurrentAuctionFixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
    register_hithink_client(&mut registry, Arc::new(client), 16_384).unwrap();
    let request: Value = serde_json::from_str(REQUEST).unwrap();
    let command = QueryCommand::new(
        request["request_id"].as_str().unwrap(),
        Operation::CurrentAuctionObservations,
        Some(request["preferred_provider"].as_str().unwrap().into()),
        CanonicalPayload::new(
            request["schema"].as_str().unwrap(),
            1,
            serde_json::to_vec(&request["payload"]).unwrap(),
            16_384,
        )
        .unwrap(),
    )
    .unwrap();
    let result = registry.execute(command).unwrap();
    let oracle: Value = serde_json::from_str(ORACLE).unwrap();
    assert_eq!(result.records.len(), 2);
    for record in &result.records {
        assert_eq!(record.schema(), oracle["record_schema"].as_str().unwrap());
        assert_eq!(record.schema_version(), 1);
    }
    let records: Vec<Value> = result
        .records
        .iter()
        .map(|record| serde_json::from_slice(record.data()).unwrap())
        .collect();
    assert_eq!(
        json!({
            "provider": result.provider,
            "batch_id": result.batch_id,
            "complete": result.complete,
            "observed_at": result.observed_at,
            "source_at": result.source_at,
            "repository_admitted": result.repository_admitted,
            "diagnostic_blocker": result.diagnostic_blocker,
            "record_schema": result.records[0].schema(),
            "record_schema_version": result.records[0].schema_version(),
            "records": records
        }),
        oracle
    );
}
