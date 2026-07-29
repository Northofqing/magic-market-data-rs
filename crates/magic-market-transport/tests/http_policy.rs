use magic_market_transport::{
    EndpointPolicy, HttpMethod, HttpRequest, HttpResponse, MediaType, ReqwestTransport,
    TransportError,
};
use std::time::Duration;

fn policy() -> EndpointPolicy {
    EndpointPolicy::new(
        "api.example.test",
        vec!["/v1/data".into()],
        vec!["series_id".into(), "start".into(), "end".into()],
        vec![MediaType::Json],
        1024,
        Duration::from_secs(10),
    )
    .unwrap()
}

#[test]
fn policy_rejects_redirect_hosts_query_keys_and_oversize_bodies() {
    assert!(HttpRequest::new(
        HttpMethod::Get,
        "https://api.example.test/v1/data?secret=x",
        vec![],
        vec![],
    )
    .and_then(|request| policy().validate_request(&request))
    .is_err());
    let response = HttpResponse::new(
        200,
        "https://other.example.test/v1/data",
        Some("application/json".into()),
        vec![b'x'; 10],
    );
    assert!(policy().validate_response(response).is_err());
    let response = HttpResponse::new(
        200,
        "https://api.example.test/v1/data",
        Some("application/json".into()),
        vec![b'x'; 1025],
    );
    assert!(matches!(
        policy().validate_response(response),
        Err(TransportError::ResourceLimit(_))
    ));
}

#[test]
fn response_status_media_type_encoding_and_limits_are_closed() {
    let exact = HttpResponse::new(
        200,
        "https://api.example.test/v1/data",
        Some("application/json; charset=utf-8".into()),
        vec![b'x'; 1024],
    );
    assert_eq!(
        policy().validate_response(exact).unwrap().body().len(),
        1024
    );

    let oversized = HttpResponse::new(
        200,
        "https://api.example.test/v1/data",
        Some("application/json".into()),
        vec![b'x'; 1025],
    );
    assert!(matches!(
        policy().validate_response(oversized),
        Err(TransportError::ResourceLimit(_))
    ));
    let missing_mime = HttpResponse::new(200, "https://api.example.test/v1/data", None, vec![]);
    assert!(matches!(
        policy().validate_response(missing_mime),
        Err(TransportError::MediaType(_))
    ));
    let redirect = HttpResponse::new(
        302,
        "https://api.example.test/v1/data",
        Some("application/json".into()),
        vec![],
    );
    assert!(matches!(
        policy().validate_response(redirect),
        Err(TransportError::Redirect(_))
    ));
    let throttled = HttpResponse::new(
        429,
        "https://api.example.test/v1/data",
        Some("application/json".into()),
        vec![],
    );
    assert!(matches!(
        policy().validate_response(throttled),
        Err(TransportError::HttpStatus { status: 429 })
    ));
    let compressed = HttpResponse::new(
        200,
        "https://api.example.test/v1/data",
        Some("application/json".into()),
        vec![],
    )
    .with_content_encoding(Some("gzip".into()));
    assert!(matches!(
        policy().validate_response(compressed),
        Err(TransportError::MediaType(_))
    ));
}

#[test]
fn request_validation_rejects_credential_and_endpoint_ambiguity_before_io() {
    for url in [
        "https://user:password@api.example.test/v1/data",
        "https://api.example.test/v1/data#fragment",
        "https://api.example.test/v1/data?unknown=value",
        "https://api.example.test:444/v1/data",
        "https://api.example.test/v1/database",
    ] {
        let request = HttpRequest::new(HttpMethod::Get, url, vec![], vec![]).unwrap();
        assert!(policy().validate_request(&request).is_err(), "{url}");
    }
    for forbidden in ["Cookie", "Authorization", "Proxy-Authorization"] {
        assert!(HttpRequest::new(
            HttpMethod::Get,
            "https://api.example.test/v1/data",
            vec![(forbidden.into(), "sensitive".into())],
            vec![],
        )
        .is_err());
    }
}

#[test]
fn request_rejects_authority_framing_and_hop_by_hop_headers() {
    for forbidden in [
        "Host",
        "Content-Length",
        "Transfer-Encoding",
        "Connection",
        "Proxy-Connection",
        "Keep-Alive",
        "TE",
        "Trailer",
        "Upgrade",
    ] {
        assert!(
            HttpRequest::new(
                HttpMethod::Post,
                "https://api.example.test/v1/data",
                vec![(forbidden.into(), "attacker-controlled".into())],
                vec![],
            )
            .is_err(),
            "{forbidden}"
        );
    }
}

#[test]
fn response_is_bound_to_the_exact_validated_request_url() {
    let request = HttpRequest::new(
        HttpMethod::Get,
        "https://api.example.test/v1/data?series_id=GDP",
        vec![],
        vec![],
    )
    .unwrap();
    let response = HttpResponse::new(
        200,
        "https://api.example.test/v1/data?series_id=CPI",
        Some("application/json".into()),
        vec![],
    );
    assert!(matches!(
        policy().validate_response_for(&request, response),
        Err(TransportError::Redirect(_))
    ));
}

#[test]
fn exact_url_binding_uses_normalized_urls_and_keeps_query_values_bound() {
    let request = HttpRequest::new(
        HttpMethod::Get,
        "https://api.example.test:443/v1/data?series_id=GDP&start=2025-01",
        vec![],
        vec![],
    )
    .unwrap();
    let equivalent = HttpResponse::new(
        200,
        "https://api.example.test/v1/data?series_id=GDP&start=2025-01",
        Some("application/json".into()),
        vec![],
    );
    assert!(policy().validate_response_for(&request, equivalent).is_ok());

    let different_value = HttpResponse::new(
        200,
        "https://api.example.test/v1/data?series_id=CPI&start=2025-01",
        Some("application/json".into()),
        vec![],
    );
    assert!(matches!(
        policy().validate_response_for(&request, different_value),
        Err(TransportError::Redirect(_))
    ));
}

#[test]
fn request_debug_redacts_values_and_body() {
    let request = HttpRequest::new(
        HttpMethod::Post,
        "https://api.example.test/v1/data?series_id=secret-series",
        vec![("X-Request-Token".into(), "secret-header".into())],
        b"secret-body".to_vec(),
    )
    .unwrap();
    let debug = format!("{request:?}");
    assert!(debug.contains("Post"));
    assert!(debug.contains("api.example.test"));
    assert!(debug.contains("/v1/data"));
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("secret-series"));
    assert!(!debug.contains("secret-header"));
    assert!(!debug.contains("secret-body"));
}

#[test]
fn production_client_initialization_and_debug_are_bounded_and_redacted() {
    let transport = ReqwestTransport::new(policy()).unwrap();
    let debug = format!("{transport:?}");
    assert!(debug.contains("ReqwestTransport"));
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("reqwest::"));
}

#[test]
fn endpoint_policy_constructor_rejects_unsafe_configuration() {
    assert!(EndpointPolicy::new(
        "API.example.test",
        vec!["/v1".into()],
        vec![],
        vec![MediaType::Json],
        1,
        Duration::from_secs(1),
    )
    .is_err());
    assert!(EndpointPolicy::new(
        "api.example.test",
        vec!["relative".into()],
        vec![],
        vec![MediaType::Json],
        1,
        Duration::from_secs(1),
    )
    .is_err());
    assert!(EndpointPolicy::new(
        "api.example.test",
        vec!["/v1".into()],
        vec!["x".into(), "x".into()],
        vec![MediaType::Json],
        1,
        Duration::from_secs(1),
    )
    .is_err());
    assert!(EndpointPolicy::new(
        "api.example.test",
        vec!["/v1".into()],
        vec![],
        vec![MediaType::Json],
        0,
        Duration::from_secs(1),
    )
    .is_err());
    assert!(EndpointPolicy::new(
        "api.example.test",
        vec!["/v1".into()],
        vec![],
        vec![MediaType::Json],
        1,
        Duration::from_secs(61),
    )
    .is_err());
}
