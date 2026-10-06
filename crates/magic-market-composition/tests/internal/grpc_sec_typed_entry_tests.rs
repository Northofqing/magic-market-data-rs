use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};

use magic_market_core::{
    CompanyFiling, CompanyFilingRequest, CoreError, DataBatch, HttpsUrl, IsoDate, NonEmptyText,
    Provenance, ProviderId, SecAccessionNumber, SecCompanyIdentity, SecPrimaryDocument,
    SourceEvidence,
};
use magic_market_service::{CanonicalPayload, Operation, QueryCommand, QueryResult, ServiceError};
use magic_sec_rs::SecEdgarError;
use serde_json::{json, Value};

use super::super::{execute_typed, COMPANY_FILINGS_RECORD_SCHEMA, COMPANY_FILINGS_REQUEST_SCHEMA};

const OBSERVED_AT: &str = "2026-09-12T12:00:00Z";
const SOURCE_AT: &str = "2026-09-12T09:00:00Z";
const BATCH_ID: &str = "offline-sec-typed-fixture:batch";

fn request_json() -> Value {
    json!({
        "companies": [{"cik":"2","ticker":"synb"},{"cik":"1","ticker":null}],
        "forms": ["10-Q","10-K"],
        "start": "2026-01-01",
        "end": "2026-09-30",
        "max_records": 20
    })
}

fn command(data: Value, schema: &str, version: u32) -> QueryCommand {
    QueryCommand::new(
        "offline-sec-typed-request",
        Operation::CompanyFilings,
        Some("SecEdgar".into()),
        CanonicalPayload::new(schema, version, serde_json::to_vec(&data).unwrap(), 65536).unwrap(),
    )
    .unwrap()
}

fn record(
    company_name: &str,
    sequence: u32,
    report_period: Option<&str>,
    accepted_at: Option<&str>,
) -> CompanyFiling {
    let accession = SecAccessionNumber::new(format!("0000000002-26-{sequence:06}")).unwrap();
    let document = SecPrimaryDocument::new(format!("offline-{sequence}.htm")).unwrap();
    let archive = format!(
        "https://www.sec.gov/Archives/edgar/data/2/{}",
        accession.without_hyphens()
    );
    let evidence = SourceEvidence::new(ProviderId::SecEdgar, OBSERVED_AT, BATCH_ID).unwrap();
    let evidence = match accepted_at {
        Some(value) => evidence.with_source_at(value).unwrap(),
        None => evidence,
    };
    CompanyFiling::new(
        SecCompanyIdentity::new("2", None::<String>).unwrap(),
        company_name,
        if sequence == 1 { "10-Q" } else { "10-K" },
        IsoDate::new("2026-09-12").unwrap(),
        report_period.map(|value| IsoDate::new(value).unwrap()),
        accession.clone(),
        document.clone(),
        HttpsUrl::new(format!("{archive}/{accession}-index.html")).unwrap(),
        HttpsUrl::new(format!("{archive}/{document}")).unwrap(),
        accepted_at.map(|value| NonEmptyText::new(value).unwrap()),
        evidence,
    )
    .unwrap()
}

fn batch() -> DataBatch<CompanyFiling> {
    DataBatch::strict(
        vec![
            record("Offline synthetic issuer", 1, None, Some(SOURCE_AT)),
            record("Offline synthetic issuer", 2, Some("2025-12-31"), None),
        ],
        Provenance::new("offline-sec-typed-fixture", OBSERVED_AT)
            .unwrap()
            .with_batch_id(BATCH_ID)
            .unwrap()
            .with_source_at(SOURCE_AT)
            .unwrap(),
    )
}

fn execute_fixture(
    command: QueryCommand,
    batch: DataBatch<CompanyFiling>,
    maximum_bytes: usize,
    calls: &AtomicUsize,
    forwarded: &RefCell<Option<CompanyFilingRequest>>,
) -> Result<QueryResult, ServiceError> {
    execute_typed(
        command,
        COMPANY_FILINGS_REQUEST_SCHEMA,
        COMPANY_FILINGS_RECORD_SCHEMA,
        "SecEdgar",
        maximum_bytes,
        |request: &CompanyFilingRequest| {
            calls.fetch_add(1, Ordering::SeqCst);
            *forwarded.borrow_mut() = Some(request.clone());
            Ok::<_, SecEdgarError>(batch)
        },
    )
}

#[test]
fn sec_typed_entry_forwards_exact_bounded_request_and_metadata() {
    let original = batch();
    let calls = AtomicUsize::new(0);
    let forwarded = RefCell::new(None);
    let result = execute_fixture(
        command(request_json(), COMPANY_FILINGS_REQUEST_SCHEMA, 1),
        original.clone(),
        4096,
        &calls,
        &forwarded,
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let captured = forwarded.borrow();
    let request = captured.as_ref().unwrap();
    assert_eq!(request.companies()[0].cik(), "0000000002");
    assert_eq!(request.companies()[0].ticker(), Some("SYNB"));
    assert_eq!(request.companies()[1].cik(), "0000000001");
    assert_eq!(request.companies()[1].ticker(), None);
    assert_eq!(request.forms()[0].as_str(), "10-Q");
    assert_eq!(request.forms()[1].as_str(), "10-K");
    assert_eq!(request.start().unwrap().as_str(), "2026-01-01");
    assert_eq!(request.end().unwrap().as_str(), "2026-09-30");
    assert_eq!(request.max_records().get(), 20);
    assert_eq!(result.provider, "SecEdgar");
    assert_eq!(result.batch_id, BATCH_ID);
    assert_eq!(result.observed_at, OBSERVED_AT);
    assert_eq!(result.source_at.as_deref(), Some(SOURCE_AT));
    assert!(result.complete);
    assert_eq!(result.records.len(), original.records().len());
    for (payload, source) in result.records.iter().zip(original.records()) {
        assert_eq!(payload.schema(), COMPANY_FILINGS_RECORD_SCHEMA);
        assert_eq!(payload.schema_version(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(payload.data()).unwrap(),
            serde_json::to_value(source).unwrap()
        );
    }
}

#[test]
fn sec_typed_entry_forwards_absent_dates_and_unfiltered_forms() {
    let mut data = request_json();
    data["start"] = Value::Null;
    data["end"] = Value::Null;
    data["forms"] = json!([]);
    let calls = AtomicUsize::new(0);
    let forwarded = RefCell::new(None);
    execute_fixture(
        command(data, COMPANY_FILINGS_REQUEST_SCHEMA, 1),
        batch(),
        4096,
        &calls,
        &forwarded,
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let captured = forwarded.borrow();
    let request = captured.as_ref().unwrap();
    assert_eq!(request.start(), None);
    assert_eq!(request.end(), None);
    assert!(request.forms().is_empty());
    assert_eq!(request.max_records().get(), 20);
}

#[test]
fn sec_typed_entry_keeps_date_precision_and_missing_report_fields() {
    let result = execute_fixture(
        command(request_json(), COMPANY_FILINGS_REQUEST_SCHEMA, 1),
        batch(),
        4096,
        &AtomicUsize::new(0),
        &RefCell::new(None),
    )
    .unwrap();
    let first: Value = serde_json::from_slice(result.records[0].data()).unwrap();
    let second: Value = serde_json::from_slice(result.records[1].data()).unwrap();
    assert_eq!(first["form"], "10-Q");
    assert_eq!(first["filing_date"], "2026-09-12");
    assert_eq!(first["report_period"], Value::Null);
    assert!(first.get("fiscal_period").is_none());
    assert_eq!(second["form"], "10-K");
    assert_eq!(second["report_period"], "2025-12-31");
    assert_eq!(second["accepted_at"], Value::Null);
    let decoded: CompanyFiling = serde_json::from_value(second).unwrap();
    assert_eq!(decoded.accepted_at(), None);
    assert_eq!(decoded.evidence().source_at(), None);
}

#[test]
fn sec_typed_entry_rejects_bad_envelopes_before_provider_call() {
    for (schema, version) in [
        ("offline.wrong.schema", 1),
        (COMPANY_FILINGS_REQUEST_SCHEMA, 2),
    ] {
        let calls = AtomicUsize::new(0);
        let forwarded = RefCell::new(None);
        let error = execute_fixture(
            command(request_json(), schema, version),
            batch(),
            4096,
            &calls,
            &forwarded,
        )
        .unwrap_err();
        assert!(
            matches!(error, ServiceError::InvalidRequest(_)),
            "{error:?}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(forwarded.borrow().is_none());
    }
}

#[test]
fn sec_typed_entry_rejects_invalid_bounded_requests_before_provider_call() {
    let mut cases = vec![json!({}), Value::Null, json!([])];
    for (field, value) in [
        ("companies", json!([])),
        ("companies", json!("2")),
        ("companies", json!([{"cik":"not-a-cik","ticker":null}])),
        ("companies", json!([{"cik":"2"},{"cik":"0000000002"}])),
        (
            "companies",
            Value::Array(
                (1..=101)
                    .map(|index| json!({"cik":index.to_string()}))
                    .collect(),
            ),
        ),
        ("forms", json!(["10-K", "10-K"])),
        ("forms", json!([""])),
        (
            "forms",
            Value::Array(
                (0..21)
                    .map(|index| json!(format!("OFFLINE-{index}")))
                    .collect(),
            ),
        ),
        ("max_records", json!(0)),
        ("max_records", json!(1001)),
        ("end", Value::Null),
        ("start", json!("2026-10-01")),
    ] {
        let mut data = request_json();
        data[field] = value;
        cases.push(data);
    }
    let mut missing_limit = request_json();
    missing_limit.as_object_mut().unwrap().remove("max_records");
    cases.push(missing_limit);
    for data in cases {
        let calls = AtomicUsize::new(0);
        let forwarded = RefCell::new(None);
        let error = execute_fixture(
            command(data.clone(), COMPANY_FILINGS_REQUEST_SCHEMA, 1),
            batch(),
            4096,
            &calls,
            &forwarded,
        )
        .unwrap_err();
        assert!(
            matches!(error, ServiceError::InvalidRequest(_)),
            "{data}: {error:?}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0, "{data}");
        assert!(forwarded.borrow().is_none(), "{data}");
    }
}

#[test]
fn sec_typed_entry_keeps_each_legacy_provider_failure() {
    let errors = [
        SecEdgarError::InvalidRequest("offline-invalid".into()),
        SecEdgarError::Authentication("offline-identification".into()),
        SecEdgarError::Unsupported("offline-no-document-body".into()),
        SecEdgarError::Decode("offline-json-invalid".into()),
        SecEdgarError::Protocol("offline-submissions-invalid".into()),
        SecEdgarError::Core(CoreError::InvalidRequest("offline-core-invalid".into())),
    ];
    for (index, error) in errors.into_iter().enumerate() {
        let expected = match index {
            0 => ServiceError::InvalidRequest("offline-invalid".into()),
            1 => ServiceError::PermissionDenied(error.to_string()),
            2 => ServiceError::Unsupported {
                operation: Operation::CompanyFilings,
                reason: "offline-no-document-body".into(),
            },
            _ => ServiceError::FailedPrecondition(error.to_string()),
        };
        let calls = AtomicUsize::new(0);
        let result = execute_typed(
            command(request_json(), COMPANY_FILINGS_REQUEST_SCHEMA, 1),
            COMPANY_FILINGS_REQUEST_SCHEMA,
            COMPANY_FILINGS_RECORD_SCHEMA,
            "SecEdgar",
            4096,
            |_: &CompanyFilingRequest| {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<DataBatch<CompanyFiling>, _>(error)
            },
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(result.unwrap_err(), expected);
    }
}

#[test]
fn sec_typed_entry_rejects_large_second_record_atomically() {
    let original = batch();
    let first = original.records()[0].clone();
    let second = record(&"offline-large-company-name".repeat(128), 2, None, None);
    let limit = 2048;
    assert!(serde_json::to_vec(&first).unwrap().len() < limit);
    assert!(serde_json::to_vec(&second).unwrap().len() > limit);
    let calls = AtomicUsize::new(0);
    let error = execute_fixture(
        command(request_json(), COMPANY_FILINGS_REQUEST_SCHEMA, 1),
        DataBatch::strict(vec![first, second], original.provenance().clone()),
        limit,
        &calls,
        &RefCell::new(None),
    )
    .unwrap_err();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        matches!(error, ServiceError::ResourceExhausted(ref reason) if reason.contains("exceeds maximum 2048")),
        "{error:?}"
    );
}

#[test]
fn sec_typed_entry_keeps_partial_quality_explicit() {
    let original = batch();
    let partial = DataBatch::best_effort(
        original.records().to_vec(),
        original.provenance().clone(),
        vec!["offline page missing".into()],
    )
    .unwrap();
    let calls = AtomicUsize::new(0);
    let result = execute_fixture(
        command(request_json(), COMPANY_FILINGS_REQUEST_SCHEMA, 1),
        partial,
        4096,
        &calls,
        &RefCell::new(None),
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!result.complete);
    assert_eq!(result.batch_id, BATCH_ID);
    assert_eq!(result.records.len(), original.records().len());
}
