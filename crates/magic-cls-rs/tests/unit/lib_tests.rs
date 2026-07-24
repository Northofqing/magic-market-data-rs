use super::*;
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
    assert_eq!(
        batch.provenance().source_at(),
        Some("2026-07-23T20:28:26+08:00")
    );
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
    assert!(matches!(error, ClsError::Protocol(_)));
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
