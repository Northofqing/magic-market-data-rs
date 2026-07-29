use super::*;
use magic_market_transport::{HttpResponse, TransportError};

struct Fixture;

impl HttpTransport for Fixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        assert_eq!(request.url(), LANDING_URL);
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("text/html".into()),
            b"landing".to_vec(),
        ))
    }
}

#[test]
fn landing_probe_builds_the_exact_request() {
    let gate = RequestGate::new(std::time::Duration::from_nanos(1)).unwrap();
    assert_eq!(probe_landing_page(&Fixture, &gate).unwrap(), 7);
}
