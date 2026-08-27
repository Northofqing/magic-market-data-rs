use std::{env, error::Error, sync::Arc, time::Duration};

use crate::derived_products::{
    IndexQuotesRequest, IntradayShapeRecord, IntradayShapeRequest, OutcomeDailyBarsRecord,
    OutcomeDailyBarsRequest, T0EvidenceRecord, T0EvidenceRequest, UpperLimitPoolReviewRecord,
    UpperLimitPoolReviewRequest,
};

use magic_baidu_rs::{BaiduClient, BaiduError};
use magic_cfets_rs::{CfetsClient, CfetsError};
use magic_cls_rs::{ClsClient, ClsError};
use magic_cninfo_rs::{CninfoClient, CninfoError};
use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError, EastmoneyMxClient};
use magic_emquant_rs::{
    EmQuantClient, EmQuantError, EMQUANT_DAILY_BARS_ADMITTED, MAX_EMQUANT_DAILY_BARS,
};
use magic_exchange_rs::{
    CffexClient, ExchangeError, HkexClient, SseClient, SseConfig, SzseClient, SzseConfig,
};
use magic_fred_rs::{FredClient, FredError};
use magic_gov_rs::{GovClient, GovError};
use magic_hithink_rs::{
    HithinkClient, HithinkError, AUCTIONS_ADMITTED as HITHINK_AUCTIONS_ADMITTED,
    CORPORATE_ACTIONS_ADMITTED as HITHINK_CORPORATE_ACTIONS_ADMITTED,
    FINANCIAL_STATEMENTS_ADMITTED as HITHINK_FINANCIAL_STATEMENTS_ADMITTED,
    HISTORICAL_BARS_ADMITTED as HITHINK_HISTORICAL_BARS_ADMITTED,
    LIMIT_POOLS_ADMITTED as HITHINK_LIMIT_POOLS_ADMITTED,
    MARKET_STATISTICS_ADMITTED as HITHINK_MARKET_STATISTICS_ADMITTED,
    POPULARITY_ADMITTED as HITHINK_POPULARITY_ADMITTED,
    SECURITY_METADATA_ADMITTED as HITHINK_SECURITY_METADATA_ADMITTED,
};
use magic_iwencai_rs::{IwencaiClient, IwencaiError, SEMANTIC_SEARCH_ADMITTED};
use magic_jin10_rs::{Jin10Client, Jin10Error, ECONOMIC_CALENDAR_ADMITTED};
use magic_market_core::{
    Announcements, Bar, BarInterval, BarsRequest, BlockTrades, BoardCategory,
    BoardConstituentProvider, BoardConstituentRequest, BoardDirectoryProvider,
    BoardDirectoryRequest, BoardFlows, BoardMembershipProvider, CompanyFilingRequest,
    CompanyFilingsProvider, ConceptHits, ConsensusData, ContractMonth, CorporateActionRequest,
    CorporateActions, DataBatch, DataStatus, DividendPlans, DragonTigerData, DragonTigerDiscovery,
    DragonTigerDiscoveryRequest, EconomicCalendarProvider, EconomicCalendarRequest,
    EconomicSeriesProvider, EconomicSeriesRequest, EvidenceTimestamp, FinancialStatements,
    FlowInterval, FlowScope, ForeignExchangeProvider, FundFlowPoint, FundFlowRequest,
    FundFlowSeries, FuturesDeliveryCalendar, FuturesDeliveryRequest, FxRequest,
    GlobalIndexProvider, GlobalIndexRequest, HistoricalBars, HolderCounts,
    InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest, InvestorQuestions, IsoDate,
    LimitPoolRequest, LimitPools, LockupEvents, MarginData, MarketAnnouncementRequest,
    MarketAnnouncements, MarketDragonTigerData, MarketDragonTigerRequest, MarketRankingKind,
    MarketStatisticsProvider, MinuteData, MinuteDataRequest, MinutePoint, MoneyFlow, MoneyFlows,
    NewsItem, NewsProvider, NonEmptyText, NorthboundDailyRequest, NorthboundDailyStatistics,
    OfficialFxFixingProvider, OfficialFxFixingRequest, OptionData, OrderBook, OrderBooks,
    PolicyDocuments, PolicyRequest, PopularityData, PositiveU32, PostCloseFlowRequest,
    PostCloseFlows, Provenance, ProviderId, ProviderTopNRankingRequest, ProviderTopNRankings,
    Quote, RealtimeQuotes, ReferenceRateProvider, ReferenceRateRequest, ResearchDocumentRequest,
    ResearchDocuments, ResearchReports, ResearchRequest, SecurityMetadataProvider,
    SecurityProfiles, SemanticSearch, SemanticSearchRequest, SourceEvidence, StatementKind,
    StrongStockReasons, TargetPriceData, TargetPriceRequest, TechnicalBarsProvider, Trades,
    TradesRequest,
};
use magic_market_router::{
    quote_source, AcceptancePolicy, AttemptStatus, FailureKind, QuoteRouter, RouterError,
    SourceError,
};
use magic_market_service::{
    CanonicalPayload, Capability, Operation, OperationRegistry, ProviderAttempt,
    ProviderFailureKind, QueryCommand, QueryResult, ServiceError,
};
use magic_nbs_rs::{NbsClient, NbsError};
use magic_pbc_rs::{PbcClient, PbcError};
use magic_sec_rs::{SecEdgarClient, SecEdgarError};
use magic_sina_rs::{SinaClient, SinaError};
use magic_stcn_rs::{StcnClient, StcnError, GLOBAL_NEWS_ADMITTED as STCN_GLOBAL_NEWS_ADMITTED};
use magic_tdx_rs::{
    BlockService, TdxBoardProvider, TdxError, TdxSecurityProfileProvider, TdxSmartClient,
};
use magic_tencent_rs::{TencentClient, TencentError};
use magic_thepaper_rs::{ThePaperClient, ThePaperError};
use magic_ths_rs::{ThsClient, ThsError};
use magic_wallstreetcn_rs::{WallstreetCnClient, WallstreetCnError};
use magic_worldbank_rs::{WorldBankClient, WorldBankError};
use magic_xinhua_rs::{XinhuaClient, XinhuaError};
use magic_yicai_rs::{YicaiClient, YicaiError};
use magic_yonhap_rs::{YonhapChannel, YonhapClient, YonhapError};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

pub const SCHEMA_VERSION: u32 = 1;
pub const NEWS_SCHEMA_VERSION: u32 = 2;
pub const T0_EVIDENCE_SCHEMA_VERSION: u32 = 2;
const EMQUANT_DAILY_BARS_SCOPE: &str = "Shanghai/Shenzhen equities; explicit inclusive start/end; unadjusted completed daily CSD OHLCV/amount; at most 800 returned rows";
const HITHINK_HISTORICAL_BARS_SCOPE: &str = "six-digit A-share equities and standard exchange indices with explicit inclusive range of at most ten years, plus Shanghai/Shenzhen ETFs at most five years; official Fuyao unadjusted completed Day bars; most recent caller limit after complete bounded response validation";
const HITHINK_MARKET_STATISTICS_SCOPE: &str = "1..=100 unique Shanghai/Shenzhen/Beijing equities; official Fuyao PE TTM, PE MRQ and PB MRQ with source nulls preserved";
const HITHINK_LIMIT_POOLS_SCOPE: &str = "official Fuyao Upper, Lower or Broken pool for one explicit Shanghai trading date; all declared pages validated before applying caller limit; PreviousUpper unsupported";
const HITHINK_POPULARITY_SCOPE: &str = "official Fuyao current 24-hour hot-stock ranking; at most 100 rows with exact identity, rank, heat and response source time";
const HITHINK_FINANCIAL_STATEMENTS_SCOPE: &str = "1..=8 unique A-share equities; most recent 20 quarterly consolidated income, balance or cash-flow statements; source nulls and per-report publication evidence preserved";
const HITHINK_CORPORATE_ACTIONS_SCOPE: &str = "one A-share equity; optional exact inclusive date range; official Fuyao implemented cash-dividend and bonus-share ex-date events; endpoint source-time absence preserved";
const HITHINK_SECURITY_METADATA_SCOPE: &str = "1..=32 unique A-share equities, standard exchange indices or exchange-traded funds; exact Fuyao thscode/name/currency identity with unpublished board, listing and price-limit fields explicitly unavailable";
const HITHINK_AUCTIONS_SCOPE: &str = "1..=100 unique A-share equities; current official Fuyao stage=final closed auction snapshot diagnostic; provider response assembly time is observed_at while trading date, source_at and directional unmatched queues remain absent";
const HITHINK_AUCTIONS_BLOCKER: &str = "Fuyao current auction snapshots omit the exact trading date, provider source time and directional unmatched bid/ask quantities; separate benchmark and calendar dates are not bound to snapshot records";
pub const REALTIME_QUOTES_REQUEST_SCHEMA: &str = "magic.market.realtime_quotes.request";
pub const REALTIME_QUOTES_RECORD_SCHEMA: &str = "magic.market.quote";
pub const HISTORICAL_BARS_REQUEST_SCHEMA: &str = "magic.market.historical_bars.request";
pub const HISTORICAL_BARS_RECORD_SCHEMA: &str = "magic.market.bar";
pub const MINUTE_DATA_REQUEST_SCHEMA: &str = "magic.market.minute_data.request";
pub const MINUTE_DATA_RECORD_SCHEMA: &str = "magic.market.minute_point";
pub const ORDER_BOOKS_REQUEST_SCHEMA: &str = "magic.market.order_books.request";
pub const ORDER_BOOKS_RECORD_SCHEMA: &str = "magic.market.order_book";
pub const TRADES_REQUEST_SCHEMA: &str = "magic.market.trades.request";
pub const TRADES_RECORD_SCHEMA: &str = "magic.market.trade";
pub const SECURITY_METADATA_REQUEST_SCHEMA: &str = "magic.market.security_metadata.request";
pub const SECURITY_METADATA_RECORD_SCHEMA: &str = "magic.market.security_metadata";
pub const MARKET_STATISTICS_REQUEST_SCHEMA: &str = "magic.market.market_statistics.request";
pub const MARKET_STATISTICS_RECORD_SCHEMA: &str = "magic.market.market_statistics";
pub const GLOBAL_INDICES_REQUEST_SCHEMA: &str = "magic.market.global_indices.request";
pub const GLOBAL_INDICES_RECORD_SCHEMA: &str = "magic.market.global_index_quote";
pub const FOREIGN_EXCHANGE_REQUEST_SCHEMA: &str = "magic.market.foreign_exchange.request";
pub const FOREIGN_EXCHANGE_RECORD_SCHEMA: &str = "magic.market.fx_quote";
pub const REFERENCE_RATES_REQUEST_SCHEMA: &str = "magic.market.reference_rates.request";
pub const REFERENCE_RATES_RECORD_SCHEMA: &str = "magic.market.reference_rate";
pub const OFFICIAL_FX_FIXINGS_REQUEST_SCHEMA: &str = "magic.market.official_fx_fixings.request";
pub const OFFICIAL_FX_FIXINGS_RECORD_SCHEMA: &str = "magic.market.official_fx_fixing";
pub const ECONOMIC_SERIES_REQUEST_SCHEMA: &str = "magic.market.economic_series.request";
pub const ECONOMIC_SERIES_RECORD_SCHEMA: &str = "magic.market.economic_observation";
pub const COMPANY_FILINGS_REQUEST_SCHEMA: &str = "magic.market.company_filings.request";
pub const COMPANY_FILINGS_RECORD_SCHEMA: &str = "magic.market.company_filing";
pub const GLOBAL_NEWS_REQUEST_SCHEMA: &str = "magic.market.global_news.request";
pub const GLOBAL_NEWS_RECORD_SCHEMA: &str = "magic.market.news_item";
pub const INSTRUMENT_NEWS_REQUEST_SCHEMA: &str = "magic.market.instrument_news.request";
pub const ANNOUNCEMENTS_REQUEST_SCHEMA: &str = "magic.market.announcements.request";
pub const ANNOUNCEMENTS_RECORD_SCHEMA: &str = "magic.market.announcement";
pub const MARKET_ANNOUNCEMENTS_REQUEST_SCHEMA: &str = "magic.market.market_announcements.request";
pub const INVESTOR_QUESTIONS_REQUEST_SCHEMA: &str = "magic.market.investor_questions.request";
pub const INVESTOR_QUESTIONS_RECORD_SCHEMA: &str = "magic.market.investor_question";
pub const POLICY_DOCUMENTS_REQUEST_SCHEMA: &str = "magic.market.policy_documents.request";
pub const POLICY_DOCUMENTS_RECORD_SCHEMA: &str = "magic.market.policy_document";
pub const SECURITY_PROFILES_REQUEST_SCHEMA: &str = "magic.market.security_profiles.request";
pub const SECURITY_PROFILES_RECORD_SCHEMA: &str = "magic.market.security_profile";
pub const FINANCIAL_STATEMENTS_REQUEST_SCHEMA: &str = "magic.market.financial_statements.request";
pub const FINANCIAL_STATEMENTS_RECORD_SCHEMA: &str = "magic.market.financial_statement";
pub const RESEARCH_REPORTS_REQUEST_SCHEMA: &str = "magic.market.research_reports.request";
pub const RESEARCH_REPORTS_RECORD_SCHEMA: &str = "magic.market.research_report";
pub const RESEARCH_DOCUMENTS_REQUEST_SCHEMA: &str = "magic.market.research_documents.request";
pub const RESEARCH_DOCUMENTS_RECORD_SCHEMA: &str = "magic.market.research_document";
pub const TARGET_PRICES_REQUEST_SCHEMA: &str = "magic.market.target_prices.request";
pub const TARGET_PRICES_RECORD_SCHEMA: &str = "magic.market.target_price_consensus";
pub const BOARD_FLOWS_REQUEST_SCHEMA: &str = "magic.market.board_flows.request";
pub const BOARD_FLOWS_RECORD_SCHEMA: &str = "magic.market.board_flow";
pub const MARGIN_DATA_REQUEST_SCHEMA: &str = "magic.market.margin_data.request";
pub const MARGIN_DATA_RECORD_SCHEMA: &str = "magic.market.margin_balance";
pub const BLOCK_TRADES_REQUEST_SCHEMA: &str = "magic.market.block_trades.request";
pub const BLOCK_TRADES_RECORD_SCHEMA: &str = "magic.market.block_trade";
pub const HOLDER_COUNTS_REQUEST_SCHEMA: &str = "magic.market.holder_counts.request";
pub const HOLDER_COUNTS_RECORD_SCHEMA: &str = "magic.market.holder_count";
pub const LOCKUP_EVENTS_REQUEST_SCHEMA: &str = "magic.market.lockup_events.request";
pub const LOCKUP_EVENTS_RECORD_SCHEMA: &str = "magic.market.lockup_event";
pub const DIVIDEND_PLANS_REQUEST_SCHEMA: &str = "magic.market.dividend_plans.request";
pub const DIVIDEND_PLANS_RECORD_SCHEMA: &str = "magic.market.dividend_plan";
pub const LIMIT_POOLS_REQUEST_SCHEMA: &str = "magic.market.limit_pools.request";
pub const LIMIT_POOLS_RECORD_SCHEMA: &str = "magic.market.limit_pool_entry";
pub const DRAGON_TIGER_REQUEST_SCHEMA: &str = "magic.market.dragon_tiger.request";
pub const DRAGON_TIGER_ENTRY_RECORD_SCHEMA: &str = "magic.market.dragon_tiger_entry";
pub const DRAGON_TIGER_SEAT_RECORD_SCHEMA: &str = "magic.market.dragon_tiger_seat";
pub const MARKET_DRAGON_TIGER_REQUEST_SCHEMA: &str = "magic.market.market_dragon_tiger.request";
pub const MARKET_DRAGON_TIGER_RECORD_SCHEMA: &str = "magic.market.dragon_tiger_disclosure";
pub const DRAGON_TIGER_DISCOVERY_REQUEST_SCHEMA: &str =
    "magic.market.dragon_tiger_discovery.request";
pub const POPULARITY_REQUEST_SCHEMA: &str = "magic.market.popularity.request";
pub const POPULARITY_RECORD_SCHEMA: &str = "magic.market.popularity_rank";
pub const PROVIDER_TOP_N_REQUEST_SCHEMA: &str = "magic.market.provider_top_n_rankings.request";
pub const PROVIDER_TOP_N_RECORD_SCHEMA: &str = "magic.market.provider_top_n_ranking";
pub const ECONOMIC_CALENDAR_REQUEST_SCHEMA: &str = "magic.market.economic_calendar.request";
pub const ECONOMIC_CALENDAR_RECORD_SCHEMA: &str = "magic.market.economic_event";
pub const NORTHBOUND_DAILY_REQUEST_SCHEMA: &str = "magic.market.northbound_daily.request";
pub const NORTHBOUND_DAILY_RECORD_SCHEMA: &str = "magic.market.northbound_daily_stat";
pub const CONSENSUS_REQUEST_SCHEMA: &str = "magic.market.consensus.request";
pub const CONSENSUS_RECORD_SCHEMA: &str = "magic.market.consensus_snapshot";
pub const STRONG_STOCK_REASONS_REQUEST_SCHEMA: &str = "magic.market.strong_stock_reasons.request";
pub const STRONG_STOCK_REASONS_RECORD_SCHEMA: &str = "magic.market.strong_stock_reason";
pub const SEMANTIC_SEARCH_REQUEST_SCHEMA: &str = "magic.market.semantic_search.request";
pub const SEMANTIC_SEARCH_RECORD_SCHEMA: &str = "magic.market.semantic_search_document";
pub const OPTION_DATA_REQUEST_SCHEMA: &str = "magic.market.option_data.request";
pub const OPTION_CONTRACT_RECORD_SCHEMA: &str = "magic.market.option_contract";
pub const OPTION_QUOTE_RECORD_SCHEMA: &str = "magic.market.option_quote";
pub const OPTION_GREEKS_RECORD_SCHEMA: &str = "magic.market.option_greeks";
pub const CORPORATE_ACTIONS_REQUEST_SCHEMA: &str = "magic.market.corporate_actions.request";
pub const CORPORATE_ACTIONS_RECORD_SCHEMA: &str = "magic.market.corporate_action";
pub const BOARD_DIRECTORY_REQUEST_SCHEMA: &str = "magic.market.board_directory.request";
pub const BOARD_DIRECTORY_RECORD_SCHEMA: &str = "magic.market.board_definition";
pub const BOARD_CONSTITUENTS_REQUEST_SCHEMA: &str = "magic.market.board_constituents.request";
pub const BOARD_MEMBERSHIPS_REQUEST_SCHEMA: &str = "magic.market.board_memberships.request";
pub const BOARD_MEMBERSHIP_RECORD_SCHEMA: &str = "magic.market.board_membership";
pub const CONCEPT_HITS_REQUEST_SCHEMA: &str = "magic.market.concept_hits.request";
pub const CONCEPT_HITS_RECORD_SCHEMA: &str = "magic.market.concept_hit";
pub const MONEY_FLOWS_REQUEST_SCHEMA: &str = "magic.market.money_flows.request";
pub const MONEY_FLOWS_RECORD_SCHEMA: &str = "magic.market.money_flow";
pub const FUTURES_DELIVERY_REQUEST_SCHEMA: &str = "magic.market.futures_delivery.request";
pub const FUTURES_DELIVERY_RECORD_SCHEMA: &str = "magic.market.futures_delivery_event";
pub const INDEX_QUOTES_REQUEST_SCHEMA: &str = "magic.market.index_quotes.request";
pub const INDEX_QUOTES_RECORD_SCHEMA: &str = REALTIME_QUOTES_RECORD_SCHEMA;
pub const INTRADAY_SHAPE_REQUEST_SCHEMA: &str = "magic.market.intraday_shape.request";
pub const INTRADAY_SHAPE_RECORD_SCHEMA: &str = "magic.market.intraday_shape";
pub const T0_EVIDENCE_REQUEST_SCHEMA: &str = "magic.market.t0_evidence.request";
pub const T0_EVIDENCE_RECORD_SCHEMA: &str = "magic.market.t0_evidence";
pub const OUTCOME_DAILY_BARS_REQUEST_SCHEMA: &str = "magic.market.outcome_daily_bars.request";
pub const OUTCOME_DAILY_BARS_RECORD_SCHEMA: &str = "magic.market.outcome_daily_bars";
pub const UPPER_LIMIT_POOL_REVIEW_REQUEST_SCHEMA: &str =
    "magic.market.upper_limit_pool_review.request";
pub const UPPER_LIMIT_POOL_REVIEW_RECORD_SCHEMA: &str = "magic.market.upper_limit_pool_review";
pub const TECHNICAL_BARS_REQUEST_SCHEMA: &str = "magic.market.technical_bars.request";
pub const TECHNICAL_BARS_RECORD_SCHEMA: &str = "magic.market.technical_bar";
pub const FUND_FLOW_SERIES_REQUEST_SCHEMA: &str = "magic.market.fund_flow_series.request";
pub const FUND_FLOW_SERIES_RECORD_SCHEMA: &str = "magic.market.fund_flow_point";
pub const POST_CLOSE_FLOWS_REQUEST_SCHEMA: &str = "magic.market.post_close_flows.request";
pub const POST_CLOSE_FLOWS_RECORD_SCHEMA: &str = "magic.market.post_close_flow";
pub const MARKET_RANKINGS_REQUEST_SCHEMA: &str = "magic.market.market_rankings.request";
pub const MARKET_RANKINGS_RECORD_SCHEMA: &str = "magic.market.market_ranking_diagnostic_entry";
pub const AUCTIONS_REQUEST_SCHEMA: &str = "magic.market.auctions.request";
pub const HITHINK_CURRENT_AUCTIONS_REQUEST_SCHEMA: &str =
    "magic.market.hithink_current_auctions.request";
pub const AUCTIONS_RECORD_SCHEMA: &str = "magic.market.opening_auction_diagnostic";
pub const HITHINK_CURRENT_AUCTIONS_RECORD_SCHEMA: &str =
    "magic.market.hithink_current_auction_snapshot";
pub const MARKET_BREADTH_REQUEST_SCHEMA: &str = "magic.market.market_breadth.request";
pub const MARKET_BREADTH_RECORD_SCHEMA: &str = "magic.market.market_breadth_diagnostic";
const TENCENT_PROVIDER: &str = "Tencent";
const TENCENT_QUOTE_SCOPE: &str =
    "1..=50 unique Shanghai/Shenzhen/Beijing six-digit A-share equities";
const TENCENT_BARS_SCOPE: &str =
    "one Shanghai/Shenzhen/Beijing six-digit A-share equity; latest bounded bars without a date range";
const TENCENT_MINUTE_SCOPE: &str =
    "one Shanghai/Shenzhen/Beijing six-digit A-share equity; current or exact source date";
const TENCENT_TRADES_SCOPE: &str =
    "one Shanghai/Shenzhen/Beijing six-digit A-share equity; bounded current-session trades without a date selector";
const TENCENT_STATISTICS_SCOPE: &str =
    "1..=50 unique Shanghai/Shenzhen equities/funds/indices or Beijing equities";

#[derive(Debug, Error)]
pub enum ProductionRegistryError {
    #[error("Baidu production client initialization failed: {0}")]
    Baidu(#[from] BaiduError),
    #[error("invalid production registry limit: {0}")]
    InvalidLimit(&'static str),
    #[error("Tencent production client initialization failed: {0}")]
    Tencent(#[from] TencentError),
    #[error("Sina production client initialization failed: {0}")]
    Sina(#[from] SinaError),
    #[error("CFETS production client initialization failed: {0}")]
    Cfets(#[from] CfetsError),
    #[error("CNInfo production client initialization failed: {0}")]
    Cninfo(#[from] CninfoError),
    #[error("State Council production client initialization failed: {0}")]
    Gov(#[from] GovError),
    #[error("WallstreetCN production client initialization failed: {0}")]
    WallstreetCn(#[from] WallstreetCnError),
    #[error("CLS production client initialization failed: {0}")]
    Cls(#[from] ClsError),
    #[error("ThePaper production client initialization failed: {0}")]
    ThePaper(#[from] ThePaperError),
    #[error("Xinhua production client initialization failed: {0}")]
    Xinhua(#[from] XinhuaError),
    #[error("Yicai production client initialization failed: {0}")]
    Yicai(#[from] YicaiError),
    #[error("Securities Times production client initialization failed: {0}")]
    Stcn(#[from] StcnError),
    #[error("Yonhap production client initialization failed: {0}")]
    Yonhap(#[from] YonhapError),
    #[error("Eastmoney production client initialization failed: {0}")]
    Eastmoney(#[from] EastmoneyError),
    #[error("FRED production client initialization failed: {0}")]
    Fred(#[from] FredError),
    #[error("SEC EDGAR production client initialization failed: {0}")]
    Sec(#[from] SecEdgarError),
    #[error("Jin10 production client initialization failed: {0}")]
    Jin10(#[from] Jin10Error),
    #[error("HKEX production client initialization failed: {0}")]
    Exchange(#[from] ExchangeError),
    #[error("Tonghuashun production client initialization failed: {0}")]
    Ths(#[from] ThsError),
    #[error("iWencai production client initialization failed: {0}")]
    Iwencai(#[from] IwencaiError),
    #[error("TDX production client initialization failed: {0}")]
    Tdx(#[from] TdxError),
    #[error("NBS production client initialization failed: {0}")]
    Nbs(#[from] NbsError),
    #[error("PBC production client initialization failed: {0}")]
    Pbc(#[from] PbcError),
    #[error("World Bank production client initialization failed: {0}")]
    WorldBank(#[from] WorldBankError),
    #[error("production operation registration failed: {0}")]
    Service(#[from] ServiceError),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RealtimeQuotesRequest {
    instruments: Vec<InstrumentId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstrumentsRequest {
    instruments: Vec<InstrumentId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitRequest {
    limit: PositiveU32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstrumentNewsRequestV2 {
    instrument: InstrumentId,
    start: Option<IsoDate>,
    end: Option<IsoDate>,
    limit: PositiveU32,
    captured_through: String,
}

#[derive(Serialize)]
struct NewsRecordPayloadV2<'a> {
    item_id: &'a str,
    title: &'a str,
    summary: Option<&'a str>,
    content: Option<&'a str>,
    publisher: &'a str,
    url: &'a str,
    published_at: &'a str,
    instruments: &'a [InstrumentId],
    topics: Vec<&'a str>,
    language: &'a str,
    evidence: &'a SourceEvidence,
}

impl<'a> From<&'a NewsItem> for NewsRecordPayloadV2<'a> {
    fn from(item: &'a NewsItem) -> Self {
        Self {
            item_id: item.item_id.as_str(),
            title: item.title.as_str(),
            summary: item.summary.as_ref().map(NonEmptyText::as_str),
            content: item.content.as_ref().map(NonEmptyText::as_str),
            publisher: item.publisher.as_str(),
            url: item.canonical_url.as_str(),
            published_at: item.published_at.as_str(),
            instruments: &item.instruments,
            topics: item.topics.iter().map(NonEmptyText::as_str).collect(),
            language: item.language.as_str(),
            evidence: &item.evidence,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FinancialStatementsRequest {
    instruments: Vec<InstrumentId>,
    kind: StatementKind,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoardFlowsRequest {
    category: BoardCategory,
    interval: FlowInterval,
    limit: PositiveU32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketRankingsRequest {
    kind: MarketRankingKind,
    limit: PositiveU32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuctionDiagnosticRequest {
    instrument: InstrumentId,
    trading_date: IsoDate,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketBreadthDiagnosticRequest {
    source_date: IsoDate,
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "request", rename_all = "snake_case")]
enum DragonTigerRequest {
    Entries(InstrumentSignalRequest),
    Seats(InstrumentSignalRequest),
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OptionDataRequest {
    Contracts {
        underlying: InstrumentId,
        expiry: Option<ContractMonth>,
    },
    Quotes {
        contracts: Vec<NonEmptyText>,
    },
    Greeks {
        contracts: Vec<NonEmptyText>,
    },
}

pub fn production_operation_registry(
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<OperationRegistry, ProductionRegistryError> {
    if provider_timeout.is_zero() {
        return Err(ProductionRegistryError::InvalidLimit(
            "provider timeout must be positive",
        ));
    }
    if maximum_payload_bytes == 0 {
        return Err(ProductionRegistryError::InvalidLimit(
            "maximum payload bytes must be positive",
        ));
    }
    let client = TencentClient::with_timeout(provider_timeout)?;
    registry_with_tencent(client, provider_timeout, maximum_payload_bytes)
}

fn registry_with_tencent(
    client: TencentClient,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<OperationRegistry, ProductionRegistryError> {
    let mut registry = OperationRegistry::all_unadmitted(
        "no evidence-backed production handler is registered for this operation",
    );
    let quotes = client.clone();
    registry.register_handler(
        Capability {
            operation: Operation::RealtimeQuotes,
            repository_admitted: true,
            runtime_available: true,
            provider: TENCENT_PROVIDER.to_owned(),
            exact_scope: TENCENT_QUOTE_SCOPE.to_owned(),
            blocker: None,
            diagnostic_available: false,
        },
        move |command| execute_tencent_quotes(&quotes, command, maximum_payload_bytes),
    )?;
    let index_quotes = client.clone();
    registry.register_handler(
        admitted(
            Operation::IndexQuotes,
            TENCENT_PROVIDER,
            "one through six unique Shanghai or Shenzhen index identities with caller-selected positive source freshness",
        ),
        move |command| {
            execute_tencent_index_quotes(&index_quotes, command, maximum_payload_bytes)
        },
    )?;
    let intraday_shape = client.clone();
    registry.register_handler(
        admitted(
            Operation::IntradayShape,
            "LocalAnalysis",
            "deterministic regular-session shape from one complete Tencent minute series",
        ),
        move |command| {
            execute_tencent_intraday_shape(&intraday_shape, command, maximum_payload_bytes)
        },
    )?;
    let bars = client.clone();
    registry.register_handler(
        capability(Operation::HistoricalBars, TENCENT_BARS_SCOPE),
        move |command| execute_tencent_bars(&bars, command, maximum_payload_bytes),
    )?;
    let minute = client.clone();
    registry.register_handler(
        capability(Operation::MinuteData, TENCENT_MINUTE_SCOPE),
        move |command| execute_tencent_minute(&minute, command, maximum_payload_bytes),
    )?;
    let order_books = client.clone();
    registry.register_handler(
        capability(Operation::OrderBooks, TENCENT_QUOTE_SCOPE),
        move |command| execute_tencent_order_books(&order_books, command, maximum_payload_bytes),
    )?;
    let trades = client.clone();
    registry.register_handler(
        capability(Operation::Trades, TENCENT_TRADES_SCOPE),
        move |command| execute_tencent_trades(&trades, command, maximum_payload_bytes),
    )?;
    let metadata = client.clone();
    registry.register_handler(
        capability(
            Operation::SecurityMetadata,
            "1..=50 unique Shanghai/Shenzhen/Beijing equities; source name/ST flag and explicitly derived board, with unproved listing date and price-limit fields retained as unavailable",
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, SECURITY_METADATA_REQUEST_SCHEMA)?;
            let batch = metadata
                .security_metadata(&request.instruments)
                .map_err(|error| map_tencent_error(Operation::SecurityMetadata, error))?;
            provider_query_result(
                batch,
                TENCENT_PROVIDER,
                SECURITY_METADATA_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;
    registry.register_handler(
        capability(Operation::MarketStatistics, TENCENT_STATISTICS_SCOPE),
        move |command| execute_tencent_statistics(&client, command, maximum_payload_bytes),
    )?;
    register_extended_providers(&mut registry, provider_timeout, maximum_payload_bytes)?;
    Ok(registry)
}

fn register_extended_providers(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let sina = SinaClient::with_timeout(provider_timeout)?;
    let global = sina.clone();
    registry.register_handler(
        admitted(
            Operation::GlobalIndices,
            "Sina",
            "verified global index identities",
        ),
        move |command| {
            execute_typed(
                command,
                GLOBAL_INDICES_REQUEST_SCHEMA,
                GLOBAL_INDICES_RECORD_SCHEMA,
                "Sina",
                maximum_payload_bytes,
                |request: &GlobalIndexRequest| global.global_indices(request),
            )
        },
    )?;
    let fx = sina.clone();
    registry.register_handler(
        admitted(
            Operation::ForeignExchange,
            "Sina",
            "verified currency-pair identities",
        ),
        move |command| {
            execute_typed(
                command,
                FOREIGN_EXCHANGE_REQUEST_SCHEMA,
                FOREIGN_EXCHANGE_RECORD_SCHEMA,
                "Sina",
                maximum_payload_bytes,
                |request: &FxRequest| fx.foreign_exchange(request),
            )
        },
    )?;
    let financials = sina.clone();
    registry.register_handler(
        admitted(
            Operation::FinancialStatements,
            "Sina",
            "bounded Shanghai/Shenzhen equity income, balance-sheet or cash-flow statements",
        ),
        move |command| {
            let request: FinancialStatementsRequest =
                decode_request(&command, FINANCIAL_STATEMENTS_REQUEST_SCHEMA)?;
            let batch = financials
                .financial_statements(&request.instruments, request.kind)
                .map_err(|error| provider_error(Operation::FinancialStatements, error))?;
            provider_query_result(
                batch,
                "Sina",
                FINANCIAL_STATEMENTS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let cfets = CfetsClient::new(provider_timeout)?;
    let rates = cfets.clone();
    registry.register_handler(
        admitted(
            Operation::ReferenceRates,
            "Cfets",
            "admitted SHIBOR and LPR families",
        ),
        move |command| {
            execute_typed(
                command,
                REFERENCE_RATES_REQUEST_SCHEMA,
                REFERENCE_RATES_RECORD_SCHEMA,
                "Cfets",
                maximum_payload_bytes,
                |request: &ReferenceRateRequest| rates.reference_rates(request),
            )
        },
    )?;
    registry.register_handler(
        admitted(
            Operation::OfficialFxFixings,
            "Cfets",
            "official central parity fixings admitted by exact currency identity",
        ),
        move |command| {
            execute_typed(
                command,
                OFFICIAL_FX_FIXINGS_REQUEST_SCHEMA,
                OFFICIAL_FX_FIXINGS_RECORD_SCHEMA,
                "Cfets",
                maximum_payload_bytes,
                |request: &OfficialFxFixingRequest| cfets.official_fx_fixings(request),
            )
        },
    )?;

    register_fred(registry, maximum_payload_bytes)?;
    register_economic_provider_parity(registry, provider_timeout, maximum_payload_bytes)?;
    register_sec(registry, maximum_payload_bytes)?;
    register_eastmoney(registry, provider_timeout, maximum_payload_bytes)?;

    let news = WallstreetCnClient::with_timeout(provider_timeout)?;
    registry.register_handler(
        admitted(
            Operation::GlobalNews,
            "WallstreetCn",
            "bounded official RSS metadata",
        ),
        move |command| {
            execute_global_news(
                command,
                &news,
                "WallstreetCn",
                ProviderId::WallstreetCn,
                maximum_payload_bytes,
            )
        },
    )?;
    register_global_news_parity(registry, provider_timeout, maximum_payload_bytes)?;
    register_sina_parity(registry, sina, maximum_payload_bytes)?;

    let cninfo = CninfoClient::new()?;
    let announcements = cninfo.clone();
    registry.register_handler(
        admitted(
            Operation::Announcements,
            "Cninfo",
            "bounded instrument announcements",
        ),
        move |command| {
            execute_typed(
                command,
                ANNOUNCEMENTS_REQUEST_SCHEMA,
                ANNOUNCEMENTS_RECORD_SCHEMA,
                "Cninfo",
                maximum_payload_bytes,
                |request: &InstrumentDateRangeRequest| announcements.announcements(request),
            )
        },
    )?;
    let market_announcements = cninfo.clone();
    registry.register_handler(
        admitted(
            Operation::MarketAnnouncements,
            "Cninfo",
            "bounded all-market announcements for an exact date range",
        ),
        move |command| {
            execute_typed(
                command,
                MARKET_ANNOUNCEMENTS_REQUEST_SCHEMA,
                ANNOUNCEMENTS_RECORD_SCHEMA,
                "Cninfo",
                maximum_payload_bytes,
                |request: &MarketAnnouncementRequest| {
                    market_announcements.market_announcements(request)
                },
            )
        },
    )?;
    registry.register_handler(
        admitted(
            Operation::InvestorQuestions,
            "Cninfo",
            "bounded instrument investor questions",
        ),
        move |command| {
            execute_typed(
                command,
                INVESTOR_QUESTIONS_REQUEST_SCHEMA,
                INVESTOR_QUESTIONS_RECORD_SCHEMA,
                "Cninfo",
                maximum_payload_bytes,
                |request: &InstrumentDateRangeRequest| cninfo.investor_questions(request),
            )
        },
    )?;

    let gov = GovClient::with_timeout(provider_timeout)?;
    registry.register_handler(
        admitted(
            Operation::PolicyDocuments,
            "StateCouncil",
            "bounded official State Council policy search",
        ),
        move |command| {
            execute_typed(
                command,
                POLICY_DOCUMENTS_REQUEST_SCHEMA,
                POLICY_DOCUMENTS_RECORD_SCHEMA,
                "StateCouncil",
                maximum_payload_bytes,
                |request: &PolicyRequest| gov.policy_documents(request),
            )
        },
    )?;
    register_additional_providers(registry, provider_timeout, maximum_payload_bytes)?;
    register_extended_handlers(registry, provider_timeout, maximum_payload_bytes)?;
    register_exact_blockers(registry)?;
    Ok(())
}

fn register_global_news_parity(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let cls = ClsClient::with_timeout(provider_timeout)?;
    register_global_news_provider(
        registry,
        cls.clone(),
        "Cls",
        ProviderId::Cailianpress,
        "bounded public CLS financial-news metadata (legacy selector)",
        maximum_payload_bytes,
    )?;
    register_global_news_provider(
        registry,
        cls,
        "Cailianpress",
        ProviderId::Cailianpress,
        "bounded public Cailianpress financial-news metadata",
        maximum_payload_bytes,
    )?;
    register_global_news_provider(
        registry,
        ThePaperClient::with_timeout(provider_timeout)?,
        "ThePaper",
        ProviderId::ThePaper,
        "bounded native The Paper finance-channel article metadata",
        maximum_payload_bytes,
    )?;
    register_global_news_provider(
        registry,
        XinhuaClient::with_timeout(provider_timeout)?,
        "XinhuaFinance",
        ProviderId::XinhuaFinance,
        "bounded first-party Xinhua Finance front-page metadata",
        maximum_payload_bytes,
    )?;
    register_global_news_provider(
        registry,
        YicaiClient::with_timeout(provider_timeout)?,
        "Yicai",
        ProviderId::Yicai,
        "bounded first-party Yicai first-page metadata",
        maximum_payload_bytes,
    )?;
    let stcn = StcnClient::with_timeout(provider_timeout)?;
    if STCN_GLOBAL_NEWS_ADMITTED {
        register_global_news_provider(
            registry,
            stcn,
            "SecuritiesTimes",
            ProviderId::SecuritiesTimes,
            "bounded first-party Securities Times front-page metadata",
            maximum_payload_bytes,
        )?;
    } else {
        registry.register_diagnostic_handler(
            blocked(
                Operation::GlobalNews,
                "SecuritiesTimes",
                "bounded diagnostic of first-party Securities Times front-page metadata",
                "the live source currently contains blank or unsafe source attribution and must be re-audited before production admission",
            ),
            move |command| execute_stcn_global_news(command, &stcn, maximum_payload_bytes),
        )?;
    }
    register_global_news_provider(
        registry,
        YonhapClient::for_channel_with_timeout(YonhapChannel::Economy, provider_timeout)?,
        "Yonhap",
        ProviderId::Yonhap,
        "bounded official Yonhap Economy RSS metadata only",
        maximum_payload_bytes,
    )?;
    Ok(())
}

fn register_global_news_provider<P>(
    registry: &mut OperationRegistry,
    client: P,
    provider: &'static str,
    expected_provider: ProviderId,
    scope: &'static str,
    maximum_payload_bytes: usize,
) -> Result<(), ServiceError>
where
    P: NewsProvider + Send + Sync + 'static,
    P::Error: Error + 'static,
{
    registry.register_handler(
        admitted(Operation::GlobalNews, provider, scope),
        move |command| {
            execute_global_news(
                command,
                &client,
                provider,
                expected_provider,
                maximum_payload_bytes,
            )
        },
    )
}

fn register_economic_provider_parity(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    register_economic_provider(
        registry,
        NbsClient::new(provider_timeout)?,
        "Nbs",
        "admitted national and regional NBS series with exact source namespace and period",
        maximum_payload_bytes,
    )?;
    register_economic_provider(
        registry,
        PbcClient::new(provider_timeout)?,
        "Pbc",
        "admitted money-supply, social-financing and regional PBC series",
        maximum_payload_bytes,
    )?;
    register_economic_provider(
        registry,
        WorldBankClient::new()?,
        "WorldBank",
        "exact admitted World Bank source 2 USA annual GDP observation",
        maximum_payload_bytes,
    )?;
    Ok(())
}

fn register_economic_provider<P>(
    registry: &mut OperationRegistry,
    client: P,
    provider: &'static str,
    scope: &'static str,
    maximum_payload_bytes: usize,
) -> Result<(), ServiceError>
where
    P: EconomicSeriesProvider + Send + Sync + 'static,
    P::Error: Error + 'static,
{
    registry.register_handler(
        admitted(Operation::EconomicSeries, provider, scope),
        move |command| {
            execute_typed(
                command,
                ECONOMIC_SERIES_REQUEST_SCHEMA,
                ECONOMIC_SERIES_RECORD_SCHEMA,
                provider,
                maximum_payload_bytes,
                |request: &EconomicSeriesRequest| client.economic_series(request),
            )
        },
    )
}

fn register_sina_parity(
    registry: &mut OperationRegistry,
    client: SinaClient,
    maximum_payload_bytes: usize,
) -> Result<(), ServiceError> {
    let quotes = client.clone();
    registry.register_handler(
        admitted(
            Operation::RealtimeQuotes,
            "Sina",
            "bounded verified Shanghai/Shenzhen/Beijing snapshot quotes",
        ),
        move |command| {
            let request: RealtimeQuotesRequest =
                decode_request(&command, REALTIME_QUOTES_REQUEST_SCHEMA)?;
            let batch = quotes
                .realtime_quotes(&request.instruments)
                .map_err(|error| provider_error(Operation::RealtimeQuotes, error))?;
            provider_query_result(
                batch,
                "Sina",
                REALTIME_QUOTES_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let bars = client.clone();
    registry.register_handler(
        admitted(
            Operation::HistoricalBars,
            "Sina",
            "one verified Shanghai/Shenzhen/Beijing instrument and bounded source interval",
        ),
        move |command| {
            execute_typed(
                command,
                HISTORICAL_BARS_REQUEST_SCHEMA,
                HISTORICAL_BARS_RECORD_SCHEMA,
                "Sina",
                maximum_payload_bytes,
                |request: &BarsRequest| bars.historical_bars(request),
            )
        },
    )?;

    let minute = client.clone();
    registry.register_handler(
        admitted(
            Operation::MinuteData,
            "Sina",
            "one verified Shanghai/Shenzhen/Beijing instrument and bounded minute request",
        ),
        move |command| {
            execute_typed(
                command,
                MINUTE_DATA_REQUEST_SCHEMA,
                MINUTE_DATA_RECORD_SCHEMA,
                "Sina",
                maximum_payload_bytes,
                |request: &MinuteDataRequest| minute.minute_data(request),
            )
        },
    )?;

    let books = client.clone();
    registry.register_handler(
        admitted(
            Operation::OrderBooks,
            "Sina",
            "bounded verified Shanghai/Shenzhen/Beijing five-level order books",
        ),
        move |command| {
            let request: InstrumentsRequest = decode_request(&command, ORDER_BOOKS_REQUEST_SCHEMA)?;
            let batch = books
                .order_books(&request.instruments)
                .map_err(|error| provider_error(Operation::OrderBooks, error))?;
            provider_query_result(
                batch,
                "Sina",
                ORDER_BOOKS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let metadata = client.clone();
    registry.register_handler(
        admitted(
            Operation::SecurityMetadata,
            "Sina",
            "bounded verified source security metadata",
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, SECURITY_METADATA_REQUEST_SCHEMA)?;
            let batch = metadata
                .security_metadata(&request.instruments)
                .map_err(|error| provider_error(Operation::SecurityMetadata, error))?;
            provider_query_result(
                batch,
                "Sina",
                SECURITY_METADATA_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    registry.register_handler(
        admitted(
            Operation::InstrumentNews,
            "Sina",
            "one Shanghai or Shenzhen A-share equity; bounded official company-news pages and inclusive source-date filter",
        ),
        move |command| execute_instrument_news(command, &client, maximum_payload_bytes),
    )?;
    Ok(())
}

fn register_exact_blockers(registry: &mut OperationRegistry) -> Result<(), ServiceError> {
    let capability = blocked(
        Operation::EconomicSeries,
        "Imf",
        "annual IMF economic-series adapter",
        "IMF DataMapper returns HTTP 403 and the replacement SDMX contract requires beta-portal authentication",
    );
    registry.register_unavailable(capability)?;
    Ok(())
}

fn register_extended_handlers(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let baidu_bars = BaiduClient::with_timeout(provider_timeout)?;
    registry.register_diagnostic_handler(
        blocked(
            Operation::HistoricalBars,
            "Baidu",
            "one A-share equity; bounded unadjusted daily OHLCV/amount",
            "trading-calendar, adjacent-session and corporate-action continuity evidence remain unproved",
        ),
        move |command| {
            execute_typed(
                command,
                HISTORICAL_BARS_REQUEST_SCHEMA,
                HISTORICAL_BARS_RECORD_SCHEMA,
                "Baidu",
                maximum_payload_bytes,
                |request: &BarsRequest| baidu_bars.historical_bars(request),
            )
        },
    )?;

    let eastmoney = EastmoneyClient::with_timeout(provider_timeout)?;
    let mx = if eastmoney_mx_key_is_configured() {
        Some(EastmoneyMxClient::from_env_with_client(&eastmoney)?)
    } else {
        None
    };
    if let Some(mx) = mx.as_ref() {
        let flow_series = mx.clone();
        registry.register_diagnostic_handler(
            blocked(
                Operation::FundFlowSeries,
                "EastmoneyMiaoxiang",
                "one Shanghai/Shenzhen equity; bounded daily main/super-large/large/medium/small net flow in CNY",
                "natural-language result cardinality and serial live stability remain repository-unadmitted",
            ),
            move |command| {
                execute_typed(
                    command,
                    FUND_FLOW_SERIES_REQUEST_SCHEMA,
                    FUND_FLOW_SERIES_RECORD_SCHEMA,
                    "EastmoneyMiaoxiang",
                    maximum_payload_bytes,
                    |request: &FundFlowRequest| flow_series.diagnose_daily_fund_flow(request),
                )
            },
        )?;

        let money_flows = mx.clone();
        registry.register_diagnostic_handler(
            blocked(
                Operation::MoneyFlows,
                "EastmoneyMiaoxiang",
                "one Shanghai/Shenzhen equity; latest bounded daily main/super-large/large/medium/small net flow in CNY",
                "source methodology and serial live stability remain repository-unadmitted",
            ),
            move |command| execute_mx_money_flow(&money_flows, command, maximum_payload_bytes),
        )?;

        let auctions = mx.clone();
        registry.register_handler(
            admitted(
                Operation::Auctions,
                "EastmoneyMiaoxiang",
                "one equity and exact source date; one-response opening-auction matched volume in shares and amount in CNY; Level-2 fields remain null",
            ),
            move |command| {
                let request: AuctionDiagnosticRequest =
                    decode_request(&command, AUCTIONS_REQUEST_SCHEMA)?;
                let batch = auctions
                    .diagnose_opening_auction(&request.instrument, &request.trading_date)
                    .map_err(|error| map_eastmoney_error(Operation::Auctions, &error))?;
                provider_query_result(
                    batch,
                    "EastmoneyMiaoxiang",
                    AUCTIONS_RECORD_SCHEMA,
                    maximum_payload_bytes,
                )
            },
        )?;

        let breadth = mx.clone();
        registry.register_handler(
            admitted(
                Operation::MarketBreadth,
                "EastmoneyMiaoxiang",
                "exact source date; one-response all-A listed/valid/up/down/flat/limit-up/limit-down counts with proved coverage",
            ),
            move |command| {
                let request: MarketBreadthDiagnosticRequest =
                    decode_request(&command, MARKET_BREADTH_REQUEST_SCHEMA)?;
                let batch = breadth
                    .diagnose_market_breadth(&request.source_date)
                    .map_err(|error| map_eastmoney_error(Operation::MarketBreadth, &error))?;
                provider_query_result(
                    batch,
                    "EastmoneyMiaoxiang",
                    MARKET_BREADTH_RECORD_SCHEMA,
                    maximum_payload_bytes,
                )
            },
        )?;
    } else {
        for capability in [
            blocked(
                Operation::FundFlowSeries,
                "EastmoneyMiaoxiang",
                "one Shanghai/Shenzhen equity; bounded daily main/super-large/large/medium/small net flow in CNY",
                "EASTMONEY_API_KEY or MX_APIKEY is not configured; natural-language result cardinality and serial live stability remain repository-unadmitted",
            ),
            blocked(
                Operation::MoneyFlows,
                "EastmoneyMiaoxiang",
                "one Shanghai/Shenzhen equity; latest bounded daily main/super-large/large/medium/small net flow in CNY",
                "EASTMONEY_API_KEY or MX_APIKEY is not configured; source methodology and serial live stability remain repository-unadmitted",
            ),
            runtime_unavailable(
                Operation::Auctions,
                "EastmoneyMiaoxiang",
                "one equity and exact source date; one-response opening-auction matched volume in shares and amount in CNY; Level-2 fields remain null",
                "EASTMONEY_API_KEY or MX_APIKEY is not configured",
            ),
            runtime_unavailable(
                Operation::MarketBreadth,
                "EastmoneyMiaoxiang",
                "exact source date; one-response all-A listed/valid/up/down/flat/limit-up/limit-down counts with proved coverage",
                "EASTMONEY_API_KEY or MX_APIKEY is not configured",
            ),
        ] {
            registry.register_unavailable(capability)?;
        }
    }

    let post_close = eastmoney.clone();
    registry.register_handler(
        admitted(
            Operation::PostCloseFlows,
            "Eastmoney",
            "bounded current-day post-close ranking using local observation time while retaining every row source time",
        ),
        move |command| {
            let request: PostCloseFlowRequest =
                decode_request(&command, POST_CLOSE_FLOWS_REQUEST_SCHEMA)?;
            let batch = post_close
                .post_close_flows(&request)
                .map_err(|error| map_eastmoney_error(Operation::PostCloseFlows, &error))?;
            provider_query_result(
                batch,
                "Eastmoney",
                POST_CLOSE_FLOWS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    registry.register_handler(
        admitted(
            Operation::MarketRankings,
            "Eastmoney",
            "one bounded A-share source ranking response with exact identity, rank, value, unit, source time and reported universe size",
        ),
        move |command| {
            let request: MarketRankingsRequest =
                decode_request(&command, MARKET_RANKINGS_REQUEST_SCHEMA)?;
            let batch = eastmoney
                .bounded_market_rankings_snapshot(&request.kind, request.limit)
                .map_err(|error| map_eastmoney_error(Operation::MarketRankings, &error))?;
            provider_query_result(
                batch,
                "Eastmoney",
                MARKET_RANKINGS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let cffex = CffexClient::new()?;
    registry.register_handler(
        admitted(
            Operation::FuturesDelivery,
            "Cffex",
            "versioned 2026 CFFEX equity-index futures delivery schedule; cash settlement; no runtime network transport",
        ),
        move |command| {
            let request: FuturesDeliveryRequest =
                decode_request(&command, FUTURES_DELIVERY_REQUEST_SCHEMA)?;
            let batch = cffex
                .futures_delivery_calendar(&request)
                .map_err(|error| provider_error(Operation::FuturesDelivery, error))?;
            provider_query_result(
                batch,
                "Cffex",
                FUTURES_DELIVERY_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;
    register_emquant(registry, provider_timeout, maximum_payload_bytes)?;
    register_hithink(registry, provider_timeout, maximum_payload_bytes)?;
    Ok(())
}

fn register_hithink(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let client = env::var("HITHINK_FINANCE_API_KEY")
        .map_err(|_| {
            HithinkError::InvalidRequest("HITHINK_FINANCE_API_KEY is not configured".into())
        })
        .and_then(|key| HithinkClient::with_timeout(key, provider_timeout));
    let client = match client {
        Ok(client) => Arc::new(client),
        Err(error) => {
            let blocker = format!("official HITHINK Fuyao API is not runtime-configured: {error}");
            for (operation, admitted_by_repository, scope) in [
                (
                    Operation::HistoricalBars,
                    HITHINK_HISTORICAL_BARS_ADMITTED,
                    HITHINK_HISTORICAL_BARS_SCOPE,
                ),
                (
                    Operation::MarketStatistics,
                    HITHINK_MARKET_STATISTICS_ADMITTED,
                    HITHINK_MARKET_STATISTICS_SCOPE,
                ),
                (
                    Operation::LimitPools,
                    HITHINK_LIMIT_POOLS_ADMITTED,
                    HITHINK_LIMIT_POOLS_SCOPE,
                ),
                (
                    Operation::Popularity,
                    HITHINK_POPULARITY_ADMITTED,
                    HITHINK_POPULARITY_SCOPE,
                ),
                (
                    Operation::FinancialStatements,
                    HITHINK_FINANCIAL_STATEMENTS_ADMITTED,
                    HITHINK_FINANCIAL_STATEMENTS_SCOPE,
                ),
                (
                    Operation::CorporateActions,
                    HITHINK_CORPORATE_ACTIONS_ADMITTED,
                    HITHINK_CORPORATE_ACTIONS_SCOPE,
                ),
                (
                    Operation::SecurityMetadata,
                    HITHINK_SECURITY_METADATA_ADMITTED,
                    HITHINK_SECURITY_METADATA_SCOPE,
                ),
                (
                    Operation::Auctions,
                    HITHINK_AUCTIONS_ADMITTED,
                    HITHINK_AUCTIONS_SCOPE,
                ),
            ] {
                let capability = if admitted_by_repository {
                    runtime_unavailable(operation, "HithinkFinance", scope, &blocker)
                } else if operation == Operation::Auctions {
                    blocked(operation, "HithinkFinance", scope, HITHINK_AUCTIONS_BLOCKER)
                } else {
                    blocked(
                        operation,
                        "HithinkFinance",
                        scope,
                        "HITHINK capability has not passed repository admission",
                    )
                };
                registry.register_unavailable(capability)?;
            }
            return Ok(());
        }
    };

    let bars = client.clone();
    registry.register_handler(
        admitted(
            Operation::HistoricalBars,
            "HithinkFinance",
            HITHINK_HISTORICAL_BARS_SCOPE,
        ),
        move |command| {
            execute_typed(
                command,
                HISTORICAL_BARS_REQUEST_SCHEMA,
                HISTORICAL_BARS_RECORD_SCHEMA,
                "HithinkFinance",
                maximum_payload_bytes,
                |request: &BarsRequest| bars.historical_bars(request),
            )
        },
    )?;

    let statistics = client.clone();
    registry.register_handler(
        admitted(
            Operation::MarketStatistics,
            "HithinkFinance",
            HITHINK_MARKET_STATISTICS_SCOPE,
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, MARKET_STATISTICS_REQUEST_SCHEMA)?;
            let batch = statistics
                .market_statistics(&request.instruments)
                .map_err(|error| provider_error(Operation::MarketStatistics, error))?;
            provider_query_result(
                batch,
                "HithinkFinance",
                MARKET_STATISTICS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let pools = client.clone();
    registry.register_handler(
        admitted(
            Operation::LimitPools,
            "HithinkFinance",
            HITHINK_LIMIT_POOLS_SCOPE,
        ),
        move |command| {
            execute_typed(
                command,
                LIMIT_POOLS_REQUEST_SCHEMA,
                LIMIT_POOLS_RECORD_SCHEMA,
                "HithinkFinance",
                maximum_payload_bytes,
                |request: &LimitPoolRequest| pools.limit_pool(request),
            )
        },
    )?;

    let financials = client.clone();
    registry.register_handler(
        admitted(
            Operation::FinancialStatements,
            "HithinkFinance",
            HITHINK_FINANCIAL_STATEMENTS_SCOPE,
        ),
        move |command| {
            let request: FinancialStatementsRequest =
                decode_request(&command, FINANCIAL_STATEMENTS_REQUEST_SCHEMA)?;
            let batch = financials
                .financial_statements(&request.instruments, request.kind)
                .map_err(|error| provider_error(Operation::FinancialStatements, error))?;
            provider_query_result(
                batch,
                "HithinkFinance",
                FINANCIAL_STATEMENTS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let actions = client.clone();
    registry.register_handler(
        admitted(
            Operation::CorporateActions,
            "HithinkFinance",
            HITHINK_CORPORATE_ACTIONS_SCOPE,
        ),
        move |command| {
            let request: CorporateActionRequest =
                decode_request(&command, CORPORATE_ACTIONS_REQUEST_SCHEMA)?;
            let response = actions
                .corporate_actions(&request)
                .map_err(|error| provider_error(Operation::CorporateActions, error))?;
            provider_query_result(
                response.into_batch(),
                "HithinkFinance",
                CORPORATE_ACTIONS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let metadata = client.clone();
    registry.register_handler(
        admitted(
            Operation::SecurityMetadata,
            "HithinkFinance",
            HITHINK_SECURITY_METADATA_SCOPE,
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, SECURITY_METADATA_REQUEST_SCHEMA)?;
            let batch = metadata
                .security_metadata(&request.instruments)
                .map_err(|error| provider_error(Operation::SecurityMetadata, error))?;
            provider_query_result(
                batch,
                "HithinkFinance",
                SECURITY_METADATA_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let auctions = client.clone();
    registry.register_diagnostic_handler(
        blocked(
            Operation::Auctions,
            "HithinkFinance",
            HITHINK_AUCTIONS_SCOPE,
            HITHINK_AUCTIONS_BLOCKER,
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, HITHINK_CURRENT_AUCTIONS_REQUEST_SCHEMA)?;
            let batch = auctions
                .probe_auction_snapshots(&request.instruments)
                .map_err(|error| provider_error(Operation::Auctions, error))?;
            provider_query_result(
                batch,
                "HithinkFinance",
                HITHINK_CURRENT_AUCTIONS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    registry.register_handler(
        admitted(
            Operation::Popularity,
            "HithinkFinance",
            HITHINK_POPULARITY_SCOPE,
        ),
        move |command| {
            let request: LimitRequest = decode_request(&command, POPULARITY_REQUEST_SCHEMA)?;
            let batch = client
                .popularity(request.limit)
                .map_err(|error| provider_error(Operation::Popularity, error))?;
            provider_query_result(
                batch,
                "HithinkFinance",
                POPULARITY_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;
    Ok(())
}

fn register_emquant(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let client = match EmQuantClient::discover()
        .and_then(|client| client.with_timeout(provider_timeout))
    {
        Ok(client) => Arc::new(client),
        Err(error) => {
            let blocker = format!("EMQuant read-only bridge is not runtime-discoverable: {error}");
            let bars = if EMQUANT_DAILY_BARS_ADMITTED {
                runtime_unavailable(
                    Operation::HistoricalBars,
                    "EmQuant",
                    EMQUANT_DAILY_BARS_SCOPE,
                    &blocker,
                )
            } else {
                blocked(
                    Operation::HistoricalBars,
                    "EmQuant",
                    EMQUANT_DAILY_BARS_SCOPE,
                    "EMQuant daily bars have not passed repository admission",
                )
            };
            registry.register_unavailable(bars)?;
            for (operation, scope) in [
                (
                    Operation::RealtimeQuotes,
                    "runtime-entitled EMQuant quote snapshot diagnostic",
                ),
                (
                    Operation::OrderBooks,
                    "runtime-entitled EMQuant five-level order-book diagnostic",
                ),
                (
                    Operation::MoneyFlows,
                    "runtime-entitled EMQuant normalized money-flow diagnostic",
                ),
            ] {
                registry.register_unavailable(blocked(operation, "EmQuant", scope, &blocker))?;
            }
            return Ok(());
        }
    };

    let quotes = client.clone();
    registry.register_diagnostic_handler(
        blocked(
            Operation::RealtimeQuotes,
            "EmQuant",
            "runtime-entitled EMQuant quote snapshot diagnostic",
            "EMQuant bridge availability and product entitlement do not constitute repository admission",
        ),
        move |command| {
            let request: RealtimeQuotesRequest =
                decode_request(&command, REALTIME_QUOTES_REQUEST_SCHEMA)?;
            let batch = quotes
                .realtime_quotes(&request.instruments)
                .map_err(|error| provider_error(Operation::RealtimeQuotes, error))?;
            provider_query_result(
                batch,
                "EmQuant",
                REALTIME_QUOTES_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    if EMQUANT_DAILY_BARS_ADMITTED {
        let bars = client.clone();
        registry.register_handler(
            admitted(
                Operation::HistoricalBars,
                "EmQuant",
                EMQUANT_DAILY_BARS_SCOPE,
            ),
            move |command| execute_emquant_daily_bars(&bars, command, maximum_payload_bytes),
        )?;
    } else {
        let bars = client.clone();
        registry.register_diagnostic_handler(
            blocked(
                Operation::HistoricalBars,
                "EmQuant",
                "runtime-entitled EMQuant daily-bar diagnostic",
                "EMQuant daily bars have not passed repository admission",
            ),
            move |command| {
                execute_typed(
                    command,
                    HISTORICAL_BARS_REQUEST_SCHEMA,
                    HISTORICAL_BARS_RECORD_SCHEMA,
                    "EmQuant",
                    maximum_payload_bytes,
                    |request: &BarsRequest| bars.historical_bars(request),
                )
            },
        )?;
    }

    let books = client.clone();
    registry.register_diagnostic_handler(
        blocked(
            Operation::OrderBooks,
            "EmQuant",
            "runtime-entitled EMQuant five-level order-book diagnostic",
            "EMQuant bridge availability and product entitlement do not constitute repository admission",
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, ORDER_BOOKS_REQUEST_SCHEMA)?;
            let batch = books
                .order_books(&request.instruments)
                .map_err(|error| provider_error(Operation::OrderBooks, error))?;
            provider_query_result(
                batch,
                "EmQuant",
                ORDER_BOOKS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    registry.register_diagnostic_handler(
        blocked(
            Operation::MoneyFlows,
            "EmQuant",
            "runtime-entitled EMQuant normalized money-flow diagnostic",
            "EMQuant bridge availability, field methodology and product entitlement remain repository-unadmitted",
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, MONEY_FLOWS_REQUEST_SCHEMA)?;
            let batch = client
                .money_flows(&request.instruments)
                .map_err(|error| provider_error(Operation::MoneyFlows, error))?;
            provider_query_result(
                batch,
                "EmQuant",
                MONEY_FLOWS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;
    Ok(())
}

fn validate_emquant_daily_bars_request(request: &BarsRequest) -> Result<(), ServiceError> {
    if request.interval() != BarInterval::Day {
        return Err(invalid(
            "EmQuant production bars require interval=Day; other intervals remain unadmitted",
        ));
    }
    if request.start().is_none() || request.end().is_none() {
        return Err(invalid(
            "EmQuant production bars require explicit start and end dates that exclude an unfinished source day",
        ));
    }
    if request.limit() > MAX_EMQUANT_DAILY_BARS {
        return Err(invalid(format!(
            "EmQuant production daily bars accept at most {MAX_EMQUANT_DAILY_BARS} rows"
        )));
    }
    if request.instrument().asset_class() != magic_market_core::AssetClass::Equity
        || !matches!(
            request.instrument().exchange(),
            magic_market_core::Exchange::Shanghai | magic_market_core::Exchange::Shenzhen
        )
    {
        return Err(invalid(
            "EmQuant production daily bars require a Shanghai or Shenzhen equity",
        ));
    }
    Ok(())
}

fn execute_emquant_daily_bars(
    client: &EmQuantClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: BarsRequest = decode_request(&command, HISTORICAL_BARS_REQUEST_SCHEMA)?;
    validate_emquant_daily_bars_request(&request)?;
    let batch = client
        .historical_bars(&request)
        .map_err(|error| provider_error(Operation::HistoricalBars, error))?;
    provider_query_result(
        batch,
        "EmQuant",
        HISTORICAL_BARS_RECORD_SCHEMA,
        maximum_payload_bytes,
    )
}

fn eastmoney_mx_key_is_configured() -> bool {
    ["EASTMONEY_API_KEY", "MX_APIKEY"]
        .iter()
        .any(|name| env::var_os(name).is_some_and(|value| !value.is_empty()))
}

#[cfg(test)]
fn hithink_key_is_configured() -> bool {
    env::var_os("HITHINK_FINANCE_API_KEY").is_some_and(|value| !value.is_empty())
}

fn execute_eastmoney_money_flow(
    client: &EastmoneyClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: InstrumentsRequest = decode_request(&command, MONEY_FLOWS_REQUEST_SCHEMA)?;
    let [instrument] = request.instruments.as_slice() else {
        return Err(ServiceError::InvalidRequest(
            "Eastmoney money flow requires exactly one instrument".to_owned(),
        ));
    };
    let request = FundFlowRequest::new(
        FlowScope::Instrument(instrument.clone()),
        FlowInterval::Day1,
        PositiveU32::new(1).map_err(precondition)?,
    )
    .map_err(precondition)?;
    let batch = client
        .fund_flow_series(&request)
        .map_err(|error| map_eastmoney_error(Operation::MoneyFlows, &error))?;
    money_flow_query_result(batch, instrument, "Eastmoney", maximum_payload_bytes)
}

fn execute_mx_money_flow(
    client: &EastmoneyMxClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: InstrumentsRequest = decode_request(&command, MONEY_FLOWS_REQUEST_SCHEMA)?;
    let [instrument] = request.instruments.as_slice() else {
        return Err(ServiceError::InvalidRequest(
            "diagnostic money flow requires exactly one instrument".to_owned(),
        ));
    };
    let request = FundFlowRequest::new(
        FlowScope::Instrument(instrument.clone()),
        FlowInterval::Day1,
        PositiveU32::new(1).map_err(precondition)?,
    )
    .map_err(precondition)?;
    let batch = client
        .diagnose_daily_fund_flow(&request)
        .map_err(|error| map_eastmoney_error(Operation::MoneyFlows, &error))?;
    money_flow_query_result(
        batch,
        instrument,
        "EastmoneyMiaoxiang",
        maximum_payload_bytes,
    )
}

fn money_flow_query_result(
    batch: DataBatch<FundFlowPoint>,
    instrument: &InstrumentId,
    provider: &str,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let point = batch.records().last().ok_or_else(|| {
        ServiceError::FailedPrecondition("Eastmoney fund-flow returned no latest point".to_owned())
    })?;
    let evidence = &point.evidence;
    let complete = point.main_net.is_some()
        && point.super_large_net.is_some()
        && point.large_net.is_some()
        && point.medium_net.is_some()
        && point.small_net.is_some()
        && evidence.source_at().is_some();
    let record = MoneyFlow::new(
        instrument.clone(),
        point.main_net,
        point.super_large_net,
        point.large_net,
        point.medium_net,
        point.small_net,
        if complete {
            DataStatus::Available
        } else {
            DataStatus::Unavailable
        },
        evidence.source_at().map(str::to_owned),
        evidence.observed_at().to_owned(),
        evidence.provider(),
        evidence.batch_id().to_owned(),
    )
    .map_err(precondition)?;
    let data = serde_json::to_vec(&record).map_err(|error| {
        ServiceError::Internal(format!("money-flow serialization failed: {error}"))
    })?;
    let payload = CanonicalPayload::new(
        MONEY_FLOWS_RECORD_SCHEMA,
        SCHEMA_VERSION,
        data,
        maximum_payload_bytes,
    )?;
    let provenance = batch.provenance();
    let batch_id = provenance.batch_id().ok_or_else(|| {
        ServiceError::FailedPrecondition("Eastmoney money-flow batch has no batch_id".to_owned())
    })?;
    Ok(QueryResult {
        provider: provider.to_owned(),
        batch_id: batch_id.to_owned(),
        complete,
        observed_at: provenance.fetched_at().to_owned(),
        source_at: provenance.source_at().map(str::to_owned),
        records: vec![payload],
        repository_admitted: false,
        diagnostic_blocker: None,
    })
}

fn register_additional_providers(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let baidu = BaiduClient::with_timeout(provider_timeout)?;
    registry.register_handler(
        admitted(
            Operation::TechnicalBars,
            "Baidu",
            "one A-share equity; bounded source-supplied unadjusted daily OHLCV/amount and optional MA5/10/20 without an adjusted-continuity claim",
        ),
        move |command| {
            execute_typed(
                command,
                TECHNICAL_BARS_REQUEST_SCHEMA,
                TECHNICAL_BARS_RECORD_SCHEMA,
                "Baidu",
                maximum_payload_bytes,
                |request: &BarsRequest| baidu.technical_bars(request),
            )
        },
    )?;

    let jin10 = Jin10Client::with_timeout(provider_timeout)?;
    let calendar = jin10.clone();
    if ECONOMIC_CALENDAR_ADMITTED {
        registry.register_handler(
            admitted(
                Operation::EconomicCalendar,
                "Jin10",
                "bounded public economic-release calendar",
            ),
            move |command| {
                execute_typed(
                    command,
                    ECONOMIC_CALENDAR_REQUEST_SCHEMA,
                    ECONOMIC_CALENDAR_RECORD_SCHEMA,
                    "Jin10",
                    maximum_payload_bytes,
                    |request: &EconomicCalendarRequest| calendar.economic_calendar(request),
                )
            },
        )?;
    } else {
        registry.register_diagnostic_handler(
            blocked(
                Operation::EconomicCalendar,
                "Jin10",
                "bounded diagnostic over economic releases in the latest public flash window",
                "Jin10 ended its free calendar and API embedding service on 2025-12-01; the latest public flash window cannot prove a complete economic calendar",
            ),
            move |command| {
                execute_typed(
                    command,
                    ECONOMIC_CALENDAR_REQUEST_SCHEMA,
                    ECONOMIC_CALENDAR_RECORD_SCHEMA,
                    "Jin10",
                    maximum_payload_bytes,
                    |request: &EconomicCalendarRequest| calendar.economic_calendar(request),
                )
            },
        )?;
    }
    registry.register_handler(
        admitted(
            Operation::GlobalNews,
            "Jin10",
            "bounded public flash-news metadata",
        ),
        move |command| {
            execute_global_news(
                command,
                &jin10,
                "Jin10",
                ProviderId::Jin10,
                maximum_payload_bytes,
            )
        },
    )?;

    let hkex = HkexClient::new()?;
    registry.register_handler(
        admitted(
            Operation::NorthboundDaily,
            "Hkex",
            "official Shanghai- or Shenzhen-connect northbound daily statistics for an exact date",
        ),
        move |command| {
            execute_typed(
                command,
                NORTHBOUND_DAILY_REQUEST_SCHEMA,
                NORTHBOUND_DAILY_RECORD_SCHEMA,
                "Hkex",
                maximum_payload_bytes,
                |request: &NorthboundDailyRequest| hkex.northbound_daily_statistics(request),
            )
        },
    )?;
    register_exchange_parity(registry, provider_timeout, maximum_payload_bytes)?;

    let ths = ThsClient::new()?;
    let consensus = ths.clone();
    registry.register_handler(
        admitted(
            Operation::Consensus,
            "Tonghuashun",
            "bounded Shanghai/Shenzhen equity consensus snapshots",
        ),
        move |command| {
            let request: InstrumentsRequest = decode_request(&command, CONSENSUS_REQUEST_SCHEMA)?;
            let batch = consensus
                .consensus(&request.instruments)
                .map_err(|error| provider_error(Operation::Consensus, error))?;
            provider_query_result(
                batch,
                "Tonghuashun",
                CONSENSUS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;
    let strong = ths.clone();
    registry.register_handler(
        admitted(
            Operation::StrongStockReasons,
            "Tonghuashun",
            "bounded equity strong-stock reason for an exact trading date",
        ),
        move |command| {
            execute_typed(
                command,
                STRONG_STOCK_REASONS_REQUEST_SCHEMA,
                STRONG_STOCK_REASONS_RECORD_SCHEMA,
                "Tonghuashun",
                maximum_payload_bytes,
                |request: &InstrumentSignalRequest| strong.strong_stock_reasons(request),
            )
        },
    )?;
    let limit_pools = ths.clone();
    registry.register_handler(
        admitted(
            Operation::LimitPools,
            "Tonghuashun",
            "upper-limit pool only for an exact trading date",
        ),
        move |command| {
            execute_typed(
                command,
                LIMIT_POOLS_REQUEST_SCHEMA,
                LIMIT_POOLS_RECORD_SCHEMA,
                "Tonghuashun",
                maximum_payload_bytes,
                |request: &LimitPoolRequest| limit_pools.limit_pool(request),
            )
        },
    )?;
    registry.register_handler(
        admitted(
            Operation::Popularity,
            "Tonghuashun",
            "bounded public hourly popularity ranking",
        ),
        move |command| {
            let request: LimitRequest = decode_request(&command, POPULARITY_REQUEST_SCHEMA)?;
            let batch = ths
                .popularity(request.limit)
                .map_err(|error| provider_error(Operation::Popularity, error))?;
            provider_query_result(
                batch,
                "Tonghuashun",
                POPULARITY_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let options = SinaClient::with_timeout(provider_timeout)?;
    registry.register_handler(
        admitted(
            Operation::OptionData,
            "Sina",
            "verified option contract discovery, quotes or Greeks",
        ),
        move |command| {
            let request: OptionDataRequest = decode_request(&command, OPTION_DATA_REQUEST_SCHEMA)?;
            match request {
                OptionDataRequest::Contracts { underlying, expiry } => {
                    let batch = options
                        .option_contracts(&underlying, expiry.as_ref())
                        .map_err(|error| provider_error(Operation::OptionData, error))?;
                    provider_query_result(
                        batch,
                        "Sina",
                        OPTION_CONTRACT_RECORD_SCHEMA,
                        maximum_payload_bytes,
                    )
                }
                OptionDataRequest::Quotes { contracts } => {
                    let batch = options
                        .option_quotes(&contracts)
                        .map_err(|error| provider_error(Operation::OptionData, error))?;
                    provider_query_result(
                        batch,
                        "Sina",
                        OPTION_QUOTE_RECORD_SCHEMA,
                        maximum_payload_bytes,
                    )
                }
                OptionDataRequest::Greeks { contracts } => {
                    let batch = options
                        .option_greeks(&contracts)
                        .map_err(|error| provider_error(Operation::OptionData, error))?;
                    provider_query_result(
                        batch,
                        "Sina",
                        OPTION_GREEKS_RECORD_SCHEMA,
                        maximum_payload_bytes,
                    )
                }
            }
        },
    )?;
    register_tdx_public(registry, provider_timeout, maximum_payload_bytes)?;
    register_iwencai(registry, maximum_payload_bytes)?;
    Ok(())
}

fn register_exchange_parity(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let sse = SseClient::with_config(SseConfig {
        timeout: provider_timeout,
        ..SseConfig::default()
    })?;
    let szse = SzseClient::with_config(SzseConfig {
        timeout: provider_timeout,
        ..SzseConfig::default()
    })?;

    let quotes = szse.clone();
    registry.register_handler(
        admitted(
            Operation::RealtimeQuotes,
            "Szse",
            "official SZSE snapshot quotes for explicitly requested Shenzhen instruments",
        ),
        move |command| {
            let request: RealtimeQuotesRequest =
                decode_request(&command, REALTIME_QUOTES_REQUEST_SCHEMA)?;
            let batch = quotes
                .realtime_quotes(&request.instruments)
                .map_err(|error| provider_error(Operation::RealtimeQuotes, error))?;
            provider_query_result(
                batch,
                "Szse",
                REALTIME_QUOTES_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let books = szse.clone();
    registry.register_handler(
        admitted(
            Operation::OrderBooks,
            "Szse",
            "official SZSE five-level books for explicitly requested Shenzhen instruments",
        ),
        move |command| {
            let request: InstrumentsRequest = decode_request(&command, ORDER_BOOKS_REQUEST_SCHEMA)?;
            let batch = books
                .order_books(&request.instruments)
                .map_err(|error| provider_error(Operation::OrderBooks, error))?;
            provider_query_result(
                batch,
                "Szse",
                ORDER_BOOKS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let sse_announcements = sse.clone();
    registry.register_handler(
        admitted(
            Operation::Announcements,
            "Sse",
            "official SSE instrument announcements for an explicit bounded date range",
        ),
        move |command| {
            execute_typed(
                command,
                ANNOUNCEMENTS_REQUEST_SCHEMA,
                ANNOUNCEMENTS_RECORD_SCHEMA,
                "Sse",
                maximum_payload_bytes,
                |request: &InstrumentDateRangeRequest| sse_announcements.announcements(request),
            )
        },
    )?;
    let szse_announcements = szse.clone();
    registry.register_handler(
        admitted(
            Operation::Announcements,
            "Szse",
            "official SZSE instrument announcements for an explicit bounded date range",
        ),
        move |command| {
            execute_typed(
                command,
                ANNOUNCEMENTS_REQUEST_SCHEMA,
                ANNOUNCEMENTS_RECORD_SCHEMA,
                "Szse",
                maximum_payload_bytes,
                |request: &InstrumentDateRangeRequest| szse_announcements.announcements(request),
            )
        },
    )?;

    registry.register_handler(
        admitted(
            Operation::DragonTiger,
            "Sse",
            "official SSE dragon-tiger entries or seats for an explicit request",
        ),
        move |command| {
            let request: DragonTigerRequest =
                decode_request(&command, DRAGON_TIGER_REQUEST_SCHEMA)?;
            match request {
                DragonTigerRequest::Entries(request) => {
                    let batch = sse
                        .dragon_tiger_entries(&request)
                        .map_err(|error| provider_error(Operation::DragonTiger, error))?;
                    provider_query_result(
                        batch,
                        "Sse",
                        DRAGON_TIGER_ENTRY_RECORD_SCHEMA,
                        maximum_payload_bytes,
                    )
                }
                DragonTigerRequest::Seats(request) => {
                    let batch = sse
                        .dragon_tiger_seats(&request)
                        .map_err(|error| provider_error(Operation::DragonTiger, error))?;
                    provider_query_result(
                        batch,
                        "Sse",
                        DRAGON_TIGER_SEAT_RECORD_SCHEMA,
                        maximum_payload_bytes,
                    )
                }
            }
        },
    )?;
    registry.register_handler(
        admitted(
            Operation::DragonTiger,
            "Szse",
            "official SZSE dragon-tiger entries or seats for an explicit request",
        ),
        move |command| {
            let request: DragonTigerRequest =
                decode_request(&command, DRAGON_TIGER_REQUEST_SCHEMA)?;
            match request {
                DragonTigerRequest::Entries(request) => {
                    let batch = szse
                        .dragon_tiger_entries(&request)
                        .map_err(|error| provider_error(Operation::DragonTiger, error))?;
                    provider_query_result(
                        batch,
                        "Szse",
                        DRAGON_TIGER_ENTRY_RECORD_SCHEMA,
                        maximum_payload_bytes,
                    )
                }
                DragonTigerRequest::Seats(request) => {
                    let batch = szse
                        .dragon_tiger_seats(&request)
                        .map_err(|error| provider_error(Operation::DragonTiger, error))?;
                    provider_query_result(
                        batch,
                        "Szse",
                        DRAGON_TIGER_SEAT_RECORD_SCHEMA,
                        maximum_payload_bytes,
                    )
                }
            }
        },
    )?;
    Ok(())
}

fn register_tdx_public(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let timeout_seconds = provider_timeout.as_secs_f64();
    let client = Arc::new(TdxSmartClient::new());
    register_tdx_market_parity(
        registry,
        client.clone(),
        timeout_seconds,
        maximum_payload_bytes,
    )?;
    let t0_evidence = client.clone();
    registry.register_handler(
        admitted(
            Operation::T0Evidence,
            "Tdx",
            "bounded TDX-only quote, book, daily-bar and five-minute-bar bundle using local observation time",
        ),
        move |command| {
            execute_tdx_t0_evidence(
                &t0_evidence,
                timeout_seconds,
                command,
                maximum_payload_bytes,
            )
        },
    )?;
    let outcome_bars = client.clone();
    registry.register_handler(
        admitted(
            Operation::OutcomeDailyBars,
            "Tdx",
            "TDX-only exact daily-bar preimage ending on the requested through date with no routing or fallback",
        ),
        move |command| {
            execute_tdx_outcome_daily_bars(
                &outcome_bars,
                timeout_seconds,
                command,
                maximum_payload_bytes,
            )
        },
    )?;
    let boards = TdxBoardProvider::new("180.153.18.170", 7709, timeout_seconds);
    let directory = boards.clone();
    registry.register_handler(
        admitted(
            Operation::BoardDirectory,
            "Tdx",
            "TDX public protocol concept or industry board directory",
        ),
        move |command| {
            execute_typed(
                command,
                BOARD_DIRECTORY_REQUEST_SCHEMA,
                BOARD_DIRECTORY_RECORD_SCHEMA,
                "Tdx",
                maximum_payload_bytes,
                |request: &BoardDirectoryRequest| directory.boards(request),
            )
        },
    )?;
    let constituents = boards.clone();
    registry.register_handler(
        admitted(
            Operation::BoardConstituents,
            "Tdx",
            "TDX public protocol exact board constituents",
        ),
        move |command| {
            execute_typed(
                command,
                BOARD_CONSTITUENTS_REQUEST_SCHEMA,
                BOARD_MEMBERSHIP_RECORD_SCHEMA,
                "Tdx",
                maximum_payload_bytes,
                |request: &BoardConstituentRequest| constituents.board_constituents(request),
            )
        },
    )?;
    registry.register_handler(
        admitted(
            Operation::BoardMemberships,
            "Tdx",
            "TDX public protocol reverse board memberships for bounded equities",
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, BOARD_MEMBERSHIPS_REQUEST_SCHEMA)?;
            let batch = boards
                .board_memberships(&request.instruments)
                .map_err(|error| provider_error(Operation::BoardMemberships, error))?;
            provider_query_result(
                batch,
                "Tdx",
                BOARD_MEMBERSHIP_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let block_service = Arc::new(BlockService::new("180.153.18.170", 7709, timeout_seconds));
    registry.register_handler(
        admitted(
            Operation::ConceptHits,
            "Tdx",
            "TDX public protocol concept memberships for bounded equities",
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, CONCEPT_HITS_REQUEST_SCHEMA)?;
            let batch = block_service
                .concept_hits(&request.instruments)
                .map_err(|error| provider_error(Operation::ConceptHits, error))?;
            provider_query_result(
                batch,
                "Tdx",
                CONCEPT_HITS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let profiles = Arc::new(TdxSecurityProfileProvider::new(
        "180.153.18.170",
        7709,
        timeout_seconds,
    )?);
    registry.register_handler(
        admitted(
            Operation::SecurityProfiles,
            "Tdx",
            "1..=8 unique Shanghai/Shenzhen equities; exact TDX name, optional finance-backed listing date and complete company-overview F10 source-line facts",
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, SECURITY_PROFILES_REQUEST_SCHEMA)?;
            let batch = profiles
                .security_profiles(&request.instruments)
                .map_err(|error| provider_error(Operation::SecurityProfiles, error))?;
            provider_query_result(
                batch,
                "Tdx",
                SECURITY_PROFILES_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    registry.register_handler(
        admitted(
            Operation::CorporateActions,
            "Tdx",
            "TDX public protocol bounded corporate actions with explicit admission date",
        ),
        move |command| {
            let request: CorporateActionRequest =
                decode_request(&command, CORPORATE_ACTIONS_REQUEST_SCHEMA)?;
            client
                .connect_to_any(Some(timeout_seconds))
                .map_err(|error| provider_error(Operation::CorporateActions, error))?;
            let response = client
                .corporate_actions(&request)
                .map_err(|error| provider_error(Operation::CorporateActions, error))?;
            provider_query_result(
                response.into_batch(),
                "Tdx",
                CORPORATE_ACTIONS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;
    Ok(())
}

fn register_tdx_market_parity(
    registry: &mut OperationRegistry,
    client: Arc<TdxSmartClient>,
    timeout_seconds: f64,
    maximum_payload_bytes: usize,
) -> Result<(), ServiceError> {
    let quotes = client.clone();
    registry.register_handler(
        admitted(
            Operation::RealtimeQuotes,
            "Tdx",
            "TDX public protocol 1..=60 normalized quotes without strict source-time freshness",
        ),
        move |command| {
            let request: RealtimeQuotesRequest =
                decode_request(&command, REALTIME_QUOTES_REQUEST_SCHEMA)?;
            quotes
                .connect_to_any(Some(timeout_seconds))
                .map_err(|error| provider_error(Operation::RealtimeQuotes, error))?;
            let batch = quotes
                .realtime_quotes(&request.instruments)
                .map_err(|error| provider_error(Operation::RealtimeQuotes, error))?;
            provider_query_result(
                batch,
                "Tdx",
                REALTIME_QUOTES_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let bars = client.clone();
    registry.register_handler(
        admitted(
            Operation::HistoricalBars,
            "Tdx",
            "TDX public protocol normalized exact-pagination historical bars",
        ),
        move |command| {
            let request: BarsRequest = decode_request(&command, HISTORICAL_BARS_REQUEST_SCHEMA)?;
            bars.connect_to_any(Some(timeout_seconds))
                .map_err(|error| provider_error(Operation::HistoricalBars, error))?;
            let batch = bars
                .historical_bars(&request)
                .map_err(|error| provider_error(Operation::HistoricalBars, error))?;
            provider_query_result(
                batch,
                "Tdx",
                HISTORICAL_BARS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let minute = client.clone();
    registry.register_handler(
        admitted(
            Operation::MinuteData,
            "Tdx",
            "TDX public protocol normalized current or historical minute observations",
        ),
        move |command| {
            let request: MinuteDataRequest = decode_request(&command, MINUTE_DATA_REQUEST_SCHEMA)?;
            minute
                .connect_to_any(Some(timeout_seconds))
                .map_err(|error| provider_error(Operation::MinuteData, error))?;
            let batch = minute
                .minute_data(&request)
                .map_err(|error| provider_error(Operation::MinuteData, error))?;
            provider_query_result(
                batch,
                "Tdx",
                MINUTE_DATA_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let books = client.clone();
    registry.register_handler(
        admitted(
            Operation::OrderBooks,
            "Tdx",
            "TDX public protocol normalized five-level order books",
        ),
        move |command| {
            let request: InstrumentsRequest = decode_request(&command, ORDER_BOOKS_REQUEST_SCHEMA)?;
            books
                .connect_to_any(Some(timeout_seconds))
                .map_err(|error| provider_error(Operation::OrderBooks, error))?;
            let batch = books
                .order_books(&request.instruments)
                .map_err(|error| provider_error(Operation::OrderBooks, error))?;
            provider_query_result(
                batch,
                "Tdx",
                ORDER_BOOKS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let trades = client.clone();
    registry.register_handler(
        admitted(
            Operation::Trades,
            "Tdx",
            "TDX public protocol bounded normalized current or historical transactions",
        ),
        move |command| {
            let request: TradesRequest = decode_request(&command, TRADES_REQUEST_SCHEMA)?;
            trades
                .connect_to_any(Some(timeout_seconds))
                .map_err(|error| provider_error(Operation::Trades, error))?;
            let batch = trades
                .trades(&request)
                .map_err(|error| provider_error(Operation::Trades, error))?;
            provider_query_result(batch, "Tdx", TRADES_RECORD_SCHEMA, maximum_payload_bytes)
        },
    )?;

    registry.register_handler(
        admitted(
            Operation::SecurityMetadata,
            "Tdx",
            "TDX public protocol bounded normalized security metadata",
        ),
        move |command| {
            let request: InstrumentsRequest =
                decode_request(&command, SECURITY_METADATA_REQUEST_SCHEMA)?;
            client
                .connect_to_any(Some(timeout_seconds))
                .map_err(|error| provider_error(Operation::SecurityMetadata, error))?;
            let batch = client
                .security_metadata(&request.instruments)
                .map_err(|error| provider_error(Operation::SecurityMetadata, error))?;
            provider_query_result(
                batch,
                "Tdx",
                SECURITY_METADATA_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;
    Ok(())
}

fn register_iwencai(
    registry: &mut OperationRegistry,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    if !SEMANTIC_SEARCH_ADMITTED {
        registry.register_unavailable(blocked(
            Operation::SemanticSearch,
            "Iwencai",
            "authorized bounded semantic research search",
            "repository semantic-search admission is disabled",
        ))?;
        return Ok(());
    }
    let has_key = env::var("MAGIC_IWENCAI_API_KEY").is_ok() || env::var("IWENCAI_API_KEY").is_ok();
    if !has_key {
        registry.register_unavailable(runtime_unavailable(
            Operation::SemanticSearch,
            "Iwencai",
            "authorized bounded semantic research search",
            "MAGIC_IWENCAI_API_KEY is not present in the server process environment",
        ))?;
        return Ok(());
    }
    let client = IwencaiClient::from_env()?;
    registry.register_handler(
        admitted(
            Operation::SemanticSearch,
            "Iwencai",
            "authorized bounded semantic research search",
        ),
        move |command| {
            execute_typed(
                command,
                SEMANTIC_SEARCH_REQUEST_SCHEMA,
                SEMANTIC_SEARCH_RECORD_SCHEMA,
                "Iwencai",
                maximum_payload_bytes,
                |request: &SemanticSearchRequest| client.semantic_search(request),
            )
        },
    )?;
    Ok(())
}

fn register_eastmoney(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let client = EastmoneyClient::with_timeout(provider_timeout)?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::FundFlowSeries,
        FUND_FLOW_SERIES_REQUEST_SCHEMA,
        FUND_FLOW_SERIES_RECORD_SCHEMA,
        "one Shanghai or Shenzhen equity; bounded one-minute or daily public fund-flow series with exact CNY values and per-record source time",
        maximum_payload_bytes,
        |client, request: &FundFlowRequest| client.fund_flow_series(request),
    )?;
    let money_flows = client.clone();
    registry.register_handler(
        admitted(
            Operation::MoneyFlows,
            "Eastmoney",
            "one Shanghai or Shenzhen equity; latest complete daily public fund-flow point with exact CNY values and source date",
        ),
        move |command| execute_eastmoney_money_flow(&money_flows, command, maximum_payload_bytes),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::ResearchReports,
        RESEARCH_REPORTS_REQUEST_SCHEMA,
        RESEARCH_REPORTS_RECORD_SCHEMA,
        "bounded instrument or industry research-report metadata",
        maximum_payload_bytes,
        |client, request: &ResearchRequest| client.research_reports(request),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::ResearchDocuments,
        RESEARCH_DOCUMENTS_REQUEST_SCHEMA,
        RESEARCH_DOCUMENTS_RECORD_SCHEMA,
        "exact Eastmoney research PDF identity and bounded document extraction",
        maximum_payload_bytes,
        |client, request: &ResearchDocumentRequest| client.research_document(request),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::TargetPrices,
        TARGET_PRICES_REQUEST_SCHEMA,
        TARGET_PRICES_RECORD_SCHEMA,
        "bounded instrument target-price consensus",
        maximum_payload_bytes,
        |client, request: &TargetPriceRequest| client.target_price_consensus(request),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::MarginData,
        MARGIN_DATA_REQUEST_SCHEMA,
        MARGIN_DATA_RECORD_SCHEMA,
        "bounded instrument/date margin balances",
        maximum_payload_bytes,
        |client, request: &InstrumentDateRangeRequest| client.margin_data(request),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::BlockTrades,
        BLOCK_TRADES_REQUEST_SCHEMA,
        BLOCK_TRADES_RECORD_SCHEMA,
        "bounded instrument/date block trades",
        maximum_payload_bytes,
        |client, request: &InstrumentDateRangeRequest| client.block_trades(request),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::HolderCounts,
        HOLDER_COUNTS_REQUEST_SCHEMA,
        HOLDER_COUNTS_RECORD_SCHEMA,
        "bounded instrument/date holder counts",
        maximum_payload_bytes,
        |client, request: &InstrumentDateRangeRequest| client.holder_counts(request),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::LockupEvents,
        LOCKUP_EVENTS_REQUEST_SCHEMA,
        LOCKUP_EVENTS_RECORD_SCHEMA,
        "bounded instrument/date lockup events",
        maximum_payload_bytes,
        |client, request: &InstrumentDateRangeRequest| client.lockup_events(request),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::DividendPlans,
        DIVIDEND_PLANS_REQUEST_SCHEMA,
        DIVIDEND_PLANS_RECORD_SCHEMA,
        "bounded instrument/date dividend plans",
        maximum_payload_bytes,
        |client, request: &InstrumentDateRangeRequest| client.dividend_plans(request),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::LimitPools,
        LIMIT_POOLS_REQUEST_SCHEMA,
        LIMIT_POOLS_RECORD_SCHEMA,
        "upper, broken, lower and previous-upper pools for an exact trading date",
        maximum_payload_bytes,
        |client, request: &LimitPoolRequest| client.limit_pool(request),
    )?;
    let upper_limit_review = client.clone();
    registry.register_handler(
        admitted(
            Operation::UpperLimitPoolReview,
            "Eastmoney",
            "atomic four-family Eastmoney limit-pool facts for one exact trading date",
        ),
        move |command| {
            execute_upper_limit_pool_review(&upper_limit_review, command, maximum_payload_bytes)
        },
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::MarketDragonTiger,
        MARKET_DRAGON_TIGER_REQUEST_SCHEMA,
        MARKET_DRAGON_TIGER_RECORD_SCHEMA,
        "bounded whole-market dragon-tiger disclosures for an exact trading date",
        maximum_payload_bytes,
        |client, request: &MarketDragonTigerRequest| client.market_dragon_tiger(request),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::DragonTigerDiscovery,
        DRAGON_TIGER_DISCOVERY_REQUEST_SCHEMA,
        DRAGON_TIGER_ENTRY_RECORD_SCHEMA,
        "bounded complete-market dragon-tiger discovery",
        maximum_payload_bytes,
        |client, request: &DragonTigerDiscoveryRequest| client.discover_dragon_tiger(request),
    )?;
    register_eastmoney_typed(
        registry,
        &client,
        Operation::ProviderTopNRankings,
        PROVIDER_TOP_N_REQUEST_SCHEMA,
        PROVIDER_TOP_N_RECORD_SCHEMA,
        "single-response A-share volume-ratio or main-net-inflow Top-N after the 15:35 gate",
        maximum_payload_bytes,
        |client, request: &ProviderTopNRankingRequest| client.provider_top_n_rankings(request),
    )?;

    let board_flows = client.clone();
    registry.register_handler(
        admitted(
            Operation::BoardFlows,
            "Eastmoney",
            "industry, concept or region board flows with an explicit interval and limit",
        ),
        move |command| {
            let request: BoardFlowsRequest = decode_request(&command, BOARD_FLOWS_REQUEST_SCHEMA)?;
            let batch = board_flows
                .board_flows(request.category, request.interval, request.limit)
                .map_err(|error| provider_error(Operation::BoardFlows, error))?;
            provider_query_result(
                batch,
                "Eastmoney",
                BOARD_FLOWS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;
    let popularity = client.clone();
    registry.register_handler(
        admitted(
            Operation::Popularity,
            "Eastmoney",
            "bounded public popularity ranking",
        ),
        move |command| {
            let request: LimitRequest = decode_request(&command, POPULARITY_REQUEST_SCHEMA)?;
            let batch = popularity
                .popularity(request.limit)
                .map_err(|error| provider_error(Operation::Popularity, error))?;
            provider_query_result(
                batch,
                "Eastmoney",
                POPULARITY_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;
    registry.register_handler(
        admitted(
            Operation::DragonTiger,
            "Eastmoney",
            "bounded instrument dragon-tiger entries or seats",
        ),
        move |command| {
            let request: DragonTigerRequest =
                decode_request(&command, DRAGON_TIGER_REQUEST_SCHEMA)?;
            match request {
                DragonTigerRequest::Entries(request) => {
                    let batch = client
                        .dragon_tiger_entries(&request)
                        .map_err(|error| provider_error(Operation::DragonTiger, error))?;
                    provider_query_result(
                        batch,
                        "Eastmoney",
                        DRAGON_TIGER_ENTRY_RECORD_SCHEMA,
                        maximum_payload_bytes,
                    )
                }
                DragonTigerRequest::Seats(request) => {
                    let batch = client
                        .dragon_tiger_seats(&request)
                        .map_err(|error| provider_error(Operation::DragonTiger, error))?;
                    provider_query_result(
                        batch,
                        "Eastmoney",
                        DRAGON_TIGER_SEAT_RECORD_SCHEMA,
                        maximum_payload_bytes,
                    )
                }
            }
        },
    )?;

    // The same provider exposes admitted global-news metadata as an explicitly
    // selectable alternative to the default WallstreetCN registration.
    let news = EastmoneyClient::with_timeout(provider_timeout)?;
    registry.register_handler(
        admitted(
            Operation::GlobalNews,
            "Eastmoney",
            "bounded latest Eastmoney finance-news metadata",
        ),
        move |command| {
            execute_global_news(
                command,
                &news,
                "Eastmoney",
                ProviderId::Eastmoney,
                maximum_payload_bytes,
            )
        },
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn register_eastmoney_typed<TRequest, TRecord>(
    registry: &mut OperationRegistry,
    client: &EastmoneyClient,
    operation: Operation,
    request_schema: &'static str,
    record_schema: &'static str,
    scope: &'static str,
    maximum_payload_bytes: usize,
    call: fn(&EastmoneyClient, &TRequest) -> Result<DataBatch<TRecord>, EastmoneyError>,
) -> Result<(), ServiceError>
where
    TRequest: DeserializeOwned + 'static,
    TRecord: Serialize + 'static,
{
    let client = client.clone();
    registry.register_handler(admitted(operation, "Eastmoney", scope), move |command| {
        execute_typed(
            command,
            request_schema,
            record_schema,
            "Eastmoney",
            maximum_payload_bytes,
            |request: &TRequest| call(&client, request),
        )
    })
}

fn register_fred(
    registry: &mut OperationRegistry,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let Ok(api_key) = env::var("FRED_API_KEY") else {
        registry.register_unavailable(runtime_unavailable(
            Operation::EconomicSeries,
            "Fred",
            "admitted FRED economic series",
            "FRED_API_KEY is not present in the server process environment",
        ))?;
        return Ok(());
    };
    let fred = FredClient::new(api_key)?;
    registry.register_handler(
        admitted(
            Operation::EconomicSeries,
            "Fred",
            "admitted FRED economic series",
        ),
        move |command| {
            execute_typed(
                command,
                ECONOMIC_SERIES_REQUEST_SCHEMA,
                ECONOMIC_SERIES_RECORD_SCHEMA,
                "Fred",
                maximum_payload_bytes,
                |request: &EconomicSeriesRequest| fred.economic_series(request),
            )
        },
    )?;
    Ok(())
}

fn register_sec(
    registry: &mut OperationRegistry,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let Ok(user_agent) = env::var("SEC_USER_AGENT") else {
        registry.register_unavailable(runtime_unavailable(
            Operation::CompanyFilings,
            "SecEdgar",
            "admitted bounded filing metadata",
            "SEC_USER_AGENT is not present in the server process environment",
        ))?;
        return Ok(());
    };
    let sec = SecEdgarClient::new(user_agent)?;
    registry.register_handler(
        admitted(
            Operation::CompanyFilings,
            "SecEdgar",
            "admitted bounded filing metadata; document bodies and XBRL facts excluded",
        ),
        move |command| {
            execute_typed(
                command,
                COMPANY_FILINGS_REQUEST_SCHEMA,
                COMPANY_FILINGS_RECORD_SCHEMA,
                "SecEdgar",
                maximum_payload_bytes,
                |request: &CompanyFilingRequest| sec.company_filings(request),
            )
        },
    )?;
    Ok(())
}

fn capability(operation: Operation, exact_scope: &str) -> Capability {
    admitted(operation, TENCENT_PROVIDER, exact_scope)
}

fn admitted(operation: Operation, provider: &str, exact_scope: &str) -> Capability {
    Capability {
        operation,
        repository_admitted: true,
        runtime_available: true,
        provider: provider.to_owned(),
        exact_scope: exact_scope.to_owned(),
        blocker: None,
        diagnostic_available: false,
    }
}

fn runtime_unavailable(
    operation: Operation,
    provider: &str,
    exact_scope: &str,
    blocker: &str,
) -> Capability {
    Capability {
        operation,
        repository_admitted: true,
        runtime_available: false,
        provider: provider.to_owned(),
        exact_scope: exact_scope.to_owned(),
        blocker: Some(blocker.to_owned()),
        diagnostic_available: false,
    }
}

fn blocked(operation: Operation, provider: &str, exact_scope: &str, blocker: &str) -> Capability {
    Capability {
        operation,
        repository_admitted: false,
        runtime_available: false,
        provider: provider.to_owned(),
        exact_scope: exact_scope.to_owned(),
        blocker: Some(blocker.to_owned()),
        diagnostic_available: false,
    }
}

fn execute_tencent_quotes(
    client: &TencentClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: RealtimeQuotesRequest = decode_request(&command, REALTIME_QUOTES_REQUEST_SCHEMA)?;
    let batch = client
        .realtime_quotes(&request.instruments)
        .map_err(|error| map_tencent_error(Operation::RealtimeQuotes, error))?;
    provider_query_result(
        batch,
        TENCENT_PROVIDER,
        REALTIME_QUOTES_RECORD_SCHEMA,
        maximum_payload_bytes,
    )
}

fn execute_tencent_index_quotes(
    client: &TencentClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: IndexQuotesRequest = decode_request(&command, INDEX_QUOTES_REQUEST_SCHEMA)?;
    let maximum = Duration::from_millis(request.maximum_source_age_millis());
    let policy = AcceptancePolicy::new()
        .with_require_complete(true)
        .with_require_source_at(true)
        .with_require_available_records(true)
        .with_max_source_age(maximum)
        .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
    let mut router = QuoteRouter::new(policy);
    router
        .register(quote_source(
            ProviderId::Tencent,
            Arc::new(client.clone()),
            classify_tencent_source_error,
        ))
        .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    let batch = router
        .route(request.indices())
        .map_err(map_index_quote_router_error)?
        .into_batch();
    provider_query_result(
        batch,
        TENCENT_PROVIDER,
        INDEX_QUOTES_RECORD_SCHEMA,
        maximum_payload_bytes,
    )
}

fn execute_tencent_intraday_shape(
    client: &TencentClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: IntradayShapeRequest = decode_request(&command, INTRADAY_SHAPE_REQUEST_SCHEMA)?;
    let minute_request = match request.trading_date() {
        Some(date) => MinuteDataRequest::new(request.instrument().clone())
            .with_date(date.as_str())
            .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?,
        None => MinuteDataRequest::new(request.instrument().clone()),
    };
    let batch = client
        .minute_data(&minute_request)
        .map_err(|error| map_tencent_error(Operation::IntradayShape, error))?;
    if !batch.quality().is_complete()
        && !batch
            .quality()
            .issues()
            .iter()
            .all(|issue| issue.ends_with(": cumulative amount unavailable"))
    {
        return Err(ServiceError::FailedPrecondition(format!(
            "Tencent IntradayShape input is incomplete: {}",
            batch.quality().issues().join("; ")
        )));
    }
    let evidence = tencent_batch_evidence(&batch)?;
    let regular_points = batch
        .records()
        .iter()
        .filter(|point| is_regular_session_minute(point.minute_at()))
        .collect::<Vec<_>>();
    if regular_points.is_empty() {
        return Err(ServiceError::FailedPrecondition(
            "Tencent IntradayShape input has no regular-session points".into(),
        ));
    }
    if regular_points.len()
        > usize::try_from(request.maximum_points().get()).map_err(|_| {
            ServiceError::InvalidRequest("IntradayShape maximum_points exceeds usize".into())
        })?
    {
        return Err(ServiceError::FailedPrecondition(format!(
            "Tencent regular-session minute count {} exceeds requested maximum_points {}",
            regular_points.len(),
            request.maximum_points().get()
        )));
    }
    validate_intraday_points(&regular_points, request.instrument(), &evidence)?;
    let trading_date = IsoDate::new(&regular_points[0].minute_at()[..10])
        .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    if request
        .trading_date()
        .is_some_and(|requested| requested != &trading_date)
    {
        return Err(ServiceError::FailedPrecondition(
            "Tencent IntradayShape source date contradicts the request".into(),
        ));
    }
    if request.trading_date().is_none() && trading_date != current_china_date_for_shape()? {
        return Err(ServiceError::FailedPrecondition(format!(
            "Tencent current-session minute source date {} is not the current China date",
            trading_date.as_str()
        )));
    }
    let first_at = minute_point_instant(regular_points[0].minute_at())?;
    let last = regular_points
        .last()
        .copied()
        .ok_or_else(|| ServiceError::Internal("IntradayShape lost its last point".into()))?;
    let last_at = minute_point_instant(last.minute_at())?;
    let open = regular_points[0].price();
    let latest = last.price();
    let high = magic_market_core::Price::new(
        regular_points
            .iter()
            .map(|point| point.price().get())
            .fold(f64::NEG_INFINITY, f64::max),
    )
    .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    let low = magic_market_core::Price::new(
        regular_points
            .iter()
            .map(|point| point.price().get())
            .fold(f64::INFINITY, f64::min),
    )
    .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    let (up_points, down_points, flat_points) = intraday_direction_counts(&regular_points)?;
    let cumulative_volume = Some(last.cumulative_quantity());
    let amount_presence = regular_points
        .iter()
        .map(|point| point.cumulative_amount().is_some())
        .collect::<std::collections::BTreeSet<_>>();
    if amount_presence.len() != 1 {
        return Err(ServiceError::FailedPrecondition(
            "IntradayShape cumulative amount presence changes within one source series".into(),
        ));
    }
    let cumulative_amount = last.cumulative_amount();
    let vwap = match (cumulative_amount, last.cumulative_quantity().get()) {
        (Some(amount), lots) if lots > 0.0 => Some(
            magic_market_core::Price::new(amount.get() / (lots * 100.0))
                .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?,
        ),
        _ => None,
    };
    let input_evidence = vec![evidence];
    let input_digest_sha256 = intraday_shape_digest(&request, batch.records(), &input_evidence)?;
    let point_count = PositiveU32::new(u32::try_from(regular_points.len()).map_err(|_| {
        ServiceError::FailedPrecondition("IntradayShape point count exceeds u32".into())
    })?)
    .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    let record = IntradayShapeRecord::new(
        request.instrument().clone(),
        trading_date,
        first_at,
        last_at,
        point_count,
        open,
        high,
        low,
        latest,
        vwap,
        cumulative_volume,
        cumulative_amount,
        up_points,
        down_points,
        flat_points,
        input_evidence.clone(),
        PositiveU32::new(1).map_err(|error| ServiceError::Internal(error.to_string()))?,
        input_digest_sha256.clone(),
    )
    .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    let payload = CanonicalPayload::new(
        INTRADAY_SHAPE_RECORD_SCHEMA,
        SCHEMA_VERSION,
        serde_json::to_vec(&record).map_err(|error| {
            ServiceError::Internal(format!("IntradayShape serialization failed: {error}"))
        })?,
        maximum_payload_bytes,
    )?;
    Ok(QueryResult {
        provider: "LocalAnalysis".to_owned(),
        batch_id: format!("local-analysis:intraday-shape:{input_digest_sha256}"),
        complete: true,
        observed_at: input_evidence[0].observed_at().to_owned(),
        source_at: input_evidence[0].source_at().map(str::to_owned),
        records: vec![payload],
        repository_admitted: true,
        diagnostic_blocker: None,
    })
}

fn tencent_batch_evidence<T>(batch: &DataBatch<T>) -> Result<SourceEvidence, ServiceError> {
    let provenance = batch.provenance();
    if provenance.source() != "tencent-web" {
        return Err(ServiceError::FailedPrecondition(format!(
            "IntradayShape expected tencent-web provenance, got {}",
            provenance.source()
        )));
    }
    let batch_id = provenance.batch_id().ok_or_else(|| {
        ServiceError::FailedPrecondition("IntradayShape input batch has no batch_id".into())
    })?;
    let evidence = SourceEvidence::new(ProviderId::Tencent, provenance.fetched_at(), batch_id)
        .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    match provenance.source_at() {
        Some(source_at) => evidence
            .with_source_at(source_at)
            .map_err(|error| ServiceError::FailedPrecondition(error.to_string())),
        None => Err(ServiceError::FailedPrecondition(
            "IntradayShape input provenance has no source_at".into(),
        )),
    }
}

fn is_regular_session_minute(minute_at: &str) -> bool {
    let Some(clock) = minute_at.get(11..16) else {
        return false;
    };
    ("09:30"..="11:30").contains(&clock) || ("13:00"..="15:00").contains(&clock)
}

fn current_china_date_for_shape() -> Result<IsoDate, ServiceError> {
    let offset = UtcOffset::from_hms(8, 0, 0)
        .map_err(|error| ServiceError::Internal(format!("fixed China offset failed: {error}")))?;
    IsoDate::new(
        OffsetDateTime::now_utc()
            .to_offset(offset)
            .date()
            .to_string(),
    )
    .map_err(|error| ServiceError::Internal(error.to_string()))
}

fn minute_point_instant(minute_at: &str) -> Result<String, ServiceError> {
    let date = minute_at.get(..10).ok_or_else(|| {
        ServiceError::FailedPrecondition("IntradayShape minute date is missing".into())
    })?;
    let clock = minute_at.get(11..16).ok_or_else(|| {
        ServiceError::FailedPrecondition("IntradayShape minute clock is missing".into())
    })?;
    let instant = format!("{date}T{clock}:00+08:00");
    EvidenceTimestamp::parse_instant(&instant)
        .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    Ok(instant)
}

fn validate_intraday_points(
    points: &[&MinutePoint],
    instrument: &InstrumentId,
    evidence: &SourceEvidence,
) -> Result<(), ServiceError> {
    let mut previous: Option<&str> = None;
    for point in points {
        if point.instrument() != instrument
            || point.provider() != ProviderId::Tencent
            || point.batch_id() != evidence.batch_id()
            || point.observed_at() != evidence.observed_at()
            || point.source_at().is_none()
        {
            return Err(ServiceError::FailedPrecondition(
                "IntradayShape minute evidence is incomplete or inconsistent".into(),
            ));
        }
        let expected_status = if point.cumulative_amount().is_some() {
            DataStatus::Available
        } else {
            DataStatus::Unavailable
        };
        if point.status() != expected_status {
            return Err(ServiceError::FailedPrecondition(
                "IntradayShape minute status contradicts cumulative amount availability".into(),
            ));
        }
        if previous.is_some_and(|value| value >= point.minute_at()) {
            return Err(ServiceError::FailedPrecondition(
                "IntradayShape minutes must be strictly ordered".into(),
            ));
        }
        previous = Some(point.minute_at());
    }
    Ok(())
}

fn intraday_direction_counts(points: &[&MinutePoint]) -> Result<(u32, u32, u32), ServiceError> {
    let mut up = 0_u32;
    let mut down = 0_u32;
    let mut flat = 1_u32;
    for pair in points.windows(2) {
        let previous = pair[0].price().get();
        let current = pair[1].price().get();
        match current.total_cmp(&previous) {
            std::cmp::Ordering::Greater => {
                up = up.checked_add(1).ok_or_else(|| {
                    ServiceError::FailedPrecondition("direction count overflow".into())
                })?;
            }
            std::cmp::Ordering::Less => {
                down = down.checked_add(1).ok_or_else(|| {
                    ServiceError::FailedPrecondition("direction count overflow".into())
                })?;
            }
            std::cmp::Ordering::Equal => {
                flat = flat.checked_add(1).ok_or_else(|| {
                    ServiceError::FailedPrecondition("direction count overflow".into())
                })?;
            }
        }
    }
    Ok((up, down, flat))
}

#[derive(Serialize)]
struct IntradayShapeDigestInput<'a> {
    request: &'a IntradayShapeRequest,
    points: &'a [MinutePoint],
    input_evidence: &'a [SourceEvidence],
}

fn intraday_shape_digest(
    request: &IntradayShapeRequest,
    points: &[MinutePoint],
    input_evidence: &[SourceEvidence],
) -> Result<String, ServiceError> {
    let normalized = serde_json::to_vec(&IntradayShapeDigestInput {
        request,
        points,
        input_evidence,
    })
    .map_err(|error| {
        ServiceError::Internal(format!("IntradayShape digest encoding failed: {error}"))
    })?;
    domain_separated_sha256(b"magic.intraday_shape.v1\0", &normalized)
}

fn domain_separated_sha256(domain: &[u8], normalized: &[u8]) -> Result<String, ServiceError> {
    let length = u64::try_from(normalized.len())
        .map_err(|_| ServiceError::Internal("digest input length exceeds u64".into()))?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(length.to_be_bytes());
    hasher.update(normalized);
    Ok(format!("{:x}", hasher.finalize()))
}

fn execute_upper_limit_pool_review(
    client: &EastmoneyClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: UpperLimitPoolReviewRequest =
        decode_request(&command, UPPER_LIMIT_POOL_REVIEW_REQUEST_SCHEMA)?;
    let fetch = |kind| -> Result<LimitPoolReviewInput, ServiceError> {
        let pool_request = LimitPoolRequest::new(
            kind,
            request.trading_date().clone(),
            request.per_pool_limit(),
        )
        .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
        match client.limit_pool(&pool_request) {
            Ok(batch) => {
                if !batch.quality().is_complete() {
                    return Err(ServiceError::FailedPrecondition(format!(
                        "Eastmoney upper-limit review input is incomplete: {}",
                        batch.quality().issues().join("; ")
                    )));
                }
                let evidence = eastmoney_batch_evidence(&batch)?;
                Ok(LimitPoolReviewInput {
                    records: batch.into_records(),
                    evidence,
                })
            }
            Err(EastmoneyError::VerifiedEmpty(empty)) => {
                validate_limit_pool_verified_empty(&empty, &pool_request)?;
                Ok(LimitPoolReviewInput {
                    records: Vec::new(),
                    evidence: empty.evidence().clone(),
                })
            }
            Err(error) => Err(provider_error(Operation::UpperLimitPoolReview, error)),
        }
    };
    let upper = fetch(magic_market_core::LimitPoolKind::Upper)?;
    let broken = fetch(magic_market_core::LimitPoolKind::Broken)?;
    let lower = fetch(magic_market_core::LimitPoolKind::Lower)?;
    let previous_upper = fetch(magic_market_core::LimitPoolKind::PreviousUpper)?;
    let input_evidence = vec![
        upper.evidence.clone(),
        broken.evidence.clone(),
        lower.evidence.clone(),
        previous_upper.evidence.clone(),
    ];
    let input_digest_sha256 = upper_limit_review_digest(
        request.trading_date(),
        &upper.records,
        &broken.records,
        &lower.records,
        &previous_upper.records,
        &input_evidence,
    )?;
    let record = UpperLimitPoolReviewRecord::new(
        request.trading_date().clone(),
        upper.records,
        broken.records,
        lower.records,
        previous_upper.records,
        input_evidence.clone(),
        PositiveU32::new(1).map_err(|error| ServiceError::Internal(error.to_string()))?,
        input_digest_sha256.clone(),
    )
    .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    let data = serde_json::to_vec(&record).map_err(|error| {
        ServiceError::Internal(format!(
            "UpperLimitPoolReview serialization failed: {error}"
        ))
    })?;
    let payload = CanonicalPayload::new(
        UPPER_LIMIT_POOL_REVIEW_RECORD_SCHEMA,
        SCHEMA_VERSION,
        data,
        maximum_payload_bytes,
    )?;
    let observed_at = input_evidence
        .last()
        .map(SourceEvidence::observed_at)
        .ok_or_else(|| ServiceError::Internal("UpperLimitPoolReview has no evidence".into()))?
        .to_owned();
    let source_at = common_source_at(&input_evidence);
    Ok(QueryResult {
        provider: "Eastmoney".to_owned(),
        batch_id: format!(
            "eastmoney:upper-limit-pool-review:{}:{}",
            request.trading_date().as_str(),
            input_digest_sha256
        ),
        complete: true,
        observed_at,
        source_at,
        records: vec![payload],
        repository_admitted: true,
        diagnostic_blocker: None,
    })
}

struct LimitPoolReviewInput {
    records: Vec<magic_market_core::LimitPoolEntry>,
    evidence: SourceEvidence,
}

fn validate_limit_pool_verified_empty(
    empty: &magic_market_core::VerifiedEmpty,
    request: &LimitPoolRequest,
) -> Result<(), ServiceError> {
    let expected_identity = format!(
        "{:?}:{}:limit={}",
        request.kind(),
        request.trading_date().as_str(),
        request.limit().get()
    );
    if empty.family() != "limit_pool"
        || empty.request_identity() != expected_identity
        || empty.evidence().provider() != ProviderId::Eastmoney
        || empty.evidence().source_at() != Some(request.trading_date().as_str())
        || empty.provenance().source() != "eastmoney-web"
    {
        return Err(ServiceError::FailedPrecondition(
            "Eastmoney verified-empty evidence contradicts the limit-pool request".into(),
        ));
    }
    Ok(())
}

fn eastmoney_batch_evidence<T>(batch: &DataBatch<T>) -> Result<SourceEvidence, ServiceError> {
    let provenance = batch.provenance();
    if provenance.source() != "eastmoney-web" {
        return Err(ServiceError::FailedPrecondition(format!(
            "UpperLimitPoolReview expected eastmoney-web provenance, got {}",
            provenance.source()
        )));
    }
    let batch_id = provenance.batch_id().ok_or_else(|| {
        ServiceError::FailedPrecondition("UpperLimitPoolReview input batch has no batch_id".into())
    })?;
    let evidence = SourceEvidence::new(ProviderId::Eastmoney, provenance.fetched_at(), batch_id)
        .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    match provenance.source_at() {
        Some(source_at) => evidence
            .with_source_at(source_at)
            .map_err(|error| ServiceError::FailedPrecondition(error.to_string())),
        None => Ok(evidence),
    }
}

#[derive(Serialize)]
struct UpperLimitReviewDigestInput<'a> {
    trading_date: &'a IsoDate,
    upper: &'a [magic_market_core::LimitPoolEntry],
    broken: &'a [magic_market_core::LimitPoolEntry],
    lower: &'a [magic_market_core::LimitPoolEntry],
    previous_upper: &'a [magic_market_core::LimitPoolEntry],
    input_evidence: &'a [SourceEvidence],
}

fn upper_limit_review_digest(
    trading_date: &IsoDate,
    upper: &[magic_market_core::LimitPoolEntry],
    broken: &[magic_market_core::LimitPoolEntry],
    lower: &[magic_market_core::LimitPoolEntry],
    previous_upper: &[magic_market_core::LimitPoolEntry],
    input_evidence: &[SourceEvidence],
) -> Result<String, ServiceError> {
    let normalized = serde_json::to_vec(&UpperLimitReviewDigestInput {
        trading_date,
        upper,
        broken,
        lower,
        previous_upper,
        input_evidence,
    })
    .map_err(|error| {
        ServiceError::Internal(format!(
            "UpperLimitPoolReview digest encoding failed: {error}"
        ))
    })?;
    let length = u64::try_from(normalized.len()).map_err(|_| {
        ServiceError::Internal("UpperLimitPoolReview digest input length exceeds u64".into())
    })?;
    let mut hasher = Sha256::new();
    hasher.update(b"magic.upper_limit_pool_review.v1\0");
    hasher.update(length.to_be_bytes());
    hasher.update(&normalized);
    Ok(format!("{:x}", hasher.finalize()))
}

fn common_source_at(input_evidence: &[SourceEvidence]) -> Option<String> {
    let source_at = input_evidence.first()?.source_at()?;
    input_evidence
        .iter()
        .all(|evidence| evidence.source_at() == Some(source_at))
        .then(|| source_at.to_owned())
}

fn execute_tdx_t0_evidence(
    client: &TdxSmartClient,
    timeout_seconds: f64,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: T0EvidenceRequest = decode_request_version(
        &command,
        T0_EVIDENCE_REQUEST_SCHEMA,
        T0_EVIDENCE_SCHEMA_VERSION,
    )?;
    client
        .connect_to_any(Some(timeout_seconds))
        .map_err(|error| provider_error(Operation::T0Evidence, error))?;
    let quotes = client
        .realtime_quotes(request.instruments())
        .map_err(|error| provider_error(Operation::T0Evidence, error))?;
    let books = client
        .order_books(request.instruments())
        .map_err(|error| provider_error(Operation::T0Evidence, error))?;
    let quote_evidence = tdx_batch_evidence(Operation::T0Evidence, &quotes)?;
    let book_evidence = tdx_batch_evidence(Operation::T0Evidence, &books)?;
    validate_exact_tdx_instruments("quote", request.instruments(), quotes.records(), |record| {
        record.instrument()
    })?;
    validate_exact_tdx_instruments(
        "order-book",
        request.instruments(),
        books.records(),
        |record| record.instrument(),
    )?;

    let daily_limit = checked_tdx_bar_limit("daily_bar_count", request.daily_bar_count())?;
    let five_minute_limit =
        checked_tdx_bar_limit("five_minute_bar_count", request.five_minute_bar_count())?;
    let mut records = Vec::with_capacity(request.instruments().len());
    let mut payloads = Vec::with_capacity(request.instruments().len());
    let mut all_evidence = vec![quote_evidence.clone(), book_evidence.clone()];
    for instrument in request.instruments() {
        let daily_request = BarsRequest::new(instrument.clone(), BarInterval::Day, daily_limit)
            .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
        let daily = client
            .historical_bars(&daily_request)
            .map_err(|error| provider_error(Operation::T0Evidence, error))?;
        require_complete_tdx_batch(Operation::T0Evidence, "daily bars", &daily)?;
        let daily_evidence = tdx_batch_evidence(Operation::T0Evidence, &daily)?;

        let five_minute_request =
            BarsRequest::new(instrument.clone(), BarInterval::Minute5, five_minute_limit)
                .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
        let five_minute = client
            .historical_bars(&five_minute_request)
            .map_err(|error| provider_error(Operation::T0Evidence, error))?;
        require_complete_tdx_batch(Operation::T0Evidence, "five-minute bars", &five_minute)?;
        let five_minute_evidence = tdx_batch_evidence(Operation::T0Evidence, &five_minute)?;

        let quote = exact_tdx_record("quote", quotes.records(), instrument, |record| {
            record.instrument()
        })?;
        let order_book = exact_tdx_record("order-book", books.records(), instrument, |record| {
            record.instrument()
        })?;
        let input_evidence = vec![
            quote_evidence.clone(),
            book_evidence.clone(),
            daily_evidence.clone(),
            five_minute_evidence.clone(),
        ];
        let daily_bars = daily.into_records();
        let five_minute_bars = five_minute.into_records();
        let digest = t0_evidence_digest(
            instrument,
            request.requested_at(),
            &quote,
            &order_book,
            &daily_bars,
            &five_minute_bars,
            &input_evidence,
        )?;
        let record = T0EvidenceRecord::new(
            instrument.clone(),
            request.requested_at(),
            quote,
            order_book,
            daily_bars,
            five_minute_bars,
            request.daily_bar_count(),
            request.five_minute_bar_count(),
            input_evidence,
            PositiveU32::new(T0_EVIDENCE_SCHEMA_VERSION)
                .map_err(|error| ServiceError::Internal(error.to_string()))?,
            digest,
        )
        .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
        payloads.push(CanonicalPayload::new(
            T0_EVIDENCE_RECORD_SCHEMA,
            T0_EVIDENCE_SCHEMA_VERSION,
            serde_json::to_vec(&record).map_err(|error| {
                ServiceError::Internal(format!("T0Evidence serialization failed: {error}"))
            })?,
            maximum_payload_bytes,
        )?);
        all_evidence.push(daily_evidence);
        all_evidence.push(five_minute_evidence);
        records.push(record);
    }
    let aggregate_digest = domain_separated_sha256(
        b"magic.t0_evidence.batch.v2\0",
        &serde_json::to_vec(&records).map_err(|error| {
            ServiceError::Internal(format!("T0Evidence batch digest encoding failed: {error}"))
        })?,
    )?;
    let observed_at = current_china_observed_at()?;
    Ok(QueryResult {
        provider: "Tdx".to_owned(),
        batch_id: format!("tdx:t0-evidence:{aggregate_digest}"),
        complete: true,
        observed_at,
        source_at: common_source_at(&all_evidence),
        records: payloads,
        repository_admitted: true,
        diagnostic_blocker: None,
    })
}

fn execute_tdx_outcome_daily_bars(
    client: &TdxSmartClient,
    timeout_seconds: f64,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: OutcomeDailyBarsRequest =
        decode_request(&command, OUTCOME_DAILY_BARS_REQUEST_SCHEMA)?;
    client
        .connect_to_any(Some(timeout_seconds))
        .map_err(|error| provider_error(Operation::OutcomeDailyBars, error))?;
    let limit = checked_tdx_bar_limit("limit", request.limit())?;
    let bars_request = BarsRequest::new(request.instrument().clone(), BarInterval::Day, limit)
        .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
    let batch = client
        .historical_bars(&bars_request)
        .map_err(|error| provider_error(Operation::OutcomeDailyBars, error))?;
    require_complete_tdx_batch(Operation::OutcomeDailyBars, "daily bars", &batch)?;
    let evidence = tdx_batch_evidence(Operation::OutcomeDailyBars, &batch)?;
    let bars = batch.into_records();
    let input_evidence = vec![evidence.clone()];
    let digest = outcome_daily_bars_digest(&request, &bars, &input_evidence)?;
    let record = OutcomeDailyBarsRecord::new(
        request.instrument().clone(),
        request.through().clone(),
        request.outcome_due_at(),
        bars,
        request.limit(),
        input_evidence,
        PositiveU32::new(1).map_err(|error| ServiceError::Internal(error.to_string()))?,
        digest.clone(),
    )
    .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    let payload = CanonicalPayload::new(
        OUTCOME_DAILY_BARS_RECORD_SCHEMA,
        SCHEMA_VERSION,
        serde_json::to_vec(&record).map_err(|error| {
            ServiceError::Internal(format!("OutcomeDailyBars serialization failed: {error}"))
        })?,
        maximum_payload_bytes,
    )?;
    Ok(QueryResult {
        provider: "Tdx".to_owned(),
        batch_id: format!("tdx:outcome-daily-bars:{digest}"),
        complete: true,
        observed_at: evidence.observed_at().to_owned(),
        source_at: evidence.source_at().map(str::to_owned),
        records: vec![payload],
        repository_admitted: true,
        diagnostic_blocker: None,
    })
}

fn checked_tdx_bar_limit(field: &'static str, value: PositiveU32) -> Result<u16, ServiceError> {
    u16::try_from(value.get()).map_err(|_| {
        ServiceError::InvalidRequest(format!("{field} exceeds the TDX u16 request limit"))
    })
}

fn require_complete_tdx_batch<T>(
    operation: Operation,
    family: &str,
    batch: &DataBatch<T>,
) -> Result<(), ServiceError> {
    if batch.quality().is_complete() {
        Ok(())
    } else {
        Err(ServiceError::Unavailable {
            operation,
            reason: format!(
                "TDX {family} input is incomplete: {}",
                batch.quality().issues().join("; ")
            ),
        })
    }
}

fn tdx_batch_evidence<T>(
    operation: Operation,
    batch: &DataBatch<T>,
) -> Result<SourceEvidence, ServiceError> {
    let provenance = batch.provenance();
    if provenance.source() != "tdx-smart" {
        return Err(ServiceError::FailedPrecondition(format!(
            "{} expected tdx-smart provenance, got {}",
            operation.as_str(),
            provenance.source()
        )));
    }
    let batch_id = provenance.batch_id().ok_or_else(|| {
        ServiceError::FailedPrecondition(format!(
            "{} TDX input batch has no batch_id",
            operation.as_str()
        ))
    })?;
    let evidence = SourceEvidence::new(ProviderId::Tdx, provenance.fetched_at(), batch_id)
        .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
    provenance
        .source_at()
        .map_or(Ok(evidence.clone()), |source_at| {
            evidence
                .with_source_at(source_at)
                .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))
        })
}

fn validate_exact_tdx_instruments<T>(
    family: &str,
    requested: &[InstrumentId],
    records: &[T],
    instrument: impl Fn(&T) -> &InstrumentId + Copy,
) -> Result<(), ServiceError> {
    if records.len() != requested.len()
        || requested.iter().any(|requested| {
            records
                .iter()
                .filter(|record| instrument(record) == requested)
                .count()
                != 1
        })
    {
        return Err(ServiceError::FailedPrecondition(format!(
            "TDX T0Evidence {family} identities do not exactly match the request"
        )));
    }
    Ok(())
}

fn exact_tdx_record<T: Clone>(
    family: &str,
    records: &[T],
    requested: &InstrumentId,
    instrument: impl Fn(&T) -> &InstrumentId,
) -> Result<T, ServiceError> {
    let mut matching = records
        .iter()
        .filter(|record| instrument(record) == requested);
    let record = matching.next().cloned().ok_or_else(|| {
        ServiceError::FailedPrecondition(format!(
            "TDX T0Evidence omitted {family} for {}",
            requested.code()
        ))
    })?;
    if matching.next().is_some() {
        return Err(ServiceError::FailedPrecondition(format!(
            "TDX T0Evidence duplicated {family} for {}",
            requested.code()
        )));
    }
    Ok(record)
}

fn current_china_observed_at() -> Result<String, ServiceError> {
    let offset = UtcOffset::from_hms(8, 0, 0)
        .map_err(|error| ServiceError::Internal(format!("fixed China offset failed: {error}")))?;
    let now = OffsetDateTime::now_utc().to_offset(offset);
    Ok(format!(
        "{}T{:02}:{:02}:{:02}+08:00",
        now.date(),
        now.hour(),
        now.minute(),
        now.second()
    ))
}

#[derive(Serialize)]
struct T0EvidenceDigestInput<'a> {
    instrument: &'a InstrumentId,
    requested_at: &'a str,
    quote: &'a Quote,
    order_book: &'a OrderBook,
    daily_bars: &'a [Bar],
    five_minute_bars: &'a [Bar],
    input_evidence: &'a [SourceEvidence],
}

fn t0_evidence_digest(
    instrument: &InstrumentId,
    requested_at: &str,
    quote: &Quote,
    order_book: &OrderBook,
    daily_bars: &[Bar],
    five_minute_bars: &[Bar],
    input_evidence: &[SourceEvidence],
) -> Result<String, ServiceError> {
    let normalized = serde_json::to_vec(&T0EvidenceDigestInput {
        instrument,
        requested_at,
        quote,
        order_book,
        daily_bars,
        five_minute_bars,
        input_evidence,
    })
    .map_err(|error| {
        ServiceError::Internal(format!("T0Evidence digest encoding failed: {error}"))
    })?;
    domain_separated_sha256(b"magic.t0_evidence.v2\0", &normalized)
}

#[derive(Serialize)]
struct OutcomeDailyBarsDigestInput<'a> {
    request: &'a OutcomeDailyBarsRequest,
    bars: &'a [Bar],
    input_evidence: &'a [SourceEvidence],
}

fn outcome_daily_bars_digest(
    request: &OutcomeDailyBarsRequest,
    bars: &[Bar],
    input_evidence: &[SourceEvidence],
) -> Result<String, ServiceError> {
    let normalized = serde_json::to_vec(&OutcomeDailyBarsDigestInput {
        request,
        bars,
        input_evidence,
    })
    .map_err(|error| {
        ServiceError::Internal(format!("OutcomeDailyBars digest encoding failed: {error}"))
    })?;
    domain_separated_sha256(b"magic.outcome_daily_bars.v1\0", &normalized)
}

fn classify_tencent_source_error(error: TencentError) -> SourceError {
    match error {
        TencentError::InvalidRequest(message) => {
            SourceError::stop(FailureKind::InvalidRequest, message)
        }
        TencentError::Unsupported(message) => SourceError::stop(FailureKind::Unsupported, message),
        TencentError::Transport(message) => SourceError::try_next(FailureKind::Transport, message),
        TencentError::Decode(message) | TencentError::Protocol(message) => {
            SourceError::stop(FailureKind::Protocol, message)
        }
        TencentError::Core(error) => SourceError::stop(FailureKind::Provider, error.to_string()),
    }
}

fn map_index_quote_router_error(error: RouterError) -> ServiceError {
    if let RouterError::InvalidConfiguration(message) = &error {
        return ServiceError::FailedPrecondition(message.clone());
    }
    let exhausted = matches!(&error, RouterError::Exhausted { .. });
    let attempts = error
        .attempts()
        .iter()
        .map(|attempt| {
            let provider = format!("{:?}", attempt.provider_id());
            match attempt.status() {
                AttemptStatus::Failed { kind, action, .. } => ProviderAttempt::new(
                    provider,
                    "failed",
                    route_failure_kind_code(*kind),
                    route_failure_kind_is_retryable(*kind),
                    matches!(action, magic_market_router::FailureAction::Stop),
                ),
                AttemptStatus::Rejected { kind, .. } => ProviderAttempt::new(
                    provider,
                    "rejected",
                    route_failure_kind_code(*kind),
                    false,
                    false,
                ),
                AttemptStatus::Selected => {
                    ProviderAttempt::new(provider, "selected", "selected", false, false)
                }
            }
        })
        .collect::<Result<Vec<_>, _>>();
    match attempts {
        Ok(attempts) if !attempts.is_empty() => ServiceError::ProviderRouteFailure {
            operation: Operation::IndexQuotes,
            exhausted,
            attempts,
        },
        Ok(_) => ServiceError::Internal("provider route failure has no attempts".to_owned()),
        Err(error) => error,
    }
}

const fn route_failure_kind_code(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::InvalidRequest => "invalid_request",
        FailureKind::Unsupported => "unsupported",
        FailureKind::Transport => "transport",
        FailureKind::Timeout => "timeout",
        FailureKind::RateLimited => "rate_limited",
        FailureKind::NoData => "no_data",
        FailureKind::Protocol => "protocol",
        FailureKind::Quality => "quality",
        FailureKind::Evidence => "evidence",
        FailureKind::Provider => "provider",
    }
}

const fn route_failure_kind_is_retryable(kind: FailureKind) -> bool {
    matches!(
        kind,
        FailureKind::Transport | FailureKind::Timeout | FailureKind::RateLimited
    )
}

fn execute_tencent_bars(
    client: &TencentClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: BarsRequest = decode_request(&command, HISTORICAL_BARS_REQUEST_SCHEMA)?;
    let batch = client
        .historical_bars(&request)
        .map_err(|error| map_tencent_error(Operation::HistoricalBars, error))?;
    provider_query_result(
        batch,
        TENCENT_PROVIDER,
        HISTORICAL_BARS_RECORD_SCHEMA,
        maximum_payload_bytes,
    )
}

fn execute_tencent_minute(
    client: &TencentClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: MinuteDataRequest = decode_request(&command, MINUTE_DATA_REQUEST_SCHEMA)?;
    let batch = client
        .minute_data(&request)
        .map_err(|error| map_tencent_error(Operation::MinuteData, error))?;
    provider_query_result(
        batch,
        TENCENT_PROVIDER,
        MINUTE_DATA_RECORD_SCHEMA,
        maximum_payload_bytes,
    )
}

fn execute_tencent_order_books(
    client: &TencentClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: InstrumentsRequest = decode_request(&command, ORDER_BOOKS_REQUEST_SCHEMA)?;
    let batch = client
        .order_books(&request.instruments)
        .map_err(|error| map_tencent_error(Operation::OrderBooks, error))?;
    provider_query_result(
        batch,
        TENCENT_PROVIDER,
        ORDER_BOOKS_RECORD_SCHEMA,
        maximum_payload_bytes,
    )
}

fn execute_tencent_trades(
    client: &TencentClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: TradesRequest = decode_request(&command, TRADES_REQUEST_SCHEMA)?;
    let batch = client
        .trades(&request)
        .map_err(|error| map_tencent_error(Operation::Trades, error))?;
    provider_query_result(
        batch,
        TENCENT_PROVIDER,
        TRADES_RECORD_SCHEMA,
        maximum_payload_bytes,
    )
}

fn execute_tencent_statistics(
    client: &TencentClient,
    command: QueryCommand,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: InstrumentsRequest = decode_request(&command, MARKET_STATISTICS_REQUEST_SCHEMA)?;
    let batch = client
        .market_statistics(&request.instruments)
        .map_err(|error| map_tencent_error(Operation::MarketStatistics, error))?;
    provider_query_result(
        batch,
        TENCENT_PROVIDER,
        MARKET_STATISTICS_RECORD_SCHEMA,
        maximum_payload_bytes,
    )
}

fn execute_typed<TRequest, TRecord, TError>(
    command: QueryCommand,
    request_schema: &str,
    record_schema: &str,
    provider: &str,
    maximum_payload_bytes: usize,
    call: impl FnOnce(&TRequest) -> Result<DataBatch<TRecord>, TError>,
) -> Result<QueryResult, ServiceError>
where
    TRequest: DeserializeOwned,
    TRecord: Serialize,
    TError: Error + 'static,
{
    let operation = command.operation();
    let request: TRequest = decode_request(&command, request_schema)?;
    let batch = call(&request).map_err(|error| provider_error(operation, error))?;
    provider_query_result(batch, provider, record_schema, maximum_payload_bytes)
}

fn execute_global_news<P>(
    command: QueryCommand,
    client: &P,
    provider: &str,
    expected_provider: ProviderId,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError>
where
    P: NewsProvider,
    P::Error: Error + 'static,
{
    let request: LimitRequest =
        decode_request_version(&command, GLOBAL_NEWS_REQUEST_SCHEMA, NEWS_SCHEMA_VERSION)?;
    let batch = client
        .global_news(request.limit)
        .map_err(|error| provider_error(Operation::GlobalNews, error))?;
    global_news_query_result(&batch, provider, expected_provider, maximum_payload_bytes)
}

fn execute_stcn_global_news(
    command: QueryCommand,
    client: &StcnClient,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: LimitRequest =
        decode_request_version(&command, GLOBAL_NEWS_REQUEST_SCHEMA, NEWS_SCHEMA_VERSION)?;
    let batch = client
        .probe_global_news(request.limit)
        .map_err(|error| provider_error(Operation::GlobalNews, error))?;
    global_news_query_result(
        &batch,
        "SecuritiesTimes",
        ProviderId::SecuritiesTimes,
        maximum_payload_bytes,
    )
}

fn execute_instrument_news(
    command: QueryCommand,
    client: &SinaClient,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let request: InstrumentNewsRequestV2 = decode_request_version(
        &command,
        INSTRUMENT_NEWS_REQUEST_SCHEMA,
        NEWS_SCHEMA_VERSION,
    )?;
    if request.limit.get() > 200 {
        return Err(ServiceError::InvalidRequest(
            "instrument-news limit must be at most 200".into(),
        ));
    }
    let captured_through =
        OffsetDateTime::parse(&request.captured_through, &Rfc3339).map_err(|_| {
            ServiceError::InvalidRequest("captured_through must be an RFC3339 instant".into())
        })?;
    let china_offset = UtcOffset::from_hms(8, 0, 0)
        .map_err(|error| ServiceError::Internal(format!("invalid China offset: {error}")))?;
    let cutoff_date = IsoDate::new(captured_through.to_offset(china_offset).date().to_string())
        .map_err(|error| ServiceError::Internal(error.to_string()))?;
    let provider_limit =
        PositiveU32::new(200).map_err(|error| ServiceError::Internal(error.to_string()))?;
    let mut provider_request = InstrumentDateRangeRequest::new(request.instrument, provider_limit)
        .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
    match (request.start, request.end) {
        (Some(start), Some(end)) => {
            if end != cutoff_date {
                return Err(ServiceError::InvalidRequest(format!(
                    "instrument-news end must equal the Asia/Shanghai date of captured_through ({cutoff_date})"
                )));
            }
            provider_request = provider_request
                .with_range(start, end)
                .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
        }
        (None, None) => {}
        _ => {
            return Err(ServiceError::InvalidRequest(
                "instrument-news start and end must be supplied together".into(),
            ));
        }
    }
    let batch = client
        .instrument_news(&provider_request)
        .map_err(|error| provider_error(Operation::InstrumentNews, error))?;
    instrument_news_query_result(
        batch,
        &request.captured_through,
        request.limit,
        maximum_payload_bytes,
    )
}

fn instrument_news_query_result(
    batch: DataBatch<NewsItem>,
    captured_through: &str,
    limit: PositiveU32,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    if batch.records().is_empty() {
        return source_proven_empty_instrument_news_result(&batch, captured_through);
    }
    // Validate the complete upstream batch before applying the caller cutoff,
    // so malformed evidence cannot be hidden by filtering.
    let mut validated =
        global_news_query_result(&batch, "Sina", ProviderId::Sina, maximum_payload_bytes)?;
    let filtered = filter_instrument_news_batch(batch, captured_through, limit)?;
    if filtered.records().is_empty() {
        validated.source_at = None;
        validated.records.clear();
        return Ok(validated);
    }
    global_news_query_result(&filtered, "Sina", ProviderId::Sina, maximum_payload_bytes)
}

fn source_proven_empty_instrument_news_result(
    batch: &DataBatch<NewsItem>,
    captured_through: &str,
) -> Result<QueryResult, ServiceError> {
    EvidenceTimestamp::parse_instant(captured_through).map_err(|_| {
        ServiceError::InvalidRequest("captured_through must be an RFC3339 instant".into())
    })?;
    if !batch.quality().is_complete() {
        return Err(invalid_news_evidence(
            "Sina",
            "batch_quality_incomplete",
            "quality",
            None,
        ));
    }
    let provenance = batch.provenance();
    if provenance.source() != "sina-company-news" {
        return Err(invalid_news_evidence(
            "Sina",
            "batch_source_mismatch",
            "source",
            None,
        ));
    }
    let batch_id = provenance.batch_id().ok_or_else(|| {
        invalid_news_evidence("Sina", "batch_evidence_incomplete", "batch_id", None)
    })?;
    if !batch_id.starts_with("sina-company-news:") {
        return Err(invalid_news_evidence(
            "Sina",
            "batch_identity_invalid",
            "batch_id",
            None,
        ));
    }
    let source_at = provenance.source_at().ok_or_else(|| {
        invalid_news_evidence("Sina", "batch_evidence_incomplete", "source_at", None)
    })?;
    let source_instant = EvidenceTimestamp::parse_instant(source_at)
        .map_err(|_| invalid_news_evidence("Sina", "batch_source_at_invalid", "source_at", None))?;
    let observed_instant =
        EvidenceTimestamp::parse_instant(provenance.fetched_at()).map_err(|_| {
            invalid_news_evidence("Sina", "batch_observed_at_invalid", "observed_at", None)
        })?;
    if source_instant > observed_instant {
        return Err(invalid_news_evidence(
            "Sina",
            "batch_source_after_observation",
            "source_at",
            None,
        ));
    }
    Ok(QueryResult {
        provider: "Sina".to_owned(),
        batch_id: batch_id.to_owned(),
        complete: true,
        observed_at: provenance.fetched_at().to_owned(),
        source_at: None,
        records: Vec::new(),
        repository_admitted: true,
        diagnostic_blocker: None,
    })
}

fn filter_instrument_news_batch(
    batch: DataBatch<NewsItem>,
    captured_through: &str,
    limit: PositiveU32,
) -> Result<DataBatch<NewsItem>, ServiceError> {
    let cutoff = EvidenceTimestamp::parse_instant(captured_through).map_err(|_| {
        ServiceError::InvalidRequest("captured_through must be an RFC3339 instant".into())
    })?;
    if !batch.quality().is_complete() {
        return Err(invalid_news_evidence(
            "Sina",
            "batch_quality_incomplete",
            "quality",
            None,
        ));
    }
    let provenance = batch.provenance();
    let source = provenance.source().to_owned();
    let observed_at = provenance.fetched_at().to_owned();
    let batch_id = provenance.batch_id().map(str::to_owned).ok_or_else(|| {
        invalid_news_evidence("Sina", "batch_evidence_incomplete", "batch_id", None)
    })?;
    let mut retained = Vec::new();
    for (index, record) in batch.into_records().into_iter().enumerate() {
        let published =
            EvidenceTimestamp::parse_instant(record.published_at.as_str()).map_err(|_| {
                invalid_news_evidence(
                    "Sina",
                    "record_published_at_invalid",
                    "published_at",
                    u32::try_from(index).ok(),
                )
            })?;
        if published <= cutoff {
            retained.push(record);
        }
    }
    retained.truncate(limit.get() as usize);
    let source_at = retained
        .first()
        .and_then(|record| record.evidence.source_at())
        .map(str::to_owned);
    let provenance = Provenance::new(source, observed_at)
        .and_then(|provenance| provenance.with_batch_id(batch_id))
        .map_err(|error| ServiceError::Internal(error.to_string()))?;
    let provenance = match source_at {
        Some(source_at) => provenance
            .with_source_at(source_at)
            .map_err(|error| ServiceError::Internal(error.to_string()))?,
        None => provenance,
    };
    Ok(DataBatch::strict(retained, provenance))
}

fn decode_request<T: DeserializeOwned>(
    command: &QueryCommand,
    required_schema: &str,
) -> Result<T, ServiceError> {
    decode_request_version(command, required_schema, SCHEMA_VERSION)
}

fn decode_request_version<T: DeserializeOwned>(
    command: &QueryCommand,
    required_schema: &str,
    required_version: u32,
) -> Result<T, ServiceError> {
    let payload = command.payload();
    if payload.schema() != required_schema || payload.schema_version() != required_version {
        return Err(ServiceError::InvalidRequest(format!(
            "{} requires schema {required_schema} version {required_version}",
            command.operation().as_str()
        )));
    }
    serde_json::from_slice(payload.data()).map_err(|error| {
        ServiceError::InvalidRequest(format!(
            "invalid {} request: {error}",
            command.operation().as_str()
        ))
    })
}

fn global_news_query_result(
    batch: &DataBatch<NewsItem>,
    provider: &str,
    expected_provider: ProviderId,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    if !batch.quality().is_complete() {
        return Err(invalid_news_evidence(
            provider,
            "batch_quality_incomplete",
            "quality",
            None,
        ));
    }
    if batch.records().is_empty() {
        return Err(invalid_news_evidence(
            provider,
            "batch_records_empty",
            "records",
            None,
        ));
    }
    let provenance = batch.provenance();
    let batch_id = provenance.batch_id().ok_or_else(|| {
        invalid_news_evidence(provider, "batch_evidence_incomplete", "batch_id", None)
    })?;
    let batch_source_at = provenance.source_at().ok_or_else(|| {
        invalid_news_evidence(provider, "batch_evidence_incomplete", "source_at", None)
    })?;
    parse_news_source_instant(expected_provider, batch_source_at).map_err(|_| {
        invalid_news_evidence(provider, "batch_source_at_invalid", "source_at", None)
    })?;
    let batch_observed_instant = EvidenceTimestamp::parse_instant(provenance.fetched_at())
        .map_err(|_| {
            invalid_news_evidence(provider, "batch_observed_at_invalid", "observed_at", None)
        })?;

    let mut previous = None;
    for (index, record) in batch.records().iter().enumerate() {
        let record_index = u32::try_from(index).ok();
        let evidence = &record.evidence;
        if evidence.provider() != expected_provider {
            return Err(invalid_news_evidence(
                provider,
                "record_provider_mismatch",
                "evidence.provider",
                record_index,
            ));
        }
        if evidence.batch_id() != batch_id {
            return Err(invalid_news_evidence(
                provider,
                "record_batch_mismatch",
                "evidence.batch_id",
                record_index,
            ));
        }
        let source_at = evidence.source_at().ok_or_else(|| {
            invalid_news_evidence(
                provider,
                "record_evidence_incomplete",
                "evidence.source_at",
                record_index,
            )
        })?;
        let source_instant =
            parse_news_source_instant(expected_provider, source_at).map_err(|_| {
                invalid_news_evidence(
                    provider,
                    "record_source_at_invalid",
                    "evidence.source_at",
                    record_index,
                )
            })?;
        let published_instant = EvidenceTimestamp::parse_instant(record.published_at.as_str())
            .map_err(|_| {
                invalid_news_evidence(
                    provider,
                    "record_published_at_invalid",
                    "published_at",
                    record_index,
                )
            })?;
        if source_instant != published_instant {
            return Err(invalid_news_evidence(
                provider,
                "record_published_at_mismatch",
                "published_at",
                record_index,
            ));
        }
        let observed_instant =
            EvidenceTimestamp::parse_instant(evidence.observed_at()).map_err(|_| {
                invalid_news_evidence(
                    provider,
                    "record_observed_at_invalid",
                    "evidence.observed_at",
                    record_index,
                )
            })?;
        if observed_instant > batch_observed_instant {
            return Err(invalid_news_evidence(
                provider,
                "record_observed_after_batch",
                "evidence.observed_at",
                record_index,
            ));
        }
        if source_instant > observed_instant {
            return Err(invalid_news_evidence(
                provider,
                "record_source_after_observation",
                "evidence.source_at",
                record_index,
            ));
        }
        if previous.is_some_and(|previous| source_instant > previous) {
            return Err(invalid_news_evidence(
                provider,
                "record_order_invalid",
                "records",
                record_index,
            ));
        }
        previous = Some(source_instant);
        if index == 0 && source_at != batch_source_at {
            return Err(invalid_news_evidence(
                provider,
                "batch_source_at_mismatch",
                "source_at",
                record_index,
            ));
        }
    }

    let records = batch
        .records()
        .iter()
        .map(|record| {
            let data = serde_json::to_vec(&NewsRecordPayloadV2::from(record)).map_err(|error| {
                ServiceError::Internal(format!("news record serialization failed: {error}"))
            })?;
            CanonicalPayload::new(
                GLOBAL_NEWS_RECORD_SCHEMA,
                NEWS_SCHEMA_VERSION,
                data,
                maximum_payload_bytes,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(QueryResult {
        provider: provider.to_owned(),
        batch_id: batch_id.to_owned(),
        complete: true,
        observed_at: provenance.fetched_at().to_owned(),
        source_at: Some(batch_source_at.to_owned()),
        records,
        repository_admitted: true,
        diagnostic_blocker: None,
    })
}

fn parse_news_source_instant(
    provider: ProviderId,
    source_at: &str,
) -> Result<EvidenceTimestamp, magic_market_core::CoreError> {
    let normalized = match provider {
        ProviderId::Eastmoney if source_at.len() == 16 && source_at.as_bytes()[10] == b' ' => {
            format!("{}T{}:00+08:00", &source_at[..10], &source_at[11..])
        }
        ProviderId::Jin10 | ProviderId::XinhuaFinance
            if source_at.len() == 19 && source_at.as_bytes()[10] == b' ' =>
        {
            format!("{}T{}+08:00", &source_at[..10], &source_at[11..])
        }
        ProviderId::Yicai if source_at.len() == 19 && source_at.as_bytes()[10] == b'T' => {
            format!("{source_at}+08:00")
        }
        _ => source_at.to_owned(),
    };
    EvidenceTimestamp::parse_instant(&normalized)
}

fn invalid_news_evidence(
    provider: &str,
    evidence_code: &str,
    evidence_field: &str,
    record_index: Option<u32>,
) -> ServiceError {
    ServiceError::InvalidEvidence {
        provider: provider.to_owned(),
        evidence_code: evidence_code.to_owned(),
        evidence_field: evidence_field.to_owned(),
        record_index,
        message: "news evidence is incomplete or inconsistent".to_owned(),
    }
}

fn provider_query_result<T: Serialize>(
    batch: DataBatch<T>,
    provider: &str,
    record_schema: &str,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let provenance = batch.provenance();
    let batch_id = provenance.batch_id().ok_or_else(|| {
        ServiceError::FailedPrecondition(format!("{provider} batch has no batch_id"))
    })?;
    let records = batch
        .records()
        .iter()
        .map(|quote| {
            let data = serde_json::to_vec(quote).map_err(|error| {
                ServiceError::Internal(format!("quote serialization failed: {error}"))
            })?;
            CanonicalPayload::new(record_schema, SCHEMA_VERSION, data, maximum_payload_bytes)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(QueryResult {
        provider: provider.to_owned(),
        batch_id: batch_id.to_owned(),
        complete: batch.quality().is_complete(),
        observed_at: provenance.fetched_at().to_owned(),
        source_at: provenance.source_at().map(str::to_owned),
        records,
        repository_admitted: true,
        diagnostic_blocker: None,
    })
}

fn provider_error(operation: Operation, error: impl Error + 'static) -> ServiceError {
    let source = &error as &(dyn Error + 'static);
    macro_rules! map_known {
        ($error_type:ty, $mapper:ident) => {
            if let Some(error) = source.downcast_ref::<$error_type>() {
                return $mapper(operation, error);
            }
        };
    }
    map_known!(BaiduError, map_baidu_error);
    map_known!(ClsError, map_cls_error);
    map_known!(SinaError, map_sina_error);
    map_known!(CfetsError, map_cfets_error);
    map_known!(CninfoError, map_cninfo_error);
    map_known!(EastmoneyError, map_eastmoney_error);
    map_known!(EmQuantError, map_emquant_error);
    map_known!(ExchangeError, map_exchange_error);
    map_known!(FredError, map_fred_error);
    map_known!(GovError, map_gov_error);
    map_known!(HithinkError, map_hithink_error);
    map_known!(IwencaiError, map_iwencai_error);
    map_known!(Jin10Error, map_jin10_error);
    map_known!(NbsError, map_nbs_error);
    map_known!(PbcError, map_pbc_error);
    map_known!(SecEdgarError, map_sec_error);
    map_known!(StcnError, map_stcn_error);
    map_known!(ThsError, map_ths_error);
    map_known!(ThePaperError, map_thepaper_error);
    map_known!(WallstreetCnError, map_wallstreetcn_error);
    map_known!(WorldBankError, map_worldbank_error);
    map_known!(XinhuaError, map_xinhua_error);
    map_known!(YicaiError, map_yicai_error);
    map_known!(TdxError, map_tdx_error);
    ServiceError::FailedPrecondition(format!(
        "{} provider request failed: {error}",
        operation.as_str()
    ))
}

fn map_baidu_error(operation: Operation, error: &BaiduError) -> ServiceError {
    match error {
        BaiduError::InvalidRequest(message) => invalid(message),
        BaiduError::Transport(_) => unavailable(operation, error),
        BaiduError::Unsupported(reason) => unsupported(operation, reason),
        BaiduError::Decode(_) | BaiduError::Protocol(_) | BaiduError::Core(_) => {
            precondition(error)
        }
    }
}

fn map_cls_error(operation: Operation, error: &ClsError) -> ServiceError {
    match error {
        ClsError::InvalidRequest(message) => invalid(message),
        ClsError::Unsupported(reason) => unsupported(operation, reason),
        ClsError::Transport(_) => cls_provider_failure(
            operation,
            ProviderFailureKind::Unavailable,
            "category=transport".into(),
        ),
        ClsError::HttpStatus(status) => {
            let kind = match status {
                401 | 403 => ProviderFailureKind::AuthenticationRejected,
                429 => ProviderFailureKind::RateLimited,
                500..=599 => ProviderFailureKind::Unavailable,
                _ => ProviderFailureKind::QueryRejected,
            };
            cls_provider_failure(operation, kind, format!("http_status={status}"))
        }
        ClsError::ProviderRejected { errno, message } => cls_provider_failure(
            operation,
            ProviderFailureKind::QueryRejected,
            format!("errno={errno} message={message}"),
        ),
        ClsError::Decode(message) => cls_provider_failure(
            operation,
            ProviderFailureKind::ResponseInvalid,
            format!("category=decode message={message}"),
        ),
        ClsError::Protocol(message) => cls_provider_failure(
            operation,
            ProviderFailureKind::ResponseInvalid,
            format!("category=protocol message={message}"),
        ),
        ClsError::Core(error) => cls_provider_failure(
            operation,
            ProviderFailureKind::ResponseInvalid,
            format!("category=core message={error}"),
        ),
    }
}

fn cls_provider_failure(
    operation: Operation,
    kind: ProviderFailureKind,
    provider_reason: String,
) -> ServiceError {
    ServiceError::ProviderFailure {
        operation,
        provider: "Cailianpress".into(),
        kind,
        provider_reason,
    }
}

fn map_emquant_error(operation: Operation, error: &EmQuantError) -> ServiceError {
    match error {
        EmQuantError::InvalidRequest(message) => invalid(message),
        EmQuantError::Bridge(_) => unavailable(operation, error),
        EmQuantError::Unsupported(reason) => unsupported(operation, reason),
        EmQuantError::InvalidResponse(_) | EmQuantError::Core(_) => precondition(error),
    }
}

fn map_nbs_error(operation: Operation, error: &NbsError) -> ServiceError {
    match error {
        NbsError::InvalidRequest(message) => invalid(message),
        NbsError::Transport(_) => unavailable(operation, error),
        NbsError::Unsupported(reason) => unsupported(operation, reason),
        NbsError::Decode(_) | NbsError::Protocol(_) | NbsError::Core(_) => precondition(error),
    }
}

fn map_pbc_error(operation: Operation, error: &PbcError) -> ServiceError {
    match error {
        PbcError::InvalidRequest(message) => invalid(message),
        PbcError::Transport(_) => unavailable(operation, error),
        PbcError::Unsupported(reason) => unsupported(operation, reason),
        PbcError::Decode(_) | PbcError::Protocol(_) | PbcError::Core(_) => precondition(error),
    }
}

fn map_stcn_error(operation: Operation, error: &StcnError) -> ServiceError {
    match error {
        StcnError::InvalidRequest(message) => invalid(message),
        StcnError::Transport(_) => unavailable(operation, error),
        StcnError::Unsupported(reason) => unsupported(operation, reason),
        StcnError::Decode(_) | StcnError::Protocol(_) | StcnError::Core(_) => precondition(error),
    }
}

fn map_thepaper_error(operation: Operation, error: &ThePaperError) -> ServiceError {
    match error {
        ThePaperError::InvalidRequest(message) => invalid(message),
        ThePaperError::Transport(_) => unavailable(operation, error),
        ThePaperError::Unsupported(reason) => unsupported(operation, reason),
        ThePaperError::Decode(_) | ThePaperError::Protocol(_) | ThePaperError::Core(_) => {
            precondition(error)
        }
    }
}

fn map_worldbank_error(operation: Operation, error: &WorldBankError) -> ServiceError {
    match error {
        WorldBankError::InvalidRequest(message) => invalid(message),
        WorldBankError::Authentication(_) | WorldBankError::Transport(_) => {
            unavailable(operation, error)
        }
        WorldBankError::Unsupported(reason) => unsupported(operation, reason),
        WorldBankError::Decode(_) | WorldBankError::Protocol(_) | WorldBankError::Core(_) => {
            precondition(error)
        }
    }
}

fn map_xinhua_error(operation: Operation, error: &XinhuaError) -> ServiceError {
    match error {
        XinhuaError::InvalidRequest(message) => invalid(message),
        XinhuaError::Transport(_) => unavailable(operation, error),
        XinhuaError::Unsupported(reason) => unsupported(operation, reason),
        XinhuaError::Decode(_) | XinhuaError::Protocol(_) | XinhuaError::Core(_) => {
            precondition(error)
        }
    }
}

fn map_yicai_error(operation: Operation, error: &YicaiError) -> ServiceError {
    match error {
        YicaiError::InvalidRequest(message) => invalid(message),
        YicaiError::Transport(_) => unavailable(operation, error),
        YicaiError::Unsupported(reason) => unsupported(operation, reason),
        YicaiError::Decode(_) | YicaiError::Protocol(_) | YicaiError::Core(_) => {
            precondition(error)
        }
    }
}

fn map_sina_error(operation: Operation, error: &SinaError) -> ServiceError {
    match error {
        SinaError::InvalidRequest(message) => ServiceError::InvalidRequest(message.clone()),
        SinaError::Transport(message) => ServiceError::Unavailable {
            operation,
            reason: message.clone(),
        },
        SinaError::Unsupported(reason) => ServiceError::Unsupported {
            operation,
            reason: reason.clone(),
        },
        SinaError::Decode(message) | SinaError::Protocol(message) => {
            ServiceError::FailedPrecondition(message.clone())
        }
        SinaError::Core(error) => ServiceError::FailedPrecondition(error.to_string()),
    }
}

fn invalid(message: impl ToString) -> ServiceError {
    ServiceError::InvalidRequest(message.to_string())
}

fn unavailable(operation: Operation, error: impl ToString) -> ServiceError {
    ServiceError::Unavailable {
        operation,
        reason: error.to_string(),
    }
}

fn unsupported(operation: Operation, reason: impl ToString) -> ServiceError {
    ServiceError::Unsupported {
        operation,
        reason: reason.to_string(),
    }
}

fn precondition(error: impl ToString) -> ServiceError {
    ServiceError::FailedPrecondition(error.to_string())
}

fn map_cfets_error(operation: Operation, error: &CfetsError) -> ServiceError {
    match error {
        CfetsError::InvalidRequest(message) => invalid(message),
        CfetsError::Transport(_) => unavailable(operation, error),
        CfetsError::Unsupported(reason) => unsupported(operation, reason),
        CfetsError::Decode(_) | CfetsError::Protocol(_) | CfetsError::Core(_) => {
            precondition(error)
        }
    }
}

fn map_cninfo_error(operation: Operation, error: &CninfoError) -> ServiceError {
    match error {
        CninfoError::InvalidRequest(message) => invalid(message),
        CninfoError::Unsupported(reason) => unsupported(operation, reason),
        CninfoError::Authentication(_) => ServiceError::PermissionDenied(error.to_string()),
        CninfoError::RateLimited => ServiceError::ResourceExhausted(error.to_string()),
        CninfoError::Transport(_) => unavailable(operation, error),
        CninfoError::HttpStatus(status) if *status >= 500 => unavailable(operation, error),
        CninfoError::HttpStatus(_)
        | CninfoError::Decode(_)
        | CninfoError::Schema(_)
        | CninfoError::Incomplete(_)
        | CninfoError::Core(_) => precondition(error),
    }
}

fn map_eastmoney_error(operation: Operation, error: &EastmoneyError) -> ServiceError {
    match error {
        EastmoneyError::InvalidRequest(message) => invalid(message),
        EastmoneyError::Authentication(_) => ServiceError::PermissionDenied(error.to_string()),
        EastmoneyError::Transport(_) => unavailable(operation, error),
        EastmoneyError::Unsupported(reason) => unsupported(operation, reason),
        EastmoneyError::ResponseTooLarge { .. }
        | EastmoneyError::Decode(_)
        | EastmoneyError::Protocol(_)
        | EastmoneyError::VerifiedEmpty(_)
        | EastmoneyError::Core(_) => precondition(error),
    }
}

fn map_exchange_error(operation: Operation, error: &ExchangeError) -> ServiceError {
    match error {
        ExchangeError::InvalidRequest(message) => invalid(message),
        ExchangeError::Unsupported(reason) => unsupported(operation, reason),
        ExchangeError::Authentication(_) => ServiceError::PermissionDenied(error.to_string()),
        ExchangeError::RateLimited => ServiceError::ResourceExhausted(error.to_string()),
        ExchangeError::Transport(_) | ExchangeError::Tls { .. } => unavailable(operation, error),
        ExchangeError::HttpStatus(status) if *status >= 500 => unavailable(operation, error),
        ExchangeError::HttpStatus(_)
        | ExchangeError::Decode(_)
        | ExchangeError::Schema(_)
        | ExchangeError::Incomplete(_)
        | ExchangeError::Core(_) => precondition(error),
    }
}

fn map_fred_error(operation: Operation, error: &FredError) -> ServiceError {
    match error {
        FredError::InvalidRequest(message) => invalid(message),
        FredError::Authentication(_) => ServiceError::PermissionDenied(error.to_string()),
        FredError::Transport(_) => unavailable(operation, error),
        FredError::Unsupported(reason) => unsupported(operation, reason),
        FredError::Decode(_) | FredError::Protocol(_) | FredError::Core(_) => precondition(error),
    }
}

fn map_gov_error(operation: Operation, error: &GovError) -> ServiceError {
    match error {
        GovError::InvalidRequest(message) => invalid(message),
        GovError::Transport(_) => unavailable(operation, error),
        GovError::Decode(_) | GovError::Protocol(_) | GovError::Core(_) => precondition(error),
    }
}

fn map_hithink_error(operation: Operation, error: &HithinkError) -> ServiceError {
    let (kind, provider_reason) = match error {
        HithinkError::InvalidRequest(message) => return invalid(message),
        HithinkError::Unsupported(reason) => return unsupported(operation, reason),
        HithinkError::Authentication { code, request_id } => (
            ProviderFailureKind::AuthenticationRejected,
            format!("code={code} request_id={request_id}"),
        ),
        HithinkError::RateLimited { request_id } => (
            ProviderFailureKind::RateLimited,
            format!("code=4001 request_id={request_id}"),
        ),
        HithinkError::Business { code, request_id } => {
            let kind = match code {
                1001..=1004 | 3001 | 3004 => ProviderFailureKind::QueryRejected,
                3002 | 5001..=5003 => ProviderFailureKind::Unavailable,
                _ => ProviderFailureKind::ResponseInvalid,
            };
            (kind, format!("code={code} request_id={request_id}"))
        }
        HithinkError::NotReady { request_id } => (
            ProviderFailureKind::Unavailable,
            format!("category=not_ready request_id={request_id}"),
        ),
        HithinkError::Transport(_) => (
            ProviderFailureKind::Unavailable,
            "category=transport".into(),
        ),
        HithinkError::Decode(_) => (
            ProviderFailureKind::ResponseInvalid,
            "category=decode".into(),
        ),
        HithinkError::Protocol(_) | HithinkError::Core(_) => (
            ProviderFailureKind::ResponseInvalid,
            "category=protocol".into(),
        ),
    };
    ServiceError::ProviderFailure {
        operation,
        provider: "HithinkFinance".into(),
        kind,
        provider_reason,
    }
}

fn map_iwencai_error(operation: Operation, error: &IwencaiError) -> ServiceError {
    match error {
        IwencaiError::InvalidRequest(message) => invalid(message),
        IwencaiError::Authentication(_) => ServiceError::PermissionDenied(error.to_string()),
        IwencaiError::Transport(_) => unavailable(operation, error),
        IwencaiError::Decode(_) | IwencaiError::Protocol(_) | IwencaiError::Core(_) => {
            precondition(error)
        }
    }
}

fn map_jin10_error(operation: Operation, error: &Jin10Error) -> ServiceError {
    match error {
        Jin10Error::InvalidRequest(message) => invalid(message),
        Jin10Error::Transport(_) => unavailable(operation, error),
        Jin10Error::Unsupported(reason) => unsupported(operation, reason),
        Jin10Error::Decode(_) | Jin10Error::Protocol(_) | Jin10Error::Core(_) => {
            precondition(error)
        }
    }
}

fn map_sec_error(operation: Operation, error: &SecEdgarError) -> ServiceError {
    match error {
        SecEdgarError::InvalidRequest(message) => invalid(message),
        SecEdgarError::Authentication(_) => ServiceError::PermissionDenied(error.to_string()),
        SecEdgarError::Transport(_) => unavailable(operation, error),
        SecEdgarError::Unsupported(reason) => unsupported(operation, reason),
        SecEdgarError::Decode(_) | SecEdgarError::Protocol(_) | SecEdgarError::Core(_) => {
            precondition(error)
        }
    }
}

fn map_ths_error(operation: Operation, error: &ThsError) -> ServiceError {
    match error {
        ThsError::InvalidRequest(message) => invalid(message),
        ThsError::Unsupported(reason) => unsupported(operation, reason),
        ThsError::Authentication(_) => ServiceError::PermissionDenied(error.to_string()),
        ThsError::RateLimited => ServiceError::ResourceExhausted(error.to_string()),
        ThsError::Transport(_) => unavailable(operation, error),
        ThsError::HttpStatus(status) if *status >= 500 => unavailable(operation, error),
        ThsError::Schema(message) | ThsError::Incomplete(message)
            if operation == Operation::Consensus =>
        {
            consensus_invalid_evidence(message)
        }
        ThsError::Core(error) if operation == Operation::Consensus => {
            consensus_invalid_evidence(&error.to_string())
        }
        ThsError::HttpStatus(_)
        | ThsError::Decode(_)
        | ThsError::Schema(_)
        | ThsError::Incomplete(_)
        | ThsError::VerifiedEmpty(_)
        | ThsError::ProbeAdmission(_)
        | ThsError::Core(_) => precondition(error),
    }
}

fn consensus_invalid_evidence(message: &str) -> ServiceError {
    let lower = message.to_ascii_lowercase();
    let (evidence_code, evidence_field) = if lower.contains("title")
        || lower.contains("requested code")
        || lower.contains("identity")
    {
        (
            "consensus_instrument_identity_invalid",
            "consensus.instrument_identity",
        )
    } else if lower.contains("fiscal") || lower.contains("year") {
        (
            "consensus_fiscal_year_invalid",
            "consensus.estimates.fiscal_year",
        )
    } else if lower.contains("contributor")
        || lower.contains("institution")
        || lower.contains("count")
    {
        (
            "consensus_contributor_count_invalid",
            "consensus.estimates.contributor_count",
        )
    } else if lower.contains("minimum") {
        ("consensus_minimum_invalid", "consensus.estimates.minimum")
    } else if lower.contains("maximum") {
        ("consensus_maximum_invalid", "consensus.estimates.maximum")
    } else if lower.contains("mean") {
        ("consensus_mean_invalid", "consensus.estimates.mean")
    } else if lower.contains("estimate values") {
        ("consensus_values_missing", "consensus.estimates.values")
    } else if lower.contains("table")
        || lower.contains("caption")
        || lower.contains("header")
        || lower.contains("row")
        || lower.contains("cell")
    {
        ("consensus_table_invalid", "consensus.estimates.table")
    } else {
        (
            "consensus_provider_response_invalid",
            "consensus.provider_response",
        )
    };
    ServiceError::InvalidEvidence {
        provider: "Tonghuashun".into(),
        evidence_code: evidence_code.into(),
        evidence_field: evidence_field.into(),
        record_index: None,
        message: format!(
            "Consensus rejected Tonghuashun evidence ({evidence_code} at {evidence_field})"
        ),
    }
}

fn map_wallstreetcn_error(operation: Operation, error: &WallstreetCnError) -> ServiceError {
    match error {
        WallstreetCnError::InvalidRequest(message) => invalid(message),
        WallstreetCnError::Transport(_) => unavailable(operation, error),
        WallstreetCnError::Unsupported(reason) => unsupported(operation, reason),
        WallstreetCnError::Decode(_)
        | WallstreetCnError::Protocol(_)
        | WallstreetCnError::Core(_) => precondition(error),
    }
}

fn map_tdx_error(operation: Operation, error: &TdxError) -> ServiceError {
    match error {
        TdxError::Io(_)
        | TdxError::Connection(_)
        | TdxError::ConnectionTimeout
        | TdxError::SetupFailed(_)
        | TdxError::Disconnected
        | TdxError::RetryExhausted(_) => unavailable(operation, error),
        TdxError::Coded(coded) if (2001..=2006).contains(&coded.code.code()) => {
            unavailable(operation, error)
        }
        TdxError::Unsupported(reason) => unsupported(operation, reason),
        _ => precondition(error),
    }
}

fn map_tencent_error(operation: Operation, error: TencentError) -> ServiceError {
    match error {
        TencentError::InvalidRequest(message) => ServiceError::InvalidRequest(message),
        TencentError::Transport(message) => ServiceError::Unavailable {
            operation,
            reason: message,
        },
        TencentError::Unsupported(reason) => ServiceError::Unsupported { operation, reason },
        TencentError::Decode(message) | TencentError::Protocol(message) => {
            ServiceError::FailedPrecondition(message)
        }
        TencentError::Core(error) => ServiceError::FailedPrecondition(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use magic_eastmoney_rs::EastmoneyTransport;
    use magic_market_core::{AssetClass, Quote, SecurityMetadata};
    use magic_market_service::QueryCommand;
    use magic_tencent_rs::SnapshotTransport;

    use super::*;

    fn news_record(
        id: &str,
        published_at: &str,
        source_at: Option<&str>,
        provider: ProviderId,
        observed_at: &str,
        batch_id: &str,
    ) -> NewsItem {
        let mut evidence = SourceEvidence::new(provider, observed_at, batch_id).unwrap();
        if let Some(source_at) = source_at {
            evidence = evidence.with_source_at(source_at).unwrap();
        }
        NewsItem {
            item_id: NonEmptyText::new(id).unwrap(),
            title: NonEmptyText::new(format!("news {id}")).unwrap(),
            summary: None,
            content: None,
            publisher: NonEmptyText::new("fixture").unwrap(),
            canonical_url: magic_market_core::HttpsUrl::new(format!("https://example.com/{id}"))
                .unwrap(),
            published_at: NonEmptyText::new(published_at).unwrap(),
            instruments: Vec::new(),
            topics: Vec::new(),
            language: NonEmptyText::new("zh-CN").unwrap(),
            evidence,
        }
    }

    fn news_batch(records: Vec<NewsItem>, source_at: &str, batch_id: &str) -> DataBatch<NewsItem> {
        DataBatch::strict(
            records,
            Provenance::new("jin10", "1787127606.533354000")
                .unwrap()
                .with_source_at(source_at)
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        )
    }

    #[test]
    fn global_news_v2_preserves_two_distinct_record_evidence_times() {
        let batch_id = "TEST_GLOBAL_NEWS_BATCH";
        let batch = news_batch(
            vec![
                news_record(
                    "NEWS_001",
                    "2026-08-19T16:15:37+08:00",
                    Some("2026-08-19 16:15:37"),
                    ProviderId::Jin10,
                    "1787127606.533354000",
                    batch_id,
                ),
                news_record(
                    "NEWS_002",
                    "2026-08-19T16:14:00+08:00",
                    Some("2026-08-19 16:14:00"),
                    ProviderId::Jin10,
                    "1787127605.000000000",
                    batch_id,
                ),
            ],
            "2026-08-19 16:15:37",
            batch_id,
        );
        let result =
            global_news_query_result(&batch, "Jin10", ProviderId::Jin10, 16 * 1024).unwrap();
        assert_eq!(result.source_at.as_deref(), Some("2026-08-19 16:15:37"));
        assert!(result
            .records
            .iter()
            .all(|record| record.schema_version() == 2));
        let rows = result
            .records
            .iter()
            .map(|record| serde_json::from_slice::<serde_json::Value>(record.data()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            rows[0]
                .pointer("/evidence/source_at")
                .and_then(|v| v.as_str()),
            Some("2026-08-19 16:15:37")
        );
        assert_eq!(
            rows[1]
                .pointer("/evidence/source_at")
                .and_then(|v| v.as_str()),
            Some("2026-08-19 16:14:00")
        );
    }

    #[test]
    fn global_news_v2_rejects_every_record_evidence_conflict_atomically() {
        let cases = [
            (
                news_record(
                    "bad-provider",
                    "2026-08-19T16:15:37+08:00",
                    Some("2026-08-19 16:15:37"),
                    ProviderId::Eastmoney,
                    "1787127606.533354000",
                    "batch",
                ),
                "record_provider_mismatch",
            ),
            (
                news_record(
                    "bad-batch",
                    "2026-08-19T16:15:37+08:00",
                    Some("2026-08-19 16:15:37"),
                    ProviderId::Jin10,
                    "1787127606.533354000",
                    "other-batch",
                ),
                "record_batch_mismatch",
            ),
            (
                news_record(
                    "bad-source",
                    "2026-08-19T16:14:00+08:00",
                    Some("2026-08-19 16:15:37"),
                    ProviderId::Jin10,
                    "1787127606.533354000",
                    "batch",
                ),
                "record_published_at_mismatch",
            ),
            (
                news_record(
                    "future-source",
                    "2026-08-19T16:30:00+08:00",
                    Some("2026-08-19 16:30:00"),
                    ProviderId::Jin10,
                    "1787127606.533354000",
                    "batch",
                ),
                "record_source_after_observation",
            ),
            (
                news_record(
                    "missing-source",
                    "2026-08-19T16:15:37+08:00",
                    None,
                    ProviderId::Jin10,
                    "1787127606.533354000",
                    "batch",
                ),
                "record_evidence_incomplete",
            ),
            (
                news_record(
                    "observed-after-batch",
                    "2026-08-19T16:15:37+08:00",
                    Some("2026-08-19 16:15:37"),
                    ProviderId::Jin10,
                    "1787127607.000000000",
                    "batch",
                ),
                "record_observed_after_batch",
            ),
        ];
        for (record, expected_code) in cases {
            let error = global_news_query_result(
                &news_batch(vec![record], "2026-08-19 16:15:37", "batch"),
                "Jin10",
                ProviderId::Jin10,
                16 * 1024,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                ServiceError::InvalidEvidence { evidence_code, .. }
                    if evidence_code == expected_code
            ));
        }

        let copied_batch_time = news_batch(
            vec![
                news_record(
                    "newest",
                    "2026-08-19T16:15:37+08:00",
                    Some("2026-08-19 16:15:37"),
                    ProviderId::Jin10,
                    "1787127606.533354000",
                    "batch",
                ),
                news_record(
                    "older-with-copied-batch-time",
                    "2026-08-19T16:14:00+08:00",
                    Some("2026-08-19 16:15:37"),
                    ProviderId::Jin10,
                    "1787127606.533354000",
                    "batch",
                ),
            ],
            "2026-08-19 16:15:37",
            "batch",
        );
        assert!(matches!(
            global_news_query_result(
                &copied_batch_time,
                "Jin10",
                ProviderId::Jin10,
                16 * 1024,
            ),
            Err(ServiceError::InvalidEvidence { evidence_code, .. })
                if evidence_code == "record_published_at_mismatch"
        ));
    }

    #[test]
    fn instrument_news_v2_filters_at_the_callers_exact_cutoff() {
        let batch_id = "TEST_INSTRUMENT_NEWS_BATCH";
        let batch = news_batch(
            vec![
                news_record(
                    "after-cutoff",
                    "2026-08-19T16:16:00+08:00",
                    Some("2026-08-19T16:16:00+08:00"),
                    ProviderId::Sina,
                    "1787127606.000000000",
                    batch_id,
                ),
                news_record(
                    "at-cutoff",
                    "2026-08-19T16:15:37+08:00",
                    Some("2026-08-19T16:15:37+08:00"),
                    ProviderId::Sina,
                    "1787127605.000000000",
                    batch_id,
                ),
                news_record(
                    "before-cutoff",
                    "2026-08-19T16:14:00+08:00",
                    Some("2026-08-19T16:14:00+08:00"),
                    ProviderId::Sina,
                    "1787127604.000000000",
                    batch_id,
                ),
            ],
            "2026-08-19T16:16:00+08:00",
            batch_id,
        );

        let filtered = filter_instrument_news_batch(
            batch,
            "2026-08-19T16:15:37+08:00",
            PositiveU32::new(2).unwrap(),
        )
        .unwrap();

        assert_eq!(filtered.records().len(), 2);
        assert_eq!(filtered.records()[0].item_id.as_str(), "at-cutoff");
        assert_eq!(
            filtered.provenance().source_at(),
            Some("2026-08-19T16:15:37+08:00")
        );
    }

    #[test]
    fn instrument_news_v2_preserves_a_truthful_cutoff_empty_batch() {
        let batch_id = "TEST_INSTRUMENT_NEWS_CUTOFF_EMPTY";
        let batch = news_batch(
            vec![
                news_record(
                    "after-cutoff-1",
                    "2026-08-19T16:16:00+08:00",
                    Some("2026-08-19T16:16:00+08:00"),
                    ProviderId::Sina,
                    "1787127606.000000000",
                    batch_id,
                ),
                news_record(
                    "after-cutoff-2",
                    "2026-08-19T16:15:38+08:00",
                    Some("2026-08-19T16:15:38+08:00"),
                    ProviderId::Sina,
                    "1787127605.000000000",
                    batch_id,
                ),
            ],
            "2026-08-19T16:16:00+08:00",
            batch_id,
        );

        let result = instrument_news_query_result(
            batch,
            "2026-08-19T16:15:37+08:00",
            PositiveU32::new(5).unwrap(),
            16 * 1024,
        )
        .expect("a valid cutoff-empty result must remain admitted");

        assert!(result.repository_admitted);
        assert!(result.complete);
        assert!(result.records.is_empty());
        assert_eq!(result.provider, "Sina");
        assert_eq!(result.batch_id, batch_id);
        assert_eq!(result.source_at, None);
        assert_eq!(result.observed_at, "1787127606.533354000");
        assert_eq!(result.diagnostic_blocker, None);
    }

    #[test]
    fn instrument_news_v2_cutoff_cannot_hide_invalid_upstream_evidence() {
        let batch_id = "TEST_INSTRUMENT_NEWS_INVALID_AFTER_CUTOFF";
        let batch = news_batch(
            vec![news_record(
                "invalid-after-cutoff",
                "2026-08-19T16:16:00+08:00",
                Some("2026-08-19T16:16:00+08:00"),
                ProviderId::Eastmoney,
                "1787127606.000000000",
                batch_id,
            )],
            "2026-08-19T16:16:00+08:00",
            batch_id,
        );

        assert!(matches!(
            instrument_news_query_result(
                batch,
                "2026-08-19T16:15:37+08:00",
                PositiveU32::new(5).unwrap(),
                16 * 1024,
            ),
            Err(ServiceError::InvalidEvidence {
                evidence_code,
                record_index: Some(0),
                ..
            }) if evidence_code == "record_provider_mismatch"
        ));
    }

    #[test]
    fn instrument_news_v2_preserves_a_source_proven_empty_range() {
        let batch_id = "sina-company-news:sh600000:1787127606.533354000:pages-1";
        let batch = DataBatch::strict(
            Vec::new(),
            Provenance::new("sina-company-news", "1787127606.533354000")
                .unwrap()
                .with_source_at("2026-08-19T16:16:00+08:00")
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        );

        let result = instrument_news_query_result(
            batch,
            "2026-08-19T16:15:37+08:00",
            PositiveU32::new(5).unwrap(),
            16 * 1024,
        )
        .expect("a source-proven empty date range must remain admitted");

        assert!(result.repository_admitted);
        assert!(result.complete);
        assert!(result.records.is_empty());
        assert_eq!(result.provider, "Sina");
        assert_eq!(result.batch_id, batch_id);
        assert_eq!(result.source_at, None);
        assert_eq!(result.observed_at, "1787127606.533354000");
    }

    #[test]
    fn instrument_news_v2_rejects_unproved_empty_ranges() {
        let cases = [
            (
                Provenance::new("other-source", "1787127606.533354000")
                    .unwrap()
                    .with_source_at("2026-08-19T16:16:00+08:00")
                    .unwrap()
                    .with_batch_id("sina-company-news:sh600000:1787127606.533354000:pages-1")
                    .unwrap(),
                "batch_source_mismatch",
            ),
            (
                Provenance::new("sina-company-news", "1787127606.533354000")
                    .unwrap()
                    .with_batch_id("sina-company-news:sh600000:1787127606.533354000:pages-1")
                    .unwrap(),
                "batch_evidence_incomplete",
            ),
            (
                Provenance::new("sina-company-news", "1787127606.533354000")
                    .unwrap()
                    .with_source_at("2026-08-19T16:16:00+08:00")
                    .unwrap()
                    .with_batch_id("foreign-batch")
                    .unwrap(),
                "batch_identity_invalid",
            ),
            (
                Provenance::new("sina-company-news", "2026-08-19T16:15:00+08:00")
                    .unwrap()
                    .with_source_at("2026-08-19T16:16:00+08:00")
                    .unwrap()
                    .with_batch_id("sina-company-news:sh600000:2026-08-19T16:15:00+08:00:pages-1")
                    .unwrap(),
                "batch_source_after_observation",
            ),
        ];

        for (provenance, expected_code) in cases {
            let batch = DataBatch::<NewsItem>::strict(Vec::new(), provenance);
            assert!(matches!(
                instrument_news_query_result(
                    batch,
                    "2026-08-19T16:15:37+08:00",
                    PositiveU32::new(5).unwrap(),
                    16 * 1024,
                ),
                Err(ServiceError::InvalidEvidence { evidence_code, .. })
                    if evidence_code == expected_code
            ));
        }
    }

    #[test]
    fn consensus_schema_failures_keep_safe_structured_field_diagnostics() {
        let cases = [
            (
                "consensus title code 000001 does not match requested 600000",
                "consensus_instrument_identity_invalid",
                "consensus.instrument_identity",
            ),
            (
                "invalid EPS fiscal year xyz",
                "consensus_fiscal_year_invalid",
                "consensus.estimates.fiscal_year",
            ),
            (
                "EPS mean is above maximum",
                "consensus_maximum_invalid",
                "consensus.estimates.maximum",
            ),
            (
                "EPS table has no header and data rows",
                "consensus_table_invalid",
                "consensus.estimates.table",
            ),
        ];
        for (message, expected_code, expected_field) in cases {
            let error = map_ths_error(Operation::Consensus, &ThsError::Schema(message.into()));
            assert!(matches!(
                error,
                ServiceError::InvalidEvidence {
                    provider,
                    evidence_code,
                    evidence_field,
                    record_index: None,
                    ..
                } if provider == "Tonghuashun"
                    && evidence_code == expected_code
                    && evidence_field == expected_field
            ));
        }
    }

    #[test]
    fn t0_observation_clock_is_explicit_local_china_time() {
        let observed_at = current_china_observed_at().unwrap();
        assert_eq!(observed_at.len(), 25);
        assert_eq!(observed_at.as_bytes().get(10), Some(&b'T'));
        assert!(observed_at.ends_with("+08:00"));
    }

    #[test]
    fn t0_external_decoder_rejects_v1_and_requires_v2_requested_at() {
        let data = serde_json::to_vec(&serde_json::json!({
            "instruments": [{
                "exchange": "Shanghai",
                "code": "600396",
                "asset_class": "Equity"
            }],
            "daily_bar_count": 20,
            "five_minute_bar_count": 20,
            "requested_at": "2026-08-27T09:24:10.123456+08:00"
        }))
        .unwrap();
        let command = |version| {
            QueryCommand::new(
                format!("t0-v{version}"),
                Operation::T0Evidence,
                Some("Tdx".to_owned()),
                CanonicalPayload::new(T0_EVIDENCE_REQUEST_SCHEMA, version, data.clone(), 4096)
                    .unwrap(),
            )
            .unwrap()
        };
        assert!(matches!(
            decode_request_version::<T0EvidenceRequest>(
                &command(1),
                T0_EVIDENCE_REQUEST_SCHEMA,
                T0_EVIDENCE_SCHEMA_VERSION,
            ),
            Err(ServiceError::InvalidRequest(message)) if message.contains("version 2")
        ));
        let request = decode_request_version::<T0EvidenceRequest>(
            &command(T0_EVIDENCE_SCHEMA_VERSION),
            T0_EVIDENCE_REQUEST_SCHEMA,
            T0_EVIDENCE_SCHEMA_VERSION,
        )
        .unwrap();
        assert_eq!(request.requested_at(), "2026-08-27T09:24:10.123456+08:00");
    }

    const QUOTE_RESPONSE: &str = "v_sh600396=\"1~ABC~600396~15.47~14.92~15.30~1775070~821130~950794~15.47~212~15.46~95~15.45~64~15.44~3~15.43~375~15.49~49~15.50~2721~15.51~241~15.52~450~15.53~86~~20260723094907~0.55~3.69~15.88~14.85~15.47/1775070/2729507908~1775070~272951~\";";
    const INDEX_QUOTE_RESPONSE: &str = "v_sh000001=\"1~Shanghai Composite~000001~3560.47~3544.15~3551.30~1775070~821130~950794~3560.47~212~3560.46~95~3560.45~64~3560.44~3~3560.43~375~3560.49~49~3560.50~2721~3560.51~241~3560.52~450~3560.53~86~~20260723094907~16.32~0.46~3568.88~3538.85~3560.47/1775070/2729507908~1775070~272951~\";";

    #[derive(Clone)]
    struct StaticTransport {
        calls: Arc<AtomicUsize>,
    }

    impl SnapshotTransport for StaticTransport {
        fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(QUOTE_RESPONSE.as_bytes().to_vec())
        }
    }

    #[derive(Clone)]
    struct IndexTransport {
        calls: Arc<AtomicUsize>,
    }

    impl SnapshotTransport for IndexTransport {
        fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(INDEX_QUOTE_RESPONSE.as_bytes().to_vec())
        }
    }

    #[derive(Clone)]
    struct FailingIndexTransport;

    impl SnapshotTransport for FailingIndexTransport {
        fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
            Err(TencentError::Transport("fixture TLS failure".to_owned()))
        }
    }

    #[derive(Clone)]
    struct ShapeTransport {
        calls: Arc<AtomicUsize>,
    }

    impl SnapshotTransport for ShapeTransport {
        fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(br#"{"code":0,"data":{"sh600396":{"data":[{"date":"20260723","data":["0930 10.00 10 100000.00","0931 10.20 20 204000.00","1300 10.10 30 303000.00"]}]}}}"#.to_vec())
        }
    }

    #[derive(Clone)]
    struct LimitPoolTransport {
        calls: Arc<AtomicUsize>,
    }

    impl EastmoneyTransport for LimitPoolTransport {
        fn get(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(br#"{"rc":0,"data":{"tc":1,"qdate":20260723,"pool":[{"c":"600396","m":1,"p":1308000,"zdp":9.97,"lbc":3}]}}"#.to_vec())
        }

        fn post_json(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _body: &[u8],
            _max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            Err(EastmoneyError::Protocol(
                "unexpected POST in limit-pool test".into(),
            ))
        }
    }

    fn command(schema: &str, provider: Option<&str>) -> QueryCommand {
        command_for(Operation::RealtimeQuotes, schema, provider)
    }

    fn command_for(operation: Operation, schema: &str, provider: Option<&str>) -> QueryCommand {
        let data = serde_json::to_vec(&serde_json::json!({
            "instruments": [{
                "exchange": "Shanghai",
                "code": "600396",
                "asset_class": "Equity"
            }]
        }))
        .unwrap();
        QueryCommand::new(
            "quote-1",
            operation,
            provider.map(str::to_owned),
            CanonicalPayload::new(schema, SCHEMA_VERSION, data, 4096).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn production_registry_is_exhaustive_and_only_enables_evidence_backed_operations() {
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let quote = registry
            .capabilities()
            .into_iter()
            .find(|capability| capability.operation == Operation::RealtimeQuotes)
            .unwrap();
        assert!(quote.repository_admitted);
        assert!(quote.runtime_available);
        assert_eq!(quote.provider, TENCENT_PROVIDER);
        let capabilities = registry.capabilities();
        let covered = capabilities
            .iter()
            .map(|capability| capability.operation)
            .collect::<BTreeSet<_>>();
        assert_eq!(covered.len(), magic_market_service::ALL_OPERATIONS.len());
        assert!(capabilities.iter().all(|capability| capability
            .blocker
            .as_deref()
            .is_none_or(|blocker| !blocker.contains("no evidence-backed production handler"))));
        let admitted = capabilities
            .iter()
            .filter(|capability| capability.repository_admitted)
            .map(|capability| capability.operation)
            .collect::<BTreeSet<_>>();
        assert_eq!(admitted.len(), 59);
        let blocked = magic_market_service::ALL_OPERATIONS
            .iter()
            .copied()
            .filter(|operation| !admitted.contains(operation))
            .collect::<Vec<_>>();
        assert_eq!(blocked, vec![Operation::EconomicCalendar]);
        let t0 = capabilities
            .iter()
            .find(|capability| capability.operation == Operation::T0Evidence)
            .unwrap();
        assert!(t0.repository_admitted);
        assert!(t0.runtime_available);
        assert!(!t0.diagnostic_available);
        assert!(matches!(
            registry.execute(command_for(Operation::T0Evidence, "wrong.schema", None)),
            Err(ServiceError::InvalidRequest(_))
        ));
        assert!(matches!(
            registry.execute(
                command_for(Operation::T0Evidence, "wrong.schema", Some("Tdx"))
                    .with_unadmitted_access(true)
            ),
            Err(ServiceError::InvalidRequest(_))
        ));
        let diagnostic = capabilities
            .iter()
            .filter(|capability| capability.diagnostic_available)
            .map(|capability| capability.operation)
            .collect::<BTreeSet<_>>();
        assert!(diagnostic.contains(&Operation::HistoricalBars));
        assert!(diagnostic.contains(&Operation::EconomicCalendar));
        for operation in [
            Operation::Auctions,
            Operation::FuturesDelivery,
            Operation::MarketRankings,
            Operation::MarketBreadth,
        ] {
            assert!(!diagnostic.contains(&operation));
        }
    }

    #[test]
    fn production_registry_exposes_every_new_provider_parity_registration() {
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let capabilities = registry.capabilities();
        let expected_admitted = [
            (Operation::GlobalNews, "Cls"),
            (Operation::GlobalNews, "Cailianpress"),
            (Operation::GlobalNews, "ThePaper"),
            (Operation::GlobalNews, "XinhuaFinance"),
            (Operation::GlobalNews, "Yicai"),
            (Operation::GlobalNews, "Yonhap"),
            (Operation::EconomicSeries, "Nbs"),
            (Operation::EconomicSeries, "Pbc"),
            (Operation::EconomicSeries, "WorldBank"),
            (Operation::RealtimeQuotes, "Sina"),
            (Operation::HistoricalBars, "Sina"),
            (Operation::MinuteData, "Sina"),
            (Operation::OrderBooks, "Sina"),
            (Operation::SecurityMetadata, "Sina"),
            (Operation::InstrumentNews, "Sina"),
            (Operation::IndexQuotes, "Tencent"),
            (Operation::IntradayShape, "LocalAnalysis"),
            (Operation::UpperLimitPoolReview, "Eastmoney"),
            (Operation::MarketRankings, "Eastmoney"),
            (Operation::FuturesDelivery, "Cffex"),
            (Operation::FundFlowSeries, "Eastmoney"),
            (Operation::MoneyFlows, "Eastmoney"),
            (Operation::PostCloseFlows, "Eastmoney"),
            (Operation::TechnicalBars, "Baidu"),
            (Operation::OutcomeDailyBars, "Tdx"),
            (Operation::T0Evidence, "Tdx"),
            (Operation::RealtimeQuotes, "Tdx"),
            (Operation::HistoricalBars, "Tdx"),
            (Operation::MinuteData, "Tdx"),
            (Operation::OrderBooks, "Tdx"),
            (Operation::Trades, "Tdx"),
            (Operation::SecurityMetadata, "Tdx"),
            (Operation::RealtimeQuotes, "Szse"),
            (Operation::OrderBooks, "Szse"),
            (Operation::Announcements, "Sse"),
            (Operation::Announcements, "Szse"),
            (Operation::DragonTiger, "Sse"),
            (Operation::DragonTiger, "Szse"),
        ];
        for (operation, provider) in expected_admitted {
            assert!(
                capabilities.iter().any(|capability| {
                    capability.operation == operation
                        && capability.provider == provider
                        && capability.repository_admitted
                        && capability.runtime_available
                        && !capability.diagnostic_available
                }),
                "missing admitted {provider} {} registration",
                operation.as_str()
            );
        }

        for operation in [Operation::Auctions, Operation::MarketBreadth] {
            let capability = capabilities
                .iter()
                .find(|capability| {
                    capability.operation == operation && capability.provider == "EastmoneyMiaoxiang"
                })
                .expect("missing admitted Miaoxiang registration");
            assert!(capability.repository_admitted);
            assert_eq!(
                capability.runtime_available,
                eastmoney_mx_key_is_configured()
            );
            assert!(!capability.diagnostic_available);
        }

        let emquant_bars = capabilities
            .iter()
            .find(|capability| {
                capability.operation == Operation::HistoricalBars
                    && capability.provider == "EmQuant"
            })
            .expect("missing EmQuant daily-bar registration");
        assert!(emquant_bars.repository_admitted);
        assert_eq!(
            emquant_bars.runtime_available,
            EmQuantClient::discover().is_ok()
        );
        assert!(!emquant_bars.diagnostic_available);

        for operation in [
            Operation::HistoricalBars,
            Operation::MarketStatistics,
            Operation::LimitPools,
            Operation::Popularity,
            Operation::FinancialStatements,
            Operation::CorporateActions,
            Operation::SecurityMetadata,
        ] {
            let hithink = capabilities
                .iter()
                .find(|capability| {
                    capability.operation == operation && capability.provider == "HithinkFinance"
                })
                .expect("missing official HITHINK Fuyao registration");
            assert!(hithink.repository_admitted);
            assert_eq!(hithink.runtime_available, hithink_key_is_configured());
            assert!(!hithink.diagnostic_available);
        }

        let hithink_auctions = capabilities
            .iter()
            .find(|capability| {
                capability.operation == Operation::Auctions
                    && capability.provider == "HithinkFinance"
            })
            .expect("missing official HITHINK Fuyao auction diagnostic registration");
        assert!(!hithink_auctions.repository_admitted);
        assert!(!hithink_auctions.runtime_available);
        assert_eq!(
            hithink_auctions.diagnostic_available,
            hithink_key_is_configured()
        );

        let unadmitted_with_operation_route = [
            (Operation::Auctions, "HithinkFinance", "EastmoneyMiaoxiang"),
            (Operation::EconomicSeries, "Imf", "WorldBank"),
            (Operation::FundFlowSeries, "EastmoneyMiaoxiang", "Eastmoney"),
            (Operation::HistoricalBars, "Baidu", "Tencent"),
            (Operation::MoneyFlows, "EastmoneyMiaoxiang", "Eastmoney"),
            (Operation::MoneyFlows, "EmQuant", "Eastmoney"),
            (Operation::OrderBooks, "EmQuant", "Tencent"),
            (Operation::RealtimeQuotes, "EmQuant", "Tencent"),
            (Operation::GlobalNews, "SecuritiesTimes", "Cls"),
        ];
        assert_eq!(
            capabilities
                .iter()
                .filter(|capability| !capability.repository_admitted)
                .count(),
            unadmitted_with_operation_route.len() + 1
        );
        for (operation, provider, admitted_operation_provider) in unadmitted_with_operation_route {
            assert!(
                capabilities.iter().any(|capability| {
                    capability.operation == operation
                        && capability.provider == provider
                        && !capability.repository_admitted
                }),
                "missing fail-closed {provider} {} registration",
                operation.as_str()
            );
            assert!(
                capabilities.iter().any(|capability| {
                    capability.operation == operation
                        && capability.provider == admitted_operation_provider
                        && capability.repository_admitted
                }),
                "missing explicit admitted operation route {admitted_operation_provider} for {}",
                operation.as_str()
            );
        }
    }

    #[test]
    fn jin10_economic_calendar_is_diagnostic_after_public_calendar_retirement() {
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let calendar = registry
            .capabilities()
            .into_iter()
            .find(|capability| {
                capability.operation == Operation::EconomicCalendar
                    && capability.provider == "Jin10"
            })
            .unwrap();

        assert!(!calendar.repository_admitted);
        assert!(!calendar.runtime_available);
        assert!(calendar.diagnostic_available);
        assert!(calendar
            .blocker
            .as_deref()
            .is_some_and(|blocker| blocker.contains("2025-12-01")));
    }

    #[test]
    fn securities_times_global_news_is_diagnostic_after_source_contract_drift() {
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let news = registry
            .capabilities()
            .into_iter()
            .find(|capability| {
                capability.operation == Operation::GlobalNews
                    && capability.provider == "SecuritiesTimes"
            })
            .unwrap();

        assert!(!news.repository_admitted);
        assert!(!news.runtime_available);
        assert!(news.diagnostic_available);
        assert!(news
            .blocker
            .as_deref()
            .is_some_and(|blocker| blocker.contains("source")));
    }

    #[test]
    fn emquant_production_scope_requires_daily_explicit_range() {
        let instrument = InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            "600396",
            AssetClass::Equity,
        )
        .unwrap();
        let valid = BarsRequest::new(instrument.clone(), BarInterval::Day, 5)
            .unwrap()
            .with_range("2026-08-14", "2026-08-20")
            .unwrap();
        assert!(validate_emquant_daily_bars_request(&valid).is_ok());

        let missing_range = BarsRequest::new(instrument.clone(), BarInterval::Day, 5).unwrap();
        assert!(validate_emquant_daily_bars_request(&missing_range).is_err());

        let weekly = BarsRequest::new(instrument.clone(), BarInterval::Week, 5)
            .unwrap()
            .with_range("2026-07-01", "2026-08-20")
            .unwrap();
        assert!(validate_emquant_daily_bars_request(&weekly).is_err());

        let oversized = BarsRequest::new(instrument, BarInterval::Day, 801)
            .unwrap()
            .with_range("2020-01-01", "2026-08-20")
            .unwrap();
        assert!(validate_emquant_daily_bars_request(&oversized).is_err());
    }

    #[test]
    fn quote_handler_returns_tencent_core_records() {
        let calls = Arc::new(AtomicUsize::new(0));
        let client = TencentClient::with_transport(StaticTransport {
            calls: calls.clone(),
        });
        let registry = registry_with_tencent(client, Duration::from_secs(1), 4096).unwrap();
        let result = registry
            .execute(command(REALTIME_QUOTES_REQUEST_SCHEMA, Some("Tencent")))
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(result.provider, TENCENT_PROVIDER);
        assert!(result.complete);
        assert_eq!(result.records.len(), 1);
        let quote: Quote = serde_json::from_slice(result.records[0].data()).unwrap();
        assert_eq!(quote.instrument().code(), "600396");
        assert_eq!(quote.provider(), magic_market_core::ProviderId::Tencent);
    }

    #[test]
    fn index_quotes_enforce_typed_identity_and_strict_freshness() {
        let calls = Arc::new(AtomicUsize::new(0));
        let client = TencentClient::with_transport(IndexTransport {
            calls: calls.clone(),
        });
        let registry = registry_with_tencent(client, Duration::from_secs(1), 4096).unwrap();
        let index_request = serde_json::to_vec(&serde_json::json!({
            "indices": [{
                "exchange": "Shanghai",
                "code": "000001",
                "asset_class": "Index"
            }],
            "maximum_source_age_millis": 315_576_000_000_u64
        }))
        .unwrap();
        let command = QueryCommand::new(
            "index-quotes-1",
            Operation::IndexQuotes,
            Some(TENCENT_PROVIDER.to_owned()),
            CanonicalPayload::new(
                INDEX_QUOTES_REQUEST_SCHEMA,
                SCHEMA_VERSION,
                index_request,
                4096,
            )
            .unwrap(),
        )
        .unwrap();
        let result = registry.execute(command).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(result.repository_admitted);
        assert_eq!(result.records.len(), 1);
        let quote: Quote = serde_json::from_slice(result.records[0].data()).unwrap();
        assert_eq!(quote.instrument().asset_class(), AssetClass::Index);

        let equity_request = serde_json::to_vec(&serde_json::json!({
            "indices": [{
                "exchange": "Shanghai",
                "code": "600396",
                "asset_class": "Equity"
            }],
            "maximum_source_age_millis": 5000
        }))
        .unwrap();
        let command = QueryCommand::new(
            "index-quotes-2",
            Operation::IndexQuotes,
            Some(TENCENT_PROVIDER.to_owned()),
            CanonicalPayload::new(
                INDEX_QUOTES_REQUEST_SCHEMA,
                SCHEMA_VERSION,
                equity_request,
                4096,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            registry.execute(command),
            Err(ServiceError::InvalidRequest(_))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn index_quote_route_failure_retains_its_safe_provider_attempt_trace() {
        let client = TencentClient::with_transport(FailingIndexTransport);
        let registry = registry_with_tencent(client, Duration::from_secs(1), 4096).unwrap();
        let payload = serde_json::to_vec(&serde_json::json!({
            "indices": [{
                "exchange": "Shanghai",
                "code": "000001",
                "asset_class": "Index"
            }],
            "maximum_source_age_millis": 5000
        }))
        .unwrap();
        let command = QueryCommand::new(
            "index-quotes-route-failure",
            Operation::IndexQuotes,
            Some(TENCENT_PROVIDER.to_owned()),
            CanonicalPayload::new(INDEX_QUOTES_REQUEST_SCHEMA, SCHEMA_VERSION, payload, 4096)
                .unwrap(),
        )
        .unwrap();
        let error = registry.execute(command).unwrap_err();
        let ServiceError::ProviderRouteFailure {
            operation,
            exhausted,
            attempts,
        } = error
        else {
            panic!("expected typed provider route failure");
        };
        assert_eq!(operation, Operation::IndexQuotes);
        assert!(exhausted);
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].provider(), "Tencent");
        assert_eq!(attempts[0].outcome(), "failed");
        assert_eq!(attempts[0].reason_code(), "transport");
        assert!(attempts[0].retryable());
        assert!(!attempts[0].terminal());
    }

    #[test]
    fn intraday_shape_uses_one_ordered_minute_series_and_deterministic_arithmetic() {
        let calls = Arc::new(AtomicUsize::new(0));
        let client = TencentClient::with_transport(ShapeTransport {
            calls: calls.clone(),
        });
        let registry = registry_with_tencent(client, Duration::from_secs(1), 65_536).unwrap();
        let request = IntradayShapeRequest::new(
            InstrumentId::new(
                magic_market_core::Exchange::Shanghai,
                "600396",
                AssetClass::Equity,
            )
            .unwrap(),
            Some(IsoDate::new("2026-07-23").unwrap()),
            PositiveU32::new(800).unwrap(),
        )
        .unwrap();
        let command = QueryCommand::new(
            "intraday-shape-1",
            Operation::IntradayShape,
            Some("LocalAnalysis".into()),
            CanonicalPayload::new(
                INTRADAY_SHAPE_REQUEST_SCHEMA,
                SCHEMA_VERSION,
                serde_json::to_vec(&request).unwrap(),
                65_536,
            )
            .unwrap(),
        )
        .unwrap();
        let result = registry.execute(command).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(result.repository_admitted);
        assert!(result.complete);
        assert_eq!(result.provider, "LocalAnalysis");
        let record: IntradayShapeRecord = serde_json::from_slice(result.records[0].data()).unwrap();
        assert_eq!(record.point_count().get(), 3);
        assert_eq!(record.trading_date().as_str(), "2026-07-23");
        assert_eq!(record.input_evidence().len(), 1);
    }

    #[test]
    fn upper_limit_pool_review_is_one_atomic_checked_four_family_record() {
        let calls = Arc::new(AtomicUsize::new(0));
        let client = EastmoneyClient::with_transport(LimitPoolTransport {
            calls: calls.clone(),
        });
        let request = UpperLimitPoolReviewRequest::new(
            IsoDate::new("2026-07-23").unwrap(),
            PositiveU32::new(10).unwrap(),
        )
        .unwrap();
        let command = QueryCommand::new(
            "upper-review-1",
            Operation::UpperLimitPoolReview,
            Some("Eastmoney".into()),
            CanonicalPayload::new(
                UPPER_LIMIT_POOL_REVIEW_REQUEST_SCHEMA,
                SCHEMA_VERSION,
                serde_json::to_vec(&request).unwrap(),
                65_536,
            )
            .unwrap(),
        )
        .unwrap();
        let result = execute_upper_limit_pool_review(&client, command, 65_536).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        assert!(result.repository_admitted);
        assert!(result.complete);
        assert_eq!(result.provider, "Eastmoney");
        assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
        assert_eq!(result.records.len(), 1);
        let record: UpperLimitPoolReviewRecord =
            serde_json::from_slice(result.records[0].data()).unwrap();
        assert_eq!(record.upper().len(), 1);
        assert_eq!(record.broken().len(), 1);
        assert_eq!(record.lower().len(), 1);
        assert_eq!(record.previous_upper().len(), 1);
        assert_eq!(record.maximum_streak(), Some(3));
        assert_eq!(record.input_evidence().len(), 4);
        assert_eq!(record.input_digest_sha256().len(), 64);
    }

    #[test]
    fn metadata_handler_preserves_explicit_incomplete_fields() {
        let calls = Arc::new(AtomicUsize::new(0));
        let client = TencentClient::with_transport(StaticTransport {
            calls: calls.clone(),
        });
        let registry = registry_with_tencent(client, Duration::from_secs(1), 4096).unwrap();
        let result = registry
            .execute(command_for(
                Operation::SecurityMetadata,
                SECURITY_METADATA_REQUEST_SCHEMA,
                Some("Tencent"),
            ))
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(!result.complete);
        assert_eq!(result.records.len(), 1);
        let metadata: SecurityMetadata = serde_json::from_slice(result.records[0].data()).unwrap();
        assert_eq!(metadata.name(), Some("ABC"));
        assert!(metadata.listed_on().is_none());
        assert!(metadata.price_limit().version().is_none());
    }

    #[test]
    fn security_profile_schema_rejects_before_tdx_io() {
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        assert!(matches!(
            registry.execute(command_for(
                Operation::SecurityProfiles,
                "wrong.schema",
                Some("Tdx"),
            )),
            Err(ServiceError::InvalidRequest(_))
        ));
    }

    #[test]
    fn coded_tdx_connectivity_errors_are_retryable_provider_outages() {
        for value in 2001..=2006 {
            let error = magic_tdx_rs::error_codes::ErrorCode::from_code(value)
                .unwrap()
                .err("transport fixture");
            assert!(
                matches!(
                    map_tdx_error(Operation::SecurityProfiles, &error),
                    ServiceError::Unavailable {
                        operation: Operation::SecurityProfiles,
                        ..
                    }
                ),
                "TDX connectivity error E{value} must remain retryable"
            );
        }
    }

    #[test]
    fn diagnostics_require_opt_in_and_exact_schema_while_absent_families_stay_blocked() {
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let technical = command_for(Operation::TechnicalBars, "wrong.schema", Some("Baidu"));
        assert!(matches!(
            registry.execute(technical),
            Err(ServiceError::InvalidRequest(_))
        ));

        let auction = command_for(Operation::Auctions, "wrong.schema", Some("Tdx"))
            .with_unadmitted_access(true);
        assert!(matches!(
            registry.execute(auction),
            Err(ServiceError::Unsupported { .. })
        ));
    }

    #[test]
    fn wrong_schema_and_provider_fail_before_provider_io() {
        let calls = Arc::new(AtomicUsize::new(0));
        let client = TencentClient::with_transport(StaticTransport {
            calls: calls.clone(),
        });
        let registry = registry_with_tencent(client, Duration::from_secs(1), 4096).unwrap();
        assert!(matches!(
            registry.execute(command("wrong.schema", None)),
            Err(ServiceError::InvalidRequest(_))
        ));
        assert!(matches!(
            registry.execute(command(
                REALTIME_QUOTES_REQUEST_SCHEMA,
                Some("NotRegistered")
            )),
            Err(ServiceError::Unsupported { .. })
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn provider_failures_preserve_retry_and_precondition_categories() {
        assert!(matches!(
            provider_error(
                Operation::MarketStatistics,
                HithinkError::Business {
                    code: 3002,
                    request_id: "hithink-request".into()
                }
            ),
            ServiceError::ProviderFailure {
                provider,
                kind: ProviderFailureKind::Unavailable,
                provider_reason,
                ..
            } if provider == "HithinkFinance"
                && provider_reason == "code=3002 request_id=hithink-request"
        ));
        assert!(matches!(
            provider_error(
                Operation::Popularity,
                HithinkError::Authentication {
                    code: 2003,
                    request_id: "hithink-auth".into()
                }
            ),
            ServiceError::ProviderFailure {
                kind: ProviderFailureKind::AuthenticationRejected,
                provider_reason,
                ..
            } if provider_reason == "code=2003 request_id=hithink-auth"
        ));
        assert!(matches!(
            provider_error(
                Operation::Auctions,
                HithinkError::NotReady {
                    request_id: "hithink-not-ready".into()
                }
            ),
            ServiceError::ProviderFailure {
                kind: ProviderFailureKind::Unavailable,
                provider_reason,
                ..
            } if provider_reason == "category=not_ready request_id=hithink-not-ready"
        ));
        assert!(matches!(
            provider_error(
                Operation::HistoricalBars,
                EmQuantError::Bridge("10001004 EQERR_ACCESS_EXPIRE".into())
            ),
            ServiceError::Unavailable {
                operation: Operation::HistoricalBars,
                ..
            }
        ));
        assert!(matches!(
            provider_error(
                Operation::GlobalIndices,
                SinaError::Transport("temporary TLS EOF".into())
            ),
            ServiceError::Unavailable {
                operation: Operation::GlobalIndices,
                ..
            }
        ));
        assert!(matches!(
            provider_error(
                Operation::GlobalNews,
                EastmoneyError::Protocol("unexpected article host".into())
            ),
            ServiceError::FailedPrecondition(_)
        ));
        assert!(matches!(
            provider_error(Operation::BoardDirectory, TdxError::ConnectionTimeout),
            ServiceError::Unavailable {
                operation: Operation::BoardDirectory,
                ..
            }
        ));
        assert!(matches!(
            provider_error(
                Operation::GlobalNews,
                ClsError::ProviderRejected {
                    errno: 1001,
                    message: "bad sign".into(),
                }
            ),
            ServiceError::ProviderFailure {
                operation: Operation::GlobalNews,
                provider,
                kind: ProviderFailureKind::QueryRejected,
                provider_reason,
            } if provider == "Cailianpress"
                && provider_reason == "errno=1001 message=bad sign"
        ));
        assert!(matches!(
            provider_error(Operation::GlobalNews, ClsError::HttpStatus(429)),
            ServiceError::ProviderFailure {
                provider,
                kind: ProviderFailureKind::RateLimited,
                provider_reason,
                ..
            } if provider == "Cailianpress" && provider_reason == "http_status=429"
        ));
        assert!(matches!(
            provider_error(
                Operation::GlobalNews,
                ClsError::Protocol("telegraph row identity changed".into())
            ),
            ServiceError::ProviderFailure {
                provider,
                kind: ProviderFailureKind::ResponseInvalid,
                provider_reason,
                ..
            } if provider == "Cailianpress"
                && provider_reason == "category=protocol message=telegraph row identity changed"
        ));
    }
}
