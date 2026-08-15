use magic_market_core::{
    Adjustment, AssetClass, Bar, BarInterval, DataBatch, Exchange, InstrumentId, Money, Price,
    Provenance, ProviderId, Quantity, Quote,
};
use magic_market_router::{
    AcceptancePolicy, AttemptStatus, FailoverChain, FailureKind, RouterError, SourceFn,
};
use std::time::Duration;

fn instrument(code: &str) -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap()
}

fn quote(
    code: &str,
    source_at: Option<&str>,
    observed_at: &str,
    provider: ProviderId,
    batch_id: &str,
) -> Quote {
    let quote = Quote::new(
        instrument(code),
        Price::new(15.5).unwrap(),
        Quantity::new(100.0).unwrap(),
        Some(Money::new(1_550.0).unwrap()),
        observed_at,
        provider,
        batch_id,
    )
    .unwrap();
    match source_at {
        Some(value) => quote.with_source_at(value).unwrap(),
        None => quote,
    }
}

fn batch(
    records: Vec<Quote>,
    source_at: Option<&str>,
    observed_at: &str,
    provider: ProviderId,
    batch_id: &str,
) -> DataBatch<Quote> {
    let mut provenance = Provenance::new(format!("{provider:?}-fixture"), observed_at)
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap();
    if let Some(value) = source_at {
        provenance = provenance.with_source_at(value).unwrap();
    }
    DataBatch::strict(records, provenance)
}

fn strict_policy() -> AcceptancePolicy {
    AcceptancePolicy::new()
        .with_max_source_age(Duration::from_secs(5))
        .unwrap()
}

fn route(batch: DataBatch<Quote>, policy: AcceptancePolicy) -> Result<(), RouterError> {
    let provider = batch.records()[0].provider();
    let mut chain = FailoverChain::new(policy);
    chain
        .register(SourceFn::new(provider, move |_: &[InstrumentId]| {
            Ok(batch.clone())
        }))
        .unwrap();
    chain.route(&[instrument("600396")]).map(|_| ())
}

fn rejection_message(error: &RouterError) -> &str {
    match error.attempts()[0].status() {
        AttemptStatus::Rejected { message, .. } => message,
        status => panic!("expected rejected attempt, got {status:?}"),
    }
}

#[test]
fn five_seconds_is_admitted_and_six_seconds_is_rejected() {
    let observed_at = "2026-07-27T10:00:05+08:00";
    let accepted_source = "2026-07-27T10:00:00+08:00";
    let accepted = batch(
        vec![quote(
            "600396",
            Some(accepted_source),
            observed_at,
            ProviderId::Tencent,
            "fresh:5",
        )],
        Some(accepted_source),
        observed_at,
        ProviderId::Tencent,
        "fresh:5",
    );
    route(accepted, strict_policy()).unwrap();

    let stale_source = "2026-07-27T09:59:59+08:00";
    let stale = batch(
        vec![quote(
            "600396",
            Some(stale_source),
            observed_at,
            ProviderId::Tencent,
            "fresh:6",
        )],
        Some(stale_source),
        observed_at,
        ProviderId::Tencent,
        "fresh:6",
    );
    let error = route(stale, strict_policy()).unwrap_err();
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Quality,
            ..
        }
    ));
    assert_eq!(
        rejection_message(&error),
        "batch source timestamp is stale by 6s; maximum allowed age is 5s"
    );
}

#[test]
fn five_seconds_and_one_nanosecond_is_rejected() {
    let observed_at = "2026-07-27T10:00:05.000000001+08:00";
    let source_at = "2026-07-27T10:00:00+08:00";
    let stale = batch(
        vec![quote(
            "600396",
            Some(source_at),
            observed_at,
            ProviderId::Tencent,
            "fresh:5s1ns",
        )],
        Some(source_at),
        observed_at,
        ProviderId::Tencent,
        "fresh:5s1ns",
    );
    let error = route(stale, strict_policy()).unwrap_err();
    assert_eq!(
        rejection_message(&error),
        "batch source timestamp is stale by 5.000000001s; maximum allowed age is 5s"
    );
}

#[test]
fn future_malformed_and_missing_record_times_are_rejected() {
    let observed_at = "2026-07-27T10:00:05+08:00";
    for (record_source, batch_source, expected) in [
        (
            Some("2026-07-27T10:00:05.001+08:00"),
            Some("2026-07-27T10:00:05.001+08:00"),
            "record source timestamp is later than its observed timestamp",
        ),
        (
            Some("2026-07-27T10:00:05.000000001+08:00"),
            Some("2026-07-27T10:00:05.000000001+08:00"),
            "record source timestamp is later than its observed timestamp",
        ),
        (
            Some("2026-07-27X10:00:05"),
            Some("2026-07-27X10:00:05"),
            "record source timestamp is malformed",
        ),
        (
            None,
            Some("2026-07-27T10:00:05+08:00"),
            "record source timestamp is unavailable",
        ),
        (
            Some("2026-07-27"),
            Some("2026-07-27"),
            "record source timestamp is malformed",
        ),
        (
            Some("2026-07-27T10:00:05"),
            Some("2026-07-27T10:00:05"),
            "record source timestamp is malformed",
        ),
        (
            Some("2026-07-27T10:00:05.0000000001+08:00"),
            Some("2026-07-27T10:00:05.0000000001+08:00"),
            "record source timestamp is malformed",
        ),
    ] {
        let candidate = batch(
            vec![quote(
                "600396",
                record_source,
                observed_at,
                ProviderId::Tencent,
                "fresh:invalid",
            )],
            batch_source,
            observed_at,
            ProviderId::Tencent,
            "fresh:invalid",
        );
        let error = route(candidate, strict_policy()).unwrap_err();
        assert_eq!(rejection_message(&error), expected);
    }
}

#[test]
fn malformed_batch_time_is_rejected_after_valid_record_times() {
    let observed_at = "2026-07-27T10:00:05+08:00";
    let record_source = "2026-07-27T10:00:00+08:00";
    let candidate = batch(
        vec![quote(
            "600396",
            Some(record_source),
            observed_at,
            ProviderId::Tencent,
            "fresh:malformed-batch",
        )],
        Some("2026-07-27X10:00:00"),
        observed_at,
        ProviderId::Tencent,
        "fresh:malformed-batch",
    );
    let error = route(candidate, strict_policy()).unwrap_err();
    assert_eq!(
        rejection_message(&error),
        "batch source timestamp is malformed"
    );
}

#[test]
fn batch_time_must_equal_the_oldest_record_time() {
    let observed_at = "2026-07-27T10:00:05+08:00";
    let oldest = "2026-07-27T10:00:00+08:00";
    let newest = "2026-07-27T10:00:04+08:00";
    let records = vec![
        quote(
            "600396",
            Some(newest),
            observed_at,
            ProviderId::Tencent,
            "fresh:oldest",
        ),
        quote(
            "600000",
            Some(oldest),
            observed_at,
            ProviderId::Tencent,
            "fresh:oldest",
        ),
    ];
    route(
        batch(
            records.clone(),
            Some(oldest),
            observed_at,
            ProviderId::Tencent,
            "fresh:oldest",
        ),
        strict_policy(),
    )
    .unwrap();

    let error = route(
        batch(
            records,
            Some(newest),
            observed_at,
            ProviderId::Tencent,
            "fresh:oldest",
        ),
        strict_policy(),
    )
    .unwrap_err();
    assert_eq!(
        rejection_message(&error),
        "batch source timestamp does not equal the oldest record source timestamp"
    );
}

#[test]
fn oldest_record_comparison_preserves_nanoseconds() {
    let observed_at = "2026-07-27T10:00:05+08:00";
    let oldest = "2026-07-27T10:00:00.000000001+08:00";
    let newest = "2026-07-27T10:00:00.000000002+08:00";
    let records = vec![
        quote(
            "600396",
            Some(newest),
            observed_at,
            ProviderId::Tencent,
            "fresh:nanosecond-oldest",
        ),
        quote(
            "600000",
            Some(oldest),
            observed_at,
            ProviderId::Tencent,
            "fresh:nanosecond-oldest",
        ),
    ];
    let error = route(
        batch(
            records,
            Some(newest),
            observed_at,
            ProviderId::Tencent,
            "fresh:nanosecond-oldest",
        ),
        strict_policy(),
    )
    .unwrap_err();
    assert_eq!(
        rejection_message(&error),
        "batch source timestamp does not equal the oldest record source timestamp"
    );
}

#[test]
fn record_and_batch_observation_times_must_match() {
    let source_at = "2026-07-27T10:00:00+08:00";
    let candidate = batch(
        vec![quote(
            "600396",
            Some(source_at),
            "2026-07-27T10:00:04+08:00",
            ProviderId::Tencent,
            "fresh:observed-mismatch",
        )],
        Some(source_at),
        "2026-07-27T10:00:05+08:00",
        ProviderId::Tencent,
        "fresh:observed-mismatch",
    );
    let error = route(candidate, strict_policy()).unwrap_err();
    assert_eq!(
        rejection_message(&error),
        "record observed timestamp does not match batch observation timestamp"
    );
}

#[test]
fn date_only_or_timezone_free_observation_is_not_a_realtime_instant() {
    let source_at = "2026-07-27T00:00:00Z";
    for observed_at in ["2026-07-27", "2026-07-27T00:00:01"] {
        let candidate = batch(
            vec![quote(
                "600396",
                Some(source_at),
                observed_at,
                ProviderId::Tencent,
                "fresh:ambiguous-observed",
            )],
            Some(source_at),
            observed_at,
            ProviderId::Tencent,
            "fresh:ambiguous-observed",
        );
        let error = route(candidate, strict_policy()).unwrap_err();
        assert_eq!(
            rejection_message(&error),
            "batch observation timestamp is malformed"
        );
    }
}

#[test]
fn milliseconds_and_timezone_offsets_are_compared_as_instants() {
    let observed_at = "2026-07-27T10:00:05.500+08:00";
    let source_at = "2026-07-27T02:00:00.500Z";
    let candidate = batch(
        vec![quote(
            "600396",
            Some(source_at),
            observed_at,
            ProviderId::Tencent,
            "fresh:offset",
        )],
        Some("2026-07-27T10:00:00.500+08:00"),
        observed_at,
        ProviderId::Tencent,
        "fresh:offset",
    );
    route(candidate, strict_policy()).unwrap();

    let stale = batch(
        vec![quote(
            "600396",
            Some("unix-ms:1785117599999"),
            "unix-ms:1785117605000",
            ProviderId::Tencent,
            "fresh:millis",
        )],
        Some("unix-ms:1785117599999"),
        "unix-ms:1785117605000",
        ProviderId::Tencent,
        "fresh:millis",
    );
    let error = route(stale, strict_policy()).unwrap_err();
    assert_eq!(
        rejection_message(&error),
        "batch source timestamp is stale by 5.001s; maximum allowed age is 5s"
    );
}

#[test]
fn no_freshness_policy_does_not_require_source_time() {
    let candidate = batch(
        vec![quote(
            "600396",
            None,
            "2026-07-27T10:00:05+08:00",
            ProviderId::Tdx,
            "fresh:none",
        )],
        None,
        "2026-07-27T10:00:05+08:00",
        ProviderId::Tdx,
        "fresh:none",
    );
    route(candidate, AcceptancePolicy::new()).unwrap();
    assert_eq!(AcceptancePolicy::new().max_source_age(), None);
}

#[test]
fn zero_maximum_age_is_invalid_configuration() {
    assert!(matches!(
        AcceptancePolicy::new().with_max_source_age(Duration::ZERO),
        Err(RouterError::InvalidConfiguration(message))
            if message == "maximum source age must be positive"
    ));
}

#[test]
fn maximum_age_source_requirement_cannot_be_disabled_by_setter_order() {
    let policy = AcceptancePolicy::new()
        .with_max_source_age(Duration::from_secs(5))
        .unwrap()
        .with_require_source_at(false);
    assert!(policy.require_source_at());

    let candidate = batch(
        vec![quote(
            "600396",
            None,
            "2026-07-27T10:00:05+08:00",
            ProviderId::Tdx,
            "fresh:ordered-policy",
        )],
        None,
        "2026-07-27T10:00:05+08:00",
        ProviderId::Tdx,
        "fresh:ordered-policy",
    );
    assert!(route(candidate, policy).is_err());
}

#[test]
fn non_quote_records_expose_source_and_observation_evidence_to_freshness() {
    let observed_at = "2026-07-27T10:00:05+08:00";
    let source_at = "2026-07-27T10:00:00+08:00";
    let batch_id = "fresh:bar";
    let bar = Bar::new(
        instrument("600396"),
        BarInterval::Day,
        "2026-07-27",
        "2026-07-27",
        Price::new(15.0).unwrap(),
        Price::new(16.0).unwrap(),
        Price::new(14.5).unwrap(),
        Price::new(15.5).unwrap(),
        Quantity::new(100.0).unwrap(),
        Some(Money::new(1_550.0).unwrap()),
        Adjustment::Unadjusted,
        ProviderId::Tdx,
        batch_id,
    )
    .unwrap()
    .with_source_at(source_at)
    .unwrap()
    .with_observed_at(observed_at)
    .unwrap();
    let provenance = Provenance::new("tdx-fixture", observed_at)
        .unwrap()
        .with_source_at(source_at)
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap();
    let candidate = DataBatch::strict(vec![bar], provenance);
    let mut chain = FailoverChain::new(strict_policy());
    chain
        .register(SourceFn::new(
            ProviderId::Tdx,
            move |_: &[InstrumentId]| Ok(candidate.clone()),
        ))
        .unwrap();
    chain.route(&[instrument("600396")]).unwrap();
}
