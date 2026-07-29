use super::*;
use magic_market_core::PositiveU32;

fn request(provider: ProviderId, kind: ReferenceRateKind) -> ReferenceRateRequest {
    ReferenceRateRequest::new(
        vec![ReferenceRateIdentity::new(provider, kind).unwrap()],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap()
}

fn shibor() -> ReferenceRateRequest {
    request(
        ProviderId::Cfets,
        ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
    )
}

fn rejected(body: String) {
    assert!(parse_shibor_payload(body.as_bytes(), &shibor(), "observed", "batch").is_err());
}

#[test]
fn parser_preflight_size_family_and_envelope_are_strict() {
    assert!(parse_shibor_payload(
        &vec![b'x'; MAX_RESPONSE_BYTES + 1],
        &shibor(),
        "observed",
        "batch"
    )
    .is_err());
    let foreign = request(
        ProviderId::Fred,
        ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
    );
    assert!(parse_shibor_payload(b"{}", &foreign, "observed", "batch").is_err());
    let lpr = request(
        ProviderId::Cfets,
        ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OneYear),
    );
    assert!(validate_requested_family(&lpr, RateFamily::Shibor).is_err());
    rejected("{".into());
    let fixture = include_str!("../../tests/fixtures/shibor.json");
    rejected(fixture.replace(
        "\"startDateCN\":\"2026-07-28\"",
        "\"startDateCN\":\"2026-07-27\"",
    ));
    rejected(fixture.replace("\"records\":[", "\"records\":[] , \"unused\":["));
    rejected(fixture.replace("{\"showDateCN\":\"2026-07-29\"", "\"not-an-object\""));
    rejected(fixture.replace(
        "\"showDateCN\":\"2026-07-29\"",
        "\"missing\":\"2026-07-29\"",
    ));
    rejected(fixture.replace(
        "\"showDateCN\":\"2026-07-29\"",
        "\"showDateCN\":\"2026-07-27\"",
    ));
    rejected(fixture.replace("\"ON\":\"1.4150\"", "\"missing\":\"1.4150\""));
    rejected(fixture.replace("\"ON\":\"1.4150\"", "\"ON\":\" \""));
    rejected(fixture.replace("\"ON\":\"1.4150\"", "\"ON\":\"bad\""));
    rejected(fixture.replace("\"ON\":\"1.4150\"", "\"ON\":\"NaN\""));
}

#[test]
fn exact_shibor_metadata_and_truncation_paths_are_covered() {
    let fixture = include_str!("../../tests/fixtures/shibor.json");
    rejected(fixture.replace("{\"cfgItem\":\"1Y\",\"cfgItemNm\":\"1Y\",\"sqncCd\":8}", ""));
    rejected(fixture.replace("\"cfgItemNm\":\"ON\"", "\"cfgItemNm\":\"WRONG\""));
    rejected(fixture.replace("\"sqncCd\":1", "\"sqncCd\":2"));
    let request = ReferenceRateRequest::new(
        vec![
            ReferenceRateIdentity::new(
                ProviderId::Cfets,
                ReferenceRateKind::Shibor(ReferenceTenor::OneYear),
            )
            .unwrap(),
            ReferenceRateIdentity::new(
                ProviderId::Cfets,
                ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
            )
            .unwrap(),
        ],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let batch = parse_shibor_payload(fixture.as_bytes(), &request, "observed", "batch").unwrap();
    assert_eq!(batch.records().len(), 1);
    assert_eq!(
        batch.records()[0].identity().kind(),
        &ReferenceRateKind::Shibor(ReferenceTenor::OneYear)
    );
}

#[test]
fn typed_curve_catalogs_reject_wrong_shapes_lengths_and_headings() {
    let mut shibor_value: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/shibor.json")).unwrap();
    shibor_value["data"]["baseCurveCfgList"] = serde_json::Value::String("wrong".into());
    rejected(serde_json::to_string(&shibor_value).unwrap());

    let mut shibor_value: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/shibor.json")).unwrap();
    shibor_value["data"]["baseCurveCfgList"]
        .as_array_mut()
        .unwrap()
        .pop();
    rejected(serde_json::to_string(&shibor_value).unwrap());

    let lpr_request = ReferenceRateRequest::new(
        vec![ReferenceRateIdentity::new(
            ProviderId::Cfets,
            ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OneYear),
        )
        .unwrap()],
        IsoDate::new("2026-07-01").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    let mut lpr_value: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/lpr.json")).unwrap();
    lpr_value["data"]["baseCurveCfgList"] = serde_json::json!(["1Y", "WRONG"]);
    assert!(parse_lpr_payload(
        &serde_json::to_vec(&lpr_value).unwrap(),
        &lpr_request,
        "observed",
        "batch"
    )
    .is_err());
    assert_eq!(
        parse_lpr_payload(
            include_bytes!("../../tests/fixtures/lpr.json"),
            &lpr_request,
            "observed",
            "batch"
        )
        .unwrap()
        .records()
        .len(),
        1
    );
}
