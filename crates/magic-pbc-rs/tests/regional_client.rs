use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use magic_pbc_rs::{PbcClient, REGIONAL_SOCIAL_FINANCING_URL};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const MIME: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

struct RegionalFixtureTransport {
    calls: Arc<AtomicUsize>,
}

impl HttpTransport for RegionalFixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.url(), REGIONAL_SOCIAL_FINANCING_URL);
        assert_eq!(request.headers(), &[("Accept".into(), MIME.into())]);
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some(MIME.into()),
            include_bytes!("fixtures/regional-social-financing-2025q1.xlsx").to_vec(),
        ))
    }
}

fn request() -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::Pbc,
            "regional-social-financing-flow",
            "AFRE_FLOW",
        )
        .unwrap()],
        EconomicPeriod::quarter(2025, 1).unwrap(),
        EconomicPeriod::quarter(2025, 1).unwrap(),
        PositiveU32::new(31).unwrap(),
    )
    .unwrap()
}

#[test]
fn admitted_formal_contract_uses_exact_attachment_and_mime() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = PbcClient::with_transport(Arc::new(RegionalFixtureTransport {
        calls: Arc::clone(&calls),
    }))
    .unwrap();
    let batch = client.economic_series(&request()).unwrap();
    assert_eq!(batch.records().len(), 31);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
