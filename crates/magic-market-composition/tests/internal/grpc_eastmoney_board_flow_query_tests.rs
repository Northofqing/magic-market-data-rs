use super::register_eastmoney_client;
use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::{BoardCategory, BoardFlow, FlowInterval, ProviderId, RatioUnit};
use magic_market_service::{CanonicalPayload, Operation, OperationRegistry, QueryCommand};
use std::time::{SystemTime, UNIX_EPOCH};

// Synthetic repository-contract specimen, not a native capture or admission
// witness. Daily/ten-day fields and daily size tiers are deliberate decoys.
const SOURCE_CONCEPT_DAY5: &[u8] = br#"{"rc":0,"data":{"diff":[{
    "f12":"BK1200","f14":"Offline Concept",
    "f3":4.6,"f62":111.0,"f109":-5.625,"f164":-222000000.25,
    "f160":6.6,"f174":333.0,"f165":5.125,
    "f66":10.0,"f72":20.0,"f78":30.0,"f84":40.0,
    "f204":"300274","f205":"Offline Leader","f206":0,
    "f124":1784789940
}]}}"#;

// Independent complete typed-wire oracle. Only bounded local observation and
// its independently checked batch identity are filled after execute returns.
const EXPECTED_CONCEPT_DAY5: &str = r#"{
    "board_code":"BK1200","board_name":"Offline Concept",
    "category":"Concept","interval":"Day5","rank":1,
    "return_ratio":{"value":-5.625,"unit":"Percent"},
    "main_net":-222000000.25,
    "super_large_net":null,"large_net":null,"medium_net":null,"small_net":null,
    "leader_instrument":{"exchange":"Shenzhen","code":"300274","asset_class":"Equity"},
    "leader_name":"Offline Leader","leader_return_ratio":null,
    "evidence":{"provider":"Eastmoney","source_at":"1784789940",
                "observed_at":"$LOCAL_OBSERVATION","batch_id":"$LOCAL_BATCH"}
}"#;

const EXPECTED_CONCEPT_DAY5_URL: &str = "https://push2.eastmoney.com/api/qt/clist/get?pn=1&pz=1&po=1&np=1&fltt=2&invt=2&fid=f164&fs=m%3A90%2Bt%3A3&fields=f12%2Cf14%2Cf109%2Cf164%2Cf165%2Cf204%2Cf205%2Cf206%2Cf124";

struct ConceptDay5SourceFixture;

impl EastmoneyTransport for ConceptDay5SourceFixture {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        assert_eq!(url, EXPECTED_CONCEPT_DAY5_URL);
        assert_eq!(
            headers,
            &[
                ("Accept", "application/json"),
                ("Referer", "https://data.eastmoney.com/"),
            ]
        );
        assert_eq!(maximum_bytes, 4 * 1024 * 1024);
        assert!(SOURCE_CONCEPT_DAY5.len() <= maximum_bytes);
        Ok(SOURCE_CONCEPT_DAY5.to_vec())
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Protocol(
            "offline Day5 concept fixture does not serve POST".into(),
        ))
    }
}

#[test]
fn registered_concept_day5_board_flow_preserves_source_fields_units_and_typed_wire() {
    let client = EastmoneyClient::with_transport(ConceptDay5SourceFixture);
    let mut registry = OperationRegistry::all_unadmitted("offline external source only");
    // Actual production setup only. The assertion boundary is public execute;
    // no provider trait, parser, handler, registry or result is replaced.
    register_eastmoney_client(&mut registry, client, 4096).unwrap();
    let command = QueryCommand::new(
        "offline-eastmoney-concept-day5-board-flow-v1",
        Operation::BoardFlows,
        Some("Eastmoney".into()),
        CanonicalPayload::new(
            "magic.market.board_flows.request",
            1,
            br#"{"category":"Concept","interval":"Day5","limit":1}"#.to_vec(),
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
    assert_eq!(result.source_at.as_deref(), Some("1784789940"));
    assert_ne!(
        result.source_at.as_deref(),
        Some(result.observed_at.as_str())
    );
    assert_eq!(
        result.batch_id,
        format!("eastmoney-web:board-flow:{}", result.observed_at)
    );
    assert_eq!(result.records.len(), 1);

    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.board_flow");
    assert_eq!(payload.schema_version(), 1);
    assert!(payload.data().len() <= 4096);
    let row: BoardFlow = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(row.category, BoardCategory::Concept);
    assert_eq!(row.interval, FlowInterval::Day5);
    assert_eq!(row.return_ratio.unwrap().get(), -5.625);
    assert_eq!(row.return_ratio.unwrap().unit(), RatioUnit::Percent);
    assert_eq!(row.main_net.unwrap().get(), -222_000_000.25);
    assert_eq!(row.evidence.provider(), ProviderId::Eastmoney);
    assert_eq!(row.evidence.source_at(), Some("1784789940"));
    assert_eq!(row.evidence.observed_at(), result.observed_at);
    assert_eq!(row.evidence.batch_id(), result.batch_id);

    let mut expected: serde_json::Value = serde_json::from_str(EXPECTED_CONCEPT_DAY5).unwrap();
    expected["evidence"]["observed_at"] = serde_json::Value::String(result.observed_at.clone());
    expected["evidence"]["batch_id"] = serde_json::Value::String(result.batch_id.clone());
    let actual: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(actual, expected);
}
