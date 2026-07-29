use magic_imf_rs::ImfClient;
use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_market_transport::{
    HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct FixtureTransport;

impl HttpTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        assert_eq!(request.method(), HttpMethod::Get);
        assert!(request.body().is_empty());
        assert_eq!(
            request.headers(),
            &[
                ("Accept".to_owned(), "application/json".to_owned()),
                ("User-Agent".to_owned(), "magic-imf-rs/0.2".to_owned()),
            ]
        );
        let body = if request.url() == "https://www.imf.org/external/datamapper/api/v2/indicators" {
            include_bytes!("fixtures/indicators.json").to_vec()
        } else {
            assert_eq!(
                request.url(),
                "https://www.imf.org/external/datamapper/api/v2/NGDP_RPCH/USA?periods=2024,2025"
            );
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
    let batch = client.probe_economic_series(&request).unwrap();
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
    assert!(client.probe_economic_series(&request).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn unadmitted_formal_provider_fails_before_io() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = ImfClient::with_transport(Arc::new(CountingTransport(calls.clone()))).unwrap();
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Imf, "WEO/USA", "NGDP_RPCH").unwrap()],
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2025).unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    assert!(client.economic_series(&request).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
