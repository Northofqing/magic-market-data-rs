use std::{env, error::Error, sync::Arc, time::Duration};

use magic_cfets_rs::{CfetsClient, CfetsError};
use magic_cninfo_rs::{CninfoClient, CninfoError};
use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError};
use magic_exchange_rs::{ExchangeError, HkexClient};
use magic_fred_rs::{FredClient, FredError};
use magic_gov_rs::{GovClient, GovError};
use magic_iwencai_rs::{IwencaiClient, IwencaiError, SEMANTIC_SEARCH_ADMITTED};
use magic_jin10_rs::{Jin10Client, Jin10Error};
use magic_market_core::{
    Announcements, BarsRequest, BlockTrades, BoardCategory, BoardConstituentProvider,
    BoardConstituentRequest, BoardDirectoryProvider, BoardDirectoryRequest, BoardFlows,
    BoardMembershipProvider, CompanyFilingRequest, CompanyFilingsProvider, ConceptHits,
    ConsensusData, ContractMonth, CorporateActionRequest, CorporateActions, DataBatch,
    DividendPlans, DragonTigerData, DragonTigerDiscovery, DragonTigerDiscoveryRequest,
    EconomicCalendarProvider, EconomicCalendarRequest, EconomicSeriesProvider,
    EconomicSeriesRequest, FinancialStatements, FlowInterval, ForeignExchangeProvider, FxRequest,
    GlobalIndexProvider, GlobalIndexRequest, HistoricalBars, HolderCounts,
    InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest, InvestorQuestions,
    LimitPoolRequest, LimitPools, LockupEvents, MarginData, MarketAnnouncementRequest,
    MarketAnnouncements, MarketDragonTigerData, MarketDragonTigerRequest, MarketStatisticsProvider,
    MinuteData, MinuteDataRequest, NewsProvider, NonEmptyText, NorthboundDailyRequest,
    NorthboundDailyStatistics, OfficialFxFixingProvider, OfficialFxFixingRequest, OptionData,
    OrderBooks, PolicyDocuments, PolicyRequest, PopularityData, PositiveU32,
    ProviderTopNRankingRequest, ProviderTopNRankings, RealtimeQuotes, ReferenceRateProvider,
    ReferenceRateRequest, ResearchDocumentRequest, ResearchDocuments, ResearchReports,
    ResearchRequest, SecurityMetadataProvider, SecurityProfiles, SemanticSearch,
    SemanticSearchRequest, StatementKind, StrongStockReasons, TargetPriceData, TargetPriceRequest,
    Trades, TradesRequest,
};
use magic_market_service::{
    CanonicalPayload, Capability, Operation, OperationRegistry, QueryCommand, QueryResult,
    ServiceError,
};
use magic_sec_rs::{SecEdgarClient, SecEdgarError};
use magic_sina_rs::{SinaClient, SinaError};
use magic_tdx_rs::{
    BlockService, TdxBoardProvider, TdxError, TdxSecurityProfileProvider, TdxSmartClient,
};
use magic_tencent_rs::{TencentClient, TencentError};
use magic_ths_rs::{ThsClient, ThsError};
use magic_wallstreetcn_rs::{WallstreetCnClient, WallstreetCnError};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

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
        },
        move |command| execute_tencent_quotes(&quotes, command, maximum_payload_bytes),
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
    registry.register_handler(
        admitted(
            Operation::FinancialStatements,
            "Sina",
            "bounded Shanghai/Shenzhen equity income, balance-sheet or cash-flow statements",
        ),
        move |command| {
            let request: FinancialStatementsRequest =
                decode_request(&command, FINANCIAL_STATEMENTS_REQUEST_SCHEMA)?;
            let batch = sina
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
    register_exact_blockers(registry)?;
    Ok(())
}

fn register_exact_blockers(registry: &mut OperationRegistry) -> Result<(), ServiceError> {
    for capability in [
        blocked(
            Operation::MoneyFlows,
            "Tdx",
            "normalized instrument money-flow records",
            "TDX public capability is false; LocalTerminal cumulative amount is not a money-flow contract and remains unadmitted",
        ),
        blocked(
            Operation::Auctions,
            "Tdx",
            "normalized call-auction records",
            "TDX public auction capability is false and no admitted production auction provider is configured",
        ),
        blocked(
            Operation::FuturesDelivery,
            "Cffex",
            "official CFFEX delivery calendar",
            "CFFEX remains diagnostic-only because the admitted TLS transport has not completed live evidence",
        ),
        blocked(
            Operation::TechnicalBars,
            "Baidu",
            "technical bars with calendar and corporate-action continuity",
            "Baidu technical bars remain diagnostic and repository-unadmitted",
        ),
        blocked(
            Operation::FundFlowSeries,
            "Eastmoney",
            "bounded instrument fund-flow time series",
            "Eastmoney fund-flow series capability is explicitly false after incomplete live evidence",
        ),
        blocked(
            Operation::PostCloseFlows,
            "Eastmoney",
            "strict same-snapshot post-close whole-market flows",
            "source timestamps were inconsistent across the fetched market, so production admission is false",
        ),
        blocked(
            Operation::MarketRankings,
            "Eastmoney",
            "complete-market volume-ratio or main-net-inflow rankings",
            "full-market coverage and source-time atomicity have not passed admission",
        ),
        blocked(
            Operation::MarketBreadth,
            "LocalAnalysis",
            "derived complete-market breadth snapshot",
            "no admitted complete-market source composition is registered for breadth analysis",
        ),
    ] {
        registry.register_unavailable(capability)?;
    }
    Ok(())
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

fn register_tdx_public(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    let timeout_seconds = provider_timeout.as_secs_f64();
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

    let client = Arc::new(TdxSmartClient::new());
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
    map_known!(SinaError, map_sina_error);
    map_known!(CfetsError, map_cfets_error);
    map_known!(CninfoError, map_cninfo_error);
    map_known!(EastmoneyError, map_eastmoney_error);
    map_known!(ExchangeError, map_exchange_error);
    map_known!(FredError, map_fred_error);
    map_known!(GovError, map_gov_error);
    map_known!(IwencaiError, map_iwencai_error);
    map_known!(Jin10Error, map_jin10_error);
    map_known!(SecEdgarError, map_sec_error);
    map_known!(ThsError, map_ths_error);
    map_known!(WallstreetCnError, map_wallstreetcn_error);
    map_known!(TdxError, map_tdx_error);
    ServiceError::FailedPrecondition(format!(
        "{} provider request failed: {error}",
        operation.as_str()
    ))
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

    use magic_market_core::{Quote, SecurityMetadata};
    use magic_market_service::QueryCommand;
    use magic_tencent_rs::SnapshotTransport;

    use super::*;

    const QUOTE_RESPONSE: &str = "v_sh600396=\"1~ABC~600396~15.47~14.92~15.30~1775070~821130~950794~15.47~212~15.46~95~15.45~64~15.44~3~15.43~375~15.49~49~15.50~2721~15.51~241~15.52~450~15.53~86~~20260723094907~0.55~3.69~15.88~14.85~15.47/1775070/2729507908~1775070~272951~\";";

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
            .into_iter()
            .filter(|capability| capability.repository_admitted)
            .map(|capability| capability.operation)
            .collect::<BTreeSet<_>>();
        assert_eq!(admitted.len(), 46);
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
            ]
        );
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
            registry.execute(command(REALTIME_QUOTES_REQUEST_SCHEMA, Some("Tdx"))),
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
