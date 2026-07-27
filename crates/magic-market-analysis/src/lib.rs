#![forbid(unsafe_code)]
//! Network-free deterministic analysis over normalized market records.

mod breadth;
mod diagnostics;
mod limit_sentiment;
mod moving_average;
mod valuation;

pub use breadth::{BreadthAnalysis, BreadthLimitPool, BreadthUniverse, MarketBreadthAnalysis};
pub use diagnostics::{cross_source_diagnostics, CrossSourceDiagnostics, CrossSourceObservation};
pub use limit_sentiment::{limit_sentiment, LimitSentiment};
pub use moving_average::simple_moving_average;
pub use valuation::{forward_pe, pe_digestion_years, peg, AttributedValue};

/// Deterministic analysis failure.
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("invalid analysis input: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}
