use super::*;

fn policy(media_types: Vec<MediaType>) -> EndpointPolicy {
    EndpointPolicy::new(
        "api.example.test",
        vec!["/v1/data".into(), "/root/".into()],
        vec!["series".into(), "start".into()],
        media_types,
        16,
        Duration::from_secs(1),
    )
    .unwrap()
}

#[test]
fn helpers_enforce_segment_boundaries_and_redaction() {
    assert!(path_matches_prefix("/v1/data/item", "/v1/data"));
    assert!(path_matches_prefix("/v1/data", "/v1/data"));
    assert!(path_matches_prefix("/root/item", "/root/"));
    assert!(!path_matches_prefix("/v1/database", "/v1/data"));
    let safe = redacted_url("https://user:pass@example.test/v1?q=secret#fragment");
    assert_eq!(safe, "https://example.test/v1?q=[REDACTED]#[REDACTED]");
    assert!(!safe.contains("user"));
    assert!(!safe.contains("pass"));
    assert!(!safe.contains("secret"));
    assert_eq!(redacted_url("not a URL"), "[INVALID URL; REDACTED]");
    assert_eq!(
        redacted_url("mailto:test@example.test"),
        "[INVALID URL; REDACTED]"
    );
}

#[test]
fn status_errors_keep_redirects_and_throttling_typed() {
    assert!(matches!(rejected_status(302), TransportError::Redirect(_)));
    assert!(matches!(
        rejected_status(429),
        TransportError::HttpStatus { status: 429 }
    ));
}

#[test]
fn media_type_matching_is_closed_and_case_insensitive() {
    assert!(MediaType::Json.matches("APPLICATION/JSON"));
    assert!(MediaType::Html.matches("text/html"));
    assert!(MediaType::Html.matches("application/xhtml+xml"));
    assert!(MediaType::Javascript.matches("APPLICATION/JAVASCRIPT"));
    assert!(MediaType::Javascript.matches("text/javascript"));
    assert!(MediaType::Javascript.matches("application/x-javascript"));
    assert!(MediaType::Xml.matches("application/xml"));
    assert!(MediaType::Xml.matches("TEXT/XML"));
    assert!(MediaType::PlainText.matches("text/plain"));
    assert!(MediaType::Xlsx
        .matches("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"));
    assert!(!MediaType::Json.matches("application/problem+json"));
}

#[test]
fn request_constructor_validates_every_header_boundary() {
    let valid = HttpRequest::new(
        HttpMethod::Post,
        "https://api.example.test/v1/data",
        vec![("X-Token_1".into(), "value".into())],
        b"body".to_vec(),
    )
    .unwrap();
    assert_eq!(valid.method(), HttpMethod::Post);
    assert_eq!(valid.url(), "https://api.example.test/v1/data");
    assert_eq!(valid.headers()[0].0, "X-Token_1");
    assert_eq!(valid.body(), b"body");

    for invalid in ["", "bad name", "x:y", "é"] {
        assert!(matches!(
            HttpRequest::new(
                HttpMethod::Get,
                "https://api.example.test/v1/data",
                vec![(invalid.into(), "value".into())],
                vec![],
            ),
            Err(TransportError::InvalidRequest(_))
        ));
    }
    assert!(matches!(
        HttpRequest::new(
            HttpMethod::Get,
            "https://api.example.test/v1/data",
            vec![
                ("X-Test".into(), "one".into()),
                ("x-test".into(), "two".into())
            ],
            vec![],
        ),
        Err(TransportError::InvalidRequest(_))
    ));
    for credential in ["cookie", "AUTHORIZATION", "Proxy-Authorization"] {
        assert!(matches!(
            HttpRequest::new(
                HttpMethod::Get,
                "https://api.example.test/v1/data",
                vec![(credential.into(), "secret".into())],
                vec![],
            ),
            Err(TransportError::Authentication(_))
        ));
    }
    for forbidden in [
        "host",
        "content-length",
        "transfer-encoding",
        "connection",
        "proxy-connection",
        "keep-alive",
        "te",
        "trailer",
        "upgrade",
    ] {
        assert!(matches!(
            HttpRequest::new(
                HttpMethod::Get,
                "https://api.example.test/v1/data",
                vec![(forbidden.into(), "value".into())],
                vec![],
            ),
            Err(TransportError::InvalidRequest(_))
        ));
    }
    assert!(matches!(
        HttpRequest::new(
            HttpMethod::Get,
            "https://api.example.test/v1/data",
            vec![("X-Test".into(), "line\nbreak".into())],
            vec![],
        ),
        Err(TransportError::InvalidRequest(_))
    ));
}

#[test]
fn request_and_response_debug_and_accessors_redact_payloads() {
    let request = HttpRequest::new(
        HttpMethod::Post,
        "https://api.example.test/v1/data?series=secret",
        vec![("X-Test".into(), "secret-header".into())],
        b"secret-body".to_vec(),
    )
    .unwrap();
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("Post"));
    assert!(request_debug.contains("[REDACTED]"));
    assert!(!request_debug.contains("secret-header"));
    assert!(!request_debug.contains("secret-body"));

    let response = HttpResponse::new(
        200,
        "https://api.example.test/v1/data?series=secret",
        Some("application/json".into()),
        b"secret-response".to_vec(),
    )
    .with_content_encoding(Some("identity".into()));
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.final_url(),
        "https://api.example.test/v1/data?series=secret"
    );
    assert_eq!(response.content_type(), Some("application/json"));
    assert_eq!(response.content_encoding(), Some("identity"));
    assert_eq!(response.body(), b"secret-response");
    let response_debug = format!("{response:?}");
    assert!(response_debug.contains("[REDACTED]"));
    assert!(!response_debug.contains("secret-response"));
    assert!(!response_debug.contains("series=secret"));
}

#[test]
fn endpoint_policy_constructor_rejects_each_invalid_dimension() {
    for hostname in [
        "",
        "API.example.test",
        ".api.example.test",
        "api.example.test.",
        "api..example.test",
        "-api.example.test",
        "api-.example.test",
        "api_1.example.test",
        "例子.test",
    ] {
        assert!(EndpointPolicy::new(
            hostname,
            vec!["/".into()],
            vec![],
            vec![MediaType::Json],
            1,
            Duration::from_secs(1),
        )
        .is_err());
    }
    let long_label = format!("{}.test", "a".repeat(64));
    assert!(EndpointPolicy::new(
        long_label,
        vec!["/".into()],
        vec![],
        vec![MediaType::Json],
        1,
        Duration::from_secs(1),
    )
    .is_err());
    let long_host = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(63)
    );
    assert!(EndpointPolicy::new(
        long_host,
        vec!["/".into()],
        vec![],
        vec![MediaType::Json],
        1,
        Duration::from_secs(1),
    )
    .is_err());

    for paths in [
        vec![],
        vec!["relative".into()],
        vec!["/bad?query".into()],
        vec!["/bad#fragment".into()],
        vec!["/bad\ncontrol".into()],
        vec!["/same".into(), "/same".into()],
    ] {
        assert!(EndpointPolicy::new(
            "api.example.test",
            paths,
            vec![],
            vec![MediaType::Json],
            1,
            Duration::from_secs(1),
        )
        .is_err());
    }
    for queries in [
        vec!["".into()],
        vec!["non-ascii-é".into()],
        vec!["bad&key".into()],
        vec!["bad=key".into()],
        vec!["bad#key".into()],
        vec!["same".into(), "same".into()],
    ] {
        assert!(EndpointPolicy::new(
            "api.example.test",
            vec!["/".into()],
            queries,
            vec![MediaType::Json],
            1,
            Duration::from_secs(1),
        )
        .is_err());
    }
    for media in [vec![], vec![MediaType::Json, MediaType::Json]] {
        assert!(EndpointPolicy::new(
            "api.example.test",
            vec!["/".into()],
            vec![],
            media,
            1,
            Duration::from_secs(1),
        )
        .is_err());
    }
    for max_body in [0, MAX_CONFIGURED_BODY_BYTES + 1] {
        assert!(EndpointPolicy::new(
            "api.example.test",
            vec!["/".into()],
            vec![],
            vec![MediaType::Json],
            max_body,
            Duration::from_secs(1),
        )
        .is_err());
    }
    for timeout in [Duration::ZERO, Duration::from_secs(61)] {
        assert!(EndpointPolicy::new(
            "api.example.test",
            vec!["/".into()],
            vec![],
            vec![MediaType::Json],
            1,
            timeout,
        )
        .is_err());
    }
}

#[test]
fn request_url_and_query_validation_is_closed() {
    let policy = policy(vec![MediaType::Json]);
    let valid = HttpRequest::new(
        HttpMethod::Get,
        "https://api.example.test:443/v1/data?series=GDP&start=2025",
        vec![("Accept-Encoding".into(), " Identity ".into())],
        vec![],
    )
    .unwrap();
    policy.validate_request(&valid).unwrap();

    let get_body = HttpRequest::new(
        HttpMethod::Get,
        "https://api.example.test/v1/data",
        vec![],
        vec![1],
    )
    .unwrap();
    assert!(policy.validate_request(&get_body).is_err());
    let compressed = HttpRequest::new(
        HttpMethod::Get,
        "https://api.example.test/v1/data",
        vec![("Accept-Encoding".into(), "gzip".into())],
        vec![],
    )
    .unwrap();
    assert!(policy.validate_request(&compressed).is_err());
    for url in [
        "not a URL",
        "http://api.example.test/v1/data",
        "https://user@api.example.test/v1/data",
        "https://user:pass@api.example.test/v1/data",
        "https://api.example.test:444/v1/data",
        "https://other.example.test/v1/data",
        "https://api.example.test/v1/data#fragment",
        "https://api.example.test/not-allowed",
        "https://api.example.test/v1/data?unknown=x",
        "https://api.example.test/v1/data?series=x&series=y",
    ] {
        let request = HttpRequest::new(HttpMethod::Get, url, vec![], vec![]).unwrap();
        assert!(policy.validate_request(&request).is_err(), "{url}");
    }
}

#[test]
fn response_validation_covers_status_media_and_exact_url_binding() {
    let policy = policy(vec![
        MediaType::Json,
        MediaType::Html,
        MediaType::Javascript,
        MediaType::Xml,
        MediaType::PlainText,
        MediaType::Xlsx,
    ]);
    for content_type in [
        "application/json; charset=utf-8",
        "text/html",
        "application/xml",
        "text/xml",
        "text/plain",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ] {
        let response = HttpResponse::new(
            200,
            "https://api.example.test/v1/data",
            Some(content_type.into()),
            vec![1],
        );
        policy.validate_response(response).unwrap();
    }
    for status in [301, 399, 400, 429, 500] {
        let response = HttpResponse::new(
            status,
            "https://api.example.test/v1/data",
            Some("application/json".into()),
            vec![],
        );
        assert!(policy.validate_response(response).is_err());
    }
    for response in [
        HttpResponse::new(
            200,
            "https://api.example.test/v1/data",
            Some("application/json".into()),
            vec![0; 17],
        ),
        HttpResponse::new(
            200,
            "https://api.example.test/v1/data",
            Some("application/json".into()),
            vec![],
        )
        .with_content_encoding(Some("gzip".into())),
        HttpResponse::new(200, "https://api.example.test/v1/data", None, vec![]),
        HttpResponse::new(
            200,
            "https://api.example.test/v1/data",
            Some(" ".into()),
            vec![],
        ),
        HttpResponse::new(
            200,
            "https://api.example.test/v1/data",
            Some("application/pdf".into()),
            vec![],
        ),
        HttpResponse::new(
            200,
            "https://other.example.test/v1/data",
            Some("application/json".into()),
            vec![],
        ),
    ] {
        assert!(policy.validate_response(response).is_err());
    }

    let request = HttpRequest::new(
        HttpMethod::Get,
        "https://api.example.test/v1/data?series=GDP",
        vec![],
        vec![],
    )
    .unwrap();
    let exact = HttpResponse::new(
        200,
        "https://api.example.test:443/v1/data?series=GDP",
        Some("application/json".into()),
        vec![],
    );
    policy.validate_response_for(&request, exact).unwrap();
    let changed = HttpResponse::new(
        200,
        "https://api.example.test/v1/data?series=CPI",
        Some("application/json".into()),
        vec![],
    );
    assert!(matches!(
        policy.validate_response_for(&request, changed),
        Err(TransportError::Redirect(_))
    ));
}

#[test]
fn helper_predicates_cover_valid_and_invalid_boundaries() {
    assert!(valid_header_name("X!#$%&'*+-.^_`|~09"));
    assert!(!valid_header_name(""));
    assert!(!valid_header_name("bad name"));
    assert!(is_credential_header("cookie"));
    assert!(!is_credential_header("x-cookie"));
    assert!(is_authority_or_framing_header("upgrade"));
    assert!(!is_authority_or_framing_header("x-upgrade"));
    assert!(valid_ascii_hostname("api-1.example.test"));
    assert!(!valid_ascii_hostname("Api.example.test"));
    assert!(valid_path_prefix("/"));
    assert!(!valid_path_prefix("relative"));
    assert!(valid_query_key("series_id"));
    assert!(!valid_query_key("series=id"));
}

#[test]
fn production_transport_builds_get_and_post_before_network_failure() {
    let policy = EndpointPolicy::new(
        "127.0.0.1",
        vec!["/".into()],
        vec![],
        vec![MediaType::Json],
        16,
        Duration::from_secs(1),
    )
    .unwrap();
    let transport = ReqwestTransport::new(policy).unwrap();
    assert_eq!(transport.policy().max_body_bytes, 16);
    assert!(format!("{transport:?}").contains("[REDACTED]"));
    for request in [
        HttpRequest::new(
            HttpMethod::Get,
            "https://127.0.0.1/",
            vec![("X-Test".into(), "value".into())],
            vec![],
        )
        .unwrap(),
        HttpRequest::new(
            HttpMethod::Post,
            "https://127.0.0.1/",
            vec![("Content-Type".into(), "application/json".into())],
            b"{}".to_vec(),
        )
        .unwrap(),
    ] {
        assert!(transport.execute(&request).is_err());
    }
    ensure_rustls_provider().unwrap();
}
