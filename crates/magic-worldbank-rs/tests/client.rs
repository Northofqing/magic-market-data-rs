use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesRequest, PositiveU32, ProviderId,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use magic_worldbank_rs::{WorldBankClient, WorldBankError};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct FixtureTransport;

impl HttpTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            include_bytes!("fixtures/indicator.json").to_vec(),
        ))
    }
}

#[test]
fn diagnostic_path_surfaces_the_real_structured_unit_failure() {
    let client = WorldBankClient::with_transport(Arc::new(FixtureTransport)).unwrap();
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::WorldBank,
            "source:2/country:USA",
            "NY.GDP.MKTP.CD",
        )
        .unwrap()],
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2024).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let error = client.probe_economic_series(&request).unwrap_err();
    assert!(matches!(error, WorldBankError::Protocol(_)));
    assert!(error.to_string().contains("unit"));
}

struct CountingTransport(Arc<AtomicUsize>);

impl HttpTransport for CountingTransport {
    fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(TransportError::Network("must not execute".into()))
    }
}

#[test]
fn every_key_is_preflighted_before_path_construction_and_io() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client =
        WorldBankClient::with_transport(Arc::new(CountingTransport(calls.clone()))).unwrap();
    let request = EconomicSeriesRequest::new(
        vec![
            EconomicSeriesKey::new(
                ProviderId::WorldBank,
                "source:2/country:USA",
                "NY.GDP.MKTP.CD",
            )
            .unwrap(),
            EconomicSeriesKey::new(ProviderId::WorldBank, "source:2/country:CHN", "BAD/PATH")
                .unwrap(),
        ],
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2024).unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    assert!(client.probe_economic_series(&request).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
