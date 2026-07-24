use super::*;
use std::sync::{mpsc, Mutex};

const FIXTURE: &str = r#"{
      "status_code": 0,
      "status_msg": "ok",
      "data": [
        {
          "uid": "report-1",
          "title": "人形机器人行业研究",
          "summary": "行星滚柱丝杠产业链摘要",
          "publish_date": "2026-07-22 09:00:00",
          "url": "https://www.iwencai.com/report/1",
          "score": 0.8,
          "extra": "{\"organization\":\"测试机构\"}"
        },
        {
          "uid": "report-1",
          "title": "人形机器人行业研究（高相关段落）",
          "summary": "高相关摘要",
          "publish_date": "2026-07-22 09:00:00",
          "url": "https://www.iwencai.com/report/1",
          "score": 0.9,
          "extra": {"organization":"测试机构"}
        }
      ]
    }"#;

#[derive(Debug)]
struct FixtureTransport {
    response: HttpResponse,
    request: Mutex<Option<HttpRequest>>,
}

impl IwencaiTransport for FixtureTransport {
    fn post(&self, request: &HttpRequest) -> Result<HttpResponse, IwencaiError> {
        *self
            .request
            .lock()
            .map_err(|_| IwencaiError::Transport("fixture lock poisoned".into()))? =
            Some(request.clone());
        Ok(self.response.clone())
    }
}

#[derive(Debug)]
struct BlockingTransport {
    response: HttpResponse,
    starts: mpsc::Sender<Instant>,
    releases: Mutex<mpsc::Receiver<()>>,
}

impl IwencaiTransport for BlockingTransport {
    fn post(&self, _request: &HttpRequest) -> Result<HttpResponse, IwencaiError> {
        self.starts
            .send(Instant::now())
            .map_err(|error| IwencaiError::Transport(error.to_string()))?;
        self.releases
            .lock()
            .map_err(|_| IwencaiError::Transport("release lock poisoned".into()))?
            .recv()
            .map_err(|error| IwencaiError::Transport(error.to_string()))?;
        Ok(self.response.clone())
    }
}

#[derive(Debug)]
struct CompletionTransport {
    response: HttpResponse,
    completed_at: Arc<Mutex<Option<SystemTime>>>,
}

impl IwencaiTransport for CompletionTransport {
    fn post(&self, _request: &HttpRequest) -> Result<HttpResponse, IwencaiError> {
        std::thread::sleep(Duration::from_millis(20));
        *self
            .completed_at
            .lock()
            .map_err(|_| IwencaiError::Transport("completion lock poisoned".into()))? =
            Some(SystemTime::now());
        Ok(self.response.clone())
    }
}

fn request(limit: u32) -> SemanticSearchRequest {
    SemanticSearchRequest::new(
        "人形机器人 行星滚柱丝杠",
        SemanticChannel::Report,
        magic_market_core::PositiveU32::new(limit).expect("positive"),
    )
    .expect("request")
}

#[test]
fn maps_verified_fixture_and_deduplicates_highest_score() {
    let client = IwencaiClient::with_transport(
        "fixture-key",
        FixtureTransport {
            response: HttpResponse::new(200, FIXTURE.as_bytes().to_vec()),
            request: Mutex::new(None),
        },
    )
    .expect("client");
    let batch = client
        .semantic_search(&request(50))
        .expect("fixture parses");
    assert_eq!(batch.records().len(), 1);
    let document = &batch.records()[0];
    assert_eq!(document.document_id.as_str(), "report-1");
    assert_eq!(document.channel, SemanticChannel::Report);
    assert_eq!(document.title.as_str(), "人形机器人行业研究（高相关段落）");
    assert_eq!(
        document.excerpt.as_ref().map(NonEmptyText::as_str),
        Some("高相关摘要")
    );
    assert_eq!(
        document.canonical_url.as_str(),
        "https://www.iwencai.com/report/1"
    );
    assert_eq!(
        document.published_at.as_ref().map(NonEmptyText::as_str),
        Some("2026-07-22 09:00:00")
    );
    assert_eq!(document.evidence.provider(), ProviderId::Iwencai);
    assert_eq!(batch.provenance().source(), "iwencai-openapi");
    assert_eq!(batch.provenance().source_at(), None);
}

#[test]
fn request_has_skillhub_headers_and_bounded_payload() {
    let request = build_request(
        &request(50),
        "secret",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .expect("request");
    assert_eq!(
        request.url(),
        "https://openapi.iwencai.com/v1/comprehensive/search"
    );
    assert!(request
        .headers()
        .iter()
        .any(|(name, value)| name == "Authorization" && value == "Bearer secret"));
    let body: Value = serde_json::from_slice(request.body()).expect("request JSON");
    assert_eq!(body["channels"][0], "report");
    assert_eq!(body["size"], 50);
    assert!(!format!("{request:?}").contains("secret"));
    let client = IwencaiClient::with_transport(
        "secret",
        FixtureTransport {
            response: HttpResponse::new(200, FIXTURE.as_bytes().to_vec()),
            request: Mutex::new(None),
        },
    )
    .expect("client");
    assert!(!format!("{client:?}").contains("secret"));
}

#[test]
fn missing_and_rejected_keys_are_typed_authentication_errors() {
    assert!(matches!(
        IwencaiClient::new(" "),
        Err(IwencaiError::Authentication(_))
    ));
    let client = IwencaiClient::with_transport(
        "rejected",
        FixtureTransport {
            response: HttpResponse::new(401, br#"{"detail":"not_found_apikey"}"#.to_vec()),
            request: Mutex::new(None),
        },
    )
    .expect("client");
    assert!(matches!(
        client.semantic_search(&request(1)),
        Err(IwencaiError::Authentication(_))
    ));
    let error_body = br#"{"status_code":401,"status_msg":"rejected secret-token","data":[]}"#;
    let error = parse_response(error_body, SemanticChannel::Report, 1, "observed")
        .expect_err("API-level key rejection must fail");
    assert!(matches!(error, IwencaiError::Authentication(_)));
    assert!(!error.to_string().contains("secret-token"));
}

#[test]
fn successful_empty_data_is_not_a_fake_success() {
    let error = parse_response(
        br#"{"status_code":0,"status_msg":"ok","data":[]}"#,
        SemanticChannel::Report,
        1,
        "observed",
    )
    .expect_err("empty success must fail");
    assert!(matches!(error, IwencaiError::Protocol(_)));
}

#[test]
fn provider_limit_and_official_host_are_enforced() {
    let client = IwencaiClient::with_transport(
        "fixture-key",
        FixtureTransport {
            response: HttpResponse::new(200, FIXTURE.as_bytes().to_vec()),
            request: Mutex::new(None),
        },
    )
    .expect("client");
    assert!(matches!(
        client.semantic_search(&request(51)),
        Err(IwencaiError::InvalidRequest(_))
    ));
    assert!(matches!(
        parse_response(FIXTURE.as_bytes(), SemanticChannel::Report, 1, "observed"),
        Err(IwencaiError::Protocol(_))
    ));
    assert!(ensure_official_url("https://openapi.iwencai.com/v1/x").is_ok());
    assert!(ensure_official_url("https://openapi.iwencai.com.evil.test/x").is_err());
    assert!(ensure_json_content_type(Some("application/json; charset=utf-8")).is_ok());
    assert!(ensure_json_content_type(Some("text/html")).is_err());
    assert!(ensure_json_content_type(None).is_err());
    let oversized = IwencaiClient::with_transport(
        "fixture-key",
        FixtureTransport {
            response: HttpResponse::new(200, vec![b' '; MAX_RESPONSE_BYTES + 1]),
            request: Mutex::new(None),
        },
    )
    .expect("client")
    .semantic_search(&request(1))
    .expect_err("injected transports cannot bypass the response cap");
    assert!(matches!(oversized, IwencaiError::Protocol(_)));
}

#[test]
fn observed_at_is_captured_after_the_response_completes() {
    let completed_at = Arc::new(Mutex::new(None));
    let client = IwencaiClient::with_transport(
        "fixture-key",
        CompletionTransport {
            response: HttpResponse::new(200, FIXTURE.as_bytes().to_vec()),
            completed_at: completed_at.clone(),
        },
    )
    .expect("client");
    let batch = client
        .semantic_search(&request(50))
        .expect("fixture parses");
    let completed = completed_at
        .lock()
        .expect("completion lock")
        .expect("transport completion time")
        .duration_since(UNIX_EPOCH)
        .expect("completion after epoch")
        .as_nanos();
    let mut observed_parts = batch.records()[0].evidence.observed_at().split('.');
    let seconds = observed_parts
        .next()
        .expect("seconds")
        .parse::<u128>()
        .expect("numeric seconds");
    let nanos = observed_parts
        .next()
        .expect("nanoseconds")
        .parse::<u128>()
        .expect("numeric nanoseconds");
    assert_eq!(observed_parts.next(), None);
    assert!(seconds * 1_000_000_000 + nanos >= completed);
}

#[test]
fn cloned_clients_share_a_gate_held_through_the_complete_transport_call() {
    let (starts_tx, starts_rx) = mpsc::channel();
    let (releases_tx, releases_rx) = mpsc::channel();
    let interval = Duration::from_millis(75);
    let client = IwencaiClient::from_parts(
        Arc::from("fixture-key"),
        Arc::new(BlockingTransport {
            response: HttpResponse::new(200, FIXTURE.as_bytes().to_vec()),
            starts: starts_tx,
            releases: Mutex::new(releases_rx),
        }),
        interval,
    );
    let first = {
        let client = client.clone();
        std::thread::spawn(move || client.semantic_search(&request(50)))
    };
    let first_started = starts_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first request enters transport");
    let second = {
        let client = client.clone();
        std::thread::spawn(move || client.semantic_search(&request(50)))
    };
    assert!(
        starts_rx.recv_timeout(Duration::from_millis(100)).is_err(),
        "the second clone must not enter while the first transport call is reading"
    );
    releases_tx.send(()).expect("release first request");
    let second_started = starts_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("second request enters after the first completes");
    assert!(second_started.duration_since(first_started) >= interval);
    releases_tx.send(()).expect("release second request");
    first.join().expect("first thread").expect("first request");
    second
        .join()
        .expect("second thread")
        .expect("second request");
    let snapshot = client.load_probe_snapshot().expect("probe snapshot");
    assert_eq!(snapshot.request_starts(), 2);
    assert_eq!(snapshot.maximum_concurrency(), 1);
    assert_eq!(snapshot.active_requests(), 0);
    assert!(snapshot.minimum_start_gap().expect("two request starts") >= interval);
}
