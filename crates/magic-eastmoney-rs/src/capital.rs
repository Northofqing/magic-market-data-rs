use crate::datacenter_api::{fetch_rows, instrument_filter};
use crate::mapping::{
    decimal, finite, iso_date, money, non_empty, optional_f64, optional_string, percent, quantity,
    required_string,
};
use crate::{
    validate_instrument, validate_source_instrument, validate_source_secucode, BatchContext,
    EastmoneyClient, EastmoneyError,
};
use magic_market_core::{
    BlockTrade, BlockTrades, DividendPlan, DividendPlans, HolderCount, HolderCounts,
    InstrumentDateRangeRequest, LockupEvent, LockupEvents, MarginBalance, MarginData, Money,
    NonEmptyText, Price, Quantity, Ratio,
};
use serde_json::Value;

impl MarginData for EastmoneyClient {
    type Error = EastmoneyError;

    fn margin_data(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<magic_market_core::DataBatch<MarginBalance>, Self::Error> {
        let rows = capital_rows(self, request, "RPTA_WEB_RZRQ_GGMX", "SCODE", "DATE", "DATE")?;
        let context = context_for_rows("margin", &rows, "DATE")?;
        let records = rows
            .iter()
            .map(|row| map_margin(row, request, &context))
            .collect::<Result<Vec<_>, _>>()?;
        context.finish(records)
    }
}

impl BlockTrades for EastmoneyClient {
    type Error = EastmoneyError;

    fn block_trades(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<magic_market_core::DataBatch<BlockTrade>, Self::Error> {
        let rows = capital_rows(
            self,
            request,
            "RPT_DATA_BLOCKTRADE",
            "SECURITY_CODE",
            "TRADE_DATE",
            "TRADE_DATE",
        )?;
        let context = context_for_rows("block-trades", &rows, "TRADE_DATE")?;
        let records = rows
            .iter()
            .map(|row| map_block_trade(row, request, &context))
            .collect::<Result<Vec<_>, _>>()?;
        context.finish(records)
    }
}

impl HolderCounts for EastmoneyClient {
    type Error = EastmoneyError;

    fn holder_counts(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<magic_market_core::DataBatch<HolderCount>, Self::Error> {
        let rows = capital_rows(
            self,
            request,
            "RPT_HOLDERNUMLATEST",
            "SECURITY_CODE",
            "END_DATE",
            "END_DATE",
        )?;
        let context = context_for_rows("holder-counts", &rows, "END_DATE")?;
        let records = rows
            .iter()
            .map(|row| map_holder_count(row, request, &context))
            .collect::<Result<Vec<_>, _>>()?;
        context.finish(records)
    }
}

impl LockupEvents for EastmoneyClient {
    type Error = EastmoneyError;

    fn lockup_events(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<magic_market_core::DataBatch<LockupEvent>, Self::Error> {
        let rows = capital_rows(
            self,
            request,
            "RPT_LIFT_STAGE",
            "SECURITY_CODE",
            "FREE_DATE",
            "FREE_DATE",
        )?;
        let context = context_for_rows("lockups", &rows, "FREE_DATE")?;
        let records = rows
            .iter()
            .map(|row| map_lockup(row, request, &context))
            .collect::<Result<Vec<_>, _>>()?;
        context.finish(records)
    }
}

impl DividendPlans for EastmoneyClient {
    type Error = EastmoneyError;

    fn dividend_plans(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<magic_market_core::DataBatch<DividendPlan>, Self::Error> {
        let rows = capital_rows(
            self,
            request,
            "RPT_SHAREBONUS_DET",
            "SECURITY_CODE",
            "REPORT_DATE",
            "EX_DIVIDEND_DATE",
        )?;
        let context = context_for_rows("dividends", &rows, "REPORT_DATE")?;
        let records = rows
            .iter()
            .map(|row| map_dividend(row, request, &context))
            .collect::<Result<Vec<_>, _>>()?;
        context.finish(records)
    }
}

fn capital_rows(
    client: &EastmoneyClient,
    request: &InstrumentDateRangeRequest,
    report_name: &str,
    code_column: &str,
    date_column: &str,
    sort_column: &str,
) -> Result<Vec<Value>, EastmoneyError> {
    validate_instrument(request.instrument())?;
    let filter = instrument_filter(
        code_column,
        request.instrument().code(),
        date_column,
        request.start(),
        request.end(),
    );
    fetch_rows(
        client,
        report_name,
        &filter,
        sort_column,
        request.limit().get(),
    )
}

fn context_for_rows(
    family: &str,
    rows: &[Value],
    date_key: &'static str,
) -> Result<BatchContext, EastmoneyError> {
    let source_at = rows
        .iter()
        .filter_map(|row| optional_string(row.get(date_key)).ok().flatten())
        .max();
    BatchContext::new(family, source_at.as_deref())
}

fn map_margin(
    row: &Value,
    request: &InstrumentDateRangeRequest,
    context: &BatchContext,
) -> Result<MarginBalance, EastmoneyError> {
    validate_capital_row(row, request, "SCODE", "DATE")?;
    let source_at = required_string(row, "DATE")?;
    Ok(MarginBalance {
        instrument: request.instrument().clone(),
        trading_date: iso_date(&source_at)?,
        financing_balance: opt_money(row, "RZYE")?,
        financing_buy: opt_money(row, "RZMRE")?,
        financing_repayment: opt_money(row, "RZCHE")?,
        securities_lending_balance: opt_money(row, "RQYE")?,
        securities_lending_sell: opt_quantity(row, "RQMCL")?,
        securities_lending_repayment: opt_quantity(row, "RQCHL")?,
        total_balance: opt_money(row, "RZRQYE")?,
        evidence: context.evidence_at(Some(&source_at))?,
    })
}

fn map_block_trade(
    row: &Value,
    request: &InstrumentDateRangeRequest,
    context: &BatchContext,
) -> Result<BlockTrade, EastmoneyError> {
    validate_capital_row(row, request, "SECURITY_CODE", "TRADE_DATE")?;
    let source_at = required_string(row, "TRADE_DATE")?;
    Ok(BlockTrade {
        instrument: request.instrument().clone(),
        trading_date: iso_date(&source_at)?,
        traded_at: non_empty(optional_string(row.get("TRADE_TIME"))?)?,
        price: required_price(row, "DEAL_PRICE")?,
        close_price: optional_f64(row.get("CLOSE_PRICE"))?
            .map(Price::new)
            .transpose()?,
        premium_ratio: opt_decimal(row, "PREMIUM_RATIO")?,
        volume: required_quantity(row, "DEAL_VOLUME")?,
        amount: opt_money(row, "DEAL_AMT")?,
        buyer: non_empty(optional_string(row.get("BUYER_NAME"))?)?,
        seller: non_empty(optional_string(row.get("SELLER_NAME"))?)?,
        evidence: context.evidence_at(Some(&source_at))?,
    })
}

fn map_holder_count(
    row: &Value,
    request: &InstrumentDateRangeRequest,
    context: &BatchContext,
) -> Result<HolderCount, EastmoneyError> {
    validate_capital_row(row, request, "SECURITY_CODE", "END_DATE")?;
    let source_at = required_string(row, "END_DATE")?;
    Ok(HolderCount {
        instrument: request.instrument().clone(),
        report_date: iso_date(&source_at)?,
        holders: required_quantity(row, "HOLDER_NUM")?,
        holder_change: finite(optional_f64(row.get("HOLDER_NUM_CHANGE"))?)?,
        change_ratio: opt_percent(row, "HOLDER_NUM_RATIO")?,
        average_shares_per_holder: opt_quantity(row, "AVG_FREE_SHARES")?,
        evidence: context.evidence_at(Some(&source_at))?,
    })
}

fn map_lockup(
    row: &Value,
    request: &InstrumentDateRangeRequest,
    context: &BatchContext,
) -> Result<LockupEvent, EastmoneyError> {
    validate_capital_row(row, request, "SECURITY_CODE", "FREE_DATE")?;
    let source_at = required_string(row, "FREE_DATE")?;
    Ok(LockupEvent {
        instrument: request.instrument().clone(),
        listing_date: iso_date(&source_at)?,
        share_type: NonEmptyText::new(required_string(row, "FREE_SHARES_TYPE")?)?,
        shares: required_scaled_quantity(row, "FREE_SHARES", 10_000.0)?,
        able_shares: optional_f64(row.get("ABLE_FREE_SHARES"))?
            .map(|value| Quantity::new(value * 10_000.0))
            .transpose()?,
        free_float_ratio: opt_decimal(row, "FREE_RATIO")?,
        market_value: optional_f64(row.get("LIFT_MARKET_CAP"))?
            .map(|value| Money::new(value * 10_000.0))
            .transpose()?,
        evidence: context.evidence_at(Some(&source_at))?,
    })
}

fn map_dividend(
    row: &Value,
    request: &InstrumentDateRangeRequest,
    context: &BatchContext,
) -> Result<DividendPlan, EastmoneyError> {
    validate_capital_row(row, request, "SECURITY_CODE", "REPORT_DATE")?;
    let source_at = required_string(row, "REPORT_DATE")?;
    Ok(DividendPlan {
        instrument: request.instrument().clone(),
        report_date: iso_date(&source_at)?,
        ex_dividend_date: optional_string(row.get("EX_DIVIDEND_DATE"))?
            .map(|value| iso_date(&value))
            .transpose()?,
        state: NonEmptyText::new(required_string(row, "ASSIGN_PROGRESS")?)?,
        cash_per_ten: finite(optional_f64(row.get("PRETAX_BONUS_RMB"))?)?,
        bonus_per_ten: finite(optional_f64(row.get("BONUS_RATIO"))?)?,
        transfer_per_ten: finite(optional_f64(row.get("TRANSFER_RATIO"))?)?,
        allotment_per_ten: finite(optional_f64(row.get("ALLOTMENT_RATIO"))?)?,
        reduction_ratio: opt_percent(row, "REDUCTION_RATIO")?,
        evidence: context.evidence_at(Some(&source_at))?,
    })
}

fn opt_money(row: &Value, key: &'static str) -> Result<Option<Money>, EastmoneyError> {
    money(optional_f64(row.get(key))?)
}

fn opt_quantity(row: &Value, key: &'static str) -> Result<Option<Quantity>, EastmoneyError> {
    quantity(optional_f64(row.get(key))?)
}

fn opt_percent(row: &Value, key: &'static str) -> Result<Option<Ratio>, EastmoneyError> {
    percent(optional_f64(row.get(key))?)
}

fn opt_decimal(row: &Value, key: &'static str) -> Result<Option<Ratio>, EastmoneyError> {
    decimal(optional_f64(row.get(key))?)
}

fn validate_capital_row(
    row: &Value,
    request: &InstrumentDateRangeRequest,
    code_key: &'static str,
    date_key: &'static str,
) -> Result<(), EastmoneyError> {
    // Every verified capital endpoint returns its report-specific code field
    // plus SECUCODE (for example, SCODE + SECUCODE for margin data and
    // SECURITY_CODE + SECUCODE for the other families). The request filter is
    // not evidence that the response belongs to that instrument, so neither
    // half of this source identity may be omitted in a strict batch.
    let source_code = required_string(row, code_key)?;
    let secucode = required_string(row, "SECUCODE")?;
    validate_source_instrument(request.instrument(), &source_code, None)?;
    validate_source_secucode(request.instrument(), &secucode)?;
    if let Some(source_date) = optional_string(row.get(date_key))? {
        let source_date = iso_date(&source_date)?;
        if let Some(start) = request.start() {
            if &source_date < start {
                return Err(EastmoneyError::Protocol(format!(
                    "Eastmoney source date {} precedes requested start {}",
                    source_date.as_str(),
                    start.as_str()
                )));
            }
        }
        if let Some(end) = request.end() {
            if &source_date > end {
                return Err(EastmoneyError::Protocol(format!(
                    "Eastmoney source date {} exceeds requested end {}",
                    source_date.as_str(),
                    end.as_str()
                )));
            }
        }
    }
    Ok(())
}

fn required_number(row: &Value, key: &'static str) -> Result<f64, EastmoneyError> {
    optional_f64(row.get(key))?
        .ok_or_else(|| EastmoneyError::Protocol(format!("required numeric field {key} is absent")))
}

fn required_price(row: &Value, key: &'static str) -> Result<Price, EastmoneyError> {
    Ok(Price::new(required_number(row, key)?)?)
}

fn required_quantity(row: &Value, key: &'static str) -> Result<Quantity, EastmoneyError> {
    Ok(Quantity::new(required_number(row, key)?)?)
}

fn required_scaled_quantity(
    row: &Value,
    key: &'static str,
    scale: f64,
) -> Result<Quantity, EastmoneyError> {
    Ok(Quantity::new(required_number(row, key)? * scale)?)
}

#[cfg(test)]
#[path = "../tests/internal/capital_tests.rs"]
mod tests;
