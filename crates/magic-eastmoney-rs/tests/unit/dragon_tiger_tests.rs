use super::{map_entries, map_seats};
use magic_market_core::{
    AssetClass, DragonTigerSide, Exchange, InstrumentId, InstrumentSignalRequest, PositiveU32,
    RatioUnit,
};
use serde_json::json;

fn request() -> InstrumentSignalRequest {
    InstrumentSignalRequest::new(
        InstrumentId::new(Exchange::Shenzhen, "002475", AssetClass::Equity).unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap()
}

#[test]
fn maps_entry_amounts_reason_turnover_and_evidence() {
    let batch = map_entries(
        &[json!({"SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
                "TRADE_DATE":"2026-07-23 00:00:00",
                "EXPLANATION":"日涨幅偏离值达到7%",
                "BILLBOARD_BUY_AMT":100,"BILLBOARD_SELL_AMT":40,
                "BILLBOARD_NET_AMT":60,"TURNOVERRATE":12.5})],
        &request(),
    )
    .unwrap();
    let entry = &batch.records()[0];
    assert_eq!(entry.entry_id.as_str(), "002475:2026-07-23");
    assert_eq!(entry.trading_date.as_str(), "2026-07-23");
    assert_eq!(
        entry.reason.as_ref().unwrap().as_str(),
        "日涨幅偏离值达到7%"
    );
    assert_eq!(entry.buy_amount.unwrap().get(), 100.0);
    assert_eq!(entry.sell_amount.unwrap().get(), 40.0);
    assert_eq!(entry.net_amount.unwrap().get(), 60.0);
    assert_eq!(entry.turnover_rate.unwrap().get(), 12.5);
    assert_eq!(entry.turnover_rate.unwrap().unit(), RatioUnit::Percent);
    assert_eq!(entry.evidence.source_at(), Some("2026-07-23 00:00:00"));
}

#[test]
fn maps_buy_and_sell_seats_with_independent_ranks() {
    let rows = vec![
        (
            DragonTigerSide::Buy,
            json!({"SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
                    "TRADE_DATE":"2026-07-23 00:00:00",
                    "OPERATEDEPT_NAME":"机构甲","BUY":100,"SELL":10,"NET":90}),
        ),
        (
            DragonTigerSide::Sell,
            json!({"SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
                    "TRADE_DATE":"2026-07-23 00:00:00",
                    "OPERATEDEPT_NAME":"机构乙","BUY":5,"SELL":80,"NET":-75}),
        ),
    ];
    let batch = map_seats(&rows, &request(), "2026-07-23").unwrap();
    assert_eq!(batch.records()[0].side, DragonTigerSide::Buy);
    assert_eq!(batch.records()[0].rank.get(), 1);
    assert_eq!(batch.records()[0].seat_name.as_str(), "机构甲");
    assert_eq!(batch.records()[0].amount.get(), 100.0);
    assert_eq!(batch.records()[0].buy_amount.unwrap().get(), 100.0);
    assert_eq!(batch.records()[0].sell_amount.unwrap().get(), 10.0);
    assert_eq!(batch.records()[0].net_amount.unwrap().get(), 90.0);
    assert_eq!(batch.records()[1].side, DragonTigerSide::Sell);
    assert_eq!(batch.records()[1].rank.get(), 1);
    assert_eq!(batch.records()[1].amount.get(), 80.0);
}

#[test]
fn source_code_and_requested_trading_date_must_match() {
    let request =
        request().with_trading_date(magic_market_core::IsoDate::new("2026-07-23").unwrap());
    let wrong_code = map_entries(
        &[json!({
            "SECURITY_CODE":"600396",
            "SECUCODE":"600396.SH",
            "TRADE_DATE":"2026-07-23",
            "BILLBOARD_BUY_AMT":1
        })],
        &request,
    );
    assert!(matches!(
        wrong_code,
        Err(crate::EastmoneyError::Protocol(_))
    ));
    let wrong_date = map_entries(
        &[json!({
            "SECURITY_CODE":"002475",
            "SECUCODE":"002475.SZ",
            "TRADE_DATE":"2026-07-22",
            "BILLBOARD_BUY_AMT":1
        })],
        &request,
    );
    assert!(matches!(
        wrong_date,
        Err(crate::EastmoneyError::Protocol(_))
    ));
    let wrong_seat = map_seats(
        &[(
            DragonTigerSide::Buy,
            json!({
                "SECURITY_CODE":"600396",
                "SECUCODE":"600396.SH",
                "TRADE_DATE":"2026-07-23",
                "OPERATEDEPT_NAME":"x",
                "BUY":1
            }),
        )],
        &request,
        "2026-07-23",
    );
    assert!(matches!(
        wrong_seat,
        Err(crate::EastmoneyError::Protocol(_))
    ));
}

#[test]
fn every_entry_and_seat_requires_the_real_identity_pair() {
    for row in [
        json!({
            "SECURITY_CODE":"002475",
            "TRADE_DATE":"2026-07-23",
            "BILLBOARD_BUY_AMT":1
        }),
        json!({
            "SECUCODE":"002475.SZ",
            "TRADE_DATE":"2026-07-23",
            "BILLBOARD_BUY_AMT":1
        }),
    ] {
        assert!(matches!(
            map_entries(&[row], &request()),
            Err(crate::EastmoneyError::Protocol(_))
        ));
    }
    for row in [
        json!({
            "SECURITY_CODE":"002475",
            "TRADE_DATE":"2026-07-23",
            "OPERATEDEPT_NAME":"x",
            "BUY":1
        }),
        json!({
            "SECUCODE":"002475.SZ",
            "TRADE_DATE":"2026-07-23",
            "OPERATEDEPT_NAME":"x",
            "BUY":1
        }),
    ] {
        assert!(matches!(
            map_seats(&[(DragonTigerSide::Buy, row)], &request(), "2026-07-23"),
            Err(crate::EastmoneyError::Protocol(_))
        ));
    }
}

#[test]
fn every_seat_requires_a_matching_source_trade_date() {
    for row in [
        json!({
            "SECURITY_CODE":"002475",
            "SECUCODE":"002475.SZ",
            "OPERATEDEPT_NAME":"x",
            "BUY":1
        }),
        json!({
            "SECURITY_CODE":"002475",
            "SECUCODE":"002475.SZ",
            "TRADE_DATE":"2026-07-22",
            "OPERATEDEPT_NAME":"x",
            "BUY":1
        }),
    ] {
        assert!(matches!(
            map_seats(&[(DragonTigerSide::Buy, row)], &request(), "2026-07-23"),
            Err(crate::EastmoneyError::Protocol(_))
        ));
    }
}

#[test]
fn entry_gross_amounts_must_be_non_negative_and_net_must_reconcile() {
    for row in [
        json!({
            "SECURITY_CODE":"002475",
            "SECUCODE":"002475.SZ",
            "TRADE_DATE":"2026-07-23",
            "BILLBOARD_BUY_AMT":-1,
            "BILLBOARD_SELL_AMT":40,
            "BILLBOARD_NET_AMT":-41
        }),
        json!({
            "SECURITY_CODE":"002475",
            "SECUCODE":"002475.SZ",
            "TRADE_DATE":"2026-07-23",
            "BILLBOARD_BUY_AMT":100,
            "BILLBOARD_SELL_AMT":40,
            "BILLBOARD_NET_AMT":59
        }),
    ] {
        assert!(matches!(
            map_entries(&[row], &request()),
            Err(crate::EastmoneyError::Protocol(_))
        ));
    }
}

#[test]
fn seat_gross_amounts_must_be_non_negative_and_net_must_reconcile() {
    for row in [
        json!({
            "SECURITY_CODE":"002475",
            "SECUCODE":"002475.SZ",
            "TRADE_DATE":"2026-07-23",
            "OPERATEDEPT_NAME":"机构甲",
            "BUY":-1,
            "SELL":10,
            "NET":-11
        }),
        json!({
            "SECURITY_CODE":"002475",
            "SECUCODE":"002475.SZ",
            "TRADE_DATE":"2026-07-23",
            "OPERATEDEPT_NAME":"机构甲",
            "BUY":100,
            "SELL":10,
            "NET":89
        }),
    ] {
        assert!(matches!(
            map_seats(&[(DragonTigerSide::Buy, row)], &request(), "2026-07-23"),
            Err(crate::EastmoneyError::Protocol(_))
        ));
    }
}

#[test]
fn duplicate_entry_and_seat_business_identities_are_rejected() {
    let entry = json!({
        "SECURITY_CODE":"002475",
        "SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23",
        "BILLBOARD_BUY_AMT":100,
        "BILLBOARD_SELL_AMT":40,
        "BILLBOARD_NET_AMT":60
    });
    assert!(matches!(
        map_entries(&[entry.clone(), entry], &request()),
        Err(crate::EastmoneyError::Protocol(message))
            if message.contains("duplicate")
    ));

    let seat = json!({
        "SECURITY_CODE":"002475",
        "SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23",
        "OPERATEDEPT_NAME":"机构甲",
        "BUY":100,
        "SELL":10,
        "NET":90
    });
    assert!(matches!(
        map_seats(
            &[
                (DragonTigerSide::Buy, seat.clone()),
                (DragonTigerSide::Buy, seat)
            ],
            &request(),
            "2026-07-23"
        ),
        Err(crate::EastmoneyError::Protocol(message))
            if message.contains("duplicate")
    ));
}
