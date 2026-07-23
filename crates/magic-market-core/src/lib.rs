#![forbid(unsafe_code)]
//! Provider-neutral market-data contracts.
mod batch;
mod capital;
mod company;
mod content;
mod enrichment;
mod error;
mod evidence;
mod instrument;
mod limit_pool;
mod options;
mod provenance;
mod provider;
mod research;
mod signals;
mod validated;
mod value;
pub use batch::{DataBatch, QualityReport};
pub use capital::{
    BlockTrade, BlockTrades, BoardFlow, BoardFlows, CapitalCapabilities, DividendPlan,
    DividendPlans, FlowInterval, FlowScope, FundFlowPoint, FundFlowRequest, FundFlowSeries,
    HolderCount, HolderCounts, InstrumentDateRangeRequest, LockupEvent, LockupEvents,
    MarginBalance, MarginData,
};
pub use company::{
    CompanyCapabilities, FinancialLine, FinancialStatement, FinancialStatements, ProfileFact,
    SecurityProfile, SecurityProfiles, StatementKind,
};
pub use content::{
    Announcement, Announcements, ContentCapabilities, InvestorQuestion, InvestorQuestions,
    NewsItem, NewsProvider,
};
pub use enrichment::{
    MarketStatistics, MarketStatisticsProvider, TechnicalBar, TechnicalBarsProvider,
};
pub use error::CoreError;
pub use evidence::SourceEvidence;
pub use instrument::{AssetClass, Exchange, InstrumentId};
pub use limit_pool::{
    LimitPoolCapabilities, LimitPoolEntry, LimitPoolKind, LimitPoolRequest, LimitPools,
};
pub use options::{
    OptionCapabilities, OptionContract, OptionData, OptionGreeks, OptionKind, OptionQuote,
};
pub use provenance::Provenance;
pub use provider::{
    Adjustment, AsyncHistoricalBars, AsyncMinuteData, AsyncRealtimeQuotes, AsyncTrades,
    AuctionSnapshot, Auctions, Bar, BarInterval, BarsRequest, Board, BookLevel, Capabilities,
    DataStatus, HistoricalBars, MinuteData, MinuteDataRequest, MinutePoint, MoneyFlow, MoneyFlows,
    OrderBook, OrderBooks, PriceLimitRule, ProviderId, Quote, RealtimeQuotes, SecurityMetadata,
    SecurityMetadataProvider, SourcedRecord, Trade, TradeSide, Trades, TradesRequest,
};
pub use research::{
    ConsensusData, ConsensusSnapshot, EarningsEstimate, ReportScope, ResearchCapabilities,
    ResearchReport, ResearchReports, ResearchRequest, SemanticChannel, SemanticSearch,
    SemanticSearchDocument, SemanticSearchRequest,
};
pub use signals::{
    BoardCategory, BoardMembership, BoardMembershipProvider, ConceptHit, ConceptHits,
    DragonTigerData, DragonTigerEntry, DragonTigerSeat, DragonTigerSide, InstrumentSignalRequest,
    MarketRankingEntry, MarketRankingKind, MarketRankings, PopularityData, PopularityRank,
    SignalCapabilities, StrongStockReason, StrongStockReasons,
};
pub use validated::{FiniteNumber, HttpsUrl, IsoDate, NonEmptyText, PositiveU32};
pub use value::{Money, Price, Quantity, Ratio, RatioUnit};
