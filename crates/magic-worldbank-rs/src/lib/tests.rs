use super::*;
use magic_market_core::{EconomicPeriod, EconomicSeriesKey, PositiveU32};
use magic_market_transport::{HttpRequest, HttpResponse};

struct NoIo;
impl HttpTransport for NoIo {
    fn execute(&self, _: &HttpRequest) -> Result<HttpResponse, TransportError> {
        unreachable!()
    }
}

#[test]
fn constructor_debug_capabilities_and_foreign_request_are_explicit() {
    assert!(WorldBankClient::new().is_ok());
    let client = WorldBankClient::with_transport(Arc::new(NoIo)).unwrap();
    assert!(format!("{client:?}").contains("[REDACTED]"));
    assert!(WorldBankClient::economic_data_capabilities().economic_series);
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap()],
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2024).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert!(client.probe_economic_series(&request).is_err());
    assert!(matches!(
        client.economic_series(&request),
        Err(WorldBankError::Unsupported(_))
    ));
}
