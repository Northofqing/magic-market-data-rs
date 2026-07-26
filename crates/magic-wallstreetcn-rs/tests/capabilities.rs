use magic_market_core::{
    AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, NewsProvider, PositiveU32,
};
use magic_wallstreetcn_rs::{
    HttpRequest, HttpResponse, WallstreetCnClient, WallstreetCnError, WallstreetCnTransport,
    GLOBAL_NEWS_ADMITTED,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct FixtureTransport {
    calls: Arc<AtomicUsize>,
}

impl WallstreetCnTransport for FixtureTransport {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, WallstreetCnError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse::new(
            request.url(),
            Some("text/html; charset=UTF-8".into()),
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <rss version="2.0">
                  <channel>
                    <title>华尔街见闻</title>
                    <link>https://wallstreetcn.com</link>
                    <language>zh-hans</language>
                    <item>
                      <title>合成财经快讯</title>
                      <link>https://wallstreetcn.com/articles/3779002</link>
                      <source>华尔街见闻</source>
                      <pubDate>Sun, 26 Jul 2026 10:30:00 +0800</pubDate>
                    </item>
                  </channel>
                </rss>"#
                .as_bytes()
                .to_vec(),
        ))
    }
}

fn request() -> InstrumentDateRangeRequest {
    InstrumentDateRangeRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600000", AssetClass::Equity).unwrap(),
        PositiveU32::new(5).unwrap(),
    )
    .unwrap()
}

fn fixture_client() -> (WallstreetCnClient, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = WallstreetCnClient::with_transport(FixtureTransport {
        calls: Arc::clone(&calls),
    });
    (client, calls)
}

fn assert_provider_bounds<T>()
where
    T: NewsProvider<Error = WallstreetCnError> + Send + Sync + Clone,
{
}

#[test]
fn public_capability_matches_live_admission() {
    assert_provider_bounds::<WallstreetCnClient>();
    let capabilities = WallstreetCnClient::content_capabilities();
    assert!(!capabilities.instrument_news);
    assert_eq!(capabilities.global_news, GLOBAL_NEWS_ADMITTED);
}

#[test]
fn instrument_news_is_unsupported_without_transport() {
    let (client, calls) = fixture_client();
    assert!(matches!(
        client.instrument_news(&request()),
        Err(WallstreetCnError::Unsupported(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn explicit_diagnostic_path_fetches_metadata_only() {
    let (client, calls) = fixture_client();
    let batch = client
        .probe_global_news(PositiveU32::new(1).unwrap())
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(batch.records().len(), 1);
    assert!(batch.records()[0].summary.is_none());
    assert!(batch.records()[0].content.is_none());
    assert!(batch.records()[0].instruments.is_empty());
}

#[test]
fn global_news_obeys_the_admission_flag() {
    let (client, calls) = fixture_client();
    let result = client.global_news(PositiveU32::new(1).unwrap());
    if GLOBAL_NEWS_ADMITTED {
        assert_eq!(result.unwrap().records().len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    } else {
        assert!(matches!(result, Err(WallstreetCnError::Unsupported(_))));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn invalid_diagnostic_limit_fails_before_transport() {
    let (client, calls) = fixture_client();
    assert!(matches!(
        client.probe_global_news(PositiveU32::new(51).unwrap()),
        Err(WallstreetCnError::InvalidRequest(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
