#[path = "internal/probe_time_series_provider_contract.rs"]
mod time_series_provider_contract;

use magic_market_core::{
    verify_admitted_batch, verify_admitted_time_series_batch, verify_verified_empty, DataBatch,
    EvidenceTimestamp, ProbeAdmissionError, ProbeAdmissionPolicy, ProbeStatus, Provenance,
    ProviderId, SourceEvidence, VerifiedEmpty,
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
fn evidence_timestamps_preserve_nanoseconds_and_reject_excess_precision() {
    let source = EvidenceTimestamp::parse_instant("2026-07-23T10:00:00Z").unwrap();
    let exact = EvidenceTimestamp::parse_instant("2026-07-23T10:00:05Z").unwrap();
    let over = EvidenceTimestamp::parse_instant("2026-07-23T10:00:05.000000001Z").unwrap();
    let epoch_over = EvidenceTimestamp::parse_instant("1784786405.000000001").unwrap();
    let epoch_source = EvidenceTimestamp::parse_instant("1784786400").unwrap();

    assert_eq!(exact.duration_since(source), Some(Duration::from_secs(5)));
    assert_eq!(
        over.duration_since(source),
        Some(Duration::from_secs(5) + Duration::from_nanos(1))
    );
    assert_eq!(
        epoch_over.duration_since(epoch_source),
        Some(Duration::from_secs(5) + Duration::from_nanos(1))
    );
    for invalid in [
        "20240101",
        "2026-07-23T10:00:05.0000000001Z",
        "1784786405.0000000001",
    ] {
        assert!(
            EvidenceTimestamp::parse_instant(invalid).is_err(),
            "excess timestamp precision was silently accepted: {invalid}"
        );
    }
}

#[test]
fn maximum_source_age_requires_an_unambiguous_source_instant() {
    let policy = ProbeAdmissionPolicy::new(ProviderId::Eastmoney)
        .with_max_source_age(Duration::from_secs(5))
        .unwrap();
    let batch_id = "eastmoney:strict-instant";

    let evidence_without_source =
        SourceEvidence::new(ProviderId::Eastmoney, "2026-07-23T10:00:00Z", batch_id).unwrap();
    let provenance_without_source = Provenance::new("fixture", "2026-07-23T10:00:00Z")
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap();
    let missing = DataBatch::strict(
        vec![Record {
            identity: "AP-1",
            evidence: evidence_without_source,
        }],
        provenance_without_source,
    );
    assert!(matches!(
        verify_admitted_batch(
            &missing,
            &policy,
            |record| &record.evidence,
            |record| record.identity.to_owned()
        ),
        Err(ProbeAdmissionError::MissingSourceTime)
    ));

    for (source, observed) in [
        ("2026-07-23", "2026-07-23"),
        ("2026-07-23T09:59:59", "2026-07-23T10:00:00"),
    ] {
        let candidate = DataBatch::strict(
            vec![Record {
                identity: "AP-1",
                evidence: evidence(ProviderId::Eastmoney, observed, source, batch_id),
            }],
            provenance(observed, source, batch_id),
        );
        assert!(
            matches!(
                verify_admitted_batch(
                    &candidate,
                    &policy,
                    |record| &record.evidence,
                    |record| record.identity.to_owned()
                ),
                Err(ProbeAdmissionError::InvalidTimestamp { .. })
            ),
            "ambiguous max-age timestamp was admitted: source={source} observed={observed}"
        );
    }
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
    assert_eq!(ProbeStatus::Admitted.to_string(), "admitted");
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
fn ordered_time_series_retains_per_record_times_and_latest_batch_time() {
    let observed = "2026-07-23T10:01:00+08:00";
    let batch_id = "eastmoney:fund-flow:1";
    let records = vec![
        Record {
            identity: "09:59",
            evidence: evidence(
                ProviderId::Eastmoney,
                observed,
                "2026-07-23T09:59:00+08:00",
                batch_id,
            ),
        },
        Record {
            identity: "10:00",
            evidence: evidence(
                ProviderId::Eastmoney,
                observed,
                "2026-07-23T10:00:00+08:00",
                batch_id,
            ),
        },
    ];
    let batch = DataBatch::strict(
        records,
        provenance(observed, "2026-07-23T10:00:00+08:00", batch_id),
    );
    let policy = ProbeAdmissionPolicy::new(ProviderId::Eastmoney).require_source_at();
    assert_eq!(
        verify_admitted_time_series_batch(
            &batch,
            &policy,
            |record| &record.evidence,
            |record| record.identity.to_owned(),
        )
        .unwrap(),
        ProbeStatus::Admitted
    );
}

#[test]
fn time_series_rejects_backwards_or_non_latest_batch_time() {
    let observed = "2026-07-23T10:01:00+08:00";
    let batch_id = "eastmoney:fund-flow:2";
    let policy = ProbeAdmissionPolicy::new(ProviderId::Eastmoney).require_source_at();
    let records = vec![
        Record {
            identity: "10:00",
            evidence: evidence(
                ProviderId::Eastmoney,
                observed,
                "2026-07-23T10:00:00+08:00",
                batch_id,
            ),
        },
        Record {
            identity: "09:59",
            evidence: evidence(
                ProviderId::Eastmoney,
                observed,
                "2026-07-23T09:59:00+08:00",
                batch_id,
            ),
        },
    ];
    let backwards = DataBatch::strict(
        records,
        provenance(observed, "2026-07-23T09:59:00+08:00", batch_id),
    );
    assert!(matches!(
        verify_admitted_time_series_batch(
            &backwards,
            &policy,
            |record| &record.evidence,
            |record| record.identity.to_owned(),
        ),
        Err(ProbeAdmissionError::NonMonotonicSourceTime { .. })
    ));

    let ordered = DataBatch::strict(
        vec![Record {
            identity: "10:00",
            evidence: evidence(
                ProviderId::Eastmoney,
                observed,
                "2026-07-23T10:00:00+08:00",
                batch_id,
            ),
        }],
        provenance(observed, "2026-07-23T09:59:00+08:00", batch_id),
    );
    assert!(matches!(
        verify_admitted_time_series_batch(
            &ordered,
            &policy,
            |record| &record.evidence,
            |record| record.identity.to_owned(),
        ),
        Err(ProbeAdmissionError::SourceAtMismatch { .. })
    ));
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

    let future_millisecond = DataBatch::strict(
        vec![Record {
            identity: "600519.SH",
            evidence: evidence(
                ProviderId::Tonghuashun,
                observed,
                "2026-07-23T10:00:00.001+08:00",
                batch_id,
            ),
        }],
        provenance(observed, "2026-07-23T10:00:00.001+08:00", batch_id),
    );
    assert!(matches!(
        verify_admitted_batch(
            &future_millisecond,
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
    assert_eq!(
        empty.reason(),
        "source explicitly reports no current institutional forecast"
    );
    assert_eq!(empty.evidence().provider(), ProviderId::Tonghuashun);
    assert_eq!(empty.provenance().batch_id(), Some(batch_id));
    assert_eq!(
        empty.to_string(),
        "family=consensus request_identity=600396.SH reason=source explicitly reports no current institutional forecast"
    );
}

#[test]
fn policy_and_verified_empty_reject_invalid_configuration_and_evidence() {
    assert!(matches!(
        ProbeAdmissionPolicy::new(ProviderId::Tonghuashun).with_max_source_age(Duration::ZERO),
        Err(magic_market_core::CoreError::InvalidRequest(_))
    ));

    let observed = "2026-07-23T10:00:00+08:00";
    let source = "2026-07-23T09:59:30+08:00";
    let batch_id = "ths:consensus:empty";
    for (family, request_identity, reason) in [
        ("", "600396.SH", "source empty"),
        ("consensus", "", "source empty"),
        ("consensus", "600396.SH", ""),
    ] {
        assert!(VerifiedEmpty::new(
            family,
            request_identity,
            reason,
            evidence(ProviderId::Tonghuashun, observed, source, batch_id),
            provenance(observed, source, batch_id),
        )
        .is_err());
    }

    let mismatched = VerifiedEmpty::new(
        "consensus",
        "600396.SH",
        "source empty",
        evidence(
            ProviderId::Tonghuashun,
            observed,
            source,
            "ths:consensus:other",
        ),
        provenance(observed, source, batch_id),
    );
    assert!(matches!(
        mismatched,
        Err(ProbeAdmissionError::BatchIdMismatch { .. })
    ));
}

#[test]
fn every_evidence_mismatch_is_rejected_explicitly() {
    let observed = "2026-07-23T10:00:00+08:00";
    let source = "2026-07-23T09:59:30+08:00";
    let batch_id = "ths:consensus:1";

    let cases = [
        (
            DataBatch::strict(
                vec![Record {
                    identity: "600519.SH",
                    evidence: evidence(ProviderId::Eastmoney, observed, source, batch_id),
                }],
                provenance(observed, source, batch_id),
            ),
            "provider",
        ),
        (
            DataBatch::strict(
                vec![Record {
                    identity: "600519.SH",
                    evidence: evidence(
                        ProviderId::Tonghuashun,
                        "2026-07-23T10:00:01+08:00",
                        source,
                        batch_id,
                    ),
                }],
                provenance(observed, source, batch_id),
            ),
            "observed",
        ),
        (
            DataBatch::strict(
                vec![Record {
                    identity: "600519.SH",
                    evidence: evidence(
                        ProviderId::Tonghuashun,
                        observed,
                        "2026-07-23T09:59:29+08:00",
                        batch_id,
                    ),
                }],
                provenance(observed, source, batch_id),
            ),
            "source",
        ),
    ];

    for (batch, expected) in cases {
        let error = verify_admitted_batch(
            &batch,
            &policy(),
            |record| &record.evidence,
            |record| record.identity.to_owned(),
        )
        .unwrap_err();
        assert!(
            matches!(
                (&error, expected),
                (ProbeAdmissionError::ProviderMismatch { .. }, "provider")
                    | (ProbeAdmissionError::ObservedAtMismatch { .. }, "observed")
                    | (ProbeAdmissionError::SourceAtMismatch { .. }, "source")
            ),
            "unexpected {expected} mismatch error: {error}"
        );
    }

    assert!(serde_json::from_value::<Provenance>(serde_json::json!({
        "source": "fixture",
        "source_at": source,
        "fetched_at": observed,
        "batch_id": null
    }))
    .is_err());
}

#[test]
fn missing_source_time_and_invalid_identities_are_not_admitted() {
    let observed = "2026-07-23T10:00:00+08:00";
    let batch_id = "ths:consensus:1";
    let no_source = DataBatch::strict(
        vec![Record {
            identity: "600519.SH",
            evidence: SourceEvidence::new(ProviderId::Tonghuashun, observed, batch_id).unwrap(),
        }],
        Provenance::new("fixture", observed)
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    );
    assert!(matches!(
        verify_admitted_batch(
            &no_source,
            &policy(),
            |record| &record.evidence,
            |record| record.identity.to_owned()
        ),
        Err(ProbeAdmissionError::MissingSourceTime)
    ));
    assert_eq!(
        verify_admitted_batch(
            &no_source,
            &ProbeAdmissionPolicy::new(ProviderId::Tonghuashun),
            |record| &record.evidence,
            |record| record.identity.to_owned()
        )
        .unwrap(),
        ProbeStatus::Admitted
    );

    for identity in [" ", "600519.SH\ncontrol"] {
        let batch = DataBatch::strict(
            vec![Record {
                identity,
                evidence: SourceEvidence::new(ProviderId::Tonghuashun, observed, batch_id).unwrap(),
            }],
            Provenance::new("fixture", observed)
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        );
        assert!(matches!(
            verify_admitted_batch(
                &batch,
                &ProbeAdmissionPolicy::new(ProviderId::Tonghuashun),
                |record| &record.evidence,
                |record| record.identity.to_owned()
            ),
            Err(ProbeAdmissionError::EmptyIdentity)
        ));
    }
}

#[test]
fn supported_timestamp_forms_and_calendar_edges_are_verified() {
    let batch_id = "eastmoney:reports:timestamp";
    let accepted = [
        ("1784786400", "1784786400"),
        ("1784786400.123", "1784786400.999"),
        ("2024-02-29", "2024-02-29"),
        ("2026-07-23 10:00:00", "2026-07-23T10:00:00Z"),
        (
            "2026-07-23T10:00:00.123+08:00",
            "2026-07-23T10:00:00.123+08:00",
        ),
        ("2026-07-23T10:00:00-08:00", "2026-07-24T02:00:00Z"),
    ];
    for (source, observed) in accepted {
        let batch = DataBatch::strict(
            vec![Record {
                identity: "AP-1",
                evidence: evidence(ProviderId::Eastmoney, observed, source, batch_id),
            }],
            provenance(observed, source, batch_id),
        );
        assert_eq!(
            verify_admitted_batch(
                &batch,
                &ProbeAdmissionPolicy::new(ProviderId::Eastmoney).require_source_at(),
                |record| &record.evidence,
                |record| record.identity.to_owned()
            )
            .unwrap(),
            ProbeStatus::Admitted,
            "source={source} observed={observed}"
        );
    }

    for invalid in [
        "unix-ms:",
        "unix-ms:1x",
        "2023-02-29",
        "2026-13-01",
        "2026-07-23T10:00",
        "2026-07-23X10:00:00Z",
        "2026-07-23T24:00:00Z",
        "2026-07-23T10:60:00Z",
        "2026-07-23T10:00:60Z",
        "2026-07-23T10:00:00+24:00",
        "2026-07-23T10:00:00+08:60",
        "2026-07-23T10:00:00+0800",
    ] {
        let batch = DataBatch::strict(
            vec![Record {
                identity: "AP-1",
                evidence: evidence(
                    ProviderId::Eastmoney,
                    "2026-07-24T10:00:00+08:00",
                    invalid,
                    batch_id,
                ),
            }],
            provenance("2026-07-24T10:00:00+08:00", invalid, batch_id),
        );
        assert!(
            matches!(
                verify_admitted_batch(
                    &batch,
                    &ProbeAdmissionPolicy::new(ProviderId::Eastmoney).require_source_at(),
                    |record| &record.evidence,
                    |record| record.identity.to_owned()
                ),
                Err(ProbeAdmissionError::InvalidTimestamp {
                    field: "source_at",
                    ..
                })
            ),
            "invalid timestamp admitted: {invalid}"
        );
    }

    let invalid_observed = DataBatch::strict(
        vec![Record {
            identity: "AP-1",
            evidence: evidence(
                ProviderId::Eastmoney,
                "invalid-observed",
                "2026-07-23",
                batch_id,
            ),
        }],
        provenance("invalid-observed", "2026-07-23", batch_id),
    );
    assert!(matches!(
        verify_admitted_batch(
            &invalid_observed,
            &ProbeAdmissionPolicy::new(ProviderId::Eastmoney).require_source_at(),
            |record| &record.evidence,
            |record| record.identity.to_owned()
        ),
        Err(ProbeAdmissionError::InvalidTimestamp {
            field: "observed_at",
            ..
        })
    ));
}

#[test]
fn stale_diagnostics_preserve_nanosecond_precision() {
    let observed = "2026-07-23T10:00:05.000000001+08:00";
    let source = "2026-07-23T10:00:00+08:00";
    let batch_id = "eastmoney:reports:nanosecond-stale";
    let batch = DataBatch::strict(
        vec![Record {
            identity: "AP-1",
            evidence: evidence(ProviderId::Eastmoney, observed, source, batch_id),
        }],
        provenance(observed, source, batch_id),
    );
    let policy = ProbeAdmissionPolicy::new(ProviderId::Eastmoney)
        .require_source_at()
        .with_max_source_age(Duration::from_secs(5))
        .unwrap();

    let error = verify_admitted_batch(
        &batch,
        &policy,
        |record| &record.evidence,
        |record| record.identity.to_owned(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProbeAdmissionError::StaleSourceTime {
            age_nanos: 5_000_000_001,
            max_age_nanos: 5_000_000_000
        }
    ));
    assert_eq!(
        error.to_string(),
        "source_at is stale by 5000000001ns; maximum allowed age is 5000000000ns"
    );
}

#[derive(Clone, Debug)]
struct NewestFirstRecord {
    identity: &'static str,
    normalized_source_at: &'static str,
    evidence: SourceEvidence,
}

fn newest_first_fixture() -> DataBatch<NewestFirstRecord> {
    let observed = "2026-07-23T18:00:05+08:00";
    let batch_id = "newest-first-source-window";
    DataBatch::strict(
        vec![
            NewestFirstRecord {
                identity: "latest",
                normalized_source_at: "2026-07-23T18:00:00+08:00",
                evidence: evidence(
                    ProviderId::Tonghuashun,
                    observed,
                    "2026-07-23 18:00:00",
                    batch_id,
                ),
            },
            NewestFirstRecord {
                identity: "previous",
                normalized_source_at: "2026-07-23T09:59:59Z",
                evidence: evidence(
                    ProviderId::Tonghuashun,
                    observed,
                    "2026-07-23 17:59:59",
                    batch_id,
                ),
            },
        ],
        provenance(observed, "2026-07-23 18:00:00", batch_id),
    )
}

fn verify_newest_first_fixture(
    batch: &DataBatch<NewestFirstRecord>,
    policy: &ProbeAdmissionPolicy,
) -> Result<ProbeStatus, ProbeAdmissionError> {
    magic_market_core::verify_admitted_newest_first_batch(
        batch,
        policy,
        |record| &record.evidence,
        |record| record.normalized_source_at,
        |record| record.identity.to_owned(),
    )
}

#[test]
fn newest_first_admission_preserves_raw_source_and_checks_normalized_instants() {
    let policy = ProbeAdmissionPolicy::new(ProviderId::Tonghuashun)
        .with_max_source_age(Duration::from_secs(5))
        .unwrap();
    let batch = newest_first_fixture();
    assert_eq!(
        verify_newest_first_fixture(&batch, &policy).unwrap(),
        ProbeStatus::Admitted
    );
    assert_eq!(batch.provenance().source_at(), Some("2026-07-23 18:00:00"));
    assert_eq!(
        batch.records()[1].evidence.source_at(),
        Some("2026-07-23 17:59:59")
    );
}

#[test]
fn newest_first_admission_rejects_incoherent_batch_and_record_evidence() {
    let original = newest_first_fixture();
    let policy = ProbeAdmissionPolicy::new(ProviderId::Tonghuashun);
    let observed = original.provenance().fetched_at();
    let batch_id = original.provenance().batch_id().unwrap();
    let raw_source = original.provenance().source_at().unwrap();
    let missing_source = Provenance::new("fixture", observed)
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap();
    for (candidate, expected) in [
        (
            DataBatch::strict(Vec::new(), original.provenance().clone()),
            ProbeAdmissionError::EmptyBatch,
        ),
        (
            DataBatch::best_effort(
                original.records().to_vec(),
                original.provenance().clone(),
                vec!["source window truncated".into()],
            )
            .unwrap(),
            ProbeAdmissionError::IncompleteQuality {
                issues: vec!["source window truncated".into()],
            },
        ),
        (
            DataBatch::strict(original.records().to_vec(), missing_source),
            ProbeAdmissionError::MissingSourceTime,
        ),
    ] {
        assert_eq!(
            verify_newest_first_fixture(&candidate, &policy).unwrap_err(),
            expected
        );
    }

    for (replacement, expected) in [
        (
            evidence(ProviderId::Jin10, observed, raw_source, batch_id),
            ProbeAdmissionError::ProviderMismatch {
                expected: ProviderId::Tonghuashun,
                actual: ProviderId::Jin10,
            },
        ),
        (
            evidence(
                ProviderId::Tonghuashun,
                "2026-07-23T18:00:06+08:00",
                raw_source,
                batch_id,
            ),
            ProbeAdmissionError::ObservedAtMismatch {
                expected: observed.into(),
                actual: "2026-07-23T18:00:06+08:00".into(),
            },
        ),
        (
            evidence(ProviderId::Tonghuashun, observed, raw_source, "other-batch"),
            ProbeAdmissionError::BatchIdMismatch {
                expected: batch_id.into(),
                actual: "other-batch".into(),
            },
        ),
        (
            SourceEvidence::new(ProviderId::Tonghuashun, observed, batch_id).unwrap(),
            ProbeAdmissionError::MissingSourceTime,
        ),
        (
            evidence(
                ProviderId::Tonghuashun,
                observed,
                "2026-07-23 17:59:59",
                batch_id,
            ),
            ProbeAdmissionError::SourceAtMismatch {
                expected: Some(raw_source.into()),
                actual: Some("2026-07-23 17:59:59".into()),
            },
        ),
    ] {
        let mut records = original.records().to_vec();
        records[0].evidence = replacement;
        let candidate = DataBatch::strict(records, original.provenance().clone());
        assert_eq!(
            verify_newest_first_fixture(&candidate, &policy).unwrap_err(),
            expected
        );
    }
}

#[test]
fn newest_first_admission_requires_descending_instants_and_unique_valid_identities() {
    let original = newest_first_fixture();
    let policy = ProbeAdmissionPolicy::new(ProviderId::Tonghuashun);
    let mut ascending = original.records().to_vec();
    ascending[1].normalized_source_at = "2026-07-23T18:00:01+08:00";
    let candidate = DataBatch::strict(ascending, original.provenance().clone());
    assert_eq!(
        verify_newest_first_fixture(&candidate, &policy).unwrap_err(),
        ProbeAdmissionError::NonDescendingSourceTime {
            previous: "2026-07-23T18:00:00+08:00".into(),
            actual: "2026-07-23T18:00:01+08:00".into(),
        }
    );

    // Different stories may share an instant; the identity, not time, is unique.
    let mut same_instant = original.records().to_vec();
    same_instant[1].normalized_source_at = "2026-07-23T10:00:00Z";
    let candidate = DataBatch::strict(same_instant, original.provenance().clone());
    assert_eq!(
        verify_newest_first_fixture(&candidate, &policy).unwrap(),
        ProbeStatus::Admitted
    );

    for invalid_identity in ["", "   ", "story\tbroken", "story\u{007f}broken"] {
        let mut records = original.records().to_vec();
        records[1].identity = invalid_identity;
        let candidate = DataBatch::strict(records, original.provenance().clone());
        assert_eq!(
            verify_newest_first_fixture(&candidate, &policy).unwrap_err(),
            ProbeAdmissionError::EmptyIdentity,
            "invalid business identity admitted: {invalid_identity:?}"
        );
    }
    let mut duplicate = original.records().to_vec();
    duplicate[1].identity = " latest ";
    let candidate = DataBatch::strict(duplicate, original.provenance().clone());
    assert_eq!(
        verify_newest_first_fixture(&candidate, &policy).unwrap_err(),
        ProbeAdmissionError::DuplicateIdentity {
            identity: "latest".into()
        }
    );
}

#[test]
fn newest_first_admission_enforces_instant_freshness_without_rewriting_source_time() {
    let original = newest_first_fixture();
    let policy = ProbeAdmissionPolicy::new(ProviderId::Tonghuashun)
        .with_max_source_age(Duration::from_secs(5))
        .unwrap();
    for invalid in [
        "2026-07-23",
        "2026-07-23T18:00:00",
        "2026-02-30T18:00:00+08:00",
        "not-a-timestamp",
    ] {
        let mut records = original.records().to_vec();
        records[1].normalized_source_at = invalid;
        let candidate = DataBatch::strict(records, original.provenance().clone());
        assert_eq!(
            verify_newest_first_fixture(&candidate, &policy).unwrap_err(),
            ProbeAdmissionError::InvalidTimestamp {
                field: "normalized_source_at",
                value: invalid.into()
            }
        );
    }
    let mut future = original.records().to_vec();
    future[1].normalized_source_at = "2026-07-23T18:00:05.000000001+08:00";
    let candidate = DataBatch::strict(future, original.provenance().clone());
    assert_eq!(
        verify_newest_first_fixture(&candidate, &policy).unwrap_err(),
        ProbeAdmissionError::FutureSourceTime {
            source_at: "2026-07-23T18:00:05.000000001+08:00".into(),
            observed_at: "2026-07-23T18:00:05+08:00".into()
        }
    );
    let later_observation = "2026-07-23T18:00:05.000000001+08:00";
    let batch_id = original.provenance().batch_id().unwrap();
    let mut stale = original.records().to_vec();
    for record in &mut stale {
        record.evidence = evidence(
            ProviderId::Tonghuashun,
            later_observation,
            record.evidence.source_at().unwrap(),
            batch_id,
        );
    }
    let candidate = DataBatch::strict(
        stale,
        provenance(later_observation, "2026-07-23 18:00:00", batch_id),
    );
    assert_eq!(
        verify_newest_first_fixture(&candidate, &policy).unwrap_err(),
        ProbeAdmissionError::StaleSourceTime {
            age_nanos: 5_000_000_001,
            max_age_nanos: 5_000_000_000
        }
    );
    // Without a freshness bound the same ordered window remains source-proven.
    assert_eq!(
        verify_newest_first_fixture(
            &candidate,
            &ProbeAdmissionPolicy::new(ProviderId::Tonghuashun)
        )
        .unwrap(),
        ProbeStatus::Admitted
    );
    let invalid_observation = DataBatch::strict(
        original.records().to_vec(),
        provenance("2026-07-23T18:00:05", "2026-07-23 18:00:00", batch_id),
    );
    assert_eq!(
        verify_newest_first_fixture(&invalid_observation, &policy).unwrap_err(),
        ProbeAdmissionError::InvalidTimestamp {
            field: "observed_at",
            value: "2026-07-23T18:00:05".into()
        }
    );
}

mod fund_flow_time_series_freshness_release {
    use magic_market_core::{
        verify_admitted_time_series_batch, AssetClass, DataBatch, Exchange, FlowInterval,
        FlowScope, FundFlowPoint, InstrumentId, Money, NonEmptyText, ProbeAdmissionError,
        ProbeAdmissionPolicy, ProbeStatus, Provenance, ProviderId, SourceEvidence,
    };
    use std::time::Duration;

    fn fund_flow_window(observed_at: &str, batch_source_at: &str) -> DataBatch<FundFlowPoint> {
        let records = [
            ("2026-07-23T10:00:00Z", 1.25),
            ("2026-07-23T10:01:00Z", -2.5),
        ]
        .into_iter()
        .map(|(source_at, main_net)| FundFlowPoint {
            scope: FlowScope::Instrument(
                InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
            ),
            interval: FlowInterval::Minute1,
            period_at: NonEmptyText::new(source_at).unwrap(),
            main_net: Some(Money::new(main_net).unwrap()),
            main_ratio: None,
            super_large_net: None,
            large_net: None,
            medium_net: None,
            small_net: None,
            evidence: SourceEvidence::new(
                ProviderId::Eastmoney,
                observed_at,
                "eastmoney:fund-flow:latest-point-boundary",
            )
            .unwrap()
            .with_source_at(source_at)
            .unwrap(),
        })
        .collect();
        let provenance = Provenance::new("offline-fund-flow-window", observed_at)
            .unwrap()
            .with_source_at(batch_source_at)
            .unwrap()
            .with_batch_id("eastmoney:fund-flow:latest-point-boundary")
            .unwrap();
        DataBatch::strict(records, provenance)
    }

    #[test]
    fn time_series_freshness_uses_the_final_point_and_preserves_exact_batch_source() {
        let policy = ProbeAdmissionPolicy::new(ProviderId::Eastmoney)
            .with_max_source_age(Duration::from_secs(5))
            .unwrap();
        let boundary = fund_flow_window("2026-07-23T10:01:05Z", "2026-07-23T10:01:00Z");

        // The first point is 65 seconds old; only the final point bounds freshness.
        assert_eq!(
            verify_admitted_time_series_batch(
                &boundary,
                &policy,
                |point| &point.evidence,
                |point| point.period_at.as_str().to_owned(),
            ),
            Ok(ProbeStatus::Admitted)
        );
        assert_eq!(boundary.records().len(), 2);
        assert!(boundary.quality().is_complete());
        assert!(boundary.quality().issues().is_empty());
        assert_eq!(
            boundary.records()[0].period_at.as_str(),
            "2026-07-23T10:00:00Z"
        );
        assert_eq!(
            boundary.records()[1].period_at.as_str(),
            "2026-07-23T10:01:00Z"
        );
        assert_eq!(boundary.records()[0].main_net.unwrap().get(), 1.25);
        assert_eq!(boundary.records()[1].main_net.unwrap().get(), -2.5);
        assert_eq!(
            boundary.records()[0].evidence.source_at(),
            Some("2026-07-23T10:00:00Z")
        );
        assert_eq!(
            boundary.records()[1].evidence.source_at(),
            Some("2026-07-23T10:01:00Z")
        );
        assert_eq!(boundary.provenance().source(), "offline-fund-flow-window");
        assert_eq!(
            boundary.provenance().source_at(),
            Some("2026-07-23T10:01:00Z")
        );
        assert_eq!(boundary.provenance().fetched_at(), "2026-07-23T10:01:05Z");
        assert_eq!(
            boundary.provenance().batch_id(),
            Some("eastmoney:fund-flow:latest-point-boundary")
        );
        for point in boundary.records() {
            assert_eq!(point.evidence.provider(), ProviderId::Eastmoney);
            assert_eq!(point.evidence.observed_at(), "2026-07-23T10:01:05Z");
            assert_eq!(
                point.evidence.batch_id(),
                "eastmoney:fund-flow:latest-point-boundary"
            );
        }

        // Every record and the batch share the shifted observation; no earlier
        // provenance/ordering mismatch can explain the exact stale-age error.
        let stale = fund_flow_window("2026-07-23T10:01:05.000000001Z", "2026-07-23T10:01:00Z");
        assert_eq!(
            verify_admitted_time_series_batch(
                &stale,
                &policy,
                |point| &point.evidence,
                |point| point.period_at.as_str().to_owned(),
            ),
            Err(ProbeAdmissionError::StaleSourceTime {
                age_nanos: 5_000_000_001,
                max_age_nanos: 5_000_000_000,
            })
        );

        // The two strings denote the same instant, but atomic batch evidence
        // must retain the final record's exact source string.
        let equivalent_but_mismatched_source =
            fund_flow_window("2026-07-23T10:01:05Z", "2026-07-23T18:01:00+08:00");
        assert_eq!(
            verify_admitted_time_series_batch(
                &equivalent_but_mismatched_source,
                &policy,
                |point| &point.evidence,
                |point| point.period_at.as_str().to_owned(),
            ),
            Err(ProbeAdmissionError::SourceAtMismatch {
                expected: Some("2026-07-23T18:01:00+08:00".to_owned()),
                actual: Some("2026-07-23T10:01:00Z".to_owned()),
            })
        );
    }
}

mod time_series_clock_public_contract {
    use magic_market_core::{
        verify_admitted_time_series_batch, AssetClass, DataBatch, Exchange, FlowInterval,
        FlowScope, FundFlowPoint, InstrumentId, Money, NonEmptyText, ProbeAdmissionError,
        ProbeAdmissionPolicy, ProbeStatus, Provenance, ProviderId, SourceEvidence,
    };

    fn window(first_source_at: Option<&str>, observed_at: &str) -> DataBatch<FundFlowPoint> {
        let instrument =
            InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
        let records = [
            ("2026-07-23T10:00:00Z", first_source_at, 1.25),
            ("2026-07-23T10:01:00Z", Some("2026-07-23T10:01:00Z"), -2.5),
        ]
        .into_iter()
        .map(|(period_at, source_at, main_net)| {
            let evidence = SourceEvidence::new(
                ProviderId::Eastmoney,
                observed_at,
                "offline:fund-flow:clock-pair",
            )
            .unwrap();
            let evidence = match source_at {
                Some(source_at) => evidence.with_source_at(source_at).unwrap(),
                None => evidence,
            };
            FundFlowPoint {
                scope: FlowScope::Instrument(instrument.clone()),
                interval: FlowInterval::Minute1,
                period_at: NonEmptyText::new(period_at).unwrap(),
                main_net: Some(Money::new(main_net).unwrap()),
                main_ratio: None,
                super_large_net: None,
                large_net: None,
                medium_net: None,
                small_net: None,
                evidence,
            }
        })
        .collect();
        let provenance = Provenance::new("offline-fund-flow-window", observed_at)
            .unwrap()
            .with_source_at("2026-07-23T10:01:00Z")
            .unwrap()
            .with_batch_id("offline:fund-flow:clock-pair")
            .unwrap();
        DataBatch::strict(records, provenance)
    }

    #[test]
    fn time_series_rejects_missing_malformed_and_future_record_clocks() {
        let policy = ProbeAdmissionPolicy::new(ProviderId::Eastmoney).require_source_at();
        let control = window(Some("2026-07-23T10:00:00Z"), "2026-07-23T10:01:00Z");
        assert_eq!(
            verify_admitted_time_series_batch(
                &control,
                &policy,
                |point| &point.evidence,
                |point| point.period_at.as_str().to_owned(),
            ),
            Ok(ProbeStatus::Admitted),
            "the final source instant may equal the observation instant"
        );
        assert_eq!(control.records().len(), 2);
        assert_eq!(control.records()[0].main_net.unwrap().get(), 1.25);
        assert_eq!(control.records()[1].main_net.unwrap().get(), -2.5);
        assert_eq!(
            control.records()[0].evidence.source_at(),
            Some("2026-07-23T10:00:00Z")
        );

        let cases = [
            (
                "missing source time",
                None,
                "2026-07-23T10:01:00Z",
                ProbeAdmissionError::MissingSourceTime,
            ),
            (
                "malformed source time",
                Some("not-a-timestamp"),
                "2026-07-23T10:01:00Z",
                ProbeAdmissionError::InvalidTimestamp {
                    field: "source_at",
                    value: "not-a-timestamp".into(),
                },
            ),
            (
                "malformed but exactly matching observation times",
                Some("2026-07-23T10:00:00Z"),
                "not-a-timestamp",
                ProbeAdmissionError::InvalidTimestamp {
                    field: "observed_at",
                    value: "not-a-timestamp".into(),
                },
            ),
            (
                "source one nanosecond after observation",
                Some("2026-07-23T10:01:00.000000001Z"),
                "2026-07-23T10:01:00Z",
                ProbeAdmissionError::FutureSourceTime {
                    source_at: "2026-07-23T10:01:00.000000001Z".into(),
                    observed_at: "2026-07-23T10:01:00Z".into(),
                },
            ),
        ];
        for (case, first_source_at, observed_at, expected_error) in cases {
            let batch = window(first_source_at, observed_at);
            assert_eq!(batch.records().len(), 2, "{case}");
            assert!(batch.quality().is_complete(), "{case}");
            assert!(batch.quality().issues().is_empty(), "{case}");
            assert_eq!(batch.provenance().fetched_at(), observed_at, "{case}");
            assert_eq!(
                batch.provenance().batch_id(),
                Some("offline:fund-flow:clock-pair"),
                "{case}"
            );
            assert_eq!(
                batch.provenance().source_at(),
                Some("2026-07-23T10:01:00Z"),
                "{case}"
            );
            for point in batch.records() {
                assert_eq!(point.evidence.provider(), ProviderId::Eastmoney, "{case}");
                assert_eq!(point.evidence.observed_at(), observed_at, "{case}");
                assert_eq!(
                    point.evidence.batch_id(),
                    "offline:fund-flow:clock-pair",
                    "{case}"
                );
            }
            let before = batch.clone();
            assert_eq!(
                verify_admitted_time_series_batch(
                    &batch,
                    &policy,
                    |point| &point.evidence,
                    |point| point.period_at.as_str().to_owned(),
                ),
                Err(expected_error),
                "{case}: clock rejection must not be a prior provenance mismatch"
            );
            assert_eq!(batch, before, "{case}: rejection must preserve the window");
            assert_eq!(
                batch.records()[0].evidence.source_at(),
                first_source_at,
                "{case}"
            );
            assert_eq!(
                batch.records()[1].evidence.source_at(),
                Some("2026-07-23T10:01:00Z"),
                "{case}"
            );
            assert_eq!(
                batch.records()[0].period_at.as_str(),
                "2026-07-23T10:00:00Z"
            );
            assert_eq!(
                batch.records()[1].period_at.as_str(),
                "2026-07-23T10:01:00Z"
            );
            assert_eq!(batch.records()[0].main_net.unwrap().get(), 1.25);
            assert_eq!(batch.records()[1].main_net.unwrap().get(), -2.5);
        }
    }
}
