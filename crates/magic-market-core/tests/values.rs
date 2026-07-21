use magic_market_core::{AssetClass, CoreError, Exchange, InstrumentId, Money, Price, Quantity, Ratio};
#[test]
fn rejects_invalid_financial_values() {
    assert!(matches!(Price::new(0.0), Err(CoreError::InvalidValue { field: "price", .. })));
    assert!(Price::new(f64::NAN).is_err()); assert!(Quantity::new(-1.0).is_err());
    assert!(Money::new(f64::INFINITY).is_err()); assert!(Ratio::decimal(f64::NAN).is_err());
}
#[test]
fn instrument_code_is_trimmed_but_never_empty() {
    let id = InstrumentId::new(Exchange::Shanghai, " 600000 ", AssetClass::Equity).unwrap();
    assert_eq!(id.code(), "600000");
    assert!(InstrumentId::new(Exchange::Shenzhen, "   ", AssetClass::Equity).is_err());
}
