use magic_market_core::{
    AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, NewsProvider, PositiveU32,
};
use magic_market_transport::{
    HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use magic_stcn_rs::{StcnClient, StcnError, GLOBAL_NEWS_ADMITTED};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct CountingTransport(Arc<AtomicUsize>);

impl HttpTransport for CountingTransport {
    fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(TransportError::Network("test transport".into()))
    }
}

#[test]
fn only_global_news_can_be_admitted() {
    let capabilities = StcnClient::content_capabilities();
    assert!(!capabilities.instrument_news);
    assert_eq!(capabilities.global_news, GLOBAL_NEWS_ADMITTED);
    assert!(!capabilities.announcements);
    assert!(!capabilities.market_announcements);
    assert!(!capabilities.investor_questions);
}

#[test]
fn unsupported_instrument_news_performs_no_io() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = StcnClient::with_transport(CountingTransport(calls.clone())).expect("client");
    let request = InstrumentDateRangeRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600000", AssetClass::Equity).expect("instrument"),
        PositiveU32::new(5).expect("limit"),
    )
    .expect("request");
    assert!(matches!(
        client.instrument_news(&request),
        Err(StcnError::Unsupported(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

struct RecordingTransport(Arc<Mutex<Vec<HttpRequest>>>);

impl HttpTransport for RecordingTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.lock().expect("request lock").push(request.clone());
        Ok(HttpResponse::new(
            200,
            "https://www.stcn.com/article/list.html?type=kx",
            Some("application/json; charset=utf-8".into()),
            include_bytes!("fixtures/quick-news.json").to_vec(),
        ))
    }
}

#[test]
fn initial_probe_uses_exact_xhr_headers_and_omits_cursors() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let client = StcnClient::with_transport(RecordingTransport(requests.clone())).expect("client");
    let batch = client
        .probe_global_news(PositiveU32::new(1).expect("limit"))
        .expect("fixture response");
    assert_eq!(batch.records().len(), 1);
    client
        .probe_global_news(PositiveU32::new(1).expect("limit"))
        .expect("second fixture response");

    let requests = requests.lock().expect("request lock");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
    assert_eq!(requests[0].method(), HttpMethod::Get);
    assert_eq!(
        requests[0].url(),
        "https://www.stcn.com/article/list.html?type=kx"
    );
    assert!(!requests[0].url().contains("page_time"));
    assert!(!requests[0].url().contains("last_time"));
    assert_eq!(
        requests[0].headers(),
        [
            (
                "Accept".into(),
                "application/json, text/javascript, */*; q=0.01".into(),
            ),
            ("X-Requested-With".into(), "XMLHttpRequest".into()),
            (
                "Referer".into(),
                "https://www.stcn.com/article/list/kx.html".into(),
            ),
            ("User-Agent".into(), "magic-stcn-rs/0.2".into()),
        ]
    );
    assert!(requests[0].body().is_empty());
    drop(requests);

    let snapshot = client.load_probe_snapshot().expect("load snapshot");
    assert_eq!(snapshot.request_starts(), 2);
    assert!(snapshot.minimum_start_gap().expect("two request starts") >= Duration::from_secs(1));
    assert_eq!(snapshot.maximum_concurrency(), 1);
    assert_eq!(snapshot.active_requests(), 0);
}

#[test]
fn admitted_formal_global_news_uses_the_same_bounded_path() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let client = StcnClient::with_transport(RecordingTransport(requests.clone())).expect("client");
    assert_eq!(
        client
            .global_news(PositiveU32::new(1).expect("limit"))
            .expect("admitted fixture response")
            .records()
            .len(),
        1
    );
    assert_eq!(requests.lock().expect("request lock").len(), 1);
}
