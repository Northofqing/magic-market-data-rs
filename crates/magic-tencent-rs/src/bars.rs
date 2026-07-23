use crate::{now, validate_instruments, TencentClient, TencentError};
use magic_market_core::{
    Adjustment, Bar, BarInterval, BarsRequest, DataBatch, HistoricalBars, Money, Price, ProviderId,
    Quantity,
};
use serde_json::Value;
use std::collections::HashSet;

const DAILY_ENDPOINT: &str = "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=";
const INTRADAY_ENDPOINT: &str = "https://ifzq.gtimg.cn/appstock/app/kline/mkline?param=";
const MAX_BARS: u16 = 800;

struct BarSource {
    endpoint: &'static str,
    key: &'static str,
    intraday: bool,
}

fn bar_source(interval: BarInterval) -> Result<BarSource, TencentError> {
    Ok(match interval {
        BarInterval::Minute1 => BarSource {
            endpoint: INTRADAY_ENDPOINT,
            key: "m1",
            intraday: true,
        },
        BarInterval::Minute5 => BarSource {
            endpoint: INTRADAY_ENDPOINT,
            key: "m5",
            intraday: true,
        },
        BarInterval::Minute15 => BarSource {
            endpoint: INTRADAY_ENDPOINT,
            key: "m15",
            intraday: true,
        },
        BarInterval::Minute30 => BarSource {
            endpoint: INTRADAY_ENDPOINT,
            key: "m30",
            intraday: true,
        },
        BarInterval::Hour1 => BarSource {
            endpoint: INTRADAY_ENDPOINT,
            key: "m60",
            intraday: true,
        },
        BarInterval::Day => BarSource {
            endpoint: DAILY_ENDPOINT,
            key: "day",
            intraday: false,
        },
        BarInterval::Week => BarSource {
            endpoint: DAILY_ENDPOINT,
            key: "week",
            intraday: false,
        },
        BarInterval::Month => BarSource {
            endpoint: DAILY_ENDPOINT,
            key: "month",
            intraday: false,
        },
        BarInterval::Year => {
            return Err(TencentError::Unsupported(
                "Tencent year parameter returned a daily-shaped current record in live validation"
                    .into(),
            ));
        }
    })
}

fn value_text<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, TencentError> {
    value
        .as_str()
        .ok_or_else(|| TencentError::Protocol(format!("{field} must be a JSON string")))
}

fn number(value: &Value, field: &'static str) -> Result<f64, TencentError> {
    let text = value_text(value, field)?;
    let parsed = text
        .parse::<f64>()
        .map_err(|_| TencentError::Protocol(format!("{field} is not numeric: {text:?}")))?;
    if !parsed.is_finite() {
        return Err(TencentError::Protocol(format!("{field} must be finite")));
    }
    Ok(parsed)
}

fn normalized_bar_time(value: &str, intraday: bool) -> Result<(String, String), TencentError> {
    if intraday {
        if value.len() != 12 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(TencentError::Protocol(format!(
                "intraday bar time must use YYYYMMDDHHMM: {value:?}"
            )));
        }
        let display = format!(
            "{}-{}-{} {}:{}:00",
            &value[0..4],
            &value[4..6],
            &value[6..8],
            &value[8..10],
            &value[10..12]
        );
        let source = format!(
            "{}-{}-{}T{}:{}:00+08:00",
            &value[0..4],
            &value[4..6],
            &value[6..8],
            &value[8..10],
            &value[10..12]
        );
        Ok((display, source))
    } else {
        Ok((value.to_owned(), value.to_owned()))
    }
}

pub(crate) fn parse_bars_response(
    bytes: &[u8],
    symbol: &str,
    interval: BarInterval,
    limit: u16,
    instrument: &magic_market_core::InstrumentId,
    observed_at: &str,
) -> Result<DataBatch<Bar>, TencentError> {
    let root: Value = serde_json::from_slice(bytes)
        .map_err(|error| TencentError::Decode(format!("bar JSON: {error}")))?;
    let code = root
        .get("code")
        .and_then(Value::as_i64)
        .ok_or_else(|| TencentError::Protocol("bar response code is missing".into()))?;
    if code != 0 {
        let message = root.get("msg").and_then(Value::as_str).unwrap_or("unknown");
        return Err(TencentError::Protocol(format!(
            "bar endpoint returned code {code}: {message}"
        )));
    }
    let source = bar_source(interval)?;
    let rows = root
        .get("data")
        .and_then(|value| value.get(symbol))
        .and_then(|value| value.get(source.key))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            TencentError::Protocol(format!("bar response omitted {symbol}.{}", source.key))
        })?;
    if rows.is_empty() {
        return Err(TencentError::Protocol("bar response is empty".into()));
    }
    if rows.len() > usize::from(limit) + 1 {
        return Err(TencentError::Protocol(format!(
            "bar endpoint returned {} records for limit {limit}",
            rows.len()
        )));
    }
    let start = rows.len().saturating_sub(usize::from(limit));
    let batch_id = format!("tencent-web:{observed_at}:bars");
    let mut records = Vec::with_capacity(rows.len() - start);
    let mut seen = HashSet::with_capacity(rows.len() - start);
    let mut previous: Option<String> = None;
    for row in &rows[start..] {
        let fields = row
            .as_array()
            .ok_or_else(|| TencentError::Protocol("bar row must be an array".into()))?;
        let expected = if source.intraday { 8 } else { 6 };
        if fields.len() != expected {
            return Err(TencentError::Protocol(format!(
                "{} bar row has {} fields; expected {expected}",
                source.key,
                fields.len()
            )));
        }
        let source_label = value_text(&fields[0], "bar time")?;
        let (bar_time, source_at) = normalized_bar_time(source_label, source.intraday)?;
        if previous.as_ref().is_some_and(|value| value >= &bar_time)
            || !seen.insert(bar_time.clone())
        {
            return Err(TencentError::Protocol(
                "bar records are duplicated or unordered".into(),
            ));
        }
        let open = Price::new(number(&fields[1], "bar open")?)?;
        let close = Price::new(number(&fields[2], "bar close")?)?;
        let high = Price::new(number(&fields[3], "bar high")?)?;
        let low = Price::new(number(&fields[4], "bar low")?)?;
        let volume = Quantity::new(number(&fields[5], "bar volume")?)?;
        let bar = Bar::new(
            instrument.clone(),
            interval,
            bar_time.clone(),
            bar_time.clone(),
            open,
            high,
            low,
            close,
            volume,
            None::<Money>,
            Adjustment::Unadjusted,
            ProviderId::Tencent,
            batch_id.clone(),
        )?
        .with_source_at(source_at)?;
        previous = Some(bar_time);
        records.push(bar);
    }
    let latest_source_at = records
        .last()
        .and_then(Bar::source_at)
        .ok_or_else(|| TencentError::Protocol("bar source time is missing".into()))?;
    let provenance = magic_market_core::Provenance::new("tencent-web", observed_at)?
        .with_source_at(latest_source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

impl HistoricalBars for TencentClient {
    type Bar = Bar;
    type Error = TencentError;

    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        if request.start().is_some() || request.end().is_some() {
            return Err(TencentError::Unsupported(
                "Tencent bars do not expose a verified normalized date-range selector".into(),
            ));
        }
        if request.limit() > MAX_BARS {
            return Err(TencentError::InvalidRequest(format!(
                "Tencent bars accept at most {MAX_BARS} records"
            )));
        }
        let symbol = validate_instruments(std::slice::from_ref(request.instrument()))?
            .pop()
            .ok_or_else(|| TencentError::InvalidRequest("bar instrument is missing".into()))?;
        let source = bar_source(request.interval())?;
        if source.intraday
            && request.instrument().exchange() == magic_market_core::Exchange::Beijing
        {
            return Err(TencentError::Unsupported(
                "Tencent intraday K-line endpoint returned no Beijing records in live validation"
                    .into(),
            ));
        }
        let url = if source.intraday {
            format!(
                "{}{},{},,{}",
                source.endpoint,
                symbol,
                source.key,
                request.limit()
            )
        } else {
            format!(
                "{}{},{},,,{},none",
                source.endpoint,
                symbol,
                source.key,
                request.limit()
            )
        };
        let bytes = self.transport.get(&url)?;
        let observed_at = now()?;
        parse_bars_response(
            &bytes,
            &symbol,
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
    use magic_market_core::{AssetClass, Exchange, InstrumentId};

    fn instrument() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
    }

    #[test]
    fn parses_daily_bars_and_applies_exact_limit() {
        let fixture = br#"{"code":0,"msg":"","data":{"sh600396":{"day":[["2026-07-21","12.98","13.56","13.99","11.80","3326679"],["2026-07-22","12.98","14.92","14.92","12.90","2472884"],["2026-07-23","15.30","15.89","16.35","14.85","2873766"]]}}}"#;
        let batch = parse_bars_response(
            fixture,
            "sh600396",
            BarInterval::Day,
            2,
            &instrument(),
            "observed",
        )
        .unwrap();
        assert_eq!(batch.records().len(), 2);
        assert_eq!(batch.records()[0].bar_start(), "2026-07-22");
        assert_eq!(batch.records()[1].close().get(), 15.89);
        assert_eq!(batch.records()[1].adjustment(), Adjustment::Unadjusted);
    }

    #[test]
    fn parses_intraday_bar_and_rejects_bad_shape_or_order() {
        let fixture = br#"{"code":0,"msg":"","data":{"sh600396":{"m5":[["202607231125","15.78","15.97","15.97","15.75","18315.00",{},"12.44"],["202607231130","15.96","15.89","16.00","15.87","11441.00",{},"7.77"]]}}}"#;
        let batch = parse_bars_response(
            fixture,
            "sh600396",
            BarInterval::Minute5,
            2,
            &instrument(),
            "observed",
        )
        .unwrap();
        assert_eq!(batch.records()[0].bar_start(), "2026-07-23 11:25:00");
        assert_eq!(
            batch.records()[1].source_at(),
            Some("2026-07-23T11:30:00+08:00")
        );

        let duplicate = br#"{"code":0,"data":{"sh600396":{"m5":[["202607231125","15.78","15.97","15.97","15.75","1",{},"1"],["202607231125","15.78","15.97","15.97","15.75","1",{},"1"]]}}}"#;
        assert!(parse_bars_response(
            duplicate,
            "sh600396",
            BarInterval::Minute5,
            2,
            &instrument(),
            "observed"
        )
        .is_err());
    }

    #[test]
    fn rejects_endpoint_error_and_inconsistent_ohlc() {
        let bad_code = br#"{"code":1,"msg":"bad params","data":{}}"#;
        assert!(parse_bars_response(
            bad_code,
            "sh600396",
            BarInterval::Day,
            1,
            &instrument(),
            "observed"
        )
        .is_err());
        let bad_ohlc = br#"{"code":0,"data":{"sh600396":{"day":[["2026-07-23","15.30","15.89","14.00","14.85","1"]]}}}"#;
        assert!(parse_bars_response(
            bad_ohlc,
            "sh600396",
            BarInterval::Day,
            1,
            &instrument(),
            "observed"
        )
        .is_err());
        assert!(matches!(
            bar_source(BarInterval::Year),
            Err(TencentError::Unsupported(_))
        ));
    }
}
