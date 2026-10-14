use magic_market_core::{
    verify_admitted_time_series_batch, AssetClass, DataBatch, Exchange, FlowInterval, FlowScope,
    FundFlowPoint, InstrumentId, Money, NonEmptyText, ProbeAdmissionError, ProbeAdmissionPolicy,
    ProbeStatus, Provenance, ProviderId, SourceEvidence,
};

#[test]
fn time_series_admission_rejects_a_foreign_provider_between_valid_fund_flow_points() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let observed_at = "2026-07-23T10:03:00Z";
    let batch_id = "offline:fund-flow:middle-provider";
    let window = |middle_provider| {
        let records = [
            ("2026-07-23T10:00:00Z", ProviderId::Eastmoney, 1.25),
            ("2026-07-23T10:01:00Z", middle_provider, -2.5),
            ("2026-07-23T10:02:00Z", ProviderId::Eastmoney, 0.0),
        ]
        .into_iter()
        .map(|(source_at, provider, main_net)| FundFlowPoint {
            scope: FlowScope::Instrument(instrument.clone()),
            interval: FlowInterval::Minute1,
            period_at: NonEmptyText::new(source_at).unwrap(),
            main_net: Some(Money::new(main_net).unwrap()),
            main_ratio: None,
            super_large_net: None,
            large_net: None,
            medium_net: None,
            small_net: None,
            evidence: SourceEvidence::new(provider, observed_at, batch_id)
                .unwrap()
                .with_source_at(source_at)
                .unwrap(),
        })
        .collect();
        let provenance = Provenance::new("offline-fund-flow-window", observed_at)
            .unwrap()
            .with_source_at("2026-07-23T10:02:00Z")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap();
        DataBatch::strict(records, provenance)
    };
    let policy = ProbeAdmissionPolicy::new(ProviderId::Eastmoney).require_source_at();
    let admitted = window(ProviderId::Eastmoney);
    assert_eq!(
        verify_admitted_time_series_batch(
            &admitted,
            &policy,
            |point| &point.evidence,
            |point| point.period_at.as_str().to_owned(),
        ),
        Ok(ProbeStatus::Admitted),
        "the complete ordered single-provider control window must be admitted"
    );

    // Change only the middle record's provider. A valid first point and valid
    // final point must not hide foreign evidence inside an otherwise valid window.
    let mixed = window(ProviderId::Tencent);
    assert_eq!(
        verify_admitted_time_series_batch(
            &mixed,
            &policy,
            |point| &point.evidence,
            |point| point.period_at.as_str().to_owned(),
        ),
        Err(ProbeAdmissionError::ProviderMismatch {
            expected: ProviderId::Eastmoney,
            actual: ProviderId::Tencent,
        }),
        "every point must belong to the expected provider, not only the endpoints"
    );
}

#[test]
fn time_series_admission_rejects_incoherent_fund_flow_batch_envelopes_without_mutation() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let point = |source_at: &str, main_net: f64, batch_id: &str| FundFlowPoint {
        scope: FlowScope::Instrument(instrument.clone()),
        interval: FlowInterval::Minute1,
        period_at: NonEmptyText::new(source_at).unwrap(),
        main_net: Some(Money::new(main_net).unwrap()),
        main_ratio: None,
        super_large_net: None,
        large_net: None,
        medium_net: None,
        small_net: None,
        evidence: SourceEvidence::new(ProviderId::Eastmoney, "2026-07-23T10:03:00Z", batch_id)
            .unwrap()
            .with_source_at(source_at)
            .unwrap(),
    };
    let control = DataBatch::strict(
        vec![
            point(
                "2026-07-23T10:00:00Z",
                125_000.0,
                "offline:fund-flow:envelope-v1",
            ),
            point(
                "2026-07-23T10:01:00Z",
                -250_000.0,
                "offline:fund-flow:envelope-v1",
            ),
            point("2026-07-23T10:02:00Z", 0.0, "offline:fund-flow:envelope-v1"),
        ],
        Provenance::new("offline-fund-flow-envelope", "2026-07-23T10:03:00Z")
            .unwrap()
            .with_source_at("2026-07-23T10:02:00Z")
            .unwrap()
            .with_batch_id("offline:fund-flow:envelope-v1")
            .unwrap(),
    );
    let policy = ProbeAdmissionPolicy::new(ProviderId::Eastmoney).require_source_at();
    let control_before = control.clone();
    assert_eq!(
        verify_admitted_time_series_batch(
            &control,
            &policy,
            |point| &point.evidence,
            |point| point.period_at.as_str().to_owned(),
        ),
        Ok(ProbeStatus::Admitted),
    );
    assert_eq!(
        control, control_before,
        "admission must not rewrite the window"
    );
    assert!(control.quality().is_complete());
    assert_eq!(control.quality().issues(), &[] as &[String]);
    assert_eq!(control.provenance().source(), "offline-fund-flow-envelope");
    assert_eq!(control.provenance().fetched_at(), "2026-07-23T10:03:00Z");
    assert_eq!(
        control.provenance().source_at(),
        Some("2026-07-23T10:02:00Z")
    );
    assert_eq!(
        control.provenance().batch_id(),
        Some("offline:fund-flow:envelope-v1")
    );
    assert_eq!(
        control
            .records()
            .iter()
            .map(|point| (point.period_at.as_str(), point.main_net.map(Money::get),))
            .collect::<Vec<_>>(),
        vec![
            ("2026-07-23T10:00:00Z", Some(125_000.0)),
            ("2026-07-23T10:01:00Z", Some(-250_000.0)),
            ("2026-07-23T10:02:00Z", Some(0.0)),
        ],
    );
    assert_eq!(
        control
            .records()
            .iter()
            .map(|point| (
                point.evidence.provider(),
                point.evidence.observed_at(),
                point.evidence.batch_id(),
                point.evidence.source_at(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                ProviderId::Eastmoney,
                "2026-07-23T10:03:00Z",
                "offline:fund-flow:envelope-v1",
                Some("2026-07-23T10:00:00Z")
            ),
            (
                ProviderId::Eastmoney,
                "2026-07-23T10:03:00Z",
                "offline:fund-flow:envelope-v1",
                Some("2026-07-23T10:01:00Z")
            ),
            (
                ProviderId::Eastmoney,
                "2026-07-23T10:03:00Z",
                "offline:fund-flow:envelope-v1",
                Some("2026-07-23T10:02:00Z")
            ),
        ],
    );
    for point in control.records() {
        assert_eq!(
            point.scope,
            FlowScope::Instrument(
                InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
            )
        );
        assert_eq!(point.interval, FlowInterval::Minute1);
        assert_eq!(point.main_ratio, None);
        assert_eq!(point.super_large_net, None);
        assert_eq!(point.large_net, None);
        assert_eq!(point.medium_net, None);
        assert_eq!(point.small_net, None);
    }

    let incomplete = DataBatch::best_effort(
        control.records().to_vec(),
        control.provenance().clone(),
        vec!["source reports an omitted point".to_owned()],
    )
    .unwrap();
    assert_eq!(incomplete.records(), control.records());
    assert!(!incomplete.quality().is_complete());
    assert_eq!(
        incomplete.quality().issues(),
        &["source reports an omitted point".to_owned()]
    );

    let sibling = DataBatch::strict(
        vec![
            control.records()[0].clone(),
            point(
                "2026-07-23T10:01:00Z",
                -250_000.0,
                "offline:fund-flow:sibling-v1",
            ),
            control.records()[2].clone(),
        ],
        control.provenance().clone(),
    );
    assert_eq!(sibling.records()[0], control.records()[0]);
    assert_eq!(sibling.records()[2], control.records()[2]);
    assert_eq!(
        sibling.records()[1],
        FundFlowPoint {
            evidence: SourceEvidence::new(
                ProviderId::Eastmoney,
                "2026-07-23T10:03:00Z",
                "offline:fund-flow:sibling-v1",
            )
            .unwrap()
            .with_source_at("2026-07-23T10:01:00Z")
            .unwrap(),
            ..control.records()[1].clone()
        }
    );

    // Equal adjacent source times are nondecreasing. The last point and batch
    // source remain 10:02, so neither ordering nor final-source mismatch masks
    // the repeated business identity at 10:00.
    let duplicate = DataBatch::strict(
        vec![
            control.records()[0].clone(),
            control.records()[0].clone(),
            control.records()[2].clone(),
        ],
        control.provenance().clone(),
    );
    assert_eq!(duplicate.records()[1], control.records()[0]);
    assert_eq!(duplicate.records()[2], control.records()[2]);

    for (case, candidate, expected_error) in [
        (
            "IncompleteQuality",
            incomplete,
            ProbeAdmissionError::IncompleteQuality {
                issues: vec!["source reports an omitted point".to_owned()],
            },
        ),
        (
            "middleBatchIdMismatch",
            sibling,
            ProbeAdmissionError::BatchIdMismatch {
                expected: "offline:fund-flow:envelope-v1".to_owned(),
                actual: "offline:fund-flow:sibling-v1".to_owned(),
            },
        ),
        (
            "DuplicateIdentity",
            duplicate,
            ProbeAdmissionError::DuplicateIdentity {
                identity: "2026-07-23T10:00:00Z".to_owned(),
            },
        ),
    ] {
        assert_eq!(candidate.records().len(), 3, "{case}");
        assert_eq!(candidate.provenance(), control.provenance(), "{case}");
        let before = candidate.clone();
        assert_eq!(
            verify_admitted_time_series_batch(
                &candidate,
                &policy,
                |point| &point.evidence,
                |point| point.period_at.as_str().to_owned(),
            ),
            Err(expected_error),
            "{case}: only the exact batch-envelope refusal is acceptable",
        );
        assert_eq!(
            candidate, before,
            "{case}: rejection must preserve the entire window"
        );
    }
    assert_eq!(
        control, control_before,
        "negative windows must not alter the control"
    );
}
