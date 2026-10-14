use super::register_eastmoney_client;
use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::{DragonTigerEntry, ProviderId, RatioUnit};
use magic_market_service::{CanonicalPayload, Operation, OperationRegistry, QueryCommand};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// Synthetic external report page using the native BILLBOARD_* fields already
// exercised by the provider's dragon_tiger_tests.rs. Not live admission evidence.
const ENTRIES_REPORT: &str = r#"{
    "success":true,"code":0,"result":{"pages":1,"data":[{
        "SECURITY_CODE":"002475","SECUCODE":"002475.SZ","TRADE_DATE":"2026-07-23",
        "TRADE_ID":1001,"EXPLANATION":"日涨幅偏离值达到7%",
        "BILLBOARD_BUY_AMT":100,"BILLBOARD_SELL_AMT":40,"BILLBOARD_NET_AMT":60,
        "TURNOVERRATE":12.5
    }]}
}"#;

// Complete independent wire oracle. Only separately bounded observation time
// and its validated batch identifier are filled after public Execute below.
const EXPECTED_ENTRY: &str = r#"{
    "entry_id":"002475:2026-07-23:1001",
    "instrument":{"exchange":"Shenzhen","code":"002475","asset_class":"Equity"},
    "instrument_name":null,"trading_date":"2026-07-23",
    "reason":"日涨幅偏离值达到7%",
    "buy_amount":100.0,"sell_amount":40.0,"net_amount":60.0,
    "turnover_rate":{"value":12.5,"unit":"Percent"},
    "evidence":{"provider":"Eastmoney","source_at":"2026-07-23",
                "observed_at":"","batch_id":""}
}"#;

struct EntriesReportFixture(Mutex<Option<&'static [u8]>>);

// Deliberately not a general URL decoder: these are the only escapes permitted
// by this one fixed fixture's expected query. Unknown escapes and '+' fail.
fn fixture_query_value(raw: &str) -> Result<String, EastmoneyError> {
    let mut value = raw.to_owned();
    for (upper, lower, decoded) in [
        ("%28", "%28", "("),
        ("%29", "%29", ")"),
        ("%3D", "%3d", "="),
        ("%22", "%22", "\""),
        ("%27", "%27", "'"),
    ] {
        value = value.replace(upper, decoded).replace(lower, decoded);
    }
    if value.contains(['%', '+']) {
        return Err(EastmoneyError::Protocol(
            "entries fixture query contains an unsupported escape".into(),
        ));
    }
    Ok(value)
}

impl EastmoneyTransport for EntriesReportFixture {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        let (endpoint, query) = url.split_once('?').ok_or_else(|| {
            EastmoneyError::Protocol("entries fixture requires a bounded GET query".into())
        })?;
        let mut actual_query = BTreeMap::new();
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').ok_or_else(|| {
                EastmoneyError::Protocol("entries fixture requires query pairs".into())
            })?;
            if actual_query
                .insert(key, fixture_query_value(value)?)
                .is_some()
            {
                return Err(EastmoneyError::Protocol(
                    "entries fixture rejects duplicate query keys".into(),
                ));
            }
        }
        let expected_query = BTreeMap::from([
            ("reportName", "RPT_DAILYBILLBOARD_DETAILSNEW".to_owned()),
            ("columns", "ALL".to_owned()),
            (
                "filter",
                "(SECURITY_CODE=\"002475\")(TRADE_DATE='2026-07-23')".to_owned(),
            ),
            ("pageNumber", "1".to_owned()),
            ("pageSize", "500".to_owned()),
            ("sortColumns", "TRADE_DATE".to_owned()),
            ("sortTypes", "-1".to_owned()),
            ("source", "WEB".to_owned()),
            ("client", "WEB".to_owned()),
        ]);
        let mut actual_headers = BTreeMap::new();
        for (key, value) in headers {
            if actual_headers
                .insert(key.to_ascii_lowercase(), *value)
                .is_some()
            {
                return Err(EastmoneyError::Protocol(
                    "entries fixture rejects duplicate headers".into(),
                ));
            }
        }
        let expected_headers = BTreeMap::from([
            ("accept".to_owned(), "application/json"),
            ("referer".to_owned(), "https://data.eastmoney.com/"),
        ]);
        if endpoint != "https://datacenter-web.eastmoney.com/api/data/v1/get"
            || actual_query != expected_query
            || actual_headers != expected_headers
            || maximum_bytes != 4 * 1024 * 1024
        {
            return Err(EastmoneyError::Protocol(
                "entries fixture request or transport bound mismatch".into(),
            ));
        }
        self.0
            .lock()
            .map_err(|_| EastmoneyError::Protocol("entries fixture poisoned".into()))?
            .take()
            .map(<[u8]>::to_vec)
            .ok_or_else(|| EastmoneyError::Protocol("entries fixture has no further pages".into()))
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Protocol(
            "entries fixture does not serve POST requests".into(),
        ))
    }
}

#[test]
fn registered_dragon_tiger_entries_preserve_amounts_and_disclosure_evidence() {
    let client = EastmoneyClient::with_transport(EntriesReportFixture(Mutex::new(Some(
        ENTRIES_REPORT.as_bytes(),
    ))));
    let mut registry = OperationRegistry::all_unadmitted("offline external source only");
    // Actual production registration is setup; public Execute is the test seam.
    register_eastmoney_client(&mut registry, client, 4096).unwrap();
    let command = QueryCommand::new(
        "offline-eastmoney-dragon-tiger-entry-v1",
        Operation::DragonTiger,
        Some("Eastmoney".into()),
        CanonicalPayload::new(
            "magic.market.dragon_tiger.request",
            1,
            br#"{"kind":"entries","request":{"instrument":{"exchange":"Shenzhen","code":"002475","asset_class":"Equity"},"trading_date":"2026-07-23","limit":1}}"#.to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap();
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let result = registry.execute(command).unwrap();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let observed_millis: u128 = result
        .observed_at
        .strip_prefix("unix-ms:")
        .unwrap()
        .parse()
        .unwrap();
    assert!(before <= observed_millis && observed_millis <= after);
    assert_eq!(result.provider, "Eastmoney");
    assert!(result.complete);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
    assert_ne!(
        result.source_at.as_deref(),
        Some(result.observed_at.as_str())
    );
    assert_eq!(
        result.batch_id,
        format!("eastmoney-web:dragon-tiger-entries:{}", result.observed_at)
    );
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.dragon_tiger_entry");
    assert_eq!(payload.schema_version(), 1);
    assert!(payload.data().len() <= 4096);

    let actual: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    let mut expected: serde_json::Value = serde_json::from_str(EXPECTED_ENTRY).unwrap();
    expected["evidence"]["observed_at"] = serde_json::Value::String(result.observed_at.clone());
    expected["evidence"]["batch_id"] = serde_json::Value::String(result.batch_id.clone());
    assert_eq!(actual, expected);

    let entry: DragonTigerEntry = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(entry.buy_amount().unwrap().get(), 100.0);
    assert_eq!(entry.sell_amount().unwrap().get(), 40.0);
    assert_eq!(entry.net_amount().unwrap().get(), 60.0);
    assert_eq!(entry.turnover_rate().unwrap().get(), 12.5);
    assert_eq!(entry.turnover_rate().unwrap().unit(), RatioUnit::Percent);
    assert_eq!(entry.evidence().provider(), ProviderId::Eastmoney);
    assert_eq!(entry.evidence().source_at(), Some("2026-07-23"));
    assert_eq!(entry.evidence().observed_at(), result.observed_at);
    assert_eq!(entry.evidence().batch_id(), result.batch_id);
}
