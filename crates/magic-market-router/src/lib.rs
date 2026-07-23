#![forbid(unsafe_code)]
//! Provider-neutral, evidence-preserving market-data failover routing.

mod adapters;
mod error;
mod router;
mod source;

pub use adapters::{
    auction_source, bars_source, minute_source, money_flow_source, order_book_source, quote_source,
    security_metadata_source, trades_source, AuctionRouter, BarsRouter, MinuteRouter,
    MoneyFlowRouter, OrderBookRouter, QuoteRouter, SecurityMetadataRouter, TradesRouter,
};
pub use error::{FailureAction, FailureKind, SourceError};
pub use router::{
    AcceptancePolicy, AttemptStatus, FailoverChain, RouteAttempt, RouteOutcome, RouterError,
};
pub use source::{RoutedSource, SourceFn};
