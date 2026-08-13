use super::*;
use magic_market_core::{PositiveU32, SecPrimaryDocument};
use serde_json::Value;

const OBSERVED_AT: &str = "2026-07-29T00:00:00Z";
const BATCH_ID: &str = "sec-unit";

fn company(ticker: Option<&str>) -> SecCompanyIdentity {
    SecCompanyIdentity::new("320193", ticker).unwrap()
}

fn request(
    ticker: Option<&str>,
    forms: &[&str],
    range: Option<(&str, &str)>,
) -> CompanyFilingRequest {
    let (start, end) = range.map_or((None, None), |(start, end)| {
        (
            Some(IsoDate::new(start).unwrap()),
            Some(IsoDate::new(end).unwrap()),
        )
    });
    CompanyFilingRequest::new(
        vec![company(ticker)],
        forms
            .iter()
            .map(|form| NonEmptyText::new(*form).unwrap())
            .collect(),
        start,
        end,
        PositiveU32::new(10).unwrap(),
    )
    .unwrap()
}

fn parent(body: &[u8]) -> ParsedCompany {
    parse_parent(
        body,
        &company(Some("AAPL")),
        &request(Some("AAPL"), &[], None),
        OBSERVED_AT,
        BATCH_ID,
    )
    .unwrap()
}

fn descriptor(sequence: usize, count: usize, from: &str, to: &str) -> OlderFileWire {
    OlderFileWire {
        name: format!("CIK0000320193-submissions-{sequence:03}.json"),
        filing_count: count,
        filing_from: from.into(),
        filing_to: to.into(),
    }
}

#[test]
fn cik_company_name_and_acceptance_helpers_cover_closed_shapes() {
    assert_eq!(
        CikWire::Text("0000320193".into()).into_string(),
        "0000320193"
    );
    assert_eq!(CikWire::Number(320_193).into_string(), "320193");

    validate_company_name("Apple Inc.").unwrap();
    for invalid in ["", " \t ", "unsafe\nname"] {
        assert!(validate_company_name(invalid).is_err());
    }
    assert!(validate_company_name(&"x".repeat(MAX_COMPANY_NAME_CHARS + 1)).is_err());

    let empty = parse_acceptance("").unwrap();
    assert!(empty.text.is_none());
    assert!(empty.sort_key.is_none());
    let accepted = parse_acceptance("2025-05-01T20:26:29+02:00").unwrap();
    assert_eq!(accepted.text.unwrap().as_str(), "2025-05-01T18:26:29Z");
    assert!(accepted.sort_key.is_some());
    assert!(parse_acceptance("2025-05-01 18:26:29").is_err());
}

#[test]
fn response_company_and_filter_helpers_cover_optional_tickers_and_ranges() {
    let no_ticker = company(None);
    assert_eq!(
        checked_response_company(&no_ticker, &[]).unwrap().ticker(),
        None
    );
    assert_eq!(
        checked_response_company(&no_ticker, &["AAPL".into()])
            .unwrap()
            .ticker(),
        Some("AAPL")
    );
    assert_eq!(
        checked_response_company(&company(Some("aapl")), &["AAPL".into()])
            .unwrap()
            .ticker(),
        Some("AAPL")
    );
    assert!(checked_response_company(&company(Some("MSFT")), &["AAPL".into()]).is_err());

    let parsed = parent(include_bytes!("../../tests/fixtures/submissions.json"));
    let record = &parsed.records[0];
    assert!(matches_filters(record, &request(Some("AAPL"), &[], None)));
    assert!(matches_filters(
        record,
        &request(Some("AAPL"), &["10-Q"], Some(("2025-01-01", "2025-12-31")))
    ));
    assert!(!matches_filters(
        record,
        &request(Some("AAPL"), &["8-K"], Some(("2025-01-01", "2025-12-31")))
    ));
    assert!(!matches_filters(
        record,
        &request(Some("AAPL"), &[], Some(("2024-01-01", "2024-12-31")))
    ));
}

#[test]
fn canonical_url_helpers_cover_zero_cik_and_reject_unsafe_components() {
    let accession = SecAccessionNumber::new("0000000000-25-000001").unwrap();
    let primary = SecPrimaryDocument::new("document.htm").unwrap();
    let (index, document) = canonical_urls("0000000000", &accession, &primary).unwrap();
    assert!(index
        .as_str()
        .starts_with("https://www.sec.gov/Archives/edgar/data/0/"));
    assert!(document.as_str().ends_with("/document.htm"));

    let base = "https://www.sec.gov/Archives/edgar/data/1/abc/";
    validate_canonical_url(&format!("{base}file.htm"), base).unwrap();
    for url in [
        "https://www.sec.gov/Archives/edgar/data/2/abc/file.htm",
        "https://www.sec.gov/Archives/edgar/data/1/abc/file.htm?x=1",
        "https://www.sec.gov/Archives/edgar/data/1/abc/file.htm#x",
        "https://www.sec.gov/Archives/edgar/data/1/abc/@file.htm",
        "https://www.sec.gov/Archives/edgar/data/1/abc/file\n.htm",
    ] {
        assert!(validate_canonical_url(url, base).is_err(), "{url:?}");
    }
    assert!(
        validate_canonical_url("https://example.test/file.htm", "https://example.test/").is_err()
    );
}

#[test]
fn older_catalog_validation_covers_limits_order_overlap_and_boundaries() {
    let too_many = (1..=MAX_OLDER_FILES + 1)
        .map(|sequence| descriptor(sequence, 1, "2024-01-01", "2024-01-01"))
        .collect();
    assert!(parse_older_descriptors(too_many, "0000320193").is_err());

    let duplicate = vec![
        descriptor(1, 1, "2024-01-01", "2024-01-01"),
        descriptor(1, 1, "2023-01-01", "2023-01-01"),
    ];
    assert!(parse_older_descriptors(duplicate, "0000320193").is_err());
    assert!(parse_older_descriptors(
        vec![descriptor(
            1,
            MAX_DECODED_FILINGS + 1,
            "2024-01-01",
            "2024-01-01"
        )],
        "0000320193"
    )
    .is_err());
    assert!(parse_older_descriptors(
        vec![descriptor(1, 1, "2024-12-31", "2024-01-01")],
        "0000320193"
    )
    .is_err());
    assert!(parse_older_descriptors(
        vec![
            descriptor(1, 1, "2024-01-01", "2024-12-31"),
            descriptor(2, 1, "2024-06-01", "2025-01-01"),
        ],
        "0000320193"
    )
    .is_err());

    let sorted = parse_older_descriptors(
        vec![
            descriptor(2, 1, "2023-01-01", "2023-12-31"),
            descriptor(1, 1, "2024-01-01", "2024-12-31"),
        ],
        "0000320193",
    )
    .unwrap();
    assert!(sorted[0].name.ends_with("-001.json"));
    validate_recent_catalog_boundary(&[], &sorted).unwrap();
    validate_recent_catalog_boundary(&[IsoDate::new("2025-01-01").unwrap()], &sorted).unwrap();
    assert!(
        validate_recent_catalog_boundary(&[IsoDate::new("2024-06-01").unwrap()], &sorted).is_err()
    );
}

#[test]
fn older_filename_contract_rejects_each_shape_error() {
    validate_older_filename("CIK0000320193-submissions-001.json", "0000320193").unwrap();
    for name in [
        "CIK0000789019-submissions-001.json",
        "CIK0000320193-submissions-001.txt",
        "CIK0000320193-submissions-01.json",
        "CIK0000320193-submissions-abc.json",
    ] {
        assert!(validate_older_filename(name, "0000320193").is_err());
    }
}

#[test]
fn empty_older_file_and_malformed_envelopes_fail_explicitly() {
    assert!(matches!(
        parse_parent(
            b"not-json",
            &company(None),
            &request(None, &[], None),
            OBSERVED_AT,
            BATCH_ID
        ),
        Err(SecEdgarError::Decode(_))
    ));
    let empty = serde_json::json!({
        "accessionNumber": [],
        "filingDate": [],
        "reportDate": [],
        "acceptanceDateTime": [],
        "act": [],
        "form": [],
        "fileNumber": [],
        "filmNumber": [],
        "items": [],
        "size": [],
        "isXBRL": [],
        "isInlineXBRL": [],
        "primaryDocument": [],
        "primaryDocDescription": []
    });
    assert!(matches!(
        parse_older(
            &serde_json::to_vec(&empty).unwrap(),
            &company(None),
            "Apple Inc.",
            &request(None, &[], None),
            OBSERVED_AT,
            BATCH_ID
        ),
        Err(SecEdgarError::Protocol(_))
    ));
    assert!(matches!(
        parse_older(
            b"not-json",
            &company(None),
            "Apple Inc.",
            &request(None, &[], None),
            OBSERVED_AT,
            BATCH_ID
        ),
        Err(SecEdgarError::Decode(_))
    ));
}

#[test]
fn filing_accession_may_bind_a_distinct_login_or_agent_cik() {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../tests/fixtures/submissions.json")).unwrap();
    value["filings"]["recent"]["accessionNumber"][0] = Value::String("0000789019-25-000057".into());
    let parsed = parse_parent(
        &serde_json::to_vec(&value).unwrap(),
        &company(Some("AAPL")),
        &request(Some("AAPL"), &[], None),
        OBSERVED_AT,
        BATCH_ID,
    )
    .unwrap();
    assert_eq!(parsed.records[0].company().cik(), "0000320193");
    assert!(parsed
        .records
        .iter()
        .any(|record| record.accession().as_str() == "0000789019-25-000057"));
}

#[test]
fn merge_and_sort_helpers_reject_inconsistent_cross_file_state() {
    let parsed = parent(include_bytes!("../../tests/fixtures/submissions.json"));
    let mut destination = parsed.records.clone();
    let mut destination_index = parsed.record_index.clone();
    let mut destination_signatures = parsed.signatures.clone();
    merge_records(
        &mut destination,
        &mut destination_index,
        &mut destination_signatures,
        parsed.records.clone(),
        parsed.signatures.clone(),
    )
    .unwrap();
    assert_eq!(destination.len(), parsed.records.len());

    let identity = filing_identity(&parsed.records[0]);
    let mut inconsistent_index = HashMap::from([(identity.clone(), usize::MAX)]);
    assert!(merge_records(
        &mut parsed.records.clone(),
        &mut inconsistent_index,
        &mut HashMap::new(),
        vec![parsed.records[0].clone()],
        HashMap::new(),
    )
    .is_err());

    let mut conflicting = parsed.signatures[&identity].clone();
    conflicting.size += 1;
    assert!(merge_records(
        &mut parsed.records.clone(),
        &mut parsed.record_index.clone(),
        &mut parsed.signatures.clone(),
        Vec::new(),
        HashMap::from([(identity.clone(), conflicting)]),
    )
    .is_err());

    let mut changed: Value =
        serde_json::from_slice(include_bytes!("../../tests/fixtures/submissions.json")).unwrap();
    changed["name"] = Value::String("Different Company Name".into());
    let changed = parse_parent(
        &serde_json::to_vec(&changed).unwrap(),
        &company(Some("AAPL")),
        &request(Some("AAPL"), &[], None),
        OBSERVED_AT,
        BATCH_ID,
    )
    .unwrap();
    assert!(merge_records(
        &mut parsed.records.clone(),
        &mut parsed.record_index.clone(),
        &mut HashMap::new(),
        changed.records,
        HashMap::new(),
    )
    .is_err());

    let mut reversed = parsed.records.clone();
    reversed.reverse();
    sort_records(&mut reversed);
    assert!(reversed
        .windows(2)
        .all(|pair| pair[0].filing_date() >= pair[1].filing_date()));
}
