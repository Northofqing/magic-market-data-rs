use super::*;
use magic_market_core::{EconomicPeriod, EconomicSeriesKey, PositiveU32};
use magic_market_transport::{HttpRequest, HttpResponse};

struct NoIo;
impl HttpTransport for NoIo {
    fn execute(&self, _: &HttpRequest) -> Result<HttpResponse, TransportError> {
        unreachable!()
    }
}

fn request(provider: ProviderId, count: usize, annual: bool) -> EconomicSeriesRequest {
    let start = if annual {
        EconomicPeriod::year(2024).unwrap()
    } else {
        EconomicPeriod::month(2024, 1).unwrap()
    };
    let end = if annual {
        EconomicPeriod::year(2025).unwrap()
    } else {
        EconomicPeriod::month(2024, 2).unwrap()
    };
    EconomicSeriesRequest::new(
        (0..count)
            .map(|index| {
                EconomicSeriesKey::new(provider, "WEO/USA", format!("SERIES_{index}")).unwrap()
            })
            .collect(),
        start,
        end,
        PositiveU32::new(100).unwrap(),
    )
    .unwrap()
}

#[test]
fn constructor_debug_and_request_preflight_cover_every_rejection() {
    assert!(ImfClient::new().is_ok());
    let client = ImfClient::with_transport(Arc::new(NoIo)).unwrap();
    assert!(format!("{client:?}").contains("[REDACTED]"));
    assert!(validate_request(&request(ProviderId::Fred, 1, true)).is_err());
    assert!(validate_request(&request(ProviderId::Imf, 21, true)).is_err());
    assert!(validate_request(&request(ProviderId::Imf, 1, false)).is_err());
}
