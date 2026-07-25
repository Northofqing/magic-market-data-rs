#![forbid(unsafe_code)]
//! Provider-neutral, evidence-preserving market-data failover routing.

mod adapters;
mod error;
mod market_announcements;
mod router;
mod source;

pub use adapters::{
    announcement_source, auction_source, bars_source, block_trade_source, board_flow_source,
    board_membership_source, concept_hit_source, consensus_source, dividend_source,
    dragon_tiger_entry_source, dragon_tiger_seat_source, financial_statement_source,
    fund_flow_series_source, global_news_source, holder_count_source, instrument_news_source,
    investor_question_source, limit_pool_source, lockup_source, margin_source,
    market_dragon_tiger_source, market_ranking_source, market_statistics_source, minute_source,
    money_flow_source, northbound_daily_source, option_contract_source, option_greeks_source,
    option_quote_source, order_book_source, popularity_source, post_close_flow_source,
    quote_source, research_source, security_metadata_source, security_profile_source,
    semantic_search_source, strong_stock_reason_source, technical_bars_source, trades_source,
    AnnouncementRouter, AuctionRouter, BarsRouter, BlockTradeRouter, BoardFlowRequest,
    BoardFlowRouter, BoardMembershipRouter, ConceptHitRouter, ConsensusRouter, DividendRouter,
    DragonTigerEntryRouter, DragonTigerSeatRouter, FinancialStatementRequest,
    FinancialStatementRouter, FundFlowSeriesRouter, GlobalNewsRouter, HolderCountRouter,
    InstrumentNewsRouter, InvestorQuestionRouter, LimitPoolRouter, LockupRouter, MarginRouter,
    MarketDragonTigerRouter, MarketRankingRequest, MarketRankingRouter, MarketStatisticsRouter,
    MinuteRouter, MoneyFlowRouter, NorthboundDailyRouter, OptionContractRouter,
    OptionContractsRequest, OptionGreeksRouter, OptionQuoteRouter, OrderBookRouter,
    PopularityRouter, PostCloseFlowRouter, QuoteRouter, ResearchRouter, SecurityMetadataRouter,
    SecurityProfileRouter, SemanticSearchRouter, StrongStockReasonRouter, TechnicalBarsRouter,
    TradesRouter,
};
pub use error::{FailureAction, FailureKind, SourceError};
pub use market_announcements::{market_announcement_source, MarketAnnouncementRouter};
pub use router::{
    AcceptancePolicy, AttemptStatus, FailoverChain, RouteAttempt, RouteOutcome, RouterError,
};
pub use source::{RoutedSource, SourceFn};
