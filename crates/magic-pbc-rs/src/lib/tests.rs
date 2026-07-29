use super::*;
use magic_market_core::{EconomicPeriod, EconomicSeriesKey, PositiveU32};
use magic_market_transport::{HttpRequest, HttpResponse};

struct NoIo;
impl HttpTransport for NoIo {
    fn execute(&self, _: &HttpRequest) -> Result<HttpResponse, TransportError> {
        unreachable!()
    }
}

fn request(
    provider: ProviderId,
    namespace: &str,
    codes: &[&str],
    start: EconomicPeriod,
    end: EconomicPeriod,
) -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        codes
            .iter()
            .map(|code| EconomicSeriesKey::new(provider, namespace, *code).unwrap())
            .collect(),
        start,
        end,
        PositiveU32::new(100).unwrap(),
    )
    .unwrap()
}

#[test]
fn constructors_capabilities_snapshot_and_timestamp_are_covered() {
    assert!(PbcClient::new(Duration::ZERO).is_err());
    assert!(PbcClient::new(Duration::from_secs(1)).is_ok());
    let client = PbcClient::with_transport(Arc::new(NoIo)).unwrap();
    let snapshot = client.load_probe_snapshot().unwrap();
    assert_eq!(snapshot.request_starts(), 0);
    assert!(PbcClient::economic_data_capabilities().economic_series);
    assert!(now_timestamp().contains('T'));
}

#[test]
fn request_preflight_rejects_every_unadmitted_shape_before_io() {
    let month = || EconomicPeriod::month(2024, 1).unwrap();
    assert!(validate_request(&request(
        ProviderId::Fred,
        "money-supply",
        &["M2"],
        month(),
        month(),
    ))
    .is_err());
    assert!(validate_request(&request(
        ProviderId::Pbc,
        "social-financing",
        &["M2"],
        month(),
        month(),
    ))
    .is_err());
    assert!(validate_request(&request(
        ProviderId::Pbc,
        "money-supply",
        &["M0", "M1", "M2", "M3"],
        month(),
        month(),
    ))
    .is_err());
    assert!(validate_request(&request(
        ProviderId::Pbc,
        "money-supply",
        &["M2"],
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2024).unwrap(),
    ))
    .is_err());
    let cross_year = request(
        ProviderId::Pbc,
        "money-supply",
        &["M2"],
        EconomicPeriod::month(2024, 12).unwrap(),
        EconomicPeriod::month(2025, 1).unwrap(),
    );
    let client = PbcClient::with_transport(Arc::new(NoIo)).unwrap();
    assert!(matches!(
        client.probe_money_supply(&cross_year),
        Err(PbcError::Unsupported(_))
    ));
}
