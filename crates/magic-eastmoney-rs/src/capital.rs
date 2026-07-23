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
mod tests {
    use super::{map_block_trade, map_dividend, map_holder_count, map_lockup, map_margin};
    use crate::BatchContext;
    use magic_market_core::{
        AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, PositiveU32, RatioUnit,
    };
    use serde_json::json;

    fn request() -> InstrumentDateRangeRequest {
        InstrumentDateRangeRequest::new(
            InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
            PositiveU32::new(10).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn maps_all_margin_and_block_trade_fields() {
        let context = BatchContext::new("fixture", None).unwrap();
        let margin = map_margin(
            &json!({"SCODE":"600519","SECUCODE":"600519.SH",
                "DATE":"2026-07-23 00:00:00","RZYE":1,"RZMRE":2,"RZCHE":3,
                "RQYE":4,"RQMCL":5,"RQCHL":6,"RZRQYE":7}),
            &request(),
            &context,
        )
        .unwrap();
        assert_eq!(margin.trading_date.as_str(), "2026-07-23");
        assert_eq!(margin.financing_balance.unwrap().get(), 1.0);
        assert_eq!(margin.financing_buy.unwrap().get(), 2.0);
        assert_eq!(margin.financing_repayment.unwrap().get(), 3.0);
        assert_eq!(margin.securities_lending_balance.unwrap().get(), 4.0);
        assert_eq!(margin.securities_lending_sell.unwrap().get(), 5.0);
        assert_eq!(margin.securities_lending_repayment.unwrap().get(), 6.0);
        assert_eq!(margin.total_balance.unwrap().get(), 7.0);

        let trade = map_block_trade(
            &json!({"SECURITY_CODE":"600519","SECUCODE":"600519.SH",
                "TRADE_DATE":"2026-07-23","TRADE_TIME":"14:55:01",
                "DEAL_PRICE":1500,"CLOSE_PRICE":1498,"PREMIUM_RATIO":0.13,
                "DEAL_VOLUME":1000,"DEAL_AMT":1500000,
                "BUYER_NAME":"机构甲","SELLER_NAME":"机构乙"}),
            &request(),
            &context,
        )
        .unwrap();
        assert_eq!(trade.traded_at.unwrap().as_str(), "14:55:01");
        assert_eq!(trade.price.get(), 1500.0);
        assert_eq!(trade.close_price.unwrap().get(), 1498.0);
        assert_eq!(trade.premium_ratio.unwrap().get(), 0.13);
        assert_eq!(trade.premium_ratio.unwrap().unit(), RatioUnit::Decimal);
        assert_eq!(trade.volume.get(), 1000.0);
        assert_eq!(trade.amount.unwrap().get(), 1500000.0);
        assert_eq!(trade.buyer.unwrap().as_str(), "机构甲");
        assert_eq!(trade.seller.unwrap().as_str(), "机构乙");
    }

    #[test]
    fn maps_holder_lockup_scaling_and_dividend_units() {
        let context = BatchContext::new("fixture", None).unwrap();
        let holder = map_holder_count(
            &json!({"SECURITY_CODE":"600519","SECUCODE":"600519.SH",
                "END_DATE":"2026-06-30","HOLDER_NUM":12345,
                "HOLDER_NUM_CHANGE":-10,"HOLDER_NUM_RATIO":-0.08,
                "AVG_FREE_SHARES":4567}),
            &request(),
            &context,
        )
        .unwrap();
        assert_eq!(holder.holders.get(), 12345.0);
        assert_eq!(holder.holder_change.unwrap().get(), -10.0);
        assert_eq!(holder.change_ratio.unwrap().get(), -0.08);
        assert_eq!(holder.average_shares_per_holder.unwrap().get(), 4567.0);

        let lockup = map_lockup(
            &json!({"SECURITY_CODE":"600519","SECUCODE":"600519.SH",
                "FREE_DATE":"2026-08-01","FREE_SHARES_TYPE":"首发原股东限售股份",
                "FREE_SHARES":12.5,"ABLE_FREE_SHARES":10.0,
                "FREE_RATIO":1.2,"LIFT_MARKET_CAP":300000}),
            &request(),
            &context,
        )
        .unwrap();
        assert_eq!(lockup.shares.get(), 125000.0);
        assert_eq!(lockup.able_shares.unwrap().get(), 100000.0);
        assert_eq!(lockup.free_float_ratio.unwrap().get(), 1.2);
        assert_eq!(lockup.free_float_ratio.unwrap().unit(), RatioUnit::Decimal);
        assert_eq!(lockup.market_value.unwrap().get(), 3_000_000_000.0);

        let dividend = map_dividend(
            &json!({"SECURITY_CODE":"600519","SECUCODE":"600519.SH",
                "REPORT_DATE":"2025-12-31","EX_DIVIDEND_DATE":"2026-06-20",
                "ASSIGN_PROGRESS":"实施","PRETAX_BONUS_RMB":28.0,
                "BONUS_RATIO":1.0,"TRANSFER_RATIO":2.0,
                "ALLOTMENT_RATIO":0.5,"REDUCTION_RATIO":3.0}),
            &request(),
            &context,
        )
        .unwrap();
        assert_eq!(dividend.ex_dividend_date.unwrap().as_str(), "2026-06-20");
        assert_eq!(dividend.state.as_str(), "实施");
        assert_eq!(dividend.cash_per_ten.unwrap().get(), 28.0);
        assert_eq!(dividend.bonus_per_ten.unwrap().get(), 1.0);
        assert_eq!(dividend.transfer_per_ten.unwrap().get(), 2.0);
        assert_eq!(dividend.allotment_per_ten.unwrap().get(), 0.5);
        assert_eq!(dividend.reduction_ratio.unwrap().get(), 3.0);
    }

    #[test]
    fn source_identity_and_requested_date_range_are_not_trusted_from_the_filter() {
        let context = BatchContext::new("fixture", None).unwrap();
        let mismatched = map_margin(
            &json!({
                "SCODE":"002475",
                "SECUCODE":"002475.SZ",
                "DATE":"2026-07-23",
                "RZYE":1
            }),
            &request(),
            &context,
        );
        assert!(matches!(
            mismatched,
            Err(crate::EastmoneyError::Protocol(_))
        ));
        for mismatched in [
            map_block_trade(
                &json!({
                    "SECURITY_CODE":"002475",
                    "SECUCODE":"002475.SZ",
                    "TRADE_DATE":"2026-07-23",
                    "DEAL_PRICE":1,
                    "DEAL_VOLUME":1
                }),
                &request(),
                &context,
            )
            .map(|_| ()),
            map_holder_count(
                &json!({
                    "SECURITY_CODE":"002475",
                    "SECUCODE":"002475.SZ",
                    "END_DATE":"2026-07-23",
                    "HOLDER_NUM":1
                }),
                &request(),
                &context,
            )
            .map(|_| ()),
            map_lockup(
                &json!({
                    "SECURITY_CODE":"002475",
                    "SECUCODE":"002475.SZ",
                    "FREE_DATE":"2026-07-23",
                    "FREE_SHARES_TYPE":"x",
                    "FREE_SHARES":1
                }),
                &request(),
                &context,
            )
            .map(|_| ()),
            map_dividend(
                &json!({
                    "SECURITY_CODE":"002475",
                    "SECUCODE":"002475.SZ",
                    "REPORT_DATE":"2026-07-23",
                    "ASSIGN_PROGRESS":"x"
                }),
                &request(),
                &context,
            )
            .map(|_| ()),
        ] {
            assert!(matches!(
                mismatched,
                Err(crate::EastmoneyError::Protocol(_))
            ));
        }
        assert!(matches!(
            map_margin(
                &json!({
                    "SCODE":"600519",
                    "SECUCODE":"600519.SZ",
                    "DATE":"2026-07-23",
                    "RZYE":1
                }),
                &request(),
                &context,
            ),
            Err(crate::EastmoneyError::Protocol(_))
        ));

        let ranged = InstrumentDateRangeRequest::new(
            InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
            PositiveU32::new(10).unwrap(),
        )
        .unwrap()
        .with_range(
            magic_market_core::IsoDate::new("2026-01-01").unwrap(),
            magic_market_core::IsoDate::new("2026-06-30").unwrap(),
        )
        .unwrap();
        let outside = map_margin(
            &json!({
                "SCODE":"600519",
                "SECUCODE":"600519.SH",
                "DATE":"2026-07-23",
                "RZYE":1
            }),
            &ranged,
            &context,
        );
        assert!(matches!(outside, Err(crate::EastmoneyError::Protocol(_))));
    }

    #[test]
    fn every_capital_row_requires_its_real_code_and_secucode_identity() {
        let context = BatchContext::new("fixture", None).unwrap();
        for result in [
            map_margin(
                &json!({"SCODE":"600519","DATE":"2026-07-23","RZYE":1}),
                &request(),
                &context,
            )
            .map(|_| ()),
            map_block_trade(
                &json!({
                    "SECUCODE":"600519.SH",
                    "TRADE_DATE":"2026-07-23",
                    "DEAL_PRICE":1,
                    "DEAL_VOLUME":1
                }),
                &request(),
                &context,
            )
            .map(|_| ()),
            map_holder_count(
                &json!({
                    "SECURITY_CODE":"600519",
                    "END_DATE":"2026-07-23",
                    "HOLDER_NUM":1
                }),
                &request(),
                &context,
            )
            .map(|_| ()),
            map_lockup(
                &json!({
                    "SECUCODE":"600519.SH",
                    "FREE_DATE":"2026-07-23",
                    "FREE_SHARES_TYPE":"x",
                    "FREE_SHARES":1
                }),
                &request(),
                &context,
            )
            .map(|_| ()),
            map_dividend(
                &json!({
                    "SECURITY_CODE":"600519",
                    "REPORT_DATE":"2026-07-23",
                    "ASSIGN_PROGRESS":"x"
                }),
                &request(),
                &context,
            )
            .map(|_| ()),
        ] {
            assert!(matches!(result, Err(crate::EastmoneyError::Protocol(_))));
        }
    }
}
