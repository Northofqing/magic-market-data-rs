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
