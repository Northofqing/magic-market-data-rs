use crate::error::TdxError;
use crate::{SecurityBar, SecurityQuote, TdxHqClient};
use magic_market_core::{
    AsyncHistoricalBars, AsyncRealtimeQuotes, BarInterval, BarsRequest, BookLevel, DataBatch,
    HistoricalBars, InstrumentId, OrderBook, OrderBooks, Price, Quantity, RealtimeQuotes,
};

impl TdxHqClient {
    /// Returns the data families exposed through the core provider boundary.
    pub const fn capabilities() -> magic_market_core::Capabilities {
        magic_market_core::Capabilities {
            quotes: true,
            bars: true,
            minute: true,
            trades: true,
            fundamentals: true,
            corporate_actions: true,
            blocks: true,
            money_flow: false,
            order_book: false,
            auction: false,
        }
    }
}

fn market(id: &InstrumentId) -> u8 {
    match id.exchange() {
        magic_market_core::Exchange::Shanghai => 1,
        magic_market_core::Exchange::Shenzhen => 0,
    }
}
fn category(interval: BarInterval) -> u8 {
    match interval {
        BarInterval::Minute1 => 7,
        BarInterval::Minute5 => 0,
        BarInterval::Minute15 => 1,
        BarInterval::Minute30 => 2,
        BarInterval::Hour1 => 3,
        BarInterval::Day => 4,
        BarInterval::Week => 5,
        BarInterval::Month => 6,
        BarInterval::Year => 6,
    }
}
fn ensure_nonempty<T>(records: &[T]) -> Result<(), TdxError> {
    if records.is_empty() {
        Err(TdxError::InvalidData(
            "TDX returned an empty successful response".into(),
        ))
    } else {
        Ok(())
    }
}
fn fetched_at() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or_else(|_| "unknown".into(), |d| d.as_secs().to_string())
}
fn bars_provenance(source: &str, records: &[SecurityBar]) -> magic_market_core::Provenance {
    let p = magic_market_core::Provenance::new(source, fetched_at());
    match records.first() {
        Some(bar) => p.with_source_at(bar.datetime.clone()),
        None => p,
    }
}
fn quotes_provenance(source: &str, records: &[SecurityQuote]) -> magic_market_core::Provenance {
    let p = magic_market_core::Provenance::new(source, fetched_at());
    match records.first() {
        Some(quote) => p.with_source_at(quote.servertime.clone()),
        None => p,
    }
}
fn strict_bars(
    source: &str,
    records: Vec<SecurityBar>,
) -> Result<DataBatch<SecurityBar>, TdxError> {
    ensure_nonempty(&records)?;
    let provenance = bars_provenance(source, &records);
    Ok(DataBatch::strict(records, provenance))
}
fn strict_quotes(
    source: &str,
    records: Vec<SecurityQuote>,
) -> Result<DataBatch<SecurityQuote>, TdxError> {
    ensure_nonempty(&records)?;
    let provenance = quotes_provenance(source, &records);
    Ok(DataBatch::strict(records, provenance))
}

impl HistoricalBars for TdxHqClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        let records = self.get_security_bars(
            category(request.interval),
            market(&request.instrument),
            request.instrument.code(),
            0,
            request.limit,
            0,
        )?;
        strict_bars("tdx", records)
    }
}

impl RealtimeQuotes for TdxHqClient {
    type Quote = SecurityQuote;
    type Error = TdxError;
    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let pairs: Vec<(u8, &str)> = instruments
            .iter()
            .map(|id| (market(id), id.code()))
            .collect();
        let records = self.get_security_quotes(&pairs)?;
        strict_quotes("tdx", records)
    }
}

fn book_level(price: f64, quantity: f64) -> Result<BookLevel, TdxError> {
    let price = if price > 0.0 {
        Some(Price::new(price).map_err(|e| TdxError::InvalidData(e.to_string()))?)
    } else {
        None
    };
    let quantity = if quantity >= 0.0 {
        Some(Quantity::new(quantity).map_err(|e| TdxError::InvalidData(e.to_string()))?)
    } else {
        None
    };
    Ok(BookLevel { price, quantity })
}

impl OrderBooks for TdxHqClient {
    type Error = TdxError;
    fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        let pairs: Vec<(u8, &str)> = instruments
            .iter()
            .map(|id| (market(id), id.code()))
            .collect();
        let quotes = self.get_security_quotes(&pairs)?;
        if quotes.len() != instruments.len() {
            return Err(TdxError::InvalidData(
                "TDX order-book cardinality mismatch".into(),
            ));
        }
        let mut books = Vec::with_capacity(quotes.len());
        for (id, quote) in instruments.iter().zip(quotes) {
            let bids = [
                book_level(quote.bid1, quote.bid_vol1)?,
                book_level(quote.bid2, quote.bid_vol2)?,
                book_level(quote.bid3, quote.bid_vol3)?,
                book_level(quote.bid4, quote.bid_vol4)?,
                book_level(quote.bid5, quote.bid_vol5)?,
            ];
            let asks = [
                book_level(quote.ask1, quote.ask_vol1)?,
                book_level(quote.ask2, quote.ask_vol2)?,
                book_level(quote.ask3, quote.ask_vol3)?,
                book_level(quote.ask4, quote.ask_vol4)?,
                book_level(quote.ask5, quote.ask_vol5)?,
            ];
            books.push(OrderBook {
                instrument: id.clone(),
                bids,
                asks,
                status: magic_market_core::DataStatus::Available,
            });
        }
        Ok(DataBatch::strict(
            books,
            magic_market_core::Provenance::new("tdx", fetched_at()),
        ))
    }
}

impl HistoricalBars for crate::TdxSmartClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        let records = self.get_security_bars(
            category(request.interval),
            market(&request.instrument),
            request.instrument.code(),
            0,
            request.limit,
            0,
        )?;
        strict_bars("tdx-smart", records)
    }
}

impl RealtimeQuotes for crate::TdxSmartClient {
    type Quote = SecurityQuote;
    type Error = TdxError;
    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let pairs: Vec<(u8, &str)> = instruments
            .iter()
            .map(|id| (market(id), id.code()))
            .collect();
        let records = self.get_security_quotes(&pairs)?;
        strict_quotes("tdx-smart", records)
    }
}

impl HistoricalBars for crate::TdxDirectClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        let records = self.get_security_bars(
            category(request.interval),
            market(&request.instrument),
            request.instrument.code(),
            0,
            request.limit,
            0,
        )?;
        strict_bars("tdx-direct", records)
    }
}

impl RealtimeQuotes for crate::TdxDirectClient {
    type Quote = SecurityQuote;
    type Error = TdxError;
    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let pairs: Vec<(u8, &str)> = instruments
            .iter()
            .map(|id| (market(id), id.code()))
            .collect();
        let records = self.get_security_quotes(&pairs)?;
        strict_quotes("tdx-direct", records)
    }
}

impl AsyncHistoricalBars for crate::AsyncTdxHqClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    async fn historical_bars_async(
        &self,
        request: &BarsRequest,
    ) -> Result<DataBatch<Self::Bar>, Self::Error> {
        let records = self
            .get_security_bars(
                category(request.interval),
                market(&request.instrument),
                request.instrument.code(),
                0,
                request.limit,
                0,
            )
            .await?;
        strict_bars("tdx-async", records)
    }
}

impl AsyncRealtimeQuotes for crate::AsyncTdxHqClient {
    type Quote = SecurityQuote;
    type Error = TdxError;
    async fn realtime_quotes_async(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        let pairs: Vec<(u8, &str)> = instruments
            .iter()
            .map(|id| (market(id), id.code()))
            .collect();
        let records = self.get_security_quotes(&pairs).await?;
        strict_quotes("tdx-async", records)
    }
}
