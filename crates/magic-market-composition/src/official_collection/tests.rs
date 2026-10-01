use super::*;
use magic_market_service::Capability;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct FixtureGateway {
    calls: AtomicUsize,
}
impl BlockingQueryGateway for FixtureGateway {
    fn capabilities(&self) -> Vec<Capability> {
        Vec::new()
    }
    fn execute(&self, command: QueryCommand) -> Result<QueryResult, ServiceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let source = command.preferred_provider().unwrap();
        if source == "Nbs"
            || (source == "Pbc" && command.operation() == Operation::OfficialPublication)
        {
            return Err(ServiceError::Unavailable {
                operation: command.operation(),
                reason: "fixture outage".into(),
            });
        }
        let observed_at = "2026-10-01T01:00:00Z";
        let url = format!(
            "https://{}.example.test/original.html",
            source.to_lowercase()
        );
        let (schema, value) = if command.operation() == Operation::OfficialPublications {
            assert_eq!(
                command.payload().schema(),
                OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA
            );
            (
                OFFICIAL_PUBLICATION_LISTING_SCHEMA,
                json!({
                    "source":source, "observed_at":observed_at, "source_rows":2,
                    "response_sha256":"fixture-response-hash", "response_url":"https://fixture.example.test/list",
                    "items":[{"source":source,"canonical_url":url,"publication_label":"2026-09-30",
                        "precision":"Date","publication_label_origin":"ListingHtml"}],
                }),
            )
        } else {
            assert_eq!(
                command.payload().schema(),
                OFFICIAL_PUBLICATION_REQUEST_SCHEMA
            );
            let request: Value = serde_json::from_slice(command.payload().data()).unwrap();
            assert_eq!(request["url"], url);
            (
                OFFICIAL_PUBLICATION_SCHEMA,
                json!({"source":source,"observed_at":observed_at,
                "canonical_url":url,"precision":"Date","publication_label":"2026-09-30","content":"正文 0"}),
            )
        };
        Ok(QueryResult {
            provider: source.into(),
            batch_id: format!("fixture-{}", command.request_id()),
            complete: true,
            observed_at: observed_at.into(),
            source_at: None,
            records: vec![CanonicalPayload::new(
                schema,
                1,
                serde_json::to_vec(&value).unwrap(),
                4096,
            )
            .unwrap()],
            repository_admitted: true,
            diagnostic_blocker: None,
        })
    }
}

#[test]
fn collector_journals_source_and_original_failures_without_erasing_successes() {
    let gateway = FixtureGateway::default();
    let mut journal = Vec::new();
    let summary = collect_official_publications_round(&gateway, 1, 1, 4096, &mut journal).unwrap();
    assert_eq!(
        summary,
        OfficialCollectionSummary {
            attempted: 15,
            succeeded: 13,
            failed: 2
        }
    );
    let events = String::from_utf8(journal)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 15);
    assert_eq!(events[0]["provider"], "Nbs");
    assert_eq!(events[0]["state"], "failed");
    assert_eq!(events[1]["provider"], "Pbc");
    assert_eq!(events[1]["state"], "success");
    assert_eq!(events[1]["record"]["data"]["source_rows"], 2);
    assert_eq!(
        events[1]["record"]["data"]["response_sha256"],
        "fixture-response-hash"
    );
    assert_eq!(events[2]["provider"], "Pbc");
    assert_eq!(events[2]["state"], "failed");
    assert!(events.iter().all(|event| event["provider"] != "Gacc"));
    assert!(events
        .iter()
        .filter(|event| event["state"] == "success")
        .all(|event| event["source_at"].is_null()));
}

#[test]
fn collector_preserves_repeated_observations_with_distinct_rounds() {
    let gateway = FixtureGateway::default();
    let mut journal = Vec::new();
    for round in [1, 2] {
        collect_official_publications_round(&gateway, round, 1, 4096, &mut journal).unwrap();
    }
    let events = String::from_utf8(journal)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 30);
    assert_ne!(events[1]["request_id"], events[16]["request_id"]);
    assert_eq!(events[1]["record"], events[16]["record"]);
    assert_eq!(events[1]["round"], 1);
    assert_eq!(events[16]["round"], 2);
}

#[test]
fn collector_rejects_bounds_before_queries_and_stops_after_journal_failure() {
    let gateway = FixtureGateway::default();
    for (round, limit, maximum) in [(0, 1, 4096), (1, 0, 4096), (1, 21, 4096), (1, 1, 0)] {
        assert!(collect_official_publications_round(
            &gateway,
            round,
            limit,
            maximum,
            &mut Vec::new()
        )
        .is_err());
    }
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 0);
    struct FailedWriter;
    impl Write for FailedWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(collect_official_publications_round(&gateway, 1, 1, 4096, &mut FailedWriter).is_err());
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 1);
}
