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
use magic_emquant_rs::{EmQuantClient, EmQuantError};
use magic_exchange_rs::{
    CffexClient, CffexConfig, ExchangeError, HkexClient, SseClient, SseConfig, SzseClient,
    SzseConfig,
};
use magic_fred_rs::{FredClient, FredError};
use magic_gov_rs::{GovClient, GovError};
use magic_iwencai_rs::{IwencaiClient, IwencaiError, SEMANTIC_SEARCH_ADMITTED};
use magic_jin10_rs::{Jin10Client, Jin10Error};
use magic_market_core::{
    Announcements, Bar, BarInterval, BarsRequest, BlockTrades, BoardCategory,
    BoardConstituentProvider, BoardConstituentRequest, BoardDirectoryProvider,
    BoardDirectoryRequest, BoardFlows, BoardMembershipProvider, CompanyFilingRequest,
    CompanyFilingsProvider, ConceptHits, ConsensusData, ContractMonth, CorporateActionRequest,
    CorporateActions, DataBatch, DataStatus, DividendPlans, DragonTigerData, DragonTigerDiscovery,
    DragonTigerDiscoveryRequest, EconomicCalendarProvider, EconomicCalendarRequest,
    EconomicSeriesProvider, EconomicSeriesRequest, EvidenceTimestamp, FinancialStatements,
    FlowInterval, FlowScope, ForeignExchangeProvider, FundFlowPoint, FundFlowRequest,
    FundFlowSeries, FuturesDeliveryRequest, FxRequest, GlobalIndexProvider, GlobalIndexRequest,
    HistoricalBars, HolderCounts, InstrumentDateRangeRequest, InstrumentId,
    InstrumentSignalRequest, InvestorQuestions, IsoDate, LimitPoolRequest, LimitPools,
    LockupEvents, MarginData, MarketAnnouncementRequest, MarketAnnouncements,
    MarketDragonTigerData, MarketDragonTigerRequest, MarketRankingKind, MarketStatisticsProvider,
    MinuteData, MinuteDataRequest, MinutePoint, MoneyFlow, MoneyFlows, NewsProvider, NonEmptyText,
    NorthboundDailyRequest, NorthboundDailyStatistics, OfficialFxFixingProvider,
    OfficialFxFixingRequest, OptionData, OrderBook, OrderBooks, PolicyDocuments, PolicyRequest,
    PopularityData, PositiveU32, PostCloseFlowRequest, ProviderId, ProviderTopNRankingRequest,
    ProviderTopNRankings, Quote, RealtimeQuotes, ReferenceRateProvider, ReferenceRateRequest,
    ResearchDocumentRequest, ResearchDocuments, ResearchReports, ResearchRequest,
    SecurityMetadataProvider, SecurityProfiles, SemanticSearch, SemanticSearchRequest,
    SourceEvidence, StatementKind, StrongStockReasons, TargetPriceData, TargetPriceRequest,
    TechnicalBarsProvider, Trades, TradesRequest,
};
use magic_market_router::{
    quote_source, AcceptancePolicy, AttemptStatus, FailureKind, QuoteRouter, RouterError,
    SourceError,
};
use magic_market_service::{
    CanonicalPayload, Capability, Operation, OperationRegistry, QueryCommand, QueryResult,
    ServiceError,
};
use magic_nbs_rs::{NbsClient, NbsError};
use magic_pbc_rs::{PbcClient, PbcError};
use magic_sec_rs::{SecEdgarClient, SecEdgarError};
use magic_sina_rs::{SinaClient, SinaError};
use magic_stcn_rs::{StcnClient, StcnError};
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
use time::{OffsetDateTime, UtcOffset};

pub const SCHEMA_VERSION: u32 = 1;
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
pub const POST_CLOSE_FLOWS_RECORD_SCHEMA: &str = "magic.market.post_close_flow_diagnostic";
pub const MARKET_RANKINGS_REQUEST_SCHEMA: &str = "magic.market.market_rankings.request";
pub const MARKET_RANKINGS_RECORD_SCHEMA: &str = "magic.market.market_ranking_diagnostic_entry";
pub const AUCTIONS_REQUEST_SCHEMA: &str = "magic.market.auctions.request";
pub const AUCTIONS_RECORD_SCHEMA: &str = "magic.market.opening_auction_diagnostic";
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
            let request: LimitRequest = decode_request(&command, GLOBAL_NEWS_REQUEST_SCHEMA)?;
            let batch = news
                .global_news(request.limit)
                .map_err(|error| provider_error(Operation::GlobalNews, error))?;
            provider_query_result(
                batch,
                "WallstreetCn",
                GLOBAL_NEWS_RECORD_SCHEMA,
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
    register_diagnostic_handlers(registry, provider_timeout, maximum_payload_bytes)?;
    register_exact_blockers(registry)?;
    Ok(())
}

fn register_global_news_parity(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    register_global_news_provider(
        registry,
        ClsClient::with_timeout(provider_timeout)?,
        "Cls",
        "bounded public CLS financial-news metadata",
        maximum_payload_bytes,
    )?;
    register_global_news_provider(
        registry,
        ThePaperClient::with_timeout(provider_timeout)?,
        "ThePaper",
        "bounded native The Paper finance-channel article metadata",
        maximum_payload_bytes,
    )?;
    register_global_news_provider(
        registry,
        XinhuaClient::with_timeout(provider_timeout)?,
        "XinhuaFinance",
        "bounded first-party Xinhua Finance front-page metadata",
        maximum_payload_bytes,
    )?;
    register_global_news_provider(
        registry,
        YicaiClient::with_timeout(provider_timeout)?,
        "Yicai",
        "bounded first-party Yicai first-page metadata",
        maximum_payload_bytes,
    )?;
    register_global_news_provider(
        registry,
        StcnClient::with_timeout(provider_timeout)?,
        "SecuritiesTimes",
        "bounded first-party Securities Times front-page metadata",
        maximum_payload_bytes,
    )?;
    register_global_news_provider(
        registry,
        YonhapClient::for_channel_with_timeout(YonhapChannel::Economy, provider_timeout)?,
        "Yonhap",
        "bounded official Yonhap Economy RSS metadata only",
        maximum_payload_bytes,
    )?;
    Ok(())
}

fn register_global_news_provider<P>(
    registry: &mut OperationRegistry,
    client: P,
    provider: &'static str,
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
            let request: LimitRequest = decode_request(&command, GLOBAL_NEWS_REQUEST_SCHEMA)?;
            let batch = client
                .global_news(request.limit)
                .map_err(|error| provider_error(Operation::GlobalNews, error))?;
            provider_query_result(
                batch,
                provider,
                GLOBAL_NEWS_RECORD_SCHEMA,
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
        move |command| {
            execute_typed(
                command,
                INSTRUMENT_NEWS_REQUEST_SCHEMA,
                GLOBAL_NEWS_RECORD_SCHEMA,
                "Sina",
                maximum_payload_bytes,
                |request: &InstrumentDateRangeRequest| client.instrument_news(request),
            )
        },
    )?;
    Ok(())
}

fn register_exact_blockers(registry: &mut OperationRegistry) -> Result<(), ServiceError> {
    for capability in [
        blocked(
            Operation::Auctions,
            "Tdx",
            "normalized call-auction records",
            "TDX public auction capability is false and no admitted production auction provider is configured",
        ),
        blocked(
            Operation::MarketBreadth,
            "LocalAnalysis",
            "derived complete-market breadth snapshot",
            "no admitted complete-market source composition is registered for breadth analysis",
        ),
        blocked(
            Operation::EconomicSeries,
            "Imf",
            "annual IMF economic-series adapter",
            "IMF DataMapper returns HTTP 403 and the replacement SDMX contract requires beta-portal authentication",
        ),
    ] {
        registry.register_unavailable(capability)?;
    }
    Ok(())
}

fn register_diagnostic_handlers(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let baidu = BaiduClient::with_timeout(provider_timeout)?;
    let baidu_bars = baidu.clone();
    registry.register_diagnostic_handler(
        blocked(
            Operation::TechnicalBars,
            "Baidu",
            "one A-share equity; bounded unadjusted daily OHLCV/amount and optional source MA5/10/20",
            "trading-calendar, adjacent-session and corporate-action continuity evidence remain unproved",
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
        registry.register_default_diagnostic_handler(
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
        registry.register_default_diagnostic_handler(
            blocked(
                Operation::MoneyFlows,
                "EastmoneyMiaoxiang",
                "one Shanghai/Shenzhen equity; latest bounded daily main/super-large/large/medium/small net flow in CNY",
                "source methodology and serial live stability remain repository-unadmitted",
            ),
            move |command| execute_mx_money_flow(&money_flows, command, maximum_payload_bytes),
        )?;

        let auctions = mx.clone();
        registry.register_default_diagnostic_handler(
            blocked(
                Operation::Auctions,
                "EastmoneyMiaoxiang",
                "one equity and exact source date; opening-auction matched volume in shares and amount in CNY",
                "matched price, previous close, unmatched bid/ask, volume ratio and provider time remain unavailable",
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
        registry.register_default_diagnostic_handler(
            blocked(
                Operation::MarketBreadth,
                "EastmoneyMiaoxiang",
                "exact source date; all-A up/down/flat and limit-up/limit-down counts",
                "listed universe total, coverage and source-time skew remain unavailable",
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
        let flow_series = eastmoney.clone();
        registry.register_diagnostic_handler(
            blocked(
                Operation::FundFlowSeries,
                "Eastmoney",
                "one Shanghai/Shenzhen equity; bounded one-minute or daily source fund-flow series with absent values retained as null",
                "serial live stability admission remains incomplete",
            ),
            move |command| {
                execute_typed(
                    command,
                    FUND_FLOW_SERIES_REQUEST_SCHEMA,
                    FUND_FLOW_SERIES_RECORD_SCHEMA,
                    "Eastmoney",
                    maximum_payload_bytes,
                    |request: &FundFlowRequest| flow_series.fund_flow_series(request),
                )
            },
        )?;

        let money_flows = eastmoney.clone();
        registry.register_diagnostic_handler(
            blocked(
                Operation::MoneyFlows,
                "Eastmoney",
                "one Shanghai/Shenzhen equity; latest bounded daily source fund-flow point mapped without using TDX turnover",
                "source methodology and serial live stability remain repository-unadmitted",
            ),
            move |command| {
                execute_eastmoney_money_flow(&money_flows, command, maximum_payload_bytes)
            },
        )?;
    }

    let post_close = eastmoney.clone();
    registry.register_diagnostic_handler(
        blocked(
            Operation::PostCloseFlows,
            "Eastmoney",
            "bounded current-day post-close source ranking with per-record source evidence",
            "mixed source timestamps remain non-atomic and production admission is false",
        ),
        move |command| {
            let request: PostCloseFlowRequest =
                decode_request(&command, POST_CLOSE_FLOWS_REQUEST_SCHEMA)?;
            let batch = post_close
                .diagnose_partial_post_close_flows(&request)
                .map_err(|error| map_eastmoney_error(Operation::PostCloseFlows, &error))?;
            provider_query_result(
                batch,
                "Eastmoney",
                POST_CLOSE_FLOWS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    registry.register_diagnostic_handler(
        blocked(
            Operation::MarketRankings,
            "Eastmoney",
            "first bounded A-share source ranking page; available fields are returned and missing fields remain null",
            "complete-market coverage and source-time atomicity are not claimed",
        ),
        move |command| {
            let request: MarketRankingsRequest =
                decode_request(&command, MARKET_RANKINGS_REQUEST_SCHEMA)?;
            let batch = eastmoney
                .diagnose_partial_market_rankings(&request.kind, request.limit)
                .map_err(|error| map_eastmoney_error(Operation::MarketRankings, &error))?;
            provider_query_result(
                batch,
                "Eastmoney",
                MARKET_RANKINGS_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;

    let cffex_config = CffexConfig {
        timeout: provider_timeout,
        ..CffexConfig::default()
    };
    let cffex = CffexClient::with_config(cffex_config)?;
    registry.register_diagnostic_handler(
        blocked(
            Operation::FuturesDelivery,
            "Cffex",
            "official CFFEX equity-index futures delivery notice diagnostic",
            "official TLS/live evidence remains incomplete and delivery method can be NotProvided",
        ),
        move |command| {
            let request: FuturesDeliveryRequest =
                decode_request(&command, FUTURES_DELIVERY_REQUEST_SCHEMA)?;
            let batch = cffex
                .probe_futures_delivery_calendar(&request)
                .map_err(|error| provider_error(Operation::FuturesDelivery, error))?;
            provider_query_result(
                batch,
                "Cffex",
                FUTURES_DELIVERY_RECORD_SCHEMA,
                maximum_payload_bytes,
            )
        },
    )?;
    register_emquant_diagnostics(registry, provider_timeout, maximum_payload_bytes)?;
    Ok(())
}

fn register_emquant_diagnostics(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let client = match EmQuantClient::discover()
        .and_then(|client| client.with_timeout(provider_timeout))
    {
        Ok(client) => Arc::new(client),
        Err(error) => {
            let blocker = format!(
                "EMQuant read-only bridge is not runtime-discoverable or admitted: {error}"
            );
            for (operation, scope) in [
                (
                    Operation::RealtimeQuotes,
                    "runtime-entitled EMQuant quote snapshot diagnostic",
                ),
                (
                    Operation::HistoricalBars,
                    "runtime-entitled EMQuant daily or intraday bar diagnostic",
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

    let bars = client.clone();
    registry.register_diagnostic_handler(
        blocked(
            Operation::HistoricalBars,
            "EmQuant",
            "runtime-entitled EMQuant daily or intraday bar diagnostic",
            "EMQuant bridge availability and product entitlement do not constitute repository admission",
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

fn eastmoney_mx_key_is_configured() -> bool {
    ["EASTMONEY_API_KEY", "MX_APIKEY"]
        .iter()
        .any(|name| env::var_os(name).is_some_and(|value| !value.is_empty()))
}

fn execute_eastmoney_money_flow(
    client: &EastmoneyClient,
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
        ServiceError::FailedPrecondition(
            "Eastmoney fund-flow diagnostic returned no latest point".to_owned(),
        )
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
    let jin10 = Jin10Client::with_timeout(provider_timeout)?;
    let calendar = jin10.clone();
    registry.register_handler(
        admitted(
            Operation::EconomicCalendar,
            "Jin10",
            "bounded public economic-release calendar; an empty eligible set remains a typed source failure",
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
    registry.register_handler(
        admitted(
            Operation::GlobalNews,
            "Jin10",
            "bounded public flash-news metadata",
        ),
        move |command| {
            let request: LimitRequest = decode_request(&command, GLOBAL_NEWS_REQUEST_SCHEMA)?;
            let batch = jin10
                .global_news(request.limit)
                .map_err(|error| provider_error(Operation::GlobalNews, error))?;
            provider_query_result(
                batch,
                "Jin10",
                GLOBAL_NEWS_RECORD_SCHEMA,
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
    registry.register_diagnostic_handler(
        blocked(
            Operation::T0Evidence,
            "Tdx",
            "bounded TDX-only quote, book, daily-bar and five-minute-bar evidence bundle",
            "TDX public quote and order-book packets do not expose an admitted source timestamp; the exact available fields are diagnostic-only",
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
            let request: LimitRequest = decode_request(&command, GLOBAL_NEWS_REQUEST_SCHEMA)?;
            let batch = news
                .global_news(request.limit)
                .map_err(|error| provider_error(Operation::GlobalNews, error))?;
            provider_query_result(
                batch,
                "Eastmoney",
                GLOBAL_NEWS_RECORD_SCHEMA,
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
    let request: T0EvidenceRequest = decode_request(&command, T0_EVIDENCE_REQUEST_SCHEMA)?;
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
            &quote,
            &order_book,
            &daily_bars,
            &five_minute_bars,
            &input_evidence,
        )?;
        let record = T0EvidenceRecord::new(
            instrument.clone(),
            quote,
            order_book,
            daily_bars,
            five_minute_bars,
            request.daily_bar_count(),
            request.five_minute_bar_count(),
            input_evidence,
            PositiveU32::new(1).map_err(|error| ServiceError::Internal(error.to_string()))?,
            digest,
        )
        .map_err(|error| ServiceError::FailedPrecondition(error.to_string()))?;
        payloads.push(CanonicalPayload::new(
            T0_EVIDENCE_RECORD_SCHEMA,
            SCHEMA_VERSION,
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
        b"magic.t0_evidence.batch.v1\0",
        &serde_json::to_vec(&records).map_err(|error| {
            ServiceError::Internal(format!("T0Evidence batch digest encoding failed: {error}"))
        })?,
    )?;
    let observed_at = latest_observed_at(&all_evidence, "T0Evidence")?;
    Ok(QueryResult {
        provider: "Tdx".to_owned(),
        batch_id: format!("tdx:t0-evidence:{aggregate_digest}"),
        complete: false,
        observed_at,
        source_at: common_source_at(&all_evidence),
        records: payloads,
        repository_admitted: false,
        diagnostic_blocker: Some(
            "TDX public quote and order-book packets do not expose an admitted source timestamp"
                .to_owned(),
        ),
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

fn latest_observed_at(evidence: &[SourceEvidence], family: &str) -> Result<String, ServiceError> {
    evidence
        .iter()
        .map(SourceEvidence::observed_at)
        .max()
        .map(str::to_owned)
        .ok_or_else(|| ServiceError::Internal(format!("{family} has no input evidence")))
}

#[derive(Serialize)]
struct T0EvidenceDigestInput<'a> {
    instrument: &'a InstrumentId,
    quote: &'a Quote,
    order_book: &'a OrderBook,
    daily_bars: &'a [Bar],
    five_minute_bars: &'a [Bar],
    input_evidence: &'a [SourceEvidence],
}

fn t0_evidence_digest(
    instrument: &InstrumentId,
    quote: &Quote,
    order_book: &OrderBook,
    daily_bars: &[Bar],
    five_minute_bars: &[Bar],
    input_evidence: &[SourceEvidence],
) -> Result<String, ServiceError> {
    let normalized = serde_json::to_vec(&T0EvidenceDigestInput {
        instrument,
        quote,
        order_book,
        daily_bars,
        five_minute_bars,
        input_evidence,
    })
    .map_err(|error| {
        ServiceError::Internal(format!("T0Evidence digest encoding failed: {error}"))
    })?;
    domain_separated_sha256(b"magic.t0_evidence.v1\0", &normalized)
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
    let Some(attempt) = error.attempts().last() else {
        return ServiceError::FailedPrecondition(error.to_string());
    };
    match attempt.status() {
        AttemptStatus::Failed { kind, message, .. } => match kind {
            FailureKind::InvalidRequest => ServiceError::InvalidRequest(message.clone()),
            FailureKind::Unsupported => ServiceError::Unsupported {
                operation: Operation::IndexQuotes,
                reason: message.clone(),
            },
            FailureKind::Transport | FailureKind::Timeout | FailureKind::RateLimited => {
                ServiceError::Unavailable {
                    operation: Operation::IndexQuotes,
                    reason: message.clone(),
                }
            }
            _ => ServiceError::FailedPrecondition(message.clone()),
        },
        AttemptStatus::Rejected { message, .. } => {
            ServiceError::FailedPrecondition(message.clone())
        }
        AttemptStatus::Selected => ServiceError::FailedPrecondition(
            "IndexQuotes routing failed after a selected attempt".into(),
        ),
    }
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

fn decode_request<T: DeserializeOwned>(
    command: &QueryCommand,
    required_schema: &str,
) -> Result<T, ServiceError> {
    let payload = command.payload();
    if payload.schema() != required_schema || payload.schema_version() != SCHEMA_VERSION {
        return Err(ServiceError::InvalidRequest(format!(
            "{} requires schema {required_schema} version {SCHEMA_VERSION}",
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
        ClsError::Transport(_) => unavailable(operation, error),
        ClsError::Unsupported(reason) => unsupported(operation, reason),
        ClsError::Decode(_) | ClsError::Protocol(_) | ClsError::Core(_) => precondition(error),
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
        ThsError::HttpStatus(_)
        | ThsError::Decode(_)
        | ThsError::Schema(_)
        | ThsError::Incomplete(_)
        | ThsError::VerifiedEmpty(_)
        | ThsError::ProbeAdmission(_)
        | ThsError::Core(_) => precondition(error),
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
        assert_eq!(admitted.len(), 51);
        let blocked = magic_market_service::ALL_OPERATIONS
            .iter()
            .copied()
            .filter(|operation| !admitted.contains(operation))
            .collect::<Vec<_>>();
        assert_eq!(
            blocked,
            vec![
                Operation::MoneyFlows,
                Operation::Auctions,
                Operation::FuturesDelivery,
                Operation::TechnicalBars,
                Operation::FundFlowSeries,
                Operation::PostCloseFlows,
                Operation::MarketRankings,
                Operation::MarketBreadth,
                Operation::T0Evidence,
            ]
        );
        let t0 = capabilities
            .iter()
            .find(|capability| capability.operation == Operation::T0Evidence)
            .unwrap();
        assert!(!t0.repository_admitted);
        assert!(!t0.runtime_available);
        assert!(t0.diagnostic_available);
        assert!(matches!(
            registry.execute(command_for(Operation::T0Evidence, "wrong.schema", None)),
            Err(ServiceError::Unsupported { .. })
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
        for operation in [
            Operation::MoneyFlows,
            Operation::FuturesDelivery,
            Operation::TechnicalBars,
            Operation::HistoricalBars,
            Operation::FundFlowSeries,
            Operation::PostCloseFlows,
            Operation::MarketRankings,
            Operation::T0Evidence,
        ] {
            assert!(diagnostic.contains(&operation));
        }
        assert!(capabilities
            .iter()
            .filter(|capability| capability.provider == "Tdx"
                && capability.operation == Operation::Auctions)
            .chain(capabilities.iter().filter(|capability| {
                capability.provider == "LocalAnalysis"
                    && capability.operation == Operation::MarketBreadth
            }))
            .all(|capability| !capability.diagnostic_available));
    }

    #[test]
    fn production_registry_exposes_every_new_provider_parity_registration() {
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let capabilities = registry.capabilities();
        let expected_admitted = [
            (Operation::GlobalNews, "Cls"),
            (Operation::GlobalNews, "ThePaper"),
            (Operation::GlobalNews, "XinhuaFinance"),
            (Operation::GlobalNews, "Yicai"),
            (Operation::GlobalNews, "SecuritiesTimes"),
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
            (Operation::OutcomeDailyBars, "Tdx"),
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

        for (operation, provider) in [
            (Operation::HistoricalBars, "Baidu"),
            (Operation::RealtimeQuotes, "EmQuant"),
            (Operation::HistoricalBars, "EmQuant"),
            (Operation::OrderBooks, "EmQuant"),
            (Operation::MoneyFlows, "EmQuant"),
            (Operation::EconomicSeries, "Imf"),
        ] {
            assert!(
                capabilities.iter().any(|capability| {
                    capability.operation == operation
                        && capability.provider == provider
                        && !capability.repository_admitted
                }),
                "missing fail-closed {provider} {} registration",
                operation.as_str()
            );
        }
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
    fn diagnostics_require_opt_in_and_exact_schema_while_absent_families_stay_blocked() {
        let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
        let technical = command_for(Operation::TechnicalBars, "wrong.schema", Some("Baidu"));
        assert!(matches!(
            registry.execute(technical.clone()),
            Err(ServiceError::Unsupported { .. })
        ));
        assert!(matches!(
            registry.execute(technical.with_unadmitted_access(true)),
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
    }
}
