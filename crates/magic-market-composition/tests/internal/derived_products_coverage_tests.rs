use crate::{
    DerivedProductContractError, IntradayShapeRecord, OutcomeDailyBarsRecord, T0EvidenceRecord,
};
use magic_market_core::{
    Adjustment, AssetClass, Bar, BarInterval, BookLevel, DataStatus, Exchange, InstrumentId,
    IsoDate, Money, OrderBook, PositiveU32, Price, ProviderId, Quantity, Quote, SourceEvidence,
};
use serde_json::json;

fn complete_tdx_capture() -> T0EvidenceRecord {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
    let quote = Quote::from_parts(
        instrument.clone(),
        Some("Example".to_owned()),
        Price::new(10.5).unwrap(),
        Some(Price::new(10.0).unwrap()),
        Some(Price::new(10.0).unwrap()),
        Some(Price::new(11.0).unwrap()),
        Some(Price::new(9.0).unwrap()),
        None,
        Quantity::new(100.0).unwrap(),
        Some(Money::new(100_000.0).unwrap()),
        DataStatus::Unavailable,
        None,
        "2026-08-14T15:01:00+08:00",
        ProviderId::Tdx,
        "quote-capture",
    )
    .unwrap();
    let level = BookLevel::new(
        Some(Price::new(10.0).unwrap()),
        Some(Quantity::new(100.0).unwrap()),
    )
    .unwrap();
    let book = OrderBook::new(
        instrument.clone(),
        [level; 5],
        [level; 5],
        Some(Quantity::new(500.0).unwrap()),
        Some(Quantity::new(500.0).unwrap()),
        DataStatus::Unavailable,
        None,
        "2026-08-14T15:01:00+08:00",
        ProviderId::Tdx,
        "book-capture",
    )
    .unwrap();
    let bar = |interval, label, source_at, batch| {
        Bar::new(
            instrument.clone(),
            interval,
            label,
            label,
            Price::new(10.0).unwrap(),
            Price::new(11.0).unwrap(),
            Price::new(9.0).unwrap(),
            Price::new(10.5).unwrap(),
            Quantity::new(100.0).unwrap(),
            Some(Money::new(100_000.0).unwrap()),
            Adjustment::Unadjusted,
            ProviderId::Tdx,
            batch,
        )
        .unwrap()
        .with_source_at(source_at)
        .unwrap()
        .with_observed_at("2026-08-14T15:01:00+08:00")
        .unwrap()
    };
    let evidence =
        |batch| SourceEvidence::new(ProviderId::Tdx, "2026-08-14T15:01:00+08:00", batch).unwrap();
    T0EvidenceRecord::new(
        instrument.clone(),
        "2026-08-14T15:00:30.123456+08:00",
        quote,
        book,
        vec![bar(
            BarInterval::Day,
            "2026-08-14",
            "2026-08-14",
            "day-capture",
        )],
        vec![bar(
            BarInterval::Minute5,
            "2026-08-14 14:55:00",
            "2026-08-14 14:55",
            "minute5-capture",
        )],
        PositiveU32::new(1).unwrap(),
        PositiveU32::new(1).unwrap(),
        vec![
            evidence("quote-capture"),
            evidence("book-capture"),
            evidence("day-capture")
                .with_source_at("2026-08-14")
                .unwrap(),
            evidence("minute5-capture")
                .with_source_at("2026-08-14 14:55")
                .unwrap(),
        ],
        PositiveU32::new(1).unwrap(),
        "a".repeat(64),
    )
    .unwrap()
}

#[test]
fn incomplete_or_conflicting_tdx_captures_cannot_become_complete_t0_bundles() {
    // BR-050/BR-053 permit an observation-time TDX capture with absent quote/book
    // source times, but never missing families or substituted original evidence.
    let payload = serde_json::to_value(complete_tdx_capture()).unwrap();
    let control: T0EvidenceRecord = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(control.requested_at(), "2026-08-14T15:00:30.123456+08:00");
    assert_eq!(control.quote().source_at(), None);
    assert_eq!(control.order_book().source_at(), None);
    assert_eq!(control.daily_bar_count().get(), 1);
    assert_eq!(control.five_minute_bar_count().get(), 1);
    // TDX's normalized contract keeps both bar bounds at the source label:
    // daily source_at is date-only, and intraday source_at retains HH:MM.
    assert_eq!(control.daily_bars()[0].bar_start(), "2026-08-14");
    assert_eq!(control.daily_bars()[0].bar_end(), "2026-08-14");
    assert_eq!(control.daily_bars()[0].source_at(), Some("2026-08-14"));
    assert_eq!(
        control.five_minute_bars()[0].bar_start(),
        "2026-08-14 14:55:00"
    );
    assert_eq!(
        control.five_minute_bars()[0].bar_end(),
        "2026-08-14 14:55:00"
    );
    assert_eq!(
        control.five_minute_bars()[0].source_at(),
        Some("2026-08-14 14:55")
    );
    assert_eq!(control.input_evidence().len(), 4);
    assert_eq!(
        control.input_evidence()[0].observed_at(),
        "2026-08-14T15:01:00+08:00"
    );
    assert_eq!(control.input_evidence()[2].source_at(), Some("2026-08-14"));

    for (case, field, replacement) in [
        ("missing daily family", "/daily_bars", json!([])),
        ("missing minute family", "/five_minute_bars", json!([])),
        ("mixed quote provider", "/quote/provider", json!("Tencent")),
        (
            "mixed bar provider",
            "/daily_bars/0/provider",
            json!("Tencent"),
        ),
        ("absent evidence", "/input_evidence", json!([])),
        (
            "quote batch substitution",
            "/input_evidence/0/batch_id",
            json!("another-capture"),
        ),
        (
            "quote source time fabricated from observation",
            "/input_evidence/0/source_at",
            json!("2026-08-14T15:01:00+08:00"),
        ),
        (
            "quote observation substitution",
            "/input_evidence/0/observed_at",
            json!("2026-08-14T15:01:01+08:00"),
        ),
        (
            "daily batch substitution",
            "/input_evidence/2/batch_id",
            json!("another-day-capture"),
        ),
        (
            "daily latest source time substitution",
            "/input_evidence/2/source_at",
            json!("2026-08-13"),
        ),
        (
            "mixed evidence provider",
            "/input_evidence/3/provider",
            json!("Tencent"),
        ),
    ] {
        let mut corrupt = payload.clone();
        *corrupt
            .pointer_mut(field)
            .expect("known public payload field") = replacement;
        assert!(
            serde_json::from_value::<T0EvidenceRecord>(corrupt).is_err(),
            "{case} must not yield a complete T0 evidence record"
        );
    }

    let mut missing_evidence_family = payload;
    missing_evidence_family["input_evidence"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(
        serde_json::from_value::<T0EvidenceRecord>(missing_evidence_family).is_err(),
        "three evidence objects cannot certify all four required input families"
    );
}

#[test]
fn t0_capture_rejects_stale_conflicted_or_unsupported_quote_and_book_statuses() {
    let capture = complete_tdx_capture();
    let control_wire = serde_json::to_value(&capture).unwrap();
    let control: T0EvidenceRecord = serde_json::from_value(control_wire.clone()).unwrap();
    assert_eq!(control.quote().status(), DataStatus::Unavailable);
    assert_eq!(control.order_book().status(), DataStatus::Unavailable);
    assert_eq!(control.quote().source_at(), None);
    assert_eq!(control.order_book().source_at(), None);
    assert_eq!(control.requested_at(), "2026-08-14T15:00:30.123456+08:00");

    // These are valid Core states, but they cannot certify this derived bundle.
    // Change only one family's status: identities, bars and evidence stay fixed.
    for family in ["quote", "order_book"] {
        for status in [
            DataStatus::Stale,
            DataStatus::Conflicted,
            DataStatus::Unsupported,
        ] {
            let mut rejected_wire = control_wire.clone();
            rejected_wire[family]["status"] = json!(status);
            let quote: Quote = serde_json::from_value(rejected_wire["quote"].clone()).unwrap();
            let book: OrderBook =
                serde_json::from_value(rejected_wire["order_book"].clone()).unwrap();
            assert_eq!(
                if family == "quote" {
                    quote.status()
                } else {
                    book.status()
                },
                status,
                "{family} must remain a representable Core state before composition"
            );

            let error = T0EvidenceRecord::new(
                capture.instrument().clone(),
                capture.requested_at(),
                quote,
                book,
                capture.daily_bars().to_vec(),
                capture.five_minute_bars().to_vec(),
                PositiveU32::new(1).unwrap(),
                PositiveU32::new(1).unwrap(),
                capture.input_evidence().to_vec(),
                PositiveU32::new(1).unwrap(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap_err();
            assert!(
                matches!(
                    error,
                    DerivedProductContractError::InvalidRequest(message)
                        if message == "T0 quote/order_book must be Available or explicitly Unavailable"
                ),
                "{family} with {status:?} must not certify a complete T0 bundle"
            );

            let error = serde_json::from_value::<T0EvidenceRecord>(rejected_wire).unwrap_err();
            assert!(error.is_data(), "{family} with {status:?}: {error}");
            assert!(
                error
                    .to_string()
                    .contains("T0 quote/order_book must be Available or explicitly Unavailable"),
                "{family} with {status:?}: {error}"
            );
        }
    }
}

#[test]
fn outcome_daily_bars_require_the_requested_session_and_one_original_tdx_batch() {
    let capture = complete_tdx_capture();
    let record = OutcomeDailyBarsRecord::new(
        capture.instrument().clone(),
        IsoDate::new("2026-08-14").unwrap(),
        "2026-08-14T15:00:00+08:00",
        capture.daily_bars().to_vec(),
        PositiveU32::new(1).unwrap(),
        vec![capture.input_evidence()[2].clone()],
        PositiveU32::new(2).unwrap(),
        "0123456789abcdef".repeat(4),
    )
    .unwrap();
    let payload = serde_json::to_value(record).unwrap();
    let decoded: OutcomeDailyBarsRecord = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(decoded.instrument().code(), "600396");
    assert_eq!(decoded.requested_through().as_str(), "2026-08-14");
    assert_eq!(decoded.outcome_due_at(), "2026-08-14T15:00:00+08:00");
    assert_eq!(decoded.requested_limit().get(), 1);
    assert_eq!(decoded.bars()[0].bar_end(), "2026-08-14");
    assert_eq!(decoded.input_evidence()[0].batch_id(), "day-capture");
    assert_eq!(decoded.algorithm_revision().get(), 2);
    assert_eq!(
        decoded.input_digest_sha256(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    );

    for (case, field, replacement, expected_error) in [
        (
            "before session close",
            "/outcome_due_at",
            json!("2026-08-14T14:59:59+08:00"),
            "must not precede",
        ),
        (
            "absent bar source time",
            "/bars/0/source_at",
            json!(null),
            "require source_at and observed_at",
        ),
        (
            "absent bar observation time",
            "/bars/0/observed_at",
            json!(null),
            "require source_at and observed_at",
        ),
        (
            "different bar identity",
            "/bars/0/instrument/code",
            json!("600397"),
            "bar instrument mismatch",
        ),
        (
            "substituted evidence",
            "/input_evidence/0/batch_id",
            json!("substituted-batch"),
            "record evidence contradicts",
        ),
        (
            "different latest source time",
            "/input_evidence/0/source_at",
            json!("2026-08-13"),
            "latest source time contradicts",
        ),
        (
            "different algorithm",
            "/algorithm_id",
            json!("magic.t0_evidence"),
            "algorithm_id must be exactly",
        ),
        (
            "uppercase digest",
            "/input_digest_sha256",
            json!("A".repeat(64)),
            "64 lowercase hexadecimal",
        ),
    ] {
        let mut corrupt = payload.clone();
        *corrupt.pointer_mut(field).unwrap() = replacement;
        let error = serde_json::from_value::<OutcomeDailyBarsRecord>(corrupt).unwrap_err();
        assert!(
            error.to_string().contains(expected_error),
            "{case}: {error}"
        );
    }

    let mut missing_last_session = payload.clone();
    missing_last_session["requested_through"] = json!("2026-08-15");
    missing_last_session["outcome_due_at"] = json!("2026-08-15T15:00:00+08:00");
    let error = serde_json::from_value::<OutcomeDailyBarsRecord>(missing_last_session).unwrap_err();
    assert!(error.to_string().contains("newest outcome daily bar"));

    let mut extra_batch = payload;
    let evidence = extra_batch["input_evidence"][0].clone();
    extra_batch["input_evidence"]
        .as_array_mut()
        .unwrap()
        .push(evidence);
    let error = serde_json::from_value::<OutcomeDailyBarsRecord>(extra_batch).unwrap_err();
    assert!(error.to_string().contains("exactly one bar batch"));
}

fn shape_with_input_evidence(
    input_evidence: Vec<SourceEvidence>,
) -> Result<IntradayShapeRecord, DerivedProductContractError> {
    IntradayShapeRecord::new(
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
        IsoDate::new("2026-08-14").unwrap(),
        "2026-08-14T09:30:00+08:00",
        "2026-08-14T09:32:00+08:00",
        PositiveU32::new(3).unwrap(),
        Price::new(10.0).unwrap(),
        Price::new(11.0).unwrap(),
        Price::new(10.0).unwrap(),
        Price::new(11.0).unwrap(),
        None,
        None,
        None,
        2,
        0,
        1,
        input_evidence,
        PositiveU32::new(1).unwrap(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
}

fn two_source_ordered_inputs() -> Vec<SourceEvidence> {
    vec![
        SourceEvidence::new(
            ProviderId::Tencent,
            "2026-08-14T09:31:00+08:00",
            "minute-window-first",
        )
        .unwrap(),
        SourceEvidence::new(
            ProviderId::Tencent,
            "2026-08-14T09:33:00+08:00",
            "minute-window-second",
        )
        .unwrap()
        .with_source_at("2026-08-14T09:32:00+08:00")
        .unwrap(),
    ]
}

#[test]
fn intraday_shape_endpoints_require_ordered_instants_within_the_requested_trading_date() {
    let record = shape_with_input_evidence(two_source_ordered_inputs()).unwrap();
    let control_wire = serde_json::to_value(&record).unwrap();
    let control: IntradayShapeRecord = serde_json::from_value(control_wire.clone()).unwrap();
    assert_eq!(control.trading_date().as_str(), "2026-08-14");
    assert_eq!(control.first_at(), "2026-08-14T09:30:00+08:00");
    assert_eq!(control.last_at(), "2026-08-14T09:32:00+08:00");

    for (case, first_at, last_at) in [
        (
            "first instant after last instant",
            "2026-08-14T09:33:00+08:00",
            "2026-08-14T09:32:00+08:00",
        ),
        (
            "first endpoint on the previous date",
            "2026-08-13T15:00:00+08:00",
            "2026-08-14T09:32:00+08:00",
        ),
        (
            "last endpoint on the following date",
            "2026-08-14T09:30:00+08:00",
            "2026-08-15T09:32:00+08:00",
        ),
        // Independent worked oracle: 09:30+08:00 is 01:30Z, whereas
        // 09:31+09:00 is 00:31Z. Increasing wall-clock text is not chronology.
        (
            "wall clock rises while the actual instant falls",
            "2026-08-14T09:30:00+08:00",
            "2026-08-14T09:31:00+09:00",
        ),
    ] {
        let error = IntradayShapeRecord::new(
            record.instrument().clone(),
            IsoDate::new("2026-08-14").unwrap(),
            first_at,
            last_at,
            record.point_count(),
            record.open(),
            record.high(),
            record.low(),
            record.latest(),
            record.vwap(),
            record.cumulative_volume(),
            record.cumulative_amount(),
            record.up_points(),
            record.down_points(),
            record.flat_points(),
            record.input_evidence().to_vec(),
            record.algorithm_revision(),
            record.input_digest_sha256(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                DerivedProductContractError::InvalidRequest(message)
                    if message == "intraday endpoints must be ordered instants on trading_date"
            ),
            "{case} must not create an intraday shape"
        );

        let mut rejected_wire = control_wire.clone();
        rejected_wire["first_at"] = json!(first_at);
        rejected_wire["last_at"] = json!(last_at);
        let error = serde_json::from_value::<IntradayShapeRecord>(rejected_wire).unwrap_err();
        assert!(error.is_data(), "{case}: {error}");
        assert!(
            error
                .to_string()
                .contains("intraday endpoints must be ordered instants on trading_date"),
            "{case}: {error}"
        );
    }
}

#[test]
fn intraday_shape_price_range_must_include_both_open_and_latest_prices() {
    let record = shape_with_input_evidence(two_source_ordered_inputs()).unwrap();
    let control_wire = serde_json::to_value(&record).unwrap();
    let control: IntradayShapeRecord = serde_json::from_value(control_wire.clone()).unwrap();
    assert_eq!(control.open().get(), 10.0);
    assert_eq!(control.latest().get(), 11.0);
    assert_eq!(control.low().get(), 10.0);
    assert_eq!(control.high().get(), 11.0);

    // Independent literal windows: [10.5,11.0] excludes open=10.0;
    // [10.0,10.5] excludes latest=11.0. Both individual prices are valid.
    for (case, low, high) in [
        ("low excludes the opening price", 10.5, 11.0),
        ("high excludes the latest price", 10.0, 10.5),
    ] {
        let error = IntradayShapeRecord::new(
            record.instrument().clone(),
            record.trading_date().clone(),
            record.first_at(),
            record.last_at(),
            record.point_count(),
            record.open(),
            Price::new(high).unwrap(),
            Price::new(low).unwrap(),
            record.latest(),
            record.vwap(),
            record.cumulative_volume(),
            record.cumulative_amount(),
            record.up_points(),
            record.down_points(),
            record.flat_points(),
            record.input_evidence().to_vec(),
            record.algorithm_revision(),
            record.input_digest_sha256(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                DerivedProductContractError::InvalidRequest(message)
                    if message == "intraday OHLC/latest range is inconsistent"
            ),
            "{case} must not form a valid intraday shape"
        );

        let mut rejected_wire = control_wire.clone();
        rejected_wire["low"] = json!(low);
        rejected_wire["high"] = json!(high);
        let error = serde_json::from_value::<IntradayShapeRecord>(rejected_wire).unwrap_err();
        assert!(error.is_data(), "{case}: {error}");
        assert!(
            error
                .to_string()
                .contains("intraday OHLC/latest range is inconsistent"),
            "{case}: {error}"
        );
    }
}

#[test]
fn intraday_shape_preserves_single_provider_input_order_and_rejects_empty_or_mixed_evidence() {
    let record = shape_with_input_evidence(two_source_ordered_inputs()).unwrap();
    assert_eq!(record.input_evidence().len(), 2);
    assert_eq!(record.input_evidence()[0].provider(), ProviderId::Tencent);
    assert_eq!(record.input_evidence()[0].batch_id(), "minute-window-first");
    assert_eq!(record.input_evidence()[0].source_at(), None);
    assert_eq!(record.input_evidence()[1].provider(), ProviderId::Tencent);
    assert_eq!(
        record.input_evidence()[1].batch_id(),
        "minute-window-second"
    );
    assert_eq!(
        record.input_evidence()[1].source_at(),
        Some("2026-08-14T09:32:00+08:00")
    );

    let payload = serde_json::to_value(record).unwrap();
    let expected_inputs = json!([
        {
            "provider": "Tencent",
            "source_at": null,
            "observed_at": "2026-08-14T09:31:00+08:00",
            "batch_id": "minute-window-first"
        },
        {
            "provider": "Tencent",
            "source_at": "2026-08-14T09:32:00+08:00",
            "observed_at": "2026-08-14T09:33:00+08:00",
            "batch_id": "minute-window-second"
        }
    ]);
    assert_eq!(payload["input_evidence"], expected_inputs);
    let decoded: IntradayShapeRecord = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(decoded).unwrap()["input_evidence"],
        expected_inputs
    );

    let empty = shape_with_input_evidence(Vec::new()).unwrap_err();
    assert!(matches!(
        empty,
        DerivedProductContractError::InvalidRequest(_)
    ));
    let mut mixed = two_source_ordered_inputs();
    mixed[1] = SourceEvidence::new(
        ProviderId::Tdx,
        "2026-08-14T09:33:00+08:00",
        "minute-window-second",
    )
    .unwrap()
    .with_source_at("2026-08-14T09:32:00+08:00")
    .unwrap();
    let mixed = shape_with_input_evidence(mixed).unwrap_err();
    assert!(matches!(
        mixed,
        DerivedProductContractError::InvalidRequest(_)
    ));

    let mut empty_wire = payload.clone();
    empty_wire["input_evidence"] = json!([]);
    assert!(serde_json::from_value::<IntradayShapeRecord>(empty_wire)
        .unwrap_err()
        .is_data());
    let mut mixed_wire = payload;
    mixed_wire["input_evidence"][1]["provider"] = json!("Tdx");
    assert!(serde_json::from_value::<IntradayShapeRecord>(mixed_wire)
        .unwrap_err()
        .is_data());
}

#[test]
fn intraday_shape_wire_direction_counts_require_exact_total_without_overflow() {
    let record = shape_with_input_evidence(two_source_ordered_inputs()).unwrap();
    let payload = serde_json::to_value(record).unwrap();
    let control: IntradayShapeRecord = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(control.point_count().get(), 3);
    assert_eq!(control.up_points(), 2);
    assert_eq!(control.down_points(), 0);
    assert_eq!(control.flat_points(), 1);

    let mut mismatch = payload.clone();
    mismatch["up_points"] = json!(1);
    let error = serde_json::from_value::<IntradayShapeRecord>(mismatch).unwrap_err();
    assert!(error.is_data());
    assert!(error
        .to_string()
        .contains("direction counts must equal point_count"));

    let mut overflow = payload;
    overflow["up_points"] = json!(4_294_967_295_u32);
    overflow["down_points"] = json!(1);
    overflow["flat_points"] = json!(0);
    let error = serde_json::from_value::<IntradayShapeRecord>(overflow).unwrap_err();
    assert!(error.is_data());
    assert!(error.to_string().contains("direction counts overflow u32"));
}

mod upper_limit_review_summary_public_contract {
    use crate::{DerivedProductContractError, UpperLimitPoolReviewRecord};
    use magic_market_core::{IsoDate, LimitPoolEntry, LimitPoolKind, PositiveU32, SourceEvidence};
    use serde_json::{json, Value};

    // Hand-authored typed-record specimen, not a live response/admission witness.
    // Independent worked oracle: Upper=2, Broken=1, Lower=0, PreviousUpper=1;
    // streaks are 2, 3, null, 7, so the cross-family maximum is exactly 7.
    // This digest is a syntactically valid specimen field, not a measured input hash.
    const CONTROL_WIRE: &str = r#"{
      "trading_date":"2026-08-14",
      "upper":[
        {
          "kind":"Upper",
          "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
          "trading_date":"2026-08-14",
          "price":10.0,"change":{"value":10.0,"unit":"Percent"},
          "volume":0.0,"turnover":null,"sealed_amount":null,
          "first_seal_at":null,"last_seal_at":null,"break_count":0,"streak":2,
          "industry":null,"board_name":null,"seal_state":null,"reseal_count":null,"reason":null,"is_st":null,
          "evidence":{"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:01+08:00","batch_id":"review-upper"}
        },
        {
          "kind":"Upper",
          "instrument":{"exchange":"Shenzhen","code":"000001","asset_class":"Equity"},
          "trading_date":"2026-08-14",
          "price":11.0,"change":{"value":10.0,"unit":"Percent"},
          "volume":null,"turnover":null,"sealed_amount":null,
          "first_seal_at":null,"last_seal_at":null,"break_count":null,"streak":3,
          "industry":null,"board_name":null,"seal_state":null,"reseal_count":null,"reason":null,"is_st":null,
          "evidence":{"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:01+08:00","batch_id":"review-upper"}
        }
      ],
      "broken":[
        {
          "kind":"Broken",
          "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
          "trading_date":"2026-08-14",
          "price":12.0,"change":{"value":5.0,"unit":"Percent"},
          "volume":null,"turnover":null,"sealed_amount":null,
          "first_seal_at":null,"last_seal_at":null,"break_count":1,"streak":null,
          "industry":null,"board_name":null,"seal_state":null,"reseal_count":null,"reason":null,"is_st":null,
          "evidence":{"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:02+08:00","batch_id":"review-broken"}
        }
      ],
      "lower":[],
      "previous_upper":[
        {
          "kind":"PreviousUpper",
          "instrument":{"exchange":"Shenzhen","code":"300750","asset_class":"Equity"},
          "trading_date":"2026-08-14",
          "price":13.0,"change":{"value":-2.0,"unit":"Percent"},
          "volume":null,"turnover":null,"sealed_amount":null,
          "first_seal_at":null,"last_seal_at":null,"break_count":null,"streak":7,
          "industry":null,"board_name":null,"seal_state":null,"reseal_count":null,"reason":null,"is_st":null,
          "evidence":{"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:04+08:00","batch_id":"review-previous-upper"}
        }
      ],
      "upper_count":2,"broken_count":1,"lower_count":0,"previous_upper_count":1,
      "maximum_streak":7,
      "input_evidence":[
        {"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:01+08:00","batch_id":"review-upper"},
        {"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:02+08:00","batch_id":"review-broken"},
        {"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:03+08:00","batch_id":"review-lower"},
        {"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:04+08:00","batch_id":"review-previous-upper"}
      ],
      "algorithm_id":"magic.upper_limit_pool_review","algorithm_revision":1,
      "input_digest_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }"#;

    #[test]
    fn four_pool_review_wire_summary_cannot_contradict_its_original_pools() {
        let wire: Value = serde_json::from_str(CONTROL_WIRE).unwrap();
        let upper: Vec<LimitPoolEntry> = serde_json::from_value(wire["upper"].clone()).unwrap();
        let broken: Vec<LimitPoolEntry> = serde_json::from_value(wire["broken"].clone()).unwrap();
        let lower: Vec<LimitPoolEntry> = serde_json::from_value(wire["lower"].clone()).unwrap();
        let previous_upper: Vec<LimitPoolEntry> =
            serde_json::from_value(wire["previous_upper"].clone()).unwrap();
        let evidence: Vec<SourceEvidence> =
            serde_json::from_value(wire["input_evidence"].clone()).unwrap();

        let constructed = UpperLimitPoolReviewRecord::new(
            IsoDate::new("2026-08-14").unwrap(),
            upper.clone(),
            broken.clone(),
            lower.clone(),
            previous_upper.clone(),
            evidence.clone(),
            PositiveU32::new(1).unwrap(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap();
        // Full independent literal wire oracle: exact rows/order, null vs zero,
        // original four evidence objects, family counts, maximum and metadata.
        assert_eq!(serde_json::to_value(&constructed).unwrap(), wire);
        assert_eq!(constructed.upper_count(), 2);
        assert_eq!(constructed.broken_count(), 1);
        assert_eq!(constructed.lower_count(), 0);
        assert_eq!(constructed.previous_upper_count(), 1);
        assert_eq!(constructed.maximum_streak(), Some(7));

        let decoded: UpperLimitPoolReviewRecord = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), wire);

        // The public constructor rejects a mislabelled row before allowing it
        // to participate in the family summary. No private validator is called.
        let mut wrong_family = upper;
        wrong_family[0].kind = LimitPoolKind::Broken;
        let error = UpperLimitPoolReviewRecord::new(
            IsoDate::new("2026-08-14").unwrap(),
            wrong_family,
            broken,
            lower,
            previous_upper,
            evidence,
            PositiveU32::new(1).unwrap(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DerivedProductContractError::InvalidRequest(message)
                if message == "limit-pool entry kind contradicts its response family"
        ));

        // Only the serialized claim changes; every source row/evidence stays
        // fixed. Deserialize must reject, not silently recompute a valid record.
        for (field, forged) in [
            ("upper_count", 3),
            ("broken_count", 0),
            ("lower_count", 1),
            ("previous_upper_count", 0),
            ("maximum_streak", 3),
        ] {
            let mut inconsistent = wire.clone();
            inconsistent[field] = json!(forged);
            let error =
                serde_json::from_value::<UpperLimitPoolReviewRecord>(inconsistent).unwrap_err();
            assert!(error.is_data(), "{field}: {error}");
            assert!(
                error
                    .to_string()
                    .contains("UpperLimitPoolReview derived counts contradict its pools"),
                "{field}: {error}"
            );
        }
    }
}

mod upper_limit_review_date_public_contract {
    use crate::{DerivedProductContractError, UpperLimitPoolReviewRecord};
    use magic_market_core::{IsoDate, LimitPoolEntry, PositiveU32, SourceEvidence};
    use serde_json::{json, Value};

    // Independent typed source specimen, not a live response or admission witness.
    // Counts are 2/1/0/1; streaks 2, 3, null, 7 give the literal maximum 7.
    // The digest is only a syntactically valid fixture field, not an input commitment.
    const CONTROL_WIRE: &str = r#"{
      "trading_date":"2026-08-14",
      "upper":[
        {
          "kind":"Upper",
          "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
          "trading_date":"2026-08-14",
          "price":10.0,"change":{"value":10.0,"unit":"Percent"},
          "volume":0.0,"turnover":null,"sealed_amount":null,
          "first_seal_at":null,"last_seal_at":null,"break_count":0,"streak":2,
          "industry":null,"board_name":null,"seal_state":null,"reseal_count":null,"reason":null,"is_st":null,
          "evidence":{"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:01+08:00","batch_id":"review-upper"}
        },
        {
          "kind":"Upper",
          "instrument":{"exchange":"Shenzhen","code":"000001","asset_class":"Equity"},
          "trading_date":"2026-08-14",
          "price":11.0,"change":{"value":10.0,"unit":"Percent"},
          "volume":null,"turnover":null,"sealed_amount":null,
          "first_seal_at":null,"last_seal_at":null,"break_count":null,"streak":3,
          "industry":null,"board_name":null,"seal_state":null,"reseal_count":null,"reason":null,"is_st":null,
          "evidence":{"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:01+08:00","batch_id":"review-upper"}
        }
      ],
      "broken":[
        {
          "kind":"Broken",
          "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
          "trading_date":"2026-08-14",
          "price":12.0,"change":{"value":5.0,"unit":"Percent"},
          "volume":null,"turnover":null,"sealed_amount":null,
          "first_seal_at":null,"last_seal_at":null,"break_count":1,"streak":null,
          "industry":null,"board_name":null,"seal_state":null,"reseal_count":null,"reason":null,"is_st":null,
          "evidence":{"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:02+08:00","batch_id":"review-broken"}
        }
      ],
      "lower":[],
      "previous_upper":[
        {
          "kind":"PreviousUpper",
          "instrument":{"exchange":"Shenzhen","code":"300750","asset_class":"Equity"},
          "trading_date":"2026-08-14",
          "price":13.0,"change":{"value":-2.0,"unit":"Percent"},
          "volume":null,"turnover":null,"sealed_amount":null,
          "first_seal_at":null,"last_seal_at":null,"break_count":null,"streak":7,
          "industry":null,"board_name":null,"seal_state":null,"reseal_count":null,"reason":null,"is_st":null,
          "evidence":{"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:04+08:00","batch_id":"review-previous-upper"}
        }
      ],
      "upper_count":2,"broken_count":1,"lower_count":0,"previous_upper_count":1,
      "maximum_streak":7,
      "input_evidence":[
        {"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:01+08:00","batch_id":"review-upper"},
        {"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:02+08:00","batch_id":"review-broken"},
        {"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:03+08:00","batch_id":"review-lower"},
        {"provider":"Eastmoney","source_at":"2026-08-14","observed_at":"2026-08-14T15:05:04+08:00","batch_id":"review-previous-upper"}
      ],
      "algorithm_id":"magic.upper_limit_pool_review","algorithm_revision":1,
      "input_digest_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }"#;

    #[test]
    fn four_pool_review_rejects_adjacent_dates_in_previous_upper_without_rewriting_evidence() {
        let wire: Value = serde_json::from_str(CONTROL_WIRE).unwrap();
        let upper: Vec<LimitPoolEntry> = serde_json::from_value(wire["upper"].clone()).unwrap();
        let broken: Vec<LimitPoolEntry> = serde_json::from_value(wire["broken"].clone()).unwrap();
        let lower: Vec<LimitPoolEntry> = serde_json::from_value(wire["lower"].clone()).unwrap();
        let previous_upper: Vec<LimitPoolEntry> =
            serde_json::from_value(wire["previous_upper"].clone()).unwrap();
        let evidence: Vec<SourceEvidence> =
            serde_json::from_value(wire["input_evidence"].clone()).unwrap();
        assert_eq!(previous_upper.len(), 1);

        let constructed = UpperLimitPoolReviewRecord::new(
            IsoDate::new("2026-08-14").unwrap(),
            upper.clone(),
            broken.clone(),
            lower.clone(),
            previous_upper.clone(),
            evidence.clone(),
            PositiveU32::new(1).unwrap(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap();
        // Whole literal oracle retains row order, original evidence and null vs zero.
        assert_eq!(serde_json::to_value(&constructed).unwrap(), wire);
        assert_eq!(constructed.trading_date().as_str(), "2026-08-14");
        assert_eq!(
            (
                constructed.upper_count(),
                constructed.broken_count(),
                constructed.lower_count(),
                constructed.previous_upper_count(),
            ),
            (2, 1, 0, 1)
        );
        assert_eq!(constructed.maximum_streak(), Some(7));
        assert_eq!(constructed.upper()[0].volume.unwrap().get(), 0.0);
        assert_eq!(constructed.upper()[1].volume, None);
        assert!(constructed.lower().is_empty());
        let decoded: UpperLimitPoolReviewRecord = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), wire);

        for adjacent_date in ["2026-08-13", "2026-08-15"] {
            let mut wrong_previous_upper = previous_upper.clone();
            wrong_previous_upper.last_mut().unwrap().trading_date =
                IsoDate::new(adjacent_date).unwrap();
            let mut wrong_row_wire = wire["previous_upper"][0].clone();
            wrong_row_wire["trading_date"] = json!(adjacent_date);
            assert_eq!(
                serde_json::to_value(&wrong_previous_upper[0]).unwrap(),
                wrong_row_wire
            );
            assert_eq!(wrong_previous_upper[0].evidence, previous_upper[0].evidence);

            // Only the final family's row date changes; all earlier families stay valid.
            let error = UpperLimitPoolReviewRecord::new(
                IsoDate::new("2026-08-14").unwrap(),
                upper.clone(),
                broken.clone(),
                lower.clone(),
                wrong_previous_upper,
                evidence.clone(),
                PositiveU32::new(1).unwrap(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap_err();
            assert!(
                matches!(
                    error,
                    DerivedProductContractError::InvalidRequest(message)
                        if message == "limit-pool entry trading_date contradicts the requested date"
                ),
                "{adjacent_date} must not become part of the requested date's review"
            );

            let mut wrong_wire = wire.clone();
            wrong_wire["previous_upper"][0]["trading_date"] = json!(adjacent_date);
            // A standalone public Core row remains valid: rejection must be compositional.
            let core_row: LimitPoolEntry =
                serde_json::from_value(wrong_wire["previous_upper"][0].clone()).unwrap();
            assert_eq!(serde_json::to_value(core_row).unwrap(), wrong_row_wire);
            let error =
                serde_json::from_value::<UpperLimitPoolReviewRecord>(wrong_wire).unwrap_err();
            assert!(error.is_data(), "{adjacent_date}: {error}");
            assert!(
                error
                    .to_string()
                    .contains("limit-pool entry trading_date contradicts the requested date"),
                "{adjacent_date}: {error}"
            );
        }
        assert_eq!(serde_json::to_value(constructed).unwrap(), wire);
    }
}

mod t0_bar_family_public_contract {
    use crate::{DerivedProductContractError, T0EvidenceRecord};
    use magic_market_core::{Bar, PositiveU32};
    use serde_json::{json, Value};

    const OBSERVED_AT: &str = "2026-08-14T15:01:00+08:00";
    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    // A complete literal wire oracle, not serialization of the product under test.
    fn bar_wire(interval: &str, label: &str, source_at: &str, batch_id: &str) -> Value {
        json!({
            "instrument": {"exchange": "Shanghai", "code": "600396", "asset_class": "Equity"},
            "interval": interval,
            "bar_start": label,
            "bar_end": label,
            "open": 10.0,
            "high": 11.0,
            "low": 9.0,
            "close": 10.5,
            "volume": 100.0,
            "amount": 100000.0,
            "adjustment": "Unadjusted",
            "source_at": source_at,
            "observed_at": OBSERVED_AT,
            "provider": "Tdx",
            "batch_id": batch_id
        })
    }

    fn control_wire() -> Value {
        json!({
            "instrument": {"exchange": "Shanghai", "code": "600396", "asset_class": "Equity"},
            "requested_at": "2026-08-14T15:00:30.123456+08:00",
            "quote": {
                "instrument": {"exchange": "Shanghai", "code": "600396", "asset_class": "Equity"},
                "name": "Example",
                "price": 10.5,
                "previous_close": 10.0,
                "open": 10.0,
                "high": 11.0,
                "low": 9.0,
                "change_percent": null,
                "volume": 100.0,
                "amount": 100000.0,
                "status": "Unavailable",
                "source_at": null,
                "observed_at": OBSERVED_AT,
                "provider": "Tdx",
                "batch_id": "quote-capture"
            },
            "order_book": {
                "instrument": {"exchange": "Shanghai", "code": "600396", "asset_class": "Equity"},
                "bids": [
                    {"price": 10.0, "quantity": 100.0},
                    {"price": 10.0, "quantity": 100.0},
                    {"price": 10.0, "quantity": 100.0},
                    {"price": 10.0, "quantity": 100.0},
                    {"price": 10.0, "quantity": 100.0}
                ],
                "asks": [
                    {"price": 10.0, "quantity": 100.0},
                    {"price": 10.0, "quantity": 100.0},
                    {"price": 10.0, "quantity": 100.0},
                    {"price": 10.0, "quantity": 100.0},
                    {"price": 10.0, "quantity": 100.0}
                ],
                "total_bid_quantity": 500.0,
                "total_ask_quantity": 500.0,
                "status": "Unavailable",
                "source_at": null,
                "observed_at": OBSERVED_AT,
                "provider": "Tdx",
                "batch_id": "book-capture"
            },
            "daily_bar_count": 3,
            "five_minute_bar_count": 3,
            "daily_bars": [
                bar_wire("Day", "2026-08-12", "2026-08-12", "day-capture"),
                bar_wire("Day", "2026-08-13", "2026-08-13", "day-capture"),
                bar_wire("Day", "2026-08-14", "2026-08-14", "day-capture")
            ],
            "five_minute_bars": [
                bar_wire("Minute5", "2026-08-14 14:45:00", "2026-08-14 14:45", "minute5-capture"),
                bar_wire("Minute5", "2026-08-14 14:50:00", "2026-08-14 14:50", "minute5-capture"),
                bar_wire("Minute5", "2026-08-14 14:55:00", "2026-08-14 14:55", "minute5-capture")
            ],
            "input_evidence": [
                {"provider": "Tdx", "observed_at": OBSERVED_AT, "batch_id": "quote-capture", "source_at": null},
                {"provider": "Tdx", "observed_at": OBSERVED_AT, "batch_id": "book-capture", "source_at": null},
                {"provider": "Tdx", "observed_at": OBSERVED_AT, "batch_id": "day-capture", "source_at": "2026-08-14"},
                {"provider": "Tdx", "observed_at": OBSERVED_AT, "batch_id": "minute5-capture", "source_at": "2026-08-14 14:55"}
            ],
            "algorithm_id": "magic.t0_evidence",
            "algorithm_revision": 1,
            "input_digest_sha256": DIGEST
        })
    }

    // Component deserialization invokes each public Core value's checked boundary.
    // Only the derived checked constructor is the observation under test here.
    fn construct(wire: &Value) -> Result<T0EvidenceRecord, DerivedProductContractError> {
        T0EvidenceRecord::new(
            serde_json::from_value(wire["instrument"].clone()).unwrap(),
            wire["requested_at"].as_str().unwrap(),
            serde_json::from_value(wire["quote"].clone()).unwrap(),
            serde_json::from_value(wire["order_book"].clone()).unwrap(),
            serde_json::from_value(wire["daily_bars"].clone()).unwrap(),
            serde_json::from_value(wire["five_minute_bars"].clone()).unwrap(),
            serde_json::from_value(wire["daily_bar_count"].clone()).unwrap(),
            serde_json::from_value(wire["five_minute_bar_count"].clone()).unwrap(),
            serde_json::from_value(wire["input_evidence"].clone()).unwrap(),
            PositiveU32::new(1).unwrap(),
            DIGEST,
        )
    }

    fn reject_through_both_public_boundaries(case: &str, wire: Value, expected: &str) {
        // A Core decoder rejection cannot stand in for the derived error oracle.
        for family in ["daily_bars", "five_minute_bars"] {
            let bars: Vec<Bar> = serde_json::from_value(wire[family].clone()).unwrap();
            assert_eq!(bars.len(), wire[family].as_array().unwrap().len(), "{case}");
            assert_eq!(serde_json::to_value(bars).unwrap(), wire[family], "{case}");
        }
        let error = construct(&wire).unwrap_err();
        match error {
            DerivedProductContractError::InvalidRequest(message) => {
                assert_eq!(message, expected, "{case}");
            }
            other => panic!("{case}: expected derived InvalidRequest, got {other:?}"),
        }
        let error = serde_json::from_value::<T0EvidenceRecord>(wire).unwrap_err();
        assert!(
            error.is_data(),
            "{case}: expected contract, not JSON syntax failure"
        );
        assert_eq!(
            error.to_string(),
            format!("invalid derived-product request: {expected}"),
            "{case}"
        );
    }

    fn bounded_calendar_wire(family: &str, count: u32) -> Value {
        let mut wire = control_wire();
        let mut date = time::Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
        let mut rows = Vec::new();
        for _ in 0..count {
            let day = format!(
                "{:04}-{:02}-{:02}",
                date.year(),
                u8::from(date.month()),
                date.day()
            );
            rows.push(if family == "daily_bars" {
                bar_wire("Day", &day, &day, "day-capture")
            } else {
                bar_wire(
                    "Minute5",
                    &format!("{day} 14:55:00"),
                    &format!("{day} 14:55"),
                    "minute5-capture",
                )
            });
            date = date.next_day().unwrap();
        }
        let last_source = rows.last().unwrap()["source_at"].clone();
        let (count_field, evidence_index) = if family == "daily_bars" {
            ("daily_bar_count", 2)
        } else {
            ("five_minute_bar_count", 3)
        };
        wire[family] = json!(rows);
        wire[count_field] = json!(count);
        wire["input_evidence"][evidence_index]["source_at"] = last_source;
        wire
    }

    #[test]
    fn t0_bar_families_require_matching_intervals_strict_order_and_at_most_800_rows() {
        let expected = control_wire();
        let constructed = construct(&expected).unwrap();
        let decoded: T0EvidenceRecord = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(&constructed).unwrap(), expected);
        assert_eq!(serde_json::to_value(&decoded).unwrap(), expected);
        assert_eq!(constructed.daily_bar_count().get(), 3);
        assert_eq!(constructed.five_minute_bar_count().get(), 3);
        assert_eq!(constructed.daily_bars()[0].bar_start(), "2026-08-12");
        assert_eq!(constructed.daily_bars()[1].bar_start(), "2026-08-13");
        assert_eq!(constructed.daily_bars()[2].source_at(), Some("2026-08-14"));
        assert_eq!(
            constructed.five_minute_bars()[2].source_at(),
            Some("2026-08-14 14:55")
        );
        assert_eq!(constructed.quote().source_at(), None);
        assert_eq!(constructed.order_book().source_at(), None);
        assert_eq!(constructed.input_evidence().len(), 4);

        for (family, wrong_interval) in [("daily_bars", "Week"), ("five_minute_bars", "Minute1")] {
            let mut wrong = expected.clone();
            wrong[family][2]["interval"] = json!(wrong_interval);
            reject_through_both_public_boundaries(
                &format!("{family}: wrong interval in the last otherwise valid Core bar"),
                wrong,
                "bar interval mismatch",
            );

            let mut duplicate = expected.clone();
            duplicate[family][1] = duplicate[family][0].clone();
            reject_through_both_public_boundaries(
                &format!("{family}: duplicate middle start"),
                duplicate,
                "bars must be strictly oldest-to-newest",
            );

            let mut descending = expected.clone();
            descending[family].as_array_mut().unwrap().swap(0, 1);
            reject_through_both_public_boundaries(
                &format!("{family}: descending first pair with unchanged newest evidence"),
                descending,
                "bars must be strictly oldest-to-newest",
            );

            let at_limit = bounded_calendar_wire(family, 800);
            let at_limit_record = construct(&at_limit).unwrap();
            let at_limit_decoded: T0EvidenceRecord =
                serde_json::from_value(at_limit.clone()).unwrap();
            assert_eq!(serde_json::to_value(at_limit_record).unwrap(), at_limit);
            assert_eq!(serde_json::to_value(at_limit_decoded).unwrap(), at_limit);
            assert_eq!(at_limit[family].as_array().unwrap().len(), 800);
            assert_eq!(
                at_limit[family][799]["bar_start"],
                if family == "daily_bars" {
                    json!("2026-03-10")
                } else {
                    json!("2026-03-10 14:55:00")
                }
            );

            let over_limit = bounded_calendar_wire(family, 801);
            let rows: Vec<Bar> = serde_json::from_value(over_limit[family].clone()).unwrap();
            assert_eq!(rows.len(), 801);
            assert!(rows
                .windows(2)
                .all(|pair| pair[0].bar_start() < pair[1].bar_start()));
            assert_eq!(
                rows[800].bar_start(),
                if family == "daily_bars" {
                    "2026-03-11"
                } else {
                    "2026-03-11 14:55:00"
                }
            );
            reject_through_both_public_boundaries(
                &format!("{family}: 801 valid ordered rows with matching declared count"),
                over_limit,
                "bar count must be at most 800",
            );
        }

        // Every negative operates on a clone; neither normal public record is mutated.
        assert_eq!(serde_json::to_value(constructed).unwrap(), expected);
        assert_eq!(serde_json::to_value(decoded).unwrap(), expected);
    }
}
