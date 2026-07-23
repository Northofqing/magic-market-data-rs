use magic_market_core::{
    verify_admitted_batch, verify_verified_empty, DataBatch, ProbeAdmissionError,
    ProbeAdmissionPolicy, ProbeStatus, Provenance, ProviderId, SourceEvidence, VerifiedEmpty,
};
use std::time::Duration;

#[derive(Debug)]
struct Record {
    identity: &'static str,
    evidence: SourceEvidence,
}

fn evidence(
    provider: ProviderId,
    observed_at: &str,
    source_at: &str,
    batch_id: &str,
) -> SourceEvidence {
    SourceEvidence::new(provider, observed_at, batch_id)
        .unwrap()
        .with_source_at(source_at)
        .unwrap()
}

fn provenance(observed_at: &str, source_at: &str, batch_id: &str) -> Provenance {
    Provenance::new("fixture", observed_at)
        .unwrap()
        .with_source_at(source_at)
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap()
}

fn policy() -> ProbeAdmissionPolicy {
    ProbeAdmissionPolicy::new(ProviderId::Tonghuashun)
        .require_source_at()
        .with_max_source_age(Duration::from_secs(3_600))
        .unwrap()
}

#[test]
fn stable_machine_states_do_not_conflate_diagnostics_with_admission() {
    assert_eq!(ProbeStatus::Admitted.as_str(), "admitted");
    assert_eq!(ProbeStatus::VerifiedEmpty.as_str(), "verified_empty");
    assert_eq!(
        ProbeStatus::DiagnosticCompleteUnadmitted.as_str(),
        "diagnostic_complete_unadmitted"
    );
    assert_eq!(
        ProbeStatus::SkippedMissingSecret.as_str(),
        "skipped_missing_secret"
    );
    assert_eq!(ProbeStatus::Failed.as_str(), "failed");
    assert!(ProbeStatus::Admitted.satisfies_capability());
    assert!(ProbeStatus::VerifiedEmpty.satisfies_capability());
    assert!(!ProbeStatus::DiagnosticCompleteUnadmitted.satisfies_capability());
    assert!(!ProbeStatus::SkippedMissingSecret.satisfies_capability());
    assert!(!ProbeStatus::Failed.satisfies_capability());
}

#[test]
fn complete_non_empty_batch_with_matching_evidence_is_admitted() {
    let observed = "2026-07-23T10:00:00+08:00";
    let source = "2026-07-23T09:59:30+08:00";
    let batch_id = "ths:consensus:1";
    let batch = DataBatch::strict(
        vec![Record {
            identity: "600519.SH",
            evidence: evidence(ProviderId::Tonghuashun, observed, source, batch_id),
        }],
        provenance(observed, source, batch_id),
    );

    let status = verify_admitted_batch(
        &batch,
        &policy(),
        |record| &record.evidence,
        |record| record.identity.to_owned(),
    )
    .unwrap();

    assert_eq!(status, ProbeStatus::Admitted);
}

#[test]
fn millisecond_prefixed_observation_time_is_checked_not_ignored() {
    let observed = "unix-ms:4102444800000";
    let source = "2026-07-23";
    let batch_id = "eastmoney:reports:1";
    let batch = DataBatch::strict(
        vec![Record {
            identity: "AP-1",
            evidence: evidence(ProviderId::Eastmoney, observed, source, batch_id),
        }],
        provenance(observed, source, batch_id),
    );
    let policy = ProbeAdmissionPolicy::new(ProviderId::Eastmoney).require_source_at();

    assert_eq!(
        verify_admitted_batch(
            &batch,
            &policy,
            |record| &record.evidence,
            |record| record.identity.to_owned()
        )
        .unwrap(),
        ProbeStatus::Admitted
    );
}

#[test]
fn ordinary_empty_incomplete_and_duplicate_batches_fail() {
    let observed = "2026-07-23T10:00:00+08:00";
    let source = "2026-07-23T09:59:30+08:00";
    let batch_id = "ths:consensus:1";

    let empty = DataBatch::<Record>::strict(vec![], provenance(observed, source, batch_id));
    assert!(matches!(
        verify_admitted_batch(
            &empty,
            &policy(),
            |record| &record.evidence,
            |record| { record.identity.to_owned() }
        ),
        Err(ProbeAdmissionError::EmptyBatch)
    ));

    let incomplete = DataBatch::best_effort(
        vec![Record {
            identity: "600519.SH",
            evidence: evidence(ProviderId::Tonghuashun, observed, source, batch_id),
        }],
        provenance(observed, source, batch_id),
        vec!["missing estimate".into()],
    )
    .unwrap();
    assert!(matches!(
        verify_admitted_batch(
            &incomplete,
            &policy(),
            |record| &record.evidence,
            |record| record.identity.to_owned()
        ),
        Err(ProbeAdmissionError::IncompleteQuality { .. })
    ));

    let duplicate = DataBatch::strict(
        vec![
            Record {
                identity: "600519.SH",
                evidence: evidence(ProviderId::Tonghuashun, observed, source, batch_id),
            },
            Record {
                identity: "600519.SH",
                evidence: evidence(ProviderId::Tonghuashun, observed, source, batch_id),
            },
        ],
        provenance(observed, source, batch_id),
    );
    assert!(matches!(
        verify_admitted_batch(
            &duplicate,
            &policy(),
            |record| &record.evidence,
            |record| record.identity.to_owned()
        ),
        Err(ProbeAdmissionError::DuplicateIdentity { identity })
            if identity == "600519.SH"
    ));
}

#[test]
fn mismatched_future_and_stale_record_evidence_fail() {
    let observed = "2026-07-23T10:00:00+08:00";
    let source = "2026-07-23T09:59:30+08:00";
    let batch_id = "ths:consensus:1";

    let mismatch = DataBatch::strict(
        vec![Record {
            identity: "600519.SH",
            evidence: evidence(
                ProviderId::Tonghuashun,
                observed,
                source,
                "ths:consensus:other",
            ),
        }],
        provenance(observed, source, batch_id),
    );
    assert!(matches!(
        verify_admitted_batch(
            &mismatch,
            &policy(),
            |record| &record.evidence,
            |record| record.identity.to_owned()
        ),
        Err(ProbeAdmissionError::BatchIdMismatch { .. })
    ));

    let future = DataBatch::strict(
        vec![Record {
            identity: "600519.SH",
            evidence: evidence(
                ProviderId::Tonghuashun,
                observed,
                "2026-07-23T10:00:01+08:00",
                batch_id,
            ),
        }],
        provenance(observed, "2026-07-23T10:00:01+08:00", batch_id),
    );
    assert!(matches!(
        verify_admitted_batch(
            &future,
            &policy(),
            |record| &record.evidence,
            |record| record.identity.to_owned()
        ),
        Err(ProbeAdmissionError::FutureSourceTime { .. })
    ));

    let stale = DataBatch::strict(
        vec![Record {
            identity: "600519.SH",
            evidence: evidence(
                ProviderId::Tonghuashun,
                observed,
                "2026-07-23T08:59:59+08:00",
                batch_id,
            ),
        }],
        provenance(observed, "2026-07-23T08:59:59+08:00", batch_id),
    );
    assert!(matches!(
        verify_admitted_batch(
            &stale,
            &policy(),
            |record| &record.evidence,
            |record| record.identity.to_owned()
        ),
        Err(ProbeAdmissionError::StaleSourceTime { .. })
    ));

    let malformed_numeric = DataBatch::strict(
        vec![Record {
            identity: "600519.SH",
            evidence: evidence(
                ProviderId::Tonghuashun,
                observed,
                "1780000000.trailing",
                batch_id,
            ),
        }],
        provenance(observed, "1780000000.trailing", batch_id),
    );
    assert!(matches!(
        verify_admitted_batch(
            &malformed_numeric,
            &policy(),
            |record| &record.evidence,
            |record| record.identity.to_owned()
        ),
        Err(ProbeAdmissionError::InvalidTimestamp {
            field: "source_at",
            ..
        })
    ));
}

#[test]
fn source_proven_verified_empty_is_the_only_empty_success() {
    let observed = "2026-07-23T10:00:00+08:00";
    let source = "2026-07-23T09:59:30+08:00";
    let batch_id = "ths:consensus:empty";
    let empty = VerifiedEmpty::new(
        "consensus",
        "600396.SH",
        "source explicitly reports no current institutional forecast",
        evidence(ProviderId::Tonghuashun, observed, source, batch_id),
        provenance(observed, source, batch_id),
    )
    .unwrap();

    assert_eq!(
        verify_verified_empty(&empty, &policy()).unwrap(),
        ProbeStatus::VerifiedEmpty
    );
    assert_eq!(empty.family(), "consensus");
    assert_eq!(empty.request_identity(), "600396.SH");
}
