use crate::mapping::{money, non_empty, optional_f64, optional_string, percent, required_string};
use crate::{instrument_from_market, query_url, BatchContext, EastmoneyClient, EastmoneyError};
use magic_market_core::{
    BoardCategory, BoardFlow, BoardFlows, FlowInterval, NonEmptyText, PositiveU32,
};
use serde_json::Value;

const ENDPOINT: &str = "https://push2.eastmoney.com/api/qt/clist/get";

impl BoardFlows for EastmoneyClient {
    type Error = EastmoneyError;

    fn board_flows(
        &self,
        category: BoardCategory,
        interval: FlowInterval,
        limit: PositiveU32,
    ) -> Result<magic_market_core::DataBatch<BoardFlow>, Self::Error> {
        if limit.get() > 200 {
            return Err(EastmoneyError::InvalidRequest(
                "Eastmoney board-flow limit must be at most 200".into(),
            ));
        }
        let filter = match category {
            BoardCategory::Industry => "m:90+t:2",
            BoardCategory::Concept => "m:90+t:3",
            BoardCategory::Region => "m:90+t:1",
            BoardCategory::Unknown => {
                return Err(EastmoneyError::Unsupported(
                    "unknown board category has no Eastmoney filter".into(),
                ))
            }
        };
        let (fid, fields) = match interval {
            FlowInterval::Day1 => ("f62", "f12,f14,f3,f62,f184,f204,f205,f206,f66,f72,f78,f84"),
            FlowInterval::Day5 => ("f164", "f12,f14,f109,f164,f165,f204,f205,f206"),
            FlowInterval::Day10 => ("f174", "f12,f14,f160,f174,f175,f204,f205,f206"),
            other => {
                return Err(EastmoneyError::Unsupported(format!(
                    "board-flow interval {other:?} is not verified"
                )))
            }
        };
        let url = query_url(
            ENDPOINT,
            &[
                ("pn", "1".into()),
                ("pz", limit.get().to_string()),
                ("po", "1".into()),
                ("np", "1".into()),
                ("fltt", "2".into()),
                ("invt", "2".into()),
                ("fid", fid.into()),
                ("fs", filter.into()),
                ("fields", fields.into()),
            ],
        );
        let bytes = self.get(
            &url,
            &[
                ("Accept", "application/json"),
                ("Referer", "https://data.eastmoney.com/"),
            ],
        )?;
        parse_board_flows(&bytes, category, interval)
    }
}

fn parse_board_flows(
    bytes: &[u8],
    category: BoardCategory,
    interval: FlowInterval,
) -> Result<magic_market_core::DataBatch<BoardFlow>, EastmoneyError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    if root.get("rc").and_then(Value::as_i64) != Some(0) {
        return Err(EastmoneyError::Protocol(format!(
            "board-flow endpoint returned rc={}",
            root.get("rc").unwrap_or(&Value::Null)
        )));
    }
    let rows: &[Value] = match root.pointer("/data/diff") {
        Some(Value::Array(rows)) => rows,
        None | Some(Value::Null) if root.get("data").is_none_or(Value::is_null) => &[],
        _ => {
            return Err(EastmoneyError::Protocol(
                "board-flow data.diff is not an array".into(),
            ))
        }
    };
    let context = BatchContext::new("board-flow", None)?;
    let records = rows
        .iter()
        .enumerate()
        .map(|(index, row)| map_board(row, category, interval, index, &context))
        .collect::<Result<Vec<_>, _>>()?;
    context.finish(records)
}

fn map_board(
    row: &Value,
    category: BoardCategory,
    interval: FlowInterval,
    index: usize,
    context: &BatchContext,
) -> Result<BoardFlow, EastmoneyError> {
    let (return_key, main_key) = match interval {
        FlowInterval::Day1 => ("f3", "f62"),
        FlowInterval::Day5 => ("f109", "f164"),
        FlowInterval::Day10 => ("f160", "f174"),
        _ => {
            return Err(EastmoneyError::Unsupported(
                "unverified board-flow interval reached mapper".into(),
            ))
        }
    };
    let first_leader = optional_string(row.get("f204"))?;
    let second_leader = optional_string(row.get("f205"))?;
    let leader_code = [&first_leader, &second_leader]
        .into_iter()
        .flatten()
        .find(|value| value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_digit()))
        .cloned();
    let leader_name = [&first_leader, &second_leader]
        .into_iter()
        .flatten()
        .find(|value| !(value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_digit())))
        .cloned();
    let leader_market = optional_f64(row.get("f206"))?
        .map(|market| {
            if market.fract() != 0.0 {
                return Err(EastmoneyError::Protocol(
                    "board leader market f206 is not integral".into(),
                ));
            }
            Ok(market as i64)
        })
        .transpose()?;
    let leader_instrument = match (leader_code, leader_market) {
        (Some(code), Some(market)) => Some(instrument_from_market(&code, market)?),
        _ => None,
    };
    let (super_large_net, large_net, medium_net, small_net) = if interval == FlowInterval::Day1 {
        (
            money(optional_f64(row.get("f66"))?)?,
            money(optional_f64(row.get("f72"))?)?,
            money(optional_f64(row.get("f78"))?)?,
            money(optional_f64(row.get("f84"))?)?,
        )
    } else {
        (None, None, None, None)
    };
    Ok(BoardFlow {
        board_code: NonEmptyText::new(required_string(row, "f12")?)?,
        board_name: NonEmptyText::new(required_string(row, "f14")?)?,
        category,
        interval,
        rank: PositiveU32::new(
            u32::try_from(index + 1)
                .map_err(|_| EastmoneyError::Protocol("board-flow rank overflow".into()))?,
        )?,
        return_ratio: percent(optional_f64(row.get(return_key))?)?,
        main_net: money(optional_f64(row.get(main_key))?)?,
        super_large_net,
        large_net,
        medium_net,
        small_net,
        leader_instrument,
        leader_name: non_empty(leader_name)?,
        leader_return_ratio: None,
        evidence: context.evidence()?,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_board_flows;
    use magic_market_core::{BoardCategory, FlowInterval, RatioUnit};

    #[test]
    fn maps_daily_board_flow_rank_tiers_and_leader() {
        let fixture = r#"{"rc":0,"data":{"diff":[{
          "f12":"BK1200","f14":"电力设备","f3":4.6,"f62":11167410432,
          "f66":10,"f72":20,"f78":30,"f84":40,
          "f204":"300274","f205":"阳光电源","f206":0
        }]}}"#
            .as_bytes();
        let batch =
            parse_board_flows(fixture, BoardCategory::Industry, FlowInterval::Day1).unwrap();
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
}
