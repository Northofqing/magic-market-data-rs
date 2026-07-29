use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use magic_nbs_rs::{NbsClient, NbsError};
use std::sync::Arc;

struct NoIo;

impl HttpTransport for NoIo {
    fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        panic!("unsupported production capability must not perform I/O");
    }
}

#[test]
fn production_series_is_explicitly_unsupported_before_io() {
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Nbs, "national-monthly", "A010101").unwrap()],
        EconomicPeriod::month(2025, 6).unwrap(),
        EconomicPeriod::month(2025, 6).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let client = NbsClient::with_transport(Arc::new(NoIo)).unwrap();
    assert!(matches!(
        client.economic_series(&request),
        Err(NbsError::Unsupported(_))
    ));
}
