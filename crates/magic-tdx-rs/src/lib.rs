#![forbid(unsafe_code)]
#![allow(clippy::all)]
//! Pure-Rust TDX market-data driver.
pub mod block;
pub mod constants;
pub mod error;
pub mod error_codes;
pub mod fund;
pub mod helpers;
pub mod logging;
pub mod net;
pub mod profile;
pub mod protocol;
pub mod reader;
/// Stable request-domain enums used by higher-level adapters.
pub mod source;
pub use source::{Adjustment, BarCategory, Market};
pub use protocol::types::{IndexBar, SecurityBar, SecurityInfo, SecurityQuote};
pub use fund::types::{FundBar, FundInfo, FundQuote};
