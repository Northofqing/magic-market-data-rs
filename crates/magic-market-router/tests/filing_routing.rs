use magic_market_core::{
    CompanyFiling, CompanyFilingRequest, CompanyFilingsProvider, DataBatch, HttpsUrl, IsoDate,
    NonEmptyText, PositiveU32, Provenance, ProviderId, SecAccessionNumber, SecCompanyIdentity,
    SecPrimaryDocument, SourceEvidence,
};
use magic_market_router::{
    company_filing_source, AcceptancePolicy, AttemptStatus, CompanyFilingRouter, FailureAction,
    FailureKind, SourceError,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
#[error("fixture")]
struct FixtureError;

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Transport, "fixture transport")
}

fn classify_stop(_: FixtureError) -> SourceError {
    SourceError::stop(FailureKind::InvalidRequest, "fixture terminal")
}

struct FilingFixture {
    result: Result<DataBatch<CompanyFiling>, FixtureError>,
    calls: Arc<AtomicUsize>,
}

impl CompanyFilingsProvider for FilingFixture {
    type Error = FixtureError;

    fn company_filings(
        &self,
        _request: &CompanyFilingRequest,
    ) -> Result<DataBatch<CompanyFiling>, Self::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}

fn company(cik: &str, ticker: Option<&str>) -> SecCompanyIdentity {
    SecCompanyIdentity::new(cik, ticker).unwrap()
}

fn request(companies: Vec<SecCompanyIdentity>, max_records: u32) -> CompanyFilingRequest {
    CompanyFilingRequest::new(
        companies,
        vec![NonEmptyText::new("10-K").unwrap()],
        Some(IsoDate::new("2025-01-01").unwrap()),
        Some(IsoDate::new("2026-12-31").unwrap()),
        PositiveU32::new(max_records).unwrap(),
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn filing(
    company: SecCompanyIdentity,
    form: &str,
    date: &str,
    accepted_at: Option<&str>,
    accession: &str,
    provider: ProviderId,
    batch_id: &str,
) -> CompanyFiling {
    let accession = SecAccessionNumber::new(accession).unwrap();
    let document = SecPrimaryDocument::new("report.htm").unwrap();
    let cik_path = company.cik().trim_start_matches('0').to_owned();
    let accession_path = accession.without_hyphens();
    let mut evidence = SourceEvidence::new(provider, "2026-07-29T12:00:00Z", batch_id).unwrap();
    if let Some(accepted_at) = accepted_at {
        evidence = evidence.with_source_at(accepted_at).unwrap();
    }
    CompanyFiling::new(
        company,
        "Fixture Corp",
        form,
        IsoDate::new(date).unwrap(),
        None,
        accession,
        document,
        HttpsUrl::new(format!(
            "https://www.sec.gov/Archives/edgar/data/{cik_path}/{accession_path}/{accession_path}-index.html"
        ))
        .unwrap(),
        HttpsUrl::new(format!(
            "https://www.sec.gov/Archives/edgar/data/{cik_path}/{accession_path}/report.htm"
        ))
        .unwrap(),
        accepted_at.map(|value| NonEmptyText::new(value).unwrap()),
        evidence,
    )
    .unwrap()
}

fn batch(records: Vec<CompanyFiling>, batch_id: &str) -> DataBatch<CompanyFiling> {
    DataBatch::strict(
        records,
        Provenance::new("sec", "2026-07-29T12:00:00Z")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    )
}

fn router_for(
    provider_id: ProviderId,
    result: Result<DataBatch<CompanyFiling>, FixtureError>,
) -> CompanyFilingRouter {
    let mut router = CompanyFilingRouter::new(AcceptancePolicy::new());
    router
        .register(company_filing_source(
            provider_id,
            Arc::new(FilingFixture {
                result,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            classify,
        ))
        .unwrap();
    router
}

fn family_failure(
    request: &CompanyFilingRequest,
    records: Vec<CompanyFiling>,
) -> (FailureKind, FailureAction) {
    let router = router_for(ProviderId::SecEdgar, Ok(batch(records, "batch")));
    match router.route(request).unwrap_err().attempts()[0].status() {
        AttemptStatus::Failed { kind, action, .. } => (*kind, *action),
        status => panic!("expected filing validation failure, got {status:?}"),
    }
}

#[test]
fn filing_router_accepts_canonical_requested_records() {
    let apple = company("320193", Some("AAPL"));
    let microsoft = company("789019", Some("MSFT"));
    let request = request(vec![apple.clone(), microsoft.clone()], 4);
    let records = vec![
        filing(
            apple.clone(),
            "10-K",
            "2026-01-30",
            Some("2026-01-30T18:00:00Z"),
            "0000320193-26-000001",
            ProviderId::SecEdgar,
            "batch",
        ),
        filing(
            apple,
            "10-K",
            "2025-01-31",
            Some("2025-01-31T18:00:00Z"),
            "0000320193-25-000001",
            ProviderId::SecEdgar,
            "batch",
        ),
        filing(
            microsoft,
            "10-K",
            "2026-07-30",
            Some("2026-07-30T18:00:00Z"),
            "0000789019-26-000001",
            ProviderId::SecEdgar,
            "batch",
        ),
    ];
    let router = router_for(ProviderId::SecEdgar, Ok(batch(records, "batch")));
    assert_eq!(
        router.route(&request).unwrap().selected_provider(),
        ProviderId::SecEdgar
    );
}

#[test]
fn filing_router_rejects_unrequested_company_form_date_and_ticker_contradiction_as_evidence() {
    let apple = company("320193", Some("AAPL"));
    let request = request(vec![apple.clone()], 4);
    let cases = vec![
        filing(
            company("789019", Some("MSFT")),
            "10-K",
            "2026-01-30",
            None,
            "0000789019-26-000001",
            ProviderId::SecEdgar,
            "batch",
        ),
        filing(
            apple.clone(),
            "8-K",
            "2026-01-30",
            None,
            "0000320193-26-000002",
            ProviderId::SecEdgar,
            "batch",
        ),
        filing(
            apple.clone(),
            "10-K",
            "2024-12-31",
            None,
            "0000320193-24-000003",
            ProviderId::SecEdgar,
            "batch",
        ),
        filing(
            company("320193", Some("APPL")),
            "10-K",
            "2026-01-30",
            None,
            "0000320193-26-000004",
            ProviderId::SecEdgar,
            "batch",
        ),
    ];
    for record in cases {
        assert_eq!(
            family_failure(&request, vec![record]),
            (FailureKind::Evidence, FailureAction::TryNext)
        );
    }
}

#[test]
fn filing_router_rejects_cardinality_duplicates_and_each_order_dimension_as_quality() {
    let apple = company("320193", Some("AAPL"));
    let microsoft = company("789019", Some("MSFT"));
    let roomy = request(vec![apple.clone(), microsoft.clone()], 10);
    let duplicate = filing(
        apple.clone(),
        "10-K",
        "2026-01-30",
        None,
        "0000320193-26-000001",
        ProviderId::SecEdgar,
        "batch",
    );
    assert_eq!(
        family_failure(&roomy, vec![duplicate.clone(), duplicate]).0,
        FailureKind::Quality
    );

    let tight = request(vec![apple.clone()], 1);
    assert_eq!(
        family_failure(
            &tight,
            vec![
                filing(
                    apple.clone(),
                    "10-K",
                    "2026-01-30",
                    None,
                    "0000320193-26-000002",
                    ProviderId::SecEdgar,
                    "batch",
                ),
                filing(
                    apple.clone(),
                    "10-K",
                    "2025-01-30",
                    None,
                    "0000320193-25-000002",
                    ProviderId::SecEdgar,
                    "batch",
                ),
            ]
        )
        .0,
        FailureKind::Quality
    );

    let company_position = vec![
        filing(
            microsoft,
            "10-K",
            "2026-01-30",
            None,
            "0000789019-26-000002",
            ProviderId::SecEdgar,
            "batch",
        ),
        filing(
            apple.clone(),
            "10-K",
            "2026-01-30",
            None,
            "0000320193-26-000003",
            ProviderId::SecEdgar,
            "batch",
        ),
    ];
    assert_eq!(
        family_failure(&roomy, company_position).0,
        FailureKind::Quality
    );

    let filing_date = vec![
        filing(
            apple.clone(),
            "10-K",
            "2025-01-30",
            None,
            "0000320193-25-000004",
            ProviderId::SecEdgar,
            "batch",
        ),
        filing(
            apple.clone(),
            "10-K",
            "2026-01-30",
            None,
            "0000320193-26-000004",
            ProviderId::SecEdgar,
            "batch",
        ),
    ];
    assert_eq!(family_failure(&roomy, filing_date).0, FailureKind::Quality);

    let acceptance_time = vec![
        filing(
            apple.clone(),
            "10-K",
            "2026-01-30",
            Some("2026-01-30T17:00:00Z"),
            "0000320193-26-000005",
            ProviderId::SecEdgar,
            "batch",
        ),
        filing(
            apple,
            "10-K",
            "2026-01-30",
            Some("2026-01-30T18:00:00Z"),
            "0000320193-26-000006",
            ProviderId::SecEdgar,
            "batch",
        ),
    ];
    assert_eq!(
        family_failure(&roomy, acceptance_time).0,
        FailureKind::Quality
    );
}

#[test]
fn filing_router_leaves_wrong_provider_and_batch_id_to_generic_rejection() {
    let apple = company("320193", Some("AAPL"));
    let request = request(vec![apple.clone()], 2);
    let cases = [
        (
            filing(
                apple.clone(),
                "10-K",
                "2026-01-30",
                None,
                "0000320193-26-000010",
                ProviderId::Fred,
                "batch",
            ),
            "batch",
        ),
        (
            filing(
                apple,
                "10-K",
                "2026-01-30",
                None,
                "0000320193-26-000011",
                ProviderId::SecEdgar,
                "record-batch",
            ),
            "batch",
        ),
    ];
    for (record, provenance_batch) in cases {
        let router = router_for(
            ProviderId::SecEdgar,
            Ok(batch(vec![record], provenance_batch)),
        );
        assert!(matches!(
            router.route(&request).unwrap_err().attempts()[0].status(),
            AttemptStatus::Rejected {
                kind: FailureKind::Evidence,
                ..
            }
        ));
    }
}

#[test]
fn filing_router_preserves_provider_failure_action() {
    let request = request(vec![company("320193", Some("AAPL"))], 1);
    let first_calls = Arc::new(AtomicUsize::new(0));
    let second_calls = Arc::new(AtomicUsize::new(0));
    let mut router = CompanyFilingRouter::new(AcceptancePolicy::new());
    router
        .register(company_filing_source(
            ProviderId::SecEdgar,
            Arc::new(FilingFixture {
                result: Err(FixtureError),
                calls: Arc::clone(&first_calls),
            }),
            classify_stop,
        ))
        .unwrap();
    router
        .register(company_filing_source(
            ProviderId::Fred,
            Arc::new(FilingFixture {
                result: Err(FixtureError),
                calls: Arc::clone(&second_calls),
            }),
            classify,
        ))
        .unwrap();
    let error = router.route(&request).unwrap_err();
    assert_eq!(error.attempts().len(), 1);
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);
    assert_eq!(second_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::InvalidRequest,
            action: FailureAction::Stop,
            ..
        }
    ));
}

#[test]
fn filing_router_has_no_default_route_and_non_sec_routes_require_explicit_registration() {
    let request = request(vec![company("320193", Some("AAPL"))], 1);
    let empty = CompanyFilingRouter::new(AcceptancePolicy::new());
    assert!(empty.provider_ids().is_empty());
    assert!(empty.route(&request).unwrap_err().attempts().is_empty());

    let record = filing(
        company("320193", Some("AAPL")),
        "10-K",
        "2026-01-30",
        None,
        "0000320193-26-000020",
        ProviderId::Fred,
        "batch",
    );
    let explicit = router_for(ProviderId::Fred, Ok(batch(vec![record], "batch")));
    assert_eq!(explicit.provider_ids(), vec![ProviderId::Fred]);
    assert_eq!(
        explicit.route(&request).unwrap().selected_provider(),
        ProviderId::Fred
    );
}
