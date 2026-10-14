use super::*;
use magic_market_core::{DividendPlan, EvidenceTimestamp, InstrumentDateRangeRequest, IsoDate};
use std::time::{SystemTime, UNIX_EPOCH};

// Synthetic one-page native shape, based on the provider capital_tests.rs.
// Per-ten terms and report dates remain metadata, not proof of execution or
// shareholder selling. Neither this sample nor the local registry admits a
// new source or supplies live admission evidence.
const DIVIDEND_TERMS: &[u8] = br#"{
    "success":true,"code":0,"result":{"pages":1,"data":[
        {"SECURITY_CODE":"600519","SECUCODE":"600519.SH",
         "REPORT_DATE":"2025-12-31","EX_DIVIDEND_DATE":"2026-06-20",
         "ASSIGN_PROGRESS":"offline-source-state-A","PRETAX_BONUS_RMB":28.0,
         "BONUS_RATIO":1.0,"TRANSFER_RATIO":2.0,"ALLOTMENT_RATIO":0.5,
         "REDUCTION_RATIO":3.0},
        {"SECURITY_CODE":"600519","SECUCODE":"600519.SH",
         "REPORT_DATE":"2026-06-30","EX_DIVIDEND_DATE":null,
         "ASSIGN_PROGRESS":"offline-source-state-B","PRETAX_BONUS_RMB":0,
         "BONUS_RATIO":null,"TRANSFER_RATIO":0,"ALLOTMENT_RATIO":null,
         "REDUCTION_RATIO":null}
    ]}
}"#;

struct DividendTermsFixture;

impl EastmoneyTransport for DividendTermsFixture {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        let (endpoint, query) = url.split_once('?').ok_or_else(|| {
            EastmoneyError::Protocol("dividend fixture requires a bounded GET query".into())
        })?;
        if endpoint != "https://datacenter-web.eastmoney.com/api/data/v1/get"
            || maximum_bytes != 4 * 1024 * 1024
            || headers
                != [
                    ("Accept", "application/json"),
                    ("Referer", "https://data.eastmoney.com/"),
                ]
        {
            return Err(EastmoneyError::Protocol(
                "dividend fixture endpoint or transport bound mismatch".into(),
            ));
        }
        // Independent request literals; parameter ordering is not significant.
        // Page geometry belongs to the external source contract, not caller limit.
        for (key, expected) in [
            ("reportName", "RPT_SHAREBONUS_DET"),
            ("columns", "ALL"),
            ("filter", "%28SECURITY_CODE%3D%22600519%22%29%28REPORT_DATE%3E%3D%272025-12-31%27%29%28REPORT_DATE%3C%3D%272026-06-30%27%29"),
            ("pageNumber", "1"), ("pageSize", "500"),
            ("sortColumns", "EX_DIVIDEND_DATE"), ("sortTypes", "-1"),
            ("source", "WEB"), ("client", "WEB"),
        ] {
            let mut actual = query.split('&').filter_map(|entry| entry.split_once('='))
                .filter(|(name, _)| *name == key).map(|(_, value)| value);
            if actual.next() != Some(expected) || actual.next().is_some() {
                return Err(EastmoneyError::Protocol(format!("dividend fixture requires exactly one {key}")));
            }
        }
        Ok(DIVIDEND_TERMS.to_vec())
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Protocol(
            "dividend fixture does not serve POST requests".into(),
        ))
    }
}

#[test]
fn registered_dividend_plans_preserve_per_ten_zero_absence_and_report_date_evidence() {
    let client = EastmoneyClient::with_transport(DividendTermsFixture);
    let mut registry = OperationRegistry::all_unadmitted("offline transport sample only");
    register_eastmoney_client(&mut registry, client, 4096).unwrap();
    let request = InstrumentDateRangeRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap()
    .with_range(
        IsoDate::new("2025-12-31").unwrap(),
        IsoDate::new("2026-06-30").unwrap(),
    )
    .unwrap();
    let command = QueryCommand::new(
        "offline-dividend-public-query",
        Operation::DividendPlans,
        Some("Eastmoney".into()),
        CanonicalPayload::new(
            DIVIDEND_PLANS_REQUEST_SCHEMA,
            1,
            serde_json::to_vec(&request).unwrap(),
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
    assert_eq!(result.provider, "Eastmoney");
    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at.as_deref(), Some("2026-06-30"));
    assert!(!result.batch_id.trim().is_empty());
    let observed_millis = result
        .observed_at
        .strip_prefix("unix-ms:")
        .unwrap()
        .parse::<u128>()
        .unwrap();
    assert!(before <= observed_millis && observed_millis <= after);
    assert!(EvidenceTimestamp::parse_instant(&result.observed_at).is_ok());
    assert_eq!(result.records.len(), 2);
    let wire = result
        .records
        .iter()
        .map(|payload| {
            assert_eq!(payload.schema(), "magic.market.dividend_plan");
            assert_eq!(payload.schema_version(), 1);
            serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(wire, serde_json::json!([
        {"instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
         "report_date":"2025-12-31","ex_dividend_date":"2026-06-20","state":"offline-source-state-A",
         "cash_per_ten":28.0,"bonus_per_ten":1.0,"transfer_per_ten":2.0,"allotment_per_ten":0.5,
         "reduction_ratio":{"value":3.0,"unit":"Percent"},
         "evidence":{"provider":"Eastmoney","source_at":"2025-12-31","observed_at":result.observed_at,"batch_id":result.batch_id}},
        {"instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
         "report_date":"2026-06-30","ex_dividend_date":null,"state":"offline-source-state-B",
         "cash_per_ten":0.0,"bonus_per_ten":null,"transfer_per_ten":0.0,"allotment_per_ten":null,
         "reduction_ratio":null,
         "evidence":{"provider":"Eastmoney","source_at":"2026-06-30","observed_at":result.observed_at,"batch_id":result.batch_id}}
    ]).as_array().unwrap().to_vec());
    for payload in &result.records {
        let record: DividendPlan = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(record.evidence.provider(), ProviderId::Eastmoney);
        assert_eq!(record.evidence.batch_id(), result.batch_id);
        assert_eq!(record.evidence.observed_at(), result.observed_at);
        assert_eq!(
            record.evidence.source_at(),
            Some(record.report_date.as_str())
        );
        assert!(EvidenceTimestamp::parse_instant(record.report_date.as_str()).is_err());
    }
}
