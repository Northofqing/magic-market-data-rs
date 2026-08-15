use super::{
    map_ureq_error, read_http_response, validate_content_type, validate_endpoint,
    validate_html_content_type, validate_response_limit, EastmoneyTransport, HttpsTransport,
    DEFAULT_MAX_RESPONSE_BYTES,
};
use std::io::{self, Read};
use std::sync::mpsc;
use std::time::Duration;

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("fixture read failed"))
    }
}

struct DefaultDocumentTransport;

impl EastmoneyTransport for DefaultDocumentTransport {
    fn get(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, super::EastmoneyError> {
        Ok(b"forwarded".to_vec())
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, super::EastmoneyError> {
        unreachable!("document forwarding uses GET")
    }
}

#[test]
fn endpoint_allowlist_rejects_non_https_redirect_targets_and_lookalikes() {
    assert!(validate_endpoint("https://push2.eastmoney.com/api").is_ok());
    assert!(validate_endpoint("https://push2delay.eastmoney.com/api").is_ok());
    assert!(validate_endpoint("https://push2.eastmoney.com:443/api").is_ok());
    assert!(validate_endpoint("https://search-api-web.eastmoney.com/search/jsonp").is_err());
    assert!(validate_endpoint("http://push2.eastmoney.com/api").is_err());
    assert!(validate_endpoint("https://push2.eastmoney.com.example/api").is_err());
    assert!(validate_endpoint("https://user@push2.eastmoney.com/api").is_err());
    assert!(validate_endpoint("https://push2.eastmoney.com:8443/api").is_err());
}

#[test]
fn cloned_transports_cannot_hold_two_remote_request_slots() {
    let mut first = HttpsTransport::new(Duration::from_secs(1)).unwrap();
    first.minimum_interval = Duration::ZERO;
    let second = first.clone();
    let gate = first.acquire_slot().unwrap();
    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _gate = second.acquire_slot().unwrap();
        sender.send(()).unwrap();
    });
    assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
    drop(gate);
    receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    worker.join().unwrap();
}

#[test]
fn cloned_transports_share_the_minimum_request_interval() {
    let mut first = HttpsTransport::new(Duration::from_secs(1)).unwrap();
    first.minimum_interval = Duration::from_millis(40);
    let second = first.clone();
    drop(first.acquire_slot().unwrap());
    let started = std::time::Instant::now();
    drop(second.acquire_slot().unwrap());
    assert!(started.elapsed() >= Duration::from_millis(30));
}

#[test]
fn only_documented_json_and_jsonp_media_types_are_accepted() {
    for content_type in [
        "application/json",
        "application/json; charset=utf-8",
        "application/javascript",
        "text/javascript; charset=UTF-8",
        "text/plain",
    ] {
        validate_content_type(Some(content_type)).unwrap();
    }
    assert!(validate_content_type(None).is_err());
    assert!(validate_content_type(Some("text/html")).is_err());
    assert!(validate_content_type(Some("application/octet-stream")).is_err());
    assert!(validate_html_content_type(None).is_err());
}

#[test]
fn response_reader_enforces_status_media_type_io_and_size_bounds() {
    assert_eq!(
        read_http_response(200, Some("application/json"), &b"{}"[..], 2).unwrap(),
        b"{}"
    );
    assert!(matches!(
        read_http_response(500, Some("application/json"), &b"{}"[..], 2),
        Err(super::EastmoneyError::Transport(message)) if message.contains("500")
    ));
    assert!(read_http_response(200, Some("text/html"), &b"{}"[..], 2).is_err());
    assert!(matches!(
        read_http_response(200, Some("application/json"), FailingReader, 2),
        Err(super::EastmoneyError::Transport(message)) if message.contains("fixture read failed")
    ));
    assert!(matches!(
        read_http_response(200, Some("application/json"), &b"abc"[..], 2),
        Err(super::EastmoneyError::ResponseTooLarge { limit: 2 })
    ));
    for limit in [0, DEFAULT_MAX_RESPONSE_BYTES + 1] {
        assert!(validate_response_limit(limit).is_err());
    }
    assert!(validate_response_limit(DEFAULT_MAX_RESPONSE_BYTES).is_ok());

    let response: ureq::Response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}"
        .parse()
        .unwrap();
    assert_eq!(HttpsTransport::read_response(response, 8).unwrap(), b"{}");
}

#[test]
fn redirect_transport_error_reports_header_presence_without_echoing_target() {
    let response: ureq::Response = concat!(
        "HTTP/1.1 302 Found\r\n",
        "Location: https://push2.eastmoney.com/redirect-target\r\n",
        "\r\n"
    )
    .parse()
    .unwrap();
    let error = map_ureq_error(ureq::Error::Status(302, response));
    assert!(matches!(
        error,
        super::EastmoneyError::Transport(message)
            if message.contains("302")
                && message.contains("Location present")
                && !message.contains("redirect-target")
    ));
}

#[test]
fn status_errors_cover_missing_redirect_location_and_non_redirect_status() {
    let redirect: ureq::Response = "HTTP/1.1 302 Found\r\n\r\n".parse().unwrap();
    assert!(matches!(
        map_ureq_error(ureq::Error::Status(302, redirect)),
        super::EastmoneyError::Transport(message)
            if message.contains("Location missing")
    ));

    let failure: ureq::Response = "HTTP/1.1 503 Service Unavailable\r\n\r\n".parse().unwrap();
    assert!(matches!(
        map_ureq_error(ureq::Error::Status(503, failure)),
        super::EastmoneyError::Transport(message)
            if message == "unexpected HTTP status 503"
    ));
}

#[test]
fn prepared_requests_and_public_trait_fail_before_any_unsafe_network_call() {
    assert!(HttpsTransport::new(Duration::ZERO).is_err());
    let mut transport = HttpsTransport::new(Duration::from_secs(1)).unwrap();
    transport.minimum_interval = Duration::ZERO;

    let get = transport.prepare_get("https://push2.eastmoney.com/api", &[("X-Test", "get")]);
    assert_eq!(get.method(), "GET");
    assert_eq!(get.url(), "https://push2.eastmoney.com/api");
    assert_eq!(get.header("User-Agent"), Some(super::USER_AGENT));
    assert_eq!(get.header("X-Test"), Some("get"));

    let post = transport.prepare_post(
        "https://datacenter-web.eastmoney.com/api",
        &[("X-Test", "post")],
    );
    assert_eq!(post.method(), "POST");
    assert_eq!(post.header("Content-Type"), Some("application/json"));
    assert_eq!(post.header("X-Test"), Some("post"));

    assert!(transport
        .get("http://push2.eastmoney.com/api", &[], 1)
        .is_err());
    assert!(transport
        .get("https://push2.eastmoney.com/api", &[], 0)
        .is_err());
    assert!(transport
        .post_json("https://datacenter-web.eastmoney.com/api", &[], &[], 0)
        .is_err());
    assert!(transport
        .post_json(
            "https://datacenter-web.eastmoney.com/api",
            &[],
            &vec![0; 64 * 1024 + 1],
            DEFAULT_MAX_RESPONSE_BYTES
        )
        .is_err());
    assert!(transport
        .get_pdf("https://pdf.dfcfw.com/pdf/test.pdf", &[], 0)
        .is_err());
    assert!(transport
        .get_html("https://roll.eastmoney.com/finance.html", &[], 0)
        .is_err());

    let gate = transport.acquire_slot().unwrap();
    transport.finish_request().unwrap();
    drop(gate);
    let snapshot = transport.load_probe_snapshot().unwrap();
    assert_eq!(snapshot.request_starts(), 1);
    assert_eq!(snapshot.active_requests(), 0);
}

#[test]
fn default_document_transport_methods_forward_without_fabricating_probe_data() {
    let transport = DefaultDocumentTransport;
    assert_eq!(
        transport
            .get_pdf("https://pdf.dfcfw.com/pdf/test.pdf", &[], 8)
            .unwrap(),
        b"forwarded"
    );
    assert_eq!(
        transport
            .get_html("https://roll.eastmoney.com/finance.html", &[], 8)
            .unwrap(),
        b"forwarded"
    );
    assert!(transport.load_probe_snapshot().is_none());
}

#[test]
fn poisoned_transport_gates_and_unbalanced_probe_completion_fail_explicitly() {
    let mut limiter_poisoned = HttpsTransport::new(Duration::from_secs(1)).unwrap();
    limiter_poisoned.minimum_interval = Duration::ZERO;
    let limiter = limiter_poisoned.last_request.clone();
    assert!(std::thread::spawn(move || {
        let _guard = limiter.lock().unwrap();
        panic!("poison limiter fixture");
    })
    .join()
    .is_err());
    assert!(limiter_poisoned
        .get("https://push2.eastmoney.com/api/qt/stock/get", &[], 8)
        .is_err());
    assert!(limiter_poisoned
        .post_json(
            "https://datacenter-web.eastmoney.com/api/data/v1/get",
            &[],
            b"{}",
            8
        )
        .is_err());

    let probe_poisoned = HttpsTransport::new(Duration::from_secs(1)).unwrap();
    let probe = probe_poisoned.request_probe.clone();
    assert!(std::thread::spawn(move || {
        let _guard = probe.lock().unwrap();
        panic!("poison probe fixture");
    })
    .join()
    .is_err());
    assert!(probe_poisoned.acquire_slot().is_err());
    assert!(probe_poisoned.finish_request().is_err());
    assert!(probe_poisoned.load_probe_snapshot().is_none());

    let unbalanced = HttpsTransport::new(Duration::from_secs(1)).unwrap();
    assert!(unbalanced.finish_request().is_err());
}
