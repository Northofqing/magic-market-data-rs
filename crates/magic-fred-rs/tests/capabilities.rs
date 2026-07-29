use magic_fred_rs::{FredClient, FredError, ECONOMIC_SERIES_ADMITTED};

#[test]
fn api_key_is_required_and_never_debugged() {
    assert!(matches!(
        FredClient::new(""),
        Err(FredError::Authentication(_))
    ));
    let client = FredClient::new("secret-key-value").unwrap();
    assert!(!format!("{client:?}").contains("secret-key-value"));
    assert_eq!(
        FredClient::economic_data_capabilities().economic_series,
        ECONOMIC_SERIES_ADMITTED
    );
}
