use magic_market_core::{
    EconomicObservationStatus, EconomicPeriod, EconomicRevisionKind, EconomicSeriesKey,
    EconomicSeriesRequest, PositiveU32, ProviderId,
};
use magic_pbc_rs::{parse_regional_social_financing_workbook, REGIONAL_SOCIAL_FINANCING_CODES};

const FIXTURE: &[u8] = include_bytes!("fixtures/regional-social-financing-2025q1.xlsx");

fn request(codes: &[&str], max_rows: u32) -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        codes
            .iter()
            .map(|code| {
                EconomicSeriesKey::new(ProviderId::Pbc, "regional-social-financing-flow", *code)
                    .unwrap()
            })
            .collect(),
        EconomicPeriod::quarter(2025, 1).unwrap(),
        EconomicPeriod::quarter(2025, 1).unwrap(),
        PositiveU32::new(max_rows).unwrap(),
    )
    .unwrap()
}

#[test]
fn official_workbook_preserves_nine_families_regions_unit_and_preliminary_status() {
    let batch = parse_regional_social_financing_workbook(
        FIXTURE,
        &request(&REGIONAL_SOCIAL_FINANCING_CODES, 279),
        "2026-08-13T00:00:00Z",
        "fixture",
    )
    .unwrap();
    assert_eq!(batch.records().len(), 31 * 9);
    assert!(batch.records().iter().all(|row| {
        row.region_code().is_none()
            && row.region_name().is_some()
            && row.unit() == "亿元人民币"
            && row.scale() == Some("100 million yuan")
            && row.status() == &EconomicObservationStatus::Present
            && row.revision().map(|revision| &revision.kind)
                == Some(&EconomicRevisionKind::Preliminary)
    }));

    let beijing_total = batch.records().iter().find(|row| {
        row.series().code() == "AFRE_FLOW" && row.region_name() == Some("北京 Beijing")
    });
    assert_eq!(beijing_total.unwrap().value().unwrap().get(), 8_426.0);
    let tianjin_equity = batch.records().iter().find(|row| {
        row.series().code() == "DOMESTIC_EQUITY_FINANCING"
            && row.region_name() == Some("天津 Tianjin")
    });
    assert_eq!(tianjin_equity.unwrap().value().unwrap().get(), 0.0);
    let shaanxi_name = batch
        .records()
        .iter()
        .find(|row| row.region_name() == Some("陕西 Shanxi"));
    assert!(
        shaanxi_name.is_some(),
        "source spelling must not be corrected by inference"
    );
}

#[test]
fn requests_must_preserve_complete_region_coverage_and_exact_catalog() {
    assert!(parse_regional_social_financing_workbook(
        FIXTURE,
        &request(&["AFRE_FLOW"], 30),
        "observed",
        "batch",
    )
    .is_err());
    let wrong_period = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::Pbc,
            "regional-social-financing-flow",
            "AFRE_FLOW",
        )
        .unwrap()],
        EconomicPeriod::quarter(2025, 2).unwrap(),
        EconomicPeriod::quarter(2025, 2).unwrap(),
        PositiveU32::new(31).unwrap(),
    )
    .unwrap();
    assert!(
        parse_regional_social_financing_workbook(FIXTURE, &wrong_period, "observed", "batch",)
            .is_err()
    );
}

#[test]
fn malformed_truncated_and_oversized_workbooks_fail_closed() {
    let valid_request = request(&["AFRE_FLOW"], 31);
    assert!(parse_regional_social_financing_workbook(
        b"not an xlsx",
        &valid_request,
        "observed",
        "batch",
    )
    .is_err());
    assert!(parse_regional_social_financing_workbook(
        &FIXTURE[..FIXTURE.len() / 2],
        &valid_request,
        "observed",
        "batch",
    )
    .is_err());
    assert!(parse_regional_social_financing_workbook(
        &vec![0_u8; 256 * 1024 + 1],
        &valid_request,
        "observed",
        "batch",
    )
    .is_err());

    let mut declared_zip_bomb = FIXTURE.to_vec();
    let central = declared_zip_bomb
        .windows(4)
        .position(|bytes| bytes == [0x50, 0x4b, 0x01, 0x02])
        .unwrap();
    declared_zip_bomb[central + 24..central + 28]
        .copy_from_slice(&(4_u32 * 1024 * 1024).to_le_bytes());
    assert!(parse_regional_social_financing_workbook(
        &declared_zip_bomb,
        &valid_request,
        "observed",
        "batch",
    )
    .is_err());
}
