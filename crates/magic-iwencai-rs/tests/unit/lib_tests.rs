use super::*;
use std::io::{self, Read};
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

#[derive(Debug)]
struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("fixture read failed"))
    }
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

#[test]
fn constructors_capabilities_validation_and_https_guard_are_covered() {
    assert!(matches!(
        IwencaiClient::with_timeout("key", Duration::ZERO),
        Err(IwencaiError::InvalidRequest(_))
    ));
    let network_client =
        IwencaiClient::with_timeout("key", Duration::from_millis(1)).expect("positive timeout");
    assert!(IwencaiClient::new("key").is_ok());
    let capabilities = IwencaiClient::research_capabilities();
    assert!(!capabilities.reports);
    assert!(!capabilities.consensus);
    assert!(!capabilities.semantic_search);
    assert!(!capabilities.pdf_download);

    let invalid_request = HttpRequest {
        url: "https://evil.test/x".into(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let transport =
        HttpsTransport::new(Duration::from_millis(1)).expect("positive timeout builds transport");
    assert!(matches!(
        transport.post(&invalid_request),
        Err(IwencaiError::InvalidRequest(_))
    ));
    assert!(format!("{network_client:?}").contains("[REDACTED]"));

    assert_eq!(
        validate_api_key("  key  ".into()).expect("trimmed key"),
        "key"
    );
    assert!(validate_api_key("two words".into()).is_err());
    assert!(validate_api_key("x".repeat(4097)).is_err());
    assert!(validate_base_url("https://openapi.iwencai.com/").is_ok());
    assert!(validate_base_url("https://evil.test").is_err());
    assert!(ensure_official_url("https://openapi.iwencai.com/").is_err());
    assert!(ensure_official_url("https://openapi.iwencai.com//x").is_err());
    assert!(ensure_official_url("https://openapi.iwencai.com/x\n").is_err());
}

#[test]
fn environment_constructor_handles_missing_alias_and_invalid_base_explicitly() {
    const NAMES: [&str; 4] = [
        "MAGIC_IWENCAI_BASE_URL",
        "IWENCAI_BASE_URL",
        "MAGIC_IWENCAI_API_KEY",
        "IWENCAI_API_KEY",
    ];
    let originals = NAMES.map(std::env::var_os);
    for name in NAMES {
        std::env::remove_var(name);
    }

    assert!(matches!(
        IwencaiClient::from_env(),
        Err(IwencaiError::Authentication(_))
    ));
    std::env::set_var("IWENCAI_BASE_URL", "https://evil.test");
    std::env::set_var("IWENCAI_API_KEY", "alias-key");
    assert!(matches!(
        IwencaiClient::from_env(),
        Err(IwencaiError::InvalidRequest(_))
    ));
    std::env::set_var("IWENCAI_BASE_URL", OFFICIAL_BASE_URL);
    assert!(IwencaiClient::from_env().is_ok());

    for (name, original) in NAMES.into_iter().zip(originals) {
        if let Some(value) = original {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }
}

#[test]
fn bounded_http_reader_preserves_status_content_type_size_and_io_failures() {
    let ok = read_http_response(200, Some("application/json"), &b"{}"[..])
        .expect("valid bounded response");
    assert_eq!(ok.status(), 200);
    assert_eq!(ok.body(), b"{}");
    let denied = read_http_response(401, Some("text/plain"), &b"denied"[..])
        .expect("non-success bodies retain status for typed authentication");
    assert_eq!(denied.status(), 401);
    assert_eq!(denied.body(), b"denied");
    assert!(matches!(
        read_http_response(200, Some("text/html"), &b"{}"[..]),
        Err(IwencaiError::Protocol(_))
    ));
    assert!(matches!(
        read_http_response(200, Some("application/json"), FailingReader),
        Err(IwencaiError::Transport(_))
    ));
    assert!(matches!(
        read_http_response(500, None, vec![b'x'; MAX_RESPONSE_BYTES + 1].as_slice()),
        Err(IwencaiError::Protocol(_))
    ));
}

#[test]
fn channels_trace_ids_and_status_routes_are_explicit() {
    assert_eq!(channel_name(SemanticChannel::Report), "report");
    assert_eq!(channel_name(SemanticChannel::News), "news");
    assert_eq!(channel_name(SemanticChannel::Announcement), "announcement");
    assert_eq!(channel_name(SemanticChannel::General), "general");
    assert!(build_request(&request(1), "key", "short").is_err());
    assert!(build_request(&request(1), "key", &"z".repeat(64)).is_err());
    let generated = trace_id().expect("clock after epoch");
    assert_eq!(generated.len(), 64);
    assert!(generated.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(now().expect("clock after epoch").contains('.'));

    for (status, expected_auth) in [(403, true), (500, false)] {
        let client = IwencaiClient::with_transport(
            "key",
            FixtureTransport {
                response: HttpResponse::new(status, b"{}".to_vec()),
                request: Mutex::new(None),
            },
        )
        .expect("client");
        let error = client
            .semantic_search(&request(1))
            .expect_err("non-success status must fail");
        assert_eq!(
            matches!(error, IwencaiError::Authentication(_)),
            expected_auth
        );
    }
}

#[test]
fn response_envelope_and_non_auth_protocol_errors_fail_closed() {
    for body in [
        "{",
        r#"{"data":[]}"#,
        r#"{"status_code":{},"data":[]}"#,
        r#"{"status_code":0}"#,
        r#"{"status_code":0,"data":null}"#,
        r#"{"status_code":0,"data":{}}"#,
    ] {
        assert!(parse_response(body.as_bytes(), SemanticChannel::General, 1, "observed").is_err());
    }

    let protocol = parse_response(
        br#"{"status_code":7,"status_msg":"upstream failure","data":[]}"#,
        SemanticChannel::General,
        1,
        "observed",
    )
    .expect_err("non-auth status");
    assert!(matches!(protocol, IwencaiError::Protocol(_)));
    let auth = parse_response(
        br#"{"status_code":7,"status_msg":"missing apikey","data":[]}"#,
        SemanticChannel::General,
        1,
        "observed",
    )
    .expect_err("API-key message");
    assert!(matches!(auth, IwencaiError::Authentication(_)));

    let numeric_status_as_text = FIXTURE.replace(r#""status_code": 0"#, r#""status_code": "0""#);
    assert!(parse_response(
        numeric_status_as_text.as_bytes(),
        SemanticChannel::Report,
        50,
        "observed"
    )
    .is_ok());
}

#[test]
fn document_alias_extra_score_and_deduplication_paths_are_covered() {
    let alias_fixture = r#"{
      "status_code":0,
      "data":[{
        "id":42,
        "name":"  Alias   Title ",
        "content":" body ",
        "score":"0.75",
        "extra":"{\"link\":\"https://www.iwencai.com/doc/42\"}"
      }]
    }"#;
    let batch = parse_response(
        alias_fixture.as_bytes(),
        SemanticChannel::News,
        1,
        "observed",
    )
    .expect("aliases and extra URL");
    let document = &batch.records()[0];
    assert_eq!(document.document_id.as_str(), "42");
    assert_eq!(document.title.as_str(), "Alias Title");
    assert_eq!(
        document.excerpt.as_ref().map(NonEmptyText::as_str),
        Some("body")
    );
    assert_eq!(document.published_at, None);
    assert_eq!(document.evidence.source_at(), None);

    let lower_duplicate = FIXTURE.replace(r#""score": 0.9"#, r#""score": 0.1"#);
    let batch = parse_response(
        lower_duplicate.as_bytes(),
        SemanticChannel::Report,
        50,
        "observed",
    )
    .expect("lower duplicate is ignored");
    assert_eq!(batch.records()[0].title.as_str(), "人形机器人行业研究");

    for row in [
        Value::from(7),
        serde_json::json!({"title":"title","url":"https://www.iwencai.com/x"}),
        serde_json::json!({"uid":"id","url":"https://www.iwencai.com/x"}),
        serde_json::json!({"uid":"id","title":"title"}),
        serde_json::json!({"uid":"id","title":"title","url":"http://iwencai.com/x"}),
        serde_json::json!({"uid":"id","title":"title","url":"https://www.iwencai.com/x","score":"NaN"}),
    ] {
        assert!(parse_document(&row, SemanticChannel::General, "observed", "batch").is_err());
    }

    assert_eq!(parse_extra(None).expect("missing extra"), None);
    assert_eq!(parse_extra(Some(&Value::Null)).expect("null extra"), None);
    assert_eq!(
        parse_extra(Some(&Value::from("   "))).expect("blank extra"),
        None
    );
    assert!(matches!(
        parse_extra(Some(&Value::from("{"))),
        Err(IwencaiError::Decode(_))
    ));
    assert!(matches!(
        parse_extra(Some(&Value::from("[]"))),
        Err(IwencaiError::Protocol(_))
    ));
    assert!(matches!(
        parse_extra(Some(&Value::from(7))),
        Err(IwencaiError::Protocol(_))
    ));
}

#[test]
fn sequential_client_pacing_and_non_text_aliases_are_explicit() {
    let interval = Duration::from_millis(20);
    let client = IwencaiClient::from_parts(
        Arc::from("fixture-key"),
        Arc::new(FixtureTransport {
            response: HttpResponse::new(200, FIXTURE.as_bytes().to_vec()),
            request: Mutex::new(None),
        }),
        interval,
    );
    client
        .semantic_search(&request(50))
        .expect("first fixture request");
    let second_started = Instant::now();
    client
        .semantic_search(&request(50))
        .expect("second fixture request");
    assert!(second_started.elapsed() >= interval);

    let object = serde_json::json!({"uid": true});
    assert_eq!(
        text_alias(object.as_object().expect("object"), &["uid"]),
        None
    );
}
