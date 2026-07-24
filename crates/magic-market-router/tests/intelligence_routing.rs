use magic_market_core::{
    AssetClass, ContractMonth, DataBatch, Exchange, FiniteNumber, InstrumentId, MarketStatistics,
    MarketStatisticsProvider, Money, NonEmptyText, OptionContract, OptionData, OptionGreeks,
    OptionKind, OptionQuote, PostCloseFlow, PostCloseFlowRequest, PostCloseFlows, Price,
    Provenance, ProviderId, Ratio, RatioUnit, SourceEvidence,
};
use magic_market_router::{
    market_statistics_source, option_contract_source, option_greeks_source, option_quote_source,
    post_close_flow_source, AcceptancePolicy, AttemptStatus, FailureKind, MarketStatisticsRouter,
    OptionContractRouter, OptionGreeksRouter, OptionQuoteRouter, PostCloseFlowRouter, RoutedSource,
    SourceError,
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

struct PostCloseFixtureProvider {
    record_provider: ProviderId,
    batch_provider_name: &'static str,
    response_date: Option<&'static str>,
    duplicate_rank: bool,
    seen_dates: Mutex<Vec<String>>,
    seen_limits: Mutex<Vec<u32>>,
}

impl PostCloseFlows for PostCloseFixtureProvider {
    type Error = FixtureError;

    fn post_close_flows(
        &self,
        request: &PostCloseFlowRequest,
    ) -> Result<DataBatch<PostCloseFlow>, Self::Error> {
        self.seen_dates
            .lock()
            .unwrap()
            .push(request.trading_date().as_str().to_owned());
        self.seen_limits.lock().unwrap().push(request.limit().get());
        let response_date = self
            .response_date
            .unwrap_or_else(|| request.trading_date().as_str());
        let batch_id = format!("post-close-{response_date}");
        let source_at = format!("{response_date} 15:35:00");
        let record = PostCloseFlow::new(
            instrument(),
            Some(NonEmptyText::new("华电辽能").unwrap()),
            magic_market_core::IsoDate::new(response_date).unwrap(),
            magic_market_core::PositiveU32::new(1).unwrap(),
            Price::new(16.41).unwrap(),
            Ratio::new(9.99, RatioUnit::Percent).unwrap(),
            Money::new(100_000_000.0).unwrap(),
            None,
            None,
            SourceEvidence::new(self.record_provider, "observed", &batch_id)
                .unwrap()
                .with_source_at(&source_at)
                .unwrap(),
        )
        .unwrap();
        let records = if self.duplicate_rank {
            vec![record.clone(), record]
        } else {
            vec![record]
        };
        Ok(DataBatch::strict(
            records,
            Provenance::new(self.batch_provider_name, "observed")
                .unwrap()
                .with_source_at(source_at)
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        ))
    }
}

#[test]
fn post_close_adapter_forwards_date_and_routes_only_matching_sourced_records() {
    let wrong = Arc::new(PostCloseFixtureProvider {
        record_provider: ProviderId::Eastmoney,
        batch_provider_name: "tencent",
        response_date: None,
        duplicate_rank: false,
        seen_dates: Mutex::new(Vec::new()),
        seen_limits: Mutex::new(Vec::new()),
    });
    let valid = Arc::new(PostCloseFixtureProvider {
        record_provider: ProviderId::Eastmoney,
        batch_provider_name: "eastmoney",
        response_date: None,
        duplicate_rank: false,
        seen_dates: Mutex::new(Vec::new()),
        seen_limits: Mutex::new(Vec::new()),
    });
    let request = PostCloseFlowRequest::new(
        magic_market_core::IsoDate::new("2026-07-23").unwrap(),
        magic_market_core::PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    let mut router = PostCloseFlowRouter::new(
        AcceptancePolicy::new()
            .with_require_complete(true)
            .with_require_source_at(true),
    );
    router
        .register(post_close_flow_source(
            ProviderId::Tencent,
            Arc::clone(&wrong),
            classify,
        ))
        .unwrap();
    router
        .register(post_close_flow_source(
            ProviderId::Eastmoney,
            Arc::clone(&valid),
            classify,
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert_eq!(outcome.batch().records().len(), 1);
    assert_eq!(
        outcome.batch().records()[0].evidence().source_at(),
        Some("2026-07-23 15:35:00")
    );
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert_eq!(wrong.seen_dates.lock().unwrap().as_slice(), ["2026-07-23"]);
    assert_eq!(valid.seen_dates.lock().unwrap().as_slice(), ["2026-07-23"]);
    assert_eq!(wrong.seen_limits.lock().unwrap().as_slice(), [10]);
    assert_eq!(valid.seen_limits.lock().unwrap().as_slice(), [10]);
}

#[test]
fn post_close_adapter_rejects_wrong_dates_and_duplicate_ranks() {
    let wrong_date = Arc::new(PostCloseFixtureProvider {
        record_provider: ProviderId::Tencent,
        batch_provider_name: "tencent",
        response_date: Some("2026-07-22"),
        duplicate_rank: false,
        seen_dates: Mutex::new(Vec::new()),
        seen_limits: Mutex::new(Vec::new()),
    });
    let duplicate_rank = Arc::new(PostCloseFixtureProvider {
        record_provider: ProviderId::Sina,
        batch_provider_name: "sina",
        response_date: None,
        duplicate_rank: true,
        seen_dates: Mutex::new(Vec::new()),
        seen_limits: Mutex::new(Vec::new()),
    });
    let valid = Arc::new(PostCloseFixtureProvider {
        record_provider: ProviderId::Eastmoney,
        batch_provider_name: "eastmoney",
        response_date: None,
        duplicate_rank: false,
        seen_dates: Mutex::new(Vec::new()),
        seen_limits: Mutex::new(Vec::new()),
    });
    let request = PostCloseFlowRequest::new(
        magic_market_core::IsoDate::new("2026-07-23").unwrap(),
        magic_market_core::PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    let mut router = PostCloseFlowRouter::new(AcceptancePolicy::new());
    router
        .register(post_close_flow_source(
            ProviderId::Tencent,
            wrong_date,
            classify,
        ))
        .unwrap();
    router
        .register(post_close_flow_source(
            ProviderId::Sina,
            duplicate_rank,
            classify,
        ))
        .unwrap();
    router
        .register(post_close_flow_source(
            ProviderId::Eastmoney,
            valid,
            classify,
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert_eq!(outcome.attempts().len(), 3);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Quality,
            ..
        }
    ));
}

#[derive(Clone, Copy)]
enum PostCloseFault {
    MissingBatchSourceTime,
    InvalidBatchSourceDateSuffix,
    ExceedsLimit,
    WrongRecordDate,
    DuplicateInstrument,
}

struct FaultyPostCloseProvider(PostCloseFault);

impl PostCloseFlows for FaultyPostCloseProvider {
    type Error = FixtureError;

    fn post_close_flows(
        &self,
        request: &PostCloseFlowRequest,
    ) -> Result<DataBatch<PostCloseFlow>, Self::Error> {
        let requested = request.trading_date().as_str();
        let record_date = if matches!(self.0, PostCloseFault::WrongRecordDate) {
            "2026-07-22"
        } else {
            requested
        };
        let source_at = format!("{requested} 15:35:00");
        let record_source_at = format!("{record_date} 15:35:00");
        let batch_id = format!("post-close-fault-{requested}");
        let make_record = |code: &str, rank: u32| {
            PostCloseFlow::new(
                InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap(),
                None,
                magic_market_core::IsoDate::new(record_date).unwrap(),
                magic_market_core::PositiveU32::new(rank).unwrap(),
                Price::new(16.41).unwrap(),
                Ratio::new(9.99, RatioUnit::Percent).unwrap(),
                Money::new(100_000_000.0).unwrap(),
                None,
                None,
                SourceEvidence::new(ProviderId::Eastmoney, "observed", &batch_id)
                    .unwrap()
                    .with_source_at(&record_source_at)
                    .unwrap(),
            )
            .unwrap()
        };
        let records = match self.0 {
            PostCloseFault::ExceedsLimit => {
                vec![make_record("600396", 1), make_record("600397", 2)]
            }
            PostCloseFault::DuplicateInstrument => {
                vec![make_record("600396", 1), make_record("600396", 2)]
            }
            _ => vec![make_record("600396", 1)],
        };
        let mut provenance = Provenance::new("eastmoney", "observed")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap();
        if !matches!(self.0, PostCloseFault::MissingBatchSourceTime) {
            provenance = provenance
                .with_source_at(
                    if matches!(self.0, PostCloseFault::InvalidBatchSourceDateSuffix) {
                        format!("{requested}x15:35:00")
                    } else {
                        source_at
                    },
                )
                .unwrap();
        }
        Ok(DataBatch::strict(records, provenance))
    }
}

#[test]
fn post_close_adapter_rejects_every_batch_and_record_contract_violation() {
    for (fault, limit, expected_kind) in [
        (
            PostCloseFault::MissingBatchSourceTime,
            10,
            FailureKind::Evidence,
        ),
        (
            PostCloseFault::InvalidBatchSourceDateSuffix,
            10,
            FailureKind::Evidence,
        ),
        (PostCloseFault::ExceedsLimit, 1, FailureKind::Quality),
        (PostCloseFault::WrongRecordDate, 10, FailureKind::Evidence),
        (
            PostCloseFault::DuplicateInstrument,
            10,
            FailureKind::Quality,
        ),
    ] {
        let request = PostCloseFlowRequest::new(
            magic_market_core::IsoDate::new("2026-07-23").unwrap(),
            magic_market_core::PositiveU32::new(limit).unwrap(),
        )
        .unwrap();
        let source = post_close_flow_source(
            ProviderId::Eastmoney,
            Arc::new(FaultyPostCloseProvider(fault)),
            classify,
        );
        let error = source.fetch(&request).unwrap_err();
        assert_eq!(error.kind(), expected_kind);
    }
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
