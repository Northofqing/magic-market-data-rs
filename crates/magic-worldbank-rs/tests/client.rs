use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use magic_worldbank_rs::WorldBankClient;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct FixtureTransport;

impl HttpTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = if request.url().contains("/sources/2/series/") {
            include_bytes!("fixtures/series-metadata.json").to_vec()
        } else if request.url().contains("date=2024:2024") {
            br#"[{"page":1,"pages":1,"per_page":1000,"total":1,"sourceid":"2","lastupdated":"2026-07-13"},[{"indicator":{"id":"NY.GDP.MKTP.CD","value":"GDP (current US$)"},"country":{"id":"US","value":"United States"},"countryiso3code":"USA","date":"2024","value":29298013000000,"unit":"","obs_status":"","decimal":0}]]"#.to_vec()
        } else if request.url().contains("page=2") {
            include_bytes!("fixtures/data-page-2.json").to_vec()
        } else if request.url().contains("/country/") {
            include_bytes!("fixtures/data-page-1.json").to_vec()
        } else {
            include_bytes!("fixtures/indicator.json").to_vec()
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
fn formal_trait_admits_only_the_exact_proved_scope() {
    let client = WorldBankClient::with_transport(Arc::new(FixtureTransport)).unwrap();
    let admitted = EconomicSeriesRequest::new(
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
    let batch = client.economic_series(&admitted).unwrap();
    assert_eq!(batch.records().len(), 1);
    assert_eq!(batch.records()[0].unit(), "current US$");

    let calls = Arc::new(AtomicUsize::new(0));
    let client =
        WorldBankClient::with_transport(Arc::new(CountingTransport(calls.clone()))).unwrap();
    for (namespace, code, year) in [
        ("source:2/country:CHN", "NY.GDP.MKTP.CD", 2024),
        ("source:2/country:USA", "SP.POP.TOTL", 2024),
        ("source:2/country:USA", "NY.GDP.MKTP.CD", 2023),
    ] {
        let request = EconomicSeriesRequest::new(
            vec![EconomicSeriesKey::new(ProviderId::WorldBank, namespace, code).unwrap()],
            EconomicPeriod::year(year).unwrap(),
            EconomicPeriod::year(year).unwrap(),
            PositiveU32::new(1).unwrap(),
        )
        .unwrap();
        assert!(client.economic_series(&request).is_err());
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn diagnostic_path_uses_per_series_metadata_before_data() {
    let client = WorldBankClient::with_transport(Arc::new(FixtureTransport)).unwrap();
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::WorldBank,
            "source:2/country:USA",
            "NY.GDP.MKTP.CD",
        )
        .unwrap()],
        EconomicPeriod::year(2022).unwrap(),
        EconomicPeriod::year(2024).unwrap(),
        PositiveU32::new(3).unwrap(),
    )
    .unwrap();
    let batch = client.probe_economic_series(&request).unwrap();
    assert_eq!(batch.records().len(), 3);
    assert_eq!(batch.records()[0].unit(), "current US$");
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
