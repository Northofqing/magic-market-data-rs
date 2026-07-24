use super::{map_entries, map_seats};
use crate::test_support::ScriptedTransport;
use crate::{EastmoneyClient, EastmoneyError};
use magic_market_core::{
    AssetClass, DragonTigerData, DragonTigerSide, Exchange, InstrumentId, InstrumentSignalRequest,
    PositiveU32, RatioUnit,
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

fn datacenter_page(rows: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "success": true,
        "code": 0,
        "result": {"data": rows, "pages": 1}
    }))
    .unwrap()
}

#[test]
fn public_dragon_tiger_contract_maps_entries_and_both_seat_sides() {
    let entries = datacenter_page(json!([{
        "SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23","BILLBOARD_BUY_AMT":100,
        "BILLBOARD_SELL_AMT":40,"BILLBOARD_NET_AMT":60
    }]));
    let client = EastmoneyClient::with_transport(ScriptedTransport::from_results([Ok(entries)]));
    assert_eq!(
        client
            .dragon_tiger_entries(&request())
            .unwrap()
            .records()
            .len(),
        1
    );

    let dated = request().with_trading_date(magic_market_core::IsoDate::new("2026-07-23").unwrap());
    let buy = datacenter_page(json!([{
        "SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23","OPERATEDEPT_NAME":"机构甲",
        "BUY":100,"SELL":10,"NET":90
    }]));
    let sell = datacenter_page(json!([{
        "SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23","OPERATEDEPT_NAME":"机构乙",
        "BUY":5,"SELL":80,"NET":-75
    }]));
    let client =
        EastmoneyClient::with_transport(ScriptedTransport::from_results([Ok(buy), Ok(sell)]));
    let seats = client.dragon_tiger_seats(&dated).unwrap();
    assert_eq!(seats.records().len(), 2);
    assert_eq!(seats.records()[0].side, DragonTigerSide::Buy);
    assert_eq!(seats.records()[1].side, DragonTigerSide::Sell);
}

#[test]
fn public_seat_contract_discovers_and_validates_latest_source_date() {
    let latest = datacenter_page(json!([{
        "SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23"
    }]));
    let buy = datacenter_page(json!([{
        "SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23","OPERATEDEPT_NAME":"机构甲",
        "BUY":100,"SELL":10,"NET":90
    }]));
    let sell = datacenter_page(json!([]));
    let client = EastmoneyClient::with_transport(ScriptedTransport::from_results([
        Ok(latest),
        Ok(buy),
        Ok(sell),
    ]));
    let requests = client.transport.clone();
    let seats = client.dragon_tiger_seats(&request()).unwrap();
    assert_eq!(seats.records().len(), 1);
    drop(requests);

    let empty_latest = datacenter_page(json!([]));
    let client =
        EastmoneyClient::with_transport(ScriptedTransport::from_results([Ok(empty_latest)]));
    assert!(matches!(
        client.dragon_tiger_seats(&request()),
        Err(EastmoneyError::Protocol(_))
    ));
}

#[test]
fn dragon_tiger_requires_the_ranked_side_amount() {
    let row = json!({
        "SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23","OPERATEDEPT_NAME":"机构甲",
        "SELL":10,"NET":-10
    });
    assert!(matches!(
        map_seats(&[(DragonTigerSide::Buy, row)], &request(), "2026-07-23"),
        Err(EastmoneyError::Protocol(_))
    ));
}
