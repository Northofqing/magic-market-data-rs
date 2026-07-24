use super::{
    map_entries, map_seats, validate_seat_limit, SEAT_SIDE_CARDINALITY, SEAT_SIDE_FETCH_LIMIT,
};
use crate::test_support::ScriptedTransport;
use crate::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::{
    AssetClass, DragonTigerData, DragonTigerSide, Exchange, InstrumentId, InstrumentSignalRequest,
    IsoDate, PositiveU32, RatioUnit,
};
use serde_json::json;
use std::sync::{Arc, Mutex};

struct SeatTransport {
    buy_rows: usize,
    sell_rows: usize,
    requests: Arc<Mutex<Vec<String>>>,
}

impl EastmoneyTransport for SeatTransport {
    fn get(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.requests.lock().unwrap().push(url.to_owned());
        let (side, count) = if url.contains("reportName=RPT_BILLBOARD_DAILYDETAILSBUY") {
            (DragonTigerSide::Buy, self.buy_rows)
        } else if url.contains("reportName=RPT_BILLBOARD_DAILYDETAILSSELL") {
            (DragonTigerSide::Sell, self.sell_rows)
        } else {
            return Err(EastmoneyError::InvalidRequest(
                "seat fixture received an unexpected report".into(),
            ));
        };
        let rows = (1..=count)
            .map(|rank| scripted_seat_row(side, rank))
            .collect::<Vec<_>>();
        Ok(datacenter_page(json!(rows)))
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::InvalidRequest(
            "seat fixture does not accept POST".into(),
        ))
    }
}

fn scripted_seat_row(side: DragonTigerSide, rank: usize) -> serde_json::Value {
    let (buy, sell, name) = match side {
        DragonTigerSide::Buy => (100 + rank, 10, format!("买方机构{rank}")),
        DragonTigerSide::Sell => (5, 80 + rank, format!("卖方机构{rank}")),
    };
    json!({
        "SECURITY_CODE":"002475",
        "SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23 00:00:00",
        "OPERATEDEPT_NAME":name,
        "BUY":buy,
        "SELL":sell,
        "NET":buy as isize - sell as isize
    })
}

fn request() -> InstrumentSignalRequest {
    InstrumentSignalRequest::new(
        InstrumentId::new(Exchange::Shenzhen, "002475", AssetClass::Equity).unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap()
}

fn dated_request() -> InstrumentSignalRequest {
    request().with_trading_date(IsoDate::new("2026-07-23").unwrap())
}

fn complete_seat_rows() -> Vec<(DragonTigerSide, serde_json::Value)> {
    let mut rows = Vec::new();
    for rank in 1..=5 {
        rows.push((
            DragonTigerSide::Buy,
            scripted_seat_row(DragonTigerSide::Buy, rank),
        ));
    }
    for rank in 1..=5 {
        rows.push((
            DragonTigerSide::Sell,
            scripted_seat_row(DragonTigerSide::Sell, rank),
        ));
    }
    rows
}

fn replace_first_seat(row: serde_json::Value) -> Vec<(DragonTigerSide, serde_json::Value)> {
    let mut rows = complete_seat_rows();
    rows[0] = (DragonTigerSide::Buy, row);
    rows
}

fn side_values(side: DragonTigerSide) -> Vec<serde_json::Value> {
    (1..=5).map(|rank| scripted_seat_row(side, rank)).collect()
}

#[test]
fn seat_request_limit_reserves_one_atomic_top_five_group() {
    let instrument = InstrumentId::new(Exchange::Shenzhen, "002475", AssetClass::Equity).unwrap();
    let too_small =
        InstrumentSignalRequest::new(instrument.clone(), PositiveU32::new(9).unwrap()).unwrap();
    let exact =
        InstrumentSignalRequest::new(instrument.clone(), PositiveU32::new(10).unwrap()).unwrap();
    let larger = InstrumentSignalRequest::new(instrument, PositiveU32::new(100).unwrap()).unwrap();
    assert!(validate_seat_limit(&too_small).is_err());
    assert!(validate_seat_limit(&exact).is_ok());
    assert!(validate_seat_limit(&larger).is_ok());
    assert_eq!(SEAT_SIDE_CARDINALITY, 5);
    assert_eq!(SEAT_SIDE_FETCH_LIMIT, 6);
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
    assert_eq!(entry.entry_id().as_str(), "002475:2026-07-23");
    assert_eq!(entry.trading_date().as_str(), "2026-07-23");
    assert_eq!(entry.reason().unwrap().as_str(), "日涨幅偏离值达到7%");
    assert_eq!(entry.buy_amount().unwrap().get(), 100.0);
    assert_eq!(entry.sell_amount().unwrap().get(), 40.0);
    assert_eq!(entry.net_amount().unwrap().get(), 60.0);
    assert_eq!(entry.turnover_rate().unwrap().get(), 12.5);
    assert_eq!(entry.turnover_rate().unwrap().unit(), RatioUnit::Percent);
    assert_eq!(entry.evidence().source_at(), Some("2026-07-23 00:00:00"));
}

#[test]
fn maps_buy_and_sell_seats_with_independent_ranks() {
    let batch = map_seats(&complete_seat_rows(), &request(), "2026-07-23").unwrap();
    assert_eq!(batch.records().len(), 10);
    assert_eq!(batch.records()[0].side(), DragonTigerSide::Buy);
    assert_eq!(batch.records()[0].rank().get(), 1);
    assert_eq!(batch.records()[0].seat_name().as_str(), "买方机构1");
    assert_eq!(batch.records()[0].amount().get(), 101.0);
    assert_eq!(batch.records()[0].buy_amount().unwrap().get(), 101.0);
    assert_eq!(batch.records()[0].sell_amount().unwrap().get(), 10.0);
    assert_eq!(batch.records()[0].net_amount().unwrap().get(), 91.0);
    assert_eq!(batch.records()[5].side(), DragonTigerSide::Sell);
    assert_eq!(batch.records()[5].rank().get(), 1);
    assert_eq!(batch.records()[5].amount().get(), 81.0);
}

#[test]
fn rejects_incomplete_or_oversized_seat_groups() {
    let mut incomplete = complete_seat_rows();
    incomplete.pop();
    assert!(map_seats(&incomplete, &request(), "2026-07-23").is_err());

    let mut oversized = complete_seat_rows();
    oversized.push(oversized[0].clone());
    assert!(map_seats(&oversized, &request(), "2026-07-23").is_err());
}

#[test]
fn trait_path_uses_the_sixth_row_sentinel_before_local_truncation() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let client = EastmoneyClient::with_transport(SeatTransport {
        buy_rows: 5,
        sell_rows: 5,
        requests: Arc::clone(&requests),
    });
    let batch = client.dragon_tiger_seats(&dated_request()).unwrap();
    assert_eq!(batch.records().len(), 10);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .any(|url| url.contains("reportName=RPT_BILLBOARD_DAILYDETAILSBUY")));
    assert!(requests
        .iter()
        .any(|url| url.contains("reportName=RPT_BILLBOARD_DAILYDETAILSSELL")));
    drop(requests);

    for (buy_rows, sell_rows, expected) in [
        (6, 5, "got 6 buy and 5 sell"),
        (5, 6, "got 5 buy and 6 sell"),
    ] {
        let client = EastmoneyClient::with_transport(SeatTransport {
            buy_rows,
            sell_rows,
            requests: Arc::new(Mutex::new(Vec::new())),
        });
        let error = client.dragon_tiger_seats(&dated_request()).unwrap_err();
        assert!(matches!(
            error,
            EastmoneyError::Protocol(message) if message.contains(expected)
        ));
    }
}

#[test]
fn source_code_and_requested_trading_date_must_match() {
    let request = dated_request();
    let wrong_code = map_entries(
        &[json!({
            "SECURITY_CODE":"600396",
            "SECUCODE":"600396.SH",
            "TRADE_DATE":"2026-07-23",
            "BILLBOARD_BUY_AMT":1
        })],
        &request,
    );
    assert!(matches!(wrong_code, Err(EastmoneyError::Protocol(_))));
    let wrong_date = map_entries(
        &[json!({
            "SECURITY_CODE":"002475",
            "SECUCODE":"002475.SZ",
            "TRADE_DATE":"2026-07-22",
            "BILLBOARD_BUY_AMT":1
        })],
        &request,
    );
    assert!(matches!(wrong_date, Err(EastmoneyError::Protocol(_))));

    let rows = replace_first_seat(json!({
        "SECURITY_CODE":"600396",
        "SECUCODE":"600396.SH",
        "TRADE_DATE":"2026-07-23",
        "OPERATEDEPT_NAME":"x",
        "BUY":1
    }));
    assert!(matches!(
        map_seats(&rows, &request, "2026-07-23"),
        Err(EastmoneyError::Protocol(_))
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
            Err(EastmoneyError::Protocol(_))
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
        let rows = replace_first_seat(row);
        assert!(matches!(
            map_seats(&rows, &request(), "2026-07-23"),
            Err(EastmoneyError::Protocol(_))
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
        let rows = replace_first_seat(row);
        assert!(matches!(
            map_seats(&rows, &request(), "2026-07-23"),
            Err(EastmoneyError::Protocol(_))
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
            Err(EastmoneyError::Core(_))
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
        let rows = replace_first_seat(row);
        assert!(matches!(
            map_seats(&rows, &request(), "2026-07-23"),
            Err(EastmoneyError::Core(_))
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
        Err(EastmoneyError::Protocol(message)) if message.contains("duplicate")
    ));

    let mut rows = complete_seat_rows();
    rows[1].1["OPERATEDEPT_NAME"] = rows[0].1["OPERATEDEPT_NAME"].clone();
    assert!(matches!(
        map_seats(&rows, &request(), "2026-07-23"),
        Err(EastmoneyError::Protocol(message)) if message.contains("duplicate")
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

    let buy = datacenter_page(json!(side_values(DragonTigerSide::Buy)));
    let sell = datacenter_page(json!(side_values(DragonTigerSide::Sell)));
    let client =
        EastmoneyClient::with_transport(ScriptedTransport::from_results([Ok(buy), Ok(sell)]));
    let seats = client.dragon_tiger_seats(&dated_request()).unwrap();
    assert_eq!(seats.records().len(), 10);
    assert_eq!(seats.records()[0].side(), DragonTigerSide::Buy);
    assert_eq!(seats.records()[5].side(), DragonTigerSide::Sell);
}

#[test]
fn public_seat_contract_discovers_and_validates_latest_source_date() {
    let latest = datacenter_page(json!([{
        "SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23"
    }]));
    let buy = datacenter_page(json!(side_values(DragonTigerSide::Buy)));
    let sell = datacenter_page(json!(side_values(DragonTigerSide::Sell)));
    let client = EastmoneyClient::with_transport(ScriptedTransport::from_results([
        Ok(latest),
        Ok(buy),
        Ok(sell),
    ]));
    let seats = client.dragon_tiger_seats(&request()).unwrap();
    assert_eq!(seats.records().len(), 10);

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
    let rows = replace_first_seat(json!({
        "SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
        "TRADE_DATE":"2026-07-23","OPERATEDEPT_NAME":"机构甲",
        "SELL":10,"NET":-10
    }));
    assert!(matches!(
        map_seats(&rows, &request(), "2026-07-23"),
        Err(EastmoneyError::Protocol(_))
    ));
}
