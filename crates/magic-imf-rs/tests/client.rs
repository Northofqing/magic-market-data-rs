use magic_imf_rs::ImfClient;
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
        let body = if request.url().ends_with("/indicators") {
            include_bytes!("fixtures/indicators.json").to_vec()
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
fn injected_transport_validates_superset_before_returning_rows() {
    let client = ImfClient::with_transport(Arc::new(FixtureTransport)).unwrap();
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Imf, "WEO/USA", "NGDP_RPCH").unwrap()],
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2025).unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    let batch = client.economic_series(&request).unwrap();
    assert_eq!(batch.records().len(), 2);
}

struct CountingTransport(Arc<AtomicUsize>);

impl HttpTransport for CountingTransport {
    fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(TransportError::Network("must not execute".into()))
    }
}

#[test]
fn every_key_is_preflighted_before_catalog_io() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = ImfClient::with_transport(Arc::new(CountingTransport(calls.clone()))).unwrap();
    let request = EconomicSeriesRequest::new(
        vec![
            EconomicSeriesKey::new(ProviderId::Imf, "WEO/USA", "NGDP_RPCH").unwrap(),
            EconomicSeriesKey::new(ProviderId::Imf, "WEO/CHN", "lowercase").unwrap(),
        ],
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2025).unwrap(),
        PositiveU32::new(4).unwrap(),
    )
    .unwrap();
    assert!(client.economic_series(&request).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
