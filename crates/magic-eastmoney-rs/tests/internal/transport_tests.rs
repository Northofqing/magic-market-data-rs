use super::{validate_content_type, validate_endpoint, HttpsTransport};
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn endpoint_allowlist_rejects_non_https_redirect_targets_and_lookalikes() {
    assert!(validate_endpoint("https://push2.eastmoney.com/api").is_ok());
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
}
