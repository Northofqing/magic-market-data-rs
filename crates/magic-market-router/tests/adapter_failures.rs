use magic_market_core::*;
use magic_market_router::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("deterministic provider capacity failure")]
struct FixtureError;

#[derive(Default)]
struct FailingProvider {
    calls: AtomicUsize,
}

impl FailingProvider {
    fn fail<T>(&self) -> Result<DataBatch<T>, FixtureError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(FixtureError)
    }
}

macro_rules! impl_request_provider {
    ($provider_trait:ident, $method:ident, $request:ty, $record:ty) => {
        impl $provider_trait for FailingProvider {
            type Error = FixtureError;

            fn $method(&self, _request: &$request) -> Result<DataBatch<$record>, Self::Error> {
                self.fail()
            }
        }
    };
}

macro_rules! impl_instruments_provider {
    ($provider_trait:ident, $method:ident, $record:ty) => {
        impl $provider_trait for FailingProvider {
            type Error = FixtureError;

            fn $method(
                &self,
                _instruments: &[InstrumentId],
            ) -> Result<DataBatch<$record>, Self::Error> {
                self.fail()
            }
        }
    };
}

impl_request_provider!(
    TechnicalBarsProvider,
    technical_bars,
    BarsRequest,
    TechnicalBar
);
impl_request_provider!(
    ResearchReports,
    research_reports,
    ResearchRequest,
    ResearchReport
);
impl_request_provider!(
    SemanticSearch,
    semantic_search,
    SemanticSearchRequest,
    SemanticSearchDocument
);
impl_request_provider!(
    StrongStockReasons,
    strong_stock_reasons,
    InstrumentSignalRequest,
    StrongStockReason
);
impl_request_provider!(
    FundFlowSeries,
    fund_flow_series,
    FundFlowRequest,
    FundFlowPoint
);
impl_request_provider!(
    MarginData,
    margin_data,
    InstrumentDateRangeRequest,
    MarginBalance
);
impl_request_provider!(
    BlockTrades,
    block_trades,
    InstrumentDateRangeRequest,
    BlockTrade
);
impl_request_provider!(
    HolderCounts,
    holder_counts,
    InstrumentDateRangeRequest,
    HolderCount
);
impl_request_provider!(
    LockupEvents,
    lockup_events,
    InstrumentDateRangeRequest,
    LockupEvent
);
impl_request_provider!(
    DividendPlans,
    dividend_plans,
    InstrumentDateRangeRequest,
    DividendPlan
);
impl_request_provider!(
    Announcements,
    announcements,
    InstrumentDateRangeRequest,
    Announcement
);
impl_request_provider!(
    InvestorQuestions,
    investor_questions,
    InstrumentDateRangeRequest,
    InvestorQuestion
);
impl_request_provider!(LimitPools, limit_pool, LimitPoolRequest, LimitPoolEntry);

impl_instruments_provider!(BoardMembershipProvider, board_memberships, BoardMembership);
impl_instruments_provider!(ConsensusData, consensus, ConsensusSnapshot);
impl_instruments_provider!(ConceptHits, concept_hits, ConceptHit);
impl_instruments_provider!(SecurityProfiles, security_profiles, SecurityProfile);

impl DragonTigerData for FailingProvider {
    type Error = FixtureError;

    fn dragon_tiger_entries(
        &self,
        _request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerEntry>, Self::Error> {
        self.fail()
    }

    fn dragon_tiger_seats(
        &self,
        _request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerSeat>, Self::Error> {
        self.fail()
    }
}

impl MarketRankings for FailingProvider {
    type Error = FixtureError;

    fn market_rankings(
        &self,
        _kind: &MarketRankingKind,
        _limit: PositiveU32,
    ) -> Result<DataBatch<MarketRankingEntry>, Self::Error> {
        self.fail()
    }
}

impl PopularityData for FailingProvider {
    type Error = FixtureError;

    fn popularity(&self, _limit: PositiveU32) -> Result<DataBatch<PopularityRank>, Self::Error> {
        self.fail()
    }
}

impl BoardFlows for FailingProvider {
    type Error = FixtureError;

    fn board_flows(
        &self,
        _category: BoardCategory,
        _interval: FlowInterval,
        _limit: PositiveU32,
    ) -> Result<DataBatch<BoardFlow>, Self::Error> {
        self.fail()
    }
}

impl NewsProvider for FailingProvider {
    type Error = FixtureError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        self.fail()
    }

    fn global_news(&self, _limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        self.fail()
    }
}

impl FinancialStatements for FailingProvider {
    type Error = FixtureError;

    fn financial_statements(
        &self,
        _instruments: &[InstrumentId],
        _kind: StatementKind,
    ) -> Result<DataBatch<FinancialStatement>, Self::Error> {
        self.fail()
    }
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(
        FailureKind::RateLimited,
        "deterministic provider capacity failure",
    )
}

fn assert_retryable<Request: ?Sized, Record>(
    source: impl RoutedSource<Request, Record>,
    request: &Request,
) {
    let error = match source.fetch(request) {
        Ok(_) => panic!("fixture must fail"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), FailureKind::RateLimited);
    assert_eq!(error.action(), FailureAction::TryNext);
    assert_eq!(error.message(), "deterministic provider capacity failure");
}

#[test]
fn every_extended_adapter_maps_real_provider_failures_without_erasing_type() {
    let provider = Arc::new(FailingProvider::default());
    let instrument = instrument();
    let instruments = [instrument.clone()];
    let limit = PositiveU32::new(5).unwrap();
    let bars = BarsRequest::new(instrument.clone(), BarInterval::Day, 5).unwrap();
    let research = ResearchRequest::new(
        ReportScope::Instrument(instrument.clone()),
        PositiveU32::new(1).unwrap(),
        limit,
    )
    .unwrap();
    let semantic =
        SemanticSearchRequest::new("deterministic", SemanticChannel::General, limit).unwrap();
    let signal = InstrumentSignalRequest::new(instrument.clone(), limit).unwrap();
    let fund_flow = FundFlowRequest::new(
        FlowScope::Instrument(instrument.clone()),
        FlowInterval::Day1,
        limit,
    )
    .unwrap();
    let date_range = InstrumentDateRangeRequest::new(instrument.clone(), limit).unwrap();
    let board_flow = (BoardCategory::Industry, FlowInterval::Day1, limit);
    let ranking = (MarketRankingKind::Industry, limit);
    let financial = (vec![instrument], StatementKind::Balance);
    let limit_pool = LimitPoolRequest::new(
        LimitPoolKind::Upper,
        IsoDate::new("2026-07-23").unwrap(),
        limit,
    )
    .unwrap();

    assert_retryable(
        technical_bars_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &bars,
    );
    assert_retryable(
        research_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &research,
    );
    assert_retryable(
        consensus_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &instruments,
    );
    assert_retryable(
        semantic_search_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &semantic,
    );
    assert_retryable(
        board_membership_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &instruments,
    );
    assert_retryable(
        strong_stock_reason_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &signal,
    );
    assert_retryable(
        dragon_tiger_entry_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &signal,
    );
    assert_retryable(
        dragon_tiger_seat_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &signal,
    );
    assert_retryable(
        market_ranking_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &ranking,
    );
    assert_retryable(
        popularity_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &limit,
    );
    assert_retryable(
        concept_hit_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &instruments,
    );
    assert_retryable(
        fund_flow_series_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &fund_flow,
    );
    assert_retryable(
        board_flow_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &board_flow,
    );
    assert_retryable(
        margin_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &date_range,
    );
    assert_retryable(
        block_trade_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &date_range,
    );
    assert_retryable(
        holder_count_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &date_range,
    );
    assert_retryable(
        lockup_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &date_range,
    );
    assert_retryable(
        dividend_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &date_range,
    );
    assert_retryable(
        instrument_news_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &date_range,
    );
    assert_retryable(
        announcement_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &date_range,
    );
    assert_retryable(
        investor_question_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &date_range,
    );
    assert_retryable(
        global_news_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &limit,
    );
    assert_retryable(
        security_profile_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &instruments,
    );
    assert_retryable(
        financial_statement_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &financial,
    );
    assert_retryable(
        limit_pool_source(ProviderId::Custom, Arc::clone(&provider), classify),
        &limit_pool,
    );

    assert_eq!(provider.calls.load(Ordering::SeqCst), 25);
}
