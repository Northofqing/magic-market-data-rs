use super::*;
use crate::transport::{collect_transport_result, read_http_response, HttpsTransport};
use std::collections::VecDeque;
use std::io::{self, Read};

#[derive(Clone)]
struct FixtureTransport {
    responses: Arc<Mutex<VecDeque<HttpResponse>>>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("fixture read failed"))
    }
}

impl FixtureTransport {
    fn new(responses: Vec<HttpResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ThsTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ThsError> {
        self.requests.lock().unwrap().push(request.clone());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| ThsError::Transport("fixture response exhausted".into()))
    }
}

#[derive(Clone)]
struct CompletionTransport {
    inner: FixtureTransport,
    completed_at: Arc<Mutex<Option<u128>>>,
}

impl CompletionTransport {
    fn new(responses: Vec<HttpResponse>) -> Self {
        Self {
            inner: FixtureTransport::new(responses),
            completed_at: Arc::new(Mutex::new(None)),
        }
    }
}

impl ThsTransport for CompletionTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ThsError> {
        let response = self.inner.execute(request)?;
        let completed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| ThsError::Transport(error.to_string()))?
            .as_nanos();
        *self
            .completed_at
            .lock()
            .map_err(|_| ThsError::Transport("completion lock poisoned".into()))? =
            Some(completed_at);
        Ok(response)
    }
}

fn json_response(url: &str, fixture: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        final_url: url.into(),
        content_type: Some("application/json;charset=UTF-8".into()),
        body: fixture.as_bytes().to_vec(),
    }
}

fn html_response(url: &str, fixture: &str) -> HttpResponse {
    let (body, _, had_errors) = GBK.encode(fixture);
    assert!(!had_errors);
    HttpResponse {
        status: 200,
        final_url: url.into(),
        content_type: Some("text/html; charset=GBK".into()),
        body: body.into_owned(),
    }
}

fn sh(code: &str) -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap()
}

fn timestamp_nanos(value: &str) -> u128 {
    let (seconds, nanos) = value.split_once('.').unwrap();
    seconds.parse::<u128>().unwrap() * 1_000_000_000 + nanos.parse::<u128>().unwrap()
}

#[test]
fn named_consensus_table_maps_each_years_count_and_eps_range() {
    let url = "https://basic.10jqka.com.cn/600519/worth.html";
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![html_response(
        url,
        include_str!("../fixtures/consensus_600519.html"),
    )]));
    let batch = client.consensus(&[sh("600519")]).unwrap();
    assert_eq!(batch.records().len(), 1);
    let record = &batch.records()[0];
    assert_eq!(record.name.as_str(), "贵州茅台");
    assert_eq!(record.estimates.len(), 3);
    assert_eq!(
        record.estimates[0]
            .contributor_count()
            .map(PositiveU32::get),
        Some(46)
    );
    assert_eq!(
        record.estimates[0].eps_min().map(FiniteNumber::get),
        Some(65.02)
    );
    assert_eq!(
        record.estimates[0].eps().map(FiniteNumber::get),
        Some(68.73)
    );
    assert_eq!(
        record.estimates[0].eps_max().map(FiniteNumber::get),
        Some(77.85)
    );
    assert_eq!(record.contributor_count.map(PositiveU32::get), None);
    assert_eq!(record.evidence.source_at(), Some("2026-07-23"));
    assert!(batch.quality().is_complete());
}

#[test]
fn code_prefix_must_match_the_declared_exchange() {
    let mismatches = [
        (Exchange::Shanghai, "002594"),
        (Exchange::Shenzhen, "600396"),
        (Exchange::Beijing, "300001"),
    ];
    for (exchange, code) in mismatches {
        let instrument = InstrumentId::new(exchange, code, AssetClass::Equity).unwrap();
        assert!(matches!(
            validate_equity(&instrument),
            Err(ThsError::InvalidRequest(message)) if message.contains("exchange")
        ));
    }
    assert!(matches!(
        validate_equity(&sh("100001")),
        Err(ThsError::Unsupported(message)) if message.contains("prefix")
    ));

    let verified_beijing =
        InstrumentId::new(Exchange::Beijing, "920001", AssetClass::Equity).unwrap();
    assert!(validate_equity(&verified_beijing).is_ok());
    assert_eq!(
        equity_from_code("920001").unwrap().exchange(),
        Exchange::Beijing
    );

    let unverified_nine_prefix =
        InstrumentId::new(Exchange::Shanghai, "900901", AssetClass::Equity).unwrap();
    assert!(matches!(
        validate_equity(&unverified_nine_prefix),
        Err(ThsError::Unsupported(message)) if message.contains("prefix")
    ));
    assert!(matches!(
        equity_from_code("900901"),
        Err(ThsError::Schema(message)) if message.contains("unsupported venue prefix")
    ));
}

#[test]
fn consensus_observation_time_is_not_before_the_final_response() {
    let url = "https://basic.10jqka.com.cn/600519/worth.html";
    let transport = CompletionTransport::new(vec![html_response(
        url,
        include_str!("../fixtures/consensus_600519.html"),
    )]);
    let observed = transport.clone();
    let client = ThsClient::with_test_transport(transport);
    let batch = client.consensus(&[sh("600519")]).unwrap();
    let completed_at = observed.completed_at.lock().unwrap().unwrap();
    assert!(timestamp_nanos(batch.provenance().fetched_at()) >= completed_at);
    let snapshot = client.load_probe_snapshot().unwrap();
    assert_eq!(snapshot.request_starts(), 1);
    assert_eq!(snapshot.maximum_concurrency(), 1);
    assert_eq!(snapshot.active_requests(), 0);
}

#[test]
fn no_consensus_coverage_is_source_verified_empty_without_a_pseudo_record() {
    let url = "https://basic.10jqka.com.cn/600396/worth.html";
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![html_response(
        url,
        include_str!("../fixtures/consensus_none_600396.html"),
    )]));
    let result = client.consensus(&[sh("600396")]);
    let ThsError::VerifiedEmpty(empty) = result.unwrap_err() else {
        panic!("explicit source empty must use the typed VerifiedEmpty variant");
    };
    assert_eq!(empty.family(), "consensus");
    assert_eq!(empty.request_identity(), "600396.SH");
    assert_eq!(empty.evidence().source_at(), Some("2026-07-23"));
    assert!(empty
        .reason()
        .contains("no current institutional consensus"));
}

#[test]
fn mixed_consensus_availability_rejects_the_atomic_batch() {
    let valid_url = "https://basic.10jqka.com.cn/600519/worth.html";
    let empty_url = "https://basic.10jqka.com.cn/600396/worth.html";
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![
        html_response(valid_url, include_str!("../fixtures/consensus_600519.html")),
        html_response(
            empty_url,
            include_str!("../fixtures/consensus_none_600396.html"),
        ),
    ]));

    assert!(matches!(
        client.consensus(&[sh("600519"), sh("600396")]),
        Err(ThsError::Incomplete(message))
            if message.contains("atomic consensus batch rejected")
                && message.contains("600396")
    ));
}

#[test]
fn strong_reason_preserves_editorial_reason_and_themes() {
    let date = magic_market_core::IsoDate::new("2026-07-22").unwrap();
    let expected_url = format!(
            "{DEFAULT_STRONG_ORIGIN}/event/api/getharden/date/{}/orderby/date/orderway/desc/charset/GBK/",
            date.as_str()
        );
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(
        &expected_url,
        include_str!("../fixtures/strong_20260722.json"),
    )]));
    let request = InstrumentSignalRequest::new(
        InstrumentId::new(Exchange::Shenzhen, "000815", AssetClass::Equity).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap()
    .with_trading_date(date);
    let batch = client.strong_stock_reasons(&request).unwrap();
    assert_eq!(batch.records().len(), 1);
    assert_eq!(
        batch.records()[0].reason.as_str(),
        "算力租赁+东数西算+中国诚通旗下"
    );
    assert_eq!(batch.records()[0].subjects.len(), 3);
}

#[test]
fn limit_reveal_maps_only_source_backed_normalized_fields() {
    let request = LimitPoolRequest::new(
        LimitPoolKind::Upper,
        magic_market_core::IsoDate::new("2026-07-22").unwrap(),
        PositiveU32::new(3).unwrap(),
    )
    .unwrap();
    let expected_url = {
        let mut url = Url::parse(DEFAULT_LIMIT_URL).unwrap();
        url.query_pairs_mut()
                .append_pair("page", "1")
                .append_pair("limit", "3")
                .append_pair(
                    "field",
                    "199112,10,9001,330323,330324,330325,9002,330329,133971,133970,1968584,3475914,9003,9004",
                )
                .append_pair("filter", "HS,GEM2STAR")
                .append_pair("order_field", "330324")
                .append_pair("order_type", "0")
                .append_pair("date", "20260722");
        url.to_string()
    };
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(
        &expected_url,
        include_str!("../fixtures/limit_20260722.json"),
    )]));
    let batch = client.limit_pool(&request).unwrap();
    let record = &batch.records()[0];
    assert_eq!(record.price.get(), 69.12);
    assert_eq!(record.change.get(), 20.0);
    assert_eq!(record.break_count, Some(8));
    assert_eq!(record.streak.map(PositiveU32::get), Some(1));
    assert!(record.board_name.is_none());
    assert_eq!(
        record.seal_state.as_ref().map(NonEmptyText::as_str),
        Some("换手板")
    );
    assert_eq!(
        record.reason.as_ref().map(NonEmptyText::as_str),
        Some("废塑料化学循环+固废处理+稀土永磁")
    );
    assert!(record.reseal_count.is_none());
}

#[test]
fn present_wrong_typed_limit_metadata_is_rejected() {
    let request = LimitPoolRequest::new(
        LimitPoolKind::Upper,
        magic_market_core::IsoDate::new("2026-07-22").unwrap(),
        PositiveU32::new(3).unwrap(),
    )
    .unwrap();
    let expected_url = {
        let mut url = Url::parse(DEFAULT_LIMIT_URL).unwrap();
        url.query_pairs_mut()
                .append_pair("page", "1")
                .append_pair("limit", "3")
                .append_pair(
                    "field",
                    "199112,10,9001,330323,330324,330325,9002,330329,133971,133970,1968584,3475914,9003,9004",
                )
                .append_pair("filter", "HS,GEM2STAR")
                .append_pair("order_field", "330324")
                .append_pair("order_type", "0")
                .append_pair("date", "20260722");
        url.to_string()
    };
    let fixture = include_str!("../fixtures/limit_20260722.json");
    for malformed in [
        fixture.replace(
            r#""reason_type": "废塑料化学循环+固废处理+稀土永磁""#,
            r#""reason_type": {"text":"废塑料化学循环"}"#,
        ),
        fixture.replace(
            r#""limit_up_type": "换手板""#,
            r#""limit_up_type": ["换手板"]"#,
        ),
        fixture.replace(r#""high_days": "首板""#, r#""high_days": true"#),
    ] {
        let client = ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(
            &expected_url,
            &malformed,
        )]));
        assert!(matches!(
            client.limit_pool(&request),
            Err(ThsError::Schema(_))
        ));
    }
}

#[test]
fn absent_or_null_limit_metadata_remains_none() {
    let request = LimitPoolRequest::new(
        LimitPoolKind::Upper,
        magic_market_core::IsoDate::new("2026-07-22").unwrap(),
        PositiveU32::new(3).unwrap(),
    )
    .unwrap();
    let expected_url = {
        let mut url = Url::parse(DEFAULT_LIMIT_URL).unwrap();
        url.query_pairs_mut()
                .append_pair("page", "1")
                .append_pair("limit", "3")
                .append_pair(
                    "field",
                    "199112,10,9001,330323,330324,330325,9002,330329,133971,133970,1968584,3475914,9003,9004",
                )
                .append_pair("filter", "HS,GEM2STAR")
                .append_pair("order_field", "330324")
                .append_pair("order_type", "0")
                .append_pair("date", "20260722");
        url.to_string()
    };
    let fixture = include_str!("../fixtures/limit_20260722.json")
        .replace(
            r#""reason_type": "废塑料化学循环+固废处理+稀土永磁","#,
            r#""reason_type": null,"#,
        )
        .replace(r#"        "limit_up_type": "换手板","#, "")
        .replace(r#""high_days": "首板""#, r#""high_days": null"#);
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(
        &expected_url,
        &fixture,
    )]));
    let batch = client
        .limit_pool(&request)
        .expect("null metadata is optional");
    let record = &batch.records()[0];
    assert!(record.reason.is_none());
    assert!(record.seal_state.is_none());
    assert!(record.streak.is_none());
}

#[test]
fn popularity_maps_rank_change_return_heat_concepts_and_tag() {
    let expected_url = {
        let mut url = Url::parse(DEFAULT_POPULARITY_URL).unwrap();
        url.query_pairs_mut()
            .append_pair("stock_type", "a")
            .append_pair("type", "hour")
            .append_pair("list_type", "normal");
        url.to_string()
    };
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(
        &expected_url,
        include_str!("../fixtures/popularity.json"),
    )]));
    let batch = client.popularity(PositiveU32::new(1).unwrap()).unwrap();
    let record = &batch.records()[0];
    assert_eq!(record.rank.get(), 1);
    assert_eq!(record.heat.map(FiniteNumber::get), Some(411_579.0));
    assert_eq!(record.rank_change.map(FiniteNumber::get), Some(0.0));
    assert_eq!(record.return_ratio.map(Ratio::get), Some(-4.8556));
    assert_eq!(record.concepts.len(), 2);
    assert_eq!(
        record.tag.as_ref().map(NonEmptyText::as_str),
        Some("持续上榜")
    );
}

#[test]
fn present_wrong_typed_popularity_metadata_is_rejected() {
    let expected_url = {
        let mut url = Url::parse(DEFAULT_POPULARITY_URL).unwrap();
        url.query_pairs_mut()
            .append_pair("stock_type", "a")
            .append_pair("type", "hour")
            .append_pair("list_type", "normal");
        url.to_string()
    };
    let fixture = include_str!("../fixtures/popularity.json");
    for malformed in [
        fixture.replace(
            r#""tag": {
          "concept_tag": ["存储芯片", "中芯国际概念"],
          "popularity_tag": "持续上榜"
        }"#,
            r#""tag": []"#,
        ),
        fixture.replace(
            r#""concept_tag": ["存储芯片", "中芯国际概念"]"#,
            r#""concept_tag": {"name":"存储芯片"}"#,
        ),
        fixture.replace(
            r#""concept_tag": ["存储芯片", "中芯国际概念"]"#,
            r#""concept_tag": [7]"#,
        ),
        fixture.replace(
            r#""popularity_tag": "持续上榜""#,
            r#""popularity_tag": {"name":"持续上榜"}"#,
        ),
        fixture.replace(r#""name": "德明利""#, r#""name": ["德明利"]"#),
    ] {
        let client = ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(
            &expected_url,
            &malformed,
        )]));
        assert!(matches!(
            client.popularity(PositiveU32::new(1).unwrap()),
            Err(ThsError::Schema(_))
        ));
    }
}

#[test]
fn absent_or_null_popularity_metadata_remains_none() {
    let expected_url = {
        let mut url = Url::parse(DEFAULT_POPULARITY_URL).unwrap();
        url.query_pairs_mut()
            .append_pair("stock_type", "a")
            .append_pair("type", "hour")
            .append_pair("list_type", "normal");
        url.to_string()
    };
    let fixture = include_str!("../fixtures/popularity.json")
        .replace(r#""name": "德明利""#, r#""name": null"#)
        .replace(
            r#""concept_tag": ["存储芯片", "中芯国际概念"]"#,
            r#""concept_tag": null"#,
        )
        .replace(
            r#""popularity_tag": "持续上榜""#,
            r#""popularity_tag": null"#,
        );
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(
        &expected_url,
        &fixture,
    )]));
    let batch = client
        .popularity(PositiveU32::new(1).unwrap())
        .expect("null metadata is optional");
    let record = &batch.records()[0];
    assert!(record.name.is_none());
    assert!(record.concepts.is_empty());
    assert!(record.tag.is_none());
}

#[test]
fn empty_or_unmatched_signal_results_are_explicitly_incomplete() {
    let date = magic_market_core::IsoDate::new("2026-07-22").unwrap();
    let strong_url = format!(
            "{DEFAULT_STRONG_ORIGIN}/event/api/getharden/date/{}/orderby/date/orderway/desc/charset/GBK/",
            date.as_str()
        );
    let strong = ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(
        &strong_url,
        include_str!("../fixtures/strong_20260722.json"),
    )]));
    let strong_request = InstrumentSignalRequest::new(sh("600396"), PositiveU32::new(1).unwrap())
        .unwrap()
        .with_trading_date(date.clone());
    assert!(matches!(
        strong.strong_stock_reasons(&strong_request),
        Err(ThsError::Incomplete(message)) if message.contains("no exact match")
    ));

    let limit_request =
        LimitPoolRequest::new(LimitPoolKind::Upper, date, PositiveU32::new(1).unwrap()).unwrap();
    let limit_url = {
        let mut url = Url::parse(DEFAULT_LIMIT_URL).unwrap();
        url.query_pairs_mut()
                .append_pair("page", "1")
                .append_pair("limit", "1")
                .append_pair(
                    "field",
                    "199112,10,9001,330323,330324,330325,9002,330329,133971,133970,1968584,3475914,9003,9004",
                )
                .append_pair("filter", "HS,GEM2STAR")
                .append_pair("order_field", "330324")
                .append_pair("order_type", "0")
                .append_pair("date", "20260722");
        url.to_string()
    };
    let limit = ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(
        &limit_url,
        r#"{"status_code":0,"data":{"info":[]}}"#,
    )]));
    assert!(matches!(
        limit.limit_pool(&limit_request),
        Err(ThsError::Incomplete(message)) if message.contains("empty")
    ));

    let popularity_url = {
        let mut url = Url::parse(DEFAULT_POPULARITY_URL).unwrap();
        url.query_pairs_mut()
            .append_pair("stock_type", "a")
            .append_pair("type", "hour")
            .append_pair("list_type", "normal");
        url.to_string()
    };
    let popularity = ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(
        &popularity_url,
        r#"{"status_code":0,"data":{"stock_list":[]}}"#,
    )]));
    assert!(matches!(
        popularity.popularity(PositiveU32::new(1).unwrap()),
        Err(ThsError::Incomplete(message)) if message.contains("no ranked stocks")
    ));
}

#[test]
fn hosts_redirects_html_login_and_bounds_are_explicit() {
    let config = ThsConfig {
        popularity_url: "https://example.com/hot".into(),
        ..ThsConfig::default()
    };
    assert!(matches!(
        ThsClient::with_transport(config, FixtureTransport::new(Vec::new())),
        Err(ThsError::InvalidRequest(message)) if message.contains("allowlisted")
    ));

    let expected_url = {
        let mut url = Url::parse(DEFAULT_POPULARITY_URL).unwrap();
        url.query_pairs_mut()
            .append_pair("stock_type", "a")
            .append_pair("type", "hour")
            .append_pair("list_type", "normal");
        url.to_string()
    };
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![HttpResponse {
        status: 200,
        final_url: expected_url.clone(),
        content_type: Some("text/html".into()),
        body: b"<html>login</html>".to_vec(),
    }]));
    assert!(matches!(
        client.popularity(PositiveU32::new(1).unwrap()),
        Err(ThsError::Schema(message)) if message.contains("HTML")
    ));
}

#[test]
fn capabilities_do_not_claim_unimplemented_families() {
    let capabilities = ThsClient::capabilities();
    assert!(capabilities.research.consensus);
    assert!(capabilities.signals.strong_stock_reasons);
    assert!(capabilities.signals.popularity);
    assert!(capabilities.limit_pools.upper);
    assert!(capabilities.limit_pools.reasons);
    assert!(!capabilities.research.reports);
    assert!(!capabilities.limit_pools.broken);
}

#[test]
fn bounded_https_reader_and_transport_constructor_fail_explicitly() {
    let response = read_http_response(
        200,
        DEFAULT_POPULARITY_URL.to_owned(),
        Some("application/json".to_owned()),
        &b"{}"[..],
    )
    .unwrap();
    assert_eq!(response.body, b"{}");
    assert!(matches!(
        read_http_response(200, DEFAULT_POPULARITY_URL.to_owned(), None, FailingReader),
        Err(ThsError::Transport(_))
    ));
    assert!(matches!(
        read_http_response(
            200,
            DEFAULT_POPULARITY_URL.to_owned(),
            None,
            vec![0; MAX_RESPONSE_BYTES + 1].as_slice()
        ),
        Err(ThsError::Incomplete(_))
    ));
    assert!(matches!(
        HttpsTransport::new(Duration::ZERO),
        Err(ThsError::InvalidRequest(_))
    ));
    assert!(matches!(
        HttpsTransport::new(Duration::from_secs(61)),
        Err(ThsError::InvalidRequest(_))
    ));
    HttpsTransport::new(Duration::from_millis(1)).unwrap();
}

#[test]
fn concrete_https_transport_preserves_success_status_and_transport_failures() {
    let ok = ureq::Response::new(200, "OK", "{}").unwrap();
    let response = collect_transport_result(Ok(ok)).unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"{}");

    let denied = ureq::Response::new(403, "Forbidden", "{}").unwrap();
    let response = collect_transport_result(Err(ureq::Error::Status(403, denied))).unwrap();
    assert_eq!(response.status, 403);
    assert_eq!(response.body, b"{}");

    let transport_error = ureq::get("://").call().unwrap_err();
    assert!(matches!(
        collect_transport_result(Err(transport_error)),
        Err(ThsError::Transport(_))
    ));

    let transport = HttpsTransport::new(Duration::from_secs(1)).unwrap();
    assert!(matches!(
        transport.execute(&HttpRequest {
            method: HttpMethod::Get,
            url: "://".into(),
            headers: Vec::new(),
        }),
        Err(ThsError::Transport(_))
    ));
}

#[test]
fn constructors_configuration_and_debug_are_bounded() {
    let config = ThsConfig {
        timeout: Duration::ZERO,
        ..ThsConfig::default()
    };
    assert!(matches!(
        ThsClient::with_config(config),
        Err(ThsError::InvalidRequest(_))
    ));
    let config = ThsConfig {
        timeout: Duration::from_secs(61),
        ..ThsConfig::default()
    };
    assert!(ThsClient::with_transport(config, FixtureTransport::new(vec![])).is_err());
    let config = ThsConfig {
        minimum_interval: Duration::from_millis(999),
        ..ThsConfig::default()
    };
    assert!(ThsClient::with_transport(config, FixtureTransport::new(vec![])).is_err());
    let client = ThsClient::new().unwrap();
    assert!(format!("{client:?}").contains("ThsClient"));
    ThsClient::with_config(ThsConfig::default()).unwrap();
}

#[test]
fn request_response_and_media_guards_cover_every_failure_class() {
    let request = HttpRequest {
        method: HttpMethod::Get,
        url: DEFAULT_POPULARITY_URL.to_owned(),
        headers: vec![],
    };
    for value in [
        "http://dq.10jqka.com.cn/x",
        "https://user@dq.10jqka.com.cn/x",
        "https://dq.10jqka.com.cn:444/x",
        "https://example.com/x",
        "not a url",
    ] {
        assert!(validate_url(value).is_err(), "{value}");
    }
    assert!(validate_request(&request).is_ok());

    let make = |status, final_url: &str, content_type: Option<&str>, body: &[u8]| HttpResponse {
        status,
        final_url: final_url.to_owned(),
        content_type: content_type.map(str::to_owned),
        body: body.to_vec(),
    };
    assert!(validate_response(&request, &make(200, DEFAULT_POPULARITY_URL, None, b"{}")).is_ok());
    assert!(matches!(
        validate_response(&request, &make(401, DEFAULT_POPULARITY_URL, None, b"{}")),
        Err(ThsError::Authentication(401))
    ));
    assert!(matches!(
        validate_response(&request, &make(403, DEFAULT_POPULARITY_URL, None, b"{}")),
        Err(ThsError::Authentication(403))
    ));
    assert!(matches!(
        validate_response(&request, &make(429, DEFAULT_POPULARITY_URL, None, b"{}")),
        Err(ThsError::RateLimited)
    ));
    assert!(matches!(
        validate_response(&request, &make(503, DEFAULT_POPULARITY_URL, None, b"{}")),
        Err(ThsError::HttpStatus(503))
    ));
    assert!(validate_response(
        &request,
        &make(200, "https://dq.10jqka.com.cn/other", None, b"{}")
    )
    .is_err());
    assert!(validate_response(&request, &make(200, "https://example.com/x", None, b"{}")).is_err());
    assert!(validate_response(
        &request,
        &make(
            200,
            DEFAULT_POPULARITY_URL,
            None,
            &vec![0; MAX_RESPONSE_BYTES + 1]
        )
    )
    .is_err());

    assert!(ensure_json(&make(200, DEFAULT_POPULARITY_URL, Some("text/html"), b"{}")).is_err());
    assert!(ensure_json(&make(
        200,
        DEFAULT_POPULARITY_URL,
        Some("application/json"),
        b" []"
    ))
    .is_err());
    assert!(ensure_json(&make(200, DEFAULT_POPULARITY_URL, None, b"  {}")).is_ok());
    assert!(ensure_html(&make(
        200,
        DEFAULT_POPULARITY_URL,
        Some("application/json"),
        b"<html>"
    ))
    .is_err());
    assert!(ensure_html(&make(200, DEFAULT_POPULARITY_URL, None, b"plain")).is_err());
    assert!(ensure_html(&make(
        200,
        DEFAULT_POPULARITY_URL,
        Some("text/html"),
        b"<table>"
    ))
    .is_ok());
}

#[test]
fn primitive_schema_helpers_preserve_absence_and_reject_bad_shapes() {
    let object = serde_json::json!({"a": 1});
    let array = serde_json::json!([1]);
    assert_eq!(
        required_string(Some(&serde_json::json!(" a  b ")), "x").unwrap(),
        "a b"
    );
    assert!(required_string(None, "x").is_err());
    assert_eq!(optional_string(Some(&Value::Null), "x").unwrap(), None);
    assert!(optional_string(Some(&serde_json::json!(1)), "x").is_err());
    assert!(optional_nonempty(Some(&serde_json::json!("x")), "x")
        .unwrap()
        .is_some());
    assert!(optional_object(Some(&object), "x").unwrap().is_some());
    assert!(optional_object(Some(&array), "x").is_err());
    assert!(optional_array(Some(&array), "x").unwrap().is_some());
    assert!(optional_array(Some(&object), "x").is_err());

    assert_eq!(
        required_f64(Some(&serde_json::json!("1.5")), "x").unwrap(),
        1.5
    );
    assert_eq!(
        optional_f64(Some(&serde_json::json!("--")), "x").unwrap(),
        None
    );
    assert!(optional_f64(Some(&serde_json::json!("NaN")), "x").is_err());
    assert_eq!(required_u64(Some(&serde_json::json!("7")), "x").unwrap(), 7);
    assert_eq!(optional_u64(Some(&Value::Null), "x").unwrap(), None);
    assert!(optional_u64(Some(&serde_json::json!(-1)), "x").is_err());
    assert_eq!(
        required_i64(Some(&serde_json::json!("-7")), "x").unwrap(),
        -7
    );
    assert_eq!(
        optional_i64(Some(&serde_json::json!("")), "x").unwrap(),
        None
    );
    assert!(optional_i64(Some(&array), "x").is_err());

    assert_eq!(split_subjects(" A + + B ").unwrap().len(), 2);
    assert_eq!(parse_streak("首板"), Some(1));
    assert_eq!(parse_streak("12天8板"), Some(8));
    assert_eq!(parse_streak("未知"), None);
    assert_eq!(strip_html("<b>A</b>&nbsp;&amp;&lt;&gt;"), "A &<>");
    assert_eq!(header_index(&["年度".into()], "年度").unwrap(), 0);
    assert!(header_index(&["年度".into(), "年度".into()], "年度").is_err());
    assert_eq!(parse_html_optional_number("--", "x").unwrap(), None);
    assert!(parse_html_optional_number("NaN", "x").is_err());
    assert_eq!(
        extract_as_of_date("截至2026-07-24"),
        Some("2026-07-24".into())
    );
    assert_eq!(extract_as_of_date("截至bad"), None);

    assert!(require_status(&serde_json::json!({"status": 0}), "status").is_ok());
    assert!(require_status(
        &serde_json::json!({"status": 1, "errormsg": "bad"}),
        "status"
    )
    .is_err());
    assert!(require_status(
        &serde_json::json!({"status": 2, "message": "bad"}),
        "status"
    )
    .is_err());
}

#[test]
fn consensus_parser_rejects_malformed_and_contradictory_tables() {
    assert_eq!(parse_consensus_table("<html/>").unwrap(), None);
    for html in [
        "汇总--预测年报每股收益",
        "<table>汇总--预测年报每股收益",
        "<table>汇总--预测年报每股收益</table>",
        "<table><tr><th>年度</th></tr>汇总--预测年报每股收益</table>",
        "<table><tr><th>年度</th><th>预测机构数</th><th>最小值</th><th>均值</th><th>最大值</th></tr><tr><td>x</td><td>1</td><td>1</td><td>1</td><td>1</td></tr>汇总--预测年报每股收益</table>",
        "<table><tr><th>年度</th><th>预测机构数</th><th>最小值</th><th>均值</th><th>最大值</th></tr><tr><td>2027</td><td>x</td><td>1</td><td>1</td><td>1</td></tr>汇总--预测年报每股收益</table>",
        "<table><tr><th>年度</th><th>预测机构数</th><th>最小值</th><th>均值</th><th>最大值</th></tr><tr><td>2027</td><td>1</td><td>2</td><td>1</td><td>3</td></tr>汇总--预测年报每股收益</table>",
        "<table><tr><th>年度</th><th>预测机构数</th><th>最小值</th><th>均值</th><th>最大值</th></tr><tr><td>2027</td><td>1</td><td>1</td><td>4</td><td>3</td></tr>汇总--预测年报每股收益</table>",
        "<table><tr><th>年度</th><th>预测机构数</th><th>最小值</th><th>均值</th><th>最大值</th></tr><tr><td>2027</td><td>1</td><td>--</td><td>--</td><td>--</td></tr>汇总--预测年报每股收益</table>",
    ] {
        assert!(parse_consensus_table(html).is_err(), "{html}");
    }
    assert!(extract_rows("<tr").is_err());
    assert!(extract_rows("<tr>").is_err());
    assert!(extract_cells("<td").is_err());
    assert!(extract_cells("<td>x").is_err());
    assert_eq!(
        extract_consensus_identity("<title>贵州茅台(600519)价值分析</title>", &sh("600519"))
            .unwrap()
            .as_str(),
        "贵州茅台"
    );
    assert_eq!(
        extract_consensus_identity(
            "<title>贵州茅台(600519) 盈利预测_F10_同花顺金融服务网</title>",
            &sh("600519")
        )
        .unwrap()
        .as_str(),
        "贵州茅台"
    );
    for html in [
        "<html/>",
        "<title>贵州茅台(000001)价值分析</title>",
        "<title>(600519)价值分析</title>",
        "<title>贵州茅台 600519</title>",
    ] {
        assert!(extract_consensus_identity(html, &sh("600519")).is_err());
    }
}

#[test]
fn public_preflight_limits_reject_before_transport() {
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![]));
    assert!(client.consensus(&[]).is_err());
    let many = (0..=MAX_CONSENSUS_INSTRUMENTS)
        .map(|index| sh(&format!("6{index:05}")))
        .collect::<Vec<_>>();
    assert!(client.consensus(&many).is_err());
    assert!(client.consensus(&[sh("600396"), sh("600396")]).is_err());
    let index = InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).unwrap();
    assert!(client.consensus(&[index]).is_err());

    let no_date = InstrumentSignalRequest::new(sh("600396"), PositiveU32::new(1).unwrap()).unwrap();
    assert!(client.strong_stock_reasons(&no_date).is_err());
    let too_many = InstrumentSignalRequest::new(
        sh("600396"),
        PositiveU32::new(MAX_STRONG_LIMIT + 1).unwrap(),
    )
    .unwrap()
    .with_trading_date(magic_market_core::IsoDate::new("2026-07-24").unwrap());
    assert!(client.strong_stock_reasons(&too_many).is_err());

    let wrong_pool = LimitPoolRequest::new(
        LimitPoolKind::Broken,
        magic_market_core::IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert!(client.limit_pool(&wrong_pool).is_err());
    let large_pool = LimitPoolRequest::new(
        LimitPoolKind::Upper,
        magic_market_core::IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(MAX_LIMIT_POOL + 1).unwrap(),
    )
    .unwrap();
    assert!(client.limit_pool(&large_pool).is_err());
    assert!(client
        .popularity(PositiveU32::new(MAX_POPULARITY + 1).unwrap())
        .is_err());
}

#[test]
fn identity_and_time_helpers_cover_verified_venues_and_boundaries() {
    for (code, suffix) in [
        ("600396", "SH"),
        ("000001", "SZ"),
        ("430001", "BJ"),
        ("830001", "BJ"),
        ("920001", "BJ"),
    ] {
        let instrument = equity_from_code(code).unwrap();
        assert!(instrument_identity(&instrument).ends_with(suffix));
    }
    assert!(equity_from_code("bad").is_err());
    assert!(equity_from_code("100001").is_err());
    assert_eq!(
        unix_seconds_to_china_iso(1_784_822_400).unwrap(),
        "2026-07-24T00:00:00+08:00"
    );
    assert!(unix_seconds_to_china_iso(i64::MAX).is_err());
    assert_eq!(civil_from_days(-719_468), None);
    assert_eq!(civil_from_days(i64::MAX), None);
    let provenance = provenance("tonghuashun", "1784822400", "batch", Some("2026-07-24")).unwrap();
    assert_eq!(provenance.source_at(), Some("2026-07-24"));
}

fn limit_url_for(request: &LimitPoolRequest) -> String {
    let mut url = Url::parse(DEFAULT_LIMIT_URL).unwrap();
    url.query_pairs_mut()
        .append_pair("page", "1")
        .append_pair("limit", &request.limit().get().to_string())
        .append_pair(
            "field",
            "199112,10,9001,330323,330324,330325,9002,330329,133971,133970,1968584,3475914,9003,9004",
        )
        .append_pair("filter", "HS,GEM2STAR")
        .append_pair("order_field", "330324")
        .append_pair("order_type", "0")
        .append_pair(
            "date",
            &request.trading_date().as_str().replace('-', ""),
        );
    url.to_string()
}

fn popularity_url() -> String {
    let mut url = Url::parse(DEFAULT_POPULARITY_URL).unwrap();
    url.query_pairs_mut()
        .append_pair("stock_type", "a")
        .append_pair("type", "hour")
        .append_pair("list_type", "normal");
    url.to_string()
}

#[test]
fn real_client_paths_cover_pacing_transport_and_consensus_failures() {
    ThsClient::with_transport(ThsConfig::default(), FixtureTransport::new(vec![])).unwrap();

    let url = "https://basic.10jqka.com.cn/600519/worth.html";
    let invalid_gbk = HttpResponse {
        status: 200,
        final_url: url.into(),
        content_type: Some("text/html".into()),
        body: [b"<html>".as_slice(), &[0x81]].concat(),
    };
    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![invalid_gbk]));
    assert!(matches!(
        client.consensus(&[sh("600519")]),
        Err(ThsError::Decode(_))
    ));

    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![html_response(
        url,
        "<html>source page without requested identity</html>",
    )]));
    assert!(matches!(
        client.consensus(&[sh("600519")]),
        Err(ThsError::Schema(_))
    ));

    let client = ThsClient::with_test_transport(FixtureTransport::new(vec![html_response(
        url,
        "<html>600519 source page without EPS table</html>",
    )]));
    assert!(matches!(
        client.consensus(&[sh("600519")]),
        Err(ThsError::Schema(_))
    ));

    let response_url = popularity_url();
    let transport = FixtureTransport::new(vec![
        json_response(
            &response_url,
            r#"{"status_code":0,"data":{"stock_list":[]}}"#,
        ),
        json_response(
            &response_url,
            r#"{"status_code":0,"data":{"stock_list":[]}}"#,
        ),
    ]);
    let client = ThsClient::from_parts(
        Duration::from_millis(1),
        ThsConfig::default(),
        Arc::new(transport),
    );
    assert!(client.popularity(PositiveU32::new(1).unwrap()).is_err());
    assert!(client.popularity(PositiveU32::new(1).unwrap()).is_err());
}

#[test]
fn strong_stock_schema_bounds_and_duplicates_fail_closed() {
    let date = magic_market_core::IsoDate::new("2026-07-24").unwrap();
    let request = InstrumentSignalRequest::new(sh("600396"), PositiveU32::new(2).unwrap())
        .unwrap()
        .with_trading_date(date.clone());
    let url = format!(
        "{DEFAULT_STRONG_ORIGIN}/event/api/getharden/date/{}/orderby/date/orderway/desc/charset/GBK/",
        date.as_str()
    );
    let cases = [
        serde_json::json!({"errocode":0}),
        serde_json::json!({"errocode":0,"data":vec![serde_json::json!({"code":"x"});501]}),
        serde_json::json!({"errocode":0,"data":[
            {"code":"600396","reason":"A"},
            {"code":"600396","reason":"B"}
        ]}),
    ];
    for document in cases {
        let body = serde_json::to_string(&document).unwrap();
        let client =
            ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(&url, &body)]));
        assert!(client.strong_stock_reasons(&request).is_err());
    }
}

#[test]
fn limit_pool_schema_bounds_duplicates_and_numbers_fail_closed() {
    let request = LimitPoolRequest::new(
        LimitPoolKind::Upper,
        magic_market_core::IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(200).unwrap(),
    )
    .unwrap();
    let url = limit_url_for(&request);
    let base = serde_json::json!({
        "code":"600396","latest":16.0,"change_rate":10.0
    });
    let cases = [
        serde_json::json!({"status_code":0,"data":{}}),
        serde_json::json!({"status_code":0,"data":{"info":vec![base.clone();201]}}),
        serde_json::json!({"status_code":0,"data":{"info":[base.clone(),base.clone()]}}),
        serde_json::json!({"status_code":0,"data":{"info":[{
            "code":"600396","latest":16.0,"change_rate":10.0,"order_amount":-1
        }]}}),
        serde_json::json!({"status_code":0,"data":{"info":[{
            "code":"600396","latest":16.0,"change_rate":10.0,
            "open_num":u64::MAX
        }]}}),
    ];
    for document in cases {
        let body = serde_json::to_string(&document).unwrap();
        let client =
            ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(&url, &body)]));
        assert!(client.limit_pool(&request).is_err());
    }
}

#[test]
fn popularity_schema_bounds_duplicates_and_rank_overflow_fail_closed() {
    let url = popularity_url();
    let base = serde_json::json!({"code":"600396","order":1});
    let cases = [
        serde_json::json!({"status_code":0,"data":{}}),
        serde_json::json!({"status_code":0,"data":{"stock_list":vec![base.clone();101]}}),
        serde_json::json!({"status_code":0,"data":{"stock_list":[base.clone(),base.clone()]}}),
        serde_json::json!({"status_code":0,"data":{"stock_list":[{
            "code":"600396","order":u64::from(u32::MAX)+1
        }]}}),
    ];
    for document in cases {
        let body = serde_json::to_string(&document).unwrap();
        let client =
            ThsClient::with_test_transport(FixtureTransport::new(vec![json_response(&url, &body)]));
        assert!(client.popularity(PositiveU32::new(100).unwrap()).is_err());
    }
}

#[test]
fn residual_parser_and_identity_branches_are_explicit() {
    let short = "<table><tr><th>年度</th><th>预测机构数</th><th>最小值</th><th>均值</th><th>最大值</th></tr><tr><td>2027</td></tr>汇总--预测年报每股收益</table>";
    assert!(parse_consensus_table(short).is_err());
    assert_eq!(optional_object(None, "x").unwrap(), None);
    assert_eq!(optional_array(None, "x").unwrap(), None);
    assert_eq!(optional_f64(None, "x").unwrap(), None);
    assert_eq!(optional_f64(Some(&Value::Null), "x").unwrap(), None);
    assert!(optional_f64(Some(&serde_json::json!([])), "x").is_err());
    assert_eq!(optional_u64(None, "x").unwrap(), None);
    assert_eq!(
        optional_u64(Some(&serde_json::json!("")), "x").unwrap(),
        None
    );
    assert!(optional_u64(Some(&serde_json::json!([])), "x").is_err());
    assert_eq!(optional_i64(None, "x").unwrap(), None);
    assert_eq!(optional_i64(Some(&Value::Null), "x").unwrap(), None);
    assert!(optional_i64(Some(&serde_json::json!([])), "x").is_err());

    for (exchange, code) in [(Exchange::Beijing, "430001"), (Exchange::Beijing, "830001")] {
        let instrument = InstrumentId::new(exchange, code, AssetClass::Equity).unwrap();
        assert!(validate_equity(&instrument).is_ok());
    }
    let short = InstrumentId::new(Exchange::Shanghai, "123", AssetClass::Equity).unwrap();
    assert!(validate_equity(&short).is_err());
}
