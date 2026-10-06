use magic_market_core::{Adjustment, BookLevel, Exchange, Ratio, RatioUnit};

use super::*;

fn equity() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn index() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).unwrap()
}

fn evidence(provider: ProviderId, batch: &str) -> SourceEvidence {
    SourceEvidence::new(provider, "2026-08-14T15:01:00+08:00", batch)
        .unwrap()
        .with_source_at("2026-08-14T15:00:00+08:00")
        .unwrap()
}

fn bar(interval: BarInterval, batch: &str) -> Bar {
    let (start, end) = match interval {
        BarInterval::Day => ("2026-08-14", "2026-08-14"),
        BarInterval::Minute5 => ("2026-08-14T09:30:00", "2026-08-14T09:35:00"),
        _ => unreachable!(),
    };
    Bar::new(
        equity(),
        interval,
        start,
        end,
        Price::new(10.0).unwrap(),
        Price::new(11.0).unwrap(),
        Price::new(9.0).unwrap(),
        Price::new(10.5).unwrap(),
        Quantity::new(100.0).unwrap(),
        Some(Money::new(1_000.0).unwrap()),
        Adjustment::Unadjusted,
        ProviderId::Tdx,
        batch,
    )
    .unwrap()
    .with_source_at("2026-08-14T15:00:00+08:00")
    .unwrap()
    .with_observed_at("2026-08-14T15:01:00+08:00")
    .unwrap()
}

fn quote() -> Quote {
    Quote::from_parts(
        equity(),
        Some("Example".into()),
        Price::new(10.5).unwrap(),
        Some(Price::new(10.0).unwrap()),
        Some(Price::new(10.0).unwrap()),
        Some(Price::new(11.0).unwrap()),
        Some(Price::new(9.0).unwrap()),
        Some(Ratio::new(5.0, RatioUnit::Percent).unwrap()),
        Quantity::new(100.0).unwrap(),
        Some(Money::new(1_000.0).unwrap()),
        DataStatus::Available,
        Some("2026-08-14T15:00:00+08:00".into()),
        "2026-08-14T15:01:00+08:00",
        ProviderId::Tdx,
        "quote-batch",
    )
    .unwrap()
}

fn order_book() -> OrderBook {
    let level = BookLevel::new(
        Some(Price::new(10.0).unwrap()),
        Some(Quantity::new(100.0).unwrap()),
    )
    .unwrap();
    OrderBook::new(
        equity(),
        [level; 5],
        [level; 5],
        Some(Quantity::new(500.0).unwrap()),
        Some(Quantity::new(500.0).unwrap()),
        DataStatus::Available,
        Some("2026-08-14T15:00:00+08:00".into()),
        "2026-08-14T15:01:00+08:00",
        ProviderId::Tdx,
        "book-batch",
    )
    .unwrap()
}

#[test]
fn all_five_request_contracts_round_trip_through_their_checked_constructors() {
    let index_request = IndexQuotesRequest::new(vec![index()], 5_000).unwrap();
    let decoded: IndexQuotesRequest =
        serde_json::from_slice(&serde_json::to_vec(&index_request).unwrap()).unwrap();
    assert_eq!(decoded, index_request);

    let shape = IntradayShapeRequest::new(
        equity(),
        None,
        PositiveU32::new(MAX_INTRADAY_POINTS).unwrap(),
    )
    .unwrap();
    let decoded: IntradayShapeRequest =
        serde_json::from_slice(&serde_json::to_vec(&shape).unwrap()).unwrap();
    assert_eq!(decoded, shape);

    let t0 = T0EvidenceRequest::new(
        vec![equity()],
        PositiveU32::new(20).unwrap(),
        PositiveU32::new(48).unwrap(),
        "2026-08-14T15:01:00+08:00",
    )
    .unwrap();
    let decoded: T0EvidenceRequest =
        serde_json::from_slice(&serde_json::to_vec(&t0).unwrap()).unwrap();
    assert_eq!(decoded, t0);

    let outcome = OutcomeDailyBarsRequest::new(
        equity(),
        IsoDate::new("2026-08-14").unwrap(),
        PositiveU32::new(20).unwrap(),
        "2026-08-14T15:35:00+08:00",
    )
    .unwrap();
    let decoded: OutcomeDailyBarsRequest =
        serde_json::from_slice(&serde_json::to_vec(&outcome).unwrap()).unwrap();
    assert_eq!(decoded, outcome);

    let review = UpperLimitPoolReviewRequest::new(
        IsoDate::new("2026-08-14").unwrap(),
        PositiveU32::new(MAX_LIMIT_POOL_ROWS).unwrap(),
    )
    .unwrap();
    let decoded: UpperLimitPoolReviewRequest =
        serde_json::from_slice(&serde_json::to_vec(&review).unwrap()).unwrap();
    assert_eq!(decoded, review);
}

#[test]
fn t0_evidence_v2_requires_and_preserves_the_caller_requested_at_instant() {
    let request: T0EvidenceRequest = serde_json::from_value(serde_json::json!({
        "instruments": [equity()],
        "daily_bar_count": 20,
        "five_minute_bar_count": 48,
        "requested_at": "2026-08-27T09:57:40+08:00"
    }))
    .unwrap();

    assert_eq!(
        serde_json::to_value(request).unwrap()["requested_at"],
        "2026-08-27T09:57:40+08:00"
    );
    assert!(
        serde_json::from_value::<T0EvidenceRequest>(serde_json::json!({
            "instruments": [equity()],
            "daily_bar_count": 20,
            "five_minute_bar_count": 48
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<T0EvidenceRequest>(serde_json::json!({
            "instruments": [equity()],
            "daily_bar_count": 20,
            "five_minute_bar_count": 48,
            "requested_at": "2026-08-27 09:57:40"
        }))
        .is_err()
    );
}

#[test]
fn invalid_identity_bounds_unknown_fields_and_due_time_fail_closed() {
    assert!(IndexQuotesRequest::new(vec![equity()], 5_000).is_err());
    assert!(IndexQuotesRequest::new(vec![index(), index()], 5_000).is_err());
    assert!(IntradayShapeRequest::new(
        equity(),
        None,
        PositiveU32::new(MAX_INTRADAY_POINTS + 1).unwrap()
    )
    .is_err());
    assert!(T0EvidenceRequest::new(
        vec![equity(), equity()],
        PositiveU32::new(20).unwrap(),
        PositiveU32::new(48).unwrap(),
        "2026-08-14T15:01:00+08:00"
    )
    .is_err());
    assert!(OutcomeDailyBarsRequest::new(
        equity(),
        IsoDate::new("2026-08-14").unwrap(),
        PositiveU32::new(20).unwrap(),
        "2026-08-14T14:59:59+08:00"
    )
    .is_err());
    assert!(serde_json::from_str::<UpperLimitPoolReviewRequest>(
        r#"{"trading_date":"2026-08-14","per_pool_limit":1,"extra":true}"#
    )
    .is_err());
}

#[test]
fn derived_response_contracts_reject_tampered_counts_and_preserve_exact_inputs() {
    let shape = IntradayShapeRecord::new(
        equity(),
        IsoDate::new("2026-08-14").unwrap(),
        "2026-08-14T09:30:00+08:00",
        "2026-08-14T15:00:00+08:00",
        PositiveU32::new(3).unwrap(),
        Price::new(10.0).unwrap(),
        Price::new(11.0).unwrap(),
        Price::new(9.0).unwrap(),
        Price::new(10.5).unwrap(),
        Some(Price::new(10.2).unwrap()),
        Some(Quantity::new(100.0).unwrap()),
        Some(Money::new(1_000.0).unwrap()),
        1,
        1,
        1,
        vec![evidence(ProviderId::Tencent, "minute-batch")],
        PositiveU32::new(1).unwrap(),
        "a".repeat(64),
    )
    .unwrap();
    let decoded: IntradayShapeRecord =
        serde_json::from_slice(&serde_json::to_vec(&shape).unwrap()).unwrap();
    assert_eq!(decoded.point_count().get(), 3);

    let t0 = T0EvidenceRecord::new(
        equity(),
        "2026-08-14T15:00:30+08:00",
        quote(),
        order_book(),
        vec![bar(BarInterval::Day, "day-batch")],
        vec![bar(BarInterval::Minute5, "minute5-batch")],
        PositiveU32::new(1).unwrap(),
        PositiveU32::new(1).unwrap(),
        vec![
            evidence(ProviderId::Tdx, "quote-batch"),
            evidence(ProviderId::Tdx, "book-batch"),
            evidence(ProviderId::Tdx, "day-batch"),
            evidence(ProviderId::Tdx, "minute5-batch"),
        ],
        PositiveU32::new(1).unwrap(),
        "b".repeat(64),
    )
    .unwrap();
    let t0_round_trip: T0EvidenceRecord =
        serde_json::from_slice(&serde_json::to_vec(&t0).unwrap()).unwrap();
    assert_eq!(t0_round_trip.requested_at(), "2026-08-14T15:00:30+08:00");
    let mut t0_json = serde_json::to_value(&t0).unwrap();
    t0_json["daily_bar_count"] = serde_json::json!(2);
    assert!(serde_json::from_value::<T0EvidenceRecord>(t0_json).is_err());

    let outcome = OutcomeDailyBarsRecord::new(
        equity(),
        IsoDate::new("2026-08-14").unwrap(),
        "2026-08-14T15:35:00+08:00",
        vec![bar(BarInterval::Day, "outcome-batch")],
        PositiveU32::new(1).unwrap(),
        vec![evidence(ProviderId::Tdx, "outcome-batch")],
        PositiveU32::new(1).unwrap(),
        "c".repeat(64),
    )
    .unwrap();
    let mut outcome_json = serde_json::to_value(&outcome).unwrap();
    outcome_json["requested_limit"] = serde_json::json!(2);
    assert!(serde_json::from_value::<OutcomeDailyBarsRecord>(outcome_json).is_err());
}
