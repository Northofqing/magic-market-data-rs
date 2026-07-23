use crate::datacenter_api::fetch_rows;
use crate::mapping::{iso_date, money, optional_f64, optional_string, percent, required_string};
use crate::{
    validate_instrument, validate_source_instrument, validate_source_secucode, BatchContext,
    EastmoneyClient, EastmoneyError,
};
use magic_market_core::{
    DragonTigerData, DragonTigerEntry, DragonTigerSeat, DragonTigerSide, InstrumentSignalRequest,
    Money, NonEmptyText, PositiveU32,
};
use serde_json::Value;
use std::collections::HashSet;

impl DragonTigerData for EastmoneyClient {
    type Error = EastmoneyError;

    fn dragon_tiger_entries(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<magic_market_core::DataBatch<DragonTigerEntry>, Self::Error> {
        validate_instrument(request.instrument())?;
        let filter = signal_filter(request, None);
        let rows = fetch_rows(
            self,
            "RPT_DAILYBILLBOARD_DETAILSNEW",
            &filter,
            "TRADE_DATE",
            request.limit().get(),
        )?;
        map_entries(&rows, request)
    }

    fn dragon_tiger_seats(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<magic_market_core::DataBatch<DragonTigerSeat>, Self::Error> {
        validate_instrument(request.instrument())?;
        let source_date = match request.trading_date() {
            Some(date) => Some(date.as_str().to_owned()),
            None => {
                let latest = fetch_rows(
                    self,
                    "RPT_DAILYBILLBOARD_DETAILSNEW",
                    &signal_filter(request, None),
                    "TRADE_DATE",
                    1,
                )?;
                if let Some(row) = latest.first() {
                    validate_signal_row(row, request, None)?;
                }
                latest
                    .first()
                    .map(|row| required_string(row, "TRADE_DATE"))
                    .transpose()?
                    .map(|value| {
                        value
                            .get(..10)
                            .ok_or_else(|| {
                                EastmoneyError::Protocol(
                                    "dragon-tiger date has no YYYY-MM-DD prefix".into(),
                                )
                            })
                            .map(str::to_owned)
                    })
                    .transpose()?
            }
        };
        let Some(source_date) = source_date else {
            return BatchContext::new("dragon-tiger-seats", None)?.finish(Vec::new());
        };
        let filter = signal_filter(request, Some(&source_date));
        let mut rows = Vec::new();
        for (report, side, sort) in [
            ("RPT_BILLBOARD_DAILYDETAILSBUY", DragonTigerSide::Buy, "BUY"),
            (
                "RPT_BILLBOARD_DAILYDETAILSSELL",
                DragonTigerSide::Sell,
                "SELL",
            ),
        ] {
            let side_rows = fetch_rows(self, report, &filter, sort, request.limit().get())?;
            rows.extend(side_rows.into_iter().map(|row| (side, row)));
        }
        map_seats(&rows, request, &source_date)
    }
}

fn signal_filter(request: &InstrumentSignalRequest, forced_date: Option<&str>) -> String {
    let mut filter = format!("(SECURITY_CODE=\"{}\")", request.instrument().code());
    let date = forced_date.or_else(|| {
        request
            .trading_date()
            .map(magic_market_core::IsoDate::as_str)
    });
    if let Some(date) = date {
        filter.push_str(&format!("(TRADE_DATE='{date}')"));
    }
    filter
}

fn map_entries(
    rows: &[Value],
    request: &InstrumentSignalRequest,
) -> Result<magic_market_core::DataBatch<DragonTigerEntry>, EastmoneyError> {
    let source_at = rows
        .iter()
        .filter_map(|row| optional_string(row.get("TRADE_DATE")).ok().flatten())
        .max();
    let context = BatchContext::new("dragon-tiger-entries", source_at.as_deref())?;
    let records = rows
        .iter()
        .map(|row| {
            validate_signal_row(
                row,
                request,
                request.trading_date().map(|date| date.as_str()),
            )?;
            let date = required_string(row, "TRADE_DATE")?;
            let buy_amount = opt_money(row, "BILLBOARD_BUY_AMT")?;
            let sell_amount = opt_money(row, "BILLBOARD_SELL_AMT")?;
            let net_amount = opt_money(row, "BILLBOARD_NET_AMT")?;
            validate_amount_arithmetic("dragon-tiger entry", buy_amount, sell_amount, net_amount)?;
            Ok(DragonTigerEntry {
                entry_id: entry_id(request.instrument().code(), &date)?,
                instrument: request.instrument().clone(),
                trading_date: iso_date(&date)?,
                reason: optional_string(row.get("EXPLANATION"))?
                    .map(NonEmptyText::new)
                    .transpose()?,
                buy_amount,
                sell_amount,
                net_amount,
                turnover_rate: percent(optional_f64(row.get("TURNOVERRATE"))?)?,
                evidence: context.evidence_at(Some(&date))?,
            })
        })
        .collect::<Result<Vec<_>, EastmoneyError>>()?;
    reject_duplicates(
        records.iter().map(|record| record.entry_id.as_str()),
        "dragon-tiger entry",
    )?;
    context.finish(records)
}

fn map_seats(
    rows: &[(DragonTigerSide, Value)],
    request: &InstrumentSignalRequest,
    source_date: &str,
) -> Result<magic_market_core::DataBatch<DragonTigerSeat>, EastmoneyError> {
    let context = BatchContext::new("dragon-tiger-seats", Some(source_date))?;
    let mut buy_rank = 0_u32;
    let mut sell_rank = 0_u32;
    let mut seat_identities = HashSet::with_capacity(rows.len());
    let records = rows
        .iter()
        .map(|(side, row)| {
            validate_signal_row(row, request, Some(source_date))?;
            let rank = match side {
                DragonTigerSide::Buy => {
                    buy_rank = buy_rank
                        .checked_add(1)
                        .ok_or_else(|| EastmoneyError::Protocol("buy rank overflow".into()))?;
                    buy_rank
                }
                DragonTigerSide::Sell => {
                    sell_rank = sell_rank
                        .checked_add(1)
                        .ok_or_else(|| EastmoneyError::Protocol("sell rank overflow".into()))?;
                    sell_rank
                }
            };
            let amount_key = match side {
                DragonTigerSide::Buy => "BUY",
                DragonTigerSide::Sell => "SELL",
            };
            let amount = required_money(row, amount_key)?;
            let buy_amount = opt_money(row, "BUY")?;
            let sell_amount = opt_money(row, "SELL")?;
            let net_amount = opt_money(row, "NET")?;
            validate_amount_arithmetic("dragon-tiger seat", buy_amount, sell_amount, net_amount)?;
            let seat_name = NonEmptyText::new(required_string(row, "OPERATEDEPT_NAME")?)?;
            let seat_identity = format!("{source_date}:{side:?}:{seat_name}");
            if !seat_identities.insert(seat_identity.clone()) {
                return Err(EastmoneyError::Protocol(format!(
                    "duplicate dragon-tiger seat business identity {seat_identity}"
                )));
            }
            Ok(DragonTigerSeat {
                entry_id: entry_id(request.instrument().code(), source_date)?,
                side: *side,
                rank: PositiveU32::new(rank)?,
                seat_name,
                amount,
                buy_amount,
                sell_amount,
                net_amount,
                evidence: context.evidence_at(Some(source_date))?,
            })
        })
        .collect::<Result<Vec<_>, EastmoneyError>>()?;
    context.finish(records)
}

fn entry_id(code: &str, date: &str) -> Result<NonEmptyText, EastmoneyError> {
    let date = date
        .get(..10)
        .ok_or_else(|| EastmoneyError::Protocol("dragon-tiger date is too short".into()))?;
    Ok(NonEmptyText::new(format!("{code}:{date}"))?)
}

fn validate_signal_row(
    row: &Value,
    request: &InstrumentSignalRequest,
    expected_date: Option<&str>,
) -> Result<(), EastmoneyError> {
    // All three verified dragon-tiger datacenter reports use the same real
    // identity fields: SECURITY_CODE + SECUCODE. Require both so the request
    // filter cannot be mistaken for response-side provenance.
    let source_code = required_string(row, "SECURITY_CODE")?;
    let secucode = required_string(row, "SECUCODE")?;
    validate_source_instrument(request.instrument(), &source_code, None)?;
    validate_source_secucode(request.instrument(), &secucode)?;
    let source_date = required_string(row, "TRADE_DATE")?;
    let actual = iso_date(&source_date)?;
    if let Some(expected_date) = expected_date {
        let expected = iso_date(expected_date)?;
        if actual != expected {
            return Err(EastmoneyError::Protocol(format!(
                "Eastmoney dragon-tiger source date {} does not match requested date {}",
                actual.as_str(),
                expected.as_str()
            )));
        }
    }
    Ok(())
}

fn opt_money(row: &Value, key: &'static str) -> Result<Option<Money>, EastmoneyError> {
    money(optional_f64(row.get(key))?)
}

fn required_money(row: &Value, key: &'static str) -> Result<Money, EastmoneyError> {
    opt_money(row, key)?
        .ok_or_else(|| EastmoneyError::Protocol(format!("dragon-tiger field {key} is absent")))
}

fn validate_amount_arithmetic(
    family: &str,
    buy: Option<Money>,
    sell: Option<Money>,
    net: Option<Money>,
) -> Result<(), EastmoneyError> {
    for (field, amount) in [("buy", buy), ("sell", sell)] {
        if amount.is_some_and(|value| value.get() < 0.0) {
            return Err(EastmoneyError::Protocol(format!(
                "{family} {field} gross amount must be non-negative"
            )));
        }
    }
    if let (Some(buy), Some(sell), Some(net)) = (buy, sell, net) {
        let expected = buy.get() - sell.get();
        let scale = buy
            .get()
            .abs()
            .max(sell.get().abs())
            .max(net.get().abs())
            .max(1.0);
        if (expected - net.get()).abs() > f64::EPSILON * scale * 8.0 {
            return Err(EastmoneyError::Protocol(format!(
                "{family} net amount {} does not equal buy {} minus sell {}",
                net.get(),
                buy.get(),
                sell.get()
            )));
        }
    }
    Ok(())
}

fn reject_duplicates<'a>(
    identities: impl IntoIterator<Item = &'a str>,
    family: &str,
) -> Result<(), EastmoneyError> {
    let mut seen = HashSet::new();
    for identity in identities {
        if !seen.insert(identity) {
            return Err(EastmoneyError::Protocol(format!(
                "duplicate {family} business identity {identity}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/dragon_tiger_tests.rs"]
mod tests;
