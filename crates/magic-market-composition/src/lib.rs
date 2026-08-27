#![forbid(unsafe_code)]
//! Concrete provider-to-Router bindings.
//!
//! Core and Router remain provider-neutral. This crate is the explicit
//! composition boundary where a route may require a concrete provider type so
//! downstream wrappers cannot impersonate an admitted source.

mod derived_products;
mod eastmoney_provider_top_n_rankings;
mod grpc_production;
mod local_terminal_monitor;

pub use derived_products::{
    DerivedProductContractError, IndexQuotesRequest, IntradayShapeRecord, IntradayShapeRequest,
    OutcomeDailyBarsRecord, OutcomeDailyBarsRequest, T0EvidenceRecord, T0EvidenceRequest,
    UpperLimitPoolReviewRecord, UpperLimitPoolReviewRequest,
};
pub use eastmoney_provider_top_n_rankings::{
    EastmoneyProviderTopNRankingRouter, EastmoneyProviderTopNRouterError,
};
pub use grpc_production::{
    production_operation_registry, ProductionRegistryError, HISTORICAL_BARS_RECORD_SCHEMA,
    HISTORICAL_BARS_REQUEST_SCHEMA, INDEX_QUOTES_RECORD_SCHEMA, INDEX_QUOTES_REQUEST_SCHEMA,
    INTRADAY_SHAPE_RECORD_SCHEMA, INTRADAY_SHAPE_REQUEST_SCHEMA, MARKET_STATISTICS_RECORD_SCHEMA,
    MARKET_STATISTICS_REQUEST_SCHEMA, MINUTE_DATA_RECORD_SCHEMA, MINUTE_DATA_REQUEST_SCHEMA,
    ORDER_BOOKS_RECORD_SCHEMA, ORDER_BOOKS_REQUEST_SCHEMA, OUTCOME_DAILY_BARS_RECORD_SCHEMA,
    OUTCOME_DAILY_BARS_REQUEST_SCHEMA, REALTIME_QUOTES_RECORD_SCHEMA,
    REALTIME_QUOTES_REQUEST_SCHEMA, SCHEMA_VERSION, T0_EVIDENCE_RECORD_SCHEMA,
    T0_EVIDENCE_REQUEST_SCHEMA, T0_EVIDENCE_SCHEMA_VERSION, TRADES_RECORD_SCHEMA,
    TRADES_REQUEST_SCHEMA, UPPER_LIMIT_POOL_REVIEW_RECORD_SCHEMA,
    UPPER_LIMIT_POOL_REVIEW_REQUEST_SCHEMA,
};
pub use local_terminal_monitor::{
    DiagnosticLocalTerminalMonitorComposition, LocalMonitorCapability,
    LocalTerminalMonitorComposition, LocalTerminalMonitorCompositionError,
};
