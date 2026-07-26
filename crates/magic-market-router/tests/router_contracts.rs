use magic_market_core::{
    AssetClass, DataBatch, Exchange, InstrumentId, Money, Price, Provenance, ProviderId, Quantity,
    Quote,
};
use magic_market_router::{
    AcceptancePolicy, AttemptStatus, FailoverChain, FailureAction, FailureKind, RouterError,
    SourceError, SourceFn,
};

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn batch(provider: ProviderId, batch_id: &str, source_at: Option<&str>) -> DataBatch<Quote> {
    let record = Quote::new(
        instrument(),
        Price::new(15.5).unwrap(),
        Quantity::new(100.0).unwrap(),
        Some(Money::new(1_550.0).unwrap()),
        "observed",
        provider,
        batch_id,
    )
    .unwrap();
    let mut provenance = Provenance::new("fixture", "observed")
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap();
    if let Some(source_at) = source_at {
        provenance = provenance.with_source_at(source_at).unwrap();
    }
    DataBatch::strict(vec![record], provenance)
}

#[test]
fn rate_limit_is_retryable_and_retains_typed_audit_details() {
    let mut chain = FailoverChain::new(AcceptancePolicy::new());
    chain
        .register(SourceFn::new(ProviderId::Tdx, |_| {
            Err(SourceError::try_next(
                FailureKind::RateLimited,
                "provider quota exhausted",
            ))
        }))
        .unwrap();
    chain
        .register(SourceFn::new(ProviderId::Tencent, |_| {
            Ok(batch(
                ProviderId::Tencent,
                "tencent:rate-fallback",
                Some("2026-07-23T10:00:00+08:00"),
            ))
        }))
        .unwrap();

    let outcome = chain.route(&[instrument()]).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Tencent);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::RateLimited,
            action: FailureAction::TryNext,
            message,
        } if message == "provider quota exhausted"
    ));
}

#[test]
fn source_time_policy_rejects_missing_time_without_requiring_complete_quality() {
    let policy = AcceptancePolicy::new().with_require_source_at(true);
    assert!(!policy.require_complete());
    assert!(policy.require_source_at());

    let mut chain = FailoverChain::new(policy);
    chain
        .register(SourceFn::new(ProviderId::Tdx, |_| {
            Ok(batch(ProviderId::Tdx, "tdx:no-source-time", None))
        }))
        .unwrap();

    let error = chain.route(&[instrument()]).unwrap_err();
    assert!(matches!(error, RouterError::Exhausted { .. }));
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Quality,
            message,
        } if message == "batch source timestamp is unavailable"
    ));
}

#[test]
fn route_metadata_and_consuming_accessors_preserve_order_and_batch() {
    let policy = AcceptancePolicy::new().with_require_complete(true);
    let mut chain = FailoverChain::new(policy);
    chain
        .register(SourceFn::new(ProviderId::Tdx, |_| {
            Err(SourceError::try_next(
                FailureKind::Protocol,
                "invalid frame",
            ))
        }))
        .unwrap();
    chain
        .register(SourceFn::new(ProviderId::Tencent, |_| {
            Ok(batch(
                ProviderId::Tencent,
                "tencent:accepted",
                Some("2026-07-23T10:00:00+08:00"),
            ))
        }))
        .unwrap();

    assert_eq!(chain.policy(), policy);
    assert_eq!(
        chain.provider_ids(),
        vec![ProviderId::Tdx, ProviderId::Tencent]
    );
    assert!(format!("{chain:?}").contains("source_count: 2"));

    let outcome = chain.route(&[instrument()]).unwrap();
    let (selected, attempts) = outcome.into_parts();
    assert_eq!(selected.records().len(), 1);
    assert_eq!(attempts.len(), 2);

    let outcome = chain.route(&[instrument()]).unwrap();
    assert_eq!(outcome.into_batch().records().len(), 1);
}

#[test]
fn typed_errors_and_configuration_errors_have_stable_observable_text() {
    let source = SourceError::stop(FailureKind::Unsupported, "family unavailable");
    assert_eq!(source.to_string(), "Unsupported: family unavailable");

    let chain = FailoverChain::<[InstrumentId], Quote>::new(AcceptancePolicy::new());
    let error = chain.route(&[instrument()]).unwrap_err();
    assert!(error.attempts().is_empty());
    assert_eq!(
        error.to_string(),
        "invalid router configuration: at least one source must be registered"
    );
}
