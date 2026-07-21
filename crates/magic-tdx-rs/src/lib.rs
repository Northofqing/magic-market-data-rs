#![forbid(unsafe_code)]
//! Pure-Rust TDX market-data driver (foundation placeholder).
mod error; pub use error::{ErrorContext,ErrorKind,TdxError};
pub mod codec; pub use codec::{ByteCursor,Limits,decompress_zlib};
pub mod source; pub use source::{Adjustment,BarCategory,Market};
pub mod protocol; pub use protocol::{PacketBuilder,ResponseHeader};
