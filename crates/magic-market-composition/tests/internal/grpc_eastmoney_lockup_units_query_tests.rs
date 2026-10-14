use super::register_eastmoney_client;
use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_service::{CanonicalPayload, Operation, OperationRegistry, QueryCommand};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// Synthetic one-page source shape from the checked-in capital fixture.
// Share counts and market value are supplied in ten-thousand units, while
// FREE_RATIO is already decimal. This is not proof of shareholder selling.
const LOCKUP_UNITS: &[u8] = br#"{
    "success":true,"code":0,"result":{"pages":1,"count":1,"data":[{
        "SECURITY_CODE":"600519","SECUCODE":"600519.SH",
        "FREE_DATE":"2026-08-01",
        "FREE_SHARES_TYPE":"\u9996\u53d1\u539f\u80a1\u4e1c\u9650\u552e\u80a1\u4efd",
        "FREE_SHARES":12.5,"ABLE_FREE_SHARES":10.0,
        "FREE_RATIO":1.2,"LIFT_MARKET_CAP":300000
    }]}
}"#;

#[derive(Default)]
struct LockupUnitsFixture {
    served: AtomicBool,
}

impl EastmoneyTransport for LockupUnitsFixture {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        let (endpoint, query) = url.split_once('?').ok_or_else(|| {
            EastmoneyError::Protocol("lockup fixture requires a bounded GET query".into())
        })?;
        if endpoint != "https://datacenter-web.eastmoney.com/api/data/v1/get"
            || maximum_bytes != 4_194_304
            || headers
                != [
                    ("Accept", "application/json"),
                    ("Referer", "https://data.eastmoney.com/"),
                ]
        {
            return Err(EastmoneyError::Protocol(
                "lockup fixture endpoint or transport bound mismatch".into(),
            ));
        }

        // Compare the entire independent key set, not query ordering. Reject
        // malformed, duplicate and extra entries instead of ignoring them.
        let mut actual = BTreeMap::new();
        for entry in query.split('&') {
            let (key, value) = entry.split_once('=').ok_or_else(|| {
                EastmoneyError::Protocol("lockup fixture query entry is malformed".into())
            })?;
            if actual.insert(key, value).is_some() {
                return Err(EastmoneyError::Protocol(
                    "lockup fixture query key is duplicated".into(),
                ));
            }
        }
        let expected = BTreeMap::from([
            ("reportName", "RPT_LIFT_STAGE"),
            ("columns", "ALL"),
            (
                "filter",
                "%28SECURITY_CODE%3D%22600519%22%29%28FREE_DATE%3E%3D%272026-08-01%27%29%28FREE_DATE%3C%3D%272026-08-01%27%29",
            ),
            ("pageNumber", "1"),
            ("pageSize", "500"),
            ("sortColumns", "FREE_DATE"),
            ("sortTypes", "-1"),
            ("source", "WEB"),
            ("client", "WEB"),
        ]);
        if actual != expected {
            return Err(EastmoneyError::Protocol(
                "lockup fixture requires the exact bounded query key set".into(),
            ));
        }
        if self.served.swap(true, Ordering::AcqRel) {
            return Err(EastmoneyError::Protocol(
                "lockup fixture serves only one source page".into(),
            ));
        }
        Ok(LOCKUP_UNITS.to_vec())
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Protocol(
            "lockup fixture does not serve POST requests".into(),
        ))
    }
}

#[test]
fn registered_lockup_events_preserve_source_units_and_date_evidence() {
    let client = EastmoneyClient::with_transport(LockupUnitsFixture::default());
    let mut registry = OperationRegistry::all_unadmitted("offline external transport sample only");
    // Real production registration is setup only. The assertion seam below is
    // the public execute result, never a private mapper/client implementation.
    register_eastmoney_client(&mut registry, client, 4096).unwrap();
    let request = br#"{
        "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
        "start":"2026-08-01","end":"2026-08-01","limit":1
    }"#;
    let command = QueryCommand::new(
        "offline-lockup-units-public-query",
        Operation::LockupEvents,
        Some("Eastmoney".into()),
        CanonicalPayload::new(
            "magic.market.lockup_events.request",
            1,
            request.to_vec(),
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
    let observed_millis = result
        .observed_at
        .strip_prefix("unix-ms:")
        .unwrap()
        .parse::<u128>()
        .unwrap();
    assert!(before <= observed_millis && observed_millis <= after);
    assert_eq!(result.provider, "Eastmoney");
    assert!(result.complete);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at.as_deref(), Some("2026-08-01"));
    assert_eq!(
        result.batch_id,
        format!("eastmoney-web:lockups:{}", result.observed_at)
    );
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.lockup_event");
    assert_eq!(payload.schema_version(), 1);
    let actual: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    // Complete independent literal wire oracle. Expected values are not
    // computed by scaling source fields or invoking production helpers.
    let expected = serde_json::json!({
        "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
        "listing_date":"2026-08-01",
        "share_type":"首发原股东限售股份",
        "shares":125000.0,
        "able_shares":100000.0,
        "free_float_ratio":{"value":1.2,"unit":"Decimal"},
        "market_value":3000000000.0,
        "evidence":{
            "provider":"Eastmoney",
            "source_at":"2026-08-01",
            "observed_at":result.observed_at.as_str(),
            "batch_id":result.batch_id.as_str()
        }
    });
    assert_eq!(actual, expected);
}
