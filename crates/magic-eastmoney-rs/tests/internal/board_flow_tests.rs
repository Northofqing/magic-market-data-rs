use super::{map_board, parse_board_flows};
use crate::test_support::ScriptedTransport;
use crate::{BatchContext, EastmoneyClient, EastmoneyError};
use magic_market_core::{BoardCategory, BoardFlows, FlowInterval, PositiveU32, RatioUnit};
use serde_json::json;

#[test]
fn maps_daily_board_flow_rank_tiers_leader_and_source_time() {
    let fixture = r#"{"rc":0,"data":{"diff":[{
      "f12":"BK1200","f14":"电力设备","f3":4.6,"f62":11167410432,
      "f66":10,"f72":20,"f78":30,"f84":40,
      "f204":"300274","f205":"阳光电源","f206":0,
      "f124":1784789940
    }]}}"#
        .as_bytes();
    let batch = parse_board_flows(fixture, BoardCategory::Industry, FlowInterval::Day1).unwrap();
    let row = &batch.records()[0];
    assert_eq!(row.board_code.as_str(), "BK1200");
    assert_eq!(row.board_name.as_str(), "电力设备");
    assert_eq!(row.category, BoardCategory::Industry);
    assert_eq!(row.interval, FlowInterval::Day1);
    assert_eq!(row.rank.get(), 1);
    assert_eq!(row.return_ratio.unwrap().get(), 4.6);
    assert_eq!(row.return_ratio.unwrap().unit(), RatioUnit::Percent);
    assert_eq!(row.main_net.unwrap().get(), 11167410432.0);
    assert_eq!(row.super_large_net.unwrap().get(), 10.0);
    assert_eq!(row.large_net.unwrap().get(), 20.0);
    assert_eq!(row.medium_net.unwrap().get(), 30.0);
    assert_eq!(row.small_net.unwrap().get(), 40.0);
    assert_eq!(row.leader_instrument.as_ref().unwrap().code(), "300274");
    assert_eq!(row.leader_name.as_ref().unwrap().as_str(), "阳光电源");
    assert!(row.leader_return_ratio.is_none());
    assert_eq!(row.evidence.source_at(), Some("1784789940"));
    assert_eq!(batch.provenance().source_at(), Some("1784789940"));
}

#[test]
fn malformed_shapes_and_null_data_fail() {
    assert!(parse_board_flows(
        br#"{"rc":0,"data":null}"#,
        BoardCategory::Concept,
        FlowInterval::Day5
    )
    .is_err());
    assert!(parse_board_flows(
        br#"{"rc":0,"data":{"diff":{}}}"#,
        BoardCategory::Region,
        FlowInterval::Day10
    )
    .is_err());
}

#[test]
fn missing_invalid_or_non_atomic_source_time_fails() {
    for fixture in [
        r#"{"rc":0,"data":{"diff":[{"f12":"BK1","f14":"A","f3":1,"f62":1}]}}"#,
        r#"{"rc":0,"data":{"diff":[{"f12":"BK1","f14":"A","f3":1,"f62":1,"f124":0}]}}"#,
        r#"{"rc":0,"data":{"diff":[
          {"f12":"BK1","f14":"A","f3":1,"f62":1,"f124":1784789940},
          {"f12":"BK2","f14":"B","f3":2,"f62":2,"f124":1784789941}
        ]}}"#,
    ] {
        assert!(parse_board_flows(
            fixture.as_bytes(),
            BoardCategory::Industry,
            FlowInterval::Day1
        )
        .is_err());
    }
}

#[test]
fn public_board_flow_contract_routes_every_verified_category_and_interval() {
    const FIXTURE: &str = r#"{"rc":0,"data":{"diff":[{
      "f12":"BK1200","f14":"电力设备",
      "f3":4.6,"f62":111,"f109":5.6,"f164":222,"f160":6.6,"f174":333,
      "f204":"300274","f205":"阳光电源","f206":0,"f124":1784789940
    }]}}"#;
    for (category, expected_filter) in [
        (BoardCategory::Industry, "fs=m%3A90%2Bt%3A2"),
        (BoardCategory::Concept, "fs=m%3A90%2Bt%3A3"),
        (BoardCategory::Region, "fs=m%3A90%2Bt%3A1"),
    ] {
        for (interval, expected_fid, expected_main) in [
            (FlowInterval::Day1, "fid=f62", 111.0),
            (FlowInterval::Day5, "fid=f164", 222.0),
            (FlowInterval::Day10, "fid=f174", 333.0),
        ] {
            let transport = ScriptedTransport::from_bodies([FIXTURE.as_bytes()]);
            let requests = transport.requests();
            let client = EastmoneyClient::with_transport(transport);
            let batch = client
                .board_flows(category, interval, PositiveU32::new(1).unwrap())
                .unwrap();
            assert_eq!(batch.records()[0].main_net.unwrap().get(), expected_main);
            let request = requests.lock().unwrap()[0].clone();
            assert!(request.contains(expected_filter), "{request}");
            assert!(request.contains(expected_fid), "{request}");
        }
    }
}

#[test]
fn public_board_flow_contract_rejects_unverified_requests_before_transport() {
    let client = EastmoneyClient::with_transport(ScriptedTransport::from_bodies([]));
    assert!(matches!(
        client.board_flows(
            BoardCategory::Industry,
            FlowInterval::Day1,
            PositiveU32::new(201).unwrap()
        ),
        Err(EastmoneyError::InvalidRequest(_))
    ));
    assert!(matches!(
        client.board_flows(
            BoardCategory::Unknown,
            FlowInterval::Day1,
            PositiveU32::new(1).unwrap()
        ),
        Err(EastmoneyError::Unsupported(_))
    ));
    assert!(matches!(
        client.board_flows(
            BoardCategory::Industry,
            FlowInterval::Day120,
            PositiveU32::new(1).unwrap()
        ),
        Err(EastmoneyError::Unsupported(_))
    ));
}

#[test]
fn board_flow_decode_mapper_and_leader_market_failures_are_explicit() {
    assert!(matches!(
        parse_board_flows(b"{", BoardCategory::Industry, FlowInterval::Day1),
        Err(EastmoneyError::Decode(_))
    ));
    assert!(matches!(
        parse_board_flows(
            br#"{"rc":7,"data":null}"#,
            BoardCategory::Industry,
            FlowInterval::Day1
        ),
        Err(EastmoneyError::Protocol(_))
    ));
    assert!(matches!(
        parse_board_flows(
            br#"{"rc":0,"data":{"diff":[{
              "f12":"BK1","f14":"A","f3":1,"f62":1,
              "f204":"300274","f206":0.5,"f124":1
            }]}}"#,
            BoardCategory::Industry,
            FlowInterval::Day1
        ),
        Err(EastmoneyError::Protocol(_))
    ));
    let context = BatchContext::new("board-flow", Some("1")).unwrap();
    assert!(matches!(
        map_board(
            &json!({"f12":"BK1","f14":"A","f124":1}),
            BoardCategory::Industry,
            FlowInterval::Day120,
            0,
            &context
        ),
        Err(EastmoneyError::Unsupported(_))
    ));
}
