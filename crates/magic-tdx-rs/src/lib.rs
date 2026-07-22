#![forbid(unsafe_code)]
#![allow(clippy::all)]
//! Pure-Rust TDX market-data driver.
pub mod block;
pub mod constants;
pub mod error;
pub use error::TdxError;
mod adapter;
pub mod error_codes;
pub mod fund;
pub mod helpers;
pub mod logging;
pub mod net;
pub mod profile;
pub mod protocol;
pub mod reader;
pub mod service;
/// Stable request-domain enums used by higher-level adapters.
pub mod source;
pub use block::TdxBlockClient;
pub use fund::client::TdxHqFundClient;
pub use fund::types::{FundBar, FundInfo, FundQuote};
pub use net::async_client::AsyncTdxHqClient;
pub use net::client::TdxHqClient;
pub use net::direct_client::TdxDirectClient;
pub use net::finance_client::TdxFinanceClient;
pub use net::smart_client::TdxSmartClient;
pub use profile::ProfileClient;
pub use protocol::types::{IndexBar, SecurityBar, SecurityInfo, SecurityQuote};
pub use service::{AsyncTdxService, BlockService, ProfileService, TdxService};
pub use source::{Adjustment, BarCategory, Market};
