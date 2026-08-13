use magic_nbs_rs::{NbsClient, NATIONAL_SERIES_ADMITTED, REGIONAL_SERIES_ADMITTED};

#[test]
fn exact_national_and_beijing_scopes_are_advertised() {
    let capabilities = NbsClient::economic_data_capabilities();
    assert_eq!(NATIONAL_SERIES_ADMITTED, capabilities.economic_series);
    assert_eq!(REGIONAL_SERIES_ADMITTED, capabilities.regional_series);
    assert!(capabilities.economic_series);
    assert!(capabilities.regional_series);
}
