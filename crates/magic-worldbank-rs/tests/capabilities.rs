use magic_worldbank_rs::{WorldBankClient, ECONOMIC_SERIES_ADMITTED};

#[test]
fn missing_structured_units_prevent_admission() {
    assert!(!std::hint::black_box(ECONOMIC_SERIES_ADMITTED));
    assert!(!WorldBankClient::economic_data_capabilities().economic_series);
}
