use magic_market_core::{
    EconomicObservationStatus, EconomicPeriod, EconomicSeriesKey, EconomicSeriesRequest,
    PositiveU32, ProviderId,
};
use magic_nbs_rs::parse_national_monthly_payload;

fn request() -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Nbs, "national-monthly", "A010101").unwrap()],
        EconomicPeriod::month(2025, 6).unwrap(),
        EconomicPeriod::month(2025, 6).unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap()
}

#[test]
fn present_zero_is_not_missing_and_provenance_is_nbs() {
    let batch = parse_national_monthly_payload(
        include_bytes!("fixtures/national-monthly.json"),
        &request(),
        "2026-07-29T00:00:00Z",
        "nbs-test",
    )
    .unwrap();
    let row = &batch.records()[0];
    assert_eq!(row.value().unwrap().get(), 0.0);
    assert_eq!(row.status(), &EconomicObservationStatus::Present);
    assert_eq!(row.evidence().provider(), ProviderId::Nbs);
}

#[test]
fn malformed_or_unrequested_metadata_fails_closed() {
    let bad_month = include_str!("fixtures/national-monthly.json").replace("202506", "202513");
    assert!(
        parse_national_monthly_payload(bad_month.as_bytes(), &request(), "observed", "batch")
            .is_err()
    );

    let missing_unit =
        include_str!("fixtures/national-monthly.json").replace(", \"unit\": \"点\"", "");
    assert!(parse_national_monthly_payload(
        missing_unit.as_bytes(),
        &request(),
        "observed",
        "batch"
    )
    .is_err());

    let duplicate = include_str!("fixtures/national-monthly.json").replace(
        "]}\n    ],",
        ", {\"code\":\"A010101\",\"name\":\"重复\",\"unit\":\"点\"}]}\n    ],",
    );
    assert!(
        parse_national_monthly_payload(duplicate.as_bytes(), &request(), "observed", "batch")
            .is_err()
    );

    let missing_node = include_str!("fixtures/national-monthly.json").replace(
        "{\"code\": \"zb.A010101_sj.202506\", \"data\": {\"data\": 0.0, \"hasdata\": true}}",
        "",
    );
    assert!(parse_national_monthly_payload(
        missing_node.as_bytes(),
        &request(),
        "observed",
        "batch"
    )
    .is_err());
}
