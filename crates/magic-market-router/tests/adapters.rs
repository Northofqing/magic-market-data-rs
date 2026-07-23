use magic_market_core::{
    AssetClass, AuctionSnapshot, Auctions, Bar, BarInterval, BarsRequest, DataBatch, Exchange,
    HistoricalBars, InstrumentId, MinuteData, MinuteDataRequest, MinutePoint, MoneyFlow,
    MoneyFlows, OrderBook, OrderBooks, ProviderId, Quote, RealtimeQuotes, SecurityMetadata,
    SecurityMetadataProvider, Trade, Trades, TradesRequest,
};
use magic_market_router::{
    auction_source, bars_source, minute_source, money_flow_source, order_book_source, quote_source,
    security_metadata_source, trades_source, FailureKind, RoutedSource, SourceError,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("fixture provider failure")]
struct FixtureError;

#[derive(Default)]
struct FixtureProvider {
    calls: AtomicUsize,
}

impl FixtureProvider {
    fn fail<T>(&self) -> Result<DataBatch<T>, FixtureError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(FixtureError)
    }
}

impl RealtimeQuotes for FixtureProvider {
    type Quote = Quote;
    type Error = FixtureError;
    fn realtime_quotes(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<Quote>, Self::Error> {
        self.fail()
    }
}

impl HistoricalBars for FixtureProvider {
    type Bar = Bar;
    type Error = FixtureError;
    fn historical_bars(&self, _request: &BarsRequest) -> Result<DataBatch<Bar>, Self::Error> {
        self.fail()
    }
}

impl MinuteData for FixtureProvider {
    type Error = FixtureError;
    fn minute_data(
        &self,
        _request: &MinuteDataRequest,
    ) -> Result<DataBatch<MinutePoint>, Self::Error> {
        self.fail()
    }
}

impl Trades for FixtureProvider {
    type Error = FixtureError;
    fn trades(&self, _request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        self.fail()
    }
}

impl MoneyFlows for FixtureProvider {
    type Error = FixtureError;
    fn money_flows(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<MoneyFlow>, Self::Error> {
        self.fail()
    }
}

impl OrderBooks for FixtureProvider {
    type Error = FixtureError;
    fn order_books(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        self.fail()
    }
}

impl Auctions for FixtureProvider {
    type Error = FixtureError;
    fn auction_snapshots(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<AuctionSnapshot>, Self::Error> {
        self.fail()
    }
}

impl SecurityMetadataProvider for FixtureProvider {
    type Error = FixtureError;
    fn security_metadata(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        self.fail()
    }
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Provider, "fixture provider failure")
}

#[test]
fn every_core_family_has_a_provider_neutral_adapter() {
    let provider = Arc::new(FixtureProvider::default());
    let instruments = [instrument()];
    let bars_request = BarsRequest::new(instrument(), BarInterval::Day, 5).unwrap();
    let minute_request = MinuteDataRequest::new(instrument());
    let trades_request = TradesRequest::new(instrument(), 5).unwrap();

    assert_eq!(
        quote_source(ProviderId::Custom, Arc::clone(&provider), classify)
            .fetch(&instruments)
            .unwrap_err()
            .kind(),
        FailureKind::Provider
    );
    assert!(
        bars_source(ProviderId::Custom, Arc::clone(&provider), classify)
            .fetch(&bars_request)
            .is_err()
    );
    assert!(
        minute_source(ProviderId::Custom, Arc::clone(&provider), classify)
            .fetch(&minute_request)
            .is_err()
    );
    assert!(
        trades_source(ProviderId::Custom, Arc::clone(&provider), classify)
            .fetch(&trades_request)
            .is_err()
    );
    assert!(
        money_flow_source(ProviderId::Custom, Arc::clone(&provider), classify)
            .fetch(&instruments)
            .is_err()
    );
    assert!(
        order_book_source(ProviderId::Custom, Arc::clone(&provider), classify)
            .fetch(&instruments)
            .is_err()
    );
    assert!(
        auction_source(ProviderId::Custom, Arc::clone(&provider), classify)
            .fetch(&instruments)
            .is_err()
    );
    assert!(
        security_metadata_source(ProviderId::Custom, Arc::clone(&provider), classify)
            .fetch(&instruments)
            .is_err()
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 8);
}
