use crate::error::TdxError;
use crate::protocol::constants::{
    KLINE_15MIN, KLINE_1HOUR, KLINE_1MIN, KLINE_30MIN, KLINE_5MIN, KLINE_DAILY, KLINE_MONTHLY,
    KLINE_WEEKLY, KLINE_YEARLY,
};
use crate::protocol::types::{MinuteTimePrice, SecurityInfo, TickData};
use crate::{SecurityBar, SecurityQuote, TdxHqClient};
use magic_market_core::{
    AsyncHistoricalBars, AsyncRealtimeQuotes, AsyncTrades, AuctionSnapshot, Auctions, BarInterval,
    BarsRequest, Board, BookLevel, DataBatch, DataStatus, HistoricalBars, InstrumentId, MinuteData,
    MinuteDataRequest, MinutePoint, Money, MoneyFlow, MoneyFlows, OrderBook, OrderBooks, Price,
    PriceLimitRule, ProviderId, Quantity, Quote, Ratio, RatioUnit, RealtimeQuotes,
    SecurityMetadata, SecurityMetadataProvider, Trade, TradeSide, Trades, TradesRequest,
};
use std::collections::{HashMap, HashSet};

const CURRENT_TRADE_PAGE_SIZE: u16 = 1_800;
const HISTORICAL_TRADE_PAGE_SIZE: u16 = 2_000;

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
            security_metadata: true,
        }
    }
}

fn market(id: &InstrumentId) -> Result<u8, TdxError> {
    match id.exchange() {
        magic_market_core::Exchange::Shanghai => Ok(1),
        magic_market_core::Exchange::Shenzhen => Ok(0),
        // Live protocol evidence on 2026-07-23 uniquely returned market/code
        // `(2, 920118)` for market 2. Candidates 0 and 1 returned a mismatched
        // Shanghai record and are therefore never accepted for Beijing.
        magic_market_core::Exchange::Beijing => Ok(2),
    }
}
fn category(interval: BarInterval) -> Result<u8, TdxError> {
    match interval {
        BarInterval::Minute1 => Ok(KLINE_1MIN),
        BarInterval::Minute5 => Ok(KLINE_5MIN),
        BarInterval::Minute15 => Ok(KLINE_15MIN),
        BarInterval::Minute30 => Ok(KLINE_30MIN),
        BarInterval::Hour1 => Ok(KLINE_1HOUR),
        BarInterval::Day => Ok(KLINE_DAILY),
        BarInterval::Week => Ok(KLINE_WEEKLY),
        BarInterval::Month => Ok(KLINE_MONTHLY),
        BarInterval::Year => Ok(KLINE_YEARLY),
    }
}
fn reject_unsupported_bar_range(request: &BarsRequest) -> Result<(), TdxError> {
    if request.start().is_some() || request.end().is_some() {
        Err(TdxError::Unsupported(
            "TDX historical bars do not support normalized date ranges; omit start/end".into(),
        ))
    } else {
        Ok(())
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
fn fetched_at() -> Result<String, TdxError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .map_err(|error| {
            TdxError::InvalidData(format!("system clock is before UNIX epoch: {error}"))
        })
}
fn bars_provenance(
    source: &str,
    records: &[SecurityBar],
) -> Result<magic_market_core::Provenance, TdxError> {
    let p = magic_market_core::Provenance::new(source, fetched_at()?)?;
    match records.first() {
        Some(bar) => Ok(p.with_source_at(bar.datetime.clone())?),
        None => Ok(p),
    }
}
fn strict_bars(
    source: &str,
    records: Vec<SecurityBar>,
) -> Result<DataBatch<SecurityBar>, TdxError> {
    ensure_nonempty(&records)?;
    let provenance = bars_provenance(source, &records)?;
    Ok(DataBatch::strict(records, provenance))
}

trait BlockingTdxQuery {
    fn security_bars(
        &self,
        category: u8,
        market: u8,
        code: &str,
        start: u32,
        count: u16,
        adjust: u8,
    ) -> Result<Vec<SecurityBar>, TdxError>;

    fn security_quotes(
        &self,
        instruments: &[(u8, &str)],
    ) -> Result<Vec<SecurityQuote>, TdxError>;

    fn minute_time_data(
        &self,
        market: u8,
        code: &str,
    ) -> Result<Vec<MinuteTimePrice>, TdxError>;

    fn history_minute_time_data(
        &self,
        market: u8,
        code: &str,
        date: u32,
    ) -> Result<Vec<MinuteTimePrice>, TdxError>;

    fn transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<Vec<TickData>, TdxError>;

    fn history_transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
        date: u32,
    ) -> Result<Vec<TickData>, TdxError>;

    fn security_count(&self, market: u8) -> Result<u16, TdxError>;

    fn security_list(&self, market: u8, start: u16) -> Result<Vec<SecurityInfo>, TdxError>;
}

impl BlockingTdxQuery for TdxHqClient {
    fn security_bars(
        &self,
        category: u8,
        market: u8,
        code: &str,
        start: u32,
        count: u16,
        adjust: u8,
    ) -> Result<Vec<SecurityBar>, TdxError> {
        TdxHqClient::get_security_bars(self, category, market, code, start, count, adjust)
    }

    fn security_quotes(
        &self,
        instruments: &[(u8, &str)],
    ) -> Result<Vec<SecurityQuote>, TdxError> {
        TdxHqClient::get_security_quotes(self, instruments)
    }

    fn minute_time_data(
        &self,
        market: u8,
        code: &str,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        TdxHqClient::get_minute_time_data(self, market, code)
    }

    fn history_minute_time_data(
        &self,
        market: u8,
        code: &str,
        date: u32,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        TdxHqClient::get_history_minute_time_data(self, market, code, date)
    }

    fn transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<Vec<TickData>, TdxError> {
        TdxHqClient::get_transaction_data(self, market, code, start, count)
    }

    fn history_transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
        date: u32,
    ) -> Result<Vec<TickData>, TdxError> {
        TdxHqClient::get_history_transaction_data(self, market, code, start, count, date)
    }

    fn security_count(&self, market: u8) -> Result<u16, TdxError> {
        TdxHqClient::get_security_count(self, market)
    }

    fn security_list(&self, market: u8, start: u16) -> Result<Vec<SecurityInfo>, TdxError> {
        TdxHqClient::get_security_list(self, market, start)
    }
}

impl BlockingTdxQuery for crate::TdxSmartClient {
    fn security_bars(
        &self,
        category: u8,
        market: u8,
        code: &str,
        start: u32,
        count: u16,
        adjust: u8,
    ) -> Result<Vec<SecurityBar>, TdxError> {
        crate::TdxSmartClient::get_security_bars(
            self, category, market, code, start, count, adjust,
        )
    }

    fn security_quotes(
        &self,
        instruments: &[(u8, &str)],
    ) -> Result<Vec<SecurityQuote>, TdxError> {
        crate::TdxSmartClient::get_security_quotes(self, instruments)
    }

    fn minute_time_data(
        &self,
        market: u8,
        code: &str,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        TdxHqClient::get_minute_time_data(self.inner(), market, code)
    }

    fn history_minute_time_data(
        &self,
        market: u8,
        code: &str,
        date: u32,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        TdxHqClient::get_history_minute_time_data(self.inner(), market, code, date)
    }

    fn transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<Vec<TickData>, TdxError> {
        TdxHqClient::get_transaction_data(self.inner(), market, code, start, count)
    }

    fn history_transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
        date: u32,
    ) -> Result<Vec<TickData>, TdxError> {
        TdxHqClient::get_history_transaction_data(
            self.inner(),
            market,
            code,
            start,
            count,
            date,
        )
    }

    fn security_count(&self, market: u8) -> Result<u16, TdxError> {
        TdxHqClient::get_security_count(self.inner(), market)
    }

    fn security_list(&self, market: u8, start: u16) -> Result<Vec<SecurityInfo>, TdxError> {
        TdxHqClient::get_security_list(self.inner(), market, start)
    }
}

impl BlockingTdxQuery for crate::TdxDirectClient {
    fn security_bars(
        &self,
        category: u8,
        market: u8,
        code: &str,
        start: u32,
        count: u16,
        adjust: u8,
    ) -> Result<Vec<SecurityBar>, TdxError> {
        crate::TdxDirectClient::get_security_bars(
            self, category, market, code, start, count, adjust,
        )
    }

    fn security_quotes(
        &self,
        instruments: &[(u8, &str)],
    ) -> Result<Vec<SecurityQuote>, TdxError> {
        crate::TdxDirectClient::get_security_quotes(self, instruments)
    }

    fn minute_time_data(
        &self,
        market: u8,
        code: &str,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        crate::TdxDirectClient::get_minute_time_data(self, market, code)
    }

    fn history_minute_time_data(
        &self,
        market: u8,
        code: &str,
        date: u32,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        crate::TdxDirectClient::get_history_minute_time_data(self, market, code, date)
    }

    fn transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<Vec<TickData>, TdxError> {
        crate::TdxDirectClient::get_transaction_data(self, market, code, start, count)
    }

    fn history_transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
        date: u32,
    ) -> Result<Vec<TickData>, TdxError> {
        crate::TdxDirectClient::get_history_transaction_data(
            self, market, code, start, count, date,
        )
    }

    fn security_count(&self, market: u8) -> Result<u16, TdxError> {
        crate::TdxDirectClient::get_security_count(self, market)
    }

    fn security_list(&self, market: u8, start: u16) -> Result<Vec<SecurityInfo>, TdxError> {
        crate::TdxDirectClient::get_security_list(self, market, start)
    }
}

trait AsyncTdxQuery {
    async fn security_bars(
        &self,
        category: u8,
        market: u8,
        code: &str,
        start: u32,
        count: u16,
        adjust: u8,
    ) -> Result<Vec<SecurityBar>, TdxError>;

    async fn security_quotes(
        &self,
        instruments: &[(u8, &str)],
    ) -> Result<Vec<SecurityQuote>, TdxError>;
}

impl AsyncTdxQuery for crate::AsyncTdxHqClient {
    async fn security_bars(
        &self,
        category: u8,
        market: u8,
        code: &str,
        start: u32,
        count: u16,
        adjust: u8,
    ) -> Result<Vec<SecurityBar>, TdxError> {
        crate::AsyncTdxHqClient::get_security_bars(
            self, category, market, code, start, count, adjust,
        )
        .await
    }

    async fn security_quotes(
        &self,
        instruments: &[(u8, &str)],
    ) -> Result<Vec<SecurityQuote>, TdxError> {
        crate::AsyncTdxHqClient::get_security_quotes(self, instruments).await
    }
}

fn historical_bars_with(
    query: &impl BlockingTdxQuery,
    source: &str,
    request: &BarsRequest,
) -> Result<DataBatch<SecurityBar>, TdxError> {
    reject_unsupported_bar_range(request)?;
    let records = query.security_bars(
        category(request.interval())?,
        market(request.instrument())?,
        request.instrument().code(),
        0,
        request.limit(),
        0,
    )?;
    strict_bars(source, records)
}

async fn historical_bars_async_with(
    query: &impl AsyncTdxQuery,
    source: &str,
    request: &BarsRequest,
) -> Result<DataBatch<SecurityBar>, TdxError> {
    reject_unsupported_bar_range(request)?;
    let records = query
        .security_bars(
            category(request.interval())?,
            market(request.instrument())?,
            request.instrument().code(),
            0,
            request.limit(),
            0,
        )
        .await?;
    strict_bars(source, records)
}

async fn realtime_quotes_async_with(
    query: &impl AsyncTdxQuery,
    source: &str,
    instruments: &[InstrumentId],
) -> Result<DataBatch<Quote>, TdxError> {
    let pairs: Vec<(u8, &str)> = instruments
        .iter()
        .map(|id| market(id).map(|market| (market, id.code())))
        .collect::<Result<_, _>>()?;
    let records = query.security_quotes(&pairs).await?;
    normalize_quotes(source, instruments, records)
}

fn realtime_quotes_with(
    query: &impl BlockingTdxQuery,
    source: &str,
    instruments: &[InstrumentId],
) -> Result<DataBatch<Quote>, TdxError> {
    let pairs: Vec<(u8, &str)> = instruments
        .iter()
        .map(|id| market(id).map(|market| (market, id.code())))
        .collect::<Result<_, _>>()?;
    let records = query.security_quotes(&pairs)?;
    normalize_quotes(source, instruments, records)
}

fn minute_data_with(
    query: &impl BlockingTdxQuery,
    source: &str,
    request: &MinuteDataRequest,
) -> Result<DataBatch<MinutePoint>, TdxError> {
    let (date, records) = match request.date() {
        Some(date) => (
            date.to_owned(),
            query.history_minute_time_data(
                market(request.instrument())?,
                request.instrument().code(),
                compact_date(date)?,
            )?,
        ),
        None => {
            let compact = crate::net::utils::today_yyyymmdd();
            (
                display_date(compact)?,
                query.minute_time_data(
                    market(request.instrument())?,
                    request.instrument().code(),
                )?,
            )
        }
    };
    normalize_minute_records(source, request.instrument(), &date, records)
}

fn trades_with(
    query: &impl BlockingTdxQuery,
    current_source: &str,
    history_source: &str,
    request: &TradesRequest,
) -> Result<DataBatch<Trade>, TdxError> {
    let request_market = market(request.instrument())?;
    match request.date() {
        Some(date) => {
            let date = tdx_trade_date(date)?;
            paginate_trades(
                history_source,
                request,
                HISTORICAL_TRADE_PAGE_SIZE,
                |start, count| {
                    query.history_transaction_data(
                        request_market,
                        request.instrument().code(),
                        start,
                        count,
                        date,
                    )
                },
            )
        }
        None => paginate_trades(
            current_source,
            request,
            CURRENT_TRADE_PAGE_SIZE,
            |start, count| {
                query.transaction_data(
                    request_market,
                    request.instrument().code(),
                    start,
                    count,
                )
            },
        ),
    }
}

fn security_metadata_with(
    query: &impl BlockingTdxQuery,
    source: &str,
    instruments: &[InstrumentId],
) -> Result<DataBatch<SecurityMetadata>, TdxError> {
    validate_security_metadata_request(instruments)?;
    let records = fetch_security_records(
        instruments,
        |market| query.security_count(market),
        |market, start| query.security_list(market, start),
    )?;
    normalize_security_metadata(source, instruments, records)
}

fn compact_date(value: &str) -> Result<u32, TdxError> {
    let digits = value.replace('-', "");
    if digits.len() != 8 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(TdxError::InvalidData(format!(
            "invalid TDX minute date {value:?}"
        )));
    }
    digits
        .parse()
        .map_err(|_| TdxError::InvalidData(format!("invalid TDX minute date {value:?}")))
}

fn display_date(value: u32) -> Result<String, TdxError> {
    let value = format!("{value:08}");
    if value.len() != 8 {
        return Err(TdxError::InvalidData(
            "TDX minute date must contain eight digits".into(),
        ));
    }
    Ok(format!(
        "{}-{}-{}",
        &value[0..4],
        &value[4..6],
        &value[6..8]
    ))
}

fn valid_tdx_minute(value: &str) -> bool {
    value.len() == 5
        && value.as_bytes()[2] == b':'
        && (("09:31"..="11:30").contains(&value) || ("13:01"..="15:00").contains(&value))
}

fn normalize_minute_records(
    source: &str,
    instrument: &InstrumentId,
    date: &str,
    mut records: Vec<MinuteTimePrice>,
) -> Result<DataBatch<MinutePoint>, TdxError> {
    if records.is_empty() {
        return Err(TdxError::InvalidData(
            "TDX returned an empty minute-data response".into(),
        ));
    }
    if records.len() > 240 {
        return Err(TdxError::InvalidData(format!(
            "TDX returned {} minute points; maximum is 240",
            records.len()
        )));
    }
    records.sort_by(|left, right| left.time.cmp(&right.time));
    let observed_at = fetched_at()?;
    let batch_id = format!("{source}:{observed_at}:minute");
    let mut normalized = Vec::with_capacity(records.len());
    let mut cumulative_quantity = 0.0;
    let mut previous_time: Option<&str> = None;
    for record in &records {
        if !valid_tdx_minute(&record.time) {
            return Err(TdxError::InvalidData(format!(
                "TDX minute time is outside the verified session grid: {:?}",
                record.time
            )));
        }
        if previous_time.is_some_and(|previous| previous >= record.time.as_str()) {
            return Err(TdxError::InvalidData(
                "TDX minute times are duplicated or unordered".into(),
            ));
        }
        if !record.vol.is_finite() || record.vol < 0.0 {
            return Err(TdxError::InvalidData(
                "TDX minute volume must be finite and non-negative".into(),
            ));
        }
        cumulative_quantity += record.vol;
        if !cumulative_quantity.is_finite() {
            return Err(TdxError::InvalidData(
                "TDX cumulative minute volume overflowed".into(),
            ));
        }
        let minute_at = format!("{date} {}", record.time);
        let source_at = format!("{date}T{}:00+08:00", record.time);
        normalized.push(MinutePoint::new(
            instrument.clone(),
            minute_at,
            Price::new(record.price)?,
            Quantity::new(cumulative_quantity)?,
            None,
            DataStatus::Available,
            Some(source_at),
            observed_at.clone(),
            ProviderId::Tdx,
            batch_id.clone(),
        )?);
        previous_time = Some(&record.time);
    }
    let latest_source_at = normalized
        .last()
        .and_then(|point| point.source_at())
        .ok_or_else(|| TdxError::InvalidData("TDX minute source time is missing".into()))?;
    let provenance = magic_market_core::Provenance::new(source, observed_at)?
        .with_source_at(latest_source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(normalized, provenance))
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

    let observed_at = fetched_at()?;
    let batch_id = format!("{source}:{observed_at}:quote");
    let mut quotes = Vec::with_capacity(instruments.len());
    let mut issues = Vec::new();
    for instrument in instruments {
        let key = (market(instrument)?, instrument.code().to_owned());
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
        let source_at: Option<String> = None;
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
        issues.push(format!(
            "{}: TDX quote source timestamp format is unverified",
            instrument.code()
        ));
        quotes.push(Quote::from_parts(
            instrument.clone(),
            None,
            price,
            previous_close,
            open,
            high,
            low,
            change_percent,
            volume,
            amount,
            DataStatus::Unavailable,
            source_at,
            observed_at.clone(),
            ProviderId::Tdx,
            batch_id.clone(),
        )?);
    }
    if !by_key.is_empty() {
        return Err(TdxError::InvalidData(
            "TDX returned unexpected quotes".into(),
        ));
    }
    let provenance =
        magic_market_core::Provenance::new(source, observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::best_effort(quotes, provenance, issues)?)
}

fn tdx_trade_date(date: &str) -> Result<u32, TdxError> {
    if date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return Err(TdxError::InvalidData(
            "invalid normalized trade date".into(),
        ));
    }
    date.bytes()
        .filter(|byte| *byte != b'-')
        .try_fold(0u32, |value, byte| {
            let digit = byte
                .is_ascii_digit()
                .then_some(u32::from(byte - b'0'))
                .ok_or_else(|| TdxError::InvalidData("invalid normalized trade date".into()))?;
            value
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit))
                .ok_or_else(|| TdxError::InvalidData("normalized trade date overflow".into()))
        })
}

fn trade_side(value: u32) -> TradeSide {
    match value {
        0 => TradeSide::Buy,
        1 => TradeSide::Sell,
        2 => TradeSide::Neutral,
        value => TradeSide::Unknown(value),
    }
}

fn normalize_trade_records(
    source: &str,
    request: &TradesRequest,
    records: Vec<TickData>,
) -> Result<DataBatch<Trade>, TdxError> {
    ensure_nonempty(&records)?;
    let observed_at = fetched_at()?;
    let batch_id = format!("{source}:{observed_at}:trades");
    let mut issues = Vec::new();
    let mut trades = Vec::with_capacity(records.len());
    for record in records {
        if record.time.is_empty() {
            return Err(TdxError::InvalidData("TDX trade time is empty".into()));
        }
        let price =
            Price::new(record.price).map_err(|error| TdxError::InvalidData(error.to_string()))?;
        let quantity =
            Quantity::new(record.vol).map_err(|error| TdxError::InvalidData(error.to_string()))?;
        let side = trade_side(record.buyorsell);
        let complete = !matches!(side, TradeSide::Unknown(_));
        if !complete {
            issues.push(format!(
                "{} {}: unknown TDX trade side {}",
                request.instrument().code(),
                record.time,
                record.buyorsell
            ));
        }
        let trade_at = request.date().map_or_else(
            || record.time.clone(),
            |date| format!("{date} {}", record.time),
        );
        trades.push(Trade::new(
            request.instrument().clone(),
            trade_at.clone(),
            price,
            quantity,
            (record.num != 0).then_some(record.num),
            side,
            if complete {
                DataStatus::Available
            } else {
                DataStatus::Unavailable
            },
            // Current-session packets carry a time only; historical packets
            // are qualified with the requested source date.
            Some(trade_at),
            observed_at.clone(),
            ProviderId::Tdx,
            batch_id.clone(),
        )?);
    }
    let mut provenance =
        magic_market_core::Provenance::new(source, observed_at)?.with_batch_id(batch_id)?;
    if let Some(source_at) = trades
        .last()
        .and_then(|trade| trade.source_at().map(str::to_owned))
    {
        provenance = provenance.with_source_at(source_at)?;
    }
    Ok(DataBatch::best_effort(trades, provenance, issues)?)
}

fn paginate_trades<F>(
    source: &str,
    request: &TradesRequest,
    page_size: u16,
    mut fetch: F,
) -> Result<DataBatch<Trade>, TdxError>
where
    F: FnMut(u16, u16) -> Result<Vec<TickData>, TdxError>,
{
    let mut records = Vec::with_capacity(usize::from(request.limit()));
    let mut start = 0u16;
    let mut remaining = request.limit();
    while remaining != 0 {
        let requested = remaining.min(page_size);
        let page = fetch(start, requested)?;
        if page.len() > usize::from(requested) {
            return Err(TdxError::InvalidData(
                "TDX trade page exceeds requested cardinality".into(),
            ));
        }
        let fetched = u16::try_from(page.len())
            .map_err(|_| TdxError::InvalidData("TDX trade page is too large".into()))?;
        records.extend(page);
        if fetched < requested {
            break;
        }
        remaining -= fetched;
        if remaining == 0 {
            break;
        }
        start = start
            .checked_add(fetched)
            .ok_or_else(|| TdxError::InvalidData("TDX trade offset overflow".into()))?;
    }
    normalize_trade_records(source, request, records)
}

fn board(instrument: &InstrumentId) -> Board {
    if instrument.asset_class() != magic_market_core::AssetClass::Equity {
        return Board::Unknown;
    }
    match instrument.exchange() {
        magic_market_core::Exchange::Shanghai
            if instrument.code().starts_with("688") || instrument.code().starts_with("689") =>
        {
            Board::Star
        }
        magic_market_core::Exchange::Shenzhen
            if instrument.code().starts_with("300") || instrument.code().starts_with("301") =>
        {
            Board::ChiNext
        }
        magic_market_core::Exchange::Shanghai | magic_market_core::Exchange::Shenzhen => {
            Board::Main
        }
        magic_market_core::Exchange::Beijing => Board::Beijing,
    }
}

fn st_flag(name: &str) -> Option<bool> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let uppercase = name.to_ascii_uppercase();
    Some(
        uppercase.starts_with("ST")
            || uppercase.starts_with("*ST")
            || uppercase.starts_with("S*ST")
            || uppercase.starts_with("SST"),
    )
}

fn fetch_security_records<Count, Page>(
    instruments: &[InstrumentId],
    mut count: Count,
    mut page: Page,
) -> Result<Vec<(u8, SecurityInfo)>, TdxError>
where
    Count: FnMut(u8) -> Result<u16, TdxError>,
    Page: FnMut(u8, u16) -> Result<Vec<SecurityInfo>, TdxError>,
{
    if instruments.is_empty() {
        return Err(TdxError::InvalidData(
            "TDX security metadata request is empty".into(),
        ));
    }
    let mut requested = HashSet::new();
    let mut markets = HashSet::new();
    for instrument in instruments {
        let instrument_market = market(instrument)?;
        if !requested.insert((instrument_market, instrument.code().to_owned())) {
            return Err(TdxError::InvalidData(
                "TDX security metadata request contains duplicates".into(),
            ));
        }
        markets.insert(instrument_market);
    }

    let mut records = Vec::with_capacity(instruments.len());
    for instrument_market in markets {
        let expected = usize::from(count(instrument_market)?);
        let market_requested = requested
            .iter()
            .filter(|(value, _)| *value == instrument_market)
            .count();
        let mut seen = 0usize;
        let mut found = 0usize;
        while seen < expected && found < market_requested {
            let start = u16::try_from(seen)
                .map_err(|_| TdxError::InvalidData("TDX security list offset overflow".into()))?;
            let source_page = page(instrument_market, start)?;
            if source_page.is_empty() || seen + source_page.len() > expected {
                return Err(TdxError::InvalidData(
                    "TDX security list cardinality mismatch".into(),
                ));
            }
            seen += source_page.len();
            for record in source_page {
                if requested.contains(&(instrument_market, record.code.clone())) {
                    records.push((instrument_market, record));
                    found += 1;
                }
            }
        }
    }
    Ok(records)
}

fn validate_security_metadata_request(instruments: &[InstrumentId]) -> Result<(), TdxError> {
    if instruments
        .iter()
        .any(|instrument| instrument.exchange() == magic_market_core::Exchange::Beijing)
    {
        return Err(TdxError::Unsupported(
            "TDX market=2 serves Beijing quotes, bars, minute data, trades and order books, \
             but live-verified servers close the security-list request required for Beijing \
             security metadata"
                .into(),
        ));
    }
    Ok(())
}

fn normalize_security_metadata(
    source: &str,
    instruments: &[InstrumentId],
    records: Vec<(u8, SecurityInfo)>,
) -> Result<DataBatch<SecurityMetadata>, TdxError> {
    let mut by_key = HashMap::with_capacity(records.len());
    for (record_market, record) in records {
        let key = (record_market, record.code.clone());
        if by_key.insert(key, record).is_some() {
            return Err(TdxError::InvalidData(
                "TDX returned duplicate security metadata".into(),
            ));
        }
    }

    let observed_at = fetched_at()?;
    let batch_id = format!("{source}:{observed_at}:security-metadata");
    let mut normalized = Vec::with_capacity(instruments.len());
    let mut issues = Vec::new();
    for instrument in instruments {
        let key = (market(instrument)?, instrument.code().to_owned());
        let record = by_key.remove(&key).ok_or_else(|| {
            TdxError::InvalidData(format!(
                "TDX omitted requested security metadata for {}",
                instrument.code()
            ))
        })?;
        let name = (!record.name.trim().is_empty()).then(|| record.name.trim().to_owned());
        let is_st = if instrument.asset_class() == magic_market_core::AssetClass::Equity {
            name.as_deref().and_then(st_flag)
        } else {
            None
        };
        if name.is_none() {
            issues.push(format!("{}: security name unavailable", instrument.code()));
        }
        issues.push(format!(
            "{}: board is derived from exchange/code because the TDX list packet has no board field",
            instrument.code()
        ));
        issues.push(format!("{}: listing date unavailable", instrument.code()));
        issues.push(format!(
            "{}: source-backed price-limit rule and version unavailable",
            instrument.code()
        ));
        issues.push(format!(
            "{}: source timestamp unavailable",
            instrument.code()
        ));
        normalized.push(SecurityMetadata::new(
            instrument.clone(),
            name,
            Some(board(instrument)),
            is_st,
            None,
            PriceLimitRule::new(None, None)?,
            DataStatus::Unavailable,
            None,
            observed_at.clone(),
            ProviderId::Tdx,
            batch_id.clone(),
        )?);
    }
    if !by_key.is_empty() {
        return Err(TdxError::InvalidData(
            "TDX returned unexpected security metadata".into(),
        ));
    }
    let provenance =
        magic_market_core::Provenance::new(source, observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::best_effort(normalized, provenance, issues)?)
}

impl SecurityMetadataProvider for TdxHqClient {
    type Error = TdxError;

    fn security_metadata(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        security_metadata_with(self, "tdx", instruments)
    }
}

impl SecurityMetadataProvider for crate::TdxSmartClient {
    type Error = TdxError;

    fn security_metadata(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        security_metadata_with(self, "tdx", instruments)
    }
}

impl HistoricalBars for TdxHqClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        historical_bars_with(self, "tdx", request)
    }
}

impl MinuteData for TdxHqClient {
    type Error = TdxError;

    fn minute_data(
        &self,
        request: &MinuteDataRequest,
    ) -> Result<DataBatch<MinutePoint>, Self::Error> {
        minute_data_with(self, "tdx", request)
    }
}

impl RealtimeQuotes for TdxHqClient {
    type Quote = Quote;
    type Error = TdxError;
    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        realtime_quotes_with(self, "tdx", instruments)
    }
}

impl Trades for TdxHqClient {
    type Error = TdxError;

    fn trades(&self, request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        trades_with(self, "tdx-current", "tdx-history", request)
    }
}

fn book_level(price: f64, quantity: f64) -> Result<BookLevel, TdxError> {
    match (price, quantity) {
        (price, quantity) if !price.is_finite() || !quantity.is_finite() => Err(
            TdxError::InvalidData("TDX order-book level must be finite".into()),
        ),
        (price, quantity) if price < 0.0 || quantity < 0.0 => Err(TdxError::InvalidData(
            "TDX order-book level must be non-negative".into(),
        )),
        (0.0, _) => Ok(BookLevel::unavailable()),
        (price, quantity) => Ok(BookLevel::new(
            Some(Price::new(price)?),
            Some(Quantity::new(quantity)?),
        )?),
    }
}

fn book_depth(levels: &[BookLevel; 5]) -> Result<Option<Quantity>, TdxError> {
    let mut found = false;
    let total =
        levels
            .iter()
            .filter_map(|level| level.quantity())
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

pub(crate) fn order_book_pairs<'a>(
    instruments: &'a [InstrumentId],
    provider: &str,
) -> Result<Vec<(u8, &'a str)>, TdxError> {
    if instruments.is_empty() {
        return Err(TdxError::InvalidData(format!(
            "{provider} order-book request is empty"
        )));
    }
    let mut requested = HashSet::with_capacity(instruments.len());
    instruments
        .iter()
        .map(|instrument| {
            let pair = (market(instrument)?, instrument.code());
            if !requested.insert((pair.0, pair.1.to_owned())) {
                return Err(TdxError::InvalidData(format!(
                    "{provider} order-book request contains a duplicate instrument"
                )));
            }
            Ok(pair)
        })
        .collect()
}

pub(crate) fn ordered_order_book_quotes<'a>(
    instruments: &'a [InstrumentId],
    quotes: Vec<SecurityQuote>,
    provider: &str,
) -> Result<Vec<(&'a InstrumentId, SecurityQuote)>, TdxError> {
    let requested: HashSet<(u8, String)> = order_book_pairs(instruments, provider)?
        .into_iter()
        .map(|(market, code)| (market, code.to_owned()))
        .collect();

    let mut by_instrument = HashMap::with_capacity(quotes.len());
    for quote in quotes {
        let key = (quote.market, quote.code.clone());
        if !requested.contains(&key) {
            return Err(TdxError::InvalidData(format!(
                "{provider} order-book response contains unexpected instrument {}:{}",
                key.0, key.1
            )));
        }
        if by_instrument.insert(key.clone(), quote).is_some() {
            return Err(TdxError::InvalidData(format!(
                "{provider} order-book response contains duplicate instrument {}:{}",
                key.0, key.1
            )));
        }
    }

    instruments
        .iter()
        .map(|instrument| {
            let key = (market(instrument)?, instrument.code().to_owned());
            let quote = by_instrument.remove(&key).ok_or_else(|| {
                TdxError::InvalidData(format!(
                    "{provider} order-book response is missing instrument {}:{}",
                    key.0, key.1
                ))
            })?;
            Ok((instrument, quote))
        })
        .collect()
}

pub(crate) fn normalize_order_books(
    provider: &str,
    source: &str,
    instruments: &[InstrumentId],
    quotes: Vec<SecurityQuote>,
) -> Result<DataBatch<OrderBook>, TdxError> {
    let ordered = ordered_order_book_quotes(instruments, quotes, provider)?;
    let observed_at = fetched_at()?;
    let batch_id = format!("{source}:{observed_at}:order-book");
    let mut books = Vec::with_capacity(ordered.len());
    let mut issues = Vec::new();
    for (id, quote) in ordered {
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
        let levels_complete = bids
            .iter()
            .chain(&asks)
            .all(|level| level.price().is_some());
        if !levels_complete {
            issues.push(format!(
                "{}: one or more normalized order-book fields unavailable",
                id.code()
            ));
        }
        issues.push(format!(
            "{}: TDX order-book source timestamp format is unverified",
            id.code()
        ));
        books.push(OrderBook::new(
            id.clone(),
            bids,
            asks,
            total_bid_quantity,
            total_ask_quantity,
            DataStatus::Unavailable,
            None,
            observed_at.clone(),
            ProviderId::Tdx,
            batch_id.clone(),
        )?);
    }
    let provenance =
        magic_market_core::Provenance::new(source, observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::best_effort(books, provenance, issues)?)
}

fn order_books_with(
    query: &impl BlockingTdxQuery,
    provider: &str,
    source: &str,
    instruments: &[InstrumentId],
) -> Result<DataBatch<OrderBook>, TdxError> {
    let pairs = order_book_pairs(instruments, provider)?;
    let quotes = query.security_quotes(&pairs)?;
    normalize_order_books(provider, source, instruments, quotes)
}

impl OrderBooks for TdxHqClient {
    type Error = TdxError;
    fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        order_books_with(self, "TDX", "tdx", instruments)
    }
}

impl OrderBooks for crate::TdxSmartClient {
    type Error = TdxError;
    fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        order_books_with(self, "TDX smart", "tdx-smart", instruments)
    }
}

impl HistoricalBars for crate::TdxSmartClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        historical_bars_with(self, "tdx-smart", request)
    }
}

impl MinuteData for crate::TdxSmartClient {
    type Error = TdxError;

    fn minute_data(
        &self,
        request: &MinuteDataRequest,
    ) -> Result<DataBatch<MinutePoint>, Self::Error> {
        minute_data_with(self, "tdx", request)
    }
}

impl RealtimeQuotes for crate::TdxSmartClient {
    type Quote = Quote;
    type Error = TdxError;
    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        realtime_quotes_with(self, "tdx-smart", instruments)
    }
}

impl Trades for crate::TdxSmartClient {
    type Error = TdxError;

    fn trades(&self, request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        trades_with(self, "tdx-current", "tdx-history", request)
    }
}

impl HistoricalBars for crate::TdxDirectClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        historical_bars_with(self, "tdx-direct", request)
    }
}

impl RealtimeQuotes for crate::TdxDirectClient {
    type Quote = Quote;
    type Error = TdxError;
    fn realtime_quotes(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        realtime_quotes_with(self, "tdx-direct", instruments)
    }
}

impl Trades for crate::TdxDirectClient {
    type Error = TdxError;

    fn trades(&self, request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        trades_with(
            self,
            "tdx-direct-current",
            "tdx-direct-history",
            request,
        )
    }
}

impl AsyncHistoricalBars for crate::AsyncTdxHqClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    async fn historical_bars_async(
        &self,
        request: &BarsRequest,
    ) -> Result<DataBatch<Self::Bar>, Self::Error> {
        historical_bars_async_with(self, "tdx-async", request).await
    }
}

impl AsyncRealtimeQuotes for crate::AsyncTdxHqClient {
    type Quote = Quote;
    type Error = TdxError;
    async fn realtime_quotes_async(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<Self::Quote>, Self::Error> {
        realtime_quotes_async_with(self, "tdx-async", instruments).await
    }
}

impl AsyncTrades for crate::AsyncTdxHqClient {
    type Error = TdxError;

    async fn trades_async(&self, request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        let historical_date = request.date().map(tdx_trade_date).transpose()?;
        let request_market = market(request.instrument())?;
        let page_size = if historical_date.is_some() {
            HISTORICAL_TRADE_PAGE_SIZE
        } else {
            CURRENT_TRADE_PAGE_SIZE
        };
        let mut records = Vec::with_capacity(usize::from(request.limit()));
        let mut start = 0u16;
        let mut remaining = request.limit();
        while remaining != 0 {
            let requested = remaining.min(page_size);
            let page = match historical_date {
                Some(date) => {
                    self.get_history_transaction_data(
                        request_market,
                        request.instrument().code(),
                        start,
                        requested,
                        date,
                    )
                    .await?
                }
                None => {
                    self.get_transaction_data(
                        request_market,
                        request.instrument().code(),
                        start,
                        requested,
                    )
                    .await?
                }
            };
            if page.len() > usize::from(requested) {
                return Err(TdxError::InvalidData(
                    "TDX async trade page exceeds requested cardinality".into(),
                ));
            }
            let fetched = u16::try_from(page.len())
                .map_err(|_| TdxError::InvalidData("TDX trade page is too large".into()))?;
            records.extend(page);
            if fetched < requested {
                break;
            }
            remaining -= fetched;
            if remaining == 0 {
                break;
            }
            start = start
                .checked_add(fetched)
                .ok_or_else(|| TdxError::InvalidData("TDX trade offset overflow".into()))?;
        }
        normalize_trade_records(
            if historical_date.is_some() {
                "tdx-async-history"
            } else {
                "tdx-async-current"
            },
            request,
            records,
        )
    }
}

#[cfg(test)]
#[path = "../tests/internal/adapter.rs"]
mod tests;

macro_rules! unsupported_p0 {
    ($client:ty) => {
        impl MoneyFlows for $client {
            type Error = TdxError;
            fn money_flows(
                &self,
                _instruments: &[InstrumentId],
            ) -> Result<DataBatch<MoneyFlow>, Self::Error> {
                Err(TdxError::Unsupported(
                    "TDX quote/trade packets do not provide auditable main/net inflow fields or \
                     source methodology required by MoneyFlow"
                        .into(),
                ))
            }
        }
        impl Auctions for $client {
            type Error = TdxError;
            fn auction_snapshots(
                &self,
                _instruments: &[InstrumentId],
            ) -> Result<DataBatch<AuctionSnapshot>, Self::Error> {
                Err(TdxError::Unsupported(
                    "TDX packets do not provide the standardized indicative price and matched/\
                     unmatched quantities required by AuctionSnapshot"
                        .into(),
                ))
            }
        }
    };
}
unsupported_p0!(TdxHqClient);
unsupported_p0!(crate::TdxSmartClient);
