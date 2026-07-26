use super::{parse_fund_flow, parse_number, parse_row};
use crate::test_support::ScriptedTransport;
use crate::{EastmoneyClient, EastmoneyError};
use magic_market_core::{
    AssetClass, BoardCategory, Exchange, FlowInterval, FlowScope, FundFlowRequest, FundFlowSeries,
    InstrumentId, NonEmptyText, PositiveU32, RatioUnit,
};

fn scope() -> FlowScope {
    FlowScope::Instrument(
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
    )
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
    assert_eq!(point.evidence.source_at(), Some("2026-07-23 15:00"));
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
    for (interval, body, expected_klt) in [
        (
            FlowInterval::Minute1,
            &br#"{"rc":0,"data":{"klines":[
              "2026-07-23 15:00,100,-10,20,30,60,1.25"
            ],"code":"600396","market":1}}"#[..],
            "klt=1",
        ),
        (
            FlowInterval::Day1,
            &br#"{"rc":0,"data":{"klines":[
              "2026-07-23,100,-10,20,30,60,1.25"
            ],"code":"600396","market":1}}"#[..],
            "klt=101",
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
