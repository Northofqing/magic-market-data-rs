use magic_market_core::{
    AssetClass, AuctionSnapshot, Auctions, Bar, BarInterval, BarsRequest, DataBatch, Exchange,
    HistoricalBars, InstrumentId, MinuteData, MinuteDataRequest, MinutePoint, MoneyFlow,
    MoneyFlows, OrderBook, OrderBooks, PostCloseFlow, PostCloseFlowRequest, PostCloseFlows,
    Provenance, ProviderId, Quote, RealtimeQuotes, SecurityMetadata, SecurityMetadataProvider,
    Trade, Trades, TradesRequest,
};
use magic_market_router::{
    auction_source, bars_source, minute_source, money_flow_source, order_book_source,
    post_close_flow_source, quote_source, security_metadata_source, trades_source, FailureKind,
    RoutedSource, SourceError,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("fixture provider failure")]
struct FixtureError;

struct FixtureProvider {
    calls: AtomicUsize,
    fail: bool,
}

impl FixtureProvider {
    fn new(fail: bool) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            fail,
        }
    }

    fn result<T>(&self) -> Result<DataBatch<T>, FixtureError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(FixtureError)
        } else {
            Ok(DataBatch::strict(
                Vec::new(),
                Provenance::new("fixture", "observed").unwrap(),
            ))
        }
    }
}

impl RealtimeQuotes for FixtureProvider {
    type Quote = Quote;
    type Error = FixtureError;
    fn realtime_quotes(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<Quote>, Self::Error> {
        self.result()
    }
}

impl HistoricalBars for FixtureProvider {
    type Bar = Bar;
    type Error = FixtureError;
    fn historical_bars(&self, _request: &BarsRequest) -> Result<DataBatch<Bar>, Self::Error> {
        self.result()
    }
}

impl MinuteData for FixtureProvider {
    type Error = FixtureError;
    fn minute_data(
        &self,
        _request: &MinuteDataRequest,
    ) -> Result<DataBatch<MinutePoint>, Self::Error> {
        self.result()
    }
}

impl Trades for FixtureProvider {
    type Error = FixtureError;
    fn trades(&self, _request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        self.result()
    }
}

impl MoneyFlows for FixtureProvider {
    type Error = FixtureError;
    fn money_flows(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<MoneyFlow>, Self::Error> {
        self.result()
    }
}

impl OrderBooks for FixtureProvider {
    type Error = FixtureError;
    fn order_books(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        self.result()
    }
}

impl Auctions for FixtureProvider {
    type Error = FixtureError;
    fn auction_snapshots(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<AuctionSnapshot>, Self::Error> {
        self.result()
    }
}

impl PostCloseFlows for FixtureProvider {
    type Error = FixtureError;
    fn post_close_flows(
        &self,
        request: &PostCloseFlowRequest,
    ) -> Result<DataBatch<PostCloseFlow>, Self::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(FixtureError)
        } else {
            Ok(DataBatch::strict(
                Vec::new(),
                Provenance::new("fixture", "observed")
                    .unwrap()
                    .with_source_at(format!("{} 15:35:00", request.trading_date().as_str()))
                    .unwrap(),
            ))
        }
    }
}

impl SecurityMetadataProvider for FixtureProvider {
    type Error = FixtureError;
    fn security_metadata(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        self.result()
    }
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Provider, "fixture provider failure")
}

fn classify_tdx(error: magic_tdx_rs::TdxError) -> SourceError {
    SourceError::try_next(FailureKind::Provider, error.to_string())
}

#[test]
fn magic_tdx_registers_directly_as_a_provider_neutral_bar_source() {
    let provider = Arc::new(magic_tdx_rs::TdxHqClient::new());
    let _source = bars_source(ProviderId::Tdx, provider, classify_tdx);
}

#[test]
fn every_core_family_has_a_provider_neutral_adapter() {
    let provider = Arc::new(FixtureProvider::new(true));
    let instruments = [instrument()];
    let bars_request = BarsRequest::new(instrument(), BarInterval::Day, 5).unwrap();
    let minute_request = MinuteDataRequest::new(instrument());
    let trades_request = TradesRequest::new(instrument(), 5).unwrap();
    let post_close_request = PostCloseFlowRequest::new(
        magic_market_core::IsoDate::new("2026-07-23").unwrap(),
        magic_market_core::PositiveU32::new(10).unwrap(),
    )
    .unwrap();

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
        post_close_flow_source(ProviderId::Custom, Arc::clone(&provider), classify)
            .fetch(&post_close_request)
            .is_err()
    );
    assert!(
        security_metadata_source(ProviderId::Custom, Arc::clone(&provider), classify)
            .fetch(&instruments)
            .is_err()
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 9);
}

#[test]
fn every_adapter_preserves_a_successful_provider_batch() {
    let provider = Arc::new(FixtureProvider::new(false));
    let instruments = [instrument()];
    let bars_request = BarsRequest::new(instrument(), BarInterval::Day, 5).unwrap();
    let minute_request = MinuteDataRequest::new(instrument());
    let trades_request = TradesRequest::new(instrument(), 5).unwrap();
    let post_close_request = PostCloseFlowRequest::new(
        magic_market_core::IsoDate::new("2026-07-23").unwrap(),
        magic_market_core::PositiveU32::new(10).unwrap(),
    )
    .unwrap();

    let quote = quote_source(ProviderId::Custom, Arc::clone(&provider), classify)
        .fetch(&instruments)
        .unwrap();
    let bars = bars_source(ProviderId::Custom, Arc::clone(&provider), classify)
        .fetch(&bars_request)
        .unwrap();
    let minute = minute_source(ProviderId::Custom, Arc::clone(&provider), classify)
        .fetch(&minute_request)
        .unwrap();
    let trades = trades_source(ProviderId::Custom, Arc::clone(&provider), classify)
        .fetch(&trades_request)
        .unwrap();
    let flow = money_flow_source(ProviderId::Custom, Arc::clone(&provider), classify)
        .fetch(&instruments)
        .unwrap();
    let book = order_book_source(ProviderId::Custom, Arc::clone(&provider), classify)
        .fetch(&instruments)
        .unwrap();
    let auction = auction_source(ProviderId::Custom, Arc::clone(&provider), classify)
        .fetch(&instruments)
        .unwrap();
    let post_close = post_close_flow_source(ProviderId::Custom, Arc::clone(&provider), classify)
        .fetch(&post_close_request)
        .unwrap();
    let metadata = security_metadata_source(ProviderId::Custom, Arc::clone(&provider), classify)
        .fetch(&instruments)
        .unwrap();

    for provenance in [
        quote.provenance(),
        bars.provenance(),
        minute.provenance(),
        trades.provenance(),
        flow.provenance(),
        book.provenance(),
        auction.provenance(),
        post_close.provenance(),
        metadata.provenance(),
    ] {
        assert_eq!(provenance.source(), "fixture");
        assert_eq!(provenance.fetched_at(), "observed");
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 9);
}
