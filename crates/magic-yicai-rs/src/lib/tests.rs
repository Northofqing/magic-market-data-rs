use super::*;
use magic_market_transport::HttpResponse;

struct Failure;
impl HttpTransport for Failure {
    fn execute(&self, _: &HttpRequest) -> Result<HttpResponse, TransportError> {
        Err(TransportError::Network("fixture".into()))
    }
}

#[test]
fn constructors_helpers_limits_and_transport_failures_are_explicit() {
    assert!(YicaiClient::new().is_ok());
    assert!(YicaiClient::with_timeout(Duration::ZERO).is_err());
    assert!(endpoint_policy(Duration::from_secs(1)).is_ok());
    let built = request().unwrap();
    assert_eq!(built.url(), LIST_URL);
    assert!(validate_limit(PositiveU32::new(MAX_RETURNED_ITEMS).unwrap()).is_ok());
    assert!(validate_limit(PositiveU32::new(MAX_RETURNED_ITEMS + 1).unwrap()).is_err());
    assert!(matches!(tracker_error("x"), YicaiError::Transport(_)));
    let client = YicaiClient::with_transport(Failure).unwrap();
    assert!(format!("{client:?}").contains("YicaiClient"));
    assert!(client
        .probe_global_news(PositiveU32::new(1).unwrap())
        .is_err());
}
