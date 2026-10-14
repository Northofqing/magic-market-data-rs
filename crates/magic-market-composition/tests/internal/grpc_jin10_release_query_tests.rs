use super::*;

struct ReleaseFlashFixture(Vec<u8>);

impl magic_jin10_rs::Jin10Transport for ReleaseFlashFixture {
    fn get(&self, request: &magic_jin10_rs::HttpRequest) -> Result<Vec<u8>, Jin10Error> {
        assert_eq!(
            request.url(),
            "https://flash-api.jin10.com/get_flash_list?channel=-8200&vip=1"
        );
        Ok(self.0.clone())
    }
}

fn release_flash_body() -> serde_json::Value {
    serde_json::json!({"status":200,"message":"OK","data":[{
        "id":"202607250001","time":"2026-07-25 09:30:01","type":1,"important":1,
        "data":{"lock":false,"indicator_id":950,"country":"中国",
            "name":"规模以上工业企业利润","time_period":"6月",
            "pub_time":"2026-07-25 09:30:00","previous":-9.1,
            "consensus":null,"actual":0,"revised":null,"unit":"%","star":3,"affect":1},
        "tags":[]
    }]})
}

fn release_flash_registry(body: serde_json::Value, maximum_bytes: usize) -> OperationRegistry {
    let client =
        Jin10Client::with_transport(ReleaseFlashFixture(serde_json::to_vec(&body).unwrap()));
    let mut registry = OperationRegistry::all_unadmitted("offline fixture has no other operations");
    registry
        .register_handler(
            admitted(
                Operation::EconomicReleaseObservations,
                "Jin10",
                "current public flash window",
            ),
            move |command| execute_economic_release_observations(command, &client, maximum_bytes),
        )
        .unwrap();
    registry
}

fn release_flash_command(country: &str) -> QueryCommand {
    QueryCommand::new(
        "release-query-fixture",
        Operation::EconomicReleaseObservations,
        Some("Jin10".into()),
        CanonicalPayload::new(
            "magic.market.economic_release_observations.request",
            1,
            serde_json::to_vec(&serde_json::json!({"limit":20,"country":country})).unwrap(),
            65_536,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn jin10_public_release_query_keeps_zero_actual_and_original_release_evidence() {
    let result = release_flash_registry(release_flash_body(), 65_536)
        .execute(release_flash_command("中国"))
        .unwrap();
    assert_eq!(result.provider, "Jin10");
    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-25 09:30:01"));
    assert!(!result.batch_id.is_empty());
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(
        payload.schema(),
        "magic.market.economic_release_observation"
    );
    assert_eq!(payload.schema_version(), 1);
    let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(row["event_id"], "202607250001");
    assert_eq!(row["indicator_id"], 950);
    assert_eq!(row["country"], "中国");
    assert_eq!(row["name"], "规模以上工业企业利润");
    assert_eq!(row["period"], "6月");
    assert_eq!(row["scheduled_at"], "2026-07-25T09:30:00+08:00");
    assert_eq!(row["released_at"], "2026-07-25T09:30:01+08:00");
    assert_eq!(row["actual"], "0");
    assert_eq!(row["previous"], "-9.1");
    assert!(row["consensus"].is_null());
    assert!(row["revised"].is_null());
    assert_eq!(row["unit"], "%");
    assert_eq!(row["importance"], 3);
    assert_eq!(row["impact"], "1");
    assert_eq!(row["evidence"]["provider"], "Jin10");
    assert_eq!(row["evidence"]["source_at"], "2026-07-25 09:30:01");
    assert_eq!(row["evidence"]["observed_at"], result.observed_at);
    assert_eq!(row["evidence"]["batch_id"], result.batch_id);
    EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
}

#[test]
fn jin10_public_release_query_rejects_a_malformed_eligible_row_atomically() {
    let mut body = release_flash_body();
    let mut malformed = body["data"][0].clone();
    malformed["id"] = "202607250002".into();
    malformed["time"] = "2026-02-30 09:29:00".into();
    body["data"].as_array_mut().unwrap().push(malformed);
    let result = release_flash_registry(body, 65_536).execute(release_flash_command("中国"));
    assert!(matches!(
        result,
        Err(ServiceError::FailedPrecondition(message)) if message.contains("out of range")
    ));
}

#[test]
fn jin10_public_release_query_returns_verified_empty_without_a_source_time() {
    let result = release_flash_registry(release_flash_body(), 65_536)
        .execute(release_flash_command("美国"))
        .unwrap();
    assert_eq!(result.provider, "Jin10");
    assert!(result.repository_admitted);
    assert!(result.complete);
    assert!(result.records.is_empty());
    assert_eq!(result.source_at, None);
    assert_eq!(result.diagnostic_blocker, None);
    assert!(!result.batch_id.is_empty());
    EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
}
