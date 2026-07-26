use crate::datacenter_api::fetch_all_rows;
use crate::mapping::{iso_date, optional_f64, optional_string, percent, required_string};
use crate::{source_instrument, BatchContext, EastmoneyClient, EastmoneyError};
use magic_market_core::{
    AssetClass, DragonTigerDiscovery, DragonTigerDiscoveryRequest, DragonTigerEntry, Exchange,
    InstrumentId, Money, NonEmptyText,
};
use serde_json::Value;
use std::collections::HashSet;

impl DragonTigerDiscovery for EastmoneyClient {
    type Error = EastmoneyError;

    fn discover_dragon_tiger(
        &self,
        request: &DragonTigerDiscoveryRequest,
    ) -> Result<magic_market_core::DataBatch<DragonTigerEntry>, Self::Error> {
        let date = request.trading_date().as_str();
        let filter = format!("(TRADE_DATE='{date}')");
        let rows = fetch_all_rows(
            self,
            "RPT_DAILYBILLBOARD_DETAILSNEW",
            &filter,
            "TRADE_ID",
            10_000,
        )?;
        let context = BatchContext::new("dragon-tiger-discovery", Some(date))?;
        let mut source_ids = HashSet::with_capacity(rows.len());
        let mut records = Vec::with_capacity(rows.len().min(request.limit().get() as usize));

        for row in &rows {
            let trade_id = positive_trade_id(row.get("TRADE_ID"))?;
            if !source_ids.insert(trade_id) {
                return Err(EastmoneyError::Protocol(format!(
                    "duplicate Eastmoney dragon-tiger TRADE_ID {trade_id}"
                )));
            }
            let instrument = row_instrument(row)?;
            let source_date_text = required_string(row, "TRADE_DATE")?;
            let source_date = iso_date(&source_date_text)?;
            if &source_date != request.trading_date() {
                return Err(EastmoneyError::Protocol(format!(
                    "Eastmoney dragon-tiger source date {} does not match requested date {date}",
                    source_date.as_str()
                )));
            }

            if request
                .exchange()
                .is_some_and(|exchange| instrument.exchange() != exchange)
            {
                continue;
            }
            if records.len() == request.limit().get() as usize {
                continue;
            }

            let instrument_name = NonEmptyText::new(required_string(row, "SECURITY_NAME_ABBR")?)?;
            records.push(
                DragonTigerEntry::new(
                    NonEmptyText::new(format!("eastmoney:{date}:{trade_id}"))?,
                    instrument,
                    source_date,
                    optional_string(row.get("EXPLANATION"))?
                        .map(NonEmptyText::new)
                        .transpose()?,
                    optional_money(row, "BILLBOARD_BUY_AMT")?,
                    optional_money(row, "BILLBOARD_SELL_AMT")?,
                    optional_money(row, "BILLBOARD_NET_AMT")?,
                    percent(optional_f64(row.get("TURNOVERRATE"))?)?,
                    context.evidence_at(Some(&source_date_text))?,
                )?
                .with_instrument_name(instrument_name),
            );
        }

        context.finish_allow_empty(records)
    }
}

fn positive_trade_id(value: Option<&Value>) -> Result<u64, EastmoneyError> {
    let parsed = match value {
        Some(Value::Number(number)) => number.as_u64(),
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
                None
            } else {
                trimmed.parse::<u64>().ok()
            }
        }
        _ => None,
    }
    .filter(|value| *value > 0);
    parsed.ok_or_else(|| {
        EastmoneyError::Protocol(
            "dragon-tiger TRADE_ID must be a positive integral source identity".into(),
        )
    })
}

fn row_instrument(row: &Value) -> Result<magic_market_core::InstrumentId, EastmoneyError> {
    let source_code = required_string(row, "SECURITY_CODE")?;
    let secucode = required_string(row, "SECUCODE")?;
    let (secucode_code, suffix) = secucode.split_once('.').ok_or_else(|| {
        EastmoneyError::Protocol(format!(
            "Eastmoney source SECUCODE {secucode:?} has no exchange suffix"
        ))
    })?;
    if secucode_code != source_code {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney SECURITY_CODE {source_code:?} disagrees with SECUCODE {secucode:?}"
        )));
    }
    let exchange = match suffix.to_ascii_uppercase().as_str() {
        "SH" => Exchange::Shanghai,
        "SZ" => Exchange::Shenzhen,
        "BJ" => Exchange::Beijing,
        _ => {
            return Err(EastmoneyError::Protocol(format!(
                "unsupported Eastmoney SECUCODE suffix {suffix:?}"
            )))
        }
    };
    let security_type = required_string(row, "SECURITY_TYPE_CODE")?;
    match security_type.as_str() {
        "058001001" => source_instrument(&source_code, exchange),
        "060" => Ok(InstrumentId::new(exchange, source_code, AssetClass::Bond)?),
        _ => Err(EastmoneyError::Protocol(format!(
            "unsupported Eastmoney dragon-tiger SECURITY_TYPE_CODE {security_type:?}"
        ))),
    }
}

fn optional_money(row: &Value, key: &'static str) -> Result<Option<Money>, EastmoneyError> {
    crate::mapping::money(optional_f64(row.get(key))?)
}

#[cfg(test)]
#[path = "../tests/internal/discovery_tests.rs"]
mod tests;
