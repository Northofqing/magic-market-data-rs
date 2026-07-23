use crate::{
    now, shares_to_lots, valid_date, valid_time, validate_instruments, SinaClient, SinaError,
};
use magic_market_core::{
    Adjustment, Bar, BarInterval, BarsRequest, DataBatch, HistoricalBars, Money, Price, ProviderId,
    Quantity,
};
use serde_json::Value;
use std::collections::HashSet;

const KLINE_ENDPOINT: &str =
    "https://quotes.sina.cn/cn/api/json_v2.php/CN_MarketDataService.getKLineData";
const MAX_BARS: u16 = 800;

#[derive(Debug, Clone, Copy)]
struct BarSource {
    scale: u16,
    intraday: bool,
}

fn bar_source(interval: BarInterval) -> Result<BarSource, SinaError> {
    Ok(match interval {
        BarInterval::Minute1 => BarSource {
            scale: 1,
            intraday: true,
        },
        BarInterval::Minute5 => BarSource {
            scale: 5,
            intraday: true,
        },
        BarInterval::Minute15 => BarSource {
            scale: 15,
            intraday: true,
        },
        BarInterval::Minute30 => BarSource {
            scale: 30,
            intraday: true,
        },
        BarInterval::Hour1 => BarSource {
            scale: 60,
            intraday: true,
        },
        BarInterval::Day => BarSource {
            scale: 240,
            intraday: false,
        },
        BarInterval::Week | BarInterval::Month | BarInterval::Year => {
            return Err(SinaError::Unsupported(format!(
                "Sina interval {interval:?} has no verified scale contract"
            )));
        }
    })
}

pub(crate) fn kline_url(
    symbol: &str,
    interval: BarInterval,
    limit: u16,
) -> Result<String, SinaError> {
    let source = bar_source(interval)?;
    Ok(format!(
        "{KLINE_ENDPOINT}?symbol={symbol}&scale={}&ma=no&datalen={limit}",
        source.scale
    ))
}

#[derive(Debug, Clone)]
pub(crate) struct SourceBar {
    pub(crate) bar_time: String,
    pub(crate) source_at: String,
    pub(crate) open: f64,
    pub(crate) high: f64,
    pub(crate) low: f64,
    pub(crate) close: f64,
    pub(crate) volume_shares: f64,
    pub(crate) amount_yuan: Option<f64>,
}

fn value_text<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<&'a str, SinaError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| SinaError::Protocol(format!("bar {field} must be a JSON string")))
}

fn number(value: &str, field: &'static str) -> Result<f64, SinaError> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| SinaError::Protocol(format!("bar {field} is not numeric: {value:?}")))?;
    if !parsed.is_finite() {
        return Err(SinaError::Protocol(format!("bar {field} must be finite")));
    }
    Ok(parsed)
}

fn positive_number(value: &str, field: &'static str) -> Result<f64, SinaError> {
    let parsed = number(value, field)?;
    if parsed <= 0.0 {
        return Err(SinaError::Protocol(format!("bar {field} must be positive")));
    }
    Ok(parsed)
}

fn nonnegative_number(value: &str, field: &'static str) -> Result<f64, SinaError> {
    let parsed = number(value, field)?;
    if parsed < 0.0 {
        return Err(SinaError::Protocol(format!(
            "bar {field} must be non-negative"
        )));
    }
    Ok(parsed)
}

fn normalize_time(value: &str, intraday: bool) -> Result<(String, String), SinaError> {
    if intraday {
        if value.len() != 19
            || value.as_bytes()[10] != b' '
            || !valid_date(&value[..10])
            || !valid_time(&value[11..])
        {
            return Err(SinaError::Protocol(format!(
                "intraday bar time must use a valid YYYY-MM-DD HH:MM:SS value: {value:?}"
            )));
        }
        Ok((
            value.to_owned(),
            format!("{}T{}+08:00", &value[..10], &value[11..]),
        ))
    } else if valid_date(value) {
        Ok((value.to_owned(), value.to_owned()))
    } else {
        Err(SinaError::Protocol(format!(
            "daily bar time must use a valid YYYY-MM-DD value: {value:?}"
        )))
    }
}

pub(crate) fn parse_source_rows(
    bytes: &[u8],
    interval: BarInterval,
    limit: u16,
) -> Result<Vec<SourceBar>, SinaError> {
    let source = bar_source(interval)?;
    let root: Value = serde_json::from_slice(bytes)
        .map_err(|error| SinaError::Decode(format!("bar JSON: {error}")))?;
    let rows = root
        .as_array()
        .ok_or_else(|| SinaError::Protocol("bar response root must be an array".into()))?;
    if rows.is_empty() {
        return Err(SinaError::Protocol("bar response is empty".into()));
    }
    if rows.len() > usize::from(limit) {
        return Err(SinaError::Protocol(format!(
            "bar endpoint returned {} records for limit {limit}",
            rows.len()
        )));
    }
    let mut parsed = Vec::with_capacity(rows.len());
    let mut seen = HashSet::with_capacity(rows.len());
    let mut previous: Option<String> = None;
    for row in rows {
        let object = row
            .as_object()
            .ok_or_else(|| SinaError::Protocol("bar row must be an object".into()))?;
        let (bar_time, source_at) = normalize_time(value_text(object, "day")?, source.intraday)?;
        if previous.as_ref().is_some_and(|value| value >= &bar_time)
            || !seen.insert(bar_time.clone())
        {
            return Err(SinaError::Protocol(
                "bar records are duplicated or unordered".into(),
            ));
        }
        let amount_yuan = match object.get("amount") {
            Some(value) => Some(nonnegative_number(
                value.as_str().ok_or_else(|| {
                    SinaError::Protocol("bar amount must be a JSON string".into())
                })?,
                "amount",
            )?),
            None if source.intraday => {
                return Err(SinaError::Protocol("intraday bar amount is missing".into()));
            }
            None => None,
        };
        let open = positive_number(value_text(object, "open")?, "open")?;
        let high = positive_number(value_text(object, "high")?, "high")?;
        let low = positive_number(value_text(object, "low")?, "low")?;
        let close = positive_number(value_text(object, "close")?, "close")?;
        if low > open.min(close) || high < open.max(close) || low > high {
            return Err(SinaError::Protocol(
                "bar OHLC values have an inconsistent range".into(),
            ));
        }
        parsed.push(SourceBar {
            bar_time: bar_time.clone(),
            source_at,
            open,
            high,
            low,
            close,
            volume_shares: nonnegative_number(value_text(object, "volume")?, "volume")?,
            amount_yuan,
        });
        previous = Some(bar_time);
    }
    Ok(parsed)
}

pub(crate) fn parse_bars_response(
    bytes: &[u8],
    interval: BarInterval,
    limit: u16,
    instrument: &magic_market_core::InstrumentId,
    observed_at: &str,
) -> Result<DataBatch<Bar>, SinaError> {
    let source = parse_source_rows(bytes, interval, limit)?;
    let batch_id = format!("sina-web:{observed_at}:bars");
    let mut records = Vec::with_capacity(source.len());
    for row in source {
        records.push(
            Bar::new(
                instrument.clone(),
                interval,
                row.bar_time.clone(),
                row.bar_time,
                Price::new(row.open)?,
                Price::new(row.high)?,
                Price::new(row.low)?,
                Price::new(row.close)?,
                Quantity::new(shares_to_lots(row.volume_shares)?)?,
                row.amount_yuan.map(Money::new).transpose()?,
                Adjustment::Unadjusted,
                ProviderId::Sina,
                batch_id.clone(),
            )?
            .with_source_at(row.source_at)?,
        );
    }
    let latest_source_at = records
        .last()
        .and_then(Bar::source_at)
        .ok_or_else(|| SinaError::Protocol("bar source time is missing".into()))?;
    let provenance = magic_market_core::Provenance::new("sina-web", observed_at)?
        .with_source_at(latest_source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

impl HistoricalBars for SinaClient {
    type Bar = Bar;
    type Error = SinaError;

    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        if request.start().is_some() || request.end().is_some() {
            return Err(SinaError::Unsupported(
                "Sina bars do not expose a verified normalized date-range selector".into(),
            ));
        }
        if request.limit() > MAX_BARS {
            return Err(SinaError::InvalidRequest(format!(
                "Sina bars accept at most {MAX_BARS} records"
            )));
        }
        let symbol = validate_instruments(std::slice::from_ref(request.instrument()))?
            .pop()
            .ok_or_else(|| SinaError::InvalidRequest("bar instrument is missing".into()))?;
        let url = kline_url(&symbol, request.interval(), request.limit())?;
        let bytes = self.transport.get(&url)?;
        let observed_at = now()?;
        parse_bars_response(
            &bytes,
            request.interval(),
            request.limit(),
            request.instrument(),
            &observed_at,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magic_market_core::{AssetClass, BarInterval, Exchange, InstrumentId, Money, Quantity};

    fn instrument() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
    }

    #[test]
    fn parses_intraday_amount_and_converts_shares_to_lots() {
        let fixture = br#"[{"day":"2026-07-23 14:55:00","open":"16.410","high":"16.410","low":"16.410","close":"16.410","volume":"1243300","amount":"20402553.0000"}]"#;
        let batch =
            parse_bars_response(fixture, BarInterval::Minute5, 1, &instrument(), "observed")
                .unwrap();
        assert_eq!(batch.records().len(), 1);
        assert_eq!(
            batch.records()[0].volume(),
            Quantity::new(12_433.0).unwrap()
        );
        assert_eq!(
            batch.records()[0].amount().map(Money::get),
            Some(20_402_553.0)
        );
        assert_eq!(
            batch.records()[0].source_at(),
            Some("2026-07-23T14:55:00+08:00")
        );
    }

    #[test]
    fn parses_daily_bar_without_inventing_amount() {
        let fixture = br#"[{"day":"2026-07-23","open":"15.300","high":"16.410","low":"14.850","close":"16.410","volume":"341780059"}]"#;
        let batch =
            parse_bars_response(fixture, BarInterval::Day, 1, &instrument(), "observed").unwrap();
        assert_eq!(batch.records()[0].amount(), None);
        assert_eq!(batch.records()[0].volume().get(), 3_417_800.59);
        assert_eq!(batch.provenance().source_at(), Some("2026-07-23"));
    }

    #[test]
    fn rejects_empty_missing_amount_and_excess_rows() {
        assert!(
            parse_bars_response(br#"[]"#, BarInterval::Minute1, 1, &instrument(), "observed")
                .is_err()
        );
        let missing_amount = br#"[{"day":"2026-07-23 09:30:00","open":"1","high":"1","low":"1","close":"1","volume":"1"}]"#;
        assert!(parse_bars_response(
            missing_amount,
            BarInterval::Minute1,
            1,
            &instrument(),
            "observed"
        )
        .is_err());
        let two = br#"[{"day":"2026-07-23","open":"1","high":"1","low":"1","close":"1","volume":"1"},{"day":"2026-07-24","open":"1","high":"1","low":"1","close":"1","volume":"1"}]"#;
        assert!(parse_bars_response(two, BarInterval::Day, 1, &instrument(), "observed").is_err());
    }

    #[test]
    fn rejects_bad_shape_order_and_ohlc() {
        assert!(parse_bars_response(
            br#"{"day":"2026-07-23"}"#,
            BarInterval::Day,
            1,
            &instrument(),
            "observed"
        )
        .is_err());
        let duplicate = br#"[{"day":"2026-07-23 09:30:00","open":"1","high":"1","low":"1","close":"1","volume":"1","amount":"1"},{"day":"2026-07-23 09:30:00","open":"1","high":"1","low":"1","close":"1","volume":"1","amount":"1"}]"#;
        assert!(parse_bars_response(
            duplicate,
            BarInterval::Minute1,
            2,
            &instrument(),
            "observed"
        )
        .is_err());
        let bad_ohlc = br#"[{"day":"2026-07-23","open":"15.30","high":"14.00","low":"14.85","close":"15.89","volume":"1"}]"#;
        assert!(
            parse_bars_response(bad_ohlc, BarInterval::Day, 1, &instrument(), "observed").is_err()
        );
    }

    #[test]
    fn interval_mapping_keeps_unverified_periods_unsupported() {
        assert_eq!(bar_source(BarInterval::Minute1).unwrap().scale, 1);
        assert_eq!(bar_source(BarInterval::Hour1).unwrap().scale, 60);
        assert_eq!(bar_source(BarInterval::Day).unwrap().scale, 240);
        assert!(matches!(
            bar_source(BarInterval::Week),
            Err(SinaError::Unsupported(_))
        ));
        assert!(matches!(
            bar_source(BarInterval::Month),
            Err(SinaError::Unsupported(_))
        ));
        assert!(matches!(
            bar_source(BarInterval::Year),
            Err(SinaError::Unsupported(_))
        ));
    }
}
