use super::*;

fn key(provider: ProviderId, namespace: &str) -> EconomicSeriesKey {
    EconomicSeriesKey::new(provider, namespace, "GDP").unwrap()
}

fn metadata() -> Value {
    serde_json::from_slice(include_bytes!("../../tests/fixtures/series.json")).unwrap()
}

fn observations() -> Value {
    serde_json::from_slice(include_bytes!("../../tests/fixtures/observations.json")).unwrap()
}

fn run(
    metadata: &Value,
    observations: &Value,
    key: &EconomicSeriesKey,
    start: &EconomicPeriod,
    end: &EconomicPeriod,
) -> Result<DataBatch<EconomicObservation>, FredError> {
    parse_fred_responses(
        &serde_json::to_vec(metadata).unwrap(),
        &serde_json::to_vec(observations).unwrap(),
        &FredParseContext {
            key,
            frequency: EconomicFrequency::Quarterly,
            start,
            end,
            query_start: "2025-01-01",
            query_end: "2025-12-31",
            observed_at: "2026-07-29T00:00:00Z",
            batch_id: "fred-unit",
        },
    )
}

#[test]
fn frequency_period_and_date_helpers_cover_every_shape() {
    for (long, short, expected) in [
        ("Daily", "D", EconomicFrequency::Daily),
        ("Weekly", "W", EconomicFrequency::Weekly),
        ("Monthly", "M", EconomicFrequency::Monthly),
        ("Quarterly", "Q", EconomicFrequency::Quarterly),
        ("Annual", "A", EconomicFrequency::Annual),
    ] {
        assert_eq!(parse_frequency(long, short).unwrap(), expected);
    }
    assert!(parse_frequency("Daily", "M").is_err());
    for (date, frequency, expected) in [
        (
            "2025-01-02",
            EconomicFrequency::Daily,
            EconomicPeriod::day("2025-01-02").unwrap(),
        ),
        (
            "2025-01-06",
            EconomicFrequency::Weekly,
            EconomicPeriod::iso_week(2025, 2).unwrap(),
        ),
        (
            "2025-02-01",
            EconomicFrequency::Monthly,
            EconomicPeriod::month(2025, 2).unwrap(),
        ),
        (
            "2025-04-01",
            EconomicFrequency::Quarterly,
            EconomicPeriod::quarter(2025, 2).unwrap(),
        ),
        (
            "2025-01-01",
            EconomicFrequency::Annual,
            EconomicPeriod::year(2025).unwrap(),
        ),
    ] {
        assert_eq!(parse_period(date, frequency).unwrap(), expected);
    }
    for (date, frequency) in [
        ("2025-02-02", EconomicFrequency::Monthly),
        ("2025-02-01", EconomicFrequency::Quarterly),
        ("2025-02-01", EconomicFrequency::Annual),
        ("2025-13-01", EconomicFrequency::Weekly),
        ("2025-01-01", EconomicFrequency::Irregular),
    ] {
        assert!(parse_period(date, frequency).is_err());
    }
    assert_eq!(parse_date_parts("2024-02-29").unwrap(), (2024, 2, 29));
    for date in ["", "2024/02/29", "xxxx-02-29", "2024-13-01"] {
        assert!(parse_date_parts(date).is_err());
    }
}

#[test]
fn timestamp_and_api_error_helpers_are_exact() {
    assert_eq!(
        parse_fred_timestamp("2026-07-01 12:34:56-05").unwrap(),
        "2026-07-01T12:34:56-05:00"
    );
    assert_eq!(
        parse_fred_timestamp("2026-07-01 12:34:56+08").unwrap(),
        "2026-07-01T12:34:56+08:00"
    );
    for value in [
        "",
        "2026/07/01 12:34:56+08",
        "2026-13-01 12:34:56+08",
        "2026-07-32 12:34:56+08",
        "2026-07-01 25:34:56+08",
        "2026-07-01 12:34:56+99",
    ] {
        assert!(parse_fred_timestamp(value).is_err(), "{value}");
    }
    assert!(reject_api_error(br#"{"seriess":[]}"#).is_ok());
    assert!(matches!(
        reject_api_error(br#"{"error_code":400,"error_message":"secret"}"#),
        Err(FredError::Authentication(_))
    ));
    assert!(reject_api_error(b"{").is_err());
    assert!(ensure_no_duplicate_keys(br#"{"a":1,"a":2}"#).is_err());
}

#[test]
fn metadata_contract_rejects_namespace_cardinality_range_frequency_and_labels() {
    let start = EconomicPeriod::quarter(2025, 1).unwrap();
    let end = EconomicPeriod::quarter(2025, 4).unwrap();
    for key in [key(ProviderId::Imf, "fred"), key(ProviderId::Fred, "other")] {
        assert!(run(&metadata(), &observations(), &key, &start, &end).is_err());
    }
    let key = key(ProviderId::Fred, "fred");
    let mut empty = metadata();
    empty["seriess"] = serde_json::json!([]);
    assert!(run(&empty, &observations(), &key, &start, &end).is_err());
    let mut reversed = metadata();
    reversed["seriess"][0]["observation_start"] = Value::String("2027-01-01".into());
    assert!(run(&reversed, &observations(), &key, &start, &end).is_err());
    let mut frequency = metadata();
    frequency["seriess"][0]["frequency"] = Value::String("Daily".into());
    assert!(run(&frequency, &observations(), &key, &start, &end).is_err());
    let mut mismatched_frequency = metadata();
    mismatched_frequency["seriess"][0]["frequency"] = Value::String("Daily".into());
    mismatched_frequency["seriess"][0]["frequency_short"] = Value::String("D".into());
    assert!(run(&mismatched_frequency, &observations(), &key, &start, &end).is_err());
    for field in ["units", "seasonal_adjustment"] {
        let mut blank = metadata();
        blank["seriess"][0][field] = Value::String(" ".into());
        assert!(run(&blank, &observations(), &key, &start, &end).is_err());
    }
}

#[test]
fn observation_realtime_duplicate_order_and_projection_paths_are_explicit() {
    let key = key(ProviderId::Fred, "fred");
    let start = EconomicPeriod::quarter(2025, 1).unwrap();
    let end = EconomicPeriod::quarter(2025, 4).unwrap();

    let mut realtime = observations();
    realtime["observations"][0]["realtime_end"] = Value::String("2026-07-28".into());
    assert!(run(&metadata(), &realtime, &key, &start, &end).is_err());
    let mut duplicate = observations();
    duplicate["observations"][1]["date"] = duplicate["observations"][0]["date"].clone();
    assert!(run(&metadata(), &duplicate, &key, &start, &end).is_err());
    let mut unordered = observations();
    unordered["observations"].as_array_mut().unwrap().swap(0, 1);
    assert!(run(&metadata(), &unordered, &key, &start, &end).is_err());

    let projected_start = EconomicPeriod::quarter(2025, 2).unwrap();
    let projected_end = EconomicPeriod::quarter(2025, 3).unwrap();
    let batch = run(
        &metadata(),
        &observations(),
        &key,
        &projected_start,
        &projected_end,
    )
    .unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(
        batch.records()[0].period(),
        &EconomicPeriod::quarter(2025, 2).unwrap()
    );
}
