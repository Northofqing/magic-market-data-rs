use crate::error::TdxError;
use crate::{SecurityBar, SecurityQuote, TdxHqClient};
use magic_market_core::{
    AsyncHistoricalBars, AsyncRealtimeQuotes, AuctionSnapshot, Auctions, BarInterval, BarsRequest,
    BookLevel, DataBatch, DataStatus, HistoricalBars, InstrumentId, Money, MoneyFlow, MoneyFlows,
    OrderBook, OrderBooks, Price, ProviderId, Quantity, Quote, Ratio, RatioUnit, RealtimeQuotes,
};
use std::collections::{HashMap, HashSet};

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
            order_book: true,
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
fn strict_bars(
    source: &str,
    records: Vec<SecurityBar>,
) -> Result<DataBatch<SecurityBar>, TdxError> {
    ensure_nonempty(&records)?;
    let provenance = bars_provenance(source, &records);
    Ok(DataBatch::strict(records, provenance))
}

fn optional_quote_price(value: f64, field: &str) -> Result<Option<Price>, TdxError> {
    if !value.is_finite() || value < 0.0 {
        return Err(TdxError::InvalidData(format!(
            "TDX quote {field} must be finite and non-negative"
        )));
    }
    if value == 0.0 {
        Ok(None)
    } else {
        Price::new(value)
            .map(Some)
            .map_err(|error| TdxError::InvalidData(error.to_string()))
    }
}

pub(crate) fn normalize_quotes(
    source: &str,
    instruments: &[InstrumentId],
    records: Vec<SecurityQuote>,
) -> Result<DataBatch<Quote>, TdxError> {
    if instruments.is_empty() {
        return Err(TdxError::InvalidData("TDX quote request is empty".into()));
    }
    let mut requested = HashSet::new();
    if instruments
        .iter()
        .any(|instrument| !requested.insert(instrument.clone()))
    {
        return Err(TdxError::InvalidData(
            "TDX quote request contains duplicate instruments".into(),
        ));
    }
    if records.len() != instruments.len() {
        return Err(TdxError::InvalidData(format!(
            "TDX quote cardinality mismatch: requested {}, received {}",
            instruments.len(),
            records.len()
        )));
    }
    let mut by_key = HashMap::with_capacity(records.len());
    for record in records {
        let key = (record.market, record.code.clone());
        if by_key.insert(key, record).is_some() {
            return Err(TdxError::InvalidData(
                "TDX returned a duplicate quote".into(),
            ));
        }
    }

    let observed_at = fetched_at();
    let batch_id = format!("{source}:{observed_at}:quote");
    let mut quotes = Vec::with_capacity(instruments.len());
    let mut issues = Vec::new();
    let mut batch_source_at = None;
    for instrument in instruments {
        let key = (market(instrument), instrument.code().to_owned());
        let record = by_key
            .remove(&key)
            .ok_or_else(|| TdxError::InvalidData("TDX omitted a requested quote".into()))?;
        let price =
            Price::new(record.price).map_err(|error| TdxError::InvalidData(error.to_string()))?;
        let previous_close = optional_quote_price(record.last_close, "previous close")?;
        let open = optional_quote_price(record.open, "open")?;
        let high = optional_quote_price(record.high, "high")?;
        let low = optional_quote_price(record.low, "low")?;
        let volume =
            Quantity::new(record.vol).map_err(|error| TdxError::InvalidData(error.to_string()))?;
        if !record.amount.is_finite() || record.amount < 0.0 {
            return Err(TdxError::InvalidData(
                "TDX quote amount must be finite and non-negative".into(),
            ));
        }
        let amount = Money::new(record.amount)
            .map(Some)
            .map_err(|error| TdxError::InvalidData(error.to_string()))?;
        let change_percent = previous_close
            .map(|value| {
                Ratio::new(
                    (price.get() - value.get()) / value.get() * 100.0,
                    RatioUnit::Percent,
                )
            })
            .transpose()
            .map_err(|error| TdxError::InvalidData(error.to_string()))?;
        let source_at = (!record.servertime.is_empty()).then_some(record.servertime);
        if batch_source_at.is_none() {
            batch_source_at.clone_from(&source_at);
        }
        let complete = previous_close.is_some()
            && open.is_some()
            && high.is_some()
            && low.is_some()
            && source_at.is_some();
        if !complete {
            issues.push(format!(
                "{}: one or more normalized quote fields unavailable",
                instrument.code()
            ));
        }
        issues.push(format!(
            "{}: security name unavailable from the TDX quote packet",
            instrument.code()
        ));
        quotes.push(Quote {
            instrument: instrument.clone(),
            name: None,
            price,
            previous_close,
            open,
            high,
            low,
            change_percent,
            volume,
            amount,
            status: DataStatus::Unavailable,
            source_at,
            observed_at: observed_at.clone(),
            provider: ProviderId::Tdx,
            batch_id: batch_id.clone(),
        });
    }
    if !by_key.is_empty() {
        return Err(TdxError::InvalidData(
            "TDX returned unexpected quotes".into(),
        ));
    }
    let mut provenance =
        magic_market_core::Provenance::new(source, observed_at).with_batch_id(batch_id);
    if let Some(source_at) = batch_source_at {
        provenance = provenance.with_source_at(source_at);
    }
    Ok(DataBatch::best_effort(quotes, provenance, issues))
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
    type Quote = Quote;
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
        normalize_quotes("tdx", instruments, records)
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

fn book_depth(levels: &[BookLevel; 5]) -> Result<Option<Quantity>, TdxError> {
    let mut found = false;
    let total =
        levels
            .iter()
            .filter_map(|level| level.quantity)
            .fold(0.0, |accumulator, quantity| {
                found = true;
                accumulator + quantity.get()
            });
    if found {
        Quantity::new(total)
            .map(Some)
            .map_err(|error| TdxError::InvalidData(error.to_string()))
    } else {
        Ok(None)
    }
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
        let batch_source_at = quotes
            .iter()
            .find(|quote| !quote.servertime.is_empty())
            .map(|quote| quote.servertime.clone());
        let observed_at = fetched_at();
        let batch_id = format!("tdx:{observed_at}:order-book");
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
            let total_bid_quantity = book_depth(&bids)?;
            let total_ask_quantity = book_depth(&asks)?;
            books.push(OrderBook {
                instrument: id.clone(),
                bids,
                asks,
                total_bid_quantity,
                total_ask_quantity,
                status: magic_market_core::DataStatus::Available,
                source_at: (!quote.servertime.is_empty()).then_some(quote.servertime),
                observed_at: observed_at.clone(),
                provider: magic_market_core::ProviderId::Tdx,
                batch_id: batch_id.clone(),
            });
        }
        let mut provenance =
            magic_market_core::Provenance::new("tdx", observed_at).with_batch_id(batch_id);
        if let Some(source_at) = batch_source_at {
            provenance = provenance.with_source_at(source_at);
        }
        Ok(DataBatch::strict(books, provenance))
    }
}

impl OrderBooks for crate::TdxSmartClient {
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
                "TDX smart order-book cardinality mismatch".into(),
            ));
        }
        let batch_source_at = quotes
            .iter()
            .find(|quote| !quote.servertime.is_empty())
            .map(|quote| quote.servertime.clone());
        let observed_at = fetched_at();
        let batch_id = format!("tdx-smart:{observed_at}:order-book");
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
            let total_bid_quantity = book_depth(&bids)?;
            let total_ask_quantity = book_depth(&asks)?;
            books.push(OrderBook {
                instrument: id.clone(),
                bids,
                asks,
                total_bid_quantity,
                total_ask_quantity,
                status: magic_market_core::DataStatus::Available,
                source_at: (!quote.servertime.is_empty()).then_some(quote.servertime),
                observed_at: observed_at.clone(),
                provider: magic_market_core::ProviderId::Tdx,
                batch_id: batch_id.clone(),
            });
        }
        let mut provenance =
            magic_market_core::Provenance::new("tdx-smart", observed_at).with_batch_id(batch_id);
        if let Some(source_at) = batch_source_at {
            provenance = provenance.with_source_at(source_at);
        }
        Ok(DataBatch::strict(books, provenance))
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
    type Quote = Quote;
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
        normalize_quotes("tdx-smart", instruments, records)
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
    type Quote = Quote;
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
        normalize_quotes("tdx-direct", instruments, records)
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
    type Quote = Quote;
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
        normalize_quotes("tdx-async", instruments, records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magic_market_core::{AssetClass, Exchange};

    fn instrument(code: &str) -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap()
    }

    fn source_quote(code: &str, price: f64) -> SecurityQuote {
        SecurityQuote {
            market: 1,
            code: code.into(),
            active1: 0,
            price,
            last_close: 100.0,
            open: 101.0,
            high: 103.0,
            low: 99.0,
            servertime: "10:00:01".into(),
            vol: 1_000.0,
            cur_vol: 10.0,
            amount: 102_000.0,
            s_vol: 400.0,
            b_vol: 600.0,
            bid1: 101.9,
            bid_vol1: 10.0,
            bid2: 101.8,
            bid_vol2: 11.0,
            bid3: 101.7,
            bid_vol3: 12.0,
            bid4: 101.6,
            bid_vol4: 13.0,
            bid5: 101.5,
            bid_vol5: 14.0,
            ask1: 102.1,
            ask_vol1: 15.0,
            ask2: 102.2,
            ask_vol2: 16.0,
            ask3: 102.3,
            ask_vol3: 17.0,
            ask4: 102.4,
            ask_vol4: 18.0,
            ask5: 102.5,
            ask_vol5: 19.0,
            reversed_bytes0: 0,
            reversed_bytes1: 0,
            reversed_bytes2: 0,
            reversed_bytes3: 0,
            reversed_bytes4: 0,
            reversed_bytes5: 0,
            reversed_bytes6: 0,
            reversed_bytes7: 0,
            reversed_bytes8: 0,
            reversed_bytes9: 0,
            active2: 0,
        }
    }

    #[test]
    fn normalized_quotes_restore_request_order_and_mark_missing_name() {
        let instruments = [instrument("600001"), instrument("600002")];
        let batch = normalize_quotes(
            "test",
            &instruments,
            vec![source_quote("600002", 101.0), source_quote("600001", 102.0)],
        )
        .unwrap();
        assert_eq!(batch.records()[0].instrument.code(), "600001");
        assert_eq!(batch.records()[0].price, Price::new(102.0).unwrap());
        assert_eq!(
            batch.records()[0].change_percent,
            Some(Ratio::new(2.0, RatioUnit::Percent).unwrap())
        );
        assert_eq!(batch.records()[0].status, DataStatus::Unavailable);
        assert!(batch.records()[0].name.is_none());
        assert_eq!(batch.quality().issues.len(), 2);
    }

    #[test]
    fn normalized_quotes_reject_duplicates_and_missing_records() {
        let duplicated = [instrument("600001"), instrument("600001")];
        assert!(normalize_quotes("test", &duplicated, Vec::new()).is_err());

        let requested = [instrument("600001"), instrument("600002")];
        assert!(normalize_quotes("test", &requested, vec![source_quote("600001", 102.0)]).is_err());
    }
}

macro_rules! unsupported_p0 {
    ($client:ty) => {
        impl MoneyFlows for $client {
            type Error = TdxError;
            fn money_flows(
                &self,
                _instruments: &[InstrumentId],
            ) -> Result<DataBatch<MoneyFlow>, Self::Error> {
                Err(TdxError::Unsupported("money_flow".into()))
            }
        }
        impl Auctions for $client {
            type Error = TdxError;
            fn auction_snapshots(
                &self,
                _instruments: &[InstrumentId],
            ) -> Result<DataBatch<AuctionSnapshot>, Self::Error> {
                Err(TdxError::Unsupported("auction".into()))
            }
        }
    };
}
unsupported_p0!(TdxHqClient);
unsupported_p0!(crate::TdxSmartClient);
