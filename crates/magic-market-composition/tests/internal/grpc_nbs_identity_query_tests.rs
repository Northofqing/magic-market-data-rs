use super::register_economic_provider;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use magic_market_transport::{
    HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use magic_nbs_rs::NbsClient;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

const RESPONSE_CAP: usize = 4 * 1024 * 1024;
const URLS: [&str; 8] = [
    "https://data.stats.gov.cn/dg/website/publicrelease/web/external/new/queryIndexTreeAsync?pid=&code=4",
    "https://data.stats.gov.cn/dg/website/publicrelease/web/external/new/queryIndexTreeAsync?pid=f4c6cd795fea436c807163397dd36b98&code=4",
    "https://data.stats.gov.cn/dg/website/publicrelease/web/external/new/queryIndexTreeAsync?pid=fce37cb7a3124492ba7d1467d70b40c1&code=4",
    "https://data.stats.gov.cn/dg/website/publicrelease/web/external/new/queryIndexTreeAsync?pid=6a27e7bf8c2c4ce4b7d58d56d806f4a5&code=4",
    "https://data.stats.gov.cn/dg/website/publicrelease/web/external/new/queryIndicatorsByCid?cid=b5e122302ad745358ba7415bd4a22c2f&dt=&name=",
    "https://data.stats.gov.cn/dg/website/publicrelease/web/external/getDaCatalogTreeByIndicatorCid?indicatorCid=b5e122302ad745358ba7415bd4a22c2f",
    "https://data.stats.gov.cn/dg/website/publicrelease/web/external/getDasByDaCatalogId?daCid=a10dceae75d245008bf4b9a0e6fe1d55",
    "https://data.stats.gov.cn/dg/website/publicrelease/web/external/stream/esData",
];
const POST_BODY: &str = r#"{"cid":"b5e122302ad745358ba7415bd4a22c2f","daCatalogId":"","das":[{"text":"北京市","value":"110000000000"}],"dts":["202607MM"],"indicatorIds":["02d75ee002764e2ea53f263e52109e8f"],"rootId":"f4c6cd795fea436c807163397dd36b98","showType":"1"}"#;
const SOURCES: [&str; 8] = [
    r#"{"data":[{"_id":"f4c6cd795fea436c807163397dd36b98","name":"分省月度数据","treeinfo_pid":"69c574ab128a44e595cc0b24502b771b","treeinfo_level":3,"isLeaf":false,"type":"catalog"}],"success":true,"state":20000}"#,
    r#"{"data":[{"_id":"fce37cb7a3124492ba7d1467d70b40c1","name":"价格指数","treeinfo_pid":"f4c6cd795fea436c807163397dd36b98","treeinfo_level":4,"isLeaf":false,"type":"catalog"}],"success":true,"state":20000}"#,
    r#"{"data":[{"_id":"6a27e7bf8c2c4ce4b7d58d56d806f4a5","name":"居民消费价格分类指数","treeinfo_pid":"fce37cb7a3124492ba7d1467d70b40c1","treeinfo_level":5,"isLeaf":false,"type":"catalog"}],"success":true,"state":20000}"#,
    r#"{"data":[{"_id":"b5e122302ad745358ba7415bd4a22c2f","name":"居民消费价格分类指数 (上年同月=100) (2026-)","treeinfo_pid":"6a27e7bf8c2c4ce4b7d58d56d806f4a5","treeinfo_level":6,"isLeaf":true,"type":"catalog"}],"success":true,"state":20000}"#,
    r#"{"data":{"total":1,"list":[{"_id":"02d75ee002764e2ea53f263e52109e8f","i_showname":"居民消费价格指数 (上年同月=100) ","du_name":"%","catalogid":"b5e122302ad745358ba7415bd4a22c2f"}]},"success":true,"state":20000}"#,
    r#"{"data":[{"_id":"a10dceae75d245008bf4b9a0e6fe1d55","name":"全部地区","treeinfo_pid":"6a99abef22d44119bdf5f8e2451ac390","treeinfo_level":2}],"success":true,"state":20000}"#,
    r#"{"data":[{"catalog_id":"a10dceae75d245008bf4b9a0e6fe1d55","name_value":"110000000000","show_name":"北京市","name_text":"北京市"}],"success":true,"state":20000}"#,
    r#"{"data":[{"code":"202607MM","name":"2026年7月","values":[{"_id":"02d75ee002764e2ea53f263e52109e8f","i_showname":"居民消费价格指数 (上年同月=100) ","du_name":"%","catalogid":"b5e122302ad745358ba7415bd4a22c2f","value":"100.2","da":"110000000000","da_name":"北京市"}]}],"success":true,"state":20000}"#,
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceAttempt {
    request: HttpRequest,
    response_cap: usize,
    response_body: Option<Vec<u8>>,
}

struct IdentityFixture {
    responses: Vec<Vec<u8>>,
    attempts: Arc<Mutex<Vec<SourceAttempt>>>,
}

fn expected_request(stage: usize) -> HttpRequest {
    let mut headers = vec![("Accept".into(), "application/json".into())];
    let (method, body) = if stage == 7 {
        headers.push(("Content-Type".into(), "application/json".into()));
        (HttpMethod::Post, POST_BODY.as_bytes().to_vec())
    } else {
        (HttpMethod::Get, Vec::new())
    };
    HttpRequest::new(method, URLS[stage], headers, body).unwrap()
}

impl HttpTransport for IdentityFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut attempts = self.attempts.lock().unwrap();
        // Capture every actual external entrance, including a rejected guard.
        // Clone retains exact URL, ordered headers and body bytes; no redaction.
        attempts.push(SourceAttempt {
            request: request.clone(),
            response_cap: RESPONSE_CAP,
            response_body: None,
        });
        let stage = URLS.iter().position(|url| *url == request.url());
        let Some(stage) = stage else {
            return Err(TransportError::InvalidRequest(
                "NBS fixture rejected an unlisted exact URL".into(),
            ));
        };
        if attempts.len() > URLS.len() || request != &expected_request(stage) {
            return Err(TransportError::InvalidRequest(
                "NBS fixture rejected method, ordered headers, body or attempt bound".into(),
            ));
        }
        let body = self.responses[stage].clone();
        if body.len() > RESPONSE_CAP {
            return Err(TransportError::InvalidRequest(
                "NBS fixture response exceeded 4 MiB".into(),
            ));
        }
        attempts.last_mut().unwrap().response_body = Some(body.clone());
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body,
        ))
    }
}

fn source_responses(ambiguous_stage: Option<usize>) -> Vec<Vec<u8>> {
    let mut responses: Vec<Vec<u8>> = SOURCES
        .iter()
        .map(|body| body.as_bytes().to_vec())
        .collect();
    if let Some(stage) = ambiguous_stage {
        assert!(matches!(stage, 0 | 4 | 5 | 6));
        // Mutate external source bytes only: duplicate one valid matching node.
        let mut value: Value = serde_json::from_slice(&responses[stage]).unwrap();
        if stage == 4 {
            let duplicate = value["data"]["list"][0].clone();
            value["data"]["list"]
                .as_array_mut()
                .unwrap()
                .push(duplicate);
            value["data"]["total"] = json!(2);
        } else {
            let duplicate = value["data"][0].clone();
            value["data"].as_array_mut().unwrap().push(duplicate);
        }
        responses[stage] = serde_json::to_vec(&value).unwrap();
    }
    responses
}

fn registry(
    responses: Vec<Vec<u8>>,
    attempts: &Arc<Mutex<Vec<SourceAttempt>>>,
) -> OperationRegistry {
    // Actual constructor retains its real one-second RequestGate.
    let client = NbsClient::with_transport(Arc::new(IdentityFixture {
        responses,
        attempts: attempts.clone(),
    }))
    .unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline NBS source fixture");
    register_economic_provider(
        &mut registry,
        client,
        "Nbs",
        "admitted national and regional NBS series with exact source namespace and period",
        4096,
    )
    .unwrap();
    registry
}

fn command(request_id: &str) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::EconomicSeries,
        Some("Nbs".into()),
        CanonicalPayload::new(
            "magic.market.economic_series.request",
            1,
            br#"{"series":[{"provider":"Nbs","namespace":"beijing-cpi-yoy","code":"headline"}],"start":{"Monthly":{"year":2026,"month":7}},"end":{"Monthly":{"year":2026,"month":7}},"max_rows":1}"#.to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!command.allows_unadmitted());
    command
}

fn expected_attempts(responses: &[Vec<u8>], through_stage: usize) -> Vec<SourceAttempt> {
    (0..=through_stage)
        .map(|stage| SourceAttempt {
            request: expected_request(stage),
            response_cap: RESPONSE_CAP,
            response_body: Some(responses[stage].clone()),
        })
        .collect()
}

#[test]
fn registered_nbs_requires_unique_source_identity_before_publishing_cpi() {
    let responses = source_responses(None);
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let control_registry = registry(responses.clone(), &attempts);
    let before = OffsetDateTime::now_utc();
    let control = control_registry
        .execute(command("nbs-identity-positive-control"))
        .unwrap();
    let after = OffsetDateTime::now_utc();
    let observed = OffsetDateTime::parse(&control.observed_at, &Rfc3339).unwrap();
    assert!(before <= observed && observed <= after);
    assert_eq!(control.provider, "Nbs");
    assert!(control.complete && control.repository_admitted);
    assert_eq!(control.diagnostic_blocker, None);
    assert_eq!(control.source_at, None);
    assert_eq!(control.records.len(), 1);
    assert_eq!(
        control.batch_id,
        format!("nbs-beijing-cpi-yoy:202607:{}", control.observed_at)
    );
    assert_ne!(control.batch_id, "nbs-identity-positive-control");
    let payload = &control.records[0];
    assert_eq!(payload.schema(), "magic.market.economic_observation");
    assert_eq!(payload.schema_version(), 1);
    // Independent all-field literal facts; only dynamic observation/batch bind
    // the already-checked public envelope. Never turn 100.2 into 0.2 or infer time.
    assert_eq!(
        serde_json::from_slice::<Value>(payload.data()).unwrap(),
        json!({
            "series": {"provider":"Nbs","namespace":"beijing-cpi-yoy","code":"headline"},
            "name": "居民消费价格指数 (上年同月=100)",
            "region_code": null,
            "region_name": null,
            "period": {"Monthly":{"year":2026,"month":7}},
            "value": 100.2,
            "unit": "%",
            "scale": null,
            "seasonal_adjustment": null,
            "status": "Present",
            "released_at": null,
            "revision": null,
            "evidence": {
                "provider": "Nbs",
                "source_at": null,
                "observed_at": control.observed_at,
                "batch_id": control.batch_id
            }
        })
    );
    assert_eq!(*attempts.lock().unwrap(), expected_attempts(&responses, 7));

    // ONE source-identity uniqueness invariant, four discovery gates. Each
    // negative has a valid raw prefix and cannot publish any CPI QueryResult.
    for (stage, request_id, reason) in [
        (
            0,
            "nbs-ambiguous-catalog",
            "NBS source contract failed: NBS catalog node \"分省月度数据\" is ambiguous",
        ),
        (
            4,
            "nbs-ambiguous-indicator",
            "NBS source contract failed: NBS headline CPI indicator is ambiguous",
        ),
        (
            5,
            "nbs-ambiguous-area-catalog",
            "NBS source contract failed: NBS all-regions catalog is ambiguous",
        ),
        (
            6,
            "nbs-ambiguous-beijing-area",
            "NBS source contract failed: NBS Beijing area identity is ambiguous",
        ),
    ] {
        let responses = source_responses(Some(stage));
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let rejection_registry = registry(responses.clone(), &attempts);
        assert_eq!(
            rejection_registry.execute(command(request_id)).unwrap_err(),
            ServiceError::FailedPrecondition(reason.into())
        );
        // Lossless trace proves the exact selected discovery gate was reached,
        // and that no later area/data request or guard-rejected attempt occurred.
        assert_eq!(
            *attempts.lock().unwrap(),
            expected_attempts(&responses, stage)
        );
    }
}
