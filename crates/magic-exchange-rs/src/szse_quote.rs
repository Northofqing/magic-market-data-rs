use crate::ExchangeError;
use magic_market_core::{
    AssetClass, BookLevel, DataStatus, Exchange, InstrumentId, IsoDate, Money, OrderBook, Price,
    ProviderId, Quantity, Quote, Ratio, RatioUnit,
};
use serde::Deserialize;

pub(crate) const SZSE_QUOTE_ENDPOINT: &str = "https://www.szse.cn/api/market/ssjjhq/getTimeData";
pub(crate) const MAX_QUOTE_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct SzseQuoteSnapshot {
    quote: Quote,
    order_book: OrderBook,
}

impl SzseQuoteSnapshot {
    pub(crate) fn into_parts(self) -> (Quote, OrderBook) {
        (self.quote, self.order_book)
    }
}

pub(crate) fn build_quote_url(instrument: &InstrumentId) -> Result<String, ExchangeError> {
    validate_instrument(instrument)?;
    Ok(format!(
        "{SZSE_QUOTE_ENDPOINT}?marketId=1&code={}",
        instrument.code()
    ))
}

pub(crate) fn parse_quote_snapshot(
    instrument: &InstrumentId,
    body: &[u8],
    observed_at: &str,
    batch_id: &str,
) -> Result<SzseQuoteSnapshot, ExchangeError> {
    validate_instrument(instrument)?;
    if body.len() > MAX_QUOTE_RESPONSE_BYTES {
        return Err(schema(format!(
            "SZSE quote response exceeds {MAX_QUOTE_RESPONSE_BYTES} bytes"
        )));
    }
    let wire: QuoteResponse =
        serde_json::from_slice(body).map_err(|error| ExchangeError::Decode(error.to_string()))?;
    validate_response_identity(&wire, instrument)?;
    validate_phase(
        "tradingPhaseCode1",
        &wire.data.trading_phase_code_1,
        &["00", "02", "03", "04", "05", "06", "07", "11", "12"],
    )?;
    validate_phase(
        "tradingPhaseCode2",
        &wire.data.trading_phase_code_2,
        &["00", "04", "06", "08", "09", "11"],
    )?;

    let source_at = parse_source_at(&wire.datetime, &wire.data.market_time)?;
    let previous_close = positive_decimal(&wire.data.close, "close")?;
    let open = positive_decimal(&wire.data.open, "open")?;
    let current = positive_decimal(&wire.data.now, "now")?;
    let high = positive_decimal(&wire.data.high, "high")?;
    let low = positive_decimal(&wire.data.low, "low")?;
    let delta = decimal(&wire.data.delta, "delta")?;
    let delta_percent = decimal(&wire.data.delta_percent, "deltaPercent")?;
    validate_quote_shape(
        previous_close,
        open,
        current,
        high,
        low,
        delta,
        delta_percent,
        &wire.data.delta,
        &wire.data.delta_percent,
    )?;
    validate_nonnegative(wire.data.volume, "volume")?;
    validate_nonnegative(wire.data.amount, "amount")?;

    let name = required_text(&wire.data.name, "name")?;
    let quote = Quote::from_parts(
        instrument.clone(),
        Some(name),
        Price::new(current)?,
        Some(Price::new(previous_close)?),
        Some(Price::new(open)?),
        Some(Price::new(high)?),
        Some(Price::new(low)?),
        Some(Ratio::new(delta_percent, RatioUnit::Percent)?),
        Quantity::new(wire.data.volume)?,
        Some(Money::new(wire.data.amount)?),
        DataStatus::Available,
        Some(source_at.clone()),
        observed_at,
        ProviderId::Szse,
        batch_id,
    )?;

    let (asks, bids) = parse_book(&wire.data.sell_buy_5)?;
    let total_ask_quantity = visible_total(&asks)?;
    let total_bid_quantity = visible_total(&bids)?;
    let complete = asks
        .iter()
        .chain(&bids)
        .all(|level| level.price().is_some());
    let order_book = OrderBook::new(
        instrument.clone(),
        bids,
        asks,
        total_bid_quantity,
        total_ask_quantity,
        if complete {
            DataStatus::Available
        } else {
            DataStatus::Unavailable
        },
        Some(source_at),
        observed_at,
        ProviderId::Szse,
        batch_id,
    )?;

    Ok(SzseQuoteSnapshot { quote, order_book })
}

#[derive(Debug, Deserialize)]
struct QuoteResponse {
    datetime: String,
    code: String,
    message: String,
    data: QuoteData,
}

#[derive(Debug, Deserialize)]
struct QuoteData {
    code: String,
    name: String,
    #[serde(rename = "groupId")]
    group_id: u32,
    close: String,
    open: String,
    now: String,
    high: String,
    low: String,
    volume: f64,
    amount: f64,
    delta: String,
    #[serde(rename = "deltaPercent")]
    delta_percent: String,
    #[serde(rename = "marketTime")]
    market_time: String,
    #[serde(rename = "tradingPhaseCode1")]
    trading_phase_code_1: String,
    #[serde(rename = "tradingPhaseCode2")]
    trading_phase_code_2: String,
    #[serde(rename = "sellbuy5")]
    sell_buy_5: Vec<BookLevelWire>,
}

#[derive(Debug, Deserialize)]
struct BookLevelWire {
    #[serde(default)]
    price: Option<String>,
    #[serde(default)]
    volume: Option<f64>,
}

fn validate_instrument(instrument: &InstrumentId) -> Result<(), ExchangeError> {
    if instrument.exchange() != Exchange::Shenzhen || instrument.asset_class() != AssetClass::Equity
    {
        return Err(ExchangeError::InvalidRequest(
            "SZSE Quote requires a Shenzhen equity".into(),
        ));
    }
    let code = instrument.code();
    if code.len() != 6
        || !code.bytes().all(|byte| byte.is_ascii_digit())
        || !matches!(code.as_bytes()[0], b'0' | b'3')
    {
        return Err(ExchangeError::InvalidRequest(
            "SZSE equity code must be six digits beginning with 0 or 3".into(),
        ));
    }
    Ok(())
}

fn validate_response_identity(
    wire: &QuoteResponse,
    instrument: &InstrumentId,
) -> Result<(), ExchangeError> {
    if wire.code != "0" {
        return Err(schema(format!(
            "SZSE quote response code is {}, expected 0",
            wire.code
        )));
    }
    required_text(&wire.message, "message")?;
    if wire.data.code != instrument.code() {
        return Err(schema(format!(
            "SZSE quote response code {} contradicts requested {}",
            wire.data.code,
            instrument.code()
        )));
    }
    if !matches!(wire.data.group_id, 1 | 17) {
        return Err(schema(format!(
            "SZSE quote response has unverified equity group {}",
            wire.data.group_id
        )));
    }
    Ok(())
}

fn validate_phase(field: &str, value: &str, allowed: &[&str]) -> Result<(), ExchangeError> {
    if !allowed.contains(&value) {
        return Err(schema(format!(
            "SZSE quote {field} has unverified value {value}"
        )));
    }
    Ok(())
}

fn parse_source_at(root_time: &str, market_time: &str) -> Result<String, ExchangeError> {
    if !is_market_time(market_time) {
        return Err(schema("SZSE marketTime is not YYYY-MM-DD HH:MM:SS"));
    }
    if root_time.len() != 16
        || !root_time.is_ascii()
        || root_time.as_bytes()[10] != b' '
        || root_time != &market_time[..16]
    {
        return Err(schema(
            "SZSE response datetime contradicts the source marketTime",
        ));
    }
    Ok(format!(
        "{}T{}+08:00",
        &market_time[..10],
        &market_time[11..]
    ))
}

fn is_market_time(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 19
        || !value.is_ascii()
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b' '
        || bytes[13] != b':'
        || bytes[16] != b':'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7 | 10 | 13 | 16) || byte.is_ascii_digit())
        || IsoDate::new(&value[..10]).is_err()
    {
        return false;
    }
    let hour = value[11..13].parse::<u32>().unwrap_or(24);
    let minute = value[14..16].parse::<u32>().unwrap_or(60);
    let second = value[17..19].parse::<u32>().unwrap_or(60);
    hour < 24 && minute < 60 && second < 60
}

#[allow(clippy::too_many_arguments)]
fn validate_quote_shape(
    previous_close: f64,
    open: f64,
    current: f64,
    high: f64,
    low: f64,
    delta: f64,
    delta_percent: f64,
    delta_text: &str,
    delta_percent_text: &str,
) -> Result<(), ExchangeError> {
    if low > high || open < low || open > high || current < low || current > high {
        return Err(schema("SZSE quote OHLC values contradict each other"));
    }
    let expected_delta = current - previous_close;
    if (delta - expected_delta).abs() > decimal_tolerance(delta_text)? {
        return Err(schema(
            "SZSE quote delta contradicts current and previous close",
        ));
    }
    let expected_percent = expected_delta / previous_close * 100.0;
    if (delta_percent - expected_percent).abs() > decimal_tolerance(delta_percent_text)? {
        return Err(schema(
            "SZSE quote deltaPercent contradicts current and previous close",
        ));
    }
    Ok(())
}

fn decimal(value: &str, field: &str) -> Result<f64, ExchangeError> {
    if !is_plain_decimal(value) {
        return Err(schema(format!("SZSE quote {field} is not a plain decimal")));
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| schema(format!("SZSE quote {field} is not finite")))
}

fn positive_decimal(value: &str, field: &str) -> Result<f64, ExchangeError> {
    let number = decimal(value, field)?;
    if number <= 0.0 {
        return Err(schema(format!("SZSE quote {field} must be positive")));
    }
    Ok(number)
}

fn is_plain_decimal(value: &str) -> bool {
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    if unsigned.is_empty() {
        return false;
    }
    let mut parts = unsigned.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    !integer.is_empty()
        && integer.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.is_none_or(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        && parts.next().is_none()
}

fn decimal_tolerance(value: &str) -> Result<f64, ExchangeError> {
    if !is_plain_decimal(value) {
        return Err(schema("SZSE quote decimal precision is invalid"));
    }
    let decimals = value
        .split_once('.')
        .map_or(0_i32, |(_, fraction)| fraction.len() as i32)
        .max(2);
    Ok(0.5 * 10_f64.powi(-decimals) + f64::EPSILON * 16.0)
}

fn validate_nonnegative(value: f64, field: &str) -> Result<(), ExchangeError> {
    if !value.is_finite() || value < 0.0 {
        return Err(schema(format!(
            "SZSE quote {field} must be finite and non-negative"
        )));
    }
    Ok(())
}

fn required_text(value: &str, field: &str) -> Result<String, ExchangeError> {
    let text = value.trim();
    if text.is_empty() || text.chars().any(char::is_control) {
        return Err(schema(format!("SZSE quote {field} is missing or invalid")));
    }
    Ok(text.to_owned())
}

fn parse_book(levels: &[BookLevelWire]) -> Result<([BookLevel; 5], [BookLevel; 5]), ExchangeError> {
    if levels.len() != 10 {
        return Err(schema(format!(
            "SZSE sellbuy5 must contain exactly 10 levels, received {}",
            levels.len()
        )));
    }
    let mut asks = [BookLevel::unavailable(); 5];
    let mut bids = [BookLevel::unavailable(); 5];
    for (target, wire) in asks.iter_mut().zip(&levels[..5]) {
        *target = parse_level(wire)?;
    }
    for (target, wire) in bids.iter_mut().zip(&levels[5..]) {
        *target = parse_level(wire)?;
    }
    validate_side("ask", &asks, true)?;
    validate_side("bid", &bids, false)?;
    if let (Some(ask), Some(bid)) = (asks[0].price(), bids[0].price()) {
        if bid.get() >= ask.get() {
            return Err(schema("SZSE five-level book is crossed or locked"));
        }
    }
    Ok((asks, bids))
}

fn parse_level(wire: &BookLevelWire) -> Result<BookLevel, ExchangeError> {
    match (&wire.price, wire.volume) {
        (None, None) => Ok(BookLevel::unavailable()),
        (Some(price), Some(volume)) => {
            validate_nonnegative(volume, "book volume")?;
            let price = decimal(price, "book price")?;
            match (price, volume) {
                (0.0, 0.0) => Ok(BookLevel::unavailable()),
                (price, volume) if price > 0.0 && volume > 0.0 => Ok(BookLevel::new(
                    Some(Price::new(price)?),
                    Some(Quantity::new(volume)?),
                )?),
                _ => Err(schema(
                    "SZSE book price and volume must both be positive or both zero",
                )),
            }
        }
        _ => Err(schema(
            "SZSE book price and volume must be present or absent together",
        )),
    }
}

fn validate_side(
    side: &str,
    levels: &[BookLevel; 5],
    ascending: bool,
) -> Result<(), ExchangeError> {
    let mut previous = None;
    let mut unavailable_seen = false;
    for level in levels {
        match level.price() {
            Some(_) if unavailable_seen => {
                return Err(schema(format!(
                    "SZSE {side} book contains a gap before a visible level"
                )));
            }
            Some(price) => {
                if let Some(previous) = previous {
                    let ordered = if ascending {
                        previous < price.get()
                    } else {
                        previous > price.get()
                    };
                    if !ordered {
                        return Err(schema(format!(
                            "SZSE {side} book levels are not strictly ordered"
                        )));
                    }
                }
                previous = Some(price.get());
            }
            None => unavailable_seen = true,
        }
    }
    Ok(())
}

fn visible_total(levels: &[BookLevel; 5]) -> Result<Option<Quantity>, ExchangeError> {
    let quantities = levels
        .iter()
        .filter_map(|level| level.quantity())
        .collect::<Vec<_>>();
    if quantities.is_empty() {
        return Ok(None);
    }
    let total = quantities.into_iter().map(Quantity::get).sum();
    Ok(Some(Quantity::new(total)?))
}

fn schema(message: impl Into<String>) -> ExchangeError {
    ExchangeError::Schema(message.into())
}
