use magic_market_core::{
    CurrencyCode, FiniteNumber, IsoDate, OfficialFxFixing, OfficialFxFixingIdentity,
    OfficialFxFixingRequest, PositiveU32, ProviderId, RatioUnit, ReferenceRateIdentity,
    ReferenceRateKind, ReferenceRateObservation, ReferenceRateRequest, ReferenceTenor,
    SourceEvidence, SourcedRecord,
};

#[test]
fn currency_and_request_identities_are_checked() {
    assert_eq!(CurrencyCode::new("cny").unwrap().as_str(), "CNY");
    assert!(CurrencyCode::new("CN").is_err());
    assert!(CurrencyCode::new("C1Y").is_err());
    let pair = OfficialFxFixingIdentity::new(
        ProviderId::Cfets,
        CurrencyCode::new("USD").unwrap(),
        CurrencyCode::new("CNY").unwrap(),
    )
    .unwrap();
    assert!(OfficialFxFixingRequest::new(
        vec![pair.clone(), pair],
        IsoDate::new("2026-07-01").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(50).unwrap(),
    )
    .is_err());
}

#[test]
fn official_fixing_requires_positive_value_and_quotation_base() {
    let evidence =
        SourceEvidence::new(ProviderId::Cfets, "2026-07-29T02:00:00Z", "cfets-1").unwrap();
    assert!(OfficialFxFixing::new(
        CurrencyCode::new("USD").unwrap(),
        CurrencyCode::new("CNY").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        FiniteNumber::new(0.0).unwrap(),
        PositiveU32::new(1).unwrap(),
        None,
        None,
        evidence,
    )
    .is_err());
}

#[test]
fn serde_cannot_bypass_reference_data_invariants() {
    let zero = r#"{
      "identity":{"provider":"Cfets","base":"USD","quote":"CNY"},
      "fixing_date":"2026-07-29","value":0.0,"quotation_base":1,
      "published_at":null,"revision":null,
      "evidence":{"provider":"Cfets","source_at":null,
        "observed_at":"2026-07-29T02:00:00Z","batch_id":"cfets-1"}
    }"#;
    assert!(serde_json::from_str::<OfficialFxFixing>(zero).is_err());

    let duplicate = r#"{
      "rates":[
        {"provider":"Cfets","kind":{"Shibor":"OneMonth"}},
        {"provider":"Cfets","kind":{"Shibor":"OneMonth"}}
      ],
      "start":"2026-07-01","end":"2026-07-29","max_rows":50
    }"#;
    assert!(serde_json::from_str::<ReferenceRateRequest>(duplicate).is_err());

    let invalid_lpr = r#"{"provider":"Cfets","kind":{"LoanPrimeRate":"OneMonth"}}"#;
    assert!(serde_json::from_str::<ReferenceRateIdentity>(invalid_lpr).is_err());

    let valid = ReferenceRateIdentity::new(
        ProviderId::Cfets,
        ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
    )
    .unwrap();
    assert_eq!(valid.provider(), ProviderId::Cfets);
}

#[test]
fn reference_records_reject_provider_and_publication_evidence_conflicts() {
    let identity = ReferenceRateIdentity::new(
        ProviderId::Cfets,
        ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
    )
    .unwrap();
    let foreign = SourceEvidence::new(ProviderId::Fred, "2026-07-29T03:00:00Z", "fred-1").unwrap();
    assert!(ReferenceRateObservation::new(
        identity.clone(),
        IsoDate::new("2026-07-29").unwrap(),
        FiniteNumber::new(1.0).unwrap(),
        RatioUnit::Percent,
        None,
        None,
        foreign,
    )
    .is_err());

    let sourced = SourceEvidence::new(ProviderId::Cfets, "2026-07-29T03:00:00Z", "cfets-1")
        .unwrap()
        .with_source_at("2026-07-29T02:00:00Z")
        .unwrap();
    assert!(ReferenceRateObservation::new(
        identity,
        IsoDate::new("2026-07-29").unwrap(),
        FiniteNumber::new(1.0).unwrap(),
        RatioUnit::Percent,
        None,
        None,
        sourced,
    )
    .is_err());

    let fixing_mismatch = r#"{
      "identity":{"provider":"Fred","base":"USD","quote":"CNY"},
      "fixing_date":"2026-07-29","value":6.8,"quotation_base":1,
      "published_at":null,"revision":null,
      "evidence":{"provider":"Cfets","source_at":null,
        "observed_at":"2026-07-29T03:00:00Z","batch_id":"cfets-1"}
    }"#;
    assert!(serde_json::from_str::<OfficialFxFixing>(fixing_mismatch).is_err());
}

#[test]
fn reference_records_expose_all_source_evidence() {
    let published_at = "2026-07-29T02:00:00Z";
    let observed_at = "2026-07-29T03:00:00Z";
    let evidence = SourceEvidence::new(ProviderId::Cfets, observed_at, "cfets-evidence")
        .unwrap()
        .with_source_at(published_at)
        .unwrap();
    let rate = ReferenceRateObservation::new(
        ReferenceRateIdentity::new(
            ProviderId::Cfets,
            ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
        )
        .unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        FiniteNumber::new(1.0).unwrap(),
        RatioUnit::Percent,
        Some(magic_market_core::NonEmptyText::new(published_at).unwrap()),
        None,
        evidence.clone(),
    )
    .unwrap();
    let fixing = OfficialFxFixing::new(
        CurrencyCode::new("USD").unwrap(),
        CurrencyCode::new("CNY").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        FiniteNumber::new(6.8).unwrap(),
        PositiveU32::new(1).unwrap(),
        Some(magic_market_core::NonEmptyText::new(published_at).unwrap()),
        None,
        evidence,
    )
    .unwrap();

    for record in [&rate as &dyn SourcedRecord, &fixing as &dyn SourcedRecord] {
        assert_eq!(record.provider_id(), ProviderId::Cfets);
        assert_eq!(record.evidence_batch_id(), "cfets-evidence");
        assert_eq!(record.evidence_source_at(), Some(published_at));
        assert_eq!(record.evidence_observed_at(), Some(observed_at));
    }
}
