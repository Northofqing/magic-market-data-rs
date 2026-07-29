use magic_cfets_rs::{parse_lpr_payload, parse_shibor_payload};
use magic_market_core::{
    IsoDate, PositiveU32, ProviderId, RatioUnit, ReferenceRateIdentity, ReferenceRateKind,
    ReferenceRateRequest, ReferenceTenor,
};

fn request(kinds: Vec<ReferenceRateKind>, start: &str, end: &str) -> ReferenceRateRequest {
    ReferenceRateRequest::new(
        kinds
            .into_iter()
            .map(|kind| ReferenceRateIdentity::new(ProviderId::Cfets, kind).unwrap())
            .collect(),
        IsoDate::new(start).unwrap(),
        IsoDate::new(end).unwrap(),
        PositiveU32::new(100).unwrap(),
    )
    .unwrap()
}

#[test]
fn strict_shibor_metadata_maps_requested_percent_rates() {
    let request = request(
        vec![
            ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
            ReferenceRateKind::Shibor(ReferenceTenor::OneWeek),
        ],
        "2026-07-28",
        "2026-07-29",
    );
    let batch = parse_shibor_payload(
        include_bytes!("fixtures/shibor.json"),
        &request,
        "observed",
        "batch",
    )
    .unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(
        batch.records()[0].identity().kind(),
        &ReferenceRateKind::Shibor(ReferenceTenor::Overnight)
    );
    assert_eq!(
        batch.records()[1].identity().kind(),
        &ReferenceRateKind::Shibor(ReferenceTenor::OneWeek)
    );
    assert!(batch
        .records()
        .iter()
        .all(|row| row.unit() == RatioUnit::Percent));
}

#[test]
fn lpr_headings_and_message_fail_closed() {
    let request = request(
        vec![
            ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OverFiveYears),
            ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OneYear),
        ],
        "2026-07-01",
        "2026-07-29",
    );
    let batch = parse_lpr_payload(
        include_bytes!("fixtures/lpr.json"),
        &request,
        "observed",
        "batch",
    );
    let batch = batch.unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(
        batch.records()[0].identity().kind(),
        &ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OverFiveYears)
    );
    assert_eq!(
        batch.records()[1].identity().kind(),
        &ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OneYear)
    );

    let wrong_metadata = include_str!("fixtures/lpr.json").replace(
        "\"baseCurveCfgList\":[\"1Y\",\"5Y\"]",
        "\"baseCurveCfgList\":[{\"cfgItem\":\"1Y\"}]",
    );
    assert!(parse_lpr_payload(wrong_metadata.as_bytes(), &request, "observed", "batch").is_err());

    let mutation =
        include_str!("fixtures/lpr.json").replace("\"message\":\"\"", "\"message\":\"error\"");
    assert!(parse_lpr_payload(mutation.as_bytes(), &request, "observed", "batch").is_err());

    let empty = include_str!("fixtures/lpr.json").replace(
        "\"records\":[{\"showDateCN\":\"2026-07-20\",\"1Y\":\"3.00\",\"5Y\":\"3.50\"}]",
        "\"records\":[]",
    );
    assert!(parse_lpr_payload(empty.as_bytes(), &request, "observed", "batch").is_err());
}
