use super::*;
use std::collections::VecDeque;

#[derive(Clone)]
struct FixtureTransport {
    responses: Arc<Mutex<VecDeque<HttpResponse>>>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
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
    let batch = ThsClient::with_test_transport(transport)
        .consensus(&[sh("600519")])
        .unwrap();
    let completed_at = observed.completed_at.lock().unwrap().unwrap();
    assert!(timestamp_nanos(batch.provenance().fetched_at()) >= completed_at);
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
