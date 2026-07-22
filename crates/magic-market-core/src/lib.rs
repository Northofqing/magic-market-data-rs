#![forbid(unsafe_code)]
//! Provider-neutral market-data contracts.
mod batch;
mod error;
mod instrument;
mod provenance;
mod provider;
mod value;
pub use batch::{DataBatch, QualityReport};
pub use error::CoreError;
pub use instrument::{AssetClass, Exchange, InstrumentId};
pub use provenance::Provenance;
pub use provider::{
    Adjustment, AsyncHistoricalBars, AsyncRealtimeQuotes, AsyncTrades, AuctionSnapshot, Auctions,
    Bar, BarInterval, BarsRequest, Board, BookLevel, Capabilities, DataStatus, HistoricalBars,
    MoneyFlow, MoneyFlows, OrderBook, OrderBooks, PriceLimitRule, ProviderId, Quote,
    RealtimeQuotes, SecurityMetadata, SecurityMetadataProvider, Trade, TradeSide, Trades,
    TradesRequest,
};
pub use value::{Money, Price, Quantity, Ratio, RatioUnit};
