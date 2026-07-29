use super::*;
use magic_market_core::{EconomicPeriod, EconomicSeriesKey, PositiveU32, ProviderId};
use magic_market_transport::{HttpRequest, HttpResponse};

struct Landing;

impl HttpTransport for Landing {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("text/html".into()),
            b"official".to_vec(),
        ))
    }
}

fn request() -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Nbs, "national-monthly", "A010101").unwrap()],
        EconomicPeriod::month(2025, 6).unwrap(),
        EconomicPeriod::month(2025, 6).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap()
}

#[test]
fn diagnostics_constructors_and_landing_path_are_bounded() {
    assert!(NbsClient::new(Duration::ZERO).is_err());
    assert!(NbsClient::new(Duration::from_secs(1)).is_ok());
    let client = NbsClient::with_transport(Arc::new(Landing)).unwrap();
    assert_eq!(client.probe_public_landing_page().unwrap(), 8);
    let diagnostic = NbsDiagnosticRequest::new(request(), b"{}".to_vec()).unwrap();
    assert_eq!(diagnostic.body(), b"{}");
    assert_eq!(diagnostic.request().provider(), ProviderId::Nbs);
    assert!(NbsDiagnosticRequest::new(request(), vec![0; max_response_bytes() + 1]).is_err());
}

#[test]
fn both_diagnostic_parse_entry_points_share_the_strict_contract() {
    let client = NbsClient::with_transport(Arc::new(Landing)).unwrap();
    let diagnostic = NbsDiagnosticRequest::new(request(), b"{}".to_vec()).unwrap();
    assert!(client
        .probe_national_payload(diagnostic.request(), diagnostic.body(), "observed")
        .is_err());
    assert!(client
        .probe_national_diagnostic(&diagnostic, "observed")
        .is_err());
    assert!(!NbsClient::economic_data_capabilities().economic_series);
}
