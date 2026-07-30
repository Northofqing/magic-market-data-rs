use super::*;
use crate::test_support::ScriptedTransport;
use crate::{EastmoneyClient, EastmoneyTransport};
use magic_market_core::{IsoDate, PostCloseFlows};

#[derive(Clone)]
struct StaticTransport(Vec<u8>);

impl EastmoneyTransport for StaticTransport {
    fn get(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Ok(self.0.clone())
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Unsupported(
            "post-close fixture does not use POST".into(),
        ))
    }
}

fn request() -> PostCloseFlowRequest {
    PostCloseFlowRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap()
}

fn fixture() -> Vec<u8> {
    r#"{
          "rc":0,
          "data":{
            "total":5421,
            "diff":[
              {"f2":16.41,"f3":9.99,"f12":"600396","f13":1,"f14":"华电辽能","f62":100000000.0,"f184":12.34,"f124":1784876400},
              {"f2":11.08,"f3":5.00,"f12":"002475","f13":0,"f14":"立讯精密","f62":90000000.0,"f184":9.87,"f124":1784876400}
            ]
          }
        }"#
        .as_bytes()
        .to_vec()
}

#[test]
fn maps_strict_post_close_ranking_after_the_capture_gate() {
    let batch = parse_post_close(&fixture(), &request(), "2026-07-24T15:35:00+08:00").unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].rank().get(), 1);
    assert_eq!(batch.records()[0].main_net_ratio().get(), 12.34);
    assert_eq!(
        batch.provenance().source_at(),
        Some("2026-07-24T15:00:00+08:00")
    );
}

#[test]
fn rejects_pre_window_stale_and_unsorted_rankings() {
    assert!(parse_post_close(&fixture(), &request(), "2026-07-24T15:34:59+08:00").is_err());
    assert!(parse_post_close(&fixture(), &request(), "2026-07-25T15:35:00+08:00").is_err());
    let unsorted = String::from_utf8(fixture())
        .unwrap()
        .replace("\"f62\":90000000.0", "\"f62\":110000000.0");
    assert!(
        parse_post_close(unsorted.as_bytes(), &request(), "2026-07-24T15:35:00+08:00").is_err()
    );
}

#[test]
fn rejects_mixed_source_snapshots_and_market_identity_mismatch() {
    let mixed = String::from_utf8(fixture()).unwrap().replacen(
        "\"f124\":1784876400",
        "\"f124\":1784876401",
        1,
    );
    assert!(parse_post_close(mixed.as_bytes(), &request(), "2026-07-24T15:35:00+08:00").is_err());
    let wrong_market = String::from_utf8(fixture()).unwrap().replace(
        "\"f12\":\"600396\",\"f13\":1",
        "\"f12\":\"600396\",\"f13\":0",
    );
    assert!(parse_post_close(
        wrong_market.as_bytes(),
        &request(),
        "2026-07-24T15:35:00+08:00"
    )
    .is_err());
}

#[test]
fn post_close_schema_and_cardinality_failures_are_atomic() {
    let observed_at = "2026-07-24T15:35:00+08:00";
    let valid: Value = serde_json::from_slice(&fixture()).unwrap();
    let mut cases = Vec::new();
    cases.push(serde_json::json!({"rc":1}));
    cases.push(serde_json::json!({"rc":0}));
    cases.push(serde_json::json!({"rc":0,"data":{"diff":[]}}));
    cases.push(serde_json::json!({"rc":0,"data":{"total":1,"diff":[]}}));
    cases.push(serde_json::json!({"rc":0,"data":{"total":2}}));
    cases.push(serde_json::json!({"rc":0,"data":{"total":2,"diff":[]}}));

    for (field, replacement) in [
        ("f12", Value::Null),
        ("f13", Value::Null),
        ("f14", Value::Null),
        ("f62", Value::Null),
        ("f124", Value::Null),
        ("f2", serde_json::json!(-1)),
        ("f3", serde_json::json!("bad")),
        ("f184", serde_json::json!("bad")),
    ] {
        let mut value = valid.clone();
        value["data"]["diff"][0][field] = replacement;
        cases.push(value);
    }

    let mut duplicate = valid.clone();
    duplicate["data"]["diff"][1]["f12"] = duplicate["data"]["diff"][0]["f12"].clone();
    duplicate["data"]["diff"][1]["f13"] = duplicate["data"]["diff"][0]["f13"].clone();
    cases.push(duplicate);

    let mut wrong_date = valid.clone();
    wrong_date["data"]["diff"][0]["f124"] = serde_json::json!(1784790000);
    cases.push(wrong_date);
    let mut out_of_range = valid;
    out_of_range["data"]["diff"][0]["f124"] = serde_json::json!(u32::MAX);
    cases.push(out_of_range);

    for value in cases {
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(parse_post_close(&bytes, &request(), observed_at).is_err());
    }
    assert!(parse_post_close(b"{invalid", &request(), observed_at).is_err());
}

#[test]
fn post_close_helpers_preserve_missingness_and_time_boundaries() {
    assert!(validate_capture_window(&request(), "2026-07-24T15:35:00+08:00").is_ok());
    for observed_at in [
        "2026-07-24 15:35:00+08:00",
        "2026-07-24T15:35+08:00",
        "2026-07-24T15:34:59+08:00",
        "2026-07-25T15:35:00+08:00",
    ] {
        assert!(validate_capture_window(&request(), observed_at).is_err());
    }
    assert_eq!(
        required_f64(Some(&serde_json::json!(1.5)), "f").unwrap(),
        1.5
    );
    assert!(required_f64(None, "f").is_err());
    assert!(optional_nonempty(None).unwrap().is_none());
    assert!(optional_nonempty(Some(&Value::Null)).unwrap().is_none());
    assert!(optional_nonempty(Some(&serde_json::json!(" -- ")))
        .unwrap()
        .is_none());
    assert_eq!(
        optional_nonempty(Some(&serde_json::json!(" 华电辽能 ")))
            .unwrap()
            .unwrap()
            .as_str(),
        "华电辽能"
    );
    assert!(optional_nonempty(Some(&serde_json::json!(1))).is_err());
    assert!(china_now().is_ok());
    assert!(unix_seconds_to_china_iso(i64::MAX).is_none());
    assert!(civil_from_days(i64::MAX).is_none());
}

#[test]
fn public_post_close_provider_is_explicitly_unadmitted() {
    let client = EastmoneyClient::with_transport(StaticTransport(fixture()));
    assert!(matches!(
        client.post_close_flows(&request()),
        Err(EastmoneyError::Unsupported(message))
            if message.contains("production admission")
    ));
}

#[test]
fn public_diagnostic_rejects_a_non_current_trading_date_before_transport() {
    let transport = ScriptedTransport::from_results(std::iter::empty());
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);
    let historical_request = PostCloseFlowRequest::new(
        IsoDate::new("2000-01-04").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();

    assert!(matches!(
        client.diagnose_post_close_flows(&historical_request),
        Err(EastmoneyError::InvalidRequest(message))
            if message.contains("current China trading date")
    ));
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn post_close_diagnostic_stops_on_non_transport_failures() {
    let transport = ScriptedTransport::from_results([Err(EastmoneyError::Decode(
        "invalid response encoding".into(),
    ))]);
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);

    assert!(matches!(
        client.diagnose_post_close_flows_with_clock(
            &request(),
            || Ok("2026-07-24T15:35:00+08:00".into())
        ),
        Err(EastmoneyError::Decode(message))
            if message.contains("encoding")
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[test]
fn post_close_diagnostic_exhausts_bounded_transport_failover_explicitly() {
    let transport = ScriptedTransport::from_results(
        (1..=6).map(|attempt| Err(EastmoneyError::Transport(format!("tls-{attempt}")))),
    );
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);

    assert!(matches!(
        client.diagnose_post_close_flows_with_clock(
            &request(),
            || Ok("2026-07-24T15:35:00+08:00".into())
        ),
        Err(EastmoneyError::Transport(message))
            if message.contains("all Eastmoney post-close HTTPS endpoints failed")
                && message.contains("tls-6")
    ));
    assert_eq!(requests.lock().unwrap().len(), 6);
}

#[test]
fn post_close_url_rejects_unregistered_endpoints_and_zero_limits() {
    assert!(matches!(
        post_close_url("https://example.invalid/api", 1),
        Err(EastmoneyError::InvalidRequest(message))
            if message.contains("unregistered")
    ));
    assert!(matches!(
        post_close_url(PRIMARY_ENDPOINT, 0),
        Err(EastmoneyError::InvalidRequest(message))
            if message.contains("zero limit")
    ));
}

#[test]
fn transport_failover_retries_primary_then_uses_one_complete_delay_snapshot() {
    let transport = ScriptedTransport::from_results([
        Err(EastmoneyError::Transport("primary-1".into())),
        Err(EastmoneyError::Transport("primary-2".into())),
        Err(EastmoneyError::Transport("primary-3".into())),
        Ok(fixture()),
    ]);
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);
    let batch = client
        .diagnose_post_close_flows_with_clock(&request(), || Ok("2026-07-24T15:35:00+08:00".into()))
        .unwrap();

    assert_eq!(batch.records().len(), 2);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests[..3]
        .iter()
        .all(|request| request.contains("https://push2.eastmoney.com/")));
    assert!(requests[3].contains("https://push2delay.eastmoney.com/"));
}
