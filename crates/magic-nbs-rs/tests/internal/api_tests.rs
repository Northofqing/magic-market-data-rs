use super::*;
use magic_market_core::{EconomicSeriesKey, PositiveU32};

fn request() -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Nbs, "national-cpi-yoy", "headline").unwrap()],
        EconomicPeriod::month(2026, 7).unwrap(),
        EconomicPeriod::month(2026, 7).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap()
}

#[test]
fn admitted_scope_is_exact() {
    assert!(validate_admitted_request(&request()).is_ok());
    let regional = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Nbs, "beijing-cpi-yoy", "headline").unwrap()],
        EconomicPeriod::month(2026, 7).unwrap(),
        EconomicPeriod::month(2026, 7).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        validate_admitted_request(&regional).unwrap(),
        AdmittedScope::Beijing
    );
    let wrong = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Nbs, "national-cpi-yoy", "headline").unwrap()],
        EconomicPeriod::month(2026, 6).unwrap(),
        EconomicPeriod::month(2026, 6).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        validate_admitted_request(&wrong),
        Err(NbsError::Unsupported(_))
    ));
}

#[test]
fn normalized_identity_is_strict_and_keeps_source_time_absent() {
    let indicator = Indicator {
        id: "53180dfb9c14411ba4b762307c85920c".into(),
        i_showname: format!("{CPI_HEADLINE} "),
        du_name: "%".into(),
        catalogid: "5c7452825c7c4dcba391db5ca7f335c5".into(),
    };
    let result = SeriesResult {
        code: "202607MM".into(),
        name: "2026年7月".into(),
        values: vec![SeriesValue {
            id: indicator.id.clone(),
            i_showname: indicator.i_showname.clone(),
            du_name: "%".into(),
            catalogid: indicator.catalogid.clone(),
            value: "100.5".into(),
            da: NATIONAL_CODE.into(),
            da_name: "全国".into(),
        }],
    };
    let batch = normalize_response(
        &request(),
        "2026-08-13T00:00:00Z",
        &indicator.catalogid,
        &indicator,
        vec![result],
        AdmittedScope::National,
    )
    .unwrap();
    assert_eq!(batch.records()[0].value().unwrap().get(), 100.5);
    assert_eq!(batch.records()[0].unit(), "%");
    assert!(batch.records()[0].evidence().source_at().is_none());
}
