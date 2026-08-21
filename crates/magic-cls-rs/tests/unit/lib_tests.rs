use super::*;
use std::io::{self, Read};
use std::sync::{mpsc, Mutex};

const FIXTURE: &str = r#"{
      "errno": 0,
      "data": {"roll_data": [{
        "id": 2435468,
        "title": "晶晨股份：预计半年度净利润增长",
        "brief": "财联社电报摘要",
        "content": "财联社电报全文",
        "ctime": 1784809706,
        "shareurl": "https://api3.cls.cn/share/article/2435468?os=web",
        "stock_list": [{"StockID": "sh688099"}],
        "subjects": [{"subject_name": "A股公告速递"}],
        "tags": ["业绩"]
      }]}
    }"#;

#[derive(Debug)]
struct FixtureTransport {
    response: Vec<u8>,
    request: Mutex<Option<HttpRequest>>,
}

impl ClsTransport for FixtureTransport {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, ClsError> {
        *self
            .request
            .lock()
            .map_err(|_| ClsError::Transport("fixture lock poisoned".into()))? =
            Some(request.clone());
        Ok(self.response.clone())
    }
}

#[derive(Debug)]
struct BlockingTransport {
    response: Vec<u8>,
    starts: mpsc::Sender<Instant>,
    releases: Mutex<mpsc::Receiver<()>>,
}

#[derive(Debug)]
struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("fixture read failed"))
    }
}

impl ClsTransport for BlockingTransport {
    fn get(&self, _request: &HttpRequest) -> Result<Vec<u8>, ClsError> {
        self.starts
            .send(Instant::now())
            .map_err(|error| ClsError::Transport(error.to_string()))?;
        self.releases
            .lock()
            .map_err(|_| ClsError::Transport("release lock poisoned".into()))?
            .recv()
            .map_err(|error| ClsError::Transport(error.to_string()))?;
        Ok(self.response.clone())
    }
}

#[test]
fn signed_request_and_fixture_map_every_contract_field() {
    let transport = FixtureTransport {
        response: FIXTURE.as_bytes().to_vec(),
        request: Mutex::new(None),
    };
    let client = ClsClient::with_transport(transport);
    let batch = client
        .fetch_global_news(PositiveU32::new(1).expect("positive"))
        .expect("fixture should parse");
    let item = &batch.records()[0];
    assert_eq!(item.item_id.as_str(), "2435468");
    assert_eq!(item.title.as_str(), "晶晨股份：预计半年度净利润增长");
    assert_eq!(
        item.summary.as_ref().map(NonEmptyText::as_str),
        Some("财联社电报摘要")
    );
    assert_eq!(
        item.content.as_ref().map(NonEmptyText::as_str),
        Some("财联社电报全文")
    );
    assert_eq!(item.publisher.as_str(), "财联社");
    assert_eq!(
        item.canonical_url.as_str(),
        "https://api3.cls.cn/share/article/2435468?os=web"
    );
    assert_eq!(item.published_at.as_str(), "2026-07-23T20:28:26+08:00");
    assert_eq!(item.instruments[0].code(), "688099");
    assert_eq!(item.topics.len(), 2);
    assert_eq!(item.language.as_str(), "zh-CN");
    assert_eq!(item.evidence.provider(), ProviderId::Cailianpress);
    assert_eq!(item.evidence.source_at(), Some("1784809706"));
    assert_eq!(batch.provenance().source_at(), Some("1784809706"));
}

#[test]
fn signing_matches_the_verified_cls_algorithm() {
    let request = build_request(2);
    assert_eq!(
            request.url(),
            "https://www.cls.cn/v1/roll/get_roll_list?appName=CailianpressWeb&last_time=&os=web&refresh_type=1&rn=2&sv=7.7.5&sign=681656257b917ff407cb7444df747354"
        );
}

#[test]
fn errno_and_oversized_page_are_explicit_failures() {
    let error = parse_response(
        br#"{"errno":1001,"errmsg":"bad sign","data":{"roll_data":[]}}"#,
        1,
        "observed",
    )
    .expect_err("errno must fail");
    assert!(matches!(
        error,
        ClsError::ProviderRejected { errno: 1001, message } if message == "bad sign"
    ));
    for body in [
        br#"{"errno":1001,"errmsg":7}"#.as_slice(),
        format!(r#"{{"errno":1001,"errmsg":"{}"}}"#, "x".repeat(257)).as_bytes(),
        br#"{"errno":1001,"errmsg":"bad\u0000sign"}"#.as_slice(),
    ] {
        assert!(matches!(
            parse_response(body, 1, "observed"),
            Err(ClsError::Protocol(_))
        ));
    }
    assert!(ClsClient::with_transport(FixtureTransport {
        response: FIXTURE.as_bytes().to_vec(),
        request: Mutex::new(None),
    })
    .fetch_global_news(PositiveU32::new(51).expect("positive"))
    .is_err());
    let oversized = ClsClient::with_transport(FixtureTransport {
        response: vec![b' '; MAX_RESPONSE_BYTES + 1],
        request: Mutex::new(None),
    })
    .fetch_global_news(PositiveU32::new(1).expect("positive"))
    .expect_err("injected transports cannot bypass the response cap");
    assert!(matches!(oversized, ClsError::Protocol(_)));
}

#[test]
fn unordered_source_times_are_rejected() {
    let body = br#"{
          "errno":0,
          "data":{"roll_data":[
            {"id":1,"brief":"older","content":"older","ctime":1784809706,"shareurl":"https://api3.cls.cn/share/article/1","stock_list":[],"subjects":[],"tags":[]},
            {"id":2,"brief":"newer","content":"newer","ctime":1784809800,"shareurl":"https://api3.cls.cn/share/article/2","stock_list":[],"subjects":[],"tags":[]}
          ]}
        }"#;
    assert!(matches!(
        parse_response(body, 2, "observed"),
        Err(ClsError::Protocol(_))
    ));
}

#[test]
fn only_official_cls_transport_host_is_allowed() {
    assert!(ensure_official_url("https://www.cls.cn/v1/roll/get_roll_list").is_ok());
    assert!(ensure_official_url("https://www.cls.cn.evil.test/x").is_err());
    assert!(ensure_official_url("http://www.cls.cn/x").is_err());
    assert!(ensure_json_content_type(Some("application/json; charset=utf-8")).is_ok());
    assert!(ensure_json_content_type(Some("text/html")).is_err());
    assert!(ensure_json_content_type(None).is_err());
}

#[test]
fn present_but_malformed_stock_and_topic_metadata_is_rejected() {
    for body in [
        FIXTURE.replace(
            r#""stock_list": [{"StockID": "sh688099"}]"#,
            r#""stock_list": {"StockID": "sh688099"}"#,
        ),
        FIXTURE.replace(
            r#""stock_list": [{"StockID": "sh688099"}]"#,
            r#""stock_list": [{"StockID": 688099}]"#,
        ),
        FIXTURE.replace(
            r#""subjects": [{"subject_name": "A股公告速递"}]"#,
            r#""subjects": [{"subject_name": 7}]"#,
        ),
        FIXTURE.replace(r#""tags": ["业绩"]"#, r#""tags": [{"unknown": "业绩"}]"#),
    ] {
        assert!(matches!(
            parse_response(body.as_bytes(), 1, "observed"),
            Err(ClsError::Protocol(_))
        ));
    }
}

#[test]
fn associated_instruments_preserve_verified_asset_classes() {
    let body = FIXTURE.replace(
        r#""stock_list": [{"StockID": "sh688099"}]"#,
        r#""stock_list": [
              {"StockID": "sh510050"},
              {"StockID": "sh000001"},
              {"StockID": "sh688099"},
              {"StockID": "sz159915"},
              {"StockID": "sz399001"},
              {"StockID": "sz000001"},
              {"StockID": "bj920118"}
            ]"#,
    );
    let batch = parse_response(body.as_bytes(), 1, "observed").expect("fixture parses");
    let instruments = &batch.records()[0].instruments;
    assert_eq!(instruments.len(), 7);
    assert_eq!(instruments[0].code(), "510050");
    assert_eq!(instruments[0].asset_class(), AssetClass::Fund);
    assert_eq!(instruments[1].code(), "000001");
    assert_eq!(instruments[1].asset_class(), AssetClass::Index);
    assert_eq!(instruments[2].code(), "688099");
    assert_eq!(instruments[2].asset_class(), AssetClass::Equity);
    assert_eq!(instruments[3].asset_class(), AssetClass::Fund);
    assert_eq!(instruments[4].asset_class(), AssetClass::Index);
    assert_eq!(instruments[5].asset_class(), AssetClass::Equity);
    assert_eq!(instruments[6].asset_class(), AssetClass::Equity);

    let unverified = FIXTURE.replace(
        r#""stock_list": [{"StockID": "sh688099"}]"#,
        r#""stock_list": [{"StockID": "sh900901"}]"#,
    );
    assert!(matches!(
        parse_response(unverified.as_bytes(), 1, "observed"),
        Err(ClsError::Protocol(message)) if message.contains("unverified")
    ));
}

#[test]
fn official_suffix_stock_ids_normalize_and_deduplicate() {
    let rows = serde_json::json!([
        {"StockID": "sh600000"},
        {"StockID": "sz000001"},
        {"StockID": "bj920403"},
        {"StockID": "920403.BJ"},
        {"StockID": "920344.BJ"}
    ]);
    let instruments = parse_instruments(Some(&rows)).expect("official CLS identities parse");
    assert_eq!(instruments.len(), 4);
    assert_eq!(instruments[0].exchange(), Exchange::Shanghai);
    assert_eq!(instruments[0].code(), "600000");
    assert_eq!(instruments[1].exchange(), Exchange::Shenzhen);
    assert_eq!(instruments[1].code(), "000001");
    assert_eq!(instruments[2].exchange(), Exchange::Beijing);
    assert_eq!(instruments[2].code(), "920403");
    assert_eq!(instruments[3].exchange(), Exchange::Beijing);
    assert_eq!(instruments[3].code(), "920344");

    for invalid in [
        "600000.SH",
        "000001.SZ",
        "920403.SH",
        "920403.HK",
        "920403.bj",
        "92040.BJ",
    ] {
        assert!(parse_instruments(Some(&serde_json::json!([{"StockID": invalid}]))).is_err());
    }
}

#[test]
fn cloned_clients_share_a_gate_held_through_the_complete_transport_call() {
    let (starts_tx, starts_rx) = mpsc::channel();
    let (releases_tx, releases_rx) = mpsc::channel();
    let interval = Duration::from_millis(75);
    let client = ClsClient::from_parts(
        Arc::new(BlockingTransport {
            response: FIXTURE.as_bytes().to_vec(),
            starts: starts_tx,
            releases: Mutex::new(releases_rx),
        }),
        interval,
    );
    let first = {
        let client = client.clone();
        std::thread::spawn(move || client.global_news(PositiveU32::new(1).expect("positive")))
    };
    let first_started = starts_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first request enters transport");
    let second = {
        let client = client.clone();
        std::thread::spawn(move || client.global_news(PositiveU32::new(1).expect("positive")))
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
fn constructors_debug_capabilities_and_unsupported_instrument_route_are_covered() {
    assert!(matches!(
        ClsClient::with_timeout(Duration::ZERO),
        Err(ClsError::InvalidRequest(_))
    ));
    let network_client =
        ClsClient::with_timeout(Duration::from_millis(1)).expect("positive timeout");
    assert!(format!("{network_client:?}").contains("ClsClient"));
    assert!(ClsClient::new().is_ok());
    let capabilities = ClsClient::content_capabilities();
    assert!(capabilities.global_news);
    assert!(!capabilities.instrument_news);
    assert!(!capabilities.announcements);
    assert!(!capabilities.investor_questions);

    let invalid_request = HttpRequest {
        url: "https://evil.test/x".into(),
        headers: Vec::new(),
    };
    let transport =
        HttpsTransport::new(Duration::from_millis(1)).expect("positive timeout builds transport");
    assert!(matches!(
        transport.get(&invalid_request),
        Err(ClsError::InvalidRequest(_))
    ));

    let instrument =
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).expect("instrument");
    let request =
        InstrumentDateRangeRequest::new(instrument, PositiveU32::new(1).expect("positive"))
            .expect("request");
    assert!(matches!(
        network_client.instrument_news(&request),
        Err(ClsError::Unsupported(_))
    ));
}

#[test]
fn bounded_http_reader_preserves_status_content_type_size_and_io_failures() {
    assert_eq!(
        read_http_response(200, Some("application/json"), &b"{}"[..])
            .expect("valid bounded response"),
        b"{}"
    );
    assert!(matches!(
        read_http_response(429, Some("application/json"), &b"{}"[..]),
        Err(ClsError::HttpStatus(429))
    ));
    assert!(matches!(
        read_http_response(200, Some("text/html"), &b"{}"[..]),
        Err(ClsError::Protocol(_))
    ));
    assert!(matches!(
        read_http_response(200, Some("application/json"), FailingReader),
        Err(ClsError::Transport(_))
    ));
    assert!(matches!(
        read_http_response(
            200,
            Some("application/json"),
            vec![b'x'; MAX_RESPONSE_BYTES + 1].as_slice()
        ),
        Err(ClsError::Protocol(_))
    ));
    assert!(ensure_official_url("https://www.cls.cn/").is_err());
    assert!(ensure_official_url("https://www.cls.cn//x").is_err());
    assert!(ensure_official_url("https://www.cls.cn/x\n").is_err());
}

#[test]
fn response_envelope_batch_bounds_identity_and_fallback_text_fail_closed() {
    for body in [
        "{",
        r#"{"data":{"roll_data":[]}}"#,
        r#"{"errno":"0","data":{"roll_data":[]}}"#,
        r#"{"errno":0,"data":{"roll_data":null}}"#,
        r#"{"errno":0,"data":{"roll_data":[]}}"#,
    ] {
        assert!(parse_response(body.as_bytes(), 1, "observed").is_err());
    }
    assert!(parse_response(FIXTURE.as_bytes(), 0, "observed").is_err());

    let string_id = FIXTURE.replace(r#""id": 2435468"#, r#""id": "string-id""#);
    assert_eq!(
        parse_response(string_id.as_bytes(), 1, "observed")
            .expect("string id")
            .records()[0]
            .item_id
            .as_str(),
        "string-id"
    );
    let brief_fallback = FIXTURE
        .replace(
            r#""title": "晶晨股份：预计半年度净利润增长","#,
            r#""title": "  ","#,
        )
        .replace(r#""content": "财联社电报全文","#, r#""content": null,"#);
    let batch = parse_response(brief_fallback.as_bytes(), 1, "observed").expect("brief fallback");
    let item = &batch.records()[0];
    assert_eq!(item.title.as_str(), "财联社电报摘要");
    assert_eq!(
        item.content.as_ref().map(NonEmptyText::as_str),
        Some("财联社电报摘要")
    );

    for body in [
        FIXTURE.replace(r#""id": 2435468"#, r#""id": null"#),
        FIXTURE
            .replace(
                r#""title": "晶晨股份：预计半年度净利润增长""#,
                r#""title": null"#,
            )
            .replace(r#""brief": "财联社电报摘要""#, r#""brief": "  ""#),
        FIXTURE.replace(r#""ctime": 1784809706"#, r#""ctime": 0"#),
        FIXTURE.replace(
            r#""shareurl": "https://api3.cls.cn/share/article/2435468?os=web""#,
            r#""shareurl": " ""#,
        ),
        FIXTURE.replace(
            r#""shareurl": "https://api3.cls.cn/share/article/2435468?os=web""#,
            r#""shareurl": "http://api3.cls.cn/share/article/2435468""#,
        ),
    ] {
        assert!(parse_response(body.as_bytes(), 1, "observed").is_err());
    }

    let duplicate = FIXTURE.replace(
        r#"}]}"#,
        r#"},{
          "id":2435468,"brief":"duplicate","ctime":1784809600,
          "shareurl":"https://api3.cls.cn/share/article/2435468",
          "stock_list":[],"subjects":[],"tags":[]
        }]}"#,
    );
    assert!(parse_response(duplicate.as_bytes(), 2, "observed").is_err());
}

#[test]
fn associated_identity_and_topic_helpers_cover_optional_and_rejected_shapes() {
    assert!(parse_instruments(None)
        .expect("missing instruments")
        .is_empty());
    assert!(parse_instruments(Some(&Value::Null))
        .expect("null instruments")
        .is_empty());
    assert!(parse_instruments(Some(&Value::from("bad"))).is_err());
    assert!(parse_instruments(Some(&serde_json::json!([7]))).is_err());
    assert!(parse_instruments(Some(&serde_json::json!([{"StockID":" "}]))).is_err());
    assert!(parse_instruments(Some(&serde_json::json!([{"StockID":"xx600000"}]))).is_err());
    assert!(parse_instruments(Some(&serde_json::json!([{"StockID":"sh60000x"}]))).is_err());

    let duplicate = serde_json::json!([
        {"StockID":"sh600000"},
        {"StockID":"sh600000"}
    ]);
    assert_eq!(
        parse_instruments(Some(&duplicate))
            .expect("duplicates collapse")
            .len(),
        1
    );
    for (exchange, code, expected) in [
        (Exchange::Shanghai, "601000", AssetClass::Equity),
        (Exchange::Shanghai, "603000", AssetClass::Equity),
        (Exchange::Shanghai, "605000", AssetClass::Equity),
        (Exchange::Shenzhen, "001001", AssetClass::Equity),
        (Exchange::Shenzhen, "002001", AssetClass::Equity),
        (Exchange::Shenzhen, "003001", AssetClass::Equity),
        (Exchange::Shenzhen, "300001", AssetClass::Equity),
        (Exchange::Shenzhen, "301001", AssetClass::Equity),
        (Exchange::Beijing, "430001", AssetClass::Equity),
        (Exchange::Beijing, "830001", AssetClass::Equity),
    ] {
        assert_eq!(
            classify_associated_asset(exchange, code).expect("verified asset"),
            expected
        );
    }
    assert!(classify_associated_asset(Exchange::Shanghai, "900901").is_err());

    assert!(parse_topics(None, Some(&Value::Null))
        .expect("missing topics")
        .is_empty());
    let topics = parse_topics(
        Some(&serde_json::json!([
            {"subject_name":"  业绩   增长  "},
            {"subject_name":"业绩 增长"}
        ])),
        Some(&serde_json::json!([
            {"name":"机构"},
            "机构"
        ])),
    )
    .expect("topics normalize and deduplicate");
    assert_eq!(topics.len(), 2);
    assert_eq!(topics[0].as_str(), "业绩 增长");
    assert_eq!(topics[1].as_str(), "机构");
    assert!(parse_topics(Some(&serde_json::json!([7])), None).is_err());
    assert!(optional_array(Some(&Value::from("bad")), "tags").is_err());
}

#[test]
fn time_conversion_is_bounded_and_now_is_auditable() {
    assert_eq!(
        unix_seconds_to_china_rfc3339(0).expect("epoch"),
        "1970-01-01T08:00:00+08:00"
    );
    assert!(unix_seconds_to_china_rfc3339(i64::MAX).is_err());
    assert!(unix_seconds_to_china_rfc3339(i64::MIN).is_err());
    assert!(now().expect("clock after epoch").contains('.'));
}
