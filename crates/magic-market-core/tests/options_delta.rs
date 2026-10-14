use magic_market_core::{
    CoreError, FiniteNumber, NonEmptyText, OptionGreeks, OptionGreeksInput, ProviderId,
    SourceEvidence,
};
use serde_json::json;

fn greek_delta_input(delta: Option<f64>) -> OptionGreeksInput {
    OptionGreeksInput {
        contract_code: NonEmptyText::new("10000002").unwrap(),
        name: None,
        volume: None,
        delta: delta.map(|value| FiniteNumber::new(value).unwrap()),
        gamma: None,
        theta: None,
        vega: None,
        rho: None,
        implied_volatility: None,
        high: None,
        low: None,
        trade_code: None,
        strike: None,
        last: None,
        theoretical_price: None,
        evidence: SourceEvidence::new(
            ProviderId::Sina,
            "2026-08-26T14:30:01+08:00",
            "option-delta-boundary-fixture",
        )
        .unwrap(),
    }
}

#[test]
fn option_greeks_preserve_optional_signed_delta_and_enforce_the_closed_unit_interval() {
    let lower = OptionGreeks::new(greek_delta_input(Some(-1.0))).unwrap();
    assert_eq!(lower.delta().unwrap().get(), -1.0);
    let lower_wire = serde_json::to_value(&lower).unwrap();
    assert_eq!(lower_wire["delta"], json!(-1.0));
    let lower_decoded = serde_json::from_value::<OptionGreeks>(lower_wire.clone()).unwrap();
    assert_eq!(lower_decoded.delta().unwrap().get(), -1.0);

    let put = OptionGreeks::new(greek_delta_input(Some(-0.625))).unwrap();
    assert_eq!(put.delta().unwrap().get(), -0.625);
    let put_wire = serde_json::to_value(&put).unwrap();
    assert_eq!(put_wire["delta"], json!(-0.625));
    let put_decoded = serde_json::from_value::<OptionGreeks>(put_wire).unwrap();
    assert_eq!(put_decoded.delta().unwrap().get(), -0.625);

    let upper = OptionGreeks::new(greek_delta_input(Some(1.0))).unwrap();
    assert_eq!(upper.delta().unwrap().get(), 1.0);
    let upper_wire = serde_json::to_value(&upper).unwrap();
    assert_eq!(upper_wire["delta"], json!(1.0));
    let upper_decoded = serde_json::from_value::<OptionGreeks>(upper_wire).unwrap();
    assert_eq!(upper_decoded.delta().unwrap().get(), 1.0);

    let unavailable = OptionGreeks::new(greek_delta_input(None)).unwrap();
    assert!(unavailable.delta().is_none());
    let unavailable_wire = serde_json::to_value(&unavailable).unwrap();
    assert_eq!(unavailable_wire["delta"], json!(null));
    let unavailable_decoded = serde_json::from_value::<OptionGreeks>(unavailable_wire).unwrap();
    assert!(unavailable_decoded.delta().is_none());

    // These finite literals are the neighboring f64 values outside [-1, 1].
    for invalid_delta in [-1.0000000000000002, 1.0000000000000002] {
        assert_eq!(
            OptionGreeks::new(greek_delta_input(Some(invalid_delta))).unwrap_err(),
            CoreError::InvalidRequest("option delta must be between -1 and 1".into()),
        );

        let mut invalid_wire = lower_wire.clone();
        invalid_wire["delta"] = json!(invalid_delta);
        let error = serde_json::from_value::<OptionGreeks>(invalid_wire).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("option delta must be between -1 and 1"),
            "{error}"
        );
    }
}
