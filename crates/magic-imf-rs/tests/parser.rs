use magic_imf_rs::{parse_imf_responses, parse_namespace, ImfError, ImfParseContext};
use magic_market_core::{EconomicSeriesKey, ProviderId};

#[test]
fn namespace_requires_dataset_and_area() {
    assert!(parse_namespace("WEO/USA").is_ok());
    assert!(parse_namespace("WEO").is_err());
    assert!(parse_namespace("WEO/USA/CHN").is_err());
    assert!(parse_namespace("../USA").is_err());
}

#[test]
fn validates_full_superset_then_filters_requested_area_and_years() {
    let key = EconomicSeriesKey::new(ProviderId::Imf, "WEO/USA", "NGDP_RPCH").unwrap();
    let batch = parse_imf_responses(
        include_bytes!("fixtures/indicators.json"),
        include_bytes!("fixtures/series.json"),
        &ImfParseContext {
            key: &key,
            start_year: 2024,
            end_year: 2026,
            observed_at: "2026-07-29T00:00:00Z",
            batch_id: "imf:test",
        },
    )
    .unwrap();
    assert_eq!(batch.records().len(), 3);
    assert_eq!(batch.records()[1].value().unwrap().get(), 0.0);
    assert_eq!(batch.records()[2].value().unwrap().get(), -1.0);
    assert!(batch.records()[2].revision().is_some());
    assert_eq!(batch.records()[0].released_at(), None);
    assert_eq!(batch.records()[0].evidence().source_at(), None);
    assert_eq!(batch.provenance().source_at(), None);
    assert!(batch.records()[0]
        .revision()
        .unwrap()
        .label
        .as_ref()
        .unwrap()
        .as_str()
        .contains("last-modified=2026-04-08 16:07:34"));
}

#[test]
fn malformed_unrequested_superset_still_fails() {
    let key = EconomicSeriesKey::new(ProviderId::Imf, "WEO/USA", "NGDP_RPCH").unwrap();
    let malformed = include_str!("fixtures/series.json")
        .replace("\"CHN\":{\"2024\":5.0,\"2025\":4.5}", "\"CHN\":null");
    assert!(matches!(
        parse_imf_responses(
            include_bytes!("fixtures/indicators.json"),
            malformed.as_bytes(),
            &ImfParseContext {
                key: &key,
                start_year: 2024,
                end_year: 2025,
                observed_at: "2026-07-29T00:00:00Z",
                batch_id: "imf:test",
            },
        ),
        Err(ImfError::Protocol(_))
    ));
}

#[test]
fn duplicate_keys_wrong_dataset_and_invalid_api_fail() {
    let key = EconomicSeriesKey::new(ProviderId::Imf, "WEO/USA", "NGDP_RPCH").unwrap();
    let context = ImfParseContext {
        key: &key,
        start_year: 2024,
        end_year: 2025,
        observed_at: "2026-07-29T00:00:00Z",
        batch_id: "imf:test",
    };
    let wrong = include_str!("fixtures/indicators.json").replace("\"WEO\"", "\"IFS\"");
    assert!(parse_imf_responses(
        wrong.as_bytes(),
        include_bytes!("fixtures/series.json"),
        &context
    )
    .is_err());
    let bad_api = include_str!("fixtures/series.json").replace("\"2\"", "\"1\"");
    assert!(parse_imf_responses(
        include_bytes!("fixtures/indicators.json"),
        bad_api.as_bytes(),
        &context
    )
    .is_err());
    assert!(matches!(
        parse_imf_responses(
            br#"{"indicators":{},"indicators":{}}"#,
            include_bytes!("fixtures/series.json"),
            &context
        ),
        Err(ImfError::Decode(_))
    ));
}

#[test]
fn indicator_metadata_and_single_envelope_sentinel_are_strict() {
    let key = EconomicSeriesKey::new(ProviderId::Imf, "WEO/USA", "NGDP_RPCH").unwrap();
    let context = ImfParseContext {
        key: &key,
        start_year: 2024,
        end_year: 2025,
        observed_at: "2026-07-29T00:00:00Z",
        batch_id: "imf:test",
    };
    let wrong_label =
        include_str!("fixtures/series.json").replace("Real GDP growth", "Conflicting label");
    assert!(matches!(
        parse_imf_responses(
            include_bytes!("fixtures/indicators.json"),
            wrong_label.as_bytes(),
            &context
        ),
        Err(ImfError::Protocol(_))
    ));

    let mut duplicate_sentinel: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/series.json")).unwrap();
    duplicate_sentinel["indicators"]["OTHER"] = serde_json::json!({"label":"Other"});
    duplicate_sentinel["values"]["OTHER"] = serde_json::json!({"":null});
    let duplicate_sentinel = serde_json::to_vec(&duplicate_sentinel).unwrap();
    assert!(matches!(
        parse_imf_responses(
            include_bytes!("fixtures/indicators.json"),
            &duplicate_sentinel,
            &context
        ),
        Err(ImfError::Protocol(_))
    ));

    let mut no_sentinel: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/series.json")).unwrap();
    no_sentinel["values"]["NGDP_RPCH"]
        .as_object_mut()
        .unwrap()
        .remove("");
    let no_sentinel = serde_json::to_vec(&no_sentinel).unwrap();
    assert!(matches!(
        parse_imf_responses(
            include_bytes!("fixtures/indicators.json"),
            &no_sentinel,
            &context
        ),
        Err(ImfError::Protocol(_))
    ));
}
