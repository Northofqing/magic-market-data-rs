use magic_market_core::{
    CompanyFilingRequest, IsoDate, NonEmptyText, PositiveU32, SecCompanyIdentity,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use magic_sec_rs::{SecEdgarClient, SecEdgarError};
use serde_json::Value;
use std::sync::{Arc, Mutex};

const URL: &str = "https://data.sec.gov/submissions/CIK0000320193.json";

#[derive(Debug)]
struct FixtureTransport {
    body: Vec<u8>,
    requests: Mutex<Vec<String>>,
}

impl HttpTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.requests.lock().unwrap().push(request.url().to_owned());
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json; charset=utf-8".into()),
            self.body.clone(),
        ))
    }
}

fn request(ticker: Option<&str>, forms: &[&str]) -> CompanyFilingRequest {
    CompanyFilingRequest::new(
        vec![SecCompanyIdentity::new("320193", ticker).unwrap()],
        forms
            .iter()
            .map(|form| NonEmptyText::new(*form).unwrap())
            .collect(),
        Some(IsoDate::new("2025-01-01").unwrap()),
        Some(IsoDate::new("2025-12-31").unwrap()),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap()
}

fn run(
    body: Vec<u8>,
    request: CompanyFilingRequest,
) -> Result<Vec<magic_market_core::CompanyFiling>, SecEdgarError> {
    let transport = Arc::new(FixtureTransport {
        body,
        requests: Mutex::new(Vec::new()),
    });
    let client = SecEdgarClient::with_transport(
        "magic-market-data-rs/0.2 operations@example.com",
        transport.clone(),
    )
    .unwrap();
    let records = client.probe_company_filings(&request)?.into_records();
    assert_eq!(transport.requests.lock().unwrap().as_slice(), &[URL]);
    Ok(records)
}

fn fixture_value() -> Value {
    serde_json::from_slice(include_bytes!("fixtures/submissions.json")).unwrap()
}

fn duplicate_first_row(value: &mut Value) {
    let recent = value["filings"]["recent"].as_object_mut().unwrap();
    for array in recent.values_mut() {
        let array = array.as_array_mut().unwrap();
        array.push(array[0].clone());
    }
}

#[test]
fn recent_parallel_arrays_are_validated_then_sorted_and_normalized() {
    let records = run(
        include_bytes!("fixtures/submissions.json").to_vec(),
        request(Some("aapl"), &[]),
    )
    .unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].form(), "10-Q");
    assert_eq!(records[0].filing_date().as_str(), "2025-05-02");
    assert_eq!(records[0].report_period().unwrap().as_str(), "2025-03-29");
    assert_eq!(records[0].company().cik(), "0000320193");
    assert_eq!(records[0].company().ticker(), Some("AAPL"));
    assert_eq!(records[1].report_period(), None);
    assert_eq!(
        records[0].filing_index_url().as_str(),
        "https://www.sec.gov/Archives/edgar/data/320193/000032019325000079/0000320193-25-000079-index.html"
    );
    assert_eq!(
        records[0].primary_document_url().as_str(),
        "https://www.sec.gov/Archives/edgar/data/320193/000032019325000079/aapl-20250329.htm"
    );
    assert_eq!(records[0].accepted_at(), Some("2025-05-01T18:26:29Z"));
    assert_eq!(records[0].evidence().source_at(), records[0].accepted_at());
}

#[test]
fn filtered_out_rows_cannot_hide_parallel_array_or_identity_failures() {
    let mut short = fixture_value();
    short["filings"]["recent"]["size"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(matches!(
        run(serde_json::to_vec(&short).unwrap(), request(None, &["8-K"])),
        Err(SecEdgarError::Protocol(_))
    ));

    let mut wrong_cik = fixture_value();
    wrong_cik["cik"] = Value::String("0000789019".into());
    assert!(matches!(
        run(serde_json::to_vec(&wrong_cik).unwrap(), request(None, &[])),
        Err(SecEdgarError::Protocol(_))
    ));
    assert!(matches!(
        run(
            include_bytes!("fixtures/submissions.json").to_vec(),
            request(Some("MSFT"), &[])
        ),
        Err(SecEdgarError::Protocol(_))
    ));
}

#[test]
fn malformed_source_time_and_unsafe_primary_document_fail_atomically() {
    let mut bad_time = fixture_value();
    bad_time["filings"]["recent"]["acceptanceDateTime"][0] = Value::String("not-a-time".into());
    assert!(matches!(
        run(serde_json::to_vec(&bad_time).unwrap(), request(None, &[])),
        Err(SecEdgarError::Protocol(_))
    ));

    let mut unsafe_path = fixture_value();
    unsafe_path["filings"]["recent"]["primaryDocument"][0] = Value::String("../secret.htm".into());
    assert!(matches!(
        run(
            serde_json::to_vec(&unsafe_path).unwrap(),
            request(None, &[])
        ),
        Err(SecEdgarError::Core(_))
    ));
}

#[test]
fn exact_duplicates_collapse_and_conflicts_fail() {
    let mut duplicate = fixture_value();
    duplicate_first_row(&mut duplicate);
    assert_eq!(
        run(serde_json::to_vec(&duplicate).unwrap(), request(None, &[]))
            .unwrap()
            .len(),
        2
    );

    let mut unused_field_conflict = duplicate.clone();
    unused_field_conflict["filings"]["recent"]["size"][2] = Value::from(999_u64);
    assert!(matches!(
        run(
            serde_json::to_vec(&unused_field_conflict).unwrap(),
            request(None, &[])
        ),
        Err(SecEdgarError::Protocol(_))
    ));

    duplicate["filings"]["recent"]["form"][2] = Value::String("10-K".into());
    assert!(matches!(
        run(serde_json::to_vec(&duplicate).unwrap(), request(None, &[])),
        Err(SecEdgarError::Protocol(_))
    ));
}
