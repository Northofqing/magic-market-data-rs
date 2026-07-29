use magic_market_core::{
    CompanyFilingRequest, IsoDate, NonEmptyText, PositiveU32, SecCompanyIdentity,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use magic_sec_rs::{SecEdgarClient, SecEdgarError};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const MAIN: &str = "https://data.sec.gov/submissions/CIK0000320193.json";
const OLDER: &str = "https://data.sec.gov/submissions/CIK0000320193-submissions-001.json";

#[derive(Debug, Clone)]
enum Reply {
    Body(Vec<u8>),
    Fail,
}

#[derive(Debug)]
struct MapTransport {
    replies: HashMap<String, Reply>,
    requests: Mutex<Vec<String>>,
}

impl HttpTransport for MapTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.requests.lock().unwrap().push(request.url().to_owned());
        match self.replies.get(request.url()) {
            Some(Reply::Body(body)) => Ok(HttpResponse::new(
                200,
                request.url(),
                Some("application/json".into()),
                body.clone(),
            )),
            Some(Reply::Fail) => Err(TransportError::Network("fixture failure".into())),
            None => Err(TransportError::Network("unexpected fixture URL".into())),
        }
    }
}

fn request(
    companies: Vec<SecCompanyIdentity>,
    forms: &[&str],
    range: Option<(&str, &str)>,
    max: u32,
) -> CompanyFilingRequest {
    let (start, end) = range
        .map(|(start, end)| {
            (
                Some(IsoDate::new(start).unwrap()),
                Some(IsoDate::new(end).unwrap()),
            )
        })
        .unwrap_or((None, None));
    CompanyFilingRequest::new(
        companies,
        forms
            .iter()
            .map(|form| NonEmptyText::new(*form).unwrap())
            .collect(),
        start,
        end,
        PositiveU32::new(max).unwrap(),
    )
    .unwrap()
}

fn apple() -> SecCompanyIdentity {
    SecCompanyIdentity::new("320193", Some("AAPL")).unwrap()
}

fn run(
    replies: HashMap<String, Reply>,
    request: CompanyFilingRequest,
) -> (
    Result<Vec<magic_market_core::CompanyFiling>, SecEdgarError>,
    Arc<MapTransport>,
) {
    let transport = Arc::new(MapTransport {
        replies,
        requests: Mutex::new(Vec::new()),
    });
    let client = SecEdgarClient::with_transport(
        "magic-market-data-rs/0.2 operations@example.com",
        transport.clone(),
    )
    .unwrap();
    let result = client
        .probe_company_filings(&request)
        .map(|batch| batch.into_records());
    (result, transport)
}

fn standard_replies() -> HashMap<String, Reply> {
    HashMap::from([
        (
            MAIN.into(),
            Reply::Body(include_bytes!("fixtures/submissions.json").to_vec()),
        ),
        (
            OLDER.into(),
            Reply::Body(include_bytes!("fixtures/submissions-older.json").to_vec()),
        ),
    ])
}

fn one_row(mut value: Value, index: usize) -> Value {
    for array in value.as_object_mut().unwrap().values_mut() {
        let selected = array.as_array().unwrap()[index].clone();
        *array = Value::Array(vec![selected]);
    }
    value
}

fn synthetic_rows(cik: &str, year: u8, date: &str, count: usize) -> Value {
    let accessions: Vec<String> = (0..count)
        .map(|index| format!("{cik}-{year:02}-{index:06}"))
        .collect();
    let primary_documents: Vec<String> =
        (0..count).map(|index| format!("doc-{index}.htm")).collect();
    serde_json::json!({
        "accessionNumber": accessions,
        "filingDate": vec![date; count],
        "reportDate": vec![date; count],
        "acceptanceDateTime": vec!["2025-01-02T00:00:00Z"; count],
        "act": vec!["34"; count],
        "form": vec!["10-K"; count],
        "fileNumber": vec!["001-00001"; count],
        "filmNumber": vec!["25000001"; count],
        "items": vec![""; count],
        "size": vec![1_u64; count],
        "isXBRL": vec![1_u8; count],
        "isInlineXBRL": vec![1_u8; count],
        "primaryDocument": primary_documents,
        "primaryDocDescription": vec!["10-K"; count]
    })
}

#[test]
fn intersecting_older_file_is_fully_validated_before_truncation() {
    let (result, transport) = run(
        standard_replies(),
        request(vec![apple()], &[], Some(("2024-01-01", "2024-12-31")), 1),
    );
    let records = result.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].filing_date().as_str(), "2024-12-31");
    assert_eq!(
        transport.requests.lock().unwrap().as_slice(),
        &[MAIN, OLDER]
    );
}

#[test]
fn no_range_stops_before_older_file_once_recent_rows_satisfy_limit() {
    let (result, transport) = run(standard_replies(), request(vec![apple()], &[], None, 1));
    assert_eq!(result.unwrap().len(), 1);
    assert_eq!(transport.requests.lock().unwrap().as_slice(), &[MAIN]);
}

#[test]
fn parent_filename_count_and_range_are_enforced_atomically() {
    let mut invalid_name: Value =
        serde_json::from_slice(include_bytes!("fixtures/submissions.json")).unwrap();
    invalid_name["filings"]["files"][0]["name"] =
        Value::String("CIK0000789019-submissions-001.json".into());
    let replies = HashMap::from([(
        MAIN.into(),
        Reply::Body(serde_json::to_vec(&invalid_name).unwrap()),
    )]);
    let (result, transport) = run(
        replies,
        request(vec![apple()], &[], Some(("2024-01-01", "2024-12-31")), 10),
    );
    assert!(matches!(result, Err(SecEdgarError::Protocol(_))));
    assert_eq!(transport.requests.lock().unwrap().as_slice(), &[MAIN]);

    let mut wrong_count: Value =
        serde_json::from_slice(include_bytes!("fixtures/submissions.json")).unwrap();
    wrong_count["filings"]["files"][0]["filingCount"] = Value::from(3);
    let mut replies = standard_replies();
    replies.insert(
        MAIN.into(),
        Reply::Body(serde_json::to_vec(&wrong_count).unwrap()),
    );
    let (result, _) = run(
        replies,
        request(vec![apple()], &[], Some(("2024-01-01", "2024-12-31")), 10),
    );
    assert!(matches!(result, Err(SecEdgarError::Protocol(_))));
}

#[test]
fn referenced_file_transport_failure_returns_no_partial_batch() {
    let mut replies = standard_replies();
    replies.insert(OLDER.into(), Reply::Fail);
    let (result, transport) = run(
        replies,
        request(vec![apple()], &[], Some(("2024-01-01", "2024-12-31")), 10),
    );
    assert!(matches!(result, Err(SecEdgarError::Transport(_))));
    assert_eq!(
        transport.requests.lock().unwrap().as_slice(),
        &[MAIN, OLDER]
    );
}

#[test]
fn companies_compose_atomically_in_request_order_and_cap_is_explicit() {
    let mut microsoft: Value =
        serde_json::from_slice(include_bytes!("fixtures/submissions.json")).unwrap();
    microsoft["cik"] = Value::String("0000789019".into());
    microsoft["name"] = Value::String("Microsoft Corporation".into());
    microsoft["tickers"] = serde_json::json!(["MSFT"]);
    microsoft["filings"]["files"] = serde_json::json!([]);
    for accession in microsoft["filings"]["recent"]["accessionNumber"]
        .as_array_mut()
        .unwrap()
    {
        let suffix = accession.as_str().unwrap()[10..].to_owned();
        *accession = Value::String(format!("0000789019{suffix}"));
    }
    let microsoft_url = "https://data.sec.gov/submissions/CIK0000789019.json";
    let replies = HashMap::from([
        (
            MAIN.into(),
            Reply::Body(include_bytes!("fixtures/submissions.json").to_vec()),
        ),
        (
            microsoft_url.into(),
            Reply::Body(serde_json::to_vec(&microsoft).unwrap()),
        ),
    ]);
    let (result, transport) = run(
        replies,
        request(
            vec![
                apple(),
                SecCompanyIdentity::new("789019", Some("MSFT")).unwrap(),
            ],
            &[],
            Some(("2025-01-01", "2025-12-31")),
            10,
        ),
    );
    let records = result.unwrap();
    assert_eq!(records.len(), 4);
    assert!(records[..2]
        .iter()
        .all(|record| record.company().cik() == "0000320193"));
    assert!(records[2..]
        .iter()
        .all(|record| record.company().cik() == "0000789019"));
    assert_eq!(
        transport.requests.lock().unwrap().as_slice(),
        &[MAIN, microsoft_url]
    );

    let companies = (1..=11)
        .map(|cik| SecCompanyIdentity::new(cik.to_string(), None::<String>).unwrap())
        .collect();
    let empty = HashMap::new();
    let (result, transport) = run(empty, request(companies, &[], None, 1));
    assert!(matches!(result, Err(SecEdgarError::InvalidRequest(_))));
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn filtered_rows_still_expose_cross_file_signature_conflicts() {
    let mut conflicting_older: Value =
        serde_json::from_slice(include_bytes!("fixtures/submissions-older.json")).unwrap();
    conflicting_older["accessionNumber"][0] = Value::String("0000320193-25-000057".into());
    let mut replies = standard_replies();
    replies.insert(
        OLDER.into(),
        Reply::Body(serde_json::to_vec(&conflicting_older).unwrap()),
    );
    let (result, transport) = run(
        replies,
        request(
            vec![apple()],
            &["S-1"],
            Some(("2024-01-01", "2024-12-31")),
            10,
        ),
    );
    assert!(matches!(result, Err(SecEdgarError::Protocol(_))));
    assert_eq!(
        transport.requests.lock().unwrap().as_slice(),
        &[MAIN, OLDER]
    );
}

#[test]
fn catalog_is_sorted_newest_first_before_no_range_early_stop() {
    let mut main: Value =
        serde_json::from_slice(include_bytes!("fixtures/submissions.json")).unwrap();
    main["filings"]["files"] = serde_json::json!([
        {
            "name":"CIK0000320193-submissions-002.json",
            "filingCount":1,
            "filingFrom":"2023-12-31",
            "filingTo":"2023-12-31"
        },
        {
            "name":"CIK0000320193-submissions-001.json",
            "filingCount":1,
            "filingFrom":"2024-12-31",
            "filingTo":"2024-12-31"
        }
    ]);
    let older: Value =
        serde_json::from_slice(include_bytes!("fixtures/submissions-older.json")).unwrap();
    let replies = HashMap::from([
        (MAIN.into(), Reply::Body(serde_json::to_vec(&main).unwrap())),
        (
            OLDER.into(),
            Reply::Body(serde_json::to_vec(&one_row(older, 0)).unwrap()),
        ),
    ]);
    let (result, transport) = run(replies, request(vec![apple()], &["10-K"], None, 1));
    assert_eq!(result.unwrap().len(), 1);
    assert_eq!(
        transport.requests.lock().unwrap().as_slice(),
        &[MAIN, OLDER]
    );
}

#[test]
fn overlapping_catalog_ranges_fail_before_early_stop() {
    let mut main: Value =
        serde_json::from_slice(include_bytes!("fixtures/submissions.json")).unwrap();
    main["filings"]["files"] = serde_json::json!([
        {
            "name":"CIK0000320193-submissions-001.json",
            "filingCount":2,
            "filingFrom":"2024-01-01",
            "filingTo":"2024-12-31"
        },
        {
            "name":"CIK0000320193-submissions-002.json",
            "filingCount":1,
            "filingFrom":"2024-06-01",
            "filingTo":"2025-01-01"
        }
    ]);
    let replies = HashMap::from([(MAIN.into(), Reply::Body(serde_json::to_vec(&main).unwrap()))]);
    let (result, transport) = run(replies, request(vec![apple()], &[], None, 1));
    assert!(matches!(result, Err(SecEdgarError::Protocol(_))));
    assert_eq!(transport.requests.lock().unwrap().as_slice(), &[MAIN]);
}

#[test]
fn catalog_cannot_be_newer_than_recent_even_when_budget_is_already_full() {
    let mut main: Value =
        serde_json::from_slice(include_bytes!("fixtures/submissions.json")).unwrap();
    main["filings"]["recent"]["filingDate"] = serde_json::json!(["2024-04-04", "2024-05-02"]);
    main["filings"]["files"][0]["filingFrom"] = Value::String("2025-01-01".into());
    main["filings"]["files"][0]["filingTo"] = Value::String("2025-12-31".into());
    let replies = HashMap::from([(MAIN.into(), Reply::Body(serde_json::to_vec(&main).unwrap()))]);
    let (result, transport) = run(replies, request(vec![apple()], &[], None, 1));
    assert!(matches!(result, Err(SecEdgarError::Protocol(_))));
    assert_eq!(transport.requests.lock().unwrap().as_slice(), &[MAIN]);
}

#[test]
fn multi_company_no_range_pagination_uses_one_global_budget() {
    let mut microsoft: Value =
        serde_json::from_slice(include_bytes!("fixtures/submissions.json")).unwrap();
    microsoft["cik"] = Value::String("0000789019".into());
    microsoft["name"] = Value::String("Microsoft Corporation".into());
    microsoft["tickers"] = serde_json::json!(["MSFT"]);
    microsoft["filings"]["files"][0]["name"] =
        Value::String("CIK0000789019-submissions-001.json".into());
    for accession in microsoft["filings"]["recent"]["accessionNumber"]
        .as_array_mut()
        .unwrap()
    {
        let suffix = accession.as_str().unwrap()[10..].to_owned();
        *accession = Value::String(format!("0000789019{suffix}"));
    }
    let microsoft_url = "https://data.sec.gov/submissions/CIK0000789019.json";
    let replies = HashMap::from([
        (
            MAIN.into(),
            Reply::Body(include_bytes!("fixtures/submissions.json").to_vec()),
        ),
        (
            OLDER.into(),
            Reply::Body(include_bytes!("fixtures/submissions-older.json").to_vec()),
        ),
        (
            microsoft_url.into(),
            Reply::Body(serde_json::to_vec(&microsoft).unwrap()),
        ),
    ]);
    let (result, transport) = run(
        replies,
        request(
            vec![
                apple(),
                SecCompanyIdentity::new("789019", Some("MSFT")).unwrap(),
            ],
            &["10-K"],
            None,
            1,
        ),
    );
    assert_eq!(result.unwrap().len(), 1);
    assert_eq!(
        transport.requests.lock().unwrap().as_slice(),
        &[MAIN, OLDER, microsoft_url]
    );
}

#[test]
fn large_synthetic_recent_older_merge_remains_deterministic() {
    const ROWS_PER_FILE: usize = 1_500;
    let mut main: Value =
        serde_json::from_slice(include_bytes!("fixtures/submissions.json")).unwrap();
    main["filings"]["recent"] = synthetic_rows("0000320193", 25, "2025-01-02", ROWS_PER_FILE);
    main["filings"]["files"] = serde_json::json!([{
        "name":"CIK0000320193-submissions-001.json",
        "filingCount":ROWS_PER_FILE,
        "filingFrom":"2024-01-02",
        "filingTo":"2024-01-02"
    }]);
    let older = synthetic_rows("0000320193", 24, "2024-01-02", ROWS_PER_FILE);
    let replies = HashMap::from([
        (MAIN.into(), Reply::Body(serde_json::to_vec(&main).unwrap())),
        (
            OLDER.into(),
            Reply::Body(serde_json::to_vec(&older).unwrap()),
        ),
    ]);
    let (result, transport) = run(
        replies,
        request(
            vec![apple()],
            &["10-K"],
            Some(("2024-01-01", "2025-12-31")),
            1_000,
        ),
    );
    let records = result.unwrap();
    assert_eq!(records.len(), 1_000);
    assert!(records
        .windows(2)
        .all(|pair| pair[0].filing_date() >= pair[1].filing_date()));
    assert_eq!(
        transport.requests.lock().unwrap().as_slice(),
        &[MAIN, OLDER]
    );
}
