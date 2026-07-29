use magic_market_core::{EconomicObservationStatus, EconomicSeriesKey, ProviderId};
use magic_worldbank_rs::{
    parse_world_bank_namespace, parse_world_bank_responses, WorldBankError, WorldBankParseContext,
};

fn context<'a>(key: &'a EconomicSeriesKey) -> WorldBankParseContext<'a> {
    WorldBankParseContext {
        key,
        start_year: 2022,
        end_year: 2024,
        observed_at: "2026-07-29T00:00:00Z",
        batch_id: "worldbank:test",
    }
}

#[test]
fn namespace_is_exact_and_unsafe_separators_fail() {
    let parsed = parse_world_bank_namespace("source:2/country:USA").unwrap();
    assert_eq!(parsed.source_id(), "2");
    assert_eq!(parsed.economy(), "USA");
    assert!(parse_world_bank_namespace("source:2/country:../USA").is_err());
    assert!(parse_world_bank_namespace("2/USA").is_err());
}

#[test]
fn empty_structured_unit_is_protocol_not_inference() {
    let key = EconomicSeriesKey::new(
        ProviderId::WorldBank,
        "source:2/country:USA",
        "NY.GDP.MKTP.CD",
    )
    .unwrap();
    let error = parse_world_bank_responses(
        include_bytes!("fixtures/indicator.json"),
        &[],
        &context(&key),
    )
    .unwrap_err();
    assert!(matches!(error, WorldBankError::Protocol(_)));
    assert!(error.to_string().contains("unit"));
}

#[test]
fn indicator_metadata_uses_row_source_identity_not_data_page_fields() {
    let key = EconomicSeriesKey::new(
        ProviderId::WorldBank,
        "source:2/country:USA",
        "NY.GDP.MKTP.CD",
    )
    .unwrap();
    let indicator = include_str!("fixtures/indicator.json")
        .replace("\"unit\":\"\"", "\"unit\":\"current US$\"");
    let wrong_source = indicator.replace("\"source\":{\"id\":\"2\"", "\"source\":{\"id\":\"3\"");
    assert!(matches!(
        parse_world_bank_responses(wrong_source.as_bytes(), &[], &context(&key)),
        Err(WorldBankError::Protocol(_))
    ));

    let wrong_identity = indicator.replace("\"id\":\"NY.GDP.MKTP.CD\"", "\"id\":\"SP.POP.TOTL\"");
    assert!(matches!(
        parse_world_bank_responses(wrong_identity.as_bytes(), &[], &context(&key)),
        Err(WorldBankError::Protocol(_))
    ));

    let invalid_page = indicator.replace("\"pages\":1", "\"pages\":2");
    assert!(matches!(
        parse_world_bank_responses(invalid_page.as_bytes(), &[], &context(&key)),
        Err(WorldBankError::Protocol(_))
    ));
}

#[test]
fn validates_all_pages_missing_zero_and_source_identity() {
    let key = EconomicSeriesKey::new(
        ProviderId::WorldBank,
        "source:2/country:USA",
        "NY.GDP.MKTP.CD",
    )
    .unwrap();
    let indicator = include_str!("fixtures/indicator.json")
        .replace("\"unit\":\"\"", "\"unit\":\"current US$\"");
    let batch = parse_world_bank_responses(
        indicator.as_bytes(),
        &[
            include_bytes!("fixtures/data-page-1.json"),
            include_bytes!("fixtures/data-page-2.json"),
        ],
        &context(&key),
    )
    .unwrap();
    assert_eq!(batch.records().len(), 3);
    assert_eq!(batch.records()[0].value().unwrap().get(), 25.5);
    assert!(matches!(
        batch.records()[1].status(),
        EconomicObservationStatus::Missing
    ));
    assert_eq!(batch.records()[2].value().unwrap().get(), 0.0);
    assert_eq!(batch.records()[2].region_code(), Some("USA"));
    assert_eq!(batch.records()[2].region_name(), Some("United States"));
    assert_eq!(batch.records()[2].revision(), None);
}

#[test]
fn page_metadata_drift_and_duplicate_period_fail_atomically() {
    let key = EconomicSeriesKey::new(
        ProviderId::WorldBank,
        "source:2/country:USA",
        "NY.GDP.MKTP.CD",
    )
    .unwrap();
    let indicator = include_str!("fixtures/indicator.json")
        .replace("\"unit\":\"\"", "\"unit\":\"current US$\"");
    let drift = include_str!("fixtures/data-page-2.json")
        .replace("\"sourceid\":\"2\"", "\"sourceid\":\"3\"");
    assert!(parse_world_bank_responses(
        indicator.as_bytes(),
        &[
            include_bytes!("fixtures/data-page-1.json"),
            drift.as_bytes()
        ],
        &context(&key),
    )
    .is_err());
    let duplicate = include_str!("fixtures/data-page-2.json").replace("\"2022\"", "\"2024\"");
    assert!(parse_world_bank_responses(
        indicator.as_bytes(),
        &[
            include_bytes!("fixtures/data-page-1.json"),
            duplicate.as_bytes()
        ],
        &context(&key),
    )
    .is_err());
    let revised_page_2 =
        include_str!("fixtures/data-page-2.json").replace("2026-07-01", "2026-07-02");
    assert!(matches!(
        parse_world_bank_responses(
            indicator.as_bytes(),
            &[
                include_bytes!("fixtures/data-page-1.json"),
                revised_page_2.as_bytes()
            ],
            &context(&key),
        ),
        Err(WorldBankError::Protocol(_))
    ));
}

#[test]
fn economy_source_id_iso3_and_name_are_stable_across_pages() {
    let key = EconomicSeriesKey::new(
        ProviderId::WorldBank,
        "source:2/country:USA",
        "NY.GDP.MKTP.CD",
    )
    .unwrap();
    let indicator = include_str!("fixtures/indicator.json")
        .replace("\"unit\":\"\"", "\"unit\":\"current US$\"");
    let first_page =
        include_str!("fixtures/data-page-1.json").replace("\"id\":\"US\"", "\"id\":\"USA\"");
    let second_page =
        include_str!("fixtures/data-page-2.json").replace("\"id\":\"US\"", "\"id\":\"USA\"");
    for (label, drift) in [
        (
            "iso3",
            second_page
                .clone()
                .replace("\"countryiso3code\":\"USA\"", "\"countryiso3code\":\"CAN\""),
        ),
        (
            "source-id",
            second_page
                .clone()
                .replace("\"id\":\"USA\"", "\"id\":\"ZZ\""),
        ),
        (
            "name",
            second_page
                .clone()
                .replace("\"value\":\"United States\"", "\"value\":\"Changed Name\""),
        ),
    ] {
        let result = parse_world_bank_responses(
            indicator.as_bytes(),
            &[first_page.as_bytes(), drift.as_bytes()],
            &context(&key),
        );
        assert!(
            matches!(result, Err(WorldBankError::Protocol(_))),
            "{label} drift was accepted: {result:?}"
        );
    }
}
