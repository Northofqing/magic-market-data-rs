use magic_market_core::{
    AssetClass, ContractMonth, DataBatch, Exchange, FiniteNumber, InstrumentId, MarketStatistics,
    MarketStatisticsProvider, Money, NonEmptyText, OptionContract, OptionData, OptionGreeks,
    OptionKind, OptionQuote, Provenance, ProviderId, SourceEvidence,
};
use magic_market_router::{
    market_statistics_source, option_contract_source, option_greeks_source, option_quote_source,
    AcceptancePolicy, AttemptStatus, FailureKind, MarketStatisticsRouter, OptionContractRouter,
    OptionGreeksRouter, OptionQuoteRouter, SourceError,
};
use std::sync::{Arc, Mutex};

#[derive(Debug, thiserror::Error)]
#[error("fixture")]
struct FixtureError;

struct FixtureProvider {
    record_provider: ProviderId,
    record_batch: &'static str,
    batch_provider_name: &'static str,
    batch_id: &'static str,
}

impl MarketStatisticsProvider for FixtureProvider {
    type Error = FixtureError;

    fn market_statistics(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<MarketStatistics>, Self::Error> {
        let record = MarketStatistics::new(
            instruments[0].clone(),
            None,
            None,
            None,
            None,
            Some(Money::new(1.0).unwrap()),
            None,
            None,
            None,
            None,
            SourceEvidence::new(self.record_provider, "observed", self.record_batch).unwrap(),
        )
        .unwrap();
        Ok(DataBatch::strict(
            vec![record],
            Provenance::new(self.batch_provider_name, "observed")
                .unwrap()
                .with_batch_id(self.batch_id)
                .unwrap(),
        ))
    }
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Transport, "fixture")
}

#[test]
fn intelligence_adapters_reuse_evidence_preserving_failover() {
    let wrong = Arc::new(FixtureProvider {
        record_provider: ProviderId::Tencent,
        record_batch: "wrong",
        batch_provider_name: "eastmoney",
        batch_id: "wrong",
    });
    let valid = Arc::new(FixtureProvider {
        record_provider: ProviderId::Tencent,
        record_batch: "valid",
        batch_provider_name: "tencent",
        batch_id: "valid",
    });
    let mut router = MarketStatisticsRouter::new(AcceptancePolicy::new());
    router
        .register(market_statistics_source(
            ProviderId::Eastmoney,
            wrong,
            classify,
        ))
        .unwrap();
    router
        .register(market_statistics_source(
            ProviderId::Tencent,
            valid,
            classify,
        ))
        .unwrap();

    let outcome = router.route(&[instrument()]).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Tencent);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Selected
    ));
}

struct OptionFixtureProvider {
    record_provider: ProviderId,
    record_batch: &'static str,
    batch_provider_name: &'static str,
    batch_id: &'static str,
    seen_expiry: Mutex<Vec<Option<String>>>,
}

impl OptionFixtureProvider {
    fn new(
        record_provider: ProviderId,
        record_batch: &'static str,
        batch_provider_name: &'static str,
        batch_id: &'static str,
    ) -> Self {
        Self {
            record_provider,
            record_batch,
            batch_provider_name,
            batch_id,
            seen_expiry: Mutex::new(Vec::new()),
        }
    }

    fn evidence(&self) -> SourceEvidence {
        SourceEvidence::new(self.record_provider, "observed", self.record_batch).unwrap()
    }

    fn provenance(&self) -> Provenance {
        Provenance::new(self.batch_provider_name, "observed")
            .unwrap()
            .with_batch_id(self.batch_id)
            .unwrap()
    }
}

impl OptionData for OptionFixtureProvider {
    type Error = FixtureError;

    fn option_contracts(
        &self,
        underlying: &InstrumentId,
        expiry: Option<&ContractMonth>,
    ) -> Result<DataBatch<OptionContract>, Self::Error> {
        self.seen_expiry
            .lock()
            .unwrap()
            .push(expiry.map(|month| month.as_str().to_owned()));
        Ok(DataBatch::strict(
            vec![OptionContract {
                contract_code: NonEmptyText::new("10012127").unwrap(),
                underlying: underlying.clone(),
                expiry_month: expiry
                    .cloned()
                    .unwrap_or_else(|| ContractMonth::new("2026-08").unwrap()),
                expiry: None,
                kind: OptionKind::Call,
                strike: None,
                evidence: self.evidence(),
            }],
            self.provenance(),
        ))
    }

    fn option_quotes(
        &self,
        contracts: &[NonEmptyText],
    ) -> Result<DataBatch<OptionQuote>, Self::Error> {
        Ok(DataBatch::strict(
            vec![OptionQuote {
                contract_code: contracts[0].clone(),
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
                volume: None,
                open_interest: None,
                amount: None,
                change: None,
                amplitude: None,
                quote_at: None,
                evidence: self.evidence(),
            }],
            self.provenance(),
        ))
    }

    fn option_greeks(
        &self,
        contracts: &[NonEmptyText],
    ) -> Result<DataBatch<OptionGreeks>, Self::Error> {
        Ok(DataBatch::strict(
            vec![OptionGreeks {
                contract_code: contracts[0].clone(),
                name: None,
                volume: None,
                delta: Some(FiniteNumber::new(0.5).unwrap()),
                gamma: None,
                theta: None,
                vega: None,
                rho: None,
                implied_volatility: None,
                high: None,
                low: None,
                trade_code: None,
                strike: None,
                last: None,
                theoretical_price: None,
                evidence: self.evidence(),
            }],
            self.provenance(),
        ))
    }
}

fn option_underlying() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "510050", AssetClass::Fund).unwrap()
}

#[test]
fn option_contract_adapter_forwards_month_and_rejects_wrong_evidence() {
    let wrong = Arc::new(OptionFixtureProvider::new(
        ProviderId::Sina,
        "wrong",
        "tencent",
        "wrong",
    ));
    let valid = Arc::new(OptionFixtureProvider::new(
        ProviderId::Sina,
        "valid",
        "sina",
        "valid",
    ));
    let mut router = OptionContractRouter::new(AcceptancePolicy::new());
    router
        .register(option_contract_source(ProviderId::Tencent, wrong, classify))
        .unwrap();
    router
        .register(option_contract_source(
            ProviderId::Sina,
            valid.clone(),
            classify,
        ))
        .unwrap();

    let month = ContractMonth::new("2026-08").unwrap();
    let outcome = router
        .route(&(option_underlying(), Some(month.clone())))
        .unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Sina);
    assert_eq!(outcome.batch().records()[0].expiry_month, month);
    assert_eq!(
        valid.seen_expiry.lock().unwrap().as_slice(),
        &[Some("2026-08".to_owned())]
    );
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn option_quote_and_greek_adapters_preserve_valid_batches() {
    let provider = Arc::new(OptionFixtureProvider::new(
        ProviderId::Sina,
        "valid",
        "sina",
        "valid",
    ));
    let contracts = vec![NonEmptyText::new("10012127").unwrap()];

    let mut quotes = OptionQuoteRouter::new(AcceptancePolicy::new());
    quotes
        .register(option_quote_source(
            ProviderId::Sina,
            provider.clone(),
            classify,
        ))
        .unwrap();
    assert_eq!(
        quotes
            .route(contracts.as_slice())
            .unwrap()
            .batch()
            .records()[0]
            .contract_code,
        contracts[0]
    );

    let mut greeks = OptionGreeksRouter::new(AcceptancePolicy::new());
    greeks
        .register(option_greeks_source(ProviderId::Sina, provider, classify))
        .unwrap();
    assert_eq!(
        greeks
            .route(contracts.as_slice())
            .unwrap()
            .batch()
            .records()[0]
            .delta
            .unwrap()
            .get(),
        0.5
    );
}
