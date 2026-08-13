use magic_worldbank_rs::{WorldBankClient, ECONOMIC_SERIES_ADMITTED};

#[test]
fn capability_reports_the_exact_series_scoped_admission() {
    assert!(std::hint::black_box(ECONOMIC_SERIES_ADMITTED));
    assert!(WorldBankClient::economic_data_capabilities().economic_series);
    assert!(!WorldBankClient::economic_data_capabilities().regional_series);
}
