use crate::mapping::{
    iso_date, money, optional_f64, optional_string, percent, validate_minute_timestamp,
};
use crate::{
    instrument_from_market, query_url, secid, BatchContext, EastmoneyClient, EastmoneyError,
};
use magic_market_core::{FlowInterval, FlowScope, FundFlowPoint, FundFlowRequest, FundFlowSeries};

/// Fixed public Eastmoney fund-flow is admitted for one Shanghai or Shenzhen
/// equity, one-minute or daily intervals, exact CNY values and per-row source
/// time.
pub const PUBLIC_FUND_FLOW_ADMITTED: bool = true;
use serde_json::Value;

const MINUTE_ENDPOINT: &str = "https://push2.eastmoney.com/api/qt/stock/fflow/kline/get";
// Eastmoney's current `kline` contract accepts `klt=101` and returns the same
// date plus five net-flow fields used by this adapter. The exact official
// delay host is used for daily reads because the primary currently closes its
// TLS stream without the authenticated close required by the Rust transport.
const DAILY_ENDPOINT: &str = "https://push2delay.eastmoney.com/api/qt/stock/fflow/kline/get";

impl FundFlowSeries for EastmoneyClient {
    type Error = EastmoneyError;

    fn fund_flow_series(
        &self,
        request: &FundFlowRequest,
    ) -> Result<magic_market_core::DataBatch<FundFlowPoint>, Self::Error> {
        let FlowScope::Instrument(instrument) = request.scope() else {
            return Err(EastmoneyError::Unsupported(
                "board fund flow uses the BoardFlows contract".into(),
            ));
        };
        let endpoint = match request.interval() {
            FlowInterval::Minute1 => MINUTE_ENDPOINT,
            FlowInterval::Day1 => DAILY_ENDPOINT,
            interval => {
                return Err(EastmoneyError::Unsupported(format!(
                    "instrument fund-flow interval {interval:?} is not verified"
                )))
            }
        };
        let url = query_url(
            endpoint,
            &[
                ("secid", secid(instrument)?),
                (
                    "klt",
                    if request.interval() == FlowInterval::Minute1 {
                        "1".into()
                    } else {
                        "101".into()
                    },
                ),
                ("lmt", request.limit().get().to_string()),
                ("fields1", "f1,f2,f3,f7".into()),
                (
                    "fields2",
                    if request.interval() == FlowInterval::Minute1 {
                        "f51,f52,f53,f54,f55,f56,f57".into()
                    } else {
                        "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64,f65".into()
                    },
                ),
            ],
        );
        let bytes = self.get(
            &url,
            &[
                ("Accept", "application/json"),
                ("Referer", "https://quote.eastmoney.com/"),
                ("Origin", "https://quote.eastmoney.com"),
            ],
        )?;
        parse_fund_flow(
            &bytes,
            FlowScope::Instrument(instrument.clone()),
            request.interval(),
        )
    }
}

fn parse_fund_flow(
    bytes: &[u8],
    scope: FlowScope,
    interval: FlowInterval,
) -> Result<magic_market_core::DataBatch<FundFlowPoint>, EastmoneyError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    if root.get("rc").and_then(Value::as_i64) != Some(0) {
        return Err(EastmoneyError::Protocol(format!(
            "fund-flow endpoint returned rc={}",
            root.get("rc").unwrap_or(&Value::Null)
        )));
    }
    let FlowScope::Instrument(expected_instrument) = &scope else {
        return Err(EastmoneyError::Unsupported(
            "fund-flow parser requires an instrument scope".into(),
        ));
    };
    let source_code = optional_string(root.pointer("/data/code"))?
        .ok_or_else(|| EastmoneyError::Protocol("fund-flow source code is absent".into()))?;
    let source_market = optional_f64(root.pointer("/data/market"))?
        .ok_or_else(|| EastmoneyError::Protocol("fund-flow source market is absent".into()))?;
    if source_market.fract() != 0.0 {
        return Err(EastmoneyError::Protocol(
            "fund-flow source market is not integral".into(),
        ));
    }
    let source_instrument = instrument_from_market(&source_code, source_market as i64)?;
    if &source_instrument != expected_instrument {
        return Err(EastmoneyError::Protocol(format!(
            "fund-flow source instrument {:?}.{} does not match requested {:?}.{}",
            source_instrument.exchange(),
            source_instrument.code(),
            expected_instrument.exchange(),
            expected_instrument.code()
        )));
    }
    let rows: &[Value] = match root.pointer("/data/klines") {
        Some(Value::Array(rows)) => rows,
        None | Some(Value::Null) if root.get("data").is_none_or(Value::is_null) => &[],
        _ => {
            return Err(EastmoneyError::Protocol(
                "fund-flow data.klines is not an array".into(),
            ))
        }
    };
    let parsed = rows
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| EastmoneyError::Protocol("fund-flow row is not a string".into()))
                .and_then(|row| parse_row(row, interval))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source_at = parsed.iter().map(|row| row.source_at.as_str()).max();
    let context = BatchContext::new("fund-flow", source_at)?;
    let records = parsed
        .into_iter()
        .map(|row| {
            Ok(FundFlowPoint {
                scope: scope.clone(),
                interval,
                period_at: magic_market_core::NonEmptyText::new(row.period_at.clone())?,
                main_net: money(row.main_net)?,
                main_ratio: percent(row.main_ratio)?,
                super_large_net: money(row.super_large_net)?,
                large_net: money(row.large_net)?,
                medium_net: money(row.medium_net)?,
                small_net: money(row.small_net)?,
                evidence: context.evidence_at(Some(&row.source_at))?,
            })
        })
        .collect::<Result<Vec<_>, EastmoneyError>>()?;
    context.finish(records)
}

#[derive(Debug)]
struct FlowRow {
    period_at: String,
    source_at: String,
    main_net: Option<f64>,
    small_net: Option<f64>,
    medium_net: Option<f64>,
    large_net: Option<f64>,
    super_large_net: Option<f64>,
    main_ratio: Option<f64>,
}

fn parse_row(row: &str, interval: FlowInterval) -> Result<FlowRow, EastmoneyError> {
    let fields = row.split(',').collect::<Vec<_>>();
    if fields.len() < 6 {
        return Err(EastmoneyError::Protocol(format!(
            "fund-flow row has {} fields, expected at least 6",
            fields.len()
        )));
    }
    let period_at = fields[0].trim();
    if period_at.is_empty() {
        return Err(EastmoneyError::Protocol("fund-flow period is empty".into()));
    }
    let source_at = match interval {
        FlowInterval::Minute1 => {
            validate_minute_timestamp(period_at, "fund-flow period_at")?;
            let (date, time) = period_at.split_once(' ').ok_or_else(|| {
                EastmoneyError::Protocol("fund-flow minute period has no separator".into())
            })?;
            format!("{date}T{time}:00+08:00")
        }
        FlowInterval::Day1 => {
            let source_date = iso_date(period_at)?;
            if source_date.as_str() != period_at {
                return Err(EastmoneyError::Protocol(format!(
                    "daily fund-flow period_at {period_at:?} must use YYYY-MM-DD"
                )));
            }
            period_at.to_owned()
        }
        other => {
            return Err(EastmoneyError::Unsupported(format!(
                "fund-flow parser interval {other:?} is not verified"
            )))
        }
    };
    Ok(FlowRow {
        period_at: period_at.to_owned(),
        source_at,
        main_net: parse_number(fields[1])?,
        small_net: parse_number(fields[2])?,
        medium_net: parse_number(fields[3])?,
        large_net: parse_number(fields[4])?,
        super_large_net: parse_number(fields[5])?,
        main_ratio: fields
            .get(6)
            .map(|value| parse_number(value))
            .transpose()?
            .flatten(),
    })
}

fn parse_number(value: &str) -> Result<Option<f64>, EastmoneyError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || matches!(trimmed, "-" | "--") {
        return Ok(None);
    }
    let value = trimmed.parse::<f64>().map_err(|error| {
        EastmoneyError::Protocol(format!("invalid fund-flow number {trimmed}: {error}"))
    })?;
    if !value.is_finite() {
        return Err(EastmoneyError::Protocol(
            "fund-flow number is not finite".into(),
        ));
    }
    Ok(Some(value))
}

#[cfg(test)]
#[path = "../tests/internal/fund_flow_tests.rs"]
mod tests;
