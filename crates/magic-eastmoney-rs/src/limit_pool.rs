use crate::mapping::{
    money, non_empty, optional_f64, optional_string, optional_u32, percent, quantity,
    required_string,
};
use crate::{instrument_from_market, query_url, BatchContext, EastmoneyClient, EastmoneyError};
use magic_market_core::{
    IsoDate, LimitPoolEntry, LimitPoolKind, LimitPoolRequest, LimitPools, PositiveU32, Price,
};
use serde_json::Value;

const BASE: &str = "https://push2ex.eastmoney.com";
const TOKEN: &str = "7eea3edcaed734bea9cbfc24409ed989";

impl LimitPools for EastmoneyClient {
    type Error = EastmoneyError;

    fn limit_pool(
        &self,
        request: &LimitPoolRequest,
    ) -> Result<magic_market_core::DataBatch<LimitPoolEntry>, Self::Error> {
        let (path, sort) = match request.kind() {
            LimitPoolKind::Upper => ("getTopicZTPool", "fbt:asc"),
            LimitPoolKind::Broken => ("getTopicZBPool", "fbt:asc"),
            LimitPoolKind::Lower => ("getTopicDTPool", "fund:asc"),
            LimitPoolKind::PreviousUpper => ("getYesterdayZTPool", "zs:desc"),
        };
        let date = request.trading_date().as_str().replace('-', "");
        let url = query_url(
            &format!("{BASE}/{path}"),
            &[
                ("ut", TOKEN.into()),
                ("dpt", "wz.ztzt".into()),
                ("Pageindex", "0".into()),
                ("pagesize", request.limit().get().to_string()),
                ("sort", sort.into()),
                ("date", date),
            ],
        );
        let bytes = self.get(
            &url,
            &[
                ("Accept", "application/json"),
                ("Referer", "https://quote.eastmoney.com/"),
            ],
        )?;
        parse_limit_pool(&bytes, request)
    }
}

fn parse_limit_pool(
    bytes: &[u8],
    request: &LimitPoolRequest,
) -> Result<magic_market_core::DataBatch<LimitPoolEntry>, EastmoneyError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    if root.get("rc").and_then(Value::as_i64) != Some(0) {
        return Err(EastmoneyError::Protocol(format!(
            "limit-pool endpoint returned rc={}",
            root.get("rc").unwrap_or(&Value::Null)
        )));
    }
    let rows: &[Value] = match root.pointer("/data/pool") {
        Some(Value::Array(rows)) => rows,
        None | Some(Value::Null) if root.get("data").is_none_or(Value::is_null) => &[],
        _ => {
            return Err(EastmoneyError::Protocol(
                "limit-pool data.pool is not an array".into(),
            ))
        }
    };
    let source_date = parse_qdate(&root, request.trading_date())?;
    let context = BatchContext::new("limit-pool", Some(source_date.as_str()))?;
    let records = rows
        .iter()
        .map(|row| map_entry(row, request, &source_date, &context))
        .collect::<Result<Vec<_>, _>>()?;
    context.finish(records)
}

fn parse_qdate(root: &Value, expected: &IsoDate) -> Result<IsoDate, EastmoneyError> {
    let compact = optional_string(root.pointer("/data/qdate"))?
        .ok_or_else(|| EastmoneyError::Protocol("limit-pool source qdate is absent".into()))?;
    if compact.len() != 8 || !compact.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EastmoneyError::Protocol(format!(
            "limit-pool source qdate {compact:?} must use YYYYMMDD"
        )));
    }
    let normalized = format!("{}-{}-{}", &compact[0..4], &compact[4..6], &compact[6..8]);
    let actual = IsoDate::new(normalized).map_err(|error| {
        EastmoneyError::Protocol(format!(
            "limit-pool source qdate {compact:?} is not a valid calendar date: {error}"
        ))
    })?;
    if &actual != expected {
        return Err(EastmoneyError::Protocol(format!(
            "limit-pool source qdate {} does not match requested date {}",
            actual.as_str(),
            expected.as_str()
        )));
    }
    Ok(actual)
}

fn map_entry(
    row: &Value,
    request: &LimitPoolRequest,
    source_date: &IsoDate,
    context: &BatchContext,
) -> Result<LimitPoolEntry, EastmoneyError> {
    let code = required_string(row, "c")?;
    let market = optional_f64(row.get("m"))?
        .ok_or_else(|| EastmoneyError::Protocol("limit-pool market m is absent".into()))?;
    if market.fract() != 0.0 {
        return Err(EastmoneyError::Protocol(
            "limit-pool market m is not integral".into(),
        ));
    }
    let market = market as i64;
    let raw_price = optional_f64(row.get("p"))?
        .or(optional_f64(row.get("ztp"))?)
        .ok_or_else(|| EastmoneyError::Protocol("limit-pool price is absent".into()))?;
    let streak = optional_u32(row.get("lbc"))?
        .or(optional_u32(row.get("ylbc"))?)
        .or(optional_u32(row.get("days"))?)
        .filter(|value| *value > 0)
        .map(PositiveU32::new)
        .transpose()?;
    Ok(LimitPoolEntry {
        kind: request.kind(),
        instrument: instrument_from_market(&code, market)?,
        trading_date: source_date.clone(),
        price: Price::new(raw_price / 1_000.0)?,
        change: percent(optional_f64(row.get("zdp"))?)?
            .ok_or_else(|| EastmoneyError::Protocol("limit-pool change zdp is absent".into()))?,
        volume: quantity(optional_f64(row.get("volume"))?)?,
        turnover: percent(optional_f64(row.get("hs"))?)?,
        sealed_amount: money(optional_f64(row.get("fund"))?)?,
        first_seal_at: non_empty(format_time(optional_string(row.get("fbt"))?)?)?,
        last_seal_at: non_empty(format_time(optional_string(row.get("lbt"))?)?)?,
        break_count: optional_u32(row.get("zbc"))?,
        streak,
        industry: non_empty(optional_string(row.get("hybk"))?)?,
        board_name: None,
        seal_state: None,
        reseal_count: None,
        reason: None,
        evidence: context.evidence_at(Some(source_date.as_str()))?,
    })
}

fn format_time(value: Option<String>) -> Result<Option<String>, EastmoneyError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let normalized = if value.contains(':') {
        value
    } else if value.bytes().all(|byte| byte.is_ascii_digit()) && value.len() <= 6 {
        let padded = format!("{value:0>6}");
        format!("{}:{}:{}", &padded[0..2], &padded[2..4], &padded[4..6])
    } else {
        return Err(EastmoneyError::Protocol(format!(
            "invalid limit-pool time {value}"
        )));
    };
    let bytes = normalized.as_bytes();
    if bytes.len() != 8
        || bytes[2] != b':'
        || bytes[5] != b':'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 2 | 5) || byte.is_ascii_digit())
    {
        return Err(EastmoneyError::Protocol(format!(
            "invalid limit-pool time {normalized}"
        )));
    }
    let hour = normalized[0..2].parse::<u32>().unwrap_or(u32::MAX);
    let minute = normalized[3..5].parse::<u32>().unwrap_or(u32::MAX);
    let second = normalized[6..8].parse::<u32>().unwrap_or(u32::MAX);
    if hour > 23 || minute > 59 || second > 59 {
        return Err(EastmoneyError::Protocol(format!(
            "invalid limit-pool time {normalized}"
        )));
    }
    Ok(Some(normalized))
}

#[cfg(test)]
mod tests {
    use super::{format_time, parse_limit_pool};
    use magic_market_core::{IsoDate, LimitPoolKind, LimitPoolRequest, PositiveU32, RatioUnit};

    fn request(kind: LimitPoolKind) -> LimitPoolRequest {
        LimitPoolRequest::new(
            kind,
            IsoDate::new("2026-07-23").unwrap(),
            PositiveU32::new(10).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn maps_verified_limit_pool_units_and_metadata() {
        let fixture = r#"{"rc":0,"data":{"qdate":20260723,"pool":[{
          "c":"600396","m":1,"n":"华电辽能","p":1308000,"zdp":9.97,
          "hs":4.2,"fund":12345678,"fbt":93100,"lbt":145501,
          "zbc":2,"lbc":3,"hybk":"电力行业"
        }]}}"#
            .as_bytes();
        let batch = parse_limit_pool(fixture, &request(LimitPoolKind::Upper)).unwrap();
        let row = &batch.records()[0];
        assert_eq!(row.kind, LimitPoolKind::Upper);
        assert_eq!(row.instrument.code(), "600396");
        assert_eq!(row.trading_date.as_str(), "2026-07-23");
        assert_eq!(row.price.get(), 1308.0);
        assert_eq!(row.change.get(), 9.97);
        assert_eq!(row.change.unit(), RatioUnit::Percent);
        assert!(row.volume.is_none());
        assert_eq!(row.turnover.unwrap().get(), 4.2);
        assert_eq!(row.sealed_amount.unwrap().get(), 12345678.0);
        assert_eq!(row.first_seal_at.as_ref().unwrap().as_str(), "09:31:00");
        assert_eq!(row.last_seal_at.as_ref().unwrap().as_str(), "14:55:01");
        assert_eq!(row.break_count, Some(2));
        assert_eq!(row.streak.unwrap().get(), 3);
        assert_eq!(row.industry.as_ref().unwrap().as_str(), "电力行业");
        assert!(row.board_name.is_none());
        assert!(row.seal_state.is_none());
        assert!(row.reseal_count.is_none());
        assert!(row.reason.is_none());
        assert_eq!(row.evidence.source_at(), Some("2026-07-23"));
    }

    #[test]
    fn source_qdate_is_required_calendar_valid_and_matches_the_request() {
        for fixture in [
            r#"{"rc":0,"data":{"pool":[{
              "c":"600396","m":1,"p":1308000,"zdp":9.97
            }]}}"#,
            r#"{"rc":0,"data":{"qdate":20260722,"pool":[{
              "c":"600396","m":1,"p":1308000,"zdp":9.97
            }]}}"#,
            r#"{"rc":0,"data":{"qdate":20260230,"pool":[{
              "c":"600396","m":1,"p":1308000,"zdp":9.97
            }]}}"#,
            r#"{"rc":0,"data":{"qdate":"2026-07-23","pool":[{
              "c":"600396","m":1,"p":1308000,"zdp":9.97
            }]}}"#,
        ] {
            assert!(
                parse_limit_pool(fixture.as_bytes(), &request(LimitPoolKind::PreviousUpper))
                    .is_err(),
                "{fixture}"
            );
        }
    }

    #[test]
    fn source_seal_times_require_a_real_hh_mm_ss_clock() {
        for valid in ["93100", "093100", "09:31:00"] {
            assert_eq!(
                format_time(Some(valid.into())).unwrap(),
                Some("09:31:00".into())
            );
        }
        for invalid in ["246000", "99:99:99", "09:60:00", "09:31", "1234567"] {
            assert!(format_time(Some(invalid.into())).is_err(), "{invalid}");
        }
    }

    #[test]
    fn null_pool_and_nonzero_rc_fail() {
        assert!(
            parse_limit_pool(br#"{"rc":0,"data":null}"#, &request(LimitPoolKind::Lower)).is_err()
        );
        assert!(
            parse_limit_pool(br#"{"rc":-1,"data":null}"#, &request(LimitPoolKind::Broken)).is_err()
        );
    }
}
