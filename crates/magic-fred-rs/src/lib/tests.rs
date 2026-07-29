use super::*;
use magic_market_core::{EconomicPeriod, EconomicSeriesKey, PositiveU32};
use magic_market_transport::{HttpRequest, HttpResponse};

struct NoIo;

impl HttpTransport for NoIo {
    fn execute(&self, _: &HttpRequest) -> Result<HttpResponse, TransportError> {
        unreachable!()
    }
}

fn request(provider: ProviderId, count: usize) -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        (0..count)
            .map(|index| {
                EconomicSeriesKey::new(provider, "fred", format!("SERIES_{index}")).unwrap()
            })
            .collect(),
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2025).unwrap(),
        PositiveU32::new(100).unwrap(),
    )
    .unwrap()
}

#[test]
fn api_key_and_request_preflight_cover_every_rejection() {
    assert!(checked_api_key(" key ".into()).is_ok());
    assert!(checked_api_key("\n".into()).is_err());
    assert!(checked_api_key("x".repeat(513)).is_err());
    assert!(validate_request(&request(ProviderId::Imf, 1)).is_err());
    assert!(validate_request(&request(ProviderId::Fred, 21)).is_err());
    let client = FredClient::with_transport("key", Arc::new(NoIo)).unwrap();
    assert!(format!("{client:?}").contains("[REDACTED]"));
}
