use super::positive_trade_id;
use crate::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::{
    AssetClass, DragonTigerDiscovery, DragonTigerDiscoveryRequest, Exchange, IsoDate, PositiveU32,
};
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Clone)]
struct DiscoveryTransport {
    rows: Vec<Value>,
}

impl DiscoveryTransport {
    fn fixture() -> Self {
        Self {
            rows: vec![
                row(101, "600000", "浦发银行", "600000.SH", 100.0, 40.0, 60.0),
                row(102, "123275", "沿浦转债", "123275.SZ", 80.0, 30.0, 50.0),
                row(103, "920001", "北交样本", "920001.BJ", 70.0, 20.0, 50.0),
                row(104, "600000", "浦发银行", "600000.SH", 20.0, 10.0, 10.0),
            ],
        }
    }
}

impl EastmoneyTransport for DiscoveryTransport {
    fn get(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        serde_json::to_vec(&json!({
            "success": true,
            "code": 0,
            "result": {
                "data": self.rows,
                "pages": 1,
                "count": self.rows.len()
            }
        }))
        .map_err(|error| EastmoneyError::Decode(error.to_string()))
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::InvalidRequest(
            "discovery fixture does not accept POST".into(),
        ))
    }
}

fn row(
    trade_id: u64,
    code: &str,
    name: &str,
    secucode: &str,
    buy: f64,
    sell: f64,
    net: f64,
) -> Value {
    json!({
        "TRADE_ID": trade_id,
        "SECURITY_CODE": code,
        "SECURITY_NAME_ABBR": name,
        "SECUCODE": secucode,
        "SECURITY_TYPE_CODE": if code.starts_with('1') { "060" } else { "058001001" },
        "TRADE_DATE": "2026-07-24 00:00:00",
        "EXPLANATION": "fixture reason",
        "BILLBOARD_BUY_AMT": buy,
        "BILLBOARD_SELL_AMT": sell,
        "BILLBOARD_NET_AMT": net,
        "TURNOVERRATE": 1.5
    })
}

fn request(limit: u32) -> DragonTigerDiscoveryRequest {
    DragonTigerDiscoveryRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(limit).unwrap(),
    )
    .unwrap()
}

#[test]
fn discovers_all_exchanges_and_keeps_multi_reason_ids_unique() {
    let client = EastmoneyClient::with_transport(DiscoveryTransport::fixture());
    let batch = client.discover_dragon_tiger(&request(10_000)).unwrap();
    assert_eq!(batch.records().len(), 4);
    assert_eq!(
        batch
            .records()
            .iter()
            .map(|row| row.entry_id().as_str())
            .collect::<HashSet<_>>()
            .len(),
        4
    );
    assert!(batch
        .records()
        .iter()
        .any(|row| row.instrument().exchange() == Exchange::Beijing));
    assert!(batch
        .records()
        .iter()
        .any(|row| row.instrument().asset_class() == AssetClass::Bond));
    assert_eq!(
        batch.records()[0].entry_id().as_str(),
        "eastmoney:2026-07-24:101"
    );
    assert_eq!(
        batch.records()[0].instrument_name().unwrap().as_str(),
        "浦发银行"
    );
}

#[test]
fn reads_complete_day_before_exchange_filter_and_limit() {
    let client = EastmoneyClient::with_transport(DiscoveryTransport::fixture());
    let filtered = request(1).with_exchange(Exchange::Shanghai);
    let batch = client.discover_dragon_tiger(&filtered).unwrap();
    assert_eq!(batch.records().len(), 1);
    assert_eq!(
        batch.records()[0].instrument().exchange(),
        Exchange::Shanghai
    );
}

#[test]
fn rejects_duplicate_or_non_integral_trade_ids() {
    let mut duplicate = DiscoveryTransport::fixture();
    duplicate.rows[1]["TRADE_ID"] = json!(101);
    assert!(EastmoneyClient::with_transport(duplicate)
        .discover_dragon_tiger(&request(10))
        .is_err());
    for invalid in [json!(0), json!(1.5), json!("1.0"), json!("not-an-id")] {
        assert!(positive_trade_id(Some(&invalid)).is_err());
    }
}

#[test]
fn rejects_identity_date_and_financial_invariant_failures() {
    let mut disagreement = DiscoveryTransport::fixture();
    disagreement.rows[0]["SECUCODE"] = json!("600001.SH");
    assert!(EastmoneyClient::with_transport(disagreement)
        .discover_dragon_tiger(&request(10))
        .is_err());

    let mut wrong_date = DiscoveryTransport::fixture();
    wrong_date.rows[0]["TRADE_DATE"] = json!("2026-07-23 00:00:00");
    assert!(EastmoneyClient::with_transport(wrong_date)
        .discover_dragon_tiger(&request(10))
        .is_err());

    let mut wrong_net = DiscoveryTransport::fixture();
    wrong_net.rows[0]["BILLBOARD_NET_AMT"] = json!(59.0);
    assert!(EastmoneyClient::with_transport(wrong_net)
        .discover_dragon_tiger(&request(10))
        .is_err());

    let mut missing_name = DiscoveryTransport::fixture();
    missing_name.rows[0]["SECURITY_NAME_ABBR"] = Value::Null;
    assert!(EastmoneyClient::with_transport(missing_name)
        .discover_dragon_tiger(&request(10))
        .is_err());
}
