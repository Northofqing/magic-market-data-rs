use magic_market_core::{
    AssetClass, CoreError, Exchange, FiniteNumber, HttpsUrl, InstrumentId, IsoDate, Money,
    NonEmptyText, PositiveU32, Price, Quantity, Ratio,
};
#[test]
fn rejects_invalid_financial_values() {
    assert!(matches!(
        Price::new(0.0),
        Err(CoreError::InvalidValue { field: "price", .. })
    ));
    assert!(Price::new(f64::NAN).is_err());
    assert!(Quantity::new(-1.0).is_err());
    assert!(Money::new(f64::INFINITY).is_err());
    assert!(Ratio::decimal(f64::NAN).is_err());
}
#[test]
fn instrument_code_is_trimmed_but_never_empty() {
    let id = InstrumentId::new(Exchange::Shanghai, " 600000 ", AssetClass::Equity).unwrap();
    assert_eq!(id.code(), "600000");
    assert!(InstrumentId::new(Exchange::Shenzhen, "   ", AssetClass::Equity).is_err());
    assert!(InstrumentId::new(Exchange::Shenzhen, "00\n001", AssetClass::Equity).is_err());
}

#[test]
fn intelligence_primitives_are_checked_and_serde_safe() {
    let text = NonEmptyText::new(" 华电辽能 ").unwrap();
    let url = HttpsUrl::new("https://example.com/report.pdf").unwrap();
    let date = IsoDate::new("2024-02-29").unwrap();
    let number = FiniteNumber::new(-1.25).unwrap();
    let rank = PositiveU32::new(1).unwrap();

    assert_eq!(text.as_str(), "华电辽能");
    assert_eq!(text.to_string(), "华电辽能");
    assert_eq!(
        NonEmptyText::new(" source value ").unwrap().into_string(),
        "source value"
    );
    assert_eq!(url.as_str(), "https://example.com/report.pdf");
    assert_eq!(url.to_string(), "https://example.com/report.pdf");
    assert_eq!(date.as_str(), "2024-02-29");
    assert_eq!(number.get(), -1.25);
    assert_eq!(rank.get(), 1);

    for json in [
        serde_json::to_string(&text).unwrap(),
        serde_json::to_string(&url).unwrap(),
        serde_json::to_string(&date).unwrap(),
        serde_json::to_string(&number).unwrap(),
        serde_json::to_string(&rank).unwrap(),
    ] {
        assert!(!json.is_empty());
    }

    assert!(NonEmptyText::new(" ").is_err());
    assert!(NonEmptyText::new("bad\ntext").is_err());
    assert!(NonEmptyText::new("x".repeat(16_385)).is_err());
    assert!(HttpsUrl::new("http://example.com").is_err());
    assert!(HttpsUrl::new("https://").is_err());
    assert!(HttpsUrl::new(format!("https://example.com/{}", "x".repeat(4_096))).is_err());
    assert!(HttpsUrl::new("https://example.com/bad path").is_err());
    assert!(HttpsUrl::new(r"https://example.com\bad").is_err());
    assert!(IsoDate::new("2026-02-29").is_err());
    assert!(IsoDate::new("2024-02-29").is_ok());
    assert!(FiniteNumber::new(f64::NAN).is_err());
    assert!(PositiveU32::new(0).is_err());

    assert!(serde_json::from_str::<NonEmptyText>(r#"" ""#).is_err());
    assert!(serde_json::from_str::<HttpsUrl>(r#""http://example.com""#).is_err());
    assert!(serde_json::from_str::<IsoDate>(r#""2026-02-29""#).is_err());
    assert!(serde_json::from_str::<FiniteNumber>("null").is_err());
    assert!(serde_json::from_str::<PositiveU32>("0").is_err());
}
