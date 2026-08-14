#![forbid(unsafe_code)]
//! Concrete provider-to-Router bindings.
//!
//! Core and Router remain provider-neutral. This crate is the explicit
//! composition boundary where a route may require a concrete provider type so
//! downstream wrappers cannot impersonate an admitted source.

mod eastmoney_provider_top_n_rankings;
mod grpc_production;
mod local_terminal_monitor;

pub use eastmoney_provider_top_n_rankings::{
    EastmoneyProviderTopNRankingRouter, EastmoneyProviderTopNRouterError,
};
pub use grpc_production::{
    production_operation_registry, ProductionRegistryError, HISTORICAL_BARS_RECORD_SCHEMA,
    HISTORICAL_BARS_REQUEST_SCHEMA, MARKET_STATISTICS_RECORD_SCHEMA,
    MARKET_STATISTICS_REQUEST_SCHEMA, MINUTE_DATA_RECORD_SCHEMA, MINUTE_DATA_REQUEST_SCHEMA,
    ORDER_BOOKS_RECORD_SCHEMA, ORDER_BOOKS_REQUEST_SCHEMA, REALTIME_QUOTES_RECORD_SCHEMA,
    REALTIME_QUOTES_REQUEST_SCHEMA, SCHEMA_VERSION, TRADES_RECORD_SCHEMA, TRADES_REQUEST_SCHEMA,
};
pub use local_terminal_monitor::{
    DiagnosticLocalTerminalMonitorComposition, LocalMonitorCapability,
    LocalTerminalMonitorComposition, LocalTerminalMonitorCompositionError,
};
