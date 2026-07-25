use magic_market_core::{
    AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, NewsProvider, PositiveU32,
};
use magic_yonhap_rs::{
    HttpRequest, HttpResponse, YonhapChannel, YonhapClient, YonhapError, YonhapTransport,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct FixtureTransport {
    calls: Arc<AtomicUsize>,
}

impl YonhapTransport for FixtureTransport {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, YonhapError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse::new(
            request.url(),
            Some("application/rss+xml; charset=utf-8".into()),
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <rss version="2.0">
                  <channel>
                    <item>
                      <title>韩联社测试快讯</title>
                      <link>https://cn.yna.co.kr/view/ACK20260725001100881</link>
                      <guid>ACK20260725001100881</guid>
                      <pubDate>Sat, 25 Jul 2026 15:35:00 +0900</pubDate>
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

fn assert_provider_bounds<T>()
where
    T: NewsProvider<Error = YonhapError> + Send + Sync + Clone,
{
}

#[test]
fn public_capability_is_truthful_before_live_admission() {
    assert_provider_bounds::<YonhapClient>();
    let capabilities = YonhapClient::content_capabilities();
    assert!(!capabilities.instrument_news);
    assert!(!capabilities.global_news);
}

#[test]
fn unsupported_trait_calls_do_not_touch_transport() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = YonhapClient::with_channel_and_transport(
        YonhapChannel::Economy,
        FixtureTransport {
            calls: Arc::clone(&calls),
        },
    );

    assert!(matches!(
        client.instrument_news(&request()),
        Err(YonhapError::Unsupported(_))
    ));
    assert!(matches!(
        client.global_news(PositiveU32::new(1).unwrap()),
        Err(YonhapError::Unsupported(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn explicit_diagnostic_path_fetches_metadata() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = YonhapClient::with_channel_and_transport(
        YonhapChannel::Economy,
        FixtureTransport {
            calls: Arc::clone(&calls),
        },
    );

    let batch = client
        .probe_global_news(PositiveU32::new(1).unwrap())
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(batch.records().len(), 1);
    assert!(batch.records()[0].summary.is_none());
    assert!(batch.records()[0].content.is_none());
}

#[test]
fn invalid_diagnostic_limit_fails_before_transport() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = YonhapClient::with_channel_and_transport(
        YonhapChannel::Economy,
        FixtureTransport {
            calls: Arc::clone(&calls),
        },
    );

    assert!(matches!(
        client.probe_global_news(PositiveU32::new(51).unwrap()),
        Err(YonhapError::InvalidRequest(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
