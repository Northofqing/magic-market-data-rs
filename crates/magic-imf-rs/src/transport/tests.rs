use super::*;
use magic_market_core::{EconomicPeriod, EconomicSeriesKey, PositiveU32};
use magic_market_transport::{HttpResponse, TransportError};

struct NoIo;

impl HttpTransport for NoIo {
    fn execute(&self, _: &HttpRequest) -> Result<HttpResponse, TransportError> {
        unreachable!("preflight failures must not perform I/O")
    }
}

struct CompleteFixture;

impl HttpTransport for CompleteFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = if request.url().ends_with("/indicators") {
            include_bytes!("../../tests/fixtures/indicators.json").to_vec()
        } else {
            include_bytes!("../../tests/fixtures/series.json").to_vec()
        };
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body,
        ))
    }
}

struct OversizedCatalog;

impl HttpTransport for OversizedCatalog {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            vec![b' '; 8 * 1024 * 1024 + 1],
        ))
    }
}

fn annual_request(end: u32, max_rows: u32) -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Imf, "WEO/USA", "NGDP_RPCH").unwrap()],
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(end).unwrap(),
        PositiveU32::new(max_rows).unwrap(),
    )
    .unwrap()
}

#[test]
fn policy_component_and_timestamp_helpers_are_closed() {
    assert!(policy().is_ok());
    assert!(valid_component("NGDP_RPCH"));
    assert!(valid_component("A-1"));
    assert!(!valid_component(""));
    assert!(!valid_component("lowercase"));
    assert!(!valid_component(&"A".repeat(33)));
    assert!(observed_at().unwrap().contains('T'));
}

#[test]
fn duplicate_key_walker_visits_every_json_value_family() {
    let complete = br#"{"array":[true,-1,1,1.5,"text",null,{"nested":false}]}"#;
    ensure_no_duplicate_json_keys(complete).unwrap();
    assert!(ensure_no_duplicate_json_keys(br#"{"x":1,"x":2}"#).is_err());
    assert!(ensure_no_duplicate_json_keys(br#"{"x":1} trailing"#).is_err());
}

#[test]
fn transport_preflight_rejects_provider_range_and_area_limits() {
    let gate = RequestGate::new(Duration::from_nanos(1)).unwrap();
    let foreign = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap()],
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2025).unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        fetch_series(&NoIo, &gate, &foreign),
        Err(ImfError::InvalidRequest(_))
    ));
    assert!(matches!(
        fetch_series(&NoIo, &gate, &annual_request(2074, 100)),
        Err(ImfError::InvalidRequest(_))
    ));

    let series = (0..21)
        .map(|index| {
            EconomicSeriesKey::new(
                ProviderId::Imf,
                format!("WEO/A{index:02}"),
                format!("CODE{index}"),
            )
            .unwrap()
        })
        .collect();
    let too_many_areas = EconomicSeriesRequest::new(
        series,
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2025).unwrap(),
        PositiveU32::new(100).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        fetch_series(&NoIo, &gate, &too_many_areas),
        Err(ImfError::InvalidRequest(_))
    ));
}

#[test]
fn transport_enforces_catalog_and_result_limits() {
    let gate = RequestGate::new(Duration::from_nanos(1)).unwrap();
    assert!(matches!(
        fetch_series(&OversizedCatalog, &gate, &annual_request(2025, 2)),
        Err(ImfError::Protocol(_))
    ));
    assert!(matches!(
        fetch_series(&CompleteFixture, &gate, &annual_request(2026, 1)),
        Err(ImfError::InvalidRequest(_))
    ));
}

#[test]
fn transport_orders_multiple_areas_by_requested_series_identity() {
    let request = EconomicSeriesRequest::new(
        ["CHN", "USA"]
            .into_iter()
            .map(|area| {
                EconomicSeriesKey::new(ProviderId::Imf, format!("WEO/{area}"), "NGDP_RPCH").unwrap()
            })
            .collect(),
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2026).unwrap(),
        PositiveU32::new(6).unwrap(),
    )
    .unwrap();
    let gate = RequestGate::new(Duration::from_nanos(1)).unwrap();
    let batch = fetch_series(&CompleteFixture, &gate, &request).unwrap();
    assert_eq!(batch.records().len(), 5);
    assert!(batch.records()[..2]
        .iter()
        .all(|record| record.series().namespace() == "WEO/CHN"));
    assert!(batch.records()[2..]
        .iter()
        .all(|record| record.series().namespace() == "WEO/USA"));
}
