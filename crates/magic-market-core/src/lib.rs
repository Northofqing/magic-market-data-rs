#![forbid(unsafe_code)]
//! Provider-neutral market-data contracts.
mod batch;
mod error;
mod instrument;
mod provenance;
mod value;
mod provider;
pub use batch::{DataBatch, QualityReport};
pub use error::CoreError;
pub use instrument::{AssetClass, Exchange, InstrumentId};
pub use provenance::Provenance;
pub use value::{Money, Price, Quantity, Ratio, RatioUnit};
pub use provider::{BarInterval, BarsRequest, HistoricalBars, RealtimeQuotes};
