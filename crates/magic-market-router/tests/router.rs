use magic_market_core::{
    AssetClass, DataBatch, DataStatus, Exchange, InstrumentId, Money, Price, Provenance,
    ProviderId, Quantity, Quote,
};
use magic_market_router::{
    AcceptancePolicy, AttemptStatus, FailoverChain, FailureAction, FailureKind, RouterError,
    SourceError, SourceFn,
};
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn batch(
    provider: ProviderId,
    record_batch_id: &str,
    provenance_batch_id: &str,
    source_at: bool,
    issue: Option<&str>,
) -> DataBatch<Quote> {
    let quote = Quote::new(
        instrument(),
        Price::new(15.5).unwrap(),
        Quantity::new(100.0).unwrap(),
        Some(Money::new(155_000.0).unwrap()),
        "observed",
        provider,
        record_batch_id,
    )
    .unwrap();
    let mut provenance = Provenance::new("fixture", "observed")
        .unwrap()
        .with_batch_id(provenance_batch_id)
        .unwrap();
    if source_at {
        provenance = provenance
            .with_source_at("2026-07-23T10:00:00+08:00")
            .unwrap();
    }
    match issue {
        Some(issue) => DataBatch::best_effort(vec![quote], provenance, vec![issue.into()]).unwrap(),
        None => DataBatch::strict(vec![quote], provenance),
    }
}

fn source(
    provider: ProviderId,
    result: Result<DataBatch<Quote>, SourceError>,
) -> SourceFn<[InstrumentId], Quote> {
    SourceFn::new(provider, move |_| result.clone())
}

#[test]
fn retryable_failure_falls_through_and_preserves_trace() {
    let mut chain = FailoverChain::new(AcceptancePolicy::new());
    chain
        .register(source(
            ProviderId::Tdx,
            Err(SourceError::try_next(
                FailureKind::Transport,
                "TDX disconnected",
            )),
        ))
        .unwrap();
    chain
        .register(source(
            ProviderId::Tencent,
            Ok(batch(
                ProviderId::Tencent,
                "tencent:1",
                "tencent:1",
                true,
                None,
            )),
        ))
        .unwrap();

    let outcome = chain.route(&[instrument()]).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Tencent);
    assert_eq!(outcome.attempts().len(), 2);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Transport,
            action: FailureAction::TryNext,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Selected
    ));
    assert_eq!(outcome.batch().records().len(), 1);
}

#[test]
fn first_success_does_not_call_later_source() {
    let calls = Arc::new(AtomicUsize::new(0));
    let later_calls = Arc::clone(&calls);
    let mut chain = FailoverChain::new(AcceptancePolicy::new());
    chain
        .register(source(
            ProviderId::Tdx,
            Ok(batch(ProviderId::Tdx, "tdx:1", "tdx:1", false, None)),
        ))
        .unwrap();
    chain
        .register(SourceFn::new(ProviderId::Tencent, move |_| {
            later_calls.fetch_add(1, Ordering::SeqCst);
            Ok(batch(
                ProviderId::Tencent,
                "tencent:1",
                "tencent:1",
                true,
                None,
            ))
        }))
        .unwrap();

    assert_eq!(
        chain.route(&[instrument()]).unwrap().selected_provider(),
        ProviderId::Tdx
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn terminal_failure_stops_without_calling_next_source() {
    let calls = Arc::new(AtomicUsize::new(0));
    let later_calls = Arc::clone(&calls);
    let mut chain = FailoverChain::new(AcceptancePolicy::new());
    chain
        .register(source(
            ProviderId::Tdx,
            Err(SourceError::stop(
                FailureKind::InvalidRequest,
                "duplicate code",
            )),
        ))
        .unwrap();
    chain
        .register(SourceFn::new(ProviderId::Tencent, move |_| {
            later_calls.fetch_add(1, Ordering::SeqCst);
            Ok(batch(
                ProviderId::Tencent,
                "tencent:1",
                "tencent:1",
                true,
                None,
            ))
        }))
        .unwrap();

    let error = chain.route(&[instrument()]).unwrap_err();
    assert!(matches!(error, RouterError::Stopped { .. }));
    assert_eq!(error.attempts().len(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn exhaustion_retains_ordered_attempts() {
    let mut chain = FailoverChain::new(AcceptancePolicy::new());
    chain
        .register(source(
            ProviderId::Tdx,
            Err(SourceError::try_next(FailureKind::NoData, "empty response")),
        ))
        .unwrap();
    chain
        .register(source(
            ProviderId::Tencent,
            Err(SourceError::try_next(FailureKind::Transport, "timeout")),
        ))
        .unwrap();
    let error = chain.route(&[instrument()]).unwrap_err();
    assert!(matches!(error, RouterError::Exhausted { .. }));
    assert_eq!(error.attempts()[0].provider_id(), ProviderId::Tdx);
    assert_eq!(error.attempts()[1].provider_id(), ProviderId::Tencent);
}

#[test]
fn policy_rejects_empty_incomplete_and_missing_source_time() {
    let strict = AcceptancePolicy::new()
        .with_require_complete(true)
        .with_require_source_at(true);
    let mut chain = FailoverChain::new(strict);
    chain
        .register(source(
            ProviderId::Tdx,
            Ok(DataBatch::strict(
                Vec::<Quote>::new(),
                Provenance::new("tdx", "observed").unwrap(),
            )),
        ))
        .unwrap();
    chain
        .register(source(
            ProviderId::Eastmoney,
            Ok(batch(
                ProviderId::Eastmoney,
                "eastmoney:1",
                "eastmoney:1",
                true,
                Some("partial fields"),
            )),
        ))
        .unwrap();
    chain
        .register(source(
            ProviderId::Tencent,
            Ok(batch(
                ProviderId::Tencent,
                "tencent:1",
                "tencent:1",
                false,
                None,
            )),
        ))
        .unwrap();

    let error = chain.route(&[instrument()]).unwrap_err();
    assert!(matches!(error, RouterError::Exhausted { .. }));
    assert_eq!(error.attempts().len(), 3);
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::NoData,
            ..
        }
    ));
    assert!(matches!(
        error.attempts()[1].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Quality,
            ..
        }
    ));
    assert!(matches!(
        error.attempts()[2].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Quality,
            ..
        }
    ));
}

#[test]
fn permissive_policy_accepts_partial_batch() {
    let mut chain = FailoverChain::new(AcceptancePolicy::new());
    chain
        .register(source(
            ProviderId::Tencent,
            Ok(batch(
                ProviderId::Tencent,
                "tencent:1",
                "tencent:1",
                false,
                Some("partial fields"),
            )),
        ))
        .unwrap();
    assert_eq!(
        chain.route(&[instrument()]).unwrap().selected_provider(),
        ProviderId::Tencent
    );
}

#[test]
fn duplicate_provider_registration_is_rejected() {
    let mut chain = FailoverChain::new(AcceptancePolicy::new());
    chain
        .register(source(
            ProviderId::Tencent,
            Ok(batch(
                ProviderId::Tencent,
                "tencent:1",
                "tencent:1",
                true,
                None,
            )),
        ))
        .unwrap();
    let error = chain
        .register(source(
            ProviderId::Tencent,
            Ok(batch(
                ProviderId::Tencent,
                "tencent:2",
                "tencent:2",
                true,
                None,
            )),
        ))
        .unwrap_err();
    assert!(matches!(error, RouterError::InvalidConfiguration(_)));
}

#[test]
fn evidence_mismatches_are_never_selected() {
    let cases = [
        batch(ProviderId::Tencent, "tencent:1", "tencent:1", true, None),
        batch(ProviderId::Tdx, "tdx:record", "tdx:provenance", true, None),
    ];
    for returned in cases {
        let mut chain = FailoverChain::new(AcceptancePolicy::new());
        chain
            .register(source(ProviderId::Tdx, Ok(returned)))
            .unwrap();
        let error = chain.route(&[instrument()]).unwrap_err();
        assert!(matches!(
            error.attempts()[0].status(),
            AttemptStatus::Rejected {
                kind: FailureKind::Evidence,
                ..
            }
        ));
    }
}

#[test]
fn missing_provenance_batch_id_is_never_selected() {
    let valid = batch(ProviderId::Tencent, "tencent:1", "tencent:1", true, None);
    let mut json = serde_json::to_value(valid).unwrap();
    let Value::Object(root) = &mut json else {
        panic!("batch must serialize as an object");
    };
    let Value::Object(provenance) = root.get_mut("provenance").unwrap() else {
        panic!("provenance must serialize as an object");
    };
    provenance.insert("batch_id".into(), Value::Null);
    assert!(serde_json::from_value::<DataBatch<Quote>>(json).is_err());
}

#[test]
fn record_statuses_are_enforced_without_treating_partial_as_complete() {
    let stale = Quote::from_parts(
        instrument(),
        None,
        Price::new(15.5).unwrap(),
        None,
        None,
        None,
        None,
        None,
        Quantity::new(100.0).unwrap(),
        None,
        DataStatus::Stale,
        None,
        "observed",
        ProviderId::Tencent,
        "status:stale",
    )
    .unwrap();
    let stale_batch = DataBatch::strict(
        vec![stale],
        Provenance::new("fixture", "observed")
            .unwrap()
            .with_batch_id("status:stale")
            .unwrap(),
    );
    let mut default_chain = FailoverChain::new(AcceptancePolicy::new());
    default_chain
        .register(source(ProviderId::Tencent, Ok(stale_batch)))
        .unwrap();
    let error = default_chain.route(&[instrument()]).unwrap_err();
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Quality,
            ..
        }
    ));

    let mut available_only =
        FailoverChain::new(AcceptancePolicy::new().with_require_available_records(true));
    available_only
        .register(source(
            ProviderId::Tencent,
            Ok(batch(
                ProviderId::Tencent,
                "status:partial",
                "status:partial",
                false,
                None,
            )),
        ))
        .unwrap();
    assert!(available_only.route(&[instrument()]).is_err());
}

#[test]
fn empty_chain_is_an_explicit_configuration_error() {
    let chain = FailoverChain::<[InstrumentId], Quote>::new(AcceptancePolicy::new());
    assert!(matches!(
        chain.route(&[instrument()]).unwrap_err(),
        RouterError::InvalidConfiguration(_)
    ));
}
