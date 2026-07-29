use magic_market_core::{
    EconomicFrequency, EconomicObservation, EconomicObservationStatus, EconomicPeriod,
    EconomicSeriesKey, EconomicSeriesRequest, FiniteNumber, NonEmptyText, PositiveU32, ProviderId,
    SourceEvidence, SourcedRecord,
};

fn key(code: &str) -> EconomicSeriesKey {
    EconomicSeriesKey::new(ProviderId::Pbc, "money-supply", code).unwrap()
}

fn evidence() -> SourceEvidence {
    SourceEvidence::new(ProviderId::Pbc, "2026-07-29T10:00:00Z", "pbc-1").unwrap()
}

#[test]
fn request_rejects_empty_duplicate_cross_provider_and_reversed_ranges() {
    let jan = EconomicPeriod::month(2025, 1).unwrap();
    let feb = EconomicPeriod::month(2025, 2).unwrap();
    let limit = PositiveU32::new(10).unwrap();
    assert!(EconomicSeriesRequest::new(vec![], jan.clone(), feb.clone(), limit).is_err());
    assert!(EconomicSeriesRequest::new(
        vec![key("M2"), key("M2")],
        jan.clone(),
        feb.clone(),
        limit
    )
    .is_err());
    let foreign = EconomicSeriesKey::new(ProviderId::Fred, "fred", "M2SL").unwrap();
    assert!(
        EconomicSeriesRequest::new(vec![key("M2"), foreign], jan.clone(), feb.clone(), limit)
            .is_err()
    );
    assert!(EconomicSeriesRequest::new(vec![key("M2")], feb, jan, limit).is_err());
}

#[test]
fn periods_validate_frequency_specific_boundaries() {
    assert_eq!(
        EconomicPeriod::day("2024-02-29").unwrap().frequency(),
        EconomicFrequency::Daily
    );
    assert!(EconomicPeriod::day("2023-02-29").is_err());
    assert!(EconomicPeriod::iso_week(2025, 0).is_err());
    assert!(EconomicPeriod::iso_week(2025, 54).is_err());
    assert!(EconomicPeriod::month(2025, 13).is_err());
    assert!(EconomicPeriod::quarter(2025, 5).is_err());
}

#[test]
fn periods_expose_read_only_checked_components() {
    assert_eq!(
        EconomicPeriod::day("2026-07-29").unwrap().as_day(),
        Some("2026-07-29")
    );
    assert_eq!(
        EconomicPeriod::iso_week(2026, 31).unwrap().as_iso_week(),
        Some((2026, 31))
    );
    assert_eq!(
        EconomicPeriod::month(2026, 7).unwrap().as_month(),
        Some((2026, 7))
    );
    assert_eq!(
        EconomicPeriod::quarter(2026, 3).unwrap().as_quarter(),
        Some((2026, 3))
    );
    assert_eq!(EconomicPeriod::year(2026).unwrap().as_year(), Some(2026));
    assert_eq!(
        EconomicPeriod::irregular("2026-H1").unwrap().as_irregular(),
        Some("2026-H1")
    );
}

#[test]
fn periods_preserve_the_existing_json_wire_shape() {
    let cases = [
        (
            EconomicPeriod::day("2026-07-29").unwrap(),
            r#"{"Daily":"2026-07-29"}"#,
        ),
        (
            EconomicPeriod::iso_week(2026, 31).unwrap(),
            r#"{"Weekly":{"year":2026,"week":31}}"#,
        ),
        (
            EconomicPeriod::month(2026, 7).unwrap(),
            r#"{"Monthly":{"year":2026,"month":7}}"#,
        ),
        (
            EconomicPeriod::quarter(2026, 3).unwrap(),
            r#"{"Quarterly":{"year":2026,"quarter":3}}"#,
        ),
        (
            EconomicPeriod::year(2026).unwrap(),
            r#"{"Annual":{"year":2026}}"#,
        ),
        (
            EconomicPeriod::irregular("2026-H1").unwrap(),
            r#"{"Irregular":"2026-H1"}"#,
        ),
    ];
    for (period, expected) in cases {
        assert_eq!(serde_json::to_string(&period).unwrap(), expected);
        assert_eq!(
            serde_json::from_str::<EconomicPeriod>(expected).unwrap(),
            period
        );
    }
}

#[test]
fn present_requires_value_and_non_present_forbids_value() {
    let period = EconomicPeriod::month(2025, 1).unwrap();
    assert!(EconomicObservation::new(
        key("M2"),
        "广义货币(M2)",
        None,
        None,
        period.clone(),
        None,
        "亿元",
        None,
        None,
        EconomicObservationStatus::Present,
        None,
        None,
        evidence(),
    )
    .is_err());
    assert!(EconomicObservation::new(
        key("M2"),
        "广义货币(M2)",
        None,
        None,
        period,
        Some(FiniteNumber::new(0.0).unwrap()),
        "亿元",
        None,
        None,
        EconomicObservationStatus::Missing,
        None,
        None,
        evidence(),
    )
    .is_err());
}

#[test]
fn serde_cannot_bypass_missing_value_invariant() {
    let json = r#"{
      "series":{"provider":"Pbc","namespace":"money-supply","code":"M2"},
      "name":"广义货币(M2)","region_code":null,"region_name":null,
      "period":{"Monthly":{"year":2025,"month":1}},"value":0.0,
      "unit":"亿元","scale":null,"seasonal_adjustment":null,
      "status":"Missing","released_at":null,"revision":null,
      "evidence":{"provider":"Pbc","source_at":null,
        "observed_at":"2026-07-29T10:00:00Z","batch_id":"pbc-1"}
    }"#;
    assert!(serde_json::from_str::<EconomicObservation>(json).is_err());
}

#[test]
fn economic_observation_rejects_provider_and_release_evidence_conflicts() {
    let period = EconomicPeriod::month(2025, 1).unwrap();
    let value = Some(FiniteNumber::new(1.0).unwrap());
    let foreign = SourceEvidence::new(ProviderId::Fred, "2026-07-29T10:00:00Z", "fred-1").unwrap();
    assert!(EconomicObservation::new(
        key("M2"),
        "广义货币(M2)",
        None,
        None,
        period.clone(),
        value,
        "亿元",
        None,
        None,
        EconomicObservationStatus::Present,
        None,
        None,
        foreign,
    )
    .is_err());

    let sourced = evidence().with_source_at("2026-07-29T09:00:00Z").unwrap();
    assert!(EconomicObservation::new(
        key("M2"),
        "广义货币(M2)",
        None,
        None,
        period,
        value,
        "亿元",
        None,
        None,
        EconomicObservationStatus::Present,
        Some(NonEmptyText::new("2026-07-29T08:00:00Z").unwrap()),
        None,
        sourced,
    )
    .is_err());
}

#[test]
fn economic_observation_exposes_all_source_evidence() {
    let released_at = "2026-07-29T09:00:00Z";
    let observed_at = "2026-07-29T10:00:00Z";
    let evidence = SourceEvidence::new(ProviderId::Pbc, observed_at, "pbc-evidence")
        .unwrap()
        .with_source_at(released_at)
        .unwrap();
    let observation = EconomicObservation::new(
        key("M2"),
        "广义货币(M2)",
        None,
        None,
        EconomicPeriod::month(2025, 1).unwrap(),
        Some(FiniteNumber::new(1.0).unwrap()),
        "亿元",
        None,
        None,
        EconomicObservationStatus::Present,
        Some(NonEmptyText::new(released_at).unwrap()),
        None,
        evidence,
    )
    .unwrap();
    assert_eq!(observation.provider_id(), ProviderId::Pbc);
    assert_eq!(observation.evidence_batch_id(), "pbc-evidence");
    assert_eq!(observation.evidence_source_at(), Some(released_at));
    assert_eq!(observation.evidence_observed_at(), Some(observed_at));
}
