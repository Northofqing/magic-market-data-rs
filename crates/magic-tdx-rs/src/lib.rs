#![forbid(unsafe_code)]
//! Pure-Rust TDX market-data driver (foundation placeholder).
mod error; pub use error::{ErrorContext,ErrorKind,TdxError};
pub mod codec; pub use codec::{ByteCursor,Limits,decompress_zlib};
