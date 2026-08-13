use super::*;
use magic_market_core::{EconomicPeriod, EconomicSeriesKey, PositiveU32};
use magic_market_transport::{HttpResponse, TransportError};

struct CompleteFixture;

impl HttpTransport for CompleteFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut body = if request.url().contains("/v2/indicator/") {
            include_bytes!("../../tests/fixtures/indicator.json").to_vec()
        } else if request.url().contains("/v2/sources/") {
            include_bytes!("../../tests/fixtures/series-metadata.json").to_vec()
        } else if request.url().contains("page=2") {
            include_bytes!("../../tests/fixtures/data-page-2.json").to_vec()
        } else {
            include_bytes!("../../tests/fixtures/data-page-1.json").to_vec()
        };
        if request.url().contains("/country/CHN/") {
            let mut value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            for row in value[1].as_array_mut().unwrap() {
                row["country"]["id"] = serde_json::Value::String("CN".into());
                row["country"]["value"] = serde_json::Value::String("China".into());
                row["countryiso3code"] = serde_json::Value::String("CHN".into());
            }
            body = serde_json::to_vec(&value).unwrap();
        }
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body,
        ))
    }
}

struct NoIo;

impl HttpTransport for NoIo {
    fn execute(&self, _: &HttpRequest) -> Result<HttpResponse, TransportError> {
        unreachable!("preflight failures must not perform I/O")
    }
}

struct ExcessivePageFixture;

impl HttpTransport for ExcessivePageFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = if request.url().contains("/v2/indicator/") {
            include_bytes!("../../tests/fixtures/indicator.json").to_vec()
        } else if request.url().contains("/v2/sources/") {
            include_bytes!("../../tests/fixtures/series-metadata.json").to_vec()
        } else {
            br#"[{"page":1,"pages":101,"per_page":1000,"total":0,"sourceid":"2","lastupdated":"2026-07-01"},[]]"#.to_vec()
        };
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body,
        ))
    }
}

fn annual_request(max_rows: u32) -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::WorldBank,
            "source:2/country:USA",
            "NY.GDP.MKTP.CD",
        )
        .unwrap()],
        EconomicPeriod::year(2022).unwrap(),
        EconomicPeriod::year(2024).unwrap(),
        PositiveU32::new(max_rows).unwrap(),
    )
    .unwrap()
}

#[test]
fn complete_injected_pages_cover_the_formal_transport_path() {
    let request = annual_request(3);
    let gate = RequestGate::new(Duration::from_nanos(1)).unwrap();
    let batch = fetch_series(&CompleteFixture, &gate, &request).unwrap();
    assert_eq!(batch.records().len(), 3);
}

#[test]
fn transport_preflight_rejects_provider_frequency_and_indicator_count() {
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
        Err(WorldBankError::InvalidRequest(_))
    ));

    let monthly = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::WorldBank,
            "source:2/country:USA",
            "NY.GDP.MKTP.CD",
        )
        .unwrap()],
        EconomicPeriod::month(2024, 1).unwrap(),
        EconomicPeriod::month(2024, 2).unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        fetch_series(&NoIo, &gate, &monthly),
        Err(WorldBankError::Unsupported(_))
    ));

    let unaudited_source = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::WorldBank,
            "source:11/country:USA",
            "NY.GDP.MKTP.CD",
        )
        .unwrap()],
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2024).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        fetch_series(&NoIo, &gate, &unaudited_source),
        Err(WorldBankError::Unsupported(_))
    ));

    let series = (0..61)
        .map(|index| {
            EconomicSeriesKey::new(
                ProviderId::WorldBank,
                "source:2/country:USA",
                format!("CODE{index}"),
            )
            .unwrap()
        })
        .collect();
    let too_many = EconomicSeriesRequest::new(
        series,
        EconomicPeriod::year(2024).unwrap(),
        EconomicPeriod::year(2025).unwrap(),
        PositiveU32::new(100).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        fetch_series(&NoIo, &gate, &too_many),
        Err(WorldBankError::InvalidRequest(_))
    ));
}

#[test]
fn transport_enforces_page_and_result_limits() {
    let gate = RequestGate::new(Duration::from_nanos(1)).unwrap();
    assert!(matches!(
        fetch_series(&ExcessivePageFixture, &gate, &annual_request(3)),
        Err(WorldBankError::Protocol(_))
    ));
    assert!(matches!(
        fetch_series(&CompleteFixture, &gate, &annual_request(1)),
        Err(WorldBankError::InvalidRequest(_))
    ));
}

#[test]
fn transport_orders_multiple_economy_aliases_by_requested_series_identity() {
    let request = EconomicSeriesRequest::new(
        ["CHN", "USA"]
            .into_iter()
            .map(|economy| {
                EconomicSeriesKey::new(
                    ProviderId::WorldBank,
                    format!("source:2/country:{economy}"),
                    "NY.GDP.MKTP.CD",
                )
                .unwrap()
            })
            .collect(),
        EconomicPeriod::year(2022).unwrap(),
        EconomicPeriod::year(2024).unwrap(),
        PositiveU32::new(6).unwrap(),
    )
    .unwrap();
    let gate = RequestGate::new(Duration::from_nanos(1)).unwrap();
    let batch = fetch_series(&CompleteFixture, &gate, &request).unwrap();
    assert_eq!(batch.records().len(), 6);
    assert!(batch.records()[..3]
        .iter()
        .all(|record| record.series().namespace() == "source:2/country:CHN"));
    assert!(batch.records()[3..]
        .iter()
        .all(|record| record.series().namespace() == "source:2/country:USA"));
}

#[test]
fn pure_url_page_and_json_helpers_are_strict() {
    assert!(policy().is_ok());
    assert!(valid_indicator_code("NY.GDP.MKTP.CD"));
    assert!(!valid_indicator_code(""));
    assert!(!valid_indicator_code("BAD/PATH"));
    assert!(!valid_indicator_code(&"A".repeat(65)));
    assert_eq!(
        data_url("USA", "GDP", "2", 2020, 2024, 3),
        "https://api.worldbank.org/v2/country/USA/indicator/GDP?format=json&date=2020:2024&page=3&per_page=1000&source=2"
    );
    assert_eq!(page_count(br#"[{"pages":2},[]]"#).unwrap(), 2);
    assert_eq!(page_count(br#"[{"pages":"3"},[]]"#).unwrap(), 3);
    for body in [
        br#"[]"#.as_slice(),
        br#"[{},[]]"#.as_slice(),
        br#"[{"pages":-1},[]]"#.as_slice(),
        br#"[{"pages":"bad"},[]]"#.as_slice(),
        br#"[{"pages":1,"pages":2},[]]"#.as_slice(),
    ] {
        assert!(page_count(body).is_err());
    }
    let complete = br#"{"array":[true,-1,1,1.5,"text",null,{"nested":false}]}"#;
    ensure_no_duplicate_json_keys(complete).unwrap();
    assert!(ensure_no_duplicate_json_keys(br#"{"x":1,"x":2}"#).is_err());
    assert!(ensure_no_duplicate_json_keys(br#"{"x":1} trailing"#).is_err());
    assert!(observed_at().unwrap().contains('T'));
}
