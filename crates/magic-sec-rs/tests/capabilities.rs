use magic_market_core::{
    CompanyFilingRequest, CompanyFilingsProvider, PositiveU32, SecCompanyIdentity,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use magic_sec_rs::{SecEdgarClient, SecEdgarError, FILING_METADATA_ADMITTED};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
struct NoIoTransport {
    calls: AtomicUsize,
}

impl HttpTransport for NoIoTransport {
    fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(TransportError::Internal("unexpected I/O".into()))
    }
}

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

#[test]
fn unadmitted_metadata_returns_unsupported_without_io() {
    let transport = Arc::new(NoIoTransport {
        calls: AtomicUsize::new(0),
    });
    let client = SecEdgarClient::with_transport(
        "magic-market-data-rs/0.2 operations@example.com",
        transport.clone(),
    )
    .unwrap();
    let request = CompanyFilingRequest::new(
        vec![SecCompanyIdentity::new("320193", None::<String>).unwrap()],
        vec![],
        None,
        None,
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        CompanyFilingsProvider::company_filings(&client, &request),
        Err(SecEdgarError::Unsupported(_))
    ));
    assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
}
