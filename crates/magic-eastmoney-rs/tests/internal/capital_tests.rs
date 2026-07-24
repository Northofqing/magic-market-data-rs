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
