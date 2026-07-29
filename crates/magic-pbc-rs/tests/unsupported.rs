use encoding_rs::GB18030;
use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use magic_pbc_rs::{PbcClient, PbcError};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct NoIo;

impl HttpTransport for NoIo {
    fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        panic!("unsupported PBC request must not perform I/O");
    }
}

#[test]
fn uncataloged_year_and_social_financing_fail_before_io() {
    let client = PbcClient::with_transport(Arc::new(NoIo)).unwrap();
    for (namespace, year) in [("money-supply", 2025), ("social-financing", 2024)] {
        let request = EconomicSeriesRequest::new(
            vec![EconomicSeriesKey::new(ProviderId::Pbc, namespace, "M2").unwrap()],
            EconomicPeriod::month(year, 1).unwrap(),
            EconomicPeriod::month(year, 12).unwrap(),
            PositiveU32::new(12).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            client.economic_series(&request),
            Err(PbcError::Unsupported(_))
        ));
    }
}

struct FixtureTransport {
    calls: Arc<AtomicUsize>,
}

impl HttpTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let (body, _, had_errors) = GB18030.encode(include_str!("fixtures/money-supply-2024.html"));
        assert!(!had_errors);
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("text/html; charset=GBK".into()),
            body.into_owned(),
        ))
    }
}

#[test]
fn diagnostic_probe_performs_one_bounded_fetch() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = PbcClient::with_transport(Arc::new(FixtureTransport {
        calls: Arc::clone(&calls),
    }))
    .unwrap();
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Pbc, "money-supply", "M2").unwrap()],
        EconomicPeriod::month(2024, 1).unwrap(),
        EconomicPeriod::month(2024, 12).unwrap(),
        PositiveU32::new(12).unwrap(),
    )
    .unwrap();
    assert_eq!(
        client.probe_money_supply(&request).unwrap().records().len(),
        12
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn admitted_formal_contract_uses_the_same_bounded_fetch() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = PbcClient::with_transport(Arc::new(FixtureTransport {
        calls: Arc::clone(&calls),
    }))
    .unwrap();
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Pbc, "money-supply", "M2").unwrap()],
        EconomicPeriod::month(2024, 1).unwrap(),
        EconomicPeriod::month(2024, 12).unwrap(),
        PositiveU32::new(12).unwrap(),
    )
    .unwrap();
    assert_eq!(
        client.economic_series(&request).unwrap().records().len(),
        12
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
