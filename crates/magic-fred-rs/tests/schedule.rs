use magic_fred_rs::FredClient;
use magic_market_core::{
    EconomicReleaseScheduleProvider, EconomicReleaseScheduleRequest, IsoDate, PositiveU32,
    ProviderId, SourcedRecord,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use std::sync::{Arc, Mutex};

const SCHEDULE: &[u8] = br#"{
  "realtime_start":"2026-09-01",
  "realtime_end":"2026-09-30",
  "order_by":"release_date",
  "sort_order":"asc",
  "count":2,
  "offset":0,
  "limit":1000,
  "release_dates":[
    {"release_id":10,"release_name":"Consumer Price Index","date":"2026-09-15","release_last_updated":"2026-08-01 09:30:00-05"},
    {"release_id":50,"release_name":"Employment Situation","date":"2026-09-20","release_last_updated":"2026-08-02 09:30:00-05"}
  ]
}"#;

#[derive(Clone)]
struct FixtureTransport {
    urls: Arc<Mutex<Vec<String>>>,
}

impl HttpTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.urls.lock().unwrap().push(request.url().to_owned());
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            SCHEDULE.to_vec(),
        ))
    }
}

#[test]
fn probe_returns_checked_date_only_entries_through_the_public_seam() {
    fn assert_provider<T: EconomicReleaseScheduleProvider>() {}
    assert_provider::<FredClient>();

    let urls = Arc::new(Mutex::new(Vec::new()));
    let client = FredClient::with_transport(
        "fixture-key",
        Arc::new(FixtureTransport {
            urls: Arc::clone(&urls),
        }),
    )
    .unwrap();
    let request = EconomicReleaseScheduleRequest::new(
        IsoDate::new("2026-09-01").unwrap(),
        IsoDate::new("2026-09-30").unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();

    let batch = client.probe_economic_release_schedule(&request).unwrap();
    assert!(batch.quality().is_complete());
    assert_eq!(batch.records().len(), 1);
    let entry = &batch.records()[0];
    assert_eq!(entry.release_id().get(), 10);
    assert_eq!(entry.release_name().as_str(), "Consumer Price Index");
    assert_eq!(entry.release_date().as_str(), "2026-09-15");
    assert_eq!(entry.provider_id(), ProviderId::Fred);
    assert_eq!(entry.evidence_source_at(), None);
    assert_eq!(batch.provenance().source_at(), None);

    assert_eq!(
        urls.lock().unwrap().as_slice(),
        &["https://api.stlouisfed.org/fred/releases/dates?api_key=fixture-key&file_type=json&realtime_start=2026-09-01&realtime_end=2026-09-30&limit=1000&offset=0&order_by=release_date&sort_order=asc&include_release_dates_with_no_data=true"]
    );
}

#[derive(Clone)]
struct PagedTransport {
    urls: Arc<Mutex<Vec<String>>>,
}

impl HttpTransport for PagedTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.urls.lock().unwrap().push(request.url().to_owned());
        let offset = if request.url().contains("offset=1000") {
            1000_u32
        } else {
            0_u32
        };
        let rows = if offset == 0 {
            (1..=1000)
                .map(|release_id| {
                    serde_json::json!({
                        "release_id": release_id,
                        "release_name": format!("Release {release_id}"),
                        "date": "2026-09-15",
                        "release_last_updated": "2026-08-01 09:30:00-05"
                    })
                })
                .collect::<Vec<_>>()
        } else {
            vec![serde_json::json!({
                "release_id": 1001,
                "release_name": "Release 1001",
                "date": "2026-09-15",
                "release_last_updated": "2026-08-01 09:30:00-05"
            })]
        };
        let body = serde_json::to_vec(&serde_json::json!({
            "realtime_start": "2026-09-01",
            "realtime_end": "2026-09-30",
            "order_by": "release_date",
            "sort_order": "asc",
            "count": 1001,
            "offset": offset,
            "limit": 1000,
            "release_dates": rows
        }))
        .unwrap();
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body,
        ))
    }
}

#[test]
fn probe_acquires_every_declared_page_before_applying_the_caller_limit() {
    let urls = Arc::new(Mutex::new(Vec::new()));
    let client = FredClient::with_transport(
        "fixture-key",
        Arc::new(PagedTransport {
            urls: Arc::clone(&urls),
        }),
    )
    .unwrap();
    let request = EconomicReleaseScheduleRequest::new(
        IsoDate::new("2026-09-01").unwrap(),
        IsoDate::new("2026-09-30").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();

    let batch = client.probe_economic_release_schedule(&request).unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].release_id().get(), 1);
    assert_eq!(batch.records()[1].release_id().get(), 2);
    let urls = urls.lock().unwrap();
    assert_eq!(urls.len(), 2);
    assert!(urls[0].contains("offset=0"));
    assert!(urls[1].contains("offset=1000"));
}

struct StaticTransport(Vec<u8>);

impl HttpTransport for StaticTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            self.0.clone(),
        ))
    }
}

fn schedule_request() -> EconomicReleaseScheduleRequest {
    EconomicReleaseScheduleRequest::new(
        IsoDate::new("2026-09-01").unwrap(),
        IsoDate::new("2026-09-30").unwrap(),
        PositiveU32::new(20).unwrap(),
    )
    .unwrap()
}

#[test]
fn probe_rejects_incomplete_duplicate_and_out_of_range_schedule_rows_atomically() {
    let mut incomplete: serde_json::Value = serde_json::from_slice(SCHEDULE).unwrap();
    incomplete["count"] = serde_json::json!(3);
    let mut duplicate: serde_json::Value = serde_json::from_slice(SCHEDULE).unwrap();
    duplicate["release_dates"][1]["release_id"] = serde_json::json!(10);
    duplicate["release_dates"][1]["date"] = serde_json::json!("2026-09-15");
    let mut outside: serde_json::Value = serde_json::from_slice(SCHEDULE).unwrap();
    outside["release_dates"][1]["date"] = serde_json::json!("2026-10-01");

    for invalid in [incomplete, duplicate, outside] {
        let client = FredClient::with_transport(
            "fixture-key",
            Arc::new(StaticTransport(serde_json::to_vec(&invalid).unwrap())),
        )
        .unwrap();
        assert!(client
            .probe_economic_release_schedule(&schedule_request())
            .is_err());
    }
}

#[test]
fn probe_accepts_a_source_proven_empty_date_range() {
    let mut empty: serde_json::Value = serde_json::from_slice(SCHEDULE).unwrap();
    empty["count"] = serde_json::json!(0);
    empty["release_dates"] = serde_json::json!([]);
    let client = FredClient::with_transport(
        "fixture-key",
        Arc::new(StaticTransport(serde_json::to_vec(&empty).unwrap())),
    )
    .unwrap();

    let batch = client
        .probe_economic_release_schedule(&schedule_request())
        .unwrap();
    assert!(batch.records().is_empty());
    assert!(batch.quality().is_complete());
    assert_eq!(batch.provenance().source_at(), None);
}

#[test]
fn probe_stabilizes_unspecified_same_date_order_by_release_id() {
    let mut same_date: serde_json::Value = serde_json::from_slice(SCHEDULE).unwrap();
    same_date["release_dates"][0]["release_id"] = serde_json::json!(50);
    same_date["release_dates"][0]["release_name"] = serde_json::json!("Release 50");
    same_date["release_dates"][1]["release_id"] = serde_json::json!(10);
    same_date["release_dates"][1]["release_name"] = serde_json::json!("Release 10");
    same_date["release_dates"][1]["date"] = serde_json::json!("2026-09-15");
    let client = FredClient::with_transport(
        "fixture-key",
        Arc::new(StaticTransport(serde_json::to_vec(&same_date).unwrap())),
    )
    .unwrap();

    let batch = client
        .probe_economic_release_schedule(&schedule_request())
        .unwrap();
    assert_eq!(batch.records()[0].release_id().get(), 10);
    assert_eq!(batch.records()[1].release_id().get(), 50);
}

#[test]
fn formal_provider_runs_the_same_path_after_live_admission() {
    let client =
        FredClient::with_transport("fixture-key", Arc::new(StaticTransport(SCHEDULE.to_vec())))
            .unwrap();
    let batch = client
        .economic_release_schedule(&schedule_request())
        .unwrap();
    assert_eq!(batch.records().len(), 2);
}
