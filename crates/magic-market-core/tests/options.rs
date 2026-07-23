use magic_market_core::{
    AssetClass, Exchange, FiniteNumber, InstrumentId, IsoDate, NonEmptyText, OptionContract,
    OptionGreeks, OptionKind, OptionQuote, Price, ProviderId, Quantity, SourceEvidence,
    SourcedRecord,
};

fn underlying() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "510050", AssetClass::Fund).unwrap()
}

fn evidence() -> SourceEvidence {
    SourceEvidence::new(ProviderId::Sina, "observed", "option").unwrap()
}

fn contract() -> OptionContract {
    OptionContract {
        contract_code: NonEmptyText::new("10000001").unwrap(),
        underlying: underlying(),
        expiry: IsoDate::new("2026-08-26").unwrap(),
        kind: OptionKind::Call,
        strike: Price::new(3.0).unwrap(),
        evidence: evidence(),
    }
}

#[test]
fn option_contract_quote_and_greeks_are_distinct_records() {
    let quote = OptionQuote {
        contract_code: contract().contract_code.clone(),
        bid: Some(Price::new(0.1).unwrap()),
        ask: Some(Price::new(0.11).unwrap()),
        last: None,
        volume: Some(Quantity::new(0.0).unwrap()),
        open_interest: None,
        change: None,
        quote_at: None,
        evidence: evidence(),
    };
    let greeks = OptionGreeks {
        contract_code: contract().contract_code.clone(),
        delta: Some(FiniteNumber::new(0.5).unwrap()),
        gamma: Some(FiniteNumber::new(0.1).unwrap()),
        theta: Some(FiniteNumber::new(-0.01).unwrap()),
        vega: Some(FiniteNumber::new(0.2).unwrap()),
        rho: None,
        implied_volatility: Some(FiniteNumber::new(0.25).unwrap()),
        evidence: evidence(),
    };

    assert_eq!(quote.provider_id(), ProviderId::Sina);
    assert_eq!(greeks.delta.unwrap().get(), 0.5);
    assert_eq!(
        serde_json::from_str::<OptionGreeks>(&serde_json::to_string(&greeks).unwrap()).unwrap(),
        greeks
    );
}

#[test]
fn option_asset_identity_is_available_without_weakening_underlying() {
    let option_id = InstrumentId::new(Exchange::Shanghai, "10000001", AssetClass::Option).unwrap();
    assert_eq!(option_id.asset_class(), AssetClass::Option);
    assert_eq!(contract().underlying.asset_class(), AssetClass::Fund);
}
