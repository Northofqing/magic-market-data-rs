use magic_market_analysis::{
    cross_source_diagnostics, forward_pe, limit_sentiment, pe_digestion_years, peg,
    simple_moving_average, AttributedValue, CrossSourceObservation,
};
use magic_market_core::{
    Adjustment, AssetClass, Bar, BarInterval, Exchange, FiniteNumber, InstrumentId, IsoDate,
    LimitPoolEntry, LimitPoolKind, Money, NonEmptyText, PositiveU32, Price, ProviderId, Quantity,
    Ratio, RatioUnit, SourceEvidence,
};

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn bar(day: &str, close: f64) -> Bar {
    Bar::new(
        instrument(),
        BarInterval::Day,
        day,
        day,
        Price::new(close).unwrap(),
        Price::new(close).unwrap(),
        Price::new(close).unwrap(),
        Price::new(close).unwrap(),
        Quantity::new(1.0).unwrap(),
        None,
        Adjustment::Unadjusted,
        ProviderId::Tdx,
        "bars",
    )
    .unwrap()
}

fn evidence(provider: ProviderId, batch: &str) -> SourceEvidence {
    SourceEvidence::new(provider, "observed", batch).unwrap()
}

#[test]
fn sma_has_explicit_warmup_and_rejects_bad_order() {
    let bars = [
        bar("2026-07-21", 1.0),
        bar("2026-07-22", 2.0),
        bar("2026-07-23", 3.0),
    ];
    let values = simple_moving_average(&bars, PositiveU32::new(2).unwrap()).unwrap();
    assert_eq!(values.len(), 3);
    assert!(values[0].is_none());
    assert_eq!(values[1].unwrap().get(), 1.5);
    assert_eq!(values[2].unwrap().get(), 2.5);

    let reversed = [bars[1].clone(), bars[0].clone()];
    assert!(simple_moving_average(&reversed, PositiveU32::new(2).unwrap()).is_err());
    assert!(simple_moving_average(&bars, PositiveU32::new(4).unwrap()).is_err());
}

#[test]
fn valuation_math_defines_zero_and_unit_behavior() {
    assert_eq!(
        forward_pe(Price::new(20.0).unwrap(), FiniteNumber::new(2.0).unwrap())
            .unwrap()
            .get(),
        10.0
    );
    assert!(forward_pe(Price::new(20.0).unwrap(), FiniteNumber::new(0.0).unwrap()).is_err());
    assert_eq!(
        peg(
            FiniteNumber::new(20.0).unwrap(),
            Ratio::new(10.0, RatioUnit::Percent).unwrap()
        )
        .unwrap()
        .get(),
        2.0
    );
    assert_eq!(
        peg(
            FiniteNumber::new(20.0).unwrap(),
            Ratio::new(0.1, RatioUnit::Decimal).unwrap()
        )
        .unwrap()
        .get(),
        2.0
    );
    assert!(peg(
        FiniteNumber::new(20.0).unwrap(),
        Ratio::new(0.0, RatioUnit::Percent).unwrap()
    )
    .is_err());
    let years = pe_digestion_years(
        FiniteNumber::new(60.0).unwrap(),
        FiniteNumber::new(30.0).unwrap(),
        Ratio::new(10.0, RatioUnit::Percent).unwrap(),
    )
    .unwrap();
    assert!((years.get() - 7.2725).abs() < 0.001);
}

fn pool(kind: LimitPoolKind, code: &str) -> LimitPoolEntry {
    LimitPoolEntry {
        kind,
        instrument: InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap(),
        trading_date: IsoDate::new("2026-07-23").unwrap(),
        price: Price::new(1.0).unwrap(),
        change: Ratio::new(10.0, RatioUnit::Percent).unwrap(),
        volume: None,
        turnover: None,
        sealed_amount: Some(Money::new(0.0).unwrap()),
        first_seal_at: None,
        last_seal_at: None,
        break_count: None,
        streak: None,
        industry: None,
        board_name: None,
        seal_state: None,
        reseal_count: None,
        reason: None,
        evidence: evidence(ProviderId::Eastmoney, "pool"),
    }
}

#[test]
fn limit_sentiment_handles_empty_denominator_without_fabrication() {
    let empty = limit_sentiment(&[]).unwrap();
    assert!(empty.seal_rate.is_none());
    let sentiment = limit_sentiment(&[
        pool(LimitPoolKind::Upper, "600001"),
        pool(LimitPoolKind::Broken, "600002"),
        pool(LimitPoolKind::Lower, "600003"),
    ])
    .unwrap();
    assert_eq!(sentiment.upper_count, 1);
    assert_eq!(sentiment.broken_count, 1);
    assert_eq!(sentiment.seal_rate.unwrap().get(), 50.0);
}

#[test]
fn cross_source_diagnostics_preserve_inputs_and_reject_duplicates() {
    let observations = [
        CrossSourceObservation::new(
            evidence(ProviderId::Tdx, "tdx"),
            1_000,
            Some(FiniteNumber::new(10.0).unwrap()),
        ),
        CrossSourceObservation::new(
            evidence(ProviderId::Tencent, "tencent"),
            1_250,
            Some(FiniteNumber::new(10.5).unwrap()),
        ),
    ];
    let diagnostics = cross_source_diagnostics(&observations).unwrap();
    assert_eq!(diagnostics.observation_spread_millis, 250);
    assert_eq!(diagnostics.value_spread.unwrap().get(), 0.5);
    assert_eq!(diagnostics.inputs.len(), 2);

    let duplicate = [
        observations[0].clone(),
        CrossSourceObservation::new(
            evidence(ProviderId::Tdx, "tdx-2"),
            2_000,
            Some(FiniteNumber::new(11.0).unwrap()),
        ),
    ];
    assert!(cross_source_diagnostics(&duplicate).is_err());

    let attributed = AttributedValue::new(
        NonEmptyText::new("forward_pe").unwrap(),
        FiniteNumber::new(10.0).unwrap(),
        observations
            .iter()
            .map(|item| item.evidence.clone())
            .collect(),
    )
    .unwrap();
    assert_eq!(attributed.provider, ProviderId::LocalAnalysis);
    assert_eq!(attributed.inputs.len(), 2);
}
