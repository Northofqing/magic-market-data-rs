use magic_market_core::CompanyFilingsProvider;
use magic_sec_rs::{SecEdgarClient, SecEdgarError, FILING_METADATA_ADMITTED};

#[test]
fn descriptive_user_agent_is_required_and_redacted() {
    assert!(matches!(
        SecEdgarClient::new(""),
        Err(SecEdgarError::InvalidRequest(_))
    ));
    assert!(matches!(
        SecEdgarClient::new("anonymous-client"),
        Err(SecEdgarError::InvalidRequest(_))
    ));
    let client = SecEdgarClient::new("magic-market-data-rs/0.2 operations@example.com").unwrap();
    let debug = format!("{client:?}");
    assert!(!debug.contains("operations@example.com"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn only_metadata_can_ever_be_admitted() {
    let capabilities = SecEdgarClient::capabilities();
    assert_eq!(capabilities.filing_metadata, FILING_METADATA_ADMITTED);
    assert!(!capabilities.filing_documents);
    assert!(!capabilities.xbrl_facts);
    fn assert_provider<T: CompanyFilingsProvider>() {}
    assert_provider::<SecEdgarClient>();
}

const _: () = assert!(FILING_METADATA_ADMITTED);
