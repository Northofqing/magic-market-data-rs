use super::*;

fn key(provider: ProviderId, namespace: &str, code: &str) -> EconomicSeriesKey {
    EconomicSeriesKey::new(provider, namespace, code).unwrap()
}

fn indicator() -> Value {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../tests/fixtures/indicator.json")).unwrap();
    value[1][0]["unit"] = Value::String("USD".into());
    value
}

fn page_one() -> Value {
    serde_json::from_slice(include_bytes!("../../tests/fixtures/data-page-1.json")).unwrap()
}

fn page_two() -> Value {
    serde_json::from_slice(include_bytes!("../../tests/fixtures/data-page-2.json")).unwrap()
}

fn run(
    indicator: &Value,
    pages: &[Value],
    key: &EconomicSeriesKey,
    start_year: u32,
    end_year: u32,
) -> Result<DataBatch<EconomicObservation>, WorldBankError> {
    let indicator = serde_json::to_vec(indicator).unwrap();
    let pages = pages
        .iter()
        .map(|page| serde_json::to_vec(page).unwrap())
        .collect::<Vec<_>>();
    let page_refs = pages.iter().map(Vec::as_slice).collect::<Vec<_>>();
    parse_world_bank_responses(
        &indicator,
        &page_refs,
        &WorldBankParseContext {
            key,
            start_year,
            end_year,
            observed_at: "2026-07-29T00:00:00Z",
            batch_id: "world-bank-unit",
        },
    )
}

#[test]
fn namespace_identity_and_year_helpers_are_closed() {
    let namespace = parse_world_bank_namespace("source:2/country:USA").unwrap();
    assert_eq!(namespace.source_id(), "2");
    assert_eq!(namespace.economy(), "USA");
    for value in [
        "",
        "source:2",
        "source:x/country:USA",
        "source:2/country:us/a",
        "source:2/country:USA/extra",
    ] {
        assert!(parse_world_bank_namespace(value).is_err());
    }
    assert!(valid_source_id("123"));
    assert!(!valid_source_id(""));
    assert!(!valid_source_id("x"));
    assert!(!valid_source_id("12345678901"));
    assert!(valid_code("NY.GDP_MKTP-CD", 1, 64));
    assert!(!valid_code("BAD/PATH", 1, 64));
    assert!(economy_matches("USA", "US", "USA"));
    assert!(economy_matches("US", "US", "USA"));
    assert!(!economy_matches("CHN", "US", "USA"));
    assert_eq!(parse_year("2025").unwrap(), 2025);
    for year in ["", "20x5", "1899", "10000"] {
        assert!(parse_year(year).is_err());
    }
}

#[test]
fn json_field_and_page_shape_helpers_reject_ambiguity() {
    let valid = serde_json::json!({
        "page": "1",
        "pages": 2,
        "per_page": 10,
        "total": 20,
        "sourceid": 2,
        "lastupdated": "2026-07-01",
        "name": "GDP",
        "empty": "",
        "nested": {"id":"X"}
    });
    let object = valid.as_object().unwrap();
    assert_eq!(usize_field(object, "page").unwrap(), 1);
    assert_eq!(usize_field(object, "pages").unwrap(), 2);
    assert_eq!(string_or_number_field(object, "sourceid").unwrap(), "2");
    assert_eq!(string_field(object, "name").unwrap(), "GDP");
    assert_eq!(string_field_allow_empty(object, "empty").unwrap(), "");
    assert_eq!(nested_string(object, "nested", "id").unwrap(), "X");
    for key in ["missing", "empty"] {
        assert!(string_field(object, key).is_err());
    }
    assert!(usize_field(object, "missing").is_err());
    assert!(string_or_number_field(object, "missing").is_err());
    assert!(nested_string(object, "missing", "id").is_err());
    assert!(two_element_page(&serde_json::json!({})).is_err());
    assert!(two_element_page(&serde_json::json!([])).is_err());
    let page = parse_pagination_metadata(&valid).unwrap();
    assert_eq!(page.page, 1);
    let metadata = parse_page_metadata(&valid).unwrap();
    assert_eq!(metadata.source_id, "2");
    assert!(parse_page_metadata(&serde_json::json!(1)).is_err());
    assert!(parse_pagination_metadata(&serde_json::json!(1)).is_err());
}

#[test]
fn request_and_indicator_envelopes_reject_every_identity_shape() {
    let world_bank = key(
        ProviderId::WorldBank,
        "source:2/country:USA",
        "NY.GDP.MKTP.CD",
    );
    let foreign = key(ProviderId::Fred, "source:2/country:USA", "NY.GDP.MKTP.CD");
    assert!(run(
        &indicator(),
        &[page_one(), page_two()],
        &foreign,
        2022,
        2024
    )
    .is_err());
    assert!(run(
        &indicator(),
        &[page_one(), page_two()],
        &world_bank,
        2025,
        2024
    )
    .is_err());

    let mut rows_not_array = indicator();
    rows_not_array[1] = serde_json::json!({});
    assert!(run(
        &rows_not_array,
        &[page_one(), page_two()],
        &world_bank,
        2022,
        2024
    )
    .is_err());
    let mut no_rows = indicator();
    no_rows[1] = serde_json::json!([]);
    assert!(run(&no_rows, &[page_one(), page_two()], &world_bank, 2022, 2024).is_err());
    let mut multiple = indicator();
    let duplicate = multiple[1][0].clone();
    multiple[1].as_array_mut().unwrap().push(duplicate);
    assert!(run(
        &multiple,
        &[page_one(), page_two()],
        &world_bank,
        2022,
        2024
    )
    .is_err());
    let mut row_not_object = indicator();
    row_not_object[1][0] = serde_json::json!(1);
    assert!(run(
        &row_not_object,
        &[page_one(), page_two()],
        &world_bank,
        2022,
        2024
    )
    .is_err());
}

#[test]
fn data_pages_reject_duplicate_shape_identity_period_and_coverage_drift() {
    let key = key(
        ProviderId::WorldBank,
        "source:2/country:USA",
        "NY.GDP.MKTP.CD",
    );
    let indicator = indicator();

    assert!(run(&indicator, &[page_one(), page_one()], &key, 2022, 2024).is_err());
    let mut rows_not_array = page_one();
    rows_not_array[1] = serde_json::json!({});
    assert!(run(&indicator, &[rows_not_array, page_two()], &key, 2022, 2024).is_err());
    let mut too_many = page_one();
    too_many[0]["per_page"] = serde_json::json!(1);
    let mut stable_two = page_two();
    stable_two[0]["per_page"] = serde_json::json!(1);
    assert!(run(&indicator, &[too_many, stable_two], &key, 2022, 2024).is_err());
    let mut row_not_object = page_one();
    row_not_object[1][0] = serde_json::json!(1);
    assert!(run(&indicator, &[row_not_object, page_two()], &key, 2022, 2024).is_err());
    let mut wrong_identity = page_one();
    wrong_identity[1][0]["indicator"]["id"] = Value::String("OTHER".into());
    assert!(run(&indicator, &[wrong_identity, page_two()], &key, 2022, 2024).is_err());
    assert!(run(&indicator, &[page_one(), page_two()], &key, 2023, 2024).is_err());
    let mut wrong_value = page_one();
    wrong_value[1][0]["value"] = Value::String("not-numeric".into());
    assert!(run(&indicator, &[wrong_value, page_two()], &key, 2022, 2024).is_err());

    let mut exact_duplicate = page_two();
    exact_duplicate[1][0] = page_one()[1][1].clone();
    assert!(run(&indicator, &[page_one(), exact_duplicate], &key, 2022, 2024).is_err());
    let mut conflict = page_two();
    conflict[1][0]["date"] = Value::String("2023".into());
    assert!(run(&indicator, &[page_one(), conflict], &key, 2022, 2024).is_err());
    assert!(run(&indicator, &[page_one()], &key, 2022, 2024).is_err());
}
