use super::super::register_eastmoney_market_rankings_client;
use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_service::{CanonicalPayload, Operation, OperationRegistry, QueryCommand};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// Synthetic native source response, not live admission evidence. This is the
// bounded first-page contract, not full-market pagination or dated ProviderTopN.
const RANKING_SOURCE: &str = r#"{"rc":0,"data":{"total":3,"diff":[
    {"f10":2.5,"f12":"600000","f13":1,"f14":"浦发银行","f62":0,"f124":1784872800},
    {"f10":1.5,"f12":"000001","f13":0,"f14":"平安银行","f62":-125000,"f124":1784872801},
    {"f10":1.0,"f12":"600519","f13":1,"f14":"贵州茅台","f62":-200000,"f124":1784872802}
]}}"#;

#[derive(Debug, PartialEq, Eq)]
struct RankingGetTrace {
    endpoint: String,
    query: BTreeMap<String, String>,
    headers: BTreeMap<String, String>,
    maximum_bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum RankingHttpCall {
    Get(RankingGetTrace),
    Post,
}

// Literal encoded query values keep parameter order irrelevant. No production
// URL builder, decoder, parser or ranking mapper supplies the expected values.
fn expected_ranking_query() -> BTreeMap<String, String> {
    [
        ("pn", "1"),
        ("pz", "100"),
        ("po", "1"),
        ("np", "1"),
        ("ut", "8dec03ba335b81bf4ebdf7b29ec27d15"),
        ("fltt", "2"),
        ("invt", "2"),
        ("fid", "f62"),
        ("fs", "m%3A0%2Bt%3A6%2Bf%3A%212%2Cm%3A0%2Bt%3A13%2Bf%3A%212%2Cm%3A0%2Bt%3A80%2Bf%3A%212%2Cm%3A1%2Bt%3A2%2Bf%3A%212%2Cm%3A1%2Bt%3A23%2Bf%3A%212%2Cm%3A0%2Bt%3A81%2Bs%3A262144%2Bf%3A%212"),
        ("fields", "f1%2Cf10%2Cf12%2Cf13%2Cf14%2Cf62%2Cf124"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect()
}

fn expected_ranking_headers() -> BTreeMap<String, String> {
    [
        ("Accept", "application/json"),
        ("Referer", "https://data.eastmoney.com/"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect()
}

struct RankingFixture {
    calls: Arc<Mutex<Vec<RankingHttpCall>>>,
}

impl EastmoneyTransport for RankingFixture {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        let mismatch = || EastmoneyError::Protocol("ranking fixture request mismatch".into());
        let (endpoint, query_string) = url.split_once('?').ok_or_else(mismatch)?;
        if endpoint != "https://push2.eastmoney.com/api/qt/clist/get"
            || url.contains('#')
            || maximum_bytes != 4_194_304
        {
            return Err(mismatch());
        }
        let mut query = BTreeMap::new();
        for pair in query_string.split('&') {
            let (key, value) = pair.split_once('=').ok_or_else(mismatch)?;
            if query.insert(key.to_owned(), value.to_owned()).is_some() {
                return Err(mismatch());
            }
        }
        if query != expected_ranking_query() {
            return Err(mismatch());
        }
        let mut actual_headers = BTreeMap::new();
        for (key, value) in headers {
            if actual_headers
                .insert((*key).to_owned(), (*value).to_owned())
                .is_some()
            {
                return Err(mismatch());
            }
        }
        if actual_headers != expected_ranking_headers() {
            return Err(mismatch());
        }
        self.calls
            .lock()
            .unwrap()
            .push(RankingHttpCall::Get(RankingGetTrace {
                endpoint: endpoint.to_owned(),
                query,
                headers: actual_headers,
                maximum_bytes,
            }));
        Ok(RANKING_SOURCE.as_bytes().to_vec())
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.calls.lock().unwrap().push(RankingHttpCall::Post);
        Err(EastmoneyError::Protocol(
            "ranking fixture does not serve POST".into(),
        ))
    }
}

#[test]
fn registered_market_rankings_preserve_signed_cny_and_per_row_source_evidence() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let client = EastmoneyClient::with_transport(RankingFixture {
        calls: Arc::clone(&calls),
    });
    let mut registry = OperationRegistry::all_unadmitted("offline transport fixture only");
    // The real production registrar is setup only. Execute is the test entry.
    register_eastmoney_market_rankings_client(&mut registry, client, 4096).unwrap();
    let command = QueryCommand::new(
        "offline-market-ranking-public-query",
        Operation::MarketRankings,
        Some("Eastmoney".into()),
        CanonicalPayload::new(
            "magic.market.market_rankings.request",
            1,
            br#"{"kind":"MainNetInflow","limit":2}"#.to_vec(),
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
    // Row timestamps differ; do not invent one common outer source timestamp.
    assert_eq!(result.source_at, None);
    let observed_suffix = result.observed_at.strip_prefix("unix-ms:").unwrap();
    assert!(!observed_suffix.is_empty());
    assert!(observed_suffix.bytes().all(|byte| byte.is_ascii_digit()));
    let observed_millis = observed_suffix.parse::<u128>().unwrap();
    assert!(before <= observed_millis && observed_millis <= after);
    assert_eq!(
        result.batch_id,
        format!("eastmoney-web:market-ranking-snapshot:unix-ms:{observed_millis}")
    );
    assert_eq!(result.records.len(), 2);
    let actual_wire = result
        .records
        .iter()
        .map(|payload| {
            assert_eq!(
                payload.schema(),
                "magic.market.market_ranking_diagnostic_entry"
            );
            assert_eq!(payload.schema_version(), 1);
            serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap()
        })
        .collect::<Vec<_>>();
    // All business fields are independent literals. Only the independently
    // bracketed observation and its validated batch identity are dynamic.
    let expected_wire = serde_json::json!([
        {
            "kind":"MainNetInflow",
            "source_rank":1,
            "instrument":{"exchange":"Shanghai","code":"600000","asset_class":"Equity"},
            "label":"浦发银行",
            "value":0.0,
            "unit":"Yuan",
            "source_at":"2026-07-24T14:00:00+08:00",
            "reported_universe_size":3,
            "fetched_count":3,
            "evidence":{
                "provider":"Eastmoney",
                "source_at":"2026-07-24T14:00:00+08:00",
                "observed_at":result.observed_at.as_str(),
                "batch_id":result.batch_id.as_str()
            }
        },
        {
            "kind":"MainNetInflow",
            "source_rank":2,
            "instrument":{"exchange":"Shenzhen","code":"000001","asset_class":"Equity"},
            "label":"平安银行",
            "value":-125000.0,
            "unit":"Yuan",
            "source_at":"2026-07-24T14:00:01+08:00",
            "reported_universe_size":3,
            "fetched_count":3,
            "evidence":{
                "provider":"Eastmoney",
                "source_at":"2026-07-24T14:00:01+08:00",
                "observed_at":result.observed_at.as_str(),
                "batch_id":result.batch_id.as_str()
            }
        }
    ]);
    assert_eq!(actual_wire, expected_wire.as_array().unwrap().to_vec());
    assert_eq!(
        *calls.lock().unwrap(),
        vec![RankingHttpCall::Get(RankingGetTrace {
            endpoint: "https://push2.eastmoney.com/api/qt/clist/get".into(),
            query: expected_ranking_query(),
            headers: expected_ranking_headers(),
            maximum_bytes: 4_194_304,
        })]
    );
}

#[test]
fn registered_market_rankings_rejects_oversized_reported_universe_without_publication() {
    // Test-local external transport only. Existing success test and helpers stay byte-identical.
    #[derive(Debug, PartialEq, Eq)]
    enum UniverseHttpAttempt {
        Get {
            url: String,
            headers: Vec<(String, String)>,
            maximum_bytes: usize,
        },
        Post {
            url: String,
            headers: Vec<(String, String)>,
            body: Vec<u8>,
            maximum_bytes: usize,
        },
    }

    struct UniverseFixture {
        body: Vec<u8>,
        attempts: Arc<Mutex<Vec<UniverseHttpAttempt>>>,
    }

    impl EastmoneyTransport for UniverseFixture {
        fn get(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            maximum_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            // Preserve ALL entries and duplicate/order information before every guard.
            self.attempts
                .lock()
                .unwrap()
                .push(UniverseHttpAttempt::Get {
                    url: url.to_owned(),
                    headers: headers
                        .iter()
                        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                        .collect(),
                    maximum_bytes,
                });
            let mismatch = || EastmoneyError::Protocol("universe fixture GET mismatch".into());
            let (endpoint, raw_query) = url.split_once('?').ok_or_else(mismatch)?;
            if endpoint != "https://push2.eastmoney.com/api/qt/clist/get"
                || url.contains('#')
                || maximum_bytes != 4_194_304
                || headers
                    != [
                        ("Accept", "application/json"),
                        ("Referer", "https://data.eastmoney.com/"),
                    ]
                || self.body.len() > maximum_bytes
            {
                return Err(mismatch());
            }
            let mut query = BTreeMap::new();
            for pair in raw_query.split('&') {
                let (key, value) = pair.split_once('=').ok_or_else(mismatch)?;
                if query.insert(key.to_owned(), value.to_owned()).is_some() {
                    return Err(mismatch());
                }
            }
            if query != expected_ranking_query() {
                return Err(mismatch());
            }
            Ok(self.body.clone())
        }

        fn post_json(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            body: &[u8],
            maximum_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            self.attempts
                .lock()
                .unwrap()
                .push(UniverseHttpAttempt::Post {
                    url: url.to_owned(),
                    headers: headers
                        .iter()
                        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                        .collect(),
                    body: body.to_vec(),
                    maximum_bytes,
                });
            Err(EastmoneyError::Protocol(
                "universe fixture does not serve POST".into(),
            ))
        }
    }

    fn registry(body: &[u8], attempts: &Arc<Mutex<Vec<UniverseHttpAttempt>>>) -> OperationRegistry {
        let client = EastmoneyClient::with_transport(UniverseFixture {
            body: body.to_vec(),
            attempts: Arc::clone(attempts),
        });
        let mut registry = OperationRegistry::all_unadmitted("offline ranking universe contract");
        register_eastmoney_market_rankings_client(&mut registry, client, 4096).unwrap();
        registry
    }

    fn command(request_id: &str) -> QueryCommand {
        let command = QueryCommand::new(
            request_id,
            Operation::MarketRankings,
            Some("Eastmoney".into()),
            CanonicalPayload::new(
                "magic.market.market_rankings.request",
                1,
                br#"{"kind":"MainNetInflow","limit":2}"#.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(!command.allows_unadmitted());
        command
    }

    fn assert_one_get(attempts: &Arc<Mutex<Vec<UniverseHttpAttempt>>>) {
        let attempts = attempts.lock().unwrap();
        assert_eq!(attempts.len(), 1);
        let UniverseHttpAttempt::Get {
            url,
            headers,
            maximum_bytes,
        } = &attempts[0]
        else {
            panic!("public ranking unexpectedly called POST");
        };
        let (endpoint, raw_query) = url.split_once('?').unwrap();
        assert_eq!(endpoint, "https://push2.eastmoney.com/api/qt/clist/get");
        assert!(!url.contains('#'));
        assert_eq!(*maximum_bytes, 4_194_304);
        assert_eq!(
            headers.as_slice(),
            &[
                ("Accept".to_owned(), "application/json".to_owned()),
                (
                    "Referer".to_owned(),
                    "https://data.eastmoney.com/".to_owned()
                ),
            ]
        );
        let mut query = BTreeMap::new();
        for pair in raw_query.split('&') {
            let (key, value) = pair.split_once('=').unwrap();
            assert!(query.insert(key.to_owned(), value.to_owned()).is_none());
        }
        assert_eq!(query, expected_ranking_query());
    }

    assert_eq!(RANKING_SOURCE.matches(r#""total":3"#).count(), 1);
    let oversized = RANKING_SOURCE.replacen(r#""total":3"#, r#""total":20001"#, 1);
    assert_eq!(
        oversized.replacen(r#""total":20001"#, r#""total":3"#, 1),
        RANKING_SOURCE
    );

    let normal_attempts = Arc::new(Mutex::new(Vec::new()));
    let normal_registry = registry(RANKING_SOURCE.as_bytes(), &normal_attempts);
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let result = normal_registry
        .execute(command("ranking-universe-normal-control"))
        .unwrap();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    assert_eq!(result.provider, "Eastmoney");
    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at, None);
    assert_eq!(result.records.len(), 2);
    let suffix = result.observed_at.strip_prefix("unix-ms:").unwrap();
    assert!(!suffix.is_empty());
    assert!(suffix.bytes().all(|byte| byte.is_ascii_digit()));
    let observed_millis = suffix.parse::<u128>().unwrap();
    assert!(before <= observed_millis && observed_millis <= after);
    assert_eq!(
        result.batch_id,
        format!("eastmoney-web:market-ranking-snapshot:unix-ms:{observed_millis}")
    );
    let actual = result
        .records
        .iter()
        .map(|payload| {
            assert_eq!(
                payload.schema(),
                "magic.market.market_ranking_diagnostic_entry"
            );
            assert_eq!(payload.schema_version(), 1);
            let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
            assert_eq!(row.as_object().unwrap().len(), 10);
            assert_eq!(row["instrument"].as_object().unwrap().len(), 3);
            assert_eq!(row["evidence"].as_object().unwrap().len(), 4);
            row
        })
        .collect::<Vec<_>>();
    let expected = serde_json::json!([
        {
            "kind":"MainNetInflow", "source_rank":1,
            "instrument":{"exchange":"Shanghai","code":"600000","asset_class":"Equity"},
            "label":"浦发银行", "value":0.0, "unit":"Yuan",
            "source_at":"2026-07-24T14:00:00+08:00",
            "reported_universe_size":3, "fetched_count":3,
            "evidence":{"provider":"Eastmoney","source_at":"2026-07-24T14:00:00+08:00",
                "observed_at":result.observed_at.as_str(),"batch_id":result.batch_id.as_str()}
        },
        {
            "kind":"MainNetInflow", "source_rank":2,
            "instrument":{"exchange":"Shenzhen","code":"000001","asset_class":"Equity"},
            "label":"平安银行", "value":-125000.0, "unit":"Yuan",
            "source_at":"2026-07-24T14:00:01+08:00",
            "reported_universe_size":3, "fetched_count":3,
            "evidence":{"provider":"Eastmoney","source_at":"2026-07-24T14:00:01+08:00",
                "observed_at":result.observed_at.as_str(),"batch_id":result.batch_id.as_str()}
        }
    ]);
    assert_eq!(actual, expected.as_array().unwrap().to_vec());
    assert_one_get(&normal_attempts);

    let rejected_attempts = Arc::new(Mutex::new(Vec::new()));
    let rejected_registry = registry(oversized.as_bytes(), &rejected_attempts);
    assert_eq!(
        rejected_registry
            .execute(command("ranking-universe-above-safety-limit"))
            .unwrap_err(),
        magic_market_service::ServiceError::FailedPrecondition(
            "Eastmoney protocol error: Eastmoney ranking universe 20001 is outside 1..=20000"
                .into()
        )
    );
    assert_one_get(&rejected_attempts);
}

#[test]
fn registered_volume_ratio_rejects_negative_second_row_without_publishing_prefix() {
    // Native, synthetic response bytes; only the external transport is replaced.
    const NORMAL: &str = r#"{"rc":0,"data":{"total":2,"diff":[
        {"f10":2.5,"f12":"600000","f13":1,"f14":"浦发银行","f62":0,"f124":1784872800},
        {"f10":0,"f12":"000001","f13":0,"f14":"平安银行","f62":-125000,"f124":1784872801}
    ]}}
"#;

    #[derive(Debug, PartialEq, Eq)]
    enum Attempt {
        Get {
            url: String,
            headers: Vec<(String, String)>,
            maximum_bytes: usize,
        },
        Post {
            url: String,
            headers: Vec<(String, String)>,
            body: Vec<u8>,
            maximum_bytes: usize,
        },
    }

    fn expected_headers() -> Vec<(String, String)> {
        vec![
            ("Accept".into(), "application/json".into()),
            ("Referer".into(), "https://data.eastmoney.com/".into()),
        ]
    }

    fn expected_query() -> BTreeMap<String, String> {
        [
            ("pn", "1"),
            ("pz", "100"),
            ("po", "1"),
            ("np", "1"),
            ("ut", "8dec03ba335b81bf4ebdf7b29ec27d15"),
            ("fltt", "2"),
            ("invt", "2"),
            ("fid", "f10"),
            ("fs", "m%3A0%2Bt%3A6%2Bf%3A%212%2Cm%3A0%2Bt%3A13%2Bf%3A%212%2Cm%3A0%2Bt%3A80%2Bf%3A%212%2Cm%3A1%2Bt%3A2%2Bf%3A%212%2Cm%3A1%2Bt%3A23%2Bf%3A%212%2Cm%3A0%2Bt%3A81%2Bs%3A262144%2Bf%3A%212"),
            ("fields", "f1%2Cf10%2Cf12%2Cf13%2Cf14%2Cf62%2Cf124"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
    }

    fn request_matches(url: &str, headers: &[(String, String)], maximum_bytes: usize) -> bool {
        let Some((endpoint, query_string)) = url.split_once('?') else {
            return false;
        };
        if endpoint != "https://push2.eastmoney.com/api/qt/clist/get"
            || url.contains('#')
            || maximum_bytes != 4_194_304
            || headers != expected_headers()
        {
            return false;
        }
        let mut query = BTreeMap::new();
        for pair in query_string.split('&') {
            let Some((key, value)) = pair.split_once('=') else {
                return false;
            };
            if query.insert(key.to_owned(), value.to_owned()).is_some() {
                return false;
            }
        }
        query == expected_query()
    }

    struct Fixture {
        response: Vec<u8>,
        attempts: Arc<Mutex<Vec<Attempt>>>,
    }

    impl EastmoneyTransport for Fixture {
        fn get(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            maximum_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            let raw_headers = headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect::<Vec<_>>();
            // Preserve every attempted call, including rejected calls and duplicates.
            self.attempts.lock().unwrap().push(Attempt::Get {
                url: url.to_owned(),
                headers: raw_headers.clone(),
                maximum_bytes,
            });
            if !request_matches(url, &raw_headers, maximum_bytes) {
                return Err(EastmoneyError::Protocol(
                    "volume ratio fixture request mismatch".into(),
                ));
            }
            if self.response.len() > maximum_bytes {
                return Err(EastmoneyError::ResponseTooLarge {
                    limit: maximum_bytes,
                });
            }
            Ok(self.response.clone())
        }

        fn post_json(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            body: &[u8],
            maximum_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            self.attempts.lock().unwrap().push(Attempt::Post {
                url: url.to_owned(),
                headers: headers
                    .iter()
                    .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                    .collect(),
                body: body.to_vec(),
                maximum_bytes,
            });
            Err(EastmoneyError::Protocol(
                "volume ratio fixture does not serve POST".into(),
            ))
        }
    }

    fn assert_one_get(attempts: &Arc<Mutex<Vec<Attempt>>>) {
        let attempts = attempts.lock().unwrap();
        assert_eq!(attempts.len(), 1, "no retry, failover or POST is expected");
        match &attempts[0] {
            Attempt::Get {
                url,
                headers,
                maximum_bytes,
            } => assert!(request_matches(url, headers, *maximum_bytes)),
            other => panic!("unexpected external request: {other:?}"),
        }
    }

    let command = || {
        QueryCommand::new(
            "offline-volume-ratio-public-query",
            Operation::MarketRankings,
            Some("Eastmoney".into()),
            CanonicalPayload::new(
                "magic.market.market_rankings.request",
                1,
                br#"{"kind":"VolumeRatio","limit":2}"#.to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap()
    };
    let normal_attempts = Arc::new(Mutex::new(Vec::new()));
    let normal_client = EastmoneyClient::with_transport(Fixture {
        response: NORMAL.as_bytes().to_vec(),
        attempts: Arc::clone(&normal_attempts),
    });
    let mut normal_registry = OperationRegistry::all_unadmitted("offline transport fixture only");
    // The actual production registrar is setup; public Execute is the entry.
    register_eastmoney_market_rankings_client(&mut normal_registry, normal_client, 4096).unwrap();
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let result = normal_registry.execute(command()).unwrap();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    assert_eq!(result.provider, "Eastmoney");
    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at, None);
    // Validate O independently BEFORE substituting it into the whole wire oracle.
    let observed_suffix = result.observed_at.strip_prefix("unix-ms:").unwrap();
    assert!(!observed_suffix.is_empty());
    assert!(observed_suffix.bytes().all(|byte| byte.is_ascii_digit()));
    let observed_millis = observed_suffix.parse::<u128>().unwrap();
    assert_eq!(observed_suffix, observed_millis.to_string());
    assert!(before <= observed_millis && observed_millis <= after);
    assert_eq!(
        result.batch_id,
        format!("eastmoney-web:market-ranking-snapshot:unix-ms:{observed_millis}")
    );
    assert_eq!(result.records.len(), 2);
    let actual_wire = result
        .records
        .iter()
        .map(|payload| {
            assert_eq!(
                payload.schema(),
                "magic.market.market_ranking_diagnostic_entry"
            );
            assert_eq!(payload.schema_version(), 1);
            serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap()
        })
        .collect::<Vec<_>>();
    // All business values are literal: zero is a valid dimensionless multiple,
    // and the two source timestamps remain distinct despite common O and B.
    let expected_wire = serde_json::json!([
        {
            "kind":"VolumeRatio",
            "source_rank":1,
            "instrument":{"exchange":"Shanghai","code":"600000","asset_class":"Equity"},
            "label":"浦发银行",
            "value":2.5,
            "unit":"Multiple",
            "source_at":"2026-07-24T14:00:00+08:00",
            "reported_universe_size":2,
            "fetched_count":2,
            "evidence":{
                "provider":"Eastmoney",
                "source_at":"2026-07-24T14:00:00+08:00",
                "observed_at":result.observed_at.as_str(),
                "batch_id":result.batch_id.as_str()
            }
        },
        {
            "kind":"VolumeRatio",
            "source_rank":2,
            "instrument":{"exchange":"Shenzhen","code":"000001","asset_class":"Equity"},
            "label":"平安银行",
            "value":0.0,
            "unit":"Multiple",
            "source_at":"2026-07-24T14:00:01+08:00",
            "reported_universe_size":2,
            "fetched_count":2,
            "evidence":{
                "provider":"Eastmoney",
                "source_at":"2026-07-24T14:00:01+08:00",
                "observed_at":result.observed_at.as_str(),
                "batch_id":result.batch_id.as_str()
            }
        }
    ]);
    assert_eq!(actual_wire, expected_wire.as_array().unwrap().to_vec());
    assert_one_get(&normal_attempts);

    // Sole raw delta: the selected second row changes from zero to -0.5.
    assert_eq!(NORMAL.matches("\"f10\":0,").count(), 1);
    let invalid = NORMAL.replacen("\"f10\":0,", "\"f10\":-0.5,", 1);
    assert_eq!(invalid.matches("\"f10\":-0.5,").count(), 1);
    assert_eq!(invalid.replacen("\"f10\":-0.5,", "\"f10\":0,", 1), NORMAL);
    let invalid_attempts = Arc::new(Mutex::new(Vec::new()));
    let invalid_client = EastmoneyClient::with_transport(Fixture {
        response: invalid.into_bytes(),
        attempts: Arc::clone(&invalid_attempts),
    });
    let mut invalid_registry = OperationRegistry::all_unadmitted("offline transport fixture only");
    register_eastmoney_market_rankings_client(&mut invalid_registry, invalid_client, 4096).unwrap();
    match invalid_registry.execute(command()) {
        Err(magic_market_service::ServiceError::FailedPrecondition(reason)) => assert_eq!(
            reason,
            "Eastmoney protocol error: market ranking volume ratio must be non-negative"
        ),
        Err(other) => panic!("unexpected typed error: {other:?}"),
        Ok(_) => panic!("negative second row must reject the whole public result"),
    }
    assert_one_get(&invalid_attempts);
}
