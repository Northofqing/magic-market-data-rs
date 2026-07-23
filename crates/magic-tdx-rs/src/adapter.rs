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
        validate_security_metadata_request(instruments)?;
        let records = fetch_security_records(
            instruments,
            |value| self.get_security_count(value),
            |value, start| self.get_security_list(value, start),
        )?;
        normalize_security_metadata("tdx", instruments, records)
    }
}

impl SecurityMetadataProvider for crate::TdxSmartClient {
    type Error = TdxError;

    fn security_metadata(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        <TdxHqClient as SecurityMetadataProvider>::security_metadata(self.inner(), instruments)
    }
}

impl HistoricalBars for TdxHqClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        reject_unsupported_bar_range(request)?;
        let records = self.get_security_bars(
            category(request.interval())?,
            market(request.instrument())?,
            request.instrument().code(),
            0,
            request.limit(),
            0,
        )?;
        strict_bars("tdx", records)
    }
}

impl MinuteData for TdxHqClient {
    type Error = TdxError;

    fn minute_data(
        &self,
        request: &MinuteDataRequest,
    ) -> Result<DataBatch<MinutePoint>, Self::Error> {
        let (date, records) = match request.date() {
            Some(date) => (
                date.to_owned(),
                self.get_history_minute_time_data(
                    market(request.instrument())?,
                    request.instrument().code(),
                    compact_date(date)?,
                )?,
            ),
            None => {
                let compact = crate::net::utils::today_yyyymmdd();
                (
                    display_date(compact)?,
                    self.get_minute_time_data(
                        market(request.instrument())?,
                        request.instrument().code(),
                    )?,
                )
            }
        };
        normalize_minute_records("tdx", request.instrument(), &date, records)
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
            .map(|id| market(id).map(|market| (market, id.code())))
            .collect::<Result<_, _>>()?;
        let records = self.get_security_quotes(&pairs)?;
        normalize_quotes("tdx", instruments, records)
    }
}

impl Trades for TdxHqClient {
    type Error = TdxError;

    fn trades(&self, request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        let request_market = market(request.instrument())?;
        match request.date() {
            Some(date) => {
                let date = tdx_trade_date(date)?;
                paginate_trades(
                    "tdx-history",
                    request,
                    HISTORICAL_TRADE_PAGE_SIZE,
                    |start, count| {
                        self.get_history_transaction_data(
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
                "tdx-current",
                request,
                CURRENT_TRADE_PAGE_SIZE,
                |start, count| {
                    self.get_transaction_data(
                        request_market,
                        request.instrument().code(),
                        start,
                        count,
                    )
                },
            ),
        }
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

impl OrderBooks for TdxHqClient {
    type Error = TdxError;
    fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        let pairs = order_book_pairs(instruments, "TDX")?;
        let quotes = self.get_security_quotes(&pairs)?;
        let ordered = ordered_order_book_quotes(instruments, quotes, "TDX")?;
        let observed_at = fetched_at()?;
        let batch_id = format!("tdx:{observed_at}:order-book");
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
                magic_market_core::DataStatus::Unavailable,
                None,
                observed_at.clone(),
                magic_market_core::ProviderId::Tdx,
                batch_id.clone(),
            )?);
        }
        let provenance =
            magic_market_core::Provenance::new("tdx", observed_at)?.with_batch_id(batch_id)?;
        Ok(DataBatch::best_effort(books, provenance, issues)?)
    }
}

impl OrderBooks for crate::TdxSmartClient {
    type Error = TdxError;
    fn order_books(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<OrderBook>, Self::Error> {
        let pairs = order_book_pairs(instruments, "TDX smart")?;
        let quotes = self.get_security_quotes(&pairs)?;
        let ordered = ordered_order_book_quotes(instruments, quotes, "TDX smart")?;
        let observed_at = fetched_at()?;
        let batch_id = format!("tdx-smart:{observed_at}:order-book");
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
                magic_market_core::DataStatus::Unavailable,
                None,
                observed_at.clone(),
                magic_market_core::ProviderId::Tdx,
                batch_id.clone(),
            )?);
        }
        let provenance = magic_market_core::Provenance::new("tdx-smart", observed_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::best_effort(books, provenance, issues)?)
    }
}

impl HistoricalBars for crate::TdxSmartClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        reject_unsupported_bar_range(request)?;
        let records = self.get_security_bars(
            category(request.interval())?,
            market(request.instrument())?,
            request.instrument().code(),
            0,
            request.limit(),
            0,
        )?;
        strict_bars("tdx-smart", records)
    }
}

impl MinuteData for crate::TdxSmartClient {
    type Error = TdxError;

    fn minute_data(
        &self,
        request: &MinuteDataRequest,
    ) -> Result<DataBatch<MinutePoint>, Self::Error> {
        <TdxHqClient as MinuteData>::minute_data(self.inner(), request)
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
            .map(|id| market(id).map(|market| (market, id.code())))
            .collect::<Result<_, _>>()?;
        let records = self.get_security_quotes(&pairs)?;
        normalize_quotes("tdx-smart", instruments, records)
    }
}

impl Trades for crate::TdxSmartClient {
    type Error = TdxError;

    fn trades(&self, request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        <TdxHqClient as Trades>::trades(self.inner(), request)
    }
}

impl HistoricalBars for crate::TdxDirectClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        reject_unsupported_bar_range(request)?;
        let records = self.get_security_bars(
            category(request.interval())?,
            market(request.instrument())?,
            request.instrument().code(),
            0,
            request.limit(),
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
            .map(|id| market(id).map(|market| (market, id.code())))
            .collect::<Result<_, _>>()?;
        let records = self.get_security_quotes(&pairs)?;
        normalize_quotes("tdx-direct", instruments, records)
    }
}

impl Trades for crate::TdxDirectClient {
    type Error = TdxError;

    fn trades(&self, request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        let request_market = market(request.instrument())?;
        match request.date() {
            Some(date) => {
                let date = tdx_trade_date(date)?;
                paginate_trades(
                    "tdx-direct-history",
                    request,
                    HISTORICAL_TRADE_PAGE_SIZE,
                    |start, count| {
                        self.get_history_transaction_data(
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
                "tdx-direct-current",
                request,
                CURRENT_TRADE_PAGE_SIZE,
                |start, count| {
                    self.get_transaction_data(
                        request_market,
                        request.instrument().code(),
                        start,
                        count,
                    )
                },
            ),
        }
    }
}

impl AsyncHistoricalBars for crate::AsyncTdxHqClient {
    type Bar = SecurityBar;
    type Error = TdxError;
    async fn historical_bars_async(
        &self,
        request: &BarsRequest,
    ) -> Result<DataBatch<Self::Bar>, Self::Error> {
        reject_unsupported_bar_range(request)?;
        let records = self
            .get_security_bars(
                category(request.interval())?,
                market(request.instrument())?,
                request.instrument().code(),
                0,
                request.limit(),
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
            .map(|id| market(id).map(|market| (market, id.code())))
            .collect::<Result<_, _>>()?;
        let records = self.get_security_quotes(&pairs).await?;
        normalize_quotes("tdx-async", instruments, records)
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
mod tests {
    use super::*;

    #[test]
    fn rejects_bar_ranges_instead_of_silently_ignoring_them() {
        let request = BarsRequest::new(instrument("600396"), BarInterval::Day, 5)
            .unwrap()
            .with_range("2026-07-01", "2026-07-22")
            .unwrap();
        assert!(matches!(
            reject_unsupported_bar_range(&request),
            Err(TdxError::Unsupported(_))
        ));
    }

    #[test]
    fn maps_standard_one_minute_monthly_and_yearly_categories_exactly() {
        assert_eq!(category(BarInterval::Minute1).unwrap(), 8);
        assert_eq!(category(BarInterval::Month).unwrap(), 6);
        assert_eq!(category(BarInterval::Year).unwrap(), 11);
    }

    #[test]
    fn order_book_levels_preserve_absence_atomically() {
        let absent = book_level(0.0, 0.0).unwrap();
        assert!(absent.price().is_none());
        assert!(absent.quantity().is_none());
        let half_present = book_level(0.0, 1.0).unwrap();
        assert!(half_present.price().is_none());
        assert!(half_present.quantity().is_none());
        assert!(book_level(10.0, -1.0).is_err());
    }
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
    fn order_book_quotes_are_keyed_by_market_and_code() {
        let instruments = [instrument("600001"), instrument("600002")];
        let ordered = ordered_order_book_quotes(
            &instruments,
            vec![source_quote("600002", 102.0), source_quote("600001", 101.0)],
            "test",
        )
        .unwrap();
        assert_eq!(ordered[0].0.code(), "600001");
        assert_eq!(ordered[0].1.price, 101.0);
        assert_eq!(ordered[1].0.code(), "600002");
        assert_eq!(ordered[1].1.price, 102.0);

        assert!(ordered_order_book_quotes(&[], Vec::new(), "test").is_err());
        assert!(ordered_order_book_quotes(
            &[instrument("600001"), instrument("600001")],
            vec![source_quote("600001", 101.0)],
            "test",
        )
        .is_err());
        assert!(ordered_order_book_quotes(
            &instruments,
            vec![source_quote("600001", 101.0), source_quote("600001", 102.0)],
            "test",
        )
        .is_err());
        assert!(ordered_order_book_quotes(
            &instruments,
            vec![source_quote("600001", 101.0), source_quote("600003", 103.0)],
            "test",
        )
        .is_err());
        assert!(ordered_order_book_quotes(
            &instruments,
            vec![source_quote("600001", 101.0)],
            "test",
        )
        .is_err());
    }

    fn source_trade(index: u32, side: u32) -> TickData {
        TickData {
            time: format!("10:00:{index:02}"),
            price: 1_300.0 + f64::from(index),
            vol: 100.0 + f64::from(index),
            num: index + 1,
            buyorsell: side,
            reserved: 0,
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
        assert_eq!(batch.records()[0].instrument().code(), "600001");
        assert_eq!(batch.records()[0].price(), Price::new(102.0).unwrap());
        assert_eq!(
            batch.records()[0].change_percent(),
            Some(Ratio::new(2.0, RatioUnit::Percent).unwrap())
        );
        assert_eq!(batch.records()[0].status(), DataStatus::Unavailable);
        assert!(batch.records()[0].name().is_none());
        assert!(batch.records()[0].source_at().is_none());
        assert!(batch.provenance().source_at().is_none());
        assert_eq!(batch.quality().issues().len(), 6);
    }

    #[test]
    fn normalized_quotes_reject_duplicates_and_missing_records() {
        let duplicated = [instrument("600001"), instrument("600001")];
        assert!(normalize_quotes("test", &duplicated, Vec::new()).is_err());

        let requested = [instrument("600001"), instrument("600002")];
        assert!(normalize_quotes("test", &requested, vec![source_quote("600001", 102.0)]).is_err());
    }

    #[test]
    fn normalizes_only_source_backed_security_metadata() {
        let star = instrument("688001");
        let chinext = InstrumentId::new(Exchange::Shenzhen, "300001", AssetClass::Equity).unwrap();
        let records = vec![
            (
                0,
                SecurityInfo {
                    code: "300001".into(),
                    volunit: 100,
                    decimal_point: 2,
                    name: "*ST示例".into(),
                    pre_close: 10.0,
                },
            ),
            (
                1,
                SecurityInfo {
                    code: "688001".into(),
                    volunit: 100,
                    decimal_point: 2,
                    name: "科创示例".into(),
                    pre_close: 20.0,
                },
            ),
        ];

        let batch = normalize_security_metadata("test", &[star, chinext], records).unwrap();
        assert_eq!(batch.records()[0].board(), Some(Board::Star));
        assert_eq!(batch.records()[0].is_st(), Some(false));
        assert_eq!(batch.records()[1].board(), Some(Board::ChiNext));
        assert_eq!(batch.records()[1].is_st(), Some(true));
        assert!(batch
            .records()
            .iter()
            .all(|record| record.listed_on().is_none()
                && record.price_limit().percent().is_none()
                && record.price_limit().version().is_none()
                && record.status() == DataStatus::Unavailable));
        assert!(!batch.quality().is_complete());
    }

    #[test]
    fn beijing_uses_the_live_verified_tdx_market_number() {
        let beijing = InstrumentId::new(Exchange::Beijing, "920001", AssetClass::Equity).unwrap();
        assert_eq!(market(&beijing).unwrap(), 2);
        assert_eq!(market(&instrument("600001")).unwrap(), 1);
        let shenzhen = InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap();
        assert_eq!(market(&shenzhen).unwrap(), 0);
    }

    #[test]
    fn rejects_beijing_security_metadata_before_transport() {
        let beijing = InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap();
        let error = validate_security_metadata_request(&[beijing]).unwrap_err();
        assert!(matches!(error, TdxError::Unsupported(_)));
        assert!(error.to_string().contains("security-list"));
    }

    #[test]
    fn normalizes_tdx_minute_rows_into_cumulative_chronological_points() {
        let records = vec![
            MinuteTimePrice {
                time: "09:32".into(),
                price: 15.6,
                avg_price: 15.5,
                vol: 20.0,
            },
            MinuteTimePrice {
                time: "09:31".into(),
                price: 15.4,
                avg_price: 15.4,
                vol: 10.0,
            },
        ];
        let batch =
            normalize_minute_records("test", &instrument("600396"), "2026-07-23", records).unwrap();
        assert_eq!(batch.records()[0].minute_at(), "2026-07-23 09:31");
        assert_eq!(batch.records()[0].cumulative_quantity().get(), 10.0);
        assert_eq!(batch.records()[1].cumulative_quantity().get(), 30.0);
        assert_eq!(
            batch.records()[1].source_at(),
            Some("2026-07-23T09:32:00+08:00")
        );
        assert!(batch.records()[1].cumulative_amount().is_none());
    }

    #[test]
    fn paginates_and_normalizes_historical_trades() {
        let request = TradesRequest::new(instrument("600519"), 5)
            .unwrap()
            .with_date("2026-07-21")
            .unwrap();
        let mut calls = Vec::new();
        let batch = paginate_trades("test", &request, 2, |start, count| {
            calls.push((start, count));
            Ok((start..start + count)
                .map(|index| source_trade(u32::from(index), u32::from(index % 3)))
                .collect())
        })
        .unwrap();
        assert_eq!(calls, vec![(0, 2), (2, 2), (4, 1)]);
        assert_eq!(batch.records().len(), 5);
        assert_eq!(batch.records()[0].trade_at(), "2026-07-21 10:00:00");
        assert_eq!(batch.records()[0].source_at(), Some("2026-07-21 10:00:00"));
        assert_eq!(batch.records()[0].side(), TradeSide::Buy);
        assert_eq!(batch.records()[1].side(), TradeSide::Sell);
        assert_eq!(batch.records()[2].side(), TradeSide::Neutral);
    }

    #[test]
    fn marks_unknown_trade_side_without_dropping_the_record() {
        let request = TradesRequest::new(instrument("600519"), 1).unwrap();
        let batch = normalize_trade_records("test", &request, vec![source_trade(0, 9)]).unwrap();
        assert_eq!(batch.records()[0].side(), TradeSide::Unknown(9));
        assert_eq!(batch.records()[0].status(), DataStatus::Unavailable);
        assert_eq!(batch.quality().issues().len(), 1);
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
