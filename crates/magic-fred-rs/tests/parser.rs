use magic_fred_rs::{parse_fred_responses, FredError, FredParseContext};
use magic_market_core::{
    EconomicFrequency, EconomicObservationStatus, EconomicPeriod, EconomicSeriesKey, ProviderId,
};
use std::sync::LazyLock;

static START: LazyLock<EconomicPeriod> =
    LazyLock::new(|| EconomicPeriod::quarter(2025, 1).unwrap());
static END: LazyLock<EconomicPeriod> = LazyLock::new(|| EconomicPeriod::quarter(2025, 4).unwrap());

fn context<'a>(key: &'a EconomicSeriesKey) -> FredParseContext<'a> {
    FredParseContext {
        key,
        frequency: EconomicFrequency::Quarterly,
        start: &START,
        end: &END,
        query_start: "2025-01-01",
        query_end: "2025-12-31",
        observed_at: "2026-07-29T00:00:00Z",
        batch_id: "fred:test",
    }
}

#[test]
fn parses_metadata_missing_and_zero_without_guessing() {
    let key = EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap();
    let batch = parse_fred_responses(
        include_bytes!("fixtures/series.json"),
        include_bytes!("fixtures/observations.json"),
        &context(&key),
    )
    .unwrap();
    assert_eq!(batch.records().len(), 4);
    assert_eq!(batch.records()[0].series().namespace(), "fred");
    assert_eq!(batch.records()[0].unit(), "Billions of Dollars");
    assert!(matches!(
        batch.records()[1].status(),
        EconomicObservationStatus::Missing
    ));
    assert_eq!(batch.records()[1].value(), None);
    assert_eq!(batch.records()[2].value().unwrap().get(), 0.0);
}

#[test]
fn rejects_wrong_identity_bad_quarter_nonfinite_and_duplicate_keys() {
    let wrong = EconomicSeriesKey::new(ProviderId::Fred, "fred", "CPI").unwrap();
    assert!(matches!(
        parse_fred_responses(
            include_bytes!("fixtures/series.json"),
            include_bytes!("fixtures/observations.json"),
            &context(&wrong)
        ),
        Err(FredError::Protocol(_))
    ));
    let key = EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap();
    let bad_date = include_str!("fixtures/observations.json").replace("2025-04-01", "2025-05-01");
    assert!(matches!(
        parse_fred_responses(
            include_bytes!("fixtures/series.json"),
            bad_date.as_bytes(),
            &context(&key)
        ),
        Err(FredError::Protocol(_))
    ));
    let bad_value = include_str!("fixtures/observations.json").replace("\"30142.8\"", "\"NaN\"");
    assert!(parse_fred_responses(
        include_bytes!("fixtures/series.json"),
        bad_value.as_bytes(),
        &context(&key)
    )
    .is_err());
    let duplicate = br#"{"seriess":[],"seriess":[]}"#;
    assert!(matches!(
        parse_fred_responses(
            duplicate,
            include_bytes!("fixtures/observations.json"),
            &context(&key)
        ),
        Err(FredError::Decode(_))
    ));
}

#[test]
fn authentication_envelope_is_typed_and_contains_no_secret() {
    let key = EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap();
    let error = parse_fred_responses(
        br#"{"error_code":400,"error_message":"Bad Request. api_key=secret-key-value"}"#,
        include_bytes!("fixtures/observations.json"),
        &context(&key),
    )
    .unwrap_err();
    assert!(matches!(error, FredError::Authentication(_)));
    assert!(!error.to_string().contains("secret-key-value"));
}

#[test]
fn requires_one_complete_page_with_no_remaining_rows() {
    let key = EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap();
    let remaining =
        include_str!("fixtures/observations.json").replace("\"count\":4", "\"count\":5");
    assert!(matches!(
        parse_fred_responses(
            include_bytes!("fixtures/series.json"),
            remaining.as_bytes(),
            &context(&key)
        ),
        Err(FredError::Protocol(_))
    ));
    let nonzero_offset =
        include_str!("fixtures/observations.json").replace("\"offset\":0", "\"offset\":1");
    assert!(matches!(
        parse_fred_responses(
            include_bytes!("fixtures/series.json"),
            nonzero_offset.as_bytes(),
            &context(&key)
        ),
        Err(FredError::Protocol(_))
    ));
    let altered_limit =
        include_str!("fixtures/observations.json").replace("\"limit\":100000", "\"limit\":99999");
    assert!(matches!(
        parse_fred_responses(
            include_bytes!("fixtures/series.json"),
            altered_limit.as_bytes(),
            &context(&key)
        ),
        Err(FredError::Protocol(_))
    ));
    let contradictory_vintage = include_str!("fixtures/observations.json").replacen(
        "\"realtime_end\":\"2026-07-29\"",
        "\"realtime_end\":\"2026-07-30\"",
        1,
    );
    assert!(matches!(
        parse_fred_responses(
            include_bytes!("fixtures/series.json"),
            contradictory_vintage.as_bytes(),
            &context(&key)
        ),
        Err(FredError::Protocol(_))
    ));
}

#[test]
fn rejects_narrowed_echoes_and_rows_outside_series_metadata() {
    let key = EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP").unwrap();
    let mut narrowed: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/observations.json")).unwrap();
    narrowed["observation_start"] = serde_json::json!("2025-04-01");
    let narrowed = serde_json::to_vec(&narrowed).unwrap();
    assert!(matches!(
        parse_fred_responses(
            include_bytes!("fixtures/series.json"),
            &narrowed,
            &context(&key)
        ),
        Err(FredError::Protocol(_))
    ));

    let metadata_narrowed =
        include_str!("fixtures/series.json").replace("1947-01-01", "2025-04-01");
    assert!(matches!(
        parse_fred_responses(
            metadata_narrowed.as_bytes(),
            include_bytes!("fixtures/observations.json"),
            &context(&key)
        ),
        Err(FredError::Protocol(_))
    ));
}
