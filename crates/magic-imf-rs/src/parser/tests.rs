use super::*;

fn key(provider: ProviderId, namespace: &str, code: &str) -> EconomicSeriesKey {
    EconomicSeriesKey::new(provider, namespace, code).unwrap()
}

fn catalog() -> Value {
    serde_json::from_slice(include_bytes!("../../tests/fixtures/indicators.json")).unwrap()
}

fn series() -> Value {
    serde_json::from_slice(include_bytes!("../../tests/fixtures/series.json")).unwrap()
}

fn run(
    catalog: &Value,
    series: &Value,
    key: &EconomicSeriesKey,
    start_year: u32,
    end_year: u32,
) -> Result<DataBatch<EconomicObservation>, ImfError> {
    parse_imf_responses(
        &serde_json::to_vec(catalog).unwrap(),
        &serde_json::to_vec(series).unwrap(),
        &ImfParseContext {
            key,
            start_year,
            end_year,
            observed_at: "2026-07-29T00:00:00Z",
            batch_id: "imf-unit",
        },
    )
}

#[test]
fn namespace_component_year_and_api_helpers_are_closed() {
    let namespace = parse_namespace("WEO/USA").unwrap();
    assert_eq!(namespace.dataset(), "WEO");
    assert_eq!(namespace.area(), "USA");
    for value in ["", "WEO", "WEO/usa", "WEO/USA/EXTRA", "WEO/"] {
        assert!(parse_namespace(value).is_err());
    }
    assert!(valid_component("NGDP_RPCH"));
    assert!(!valid_component(""));
    assert!(!valid_component("lowercase"));
    assert!(!valid_component(&"A".repeat(33)));
    assert_eq!(parse_year("2025").unwrap(), 2025);
    for year in ["", "20x5", "1899", "10000"] {
        assert!(parse_year(year).is_err());
    }
    let valid = serde_json::json!({"api":{"version":"2","output-method":"json"}});
    assert!(validate_api(&valid).is_ok());
    for invalid in [
        serde_json::json!({}),
        serde_json::json!({"api":[]}),
        serde_json::json!({"api":{"version":"1","output-method":"json"}}),
        serde_json::json!({"api":{"version":"2","output-method":"xml"}}),
    ] {
        assert!(validate_api(&invalid).is_err());
    }
    assert!(object_at(&valid, "missing").is_err());
}

#[test]
fn evidence_preserves_the_context_identity() {
    let key = EconomicSeriesKey::new(ProviderId::Imf, "WEO/USA", "GDP").unwrap();
    let context = ImfParseContext {
        key: &key,
        start_year: 2024,
        end_year: 2025,
        observed_at: "observed",
        batch_id: "batch",
    };
    let evidence = evidence(&context).unwrap();
    assert_eq!(evidence.provider(), ProviderId::Imf);
    assert_eq!(evidence.batch_id(), "batch");
}

#[test]
fn request_and_catalog_validation_cover_provider_range_and_required_metadata() {
    let valid = key(ProviderId::Imf, "WEO/USA", "NGDP_RPCH");
    let foreign = key(ProviderId::Fred, "WEO/USA", "NGDP_RPCH");
    assert!(run(&catalog(), &series(), &foreign, 2024, 2026).is_err());
    let unsafe_code = key(ProviderId::Imf, "WEO/USA", "lowercase");
    assert!(run(&catalog(), &series(), &unsafe_code, 2024, 2026).is_err());
    assert!(run(&catalog(), &series(), &valid, 2026, 2024).is_err());
    assert!(run(&catalog(), &series(), &valid, 2024, 2074).is_err());

    for field in ["label", "source", "unit"] {
        let mut invalid = catalog();
        invalid["indicators"]["NGDP_RPCH"][field] = Value::String(" ".into());
        assert!(run(&invalid, &series(), &valid, 2024, 2026).is_err());
    }
}

#[test]
fn response_metadata_and_value_shapes_fail_atomically() {
    let key = key(ProviderId::Imf, "WEO/USA", "NGDP_RPCH");
    let catalog = catalog();

    let mut missing = series();
    missing["indicators"]
        .as_object_mut()
        .unwrap()
        .remove("NGDP_RPCH");
    assert!(run(&catalog, &missing, &key, 2024, 2026).is_err());
    let mut metadata_not_object = series();
    metadata_not_object["indicators"]["NGDP_RPCH"] = serde_json::json!("bad");
    assert!(run(&catalog, &metadata_not_object, &key, 2024, 2026).is_err());
    let mut missing_label = series();
    missing_label["indicators"]["NGDP_RPCH"]
        .as_object_mut()
        .unwrap()
        .remove("label");
    assert!(run(&catalog, &missing_label, &key, 2024, 2026).is_err());
    let mut invalid_extra = series();
    invalid_extra["indicators"]["bad"] = serde_json::json!({"label":"bad"});
    invalid_extra["values"]["bad"] = serde_json::json!({"USA":{"2024":1}});
    assert!(run(&catalog, &invalid_extra, &key, 2024, 2026).is_err());
    let mut identity_drift = series();
    identity_drift["indicators"]["OTHER"] = serde_json::json!({"label":"Other"});
    assert!(run(&catalog, &identity_drift, &key, 2024, 2026).is_err());
    let mut areas_not_object = series();
    areas_not_object["values"]["NGDP_RPCH"] = serde_json::json!([]);
    assert!(run(&catalog, &areas_not_object, &key, 2024, 2026).is_err());
}

#[test]
fn area_sentinel_year_value_and_selection_contracts_are_closed() {
    let key = key(ProviderId::Imf, "WEO/USA", "NGDP_RPCH");
    let catalog = catalog();

    let mut non_null_sentinel = series();
    non_null_sentinel["values"]["NGDP_RPCH"][""] = serde_json::json!({});
    assert!(run(&catalog, &non_null_sentinel, &key, 2024, 2026).is_err());
    let mut invalid_area = series();
    invalid_area["values"]["NGDP_RPCH"]["lowercase"] = serde_json::json!({"2024":1});
    assert!(run(&catalog, &invalid_area, &key, 2024, 2026).is_err());
    let mut nonnumeric = series();
    nonnumeric["values"]["NGDP_RPCH"]["USA"]["2024"] = serde_json::json!("bad");
    assert!(run(&catalog, &nonnumeric, &key, 2024, 2026).is_err());
    let mut no_sentinel = series();
    no_sentinel["values"]["NGDP_RPCH"]
        .as_object_mut()
        .unwrap()
        .remove("");
    assert!(run(&catalog, &no_sentinel, &key, 2024, 2026).is_err());
    let mut absent_area = series();
    let usa = absent_area["values"]["NGDP_RPCH"]
        .as_object_mut()
        .unwrap()
        .remove("USA")
        .unwrap();
    absent_area["values"]["NGDP_RPCH"]["CAN"] = usa;
    assert!(run(&catalog, &absent_area, &key, 2024, 2026).is_err());
}

#[test]
fn missing_last_modified_uses_the_catalog_source_revision_label() {
    let key = key(ProviderId::Imf, "WEO/USA", "NGDP_RPCH");
    let mut catalog = catalog();
    catalog["indicators"]["NGDP_RPCH"]
        .as_object_mut()
        .unwrap()
        .remove("last-modified");
    let batch = run(&catalog, &series(), &key, 2024, 2026).unwrap();
    assert_eq!(batch.records().len(), 3);
    assert!(batch.records().iter().all(|record| {
        record
            .revision()
            .and_then(|revision| revision.label.as_ref())
            .is_some_and(|label| label.as_str() == "World Economic Outlook (April 2026)")
    }));
}
