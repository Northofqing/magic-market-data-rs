use crate::datacenter_api::fetch_rows;
use crate::mapping::{iso_date, money, optional_f64, optional_string, percent, required_string};
use crate::{
    source_instrument, validate_instrument, validate_source_instrument, validate_source_secucode,
    BatchContext, EastmoneyClient, EastmoneyError,
};
use magic_market_core::{
    DragonTigerData, DragonTigerDisclosure, DragonTigerEntry, DragonTigerSeat, DragonTigerSide,
    Exchange, InstrumentSignalRequest, MarketDragonTigerData, MarketDragonTigerRequest, Money,
    NonEmptyText, PositiveU32,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

const SEAT_SIDE_CARDINALITY: u32 = 5;
const SEAT_SIDE_FETCH_LIMIT: u32 = SEAT_SIDE_CARDINALITY + 1;
const MAX_MARKET_DISCOVERY_ROWS: u32 = 10_000;
const A_SHARE_SECURITY_TYPE_CODE: &str = "058001001";

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
        validate_seat_limit(request)?;
        let selected = fetch_rows(
            self,
            "RPT_DAILYBILLBOARD_DETAILSNEW",
            &signal_filter(request, None),
            "TRADE_DATE",
            1,
        )?;
        let Some(selected) = selected.first() else {
            return BatchContext::new("dragon-tiger-seats", None)?.finish(Vec::new());
        };
        validate_signal_row(
            selected,
            request,
            request.trading_date().map(|date| date.as_str()),
        )?;
        let source_date = required_string(selected, "TRADE_DATE")?;
        let source_date = source_date
            .get(..10)
            .ok_or_else(|| {
                EastmoneyError::Protocol("dragon-tiger date has no YYYY-MM-DD prefix".into())
            })?
            .to_owned();
        let selected_trade_id = trade_id(selected)?;
        let filter = format!(
            "{}(TRADE_ID=\"{selected_trade_id}\")",
            signal_filter(request, Some(&source_date))
        );
        let mut rows = Vec::new();
        for (report, side, sort) in [
            ("RPT_BILLBOARD_DAILYDETAILSBUY", DragonTigerSide::Buy, "BUY"),
            (
                "RPT_BILLBOARD_DAILYDETAILSSELL",
                DragonTigerSide::Sell,
                "SELL",
            ),
        ] {
            // Read one sentinel row beyond the admitted top five so an
            // oversized upstream result cannot be hidden by fetch_rows'
            // caller-limit truncation.
            let side_rows = fetch_rows(self, report, &filter, sort, SEAT_SIDE_FETCH_LIMIT)?;
            rows.extend(side_rows.into_iter().map(|row| (side, row)));
        }
        map_seats(&rows, request, &source_date)
    }
}

impl MarketDragonTigerData for EastmoneyClient {
    type Error = EastmoneyError;

    fn market_dragon_tiger(
        &self,
        request: &MarketDragonTigerRequest,
    ) -> Result<magic_market_core::DataBatch<DragonTigerDisclosure>, Self::Error> {
        let filter = format!(
            "(TRADE_DATE='{}')(SECURITY_TYPE_CODE=\"{A_SHARE_SECURITY_TYPE_CODE}\")",
            request.trading_date().as_str()
        );
        let rows = fetch_rows(
            self,
            "RPT_DAILYBILLBOARD_DETAILSNEW",
            &filter,
            "BILLBOARD_NET_AMT",
            MAX_MARKET_DISCOVERY_ROWS,
        )?;
        if rows.len() == MAX_MARKET_DISCOVERY_ROWS as usize {
            return Err(EastmoneyError::Protocol(format!(
                "whole-market dragon-tiger discovery reached the {MAX_MARKET_DISCOVERY_ROWS}-row safety bound"
            )));
        }
        let entries = map_market_entries(&rows, request)?;
        let provenance = entries.provenance().clone();
        let mut disclosures = Vec::with_capacity(entries.records().len());
        for entry in entries.records() {
            let filter = market_seat_filter(entry)?;
            let mut rows = Vec::with_capacity(10);
            for (report, side, sort) in [
                ("RPT_BILLBOARD_DAILYDETAILSBUY", DragonTigerSide::Buy, "BUY"),
                (
                    "RPT_BILLBOARD_DAILYDETAILSSELL",
                    DragonTigerSide::Sell,
                    "SELL",
                ),
            ] {
                let side_rows = fetch_rows(self, report, &filter, sort, SEAT_SIDE_FETCH_LIMIT)?;
                rows.extend(side_rows.into_iter().map(|row| (side, row)));
            }
            let seats = map_market_seats(&rows, entry)?;
            disclosures.push(DragonTigerDisclosure::new(entry.clone(), seats)?);
        }
        Ok(magic_market_core::DataBatch::strict(
            disclosures,
            provenance,
        ))
    }
}

fn validate_seat_limit(request: &InstrumentSignalRequest) -> Result<(), EastmoneyError> {
    if request.limit().get() < 10 {
        return Err(EastmoneyError::InvalidRequest(
            "dragon-tiger seat limit must be at least 10 for one complete buy-five/sell-five group"
                .into(),
        ));
    }
    Ok(())
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
            let trade_id = trade_id(row)?;
            Ok(DragonTigerEntry::new(
                entry_id(request.instrument().code(), &date, &trade_id)?,
                request.instrument().clone(),
                iso_date(&date)?,
                optional_string(row.get("EXPLANATION"))?
                    .map(NonEmptyText::new)
                    .transpose()?,
                opt_money(row, "BILLBOARD_BUY_AMT")?,
                opt_money(row, "BILLBOARD_SELL_AMT")?,
                opt_money(row, "BILLBOARD_NET_AMT")?,
                percent(optional_f64(row.get("TURNOVERRATE"))?)?,
                context.evidence_at(Some(&date))?,
            )?)
        })
        .collect::<Result<Vec<_>, EastmoneyError>>()?;
    reject_duplicates(
        records.iter().map(|record| record.entry_id().as_str()),
        "dragon-tiger entry",
    )?;
    context.finish(records)
}

fn map_market_entries(
    rows: &[Value],
    request: &MarketDragonTigerRequest,
) -> Result<magic_market_core::DataBatch<DragonTigerEntry>, EastmoneyError> {
    let context = BatchContext::new(
        "market-dragon-tiger-entries",
        Some(request.trading_date().as_str()),
    )?;
    let mapped = rows
        .iter()
        .map(|row| {
            let instrument = source_signal_instrument(row)?;
            let source_date = required_string(row, "TRADE_DATE")?;
            let trading_date = iso_date(&source_date)?;
            if &trading_date != request.trading_date() {
                return Err(EastmoneyError::Protocol(format!(
                    "Eastmoney dragon-tiger source date {} does not match requested date {}",
                    trading_date.as_str(),
                    request.trading_date().as_str()
                )));
            }
            let trade_id = trade_id(row)?;
            Ok(DragonTigerEntry::new(
                entry_id(instrument.code(), &source_date, &trade_id)?,
                instrument,
                trading_date,
                optional_string(row.get("EXPLANATION"))?
                    .map(NonEmptyText::new)
                    .transpose()?,
                opt_money(row, "BILLBOARD_BUY_AMT")?,
                opt_money(row, "BILLBOARD_SELL_AMT")?,
                opt_money(row, "BILLBOARD_NET_AMT")?,
                percent(optional_f64(row.get("TURNOVERRATE"))?)?,
                context.evidence()?,
            )?)
        })
        .collect::<Result<Vec<_>, EastmoneyError>>()?;
    let mut records = Vec::with_capacity(mapped.len());
    let mut positions = HashMap::with_capacity(mapped.len());
    for record in mapped {
        let identity = record.entry_id().as_str().to_owned();
        if let Some(index) = positions.get(&identity).copied() {
            if records.get(index) == Some(&record) {
                continue;
            }
            return Err(EastmoneyError::Protocol(format!(
                "conflicting duplicate dragon-tiger entry {identity}"
            )));
        }
        positions.insert(identity, records.len());
        records.push(record);
    }
    records.sort_by(|left, right| {
        match (left.net_amount(), right.net_amount()) {
            (Some(left), Some(right)) => right.get().total_cmp(&left.get()),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| {
            exchange_order(left.instrument().exchange())
                .cmp(&exchange_order(right.instrument().exchange()))
        })
        .then_with(|| left.instrument().code().cmp(right.instrument().code()))
        .then_with(|| left.entry_id().as_str().cmp(right.entry_id().as_str()))
    });
    records.truncate(request.limit().get() as usize);
    context.finish(records)
}

const fn exchange_order(exchange: Exchange) -> u8 {
    match exchange {
        Exchange::Shanghai => 0,
        Exchange::Shenzhen => 1,
        Exchange::Beijing => 2,
    }
}

fn map_seats(
    rows: &[(DragonTigerSide, Value)],
    request: &InstrumentSignalRequest,
    source_date: &str,
) -> Result<magic_market_core::DataBatch<DragonTigerSeat>, EastmoneyError> {
    let buy_count = rows
        .iter()
        .filter(|(side, _)| *side == DragonTigerSide::Buy)
        .count();
    let sell_count = rows
        .iter()
        .filter(|(side, _)| *side == DragonTigerSide::Sell)
        .count();
    if rows.len() != 10
        || buy_count != SEAT_SIDE_CARDINALITY as usize
        || sell_count != SEAT_SIDE_CARDINALITY as usize
    {
        return Err(EastmoneyError::Protocol(format!(
            "dragon-tiger seats require exactly five buy and five sell rows; got {buy_count} buy and {sell_count} sell"
        )));
    }
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
            let seat_name = NonEmptyText::new(required_string(row, "OPERATEDEPT_NAME")?)?;
            let seat_identity = format!("{source_date}:{side:?}:{seat_name}");
            if !seat_identities.insert(seat_identity.clone()) {
                return Err(EastmoneyError::Protocol(format!(
                    "duplicate dragon-tiger seat business identity {seat_identity}"
                )));
            }
            Ok(DragonTigerSeat::new(
                entry_id(request.instrument().code(), source_date, &trade_id(row)?)?,
                request.instrument().clone(),
                iso_date(source_date)?,
                *side,
                PositiveU32::new(rank)?,
                seat_name,
                amount,
                buy_amount,
                sell_amount,
                net_amount,
                context.evidence_at(Some(source_date))?,
            )?)
        })
        .collect::<Result<Vec<_>, EastmoneyError>>()?;
    context.finish(records)
}

fn map_market_seats(
    rows: &[(DragonTigerSide, Value)],
    entry: &DragonTigerEntry,
) -> Result<Vec<DragonTigerSeat>, EastmoneyError> {
    let buy_count = rows
        .iter()
        .filter(|(side, _)| *side == DragonTigerSide::Buy)
        .count();
    let sell_count = rows
        .iter()
        .filter(|(side, _)| *side == DragonTigerSide::Sell)
        .count();
    if rows.len() != 10
        || buy_count != SEAT_SIDE_CARDINALITY as usize
        || sell_count != SEAT_SIDE_CARDINALITY as usize
    {
        return Err(EastmoneyError::Protocol(format!(
            "dragon-tiger entry {} requires exactly five buy and five sell rows; got {buy_count} buy and {sell_count} sell",
            entry.entry_id()
        )));
    }
    let expected_trade_id = entry_trade_id(entry)?;
    let mut buy_rank = 0_u32;
    let mut sell_rank = 0_u32;
    let mut seat_identities = HashSet::with_capacity(rows.len());
    rows.iter()
        .map(|(side, row)| {
            let instrument = source_signal_instrument(row)?;
            if &instrument != entry.instrument() {
                return Err(EastmoneyError::Protocol(
                    "dragon-tiger seat instrument does not match its discovered entry".into(),
                ));
            }
            let source_date = required_string(row, "TRADE_DATE")?;
            if iso_date(&source_date)? != *entry.trading_date() {
                return Err(EastmoneyError::Protocol(
                    "dragon-tiger seat date does not match its discovered entry".into(),
                ));
            }
            let actual_trade_id = trade_id(row)?;
            if actual_trade_id != expected_trade_id {
                return Err(EastmoneyError::Protocol(format!(
                    "dragon-tiger seat TRADE_ID {actual_trade_id} does not match discovered entry TRADE_ID {expected_trade_id}"
                )));
            }
            let seat_name = NonEmptyText::new(required_string(row, "OPERATEDEPT_NAME")?)?;
            let amount_key = match side {
                DragonTigerSide::Buy => "BUY",
                DragonTigerSide::Sell => "SELL",
            };
            let amount = required_money(row, amount_key)?;
            let buy_amount = opt_money(row, "BUY")?;
            let sell_amount = opt_money(row, "SELL")?;
            let net_amount = opt_money(row, "NET")?;
            let seat_identity = (
                *side,
                seat_name.as_str().to_owned(),
                amount.get().to_bits(),
                buy_amount.map(|value| value.get().to_bits()),
                sell_amount.map(|value| value.get().to_bits()),
                net_amount.map(|value| value.get().to_bits()),
            );
            if !seat_identities.insert(seat_identity) {
                return Err(EastmoneyError::Protocol(format!(
                    "duplicate dragon-tiger seat row for entry {} side {side:?}",
                    entry.entry_id()
                )));
            }
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
            Ok(DragonTigerSeat::new(
                entry.entry_id().clone(),
                entry.instrument().clone(),
                entry.trading_date().clone(),
                *side,
                PositiveU32::new(rank)?,
                seat_name,
                amount,
                buy_amount,
                sell_amount,
                net_amount,
                entry.evidence().clone(),
            )?)
        })
        .collect()
}

fn market_seat_filter(entry: &DragonTigerEntry) -> Result<String, EastmoneyError> {
    Ok(format!(
        "(SECURITY_CODE=\"{}\")(TRADE_DATE='{}')(TRADE_ID=\"{}\")",
        entry.instrument().code(),
        entry.trading_date().as_str(),
        entry_trade_id(entry)?
    ))
}

fn entry_trade_id(entry: &DragonTigerEntry) -> Result<&str, EastmoneyError> {
    let (_, trade_id) = entry.entry_id().as_str().rsplit_once(':').ok_or_else(|| {
        EastmoneyError::Protocol(format!(
            "dragon-tiger entry ID {} has no TRADE_ID segment",
            entry.entry_id()
        ))
    })?;
    if !trade_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EastmoneyError::Protocol(format!(
            "dragon-tiger entry ID {} has an invalid TRADE_ID segment",
            entry.entry_id()
        )));
    }
    Ok(trade_id)
}

fn entry_id(code: &str, date: &str, trade_id: &str) -> Result<NonEmptyText, EastmoneyError> {
    let date = date
        .get(..10)
        .ok_or_else(|| EastmoneyError::Protocol("dragon-tiger date is too short".into()))?;
    Ok(NonEmptyText::new(format!("{code}:{date}:{trade_id}"))?)
}

fn trade_id(row: &Value) -> Result<String, EastmoneyError> {
    let value = required_string(row, "TRADE_ID")?;
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EastmoneyError::Protocol(format!(
            "dragon-tiger TRADE_ID {value:?} must contain only ASCII digits"
        )));
    }
    Ok(value)
}

fn source_signal_instrument(
    row: &Value,
) -> Result<magic_market_core::InstrumentId, EastmoneyError> {
    let code = required_string(row, "SECURITY_CODE")?;
    let secucode = required_string(row, "SECUCODE")?;
    let (_, suffix) = secucode.split_once('.').ok_or_else(|| {
        EastmoneyError::Protocol(format!(
            "Eastmoney source SECUCODE {secucode:?} has no exchange suffix"
        ))
    })?;
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
    let instrument = source_instrument(&code, exchange)?;
    validate_source_secucode(&instrument, &secucode)?;
    Ok(instrument)
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
