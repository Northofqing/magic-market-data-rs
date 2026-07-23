use crate::{FailoverChain, SourceError, SourceFn};
use magic_market_core::{
    AuctionSnapshot, Auctions, Bar, BarsRequest, HistoricalBars, InstrumentId, MinuteData,
    MinuteDataRequest, MinutePoint, MoneyFlow, MoneyFlows, OrderBook, OrderBooks, ProviderId,
    Quote, RealtimeQuotes, SecurityMetadata, SecurityMetadataProvider, Trade, Trades,
    TradesRequest,
};
use std::sync::Arc;

pub type QuoteRouter = FailoverChain<[InstrumentId], Quote>;
pub type BarsRouter = FailoverChain<BarsRequest, Bar>;
pub type MinuteRouter = FailoverChain<MinuteDataRequest, MinutePoint>;
pub type TradesRouter = FailoverChain<TradesRequest, Trade>;
pub type MoneyFlowRouter = FailoverChain<[InstrumentId], MoneyFlow>;
pub type OrderBookRouter = FailoverChain<[InstrumentId], OrderBook>;
pub type AuctionRouter = FailoverChain<[InstrumentId], AuctionSnapshot>;
pub type SecurityMetadataRouter = FailoverChain<[InstrumentId], SecurityMetadata>;

pub fn quote_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], Quote>
where
    Provider: RealtimeQuotes<Quote = Quote> + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.realtime_quotes(request).map_err(&classify)
    })
}

pub fn bars_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<BarsRequest, Bar>
where
    Provider: HistoricalBars<Bar = Bar> + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.historical_bars(request).map_err(&classify)
    })
}

pub fn minute_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<MinuteDataRequest, MinutePoint>
where
    Provider: MinuteData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.minute_data(request).map_err(&classify)
    })
}

pub fn trades_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<TradesRequest, Trade>
where
    Provider: Trades + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.trades(request).map_err(&classify)
    })
}

pub fn money_flow_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], MoneyFlow>
where
    Provider: MoneyFlows + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.money_flows(request).map_err(&classify)
    })
}

pub fn order_book_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], OrderBook>
where
    Provider: OrderBooks + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.order_books(request).map_err(&classify)
    })
}

pub fn auction_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], AuctionSnapshot>
where
    Provider: Auctions + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.auction_snapshots(request).map_err(&classify)
    })
}

pub fn security_metadata_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], SecurityMetadata>
where
    Provider: SecurityMetadataProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.security_metadata(request).map_err(&classify)
    })
}
