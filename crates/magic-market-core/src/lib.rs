#![forbid(unsafe_code)]
//! Provider-neutral market-data contracts.
mod error; mod instrument; mod value;
mod provenance; mod batch;
pub use error::CoreError; pub use instrument::{AssetClass, Exchange, InstrumentId};
pub use value::{Money, Price, Quantity, Ratio, RatioUnit};
pub use provenance::Provenance; pub use batch::{DataBatch,QualityReport};
