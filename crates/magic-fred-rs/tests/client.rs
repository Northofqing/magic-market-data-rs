use magic_fred_rs::FredClient;
use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct FixtureTransport;

impl HttpTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = if request.url().contains("/observations?") {
            include_bytes!("fixtures/observations.json").to_vec()
        } else {
            include_bytes!("fixtures/series.json").to_vec()
        };
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body,
        ))
    }
}

#[test]
fn injected_transport_runs_the_exact_bounded_path() {
    let client = FredClient::with_transport("fixture-key", Arc::new(FixtureTransport)).unwrap();
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap()],
        EconomicPeriod::quarter(2025, 1).unwrap(),
        EconomicPeriod::quarter(2025, 4).unwrap(),
        PositiveU32::new(4).unwrap(),
    )
    .unwrap();
    let batch = client.probe_economic_series(&request).unwrap();
    assert_eq!(batch.records().len(), 4);
}

struct CountingTransport(Arc<AtomicUsize>);

impl HttpTransport for CountingTransport {
    fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(TransportError::Network("must not execute".into()))
    }
}

#[test]
fn every_key_is_preflighted_before_first_io() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client =
        FredClient::with_transport("fixture-key", Arc::new(CountingTransport(calls.clone())))
            .unwrap();
    let request = EconomicSeriesRequest::new(
        vec![
            EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap(),
            EconomicSeriesKey::new(ProviderId::Fred, "wrong", "CPI").unwrap(),
        ],
        EconomicPeriod::quarter(2025, 1).unwrap(),
        EconomicPeriod::quarter(2025, 4).unwrap(),
        PositiveU32::new(8).unwrap(),
    )
    .unwrap();
    assert!(client.probe_economic_series(&request).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn unadmitted_formal_provider_fails_before_io() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client =
        FredClient::with_transport("fixture-key", Arc::new(CountingTransport(calls.clone())))
            .unwrap();
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap()],
        EconomicPeriod::quarter(2025, 1).unwrap(),
        EconomicPeriod::quarter(2025, 4).unwrap(),
        PositiveU32::new(4).unwrap(),
    )
    .unwrap();
    assert!(client.economic_series(&request).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
