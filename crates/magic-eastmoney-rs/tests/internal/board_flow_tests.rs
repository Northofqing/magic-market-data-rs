use super::parse_board_flows;
use magic_market_core::{BoardCategory, FlowInterval, RatioUnit};

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
