use super::register_eastmoney_client;
use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::{HolderCount, ProviderId, RatioUnit};
use magic_market_service::{CanonicalPayload, Operation, OperationRegistry, QueryCommand};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// Synthetic native report page, adapted from capital_tests.rs. Values describe
// source report fields, not trades, selling, reduction or verified disclosures.
const HOLDER_REPORTS: &[u8] = br#"{
    "success":true,"code":0,"result":{"pages":1,"count":4,"data":[
        {"SECURITY_CODE":"600519","SECUCODE":"600519.SH","END_DATE":"2026-06-30",
         "HOLDER_NUM":12345,"HOLDER_NUM_CHANGE":-10,"HOLDER_NUM_RATIO":-0.08,
         "AVG_FREE_SHARES":4567},
        {"SECURITY_CODE":"600519","SECUCODE":"600519.SH","END_DATE":"2026-06-20",
         "HOLDER_NUM":12355,"HOLDER_NUM_CHANGE":10,"HOLDER_NUM_RATIO":0.08,
         "AVG_FREE_SHARES":4568},
        {"SECURITY_CODE":"600519","SECUCODE":"600519.SH","END_DATE":"2026-06-10",
         "HOLDER_NUM":12345,"HOLDER_NUM_CHANGE":0,"HOLDER_NUM_RATIO":0,
         "AVG_FREE_SHARES":0},
        {"SECURITY_CODE":"600519","SECUCODE":"600519.SH","END_DATE":"2026-06-01",
         "HOLDER_NUM":12345,"HOLDER_NUM_RATIO":null}
    ]}
}"#;

// Independent complete wire oracle. Only validated local observation metadata
// is filled below; no business value is read from the actual output or mapped
// with production constructors, parsers or normalization helpers.
const EXPECTED_HOLDER_REPORTS: &str = r#"[
    {"instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
     "report_date":"2026-06-30","holders":12345.0,"holder_change":-10.0,
     "change_ratio":{"value":-0.08,"unit":"Percent"},"average_shares_per_holder":4567.0,
     "evidence":{"provider":"Eastmoney","source_at":"2026-06-30","observed_at":"","batch_id":""}},
    {"instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
     "report_date":"2026-06-20","holders":12355.0,"holder_change":10.0,
     "change_ratio":{"value":0.08,"unit":"Percent"},"average_shares_per_holder":4568.0,
     "evidence":{"provider":"Eastmoney","source_at":"2026-06-20","observed_at":"","batch_id":""}},
    {"instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
     "report_date":"2026-06-10","holders":12345.0,"holder_change":0.0,
     "change_ratio":{"value":0.0,"unit":"Percent"},"average_shares_per_holder":0.0,
     "evidence":{"provider":"Eastmoney","source_at":"2026-06-10","observed_at":"","batch_id":""}},
    {"instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
     "report_date":"2026-06-01","holders":12345.0,"holder_change":null,
     "change_ratio":null,"average_shares_per_holder":null,
     "evidence":{"provider":"Eastmoney","source_at":"2026-06-01","observed_at":"","batch_id":""}}
]"#;

struct HolderReportsFixture(Mutex<Option<&'static [u8]>>);

impl EastmoneyTransport for HolderReportsFixture {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        let (endpoint, query) = url.split_once('?').ok_or_else(|| {
            EastmoneyError::Protocol("holder report fixture requires a bounded GET query".into())
        })?;
        let mut actual = query
            .split('&')
            .map(|pair| {
                pair.split_once('=').ok_or_else(|| {
                    EastmoneyError::Protocol("holder report fixture requires query pairs".into())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut expected = [
            ("reportName", "RPT_HOLDERNUMLATEST"),
            ("columns", "ALL"),
            ("filter", "%28SECURITY_CODE%3D%22600519%22%29%28END_DATE%3E%3D%272026-06-01%27%29%28END_DATE%3C%3D%272026-06-30%27%29"),
            ("pageNumber", "1"),
            ("pageSize", "500"),
            ("sortColumns", "END_DATE"),
            ("sortTypes", "-1"),
            ("source", "WEB"),
            ("client", "WEB"),
        ];
        actual.sort_unstable();
        expected.sort_unstable();
        if endpoint != "https://datacenter-web.eastmoney.com/api/data/v1/get"
            || actual.as_slice() != expected.as_slice()
            || maximum_bytes != 4 * 1024 * 1024
            || headers
                != [
                    ("Accept", "application/json"),
                    ("Referer", "https://data.eastmoney.com/"),
                ]
        {
            return Err(EastmoneyError::Protocol(
                "holder report fixture request or transport bound mismatch".into(),
            ));
        }
        self.0
            .lock()
            .map_err(|_| EastmoneyError::Protocol("holder report fixture poisoned".into()))?
            .take()
            .map(<[u8]>::to_vec)
            .ok_or_else(|| {
                EastmoneyError::Protocol("holder report fixture has no further pages".into())
            })
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Protocol(
            "holder report fixture does not serve POST requests".into(),
        ))
    }
}

#[test]
fn registered_holder_counts_preserve_signed_zero_missing_and_report_date_evidence() {
    let client =
        EastmoneyClient::with_transport(HolderReportsFixture(Mutex::new(Some(HOLDER_REPORTS))));
    let mut registry = OperationRegistry::all_unadmitted("offline external source only");
    // Actual production setup only; public execute is the assertion seam.
    register_eastmoney_client(&mut registry, client, 4096).unwrap();
    let command = QueryCommand::new(
        "offline-eastmoney-holder-report-fields-v1",
        Operation::HolderCounts,
        Some("Eastmoney".into()),
        CanonicalPayload::new(
            "magic.market.holder_counts.request",
            1,
            br#"{"instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},"start":"2026-06-01","end":"2026-06-30","limit":4}"#.to_vec(),
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
    assert_eq!(result.source_at.as_deref(), Some("2026-06-30"));
    assert_ne!(
        result.source_at.as_deref(),
        Some(result.observed_at.as_str())
    );
    assert_eq!(
        result.batch_id,
        format!("eastmoney-web:holder-counts:{}", result.observed_at)
    );
    assert_eq!(result.records.len(), 4);

    let mut actual = Vec::new();
    let mut typed = Vec::new();
    for payload in &result.records {
        assert_eq!(payload.schema(), "magic.market.holder_count");
        assert_eq!(payload.schema_version(), 1);
        assert!(payload.data().len() <= 4096);
        actual.push(serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap());
        typed.push(serde_json::from_slice::<HolderCount>(payload.data()).unwrap());
    }
    assert_eq!(typed[0].holder_change.unwrap().get(), -10.0);
    assert_eq!(typed[1].holder_change.unwrap().get(), 10.0);
    assert_eq!(typed[2].holder_change.unwrap().get(), 0.0);
    assert_eq!(typed[3].holder_change, None);
    assert_eq!(typed[0].change_ratio.unwrap().unit(), RatioUnit::Percent);
    assert_eq!(typed[1].change_ratio.unwrap().unit(), RatioUnit::Percent);
    assert_eq!(typed[2].change_ratio.unwrap().unit(), RatioUnit::Percent);
    assert_eq!(typed[3].change_ratio, None);
    assert_eq!(typed[2].average_shares_per_holder.unwrap().get(), 0.0);
    assert_eq!(typed[3].average_shares_per_holder, None);
    assert_eq!(typed[0].evidence.provider(), ProviderId::Eastmoney);

    let mut expected: serde_json::Value = serde_json::from_str(EXPECTED_HOLDER_REPORTS).unwrap();
    for report in expected.as_array_mut().unwrap() {
        report["evidence"]["observed_at"] = serde_json::Value::String(result.observed_at.clone());
        report["evidence"]["batch_id"] = serde_json::Value::String(result.batch_id.clone());
    }
    assert_eq!(serde_json::Value::Array(actual), expected);
}
