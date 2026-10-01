use super::{parse_fund_flow, parse_number, parse_row};
use crate::test_support::ScriptedTransport;
use crate::{EastmoneyClient, EastmoneyError};
use magic_market_core::{
    AssetClass, BoardCategory, Exchange, FlowInterval, FlowScope, FundFlowRequest, FundFlowSeries,
    InstrumentId, NonEmptyText, PositiveU32, ProviderId, RatioUnit,
};

fn scope() -> FlowScope {
    FlowScope::Instrument(
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
    )
}

// Exact public source bytes captured from the first-party daykline endpoint.
// See docs/integrations/research/2026-10-02-eastmoney-native-flow-request-controls.md.
const NATIVE_DAY1: &[u8] = include_bytes!("../fixtures/fund_flow_day1_native.jsonp");

#[test]
fn public_daily_fund_flow_replays_native_jsonp_with_exact_identity_and_cny_values() {
    let expected_scope = FlowScope::Instrument(
        InstrumentId::new(Exchange::Shenzhen, "300005", AssetClass::Equity).unwrap(),
    );
    let transport = ScriptedTransport::from_bodies([NATIVE_DAY1]);
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);
    let request = FundFlowRequest::new(
        expected_scope.clone(),
        FlowInterval::Day1,
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    let batch = client.fund_flow_series(&request).unwrap();
    assert_eq!(batch.records().len(), 2);
    for (point, date, main, small, medium, large, super_large, ratio) in [
        (
            &batch.records()[0],
            "2026-09-29",
            -6_489_227.0,
            -21_764_504.0,
            28_253_731.0,
            -1_794_214.0,
            -4_695_013.0,
            -2.51,
        ),
        (
            &batch.records()[1],
            "2026-09-30",
            4_930_843.0,
            -67_617_987.0,
            62_687_144.0,
            176_755.0,
            4_754_088.0,
            1.66,
        ),
    ] {
        assert_eq!(point.scope, expected_scope);
        assert_eq!(point.interval, FlowInterval::Day1);
        assert_eq!(point.period_at.as_str(), date);
        assert_eq!(point.main_net.unwrap().get(), main);
        assert_eq!(point.small_net.unwrap().get(), small);
        assert_eq!(point.medium_net.unwrap().get(), medium);
        assert_eq!(point.large_net.unwrap().get(), large);
        assert_eq!(point.super_large_net.unwrap().get(), super_large);
        assert_eq!(point.main_ratio.unwrap().get(), ratio);
        assert_eq!(point.main_ratio.unwrap().unit(), RatioUnit::Percent);
        assert_eq!(point.evidence.provider(), ProviderId::Eastmoney);
        assert_eq!(point.evidence.source_at(), Some(date));
        assert_eq!(
            point.evidence.batch_id(),
            batch.provenance().batch_id().unwrap()
        );
        assert_eq!(
            point.evidence.observed_at(),
            batch.provenance().fetched_at()
        );
    }
    assert_eq!(batch.provenance().source_at(), Some("2026-09-30"));
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        ["GET https://push2his.eastmoney.com/api/qt/stock/fflow/daykline/get?secid=0.300005&klt=101&lmt=2&fields1=f1%2Cf2%2Cf3%2Cf7&fields2=f51%2Cf52%2Cf53%2Cf54%2Cf55%2Cf56%2Cf57%2Cf58%2Cf59%2Cf60%2Cf61%2Cf62%2Cf63%2Cf64%2Cf65&ut=b2884a393a59ad64002292a3e90d46a5&cb=emProbe"]
    );
}

#[test]
fn public_daily_fund_flow_rejects_scripts_and_invalid_source_payloads() {
    let request =
        FundFlowRequest::new(scope(), FlowInterval::Day1, PositiveU32::new(1).unwrap()).unwrap();
    for body in [
        &b"other({\"rc\":0});"[..],
        &b"emProbe ({\"rc\":0});"[..],
        &b"emProbe({\"rc\":0});alert(1);"[..],
        &b"emProbe({\"rc\":0});emProbe({\"rc\":0});"[..],
        &b"emProbe(/*comment*/{\"rc\":0});"[..],
        &b"emProbe({);"[..],
        &b"emProbe([]);"[..],
        &b"emProbe(null);"[..],
        &b"emProbe(\xff);"[..],
        &br#"{"rc":0,"data":{"code":"600396","market":1,"klines":["2026-07-23,1,2,3,4,5"]}}"#[..],
        &br#"emProbe({"rc":1,"data":{"code":"600396","market":1,"klines":["2026-07-23,1,2,3,4,5"]}});"#[..],
        &br#"emProbe({"rc":0,"data":{"code":"600396","market":0,"klines":["2026-07-23,1,2,3,4,5"]}});"#[..],
        &br#"emProbe({"rc":0,"data":{"code":"600519","market":1,"klines":["2026-07-23,1,2,3,4,5"]}});"#[..],
        &br#"emProbe({"rc":0,"data":{"code":"600396","market":1,"klines":["2026-02-30,1,2,3,4,5"]}});"#[..],
        &br#"emProbe({"rc":0,"data":{"code":"600396","market":1,"klines":["2026-07-23,NaN,2,3,4,5"]}});"#[..],
        &br#"emProbe({"rc":0,"data":null});"#[..],
    ] {
        let client = EastmoneyClient::with_transport(ScriptedTransport::from_bodies([body]));
        assert!(client.fund_flow_series(&request).is_err(), "{body:?}");
    }
}

#[test]
fn public_daily_fund_flow_accepts_only_outside_ascii_whitespace() {
    let body = concat!(
        " \t\n",
        r#"emProbe({"rc":0,"data":{"code":"600396","market":1,"klines":["2026-07-23,1,2,3,4,5"]}});"#,
        "\n "
    )
    .as_bytes();
    let client = EastmoneyClient::with_transport(ScriptedTransport::from_bodies([body]));
    let request =
        FundFlowRequest::new(scope(), FlowInterval::Day1, PositiveU32::new(1).unwrap()).unwrap();
    assert_eq!(
        client.fund_flow_series(&request).unwrap().records().len(),
        1
    );
}

#[test]
fn maps_minute_tier_fields_without_unit_coercion() {
    let fixture = br#"{"rc":0,"data":{"klines":[
      "2026-07-23 15:00,100.5,-10,20,30,60,1.25"
    ],"code":"600396","market":1}}"#;
    let batch = parse_fund_flow(fixture, scope(), FlowInterval::Minute1).unwrap();
    let point = &batch.records()[0];
    assert_eq!(point.period_at.as_str(), "2026-07-23 15:00");
    assert_eq!(point.main_net.unwrap().get(), 100.5);
    assert_eq!(point.small_net.unwrap().get(), -10.0);
    assert_eq!(point.medium_net.unwrap().get(), 20.0);
    assert_eq!(point.large_net.unwrap().get(), 30.0);
    assert_eq!(point.super_large_net.unwrap().get(), 60.0);
    assert_eq!(point.main_ratio.unwrap().get(), 1.25);
    assert_eq!(point.main_ratio.unwrap().unit(), RatioUnit::Percent);
    assert_eq!(
        point.evidence.source_at(),
        Some("2026-07-23T15:00:00+08:00")
    );
}

#[test]
fn maps_daily_period_with_a_strict_calendar_date() {
    let fixture = br#"{"rc":0,"data":{"klines":[
      "2026-07-23,100.5,-10,20,30,60,1.25"
    ],"code":"600396","market":1}}"#;
    let batch = parse_fund_flow(fixture, scope(), FlowInterval::Day1).unwrap();
    assert_eq!(batch.records()[0].period_at.as_str(), "2026-07-23");
    assert_eq!(batch.records()[0].evidence.source_at(), Some("2026-07-23"));
}

#[test]
fn null_data_bad_rows_and_nonzero_rc_fail() {
    assert!(parse_fund_flow(br#"{"rc":0,"data":null}"#, scope(), FlowInterval::Minute1).is_err());
    assert!(parse_fund_flow(br#"{"rc":1,"data":null}"#, scope(), FlowInterval::Minute1).is_err());
    assert!(parse_fund_flow(
        br#"{"rc":0,"data":{"klines":["bad"]}}"#,
        scope(),
        FlowInterval::Minute1
    )
    .is_err());
}

#[test]
fn source_market_and_code_must_match_requested_scope() {
    let mismatched = br#"{"rc":0,"data":{
      "code":"002475","market":1,
      "klines":["2026-07-23,100,-10,20,30,60,1.25"]
    }}"#;
    assert!(parse_fund_flow(mismatched, scope(), FlowInterval::Day1).is_err());
}

#[test]
fn period_at_rejects_malformed_or_impossible_date_and_time() {
    for (interval, period_at) in [
        (FlowInterval::Day1, "2026-02-30"),
        (FlowInterval::Day1, "2026-07-23 15:00"),
        (FlowInterval::Day1, "20260723"),
        (FlowInterval::Minute1, "2026-02-30 15:00"),
        (FlowInterval::Minute1, "2026-07-23"),
        (FlowInterval::Minute1, "2026-07-23T15:00"),
        (FlowInterval::Minute1, "2026-07-23 24:00"),
        (FlowInterval::Minute1, "2026-07-23 15:60"),
        (FlowInterval::Minute1, "2026-07-23 15:00:00"),
    ] {
        let fixture = format!(
            r#"{{"rc":0,"data":{{
              "code":"600396","market":1,
              "klines":["{period_at},100,-10,20,30,60,1.25"]
            }}}}"#
        );
        assert!(
            parse_fund_flow(fixture.as_bytes(), scope(), interval).is_err(),
            "{interval:?} {period_at}"
        );
    }
}

#[test]
fn public_fund_flow_contract_routes_minute_and_daily_source_shapes() {
    for (interval, body, expected_klt, expected_endpoint) in [
        (
            FlowInterval::Minute1,
            &br#"{"rc":0,"data":{"klines":[
              "2026-07-23 15:00,100,-10,20,30,60,1.25"
            ],"code":"600396","market":1}}"#[..],
            "klt=1",
            "GET https://push2.eastmoney.com/api/qt/stock/fflow/kline/get?",
        ),
        (
            FlowInterval::Day1,
            &br#"emProbe({"rc":0,"data":{"klines":[
              "2026-07-23,100,-10,20,30,60,1.25"
            ],"code":"600396","market":1}});"#[..],
            "klt=101",
            "GET https://push2his.eastmoney.com/api/qt/stock/fflow/daykline/get?",
        ),
    ] {
        let transport = ScriptedTransport::from_bodies([body]);
        let requests = transport.requests();
        let client = EastmoneyClient::with_transport(transport);
        let request =
            FundFlowRequest::new(scope(), interval, PositiveU32::new(1).unwrap()).unwrap();
        let batch = client.fund_flow_series(&request).unwrap();
        assert_eq!(batch.records().len(), 1);
        assert!(
            requests.lock().unwrap()[0].contains(expected_klt),
            "{:?}",
            requests.lock().unwrap()
        );
        assert!(
            requests.lock().unwrap()[0].starts_with(expected_endpoint),
            "expected endpoint {expected_endpoint}, requests={:?}",
            requests.lock().unwrap()
        );
        if interval == FlowInterval::Minute1 {
            assert!(!requests.lock().unwrap()[0].contains("&ut="));
            assert!(!requests.lock().unwrap()[0].contains("&cb="));
        }
    }
}

#[test]
fn public_fund_flow_contract_rejects_board_and_unverified_intervals() {
    let client = EastmoneyClient::with_transport(ScriptedTransport::from_bodies([]));
    let board_request = FundFlowRequest::new(
        FlowScope::Board {
            code: NonEmptyText::new("BK1200").unwrap(),
            name: NonEmptyText::new("电力设备").unwrap(),
            category: BoardCategory::Industry,
        },
        FlowInterval::Day1,
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        client.fund_flow_series(&board_request),
        Err(EastmoneyError::Unsupported(_))
    ));
    let interval_request =
        FundFlowRequest::new(scope(), FlowInterval::Day5, PositiveU32::new(1).unwrap()).unwrap();
    assert!(matches!(
        client.fund_flow_series(&interval_request),
        Err(EastmoneyError::Unsupported(_))
    ));
}

#[test]
fn fund_flow_protocol_shape_and_number_failures_are_explicit() {
    let board_scope = FlowScope::Board {
        code: NonEmptyText::new("BK1200").unwrap(),
        name: NonEmptyText::new("电力设备").unwrap(),
        category: BoardCategory::Industry,
    };
    assert!(matches!(
        parse_fund_flow(b"{", scope(), FlowInterval::Day1),
        Err(EastmoneyError::Decode(_))
    ));
    assert!(matches!(
        parse_fund_flow(
            br#"{"rc":0,"data":{"code":"600396","market":1,"klines":[]}}"#,
            board_scope,
            FlowInterval::Day1
        ),
        Err(EastmoneyError::Unsupported(_))
    ));
    for fixture in [
        r#"{"rc":0,"data":{"market":1,"klines":[]}}"#,
        r#"{"rc":0,"data":{"code":"600396","klines":[]}}"#,
        r#"{"rc":0,"data":{"code":"600396","market":1.5,"klines":[]}}"#,
        r#"{"rc":0,"data":{"code":"600396","market":1,"klines":{}}}"#,
        r#"{"rc":0,"data":{"code":"600396","market":1,"klines":[1]}}"#,
    ] {
        assert!(
            parse_fund_flow(fixture.as_bytes(), scope(), FlowInterval::Day1).is_err(),
            "{fixture}"
        );
    }
    for row in [
        "",
        "2026-07-23,1,2,3,4",
        "2026-07-23,nope,2,3,4,5",
        "2026-07-23,NaN,2,3,4,5",
    ] {
        assert!(parse_row(row, FlowInterval::Day1).is_err(), "{row}");
    }
    assert!(matches!(
        parse_row("2026-07-23,1,2,3,4,5", FlowInterval::Day120),
        Err(EastmoneyError::Unsupported(_))
    ));
    assert_eq!(parse_number(" -- ").unwrap(), None);
    assert_eq!(parse_number(" - ").unwrap(), None);
}
