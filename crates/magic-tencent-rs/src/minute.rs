use crate::{now, validate_instruments, TencentClient, TencentError};
use magic_market_core::{
    DataBatch, DataStatus, MinuteData, MinuteDataRequest, MinutePoint, Money, Price, ProviderId,
    Quantity,
};
use serde_json::Value;

const CURRENT_ENDPOINT: &str = "https://web.ifzq.gtimg.cn/appstock/app/minute/query?code=";
const HISTORY_ENDPOINT: &str = "https://web.ifzq.gtimg.cn/appstock/app/day/query?code=";
const MAX_MINUTE_POINTS: usize = 267;

fn display_date(value: &str) -> Result<String, TencentError> {
    if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(TencentError::Protocol(format!(
            "minute date must use YYYYMMDD: {value:?}"
        )));
    }
    Ok(format!(
        "{}-{}-{}",
        &value[0..4],
        &value[4..6],
        &value[6..8]
    ))
}

fn compact_date(value: &str) -> String {
    value.replace('-', "")
}

fn valid_session_minute(value: &str) -> bool {
    value.len() == 4
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (("0930"..="1130").contains(&value)
            || ("1300"..="1500").contains(&value)
            || ("1506"..="1530").contains(&value))
}

fn parse_number(value: &str, field: &'static str) -> Result<f64, TencentError> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| TencentError::Protocol(format!("{field} is not numeric: {value:?}")))?;
    if !parsed.is_finite() {
        return Err(TencentError::Protocol(format!("{field} must be finite")));
    }
    Ok(parsed)
}

fn select_rows<'a>(
    value: &'a Value,
    requested_date: Option<&str>,
) -> Result<(&'a str, &'a [Value]), TencentError> {
    if let Some(object) = value.as_object() {
        let date = object
            .get("date")
            .and_then(Value::as_str)
            .ok_or_else(|| TencentError::Protocol("current minute date is missing".into()))?;
        if requested_date.is_some() {
            return Err(TencentError::Protocol(
                "historical minute response used the current-session shape".into(),
            ));
        }
        let rows = object
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| TencentError::Protocol("current minute rows are missing".into()))?;
        return Ok((date, rows));
    }
    let sessions = value
        .as_array()
        .ok_or_else(|| TencentError::Protocol("minute data has an unknown JSON shape".into()))?;
    let requested = requested_date.ok_or_else(|| {
        TencentError::Protocol("current minute response used the historical shape".into())
    })?;
    let compact = compact_date(requested);
    let session = sessions
        .iter()
        .find(|session| session.get("date").and_then(Value::as_str) == Some(compact.as_str()))
        .ok_or_else(|| {
            TencentError::Protocol(format!("minute history omitted requested date {requested}"))
        })?;
    let date = session
        .get("date")
        .and_then(Value::as_str)
        .ok_or_else(|| TencentError::Protocol("historical minute date is missing".into()))?;
    let rows = session
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| TencentError::Protocol("historical minute rows are missing".into()))?;
    Ok((date, rows))
}

pub(crate) fn parse_minute_response(
    bytes: &[u8],
    symbol: &str,
    requested_date: Option<&str>,
    instrument: &magic_market_core::InstrumentId,
    observed_at: &str,
) -> Result<DataBatch<MinutePoint>, TencentError> {
    let root: Value = serde_json::from_slice(bytes)
        .map_err(|error| TencentError::Decode(format!("minute JSON: {error}")))?;
    let code = root
        .get("code")
        .and_then(Value::as_i64)
        .ok_or_else(|| TencentError::Protocol("minute response code is missing".into()))?;
    if code != 0 {
        return Err(TencentError::Protocol(format!(
            "minute endpoint returned code {code}"
        )));
    }
    let data = root
        .get("data")
        .and_then(|value| value.get(symbol))
        .and_then(|value| value.get("data"))
        .ok_or_else(|| TencentError::Protocol(format!("minute response omitted {symbol}")))?;
    let (compact, rows) = select_rows(data, requested_date)?;
    if rows.is_empty() {
        return Err(TencentError::Protocol("minute response is empty".into()));
    }
    if rows.len() > MAX_MINUTE_POINTS {
        return Err(TencentError::Protocol(format!(
            "minute response has {} records; maximum is {MAX_MINUTE_POINTS}",
            rows.len()
        )));
    }
    let date = display_date(compact)?;
    if requested_date.is_some_and(|requested| requested != date) {
        return Err(TencentError::Protocol(format!(
            "minute response date {date} differs from requested {}",
            requested_date.unwrap_or_default()
        )));
    }
    let batch_id = format!("tencent-web:{observed_at}:minute");
    let mut records = Vec::with_capacity(rows.len());
    let mut issues = Vec::new();
    let mut previous_time: Option<&str> = None;
    let mut previous_quantity = 0.0;
    let mut previous_amount = 0.0;
    for row in rows {
        let row = row
            .as_str()
            .ok_or_else(|| TencentError::Protocol("minute row must be a string".into()))?;
        let fields: Vec<_> = row.split_ascii_whitespace().collect();
        if fields.len() != 3 && fields.len() != 4 {
            return Err(TencentError::Protocol(format!(
                "minute row has {} fields; expected 3 or 4",
                fields.len()
            )));
        }
        let time = fields[0];
        if !valid_session_minute(time) {
            return Err(TencentError::Protocol(format!(
                "minute time is outside the verified session: {time:?}"
            )));
        }
        if previous_time.is_some_and(|previous| previous >= time) {
            return Err(TencentError::Protocol(
                "minute rows are duplicated or unordered".into(),
            ));
        }
        let price = Price::new(parse_number(fields[1], "minute price")?)?;
        let cumulative_quantity = parse_number(fields[2], "minute cumulative quantity")?;
        if cumulative_quantity < previous_quantity {
            return Err(TencentError::Protocol(
                "minute cumulative quantity regressed".into(),
            ));
        }
        let cumulative_amount = fields
            .get(3)
            .map(|value| parse_number(value, "minute cumulative amount"))
            .transpose()?;
        if cumulative_amount.is_some_and(|amount| amount < previous_amount) {
            return Err(TencentError::Protocol(
                "minute cumulative amount regressed".into(),
            ));
        }
        let minute_at = format!("{date} {}:{}", &time[0..2], &time[2..4]);
        let source_at = format!("{date}T{}:{}:00+08:00", &time[0..2], &time[2..4]);
        let status = if cumulative_amount.is_some() {
            DataStatus::Available
        } else {
            issues.push(format!(
                "{} {time}: cumulative amount unavailable",
                instrument.code()
            ));
            DataStatus::Unavailable
        };
        records.push(MinutePoint::new(
            instrument.clone(),
            minute_at,
            price,
            Quantity::new(cumulative_quantity)?,
            cumulative_amount.map(Money::new).transpose()?,
            status,
            Some(source_at),
            observed_at,
            ProviderId::Tencent,
            batch_id.clone(),
        )?);
        previous_time = Some(time);
        previous_quantity = cumulative_quantity;
        if let Some(amount) = cumulative_amount {
            previous_amount = amount;
        }
    }
    let latest_source_at = records
        .last()
        .and_then(MinutePoint::source_at)
        .ok_or_else(|| TencentError::Protocol("minute source time is missing".into()))?;
    let provenance = magic_market_core::Provenance::new("tencent-web", observed_at)?
        .with_source_at(latest_source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::best_effort(records, provenance, issues)?)
}

impl MinuteData for TencentClient {
    type Error = TencentError;

    fn minute_data(
        &self,
        request: &MinuteDataRequest,
    ) -> Result<DataBatch<MinutePoint>, Self::Error> {
        let symbol = validate_instruments(std::slice::from_ref(request.instrument()))?
            .pop()
            .ok_or_else(|| TencentError::InvalidRequest("minute instrument is missing".into()))?;
        let endpoint = if request.date().is_some() {
            HISTORY_ENDPOINT
        } else {
            CURRENT_ENDPOINT
        };
        let bytes = self.transport.get(&format!("{endpoint}{symbol}"))?;
        let observed_at = now()?;
        parse_minute_response(
            &bytes,
            &symbol,
            request.date(),
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
    fn parses_current_and_historical_minute_shapes() {
        let current = br#"{"code":0,"data":{"sh600396":{"data":{"date":"20260723","data":["0930 15.30 10 15300.00","0931 15.50 20 30800.00"]}}}}"#;
        let batch =
            parse_minute_response(current, "sh600396", None, &instrument(), "observed").unwrap();
        assert_eq!(batch.records().len(), 2);
        assert_eq!(batch.records()[1].minute_at(), "2026-07-23 09:31");
        assert_eq!(batch.records()[1].cumulative_quantity().get(), 20.0);
        assert!(batch.quality().is_complete());

        let history = br#"{"code":0,"data":{"sh600396":{"data":[{"date":"20260722","data":["0930 14.00 8 11200.00"]},{"date":"20260723","data":["0930 15.30 10 15300.00"]}]}}}"#;
        let batch = parse_minute_response(
            history,
            "sh600396",
            Some("2026-07-22"),
            &instrument(),
            "observed",
        )
        .unwrap();
        assert_eq!(batch.records()[0].minute_at(), "2026-07-22 09:30");
    }

    #[test]
    fn missing_amount_is_visible_and_not_zero() {
        let fixture =
            br#"{"code":0,"data":{"sh600396":{"data":{"date":"20260723","data":["0930 15.30 10"]}}}}"#;
        let batch =
            parse_minute_response(fixture, "sh600396", None, &instrument(), "observed").unwrap();
        assert!(batch.records()[0].cumulative_amount().is_none());
        assert_eq!(batch.records()[0].status(), DataStatus::Unavailable);
        assert!(!batch.quality().is_complete());
    }

    #[test]
    fn rejects_duplicate_time_regression_and_date_mismatch() {
        let duplicate = br#"{"code":0,"data":{"sh600396":{"data":{"date":"20260723","data":["0930 15.30 10","0930 15.30 11"]}}}}"#;
        assert!(
            parse_minute_response(duplicate, "sh600396", None, &instrument(), "observed").is_err()
        );
        let regression = br#"{"code":0,"data":{"sh600396":{"data":{"date":"20260723","data":["0930 15.30 10","0931 15.30 9"]}}}}"#;
        assert!(
            parse_minute_response(regression, "sh600396", None, &instrument(), "observed").is_err()
        );
        let history = br#"{"code":0,"data":{"sh600396":{"data":[{"date":"20260723","data":["0930 15.30 10"]}]}}}"#;
        assert!(parse_minute_response(
            history,
            "sh600396",
            Some("2026-07-22"),
            &instrument(),
            "observed"
        )
        .is_err());
    }

    #[test]
    fn accepts_only_the_live_verified_post_market_window() {
        let accepted = br#"{"code":0,"data":{"sh600396":{"data":{"date":"20260723","data":["1500 15.30 10","1506 15.30 11","1530 15.30 12"]}}}}"#;
        assert!(
            parse_minute_response(accepted, "sh600396", None, &instrument(), "observed").is_ok()
        );
        for time in ["1501", "1505", "1531"] {
            let fixture = format!(
                r#"{{"code":0,"data":{{"sh600396":{{"data":{{"date":"20260723","data":["{time} 15.30 10"]}}}}}}}}"#
            );
            assert!(parse_minute_response(
                fixture.as_bytes(),
                "sh600396",
                None,
                &instrument(),
                "observed"
            )
            .is_err());
        }
    }
}
