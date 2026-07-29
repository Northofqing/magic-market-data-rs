use magic_market_core::{CompanyFilingRequest, IsoDate, PositiveU32, SecCompanyIdentity};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use magic_sec_rs::SecEdgarClient;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct RecordingTransport {
    requests: Mutex<Vec<String>>,
}

impl HttpTransport for RecordingTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.requests.lock().unwrap().push(request.url().to_owned());
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            include_bytes!("fixtures/submissions.json").to_vec(),
        ))
    }
}

#[test]
fn provider_fetches_only_submissions_metadata_and_never_archive_documents() {
    let transport = Arc::new(RecordingTransport {
        requests: Mutex::new(Vec::new()),
    });
    let client = SecEdgarClient::with_transport(
        "magic-market-data-rs/0.2 operations@example.com",
        transport.clone(),
    )
    .unwrap();
    let request = CompanyFilingRequest::new(
        vec![SecCompanyIdentity::new("320193", Some("AAPL")).unwrap()],
        vec![],
        Some(IsoDate::new("2025-01-01").unwrap()),
        Some(IsoDate::new("2025-12-31").unwrap()),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    let batch = client.probe_company_filings(&request).unwrap();
    let requests = transport.requests.lock().unwrap();
    assert!(!requests.is_empty());
    assert!(requests
        .iter()
        .all(|url| url.starts_with("https://data.sec.gov/submissions/")));
    assert!(requests.iter().all(|url| !url.contains("/Archives/")));

    let record = serde_json::to_value(&batch.records()[0]).unwrap();
    let object = record.as_object().unwrap();
    for forbidden in [
        "body",
        "attachments",
        "attachment_list",
        "xbrl",
        "xbrl_facts",
    ] {
        assert!(!object.contains_key(forbidden), "{forbidden}");
    }
    assert!(object["filing_index_url"]
        .as_str()
        .unwrap()
        .starts_with("https://www.sec.gov/Archives/"));
    assert!(object["primary_document_url"]
        .as_str()
        .unwrap()
        .starts_with("https://www.sec.gov/Archives/"));
}
