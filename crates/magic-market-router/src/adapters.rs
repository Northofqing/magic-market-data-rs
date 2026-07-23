use crate::{FailoverChain, SourceError, SourceFn};
use magic_market_core::{
    Announcement, Announcements, AuctionSnapshot, Auctions, Bar, BarsRequest, BlockTrade,
    BlockTrades, BoardCategory, BoardFlow, BoardFlows, BoardMembership, BoardMembershipProvider,
    ConceptHit, ConceptHits, ConsensusData, ConsensusSnapshot, DividendPlan, DividendPlans,
    DragonTigerData, DragonTigerEntry, DragonTigerSeat, FinancialStatement, FinancialStatements,
    FlowInterval, FundFlowPoint, FundFlowRequest, FundFlowSeries, HistoricalBars, HolderCount,
    HolderCounts, InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest,
    InvestorQuestion, InvestorQuestions, IsoDate, LimitPoolEntry, LimitPoolRequest, LimitPools,
    LockupEvent, LockupEvents, MarginBalance, MarginData, MarketRankingEntry, MarketRankingKind,
    MarketRankings, MarketStatistics, MarketStatisticsProvider, MinuteData, MinuteDataRequest,
    MinutePoint, MoneyFlow, MoneyFlows, NewsItem, NewsProvider, NonEmptyText, OptionContract,
    OptionData, OptionGreeks, OptionQuote, OrderBook, OrderBooks, PopularityData, PopularityRank,
    PositiveU32, ProviderId, Quote, RealtimeQuotes, ResearchReport, ResearchReports,
    ResearchRequest, SecurityMetadata, SecurityMetadataProvider, SecurityProfile, SecurityProfiles,
    SemanticSearch, SemanticSearchDocument, SemanticSearchRequest, StatementKind,
    StrongStockReason, StrongStockReasons, TechnicalBar, TechnicalBarsProvider, Trade, Trades,
    TradesRequest,
};
use std::sync::Arc;

pub type QuoteRouter = FailoverChain<[InstrumentId], Quote>;
pub type BarsRouter = FailoverChain<BarsRequest, Bar>;
pub type MinuteRouter = FailoverChain<MinuteDataRequest, MinutePoint>;
pub type TradesRouter = FailoverChain<TradesRequest, Trade>;
pub type MoneyFlowRouter = FailoverChain<[InstrumentId], MoneyFlow>;
pub type OrderBookRouter = FailoverChain<[InstrumentId], OrderBook>;
pub type AuctionRouter = FailoverChain<[InstrumentId], AuctionSnapshot>;
pub type SecurityMetadataRouter = FailoverChain<[InstrumentId], SecurityMetadata>;
pub type MarketStatisticsRouter = FailoverChain<[InstrumentId], MarketStatistics>;
pub type TechnicalBarsRouter = FailoverChain<BarsRequest, TechnicalBar>;
pub type ResearchRouter = FailoverChain<ResearchRequest, ResearchReport>;
pub type ConsensusRouter = FailoverChain<[InstrumentId], ConsensusSnapshot>;
pub type SemanticSearchRouter = FailoverChain<SemanticSearchRequest, SemanticSearchDocument>;
pub type BoardMembershipRouter = FailoverChain<[InstrumentId], BoardMembership>;
pub type StrongStockReasonRouter = FailoverChain<InstrumentSignalRequest, StrongStockReason>;
pub type DragonTigerEntryRouter = FailoverChain<InstrumentSignalRequest, DragonTigerEntry>;
pub type DragonTigerSeatRouter = FailoverChain<InstrumentSignalRequest, DragonTigerSeat>;
pub type MarketRankingRequest = (MarketRankingKind, PositiveU32);
pub type MarketRankingRouter = FailoverChain<MarketRankingRequest, MarketRankingEntry>;
pub type PopularityRouter = FailoverChain<PositiveU32, PopularityRank>;
pub type ConceptHitRouter = FailoverChain<[InstrumentId], ConceptHit>;
pub type FundFlowSeriesRouter = FailoverChain<FundFlowRequest, FundFlowPoint>;
pub type BoardFlowRequest = (BoardCategory, FlowInterval, PositiveU32);
pub type BoardFlowRouter = FailoverChain<BoardFlowRequest, BoardFlow>;
pub type MarginRouter = FailoverChain<InstrumentDateRangeRequest, MarginBalance>;
pub type BlockTradeRouter = FailoverChain<InstrumentDateRangeRequest, BlockTrade>;
pub type HolderCountRouter = FailoverChain<InstrumentDateRangeRequest, HolderCount>;
pub type LockupRouter = FailoverChain<InstrumentDateRangeRequest, LockupEvent>;
pub type DividendRouter = FailoverChain<InstrumentDateRangeRequest, DividendPlan>;
pub type InstrumentNewsRouter = FailoverChain<InstrumentDateRangeRequest, NewsItem>;
pub type GlobalNewsRouter = FailoverChain<PositiveU32, NewsItem>;
pub type AnnouncementRouter = FailoverChain<InstrumentDateRangeRequest, Announcement>;
pub type InvestorQuestionRouter = FailoverChain<InstrumentDateRangeRequest, InvestorQuestion>;
pub type SecurityProfileRouter = FailoverChain<[InstrumentId], SecurityProfile>;
pub type FinancialStatementRequest = (Vec<InstrumentId>, StatementKind);
pub type FinancialStatementRouter = FailoverChain<FinancialStatementRequest, FinancialStatement>;
pub type LimitPoolRouter = FailoverChain<LimitPoolRequest, LimitPoolEntry>;
pub type OptionContractsRequest = (InstrumentId, Option<IsoDate>);
pub type OptionContractRouter = FailoverChain<OptionContractsRequest, OptionContract>;
pub type OptionQuoteRouter = FailoverChain<[NonEmptyText], OptionQuote>;
pub type OptionGreeksRouter = FailoverChain<[NonEmptyText], OptionGreeks>;

pub fn quote_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], Quote>
where
    Provider: RealtimeQuotes<Quote = Quote> + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.realtime_quotes(request).map_err(&classify)
    })
}

pub fn bars_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<BarsRequest, Bar>
where
    Provider: HistoricalBars<Bar = Bar> + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.historical_bars(request).map_err(&classify)
    })
}

pub fn minute_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<MinuteDataRequest, MinutePoint>
where
    Provider: MinuteData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.minute_data(request).map_err(&classify)
    })
}

pub fn trades_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<TradesRequest, Trade>
where
    Provider: Trades + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.trades(request).map_err(&classify)
    })
}

pub fn money_flow_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], MoneyFlow>
where
    Provider: MoneyFlows + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.money_flows(request).map_err(&classify)
    })
}

pub fn order_book_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], OrderBook>
where
    Provider: OrderBooks + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.order_books(request).map_err(&classify)
    })
}

pub fn auction_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], AuctionSnapshot>
where
    Provider: Auctions + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.auction_snapshots(request).map_err(&classify)
    })
}

pub fn security_metadata_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], SecurityMetadata>
where
    Provider: SecurityMetadataProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.security_metadata(request).map_err(&classify)
    })
}

pub fn market_statistics_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], MarketStatistics>
where
    Provider: MarketStatisticsProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.market_statistics(request).map_err(&classify)
    })
}

pub fn technical_bars_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<BarsRequest, TechnicalBar>
where
    Provider: TechnicalBarsProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.technical_bars(request).map_err(&classify)
    })
}

pub fn research_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<ResearchRequest, ResearchReport>
where
    Provider: ResearchReports + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.research_reports(request).map_err(&classify)
    })
}

pub fn consensus_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], ConsensusSnapshot>
where
    Provider: ConsensusData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.consensus(request).map_err(&classify)
    })
}

pub fn semantic_search_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<SemanticSearchRequest, SemanticSearchDocument>
where
    Provider: SemanticSearch + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.semantic_search(request).map_err(&classify)
    })
}

pub fn board_membership_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], BoardMembership>
where
    Provider: BoardMembershipProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.board_memberships(request).map_err(&classify)
    })
}

pub fn strong_stock_reason_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<InstrumentSignalRequest, StrongStockReason>
where
    Provider: StrongStockReasons + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.strong_stock_reasons(request).map_err(&classify)
    })
}

pub fn dragon_tiger_entry_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<InstrumentSignalRequest, DragonTigerEntry>
where
    Provider: DragonTigerData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.dragon_tiger_entries(request).map_err(&classify)
    })
}

pub fn dragon_tiger_seat_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<InstrumentSignalRequest, DragonTigerSeat>
where
    Provider: DragonTigerData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.dragon_tiger_seats(request).map_err(&classify)
    })
}

pub fn market_ranking_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<MarketRankingRequest, MarketRankingEntry>
where
    Provider: MarketRankings + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &MarketRankingRequest| {
        provider
            .market_rankings(&request.0, request.1)
            .map_err(&classify)
    })
}

pub fn popularity_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<PositiveU32, PopularityRank>
where
    Provider: PopularityData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.popularity(*request).map_err(&classify)
    })
}

pub fn concept_hit_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], ConceptHit>
where
    Provider: ConceptHits + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.concept_hits(request).map_err(&classify)
    })
}

pub fn fund_flow_series_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<FundFlowRequest, FundFlowPoint>
where
    Provider: FundFlowSeries + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.fund_flow_series(request).map_err(&classify)
    })
}

pub fn board_flow_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<BoardFlowRequest, BoardFlow>
where
    Provider: BoardFlows + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &BoardFlowRequest| {
        provider
            .board_flows(request.0, request.1, request.2)
            .map_err(&classify)
    })
}

macro_rules! date_range_source {
    ($function:ident, $trait:ident, $method:ident, $record:ty) => {
        pub fn $function<Provider, Classify>(
            provider_id: ProviderId,
            provider: Arc<Provider>,
            classify: Classify,
        ) -> SourceFn<InstrumentDateRangeRequest, $record>
        where
            Provider: $trait + Send + Sync + 'static,
            Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
        {
            SourceFn::new(provider_id, move |request| {
                provider.$method(request).map_err(&classify)
            })
        }
    };
}

date_range_source!(margin_source, MarginData, margin_data, MarginBalance);
date_range_source!(block_trade_source, BlockTrades, block_trades, BlockTrade);
date_range_source!(
    holder_count_source,
    HolderCounts,
    holder_counts,
    HolderCount
);
date_range_source!(lockup_source, LockupEvents, lockup_events, LockupEvent);
date_range_source!(dividend_source, DividendPlans, dividend_plans, DividendPlan);
date_range_source!(
    instrument_news_source,
    NewsProvider,
    instrument_news,
    NewsItem
);
date_range_source!(
    announcement_source,
    Announcements,
    announcements,
    Announcement
);
date_range_source!(
    investor_question_source,
    InvestorQuestions,
    investor_questions,
    InvestorQuestion
);

pub fn global_news_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<PositiveU32, NewsItem>
where
    Provider: NewsProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.global_news(*request).map_err(&classify)
    })
}

pub fn security_profile_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], SecurityProfile>
where
    Provider: SecurityProfiles + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.security_profiles(request).map_err(&classify)
    })
}

pub fn financial_statement_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<FinancialStatementRequest, FinancialStatement>
where
    Provider: FinancialStatements + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &FinancialStatementRequest| {
        provider
            .financial_statements(&request.0, request.1)
            .map_err(&classify)
    })
}

pub fn limit_pool_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<LimitPoolRequest, LimitPoolEntry>
where
    Provider: LimitPools + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.limit_pool(request).map_err(&classify)
    })
}

pub fn option_contract_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<OptionContractsRequest, OptionContract>
where
    Provider: OptionData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &OptionContractsRequest| {
        provider
            .option_contracts(&request.0, request.1.as_ref())
            .map_err(&classify)
    })
}

pub fn option_quote_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[NonEmptyText], OptionQuote>
where
    Provider: OptionData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.option_quotes(request).map_err(&classify)
    })
}

pub fn option_greeks_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[NonEmptyText], OptionGreeks>
where
    Provider: OptionData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.option_greeks(request).map_err(&classify)
    })
}
