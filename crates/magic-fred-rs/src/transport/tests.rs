use super::*;
use magic_market_core::{EconomicSeriesKey, EconomicSeriesRequest, PositiveU32, ProviderId};
use magic_market_transport::{HttpResponse, TransportError};

struct CompleteFixture;

impl HttpTransport for CompleteFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut body = if request.url().contains("/observations?") {
            include_bytes!("../../tests/fixtures/observations.json").to_vec()
        } else {
            include_bytes!("../../tests/fixtures/series.json").to_vec()
        };
        if request.url().contains("series_id=CPI") && !request.url().contains("/observations?") {
            body = String::from_utf8(body)
                .unwrap()
                .replace(r#""id":"GDP""#, r#""id":"CPI""#)
                .into_bytes();
        }
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body,
        ))
    }
}

#[test]
fn pure_request_helpers_cover_every_admitted_frequency_and_encoding() {
    assert!(policy().is_ok());
    for (period, start, end) in [
        (
            EconomicPeriod::day("2024-02-29").unwrap(),
            "2024-02-29",
            "2024-02-29",
        ),
        (
            EconomicPeriod::iso_week(2025, 1).unwrap(),
            "2024-12-30",
            "2025-01-05",
        ),
        (
            EconomicPeriod::month(2024, 2).unwrap(),
            "2024-02-01",
            "2024-02-29",
        ),
        (
            EconomicPeriod::month(2023, 2).unwrap(),
            "2023-02-01",
            "2023-02-28",
        ),
        (
            EconomicPeriod::quarter(2025, 2).unwrap(),
            "2025-04-01",
            "2025-06-30",
        ),
        (
            EconomicPeriod::year(2025).unwrap(),
            "2025-01-01",
            "2025-12-31",
        ),
    ] {
        assert_eq!(period_date(&period, true).unwrap(), start);
        assert_eq!(period_date(&period, false).unwrap(), end);
    }
    assert!(matches!(
        period_date(&EconomicPeriod::irregular("source-period").unwrap(), true),
        Err(FredError::Unsupported(_))
    ));
    assert_eq!(
        query_url("https://example.test", &[("q", "A B/+~")]),
        "https://example.test?q=A%20B%2F%2B~"
    );
    assert!(valid_code("GDP.US-1"));
    assert!(!valid_code(""));
    assert!(!valid_code(&"A".repeat(65)));
    assert!(!valid_code("BAD/PATH"));
    assert!(observed_at().unwrap().contains('T'));
}

#[test]
fn key_validation_is_provider_namespace_and_code_exact() {
    assert!(
        validate_key(&EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap()).is_ok()
    );
    assert!(
        validate_key(&EconomicSeriesKey::new(ProviderId::Imf, "fred", "GDP").unwrap()).is_err()
    );
    assert!(
        validate_key(&EconomicSeriesKey::new(ProviderId::Fred, "wrong", "GDP").unwrap()).is_err()
    );
    assert!(
        validate_key(&EconomicSeriesKey::new(ProviderId::Fred, "fred", "BAD/PATH").unwrap())
            .is_err()
    );
}

#[test]
fn duplicate_key_walker_visits_every_json_value_family() {
    let complete = br#"{"array":[true,-1,1,1.5,"text",null,{"nested":false}]}"#;
    ensure_no_duplicate_json_keys(complete).unwrap();
    assert!(ensure_no_duplicate_json_keys(br#"{"x":1,"x":2}"#).is_err());
    assert!(ensure_no_duplicate_json_keys(br#"{"x":1} trailing"#).is_err());
}

#[test]
fn complete_transport_rejects_results_above_the_caller_budget() {
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap()],
        EconomicPeriod::quarter(2025, 1).unwrap(),
        EconomicPeriod::quarter(2025, 4).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let gate = RequestGate::new(Duration::from_nanos(1)).unwrap();
    assert!(matches!(
        fetch_series(&CompleteFixture, &gate, "fixture-key", &request),
        Err(FredError::InvalidRequest(_))
    ));
}

#[test]
fn complete_transport_orders_multiple_series_by_request_identity() {
    let request = EconomicSeriesRequest::new(
        ["CPI", "GDP"]
            .into_iter()
            .map(|code| EconomicSeriesKey::new(ProviderId::Fred, "fred", code).unwrap())
            .collect(),
        EconomicPeriod::quarter(2025, 1).unwrap(),
        EconomicPeriod::quarter(2025, 4).unwrap(),
        PositiveU32::new(8).unwrap(),
    )
    .unwrap();
    let gate = RequestGate::new(Duration::from_nanos(1)).unwrap();
    let batch = fetch_series(&CompleteFixture, &gate, "fixture-key", &request).unwrap();
    assert_eq!(batch.records().len(), 8);
    assert!(batch.records()[..4]
        .iter()
        .all(|record| record.series().code() == "CPI"));
    assert!(batch.records()[4..]
        .iter()
        .all(|record| record.series().code() == "GDP"));
}
