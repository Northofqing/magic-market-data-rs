use magic_market_core::{
    AssetClass, ContractMonth, Exchange, FiniteNumber, InstrumentId, IsoDate, Money, NonEmptyText,
    OptionContract, OptionContractInput, OptionGreeks, OptionGreeksInput, OptionKind, OptionQuote,
    OptionQuoteInput, Price, ProviderId, Quantity, Ratio, RatioUnit, SourceEvidence, SourcedRecord,
};

fn underlying() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "510050", AssetClass::Fund).unwrap()
}

fn evidence() -> SourceEvidence {
    SourceEvidence::new(ProviderId::Sina, "observed", "option").unwrap()
}

fn contract() -> OptionContract {
    OptionContract::new(OptionContractInput {
        contract_code: NonEmptyText::new("10000001").unwrap(),
        underlying: underlying(),
        expiry_month: ContractMonth::new("2026-08").unwrap(),
        expiry: Some(IsoDate::new("2026-08-26").unwrap()),
        kind: OptionKind::Call,
        strike: Some(Price::new(3.0).unwrap()),
        evidence: evidence(),
    })
    .unwrap()
}

#[test]
fn option_contract_quote_and_greeks_are_distinct_records() {
    let quote = OptionQuote::new(OptionQuoteInput {
        contract_code: contract().contract_code().clone(),
        name: Some(NonEmptyText::new("50ETF购8月3000").unwrap()),
        bid: Some(Price::new(0.1).unwrap()),
        bid_quantity: Some(Quantity::new(10.0).unwrap()),
        ask: Some(Price::new(0.11).unwrap()),
        ask_quantity: Some(Quantity::new(12.0).unwrap()),
        last: None,
        previous_close: Some(Price::new(0.09).unwrap()),
        open: Some(Price::new(0.1).unwrap()),
        high: Some(Price::new(0.12).unwrap()),
        low: Some(Price::new(0.08).unwrap()),
        upper_limit: Some(Price::new(0.5).unwrap()),
        lower_limit: Some(Price::new(0.001).unwrap()),
        strike: Some(Price::new(3.0).unwrap()),
        volume: Some(Quantity::new(0.0).unwrap()),
        open_interest: None,
        amount: Some(Money::new(0.0).unwrap()),
        change: Some(Ratio::new(1.25, RatioUnit::Percent).unwrap()),
        amplitude: Some(Ratio::new(4.5, RatioUnit::Percent).unwrap()),
        quote_at: None,
        evidence: evidence(),
    })
    .unwrap();
    let greeks = OptionGreeks::new(OptionGreeksInput {
        contract_code: contract().contract_code().clone(),
        name: Some(NonEmptyText::new("50ETF购8月3000").unwrap()),
        volume: Some(Quantity::new(20.0).unwrap()),
        delta: Some(FiniteNumber::new(0.5).unwrap()),
        gamma: Some(FiniteNumber::new(0.1).unwrap()),
        theta: Some(FiniteNumber::new(-0.01).unwrap()),
        vega: Some(FiniteNumber::new(0.2).unwrap()),
        rho: None,
        implied_volatility: Some(FiniteNumber::new(0.25).unwrap()),
        high: Some(Price::new(0.12).unwrap()),
        low: Some(Price::new(0.08).unwrap()),
        trade_code: Some(NonEmptyText::new("510050C2608M03000").unwrap()),
        strike: Some(Price::new(3.0).unwrap()),
        last: Some(Price::new(0.1).unwrap()),
        theoretical_price: Some(Price::new(0.105).unwrap()),
        evidence: evidence(),
    })
    .unwrap();

    assert_eq!(quote.provider_id(), ProviderId::Sina);
    assert_eq!(quote.bid_quantity().unwrap().get(), 10.0);
    assert_eq!(greeks.delta().unwrap().get(), 0.5);
    assert_eq!(
        serde_json::from_str::<OptionGreeks>(&serde_json::to_string(&greeks).unwrap()).unwrap(),
        greeks
    );
}

#[test]
fn option_asset_identity_is_available_without_weakening_underlying() {
    let option_id = InstrumentId::new(Exchange::Shanghai, "10000001", AssetClass::Option).unwrap();
    assert_eq!(option_id.asset_class(), AssetClass::Option);
    assert_eq!(contract().underlying().asset_class(), AssetClass::Fund);
}

#[test]
fn contract_month_is_checked_and_exact_expiry_and_strike_can_be_absent() {
    let contract = OptionContract::new(OptionContractInput {
        contract_code: NonEmptyText::new("10000002").unwrap(),
        underlying: underlying(),
        expiry_month: ContractMonth::new("2026-09").unwrap(),
        expiry: None,
        kind: OptionKind::Put,
        strike: None,
        evidence: evidence(),
    })
    .unwrap();
    assert_eq!(contract.expiry_month().as_str(), "2026-09");
    assert!(contract.expiry().is_none());
    assert!(contract.strike().is_none());
    assert!(ContractMonth::new("202608").is_err());
    assert!(ContractMonth::new("2026/08").is_err());
    assert!(ContractMonth::new("202X-08").is_err());
    assert!(ContractMonth::new("2026-13").is_err());
    assert!(serde_json::from_str::<ContractMonth>("\"2026-00\"").is_err());
    assert_eq!(
        serde_json::from_str::<OptionContract>(&serde_json::to_string(&contract).unwrap()).unwrap(),
        contract
    );
}

#[test]
fn option_contract_deserialization_rejects_expiry_outside_contract_month() {
    let mut value = serde_json::to_value(contract()).unwrap();
    value["expiry"] = serde_json::json!("2026-09-01");
    assert!(serde_json::from_value::<OptionContract>(value).is_err());
}

#[test]
fn option_quote_deserialization_rechecks_cross_field_invariants() {
    let quote = OptionQuote::new(OptionQuoteInput {
        contract_code: contract().contract_code().clone(),
        name: None,
        bid: Some(Price::new(0.1).unwrap()),
        bid_quantity: Some(Quantity::new(10.0).unwrap()),
        ask: Some(Price::new(0.11).unwrap()),
        ask_quantity: Some(Quantity::new(12.0).unwrap()),
        last: Some(Price::new(0.105).unwrap()),
        previous_close: Some(Price::new(0.09).unwrap()),
        open: Some(Price::new(0.1).unwrap()),
        high: Some(Price::new(0.12).unwrap()),
        low: Some(Price::new(0.08).unwrap()),
        upper_limit: Some(Price::new(0.5).unwrap()),
        lower_limit: Some(Price::new(0.001).unwrap()),
        strike: Some(Price::new(3.0).unwrap()),
        volume: Some(Quantity::new(20.0).unwrap()),
        open_interest: Some(Quantity::new(30.0).unwrap()),
        amount: Some(Money::new(100.0).unwrap()),
        change: Some(Ratio::new(1.0, RatioUnit::Percent).unwrap()),
        amplitude: Some(Ratio::new(2.0, RatioUnit::Percent).unwrap()),
        quote_at: Some(NonEmptyText::new("2026-08-03T14:30:00+08:00").unwrap()),
        evidence: evidence(),
    })
    .unwrap();
    let valid = serde_json::to_value(&quote).unwrap();
    assert_eq!(
        serde_json::from_value::<OptionQuote>(valid.clone()).unwrap(),
        quote
    );

    for (field, invalid) in [
        ("bid_quantity", serde_json::Value::Null),
        ("ask", serde_json::json!(0.09)),
        ("high", serde_json::json!(0.07)),
        ("upper_limit", serde_json::json!(0.0005)),
        ("amount", serde_json::json!(-1.0)),
        ("amplitude", serde_json::json!(-0.01)),
        ("quote_at", serde_json::json!("2026-02-30T14:30:00+08:00")),
        ("quote_at", serde_json::json!("2026-08-03T14:30:00+24:00")),
        ("quote_at", serde_json::json!("2026-08-03T14:30:00+08:60")),
    ] {
        let mut candidate = valid.clone();
        candidate[field] = invalid;
        assert!(
            serde_json::from_value::<OptionQuote>(candidate).is_err(),
            "{field} bypassed option quote validation"
        );
    }

    let mut unicode = valid;
    unicode["quote_at"] = serde_json::json!("2026-07-23T一一一Z");
    let result = std::panic::catch_unwind(|| serde_json::from_value::<OptionQuote>(unicode));
    assert!(result.is_ok(), "malformed Unicode timestamp panicked");
    assert!(result.unwrap().is_err());
}

#[test]
fn option_greeks_deserialization_rechecks_source_domains() {
    let greeks = OptionGreeks::new(OptionGreeksInput {
        contract_code: contract().contract_code().clone(),
        name: None,
        volume: Some(Quantity::new(20.0).unwrap()),
        delta: Some(FiniteNumber::new(0.5).unwrap()),
        gamma: Some(FiniteNumber::new(0.1).unwrap()),
        theta: Some(FiniteNumber::new(-0.01).unwrap()),
        vega: Some(FiniteNumber::new(0.2).unwrap()),
        rho: None,
        implied_volatility: Some(FiniteNumber::new(0.25).unwrap()),
        high: Some(Price::new(0.12).unwrap()),
        low: Some(Price::new(0.08).unwrap()),
        trade_code: None,
        strike: Some(Price::new(3.0).unwrap()),
        last: Some(Price::new(0.1).unwrap()),
        theoretical_price: Some(Price::new(0.105).unwrap()),
        evidence: evidence(),
    })
    .unwrap();
    let valid = serde_json::to_value(&greeks).unwrap();
    assert_eq!(
        serde_json::from_value::<OptionGreeks>(valid.clone()).unwrap(),
        greeks
    );

    for (field, invalid) in [
        ("delta", serde_json::json!(1.01)),
        ("gamma", serde_json::json!(-0.01)),
        ("vega", serde_json::json!(-0.01)),
        ("implied_volatility", serde_json::json!(-0.01)),
        ("high", serde_json::json!(0.07)),
    ] {
        let mut candidate = valid.clone();
        candidate[field] = invalid;
        assert!(
            serde_json::from_value::<OptionGreeks>(candidate).is_err(),
            "{field} bypassed option Greek validation"
        );
    }
}

#[test]
fn option_quote_public_fields_preserve_complete_price_size_and_source_evidence() {
    let quote = OptionQuote::new(OptionQuoteInput {
        contract_code: NonEmptyText::new("10000001").unwrap(),
        name: Some(NonEmptyText::new("50ETF购8月3000").unwrap()),
        bid: Some(Price::new(0.1).unwrap()),
        bid_quantity: Some(Quantity::new(10.0).unwrap()),
        ask: Some(Price::new(0.11).unwrap()),
        ask_quantity: Some(Quantity::new(12.0).unwrap()),
        last: Some(Price::new(0.105).unwrap()),
        previous_close: Some(Price::new(0.09).unwrap()),
        open: Some(Price::new(0.1).unwrap()),
        high: Some(Price::new(0.12).unwrap()),
        low: Some(Price::new(0.08).unwrap()),
        upper_limit: Some(Price::new(0.5).unwrap()),
        lower_limit: Some(Price::new(0.001).unwrap()),
        strike: Some(Price::new(3.0).unwrap()),
        volume: Some(Quantity::new(20.0).unwrap()),
        open_interest: Some(Quantity::new(30.0).unwrap()),
        amount: Some(Money::new(100.0).unwrap()),
        change: Some(Ratio::new(-1.25, RatioUnit::Percent).unwrap()),
        amplitude: Some(Ratio::new(4.5, RatioUnit::Percent).unwrap()),
        quote_at: Some(NonEmptyText::new("2026-08-03T14:30:00Z").unwrap()),
        evidence: SourceEvidence::new(ProviderId::Sina, "2026-08-03T14:30:05Z", "option-full")
            .unwrap()
            .with_source_at("2026-08-03T14:30:00Z")
            .unwrap(),
    })
    .unwrap();
    let restored =
        serde_json::from_value::<OptionQuote>(serde_json::to_value(quote).unwrap()).unwrap();

    assert_eq!(restored.contract_code().as_str(), "10000001");
    assert_eq!(restored.name().unwrap().as_str(), "50ETF购8月3000");
    assert_eq!(restored.bid().unwrap().get(), 0.1);
    assert_eq!(restored.bid_quantity().unwrap().get(), 10.0);
    assert_eq!(restored.ask().unwrap().get(), 0.11);
    assert_eq!(restored.ask_quantity().unwrap().get(), 12.0);
    assert_eq!(restored.last().unwrap().get(), 0.105);
    assert_eq!(restored.previous_close().unwrap().get(), 0.09);
    assert_eq!(restored.open().unwrap().get(), 0.1);
    assert_eq!(restored.high().unwrap().get(), 0.12);
    assert_eq!(restored.low().unwrap().get(), 0.08);
    assert_eq!(restored.upper_limit().unwrap().get(), 0.5);
    assert_eq!(restored.lower_limit().unwrap().get(), 0.001);
    assert_eq!(restored.strike().unwrap().get(), 3.0);
    assert_eq!(restored.volume().unwrap().get(), 20.0);
    assert_eq!(restored.open_interest().unwrap().get(), 30.0);
    assert_eq!(restored.amount().unwrap().get(), 100.0);
    assert_eq!(restored.change().unwrap().get(), -1.25);
    assert_eq!(restored.change().unwrap().unit(), RatioUnit::Percent);
    assert_eq!(restored.amplitude().unwrap().get(), 4.5);
    assert_eq!(
        restored.quote_at().unwrap().as_str(),
        "2026-08-03T14:30:00Z"
    );
    assert_eq!(restored.evidence().observed_at(), "2026-08-03T14:30:05Z");
    assert_eq!(
        restored.evidence().source_at(),
        Some("2026-08-03T14:30:00Z")
    );
    assert_eq!(restored.provider_id(), ProviderId::Sina);
    assert_eq!(restored.evidence_batch_id(), "option-full");
}

#[test]
fn option_greeks_public_fields_preserve_signed_sensitivities_and_source_evidence() {
    let greeks = OptionGreeks::new(OptionGreeksInput {
        contract_code: NonEmptyText::new("10000002").unwrap(),
        name: Some(NonEmptyText::new("50ETF沽8月3000").unwrap()),
        volume: Some(Quantity::new(20.0).unwrap()),
        delta: Some(FiniteNumber::new(-1.0).unwrap()),
        gamma: Some(FiniteNumber::new(0.1).unwrap()),
        theta: Some(FiniteNumber::new(-0.01).unwrap()),
        vega: Some(FiniteNumber::new(0.2).unwrap()),
        rho: Some(FiniteNumber::new(-0.03).unwrap()),
        implied_volatility: Some(FiniteNumber::new(0.25).unwrap()),
        high: Some(Price::new(0.12).unwrap()),
        low: Some(Price::new(0.08).unwrap()),
        trade_code: Some(NonEmptyText::new("510050P2608M03000").unwrap()),
        strike: Some(Price::new(3.0).unwrap()),
        last: Some(Price::new(0.1).unwrap()),
        theoretical_price: Some(Price::new(0.105).unwrap()),
        evidence: SourceEvidence::new(ProviderId::Sina, "2026-08-03T14:30:05Z", "greeks-full")
            .unwrap(),
    })
    .unwrap();
    let restored =
        serde_json::from_value::<OptionGreeks>(serde_json::to_value(greeks).unwrap()).unwrap();

    assert_eq!(restored.contract_code().as_str(), "10000002");
    assert_eq!(restored.name().unwrap().as_str(), "50ETF沽8月3000");
    assert_eq!(restored.volume().unwrap().get(), 20.0);
    assert_eq!(restored.delta().unwrap().get(), -1.0);
    assert_eq!(restored.gamma().unwrap().get(), 0.1);
    assert_eq!(restored.theta().unwrap().get(), -0.01);
    assert_eq!(restored.vega().unwrap().get(), 0.2);
    assert_eq!(restored.rho().unwrap().get(), -0.03);
    assert_eq!(restored.implied_volatility().unwrap().get(), 0.25);
    assert_eq!(restored.high().unwrap().get(), 0.12);
    assert_eq!(restored.low().unwrap().get(), 0.08);
    assert_eq!(restored.trade_code().unwrap().as_str(), "510050P2608M03000");
    assert_eq!(restored.strike().unwrap().get(), 3.0);
    assert_eq!(restored.last().unwrap().get(), 0.1);
    assert_eq!(restored.theoretical_price().unwrap().get(), 0.105);
    assert_eq!(restored.evidence().observed_at(), "2026-08-03T14:30:05Z");
    assert_eq!(restored.evidence().source_at(), None);
    assert_eq!(restored.provider_id(), ProviderId::Sina);
    assert_eq!(restored.evidence_batch_id(), "greeks-full");
}

#[test]
fn option_quote_preserves_signed_change_but_rejects_negative_amplitude_in_constructor_and_wire() {
    let input = |amplitude: Option<f64>| OptionQuoteInput {
        contract_code: NonEmptyText::new("10000001").unwrap(),
        name: None,
        bid: None,
        bid_quantity: None,
        ask: None,
        ask_quantity: None,
        last: None,
        previous_close: None,
        open: None,
        high: None,
        low: None,
        upper_limit: None,
        lower_limit: None,
        strike: None,
        volume: Some(Quantity::new(0.0).unwrap()),
        open_interest: None,
        amount: Some(Money::new(0.0).unwrap()),
        change: Some(Ratio::new(-0.01, RatioUnit::Percent).unwrap()),
        amplitude: amplitude.map(|value| Ratio::new(value, RatioUnit::Percent).unwrap()),
        quote_at: Some(NonEmptyText::new("2026-08-03T14:30:00+08:00").unwrap()),
        evidence: SourceEvidence::new(
            ProviderId::Sina,
            "2026-08-03T14:30:01+08:00",
            "option-amplitude-boundary",
        )
        .unwrap()
        .with_source_at("2026-08-03T14:30:00+08:00")
        .unwrap(),
    };

    // Change is signed; amplitude is non-negative when supplied. Missing
    // source fields stay null, while source-supplied zero volume/amount is data.
    let zero_amplitude_wire = serde_json::json!({
        "contract_code": "10000001",
        "name": null,
        "bid": null,
        "bid_quantity": null,
        "ask": null,
        "ask_quantity": null,
        "last": null,
        "previous_close": null,
        "open": null,
        "high": null,
        "low": null,
        "upper_limit": null,
        "lower_limit": null,
        "strike": null,
        "volume": 0.0,
        "open_interest": null,
        "amount": 0.0,
        "change": {"value": -0.01, "unit": "Percent"},
        "amplitude": {"value": 0.0, "unit": "Percent"},
        "quote_at": "2026-08-03T14:30:00+08:00",
        "evidence": {
            "provider": "Sina",
            "source_at": "2026-08-03T14:30:00+08:00",
            "observed_at": "2026-08-03T14:30:01+08:00",
            "batch_id": "option-amplitude-boundary"
        }
    });
    for (amplitude, literal) in [
        (
            Some(0.0),
            serde_json::json!({"value": 0.0, "unit": "Percent"}),
        ),
        (
            Some(2.0),
            serde_json::json!({"value": 2.0, "unit": "Percent"}),
        ),
        (None, serde_json::Value::Null),
    ] {
        let quote = OptionQuote::new(input(amplitude)).unwrap();
        let mut expected = zero_amplitude_wire.clone();
        expected["amplitude"] = literal;
        assert_eq!(serde_json::to_value(&quote).unwrap(), expected);
        let decoded = serde_json::from_value::<OptionQuote>(expected.clone()).unwrap();
        assert_eq!(decoded, quote);
        assert_eq!(decoded.change().unwrap().get(), -0.01);
        assert_eq!(decoded.change().unwrap().unit(), RatioUnit::Percent);
        assert_eq!(serde_json::to_value(decoded).unwrap(), expected);
    }

    assert_eq!(
        OptionQuote::new(input(Some(-0.01))).unwrap_err(),
        magic_market_core::CoreError::InvalidRequest(
            "option amplitude must be non-negative".into()
        ),
    );

    // A scalar amplitude only fails Ratio's wire shape. Keep the correct
    // {value, unit} shape so the quote's business guard must reject this value.
    let mut negative_amplitude_wire = zero_amplitude_wire;
    negative_amplitude_wire["amplitude"] = serde_json::json!({"value": -0.01, "unit": "Percent"});
    let ratio =
        serde_json::from_value::<Ratio>(negative_amplitude_wire["amplitude"].clone()).unwrap();
    assert_eq!(ratio.get(), -0.01);
    assert_eq!(ratio.unit(), RatioUnit::Percent);
    let error = serde_json::from_value::<OptionQuote>(negative_amplitude_wire).unwrap_err();
    assert!(error.is_data());
    assert_eq!(
        error.to_string(),
        "invalid request: option amplitude must be non-negative"
    );
}
