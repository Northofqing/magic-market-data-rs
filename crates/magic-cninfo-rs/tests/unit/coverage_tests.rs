use super::*;
use crate::transport::{collect_transport_result, read_http_response, HttpsTransport};
use std::collections::VecDeque;
use std::io::{self, Read};

#[derive(Clone)]
struct QueueTransport {
    responses: Arc<Mutex<VecDeque<HttpResponse>>>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl QueueTransport {
    fn new(responses: Vec<HttpResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl CninfoTransport for QueueTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError> {
        self.requests.lock().unwrap().push(request.clone());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| CninfoError::Transport("fixture exhausted".into()))
    }
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("fixture read failed"))
    }
}

fn response(url: &str, body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        final_url: url.into(),
        content_type: Some("application/json".into()),
        body: body.as_bytes().to_vec(),
    }
}

fn instrument(exchange: Exchange, code: &str) -> InstrumentId {
    InstrumentId::new(exchange, code, AssetClass::Equity).unwrap()
}

fn request(exchange: Exchange, code: &str, limit: u32) -> InstrumentDateRangeRequest {
    InstrumentDateRangeRequest::new(
        instrument(exchange, code),
        magic_market_core::PositiveU32::new(limit).unwrap(),
    )
    .unwrap()
}

fn mapping(rows: Value) -> String {
    serde_json::json!({"stockList": rows}).to_string()
}

fn question_url(code: &str, org_id: &str) -> String {
    format!(
        "{DEFAULT_IRM_QUESTION_URL}?_t=1&stockcode={code}&orgId={org_id}&pageSize=30&pageNum=1&keyWord=&startDay=&endDay="
    )
}

#[test]
fn bounded_transport_reader_and_constructor_reject_invalid_limits() {
    let response = read_http_response(
        206,
        DEFAULT_MAPPING_URL.into(),
        Some("application/json".into()),
        &b"{}"[..],
    )
    .unwrap();
    assert_eq!(response.status, 206);
    assert_eq!(response.body, b"{}");
    assert!(matches!(
        read_http_response(
            200,
            DEFAULT_MAPPING_URL.into(),
            Some("application/json".into()),
            FailingReader
        ),
        Err(CninfoError::Transport(message)) if message.contains("fixture read failed")
    ));
    assert!(matches!(
        read_http_response(
            200,
            DEFAULT_MAPPING_URL.into(),
            None,
            io::repeat(b'x').take((MAX_RESPONSE_BYTES + 1) as u64)
        ),
        Err(CninfoError::Incomplete(message)) if message.contains("exceeds")
    ));
    assert!(HttpsTransport::new(Duration::from_nanos(1)).is_ok());
    assert!(HttpsTransport::new(Duration::from_secs(60)).is_ok());
    assert!(HttpsTransport::new(Duration::ZERO).is_err());
    assert!(HttpsTransport::new(Duration::from_secs(61)).is_err());

    let transport = HttpsTransport::new(Duration::from_secs(1)).unwrap();
    let get = transport.prepare(&HttpRequest {
        method: HttpMethod::Get,
        url: DEFAULT_MAPPING_URL.into(),
        headers: vec![("X-Test".into(), "get".into())],
        body: Vec::new(),
    });
    assert_eq!(get.method(), "GET");
    assert_eq!(get.url(), DEFAULT_MAPPING_URL);
    assert_eq!(get.header("X-Test"), Some("get"));
    let post = transport.prepare(&HttpRequest {
        method: HttpMethod::Post,
        url: DEFAULT_ANNOUNCEMENT_URL.into(),
        headers: vec![("X-Test".into(), "post".into())],
        body: b"body".to_vec(),
    });
    assert_eq!(post.method(), "POST");
    assert_eq!(post.header("X-Test"), Some("post"));

    let ok = ureq::Response::new(200, "OK", "{}").unwrap();
    assert_eq!(collect_transport_result(Ok(ok)).unwrap().status, 200);
    let denied = ureq::Response::new(403, "Forbidden", "{}").unwrap();
    assert_eq!(
        collect_transport_result(Err(ureq::Error::Status(403, denied)))
            .unwrap()
            .status,
        403
    );
    let transport_error = ureq::get("://").call().unwrap_err();
    assert!(matches!(
        collect_transport_result(Err(transport_error)),
        Err(CninfoError::Transport(_))
    ));
}

#[test]
fn config_and_client_constructors_cover_every_validation_branch() {
    let base = CninfoConfig::default();
    assert!(base.validate().is_ok());
    assert!(CninfoClient::new().is_ok());
    assert!(CninfoClient::with_config(base.clone()).is_ok());
    assert!(CninfoClient::with_transport(base.clone(), QueueTransport::new(Vec::new())).is_ok());
    assert!(format!(
        "{:?}",
        CninfoClient::with_test_transport(QueueTransport::new(Vec::new()))
    )
    .contains("CninfoClient"));

    let invalid = [
        CninfoConfig {
            timeout: Duration::ZERO,
            ..base.clone()
        },
        CninfoConfig {
            timeout: Duration::from_secs(61),
            ..base.clone()
        },
        CninfoConfig {
            minimum_interval: Duration::from_millis(999),
            ..base.clone()
        },
        CninfoConfig {
            mapping_cache_ttl: Duration::ZERO,
            ..base.clone()
        },
        CninfoConfig {
            max_pages: 0,
            ..base.clone()
        },
        CninfoConfig {
            max_pages: 11,
            ..base
        },
    ];
    for config in invalid {
        assert!(matches!(
            config.validate(),
            Err(CninfoError::InvalidRequest(_))
        ));
    }
}

#[test]
fn request_url_response_and_json_guards_are_exhaustive() {
    let request = HttpRequest {
        method: HttpMethod::Get,
        url: DEFAULT_MAPPING_URL.into(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    assert!(validate_request(&request).is_ok());
    assert!(matches!(
        validate_request(&HttpRequest {
            body: vec![0; 64 * 1024 + 1],
            ..request.clone()
        }),
        Err(CninfoError::InvalidRequest(message)) if message.contains("64 KiB")
    ));
    for url in [
        "http://www.cninfo.com.cn/path",
        "https://user@www.cninfo.com.cn/path",
        "https://user:pass@www.cninfo.com.cn/path",
        "https://www.cninfo.com.cn:444/path",
        "https://example.com/path",
        "not a url",
    ] {
        assert!(validate_url(url).is_err(), "{url}");
    }
    for url in [
        DEFAULT_MAPPING_URL,
        DEFAULT_ANNOUNCEMENT_URL,
        DEFAULT_IRM_LOOKUP_URL,
        DEFAULT_IRM_QUESTION_URL,
        "https://static.cninfo.com.cn/file.pdf",
    ] {
        assert!(validate_url(url).is_ok(), "{url}");
    }

    let base = HttpResponse {
        status: 200,
        final_url: DEFAULT_MAPPING_URL.into(),
        content_type: Some("application/json".into()),
        body: b"{}".to_vec(),
    };
    assert!(validate_response(&request, &base).is_ok());
    for status in [401, 403] {
        assert!(matches!(
            validate_response(
                &request,
                &HttpResponse {
                    status,
                    ..base.clone()
                }
            ),
            Err(CninfoError::Authentication(value)) if value == status
        ));
    }
    assert!(matches!(
        validate_response(
            &request,
            &HttpResponse {
                status: 429,
                ..base.clone()
            }
        ),
        Err(CninfoError::RateLimited)
    ));
    assert!(matches!(
        validate_response(
            &request,
            &HttpResponse {
                status: 500,
                ..base.clone()
            }
        ),
        Err(CninfoError::HttpStatus(500))
    ));
    assert!(matches!(
        validate_response(
            &request,
            &HttpResponse {
                final_url: DEFAULT_ANNOUNCEMENT_URL.into(),
                ..base.clone()
            }
        ),
        Err(CninfoError::Schema(message)) if message.contains("does not match")
    ));
    assert!(validate_response(
        &request,
        &HttpResponse {
            final_url: "https://example.com/redirect".into(),
            ..base.clone()
        }
    )
    .is_err());
    assert!(matches!(
        validate_response(
            &request,
            &HttpResponse {
                body: vec![0; MAX_RESPONSE_BYTES + 1],
                ..base.clone()
            }
        ),
        Err(CninfoError::Incomplete(_))
    ));

    assert!(ensure_json(&base).is_ok());
    assert!(ensure_json(&HttpResponse {
        content_type: None,
        body: b" [1]".to_vec(),
        ..base.clone()
    })
    .is_ok());
    assert!(matches!(
        ensure_json(&HttpResponse {
            content_type: Some("text/html".into()),
            ..base.clone()
        }),
        Err(CninfoError::Schema(message)) if message.contains("expected JSON")
    ));
    assert!(matches!(
        ensure_json(&HttpResponse {
            body: b" html".to_vec(),
            ..base
        }),
        Err(CninfoError::Schema(message)) if message.contains("not a JSON")
    ));
}

#[test]
fn primitive_helpers_preserve_missingness_and_reject_unsafe_values() {
    assert_eq!(json_headers("https://www.cninfo.com.cn/").len(), 3);
    assert_eq!(
        form_headers("https://www.cninfo.com.cn", "https://www.cninfo.com.cn/").len(),
        5
    );
    assert_eq!(
        String::from_utf8(encode_form(&[("a", "x y&z".into())])).unwrap(),
        "a=x+y%26z"
    );
    assert_eq!(required_text(Some(" x ".into()), "f").unwrap(), "x");
    assert!(required_text(Some(" ".into()), "f").is_err());
    assert_eq!(
        normalize_required(Some(" a \n b ".into()), "f").unwrap(),
        "a b"
    );
    assert!(normalize_required(None, "f").is_err());
    assert_eq!(
        optional_nonempty(Some(" a \t b ".into()))
            .unwrap()
            .as_ref()
            .map(NonEmptyText::as_str),
        Some("a b")
    );
    assert!(optional_nonempty(Some(" ".into())).unwrap().is_none());

    assert!(announcement_url("600396", "org", "id", "short").is_err());
    assert!(announcement_url("600396", "org", "id", "2026-07-24T00:00:00+08:00").is_ok());
    assert_eq!(
        pdf_url("/finalpage/2026-07-24/a.PDF".into())
            .unwrap()
            .as_str(),
        "https://static.cninfo.com.cn/finalpage/2026-07-24/a.PDF"
    );
    for value in ["", "..", "a\\b", "https:a"] {
        assert!(pdf_url(value.into()).is_err(), "{value}");
    }

    assert_eq!(parse_optional_millis(None, "t").unwrap(), None);
    assert_eq!(
        parse_optional_millis(Some(&Value::Null), "t").unwrap(),
        None
    );
    assert_eq!(
        parse_optional_millis(Some(&serde_json::json!(1_784_822_400_000_i64)), "t")
            .unwrap()
            .as_deref(),
        Some("2026-07-24T00:00:00+08:00")
    );
    assert_eq!(
        parse_optional_millis(Some(&Value::String("1784822400000".into())), "t")
            .unwrap()
            .as_deref(),
        Some("2026-07-24T00:00:00+08:00")
    );
    for value in [
        serde_json::json!(true),
        serde_json::json!("bad"),
        serde_json::json!(i64::MAX),
    ] {
        assert!(parse_optional_millis(Some(&value), "t").is_err());
    }
    assert!(parse_required_millis(None, "t").is_err());
    assert_eq!(parse_optional_u64(None, "n").unwrap(), None);
    assert_eq!(parse_optional_u64(Some(&Value::Null), "n").unwrap(), None);
    assert_eq!(
        parse_optional_u64(Some(&serde_json::json!(7)), "n").unwrap(),
        Some(7)
    );
    assert_eq!(
        parse_optional_u64(Some(&Value::String("8".into())), "n").unwrap(),
        Some(8)
    );
    for value in [serde_json::json!(-1), serde_json::json!(true)] {
        assert!(parse_optional_u64(Some(&value), "n").is_err());
    }

    let ranged = request(Exchange::Shanghai, "600396", 1)
        .with_range(
            magic_market_core::IsoDate::new("2026-07-23").unwrap(),
            magic_market_core::IsoDate::new("2026-07-24").unwrap(),
        )
        .unwrap();
    assert!(ensure_in_range("bad", &ranged).is_err());
    assert!(ensure_in_range("2026-07-22T00:00:00+08:00", &ranged).is_err());
    assert!(ensure_in_range("2026-07-25T00:00:00+08:00", &ranged).is_err());
    assert!(ensure_in_range("2026-07-24T00:00:00+08:00", &ranged).is_ok());
    assert!(provenance("cninfo", "observed", "batch", None).is_ok());
    assert!(provenance(
        "cninfo",
        "observed",
        "batch",
        Some("2026-07-24T00:00:00+08:00")
    )
    .is_ok());
    assert!(now().is_ok());
    assert!(unix_seconds_to_china_rfc3339(i64::MAX).is_err());
}

#[test]
fn organization_cache_rejects_schema_drift_and_reuses_exact_mapping() {
    let valid = mapping(serde_json::json!([
        {"code":"600396","orgId":"org-a"},
        {"code":"600396","orgId":"org-a"}
    ]));
    let transport = QueueTransport::new(vec![response(DEFAULT_MAPPING_URL, &valid)]);
    let observed = transport.clone();
    let client = CninfoClient::with_test_transport(transport);
    for _ in 0..2 {
        assert_eq!(
            client
                .organization_mapping(&instrument(Exchange::Shanghai, "600396"))
                .unwrap()
                .org_id
                .as_str(),
            "org-a"
        );
    }
    assert_eq!(observed.requests.lock().unwrap().len(), 1);

    let cases = [
        "{}".to_owned(),
        mapping(serde_json::json!([])),
        mapping(serde_json::json!([{"code":"bad","orgId":"org"}])),
        mapping(serde_json::json!([{"code":"600396"}])),
        mapping(serde_json::json!([
            {"code":"600396","orgId":"org-a"},
            {"code":"600396","orgId":"org-b"}
        ])),
        "{invalid".to_owned(),
    ];
    for body in cases {
        let client = CninfoClient::with_test_transport(QueueTransport::new(vec![response(
            DEFAULT_MAPPING_URL,
            &body,
        )]));
        assert!(client
            .organization_mapping(&instrument(Exchange::Shanghai, "600396"))
            .is_err());
    }
    let absent = mapping(serde_json::json!([{"code":"600703","orgId":"org"}]));
    let client = CninfoClient::with_test_transport(QueueTransport::new(vec![response(
        DEFAULT_MAPPING_URL,
        &absent,
    )]));
    assert!(matches!(
        client.organization_mapping(&instrument(Exchange::Shanghai, "600396")),
        Err(CninfoError::Unsupported(message)) if message.contains("no exact entry")
    ));
}

#[test]
fn announcement_failures_and_pacing_are_explicit() {
    let mapping = mapping(serde_json::json!([{"code":"600396","orgId":"org"}]));
    let pages = [
        r#"{"announcements":[]}"#,
        r#"{"hasMore":true,"announcements":[]}"#,
        r#"{"hasMore":false,"announcements":[{"announcementId":"dup","secCode":"600396","announcementTitle":"a","announcementTime":1784822400000},{"announcementId":"dup","secCode":"600396","announcementTitle":"b","announcementTime":1784822400000}]}"#,
        r#"{"hasMore":false,"announcements":[{"announcementId":"id","secCode":"600703","announcementTitle":"a","announcementTime":1784822400000}]}"#,
        r#"{"hasMore":false,"announcements":[{"announcementId":"id","secCode":"600396","announcementTitle":" ","announcementTime":1784822400000}]}"#,
        r#"{"hasMore":false,"announcements":[{"announcementId":"id","secCode":"600396","announcementTitle":"a","announcementTime":true}]}"#,
        r#"{"hasMore":false,"announcements":[{"announcementId":"id","secCode":"600396","announcementTitle":"a","announcementTime":1784822400000,"adjunctUrl":"../escape.pdf"}]}"#,
        "{invalid",
    ];
    for page in pages {
        let client = CninfoClient::from_parts(
            Duration::from_millis(1),
            CninfoConfig::default(),
            Arc::new(QueueTransport::new(vec![
                response(DEFAULT_MAPPING_URL, &mapping),
                response(DEFAULT_ANNOUNCEMENT_URL, page),
            ])),
        );
        assert!(client
            .announcements(&request(Exchange::Shanghai, "600396", 2))
            .is_err());
    }
    let client = CninfoClient::with_test_transport(QueueTransport::new(vec![HttpResponse {
        status: 500,
        final_url: DEFAULT_MAPPING_URL.into(),
        content_type: Some("application/json".into()),
        body: b"{}".to_vec(),
    }]));
    assert!(matches!(
        client.organization_mapping(&instrument(Exchange::Shanghai, "600396")),
        Err(CninfoError::HttpStatus(500))
    ));
}

#[test]
fn irm_lookup_page_and_record_failures_are_explicit() {
    let code = "002594";
    for lookup in [
        r#"{}"#,
        r#"{"data":[]}"#,
        r#"{"data":[{"stockCode":"002594","secid":"a"},{"stockCode":"002594","secid":"b"}]}"#,
        r#"{"data":[{"stockCode":"002594"}]}"#,
        "{invalid",
    ] {
        let client = CninfoClient::with_test_transport(QueueTransport::new(vec![response(
            DEFAULT_IRM_LOOKUP_URL,
            lookup,
        )]));
        assert!(client
            .investor_questions(&request(Exchange::Shenzhen, code, 1))
            .is_err());
    }

    let lookup = r#"{"data":[{"stockCode":"002594","secid":"org"}]}"#;
    let url = question_url(code, "org");
    for page in [
        r#"{}"#,
        r#"{"total":1,"rows":[]}"#,
        r#"{"total":2,"rows":[{"indexId":"dup","stockCode":"002594","companyShortName":"c","mainContent":"q","pubDate":1784822400000},{"indexId":"dup","stockCode":"002594","companyShortName":"c","mainContent":"q","pubDate":1784822400000}]}"#,
        r#"{"total":1,"rows":[{"indexId":"id","stockCode":"600396","companyShortName":"c","mainContent":"q","pubDate":1784822400000}]}"#,
        r#"{"total":1,"rows":[{"indexId":"id","stockCode":"002594","companyShortName":" ","mainContent":"q","pubDate":1784822400000}]}"#,
        r#"{"total":1,"rows":[{"indexId":"id","stockCode":"002594","companyShortName":"c","mainContent":"q","pubDate":1784822400000,"attachedContent":"a","attachedPubDate":true}]}"#,
        "{invalid",
    ] {
        let client = CninfoClient::with_test_transport(QueueTransport::new(vec![
            response(DEFAULT_IRM_LOOKUP_URL, lookup),
            response(&url, page),
        ]));
        assert!(client
            .investor_questions(&request(Exchange::Shenzhen, code, 2))
            .is_err());
    }
}

#[test]
fn bounded_request_and_exchange_validation_cover_supported_prefixes() {
    let oversized = InstrumentDateRangeRequest::new(
        instrument(Exchange::Shanghai, "600396"),
        magic_market_core::PositiveU32::new(MAX_RECORDS + 1).unwrap(),
    )
    .unwrap();
    assert!(validate_bounded_request(&oversized).is_err());
    let non_equity = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Index).unwrap();
    assert!(matches!(
        validate_instrument(&non_equity),
        Err(CninfoError::Unsupported(_))
    ));
    let malformed = InstrumentId::new(Exchange::Shanghai, "60039A", AssetClass::Equity).unwrap();
    assert!(validate_instrument(&malformed).is_err());
    for code in ["430001", "830001", "920001"] {
        assert!(validate_instrument(&instrument(Exchange::Beijing, code)).is_ok());
    }
}
