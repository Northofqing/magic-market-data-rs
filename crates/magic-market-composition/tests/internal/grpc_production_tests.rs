use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use magic_eastmoney_rs::EastmoneyTransport;
use magic_market_core::{AssetClass, Quote, SecurityMetadata};
use magic_market_service::QueryCommand;
use magic_sina_rs::SnapshotTransport as SinaSnapshotTransport;
use magic_tencent_rs::SnapshotTransport;

use super::*;

struct HistoricalHithinkTransport {
    body: Vec<u8>,
    calls: Arc<AtomicUsize>,
}

impl magic_market_transport::HttpTransport for HistoricalHithinkTransport {
    fn execute(
        &self,
        request: &magic_market_transport::HttpRequest,
    ) -> Result<magic_market_transport::HttpResponse, magic_market_transport::TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(magic_market_transport::HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            self.body.clone(),
        ))
    }
}

fn historical_hithink_registry(
    empty: bool,
    maximum_payload_bytes: usize,
) -> (OperationRegistry, Arc<AtomicUsize>, Vec<u8>) {
    let rows = if empty {
        Vec::new()
    } else {
        [1787068800000_i64, 1786982400000_i64]
            .map(|date_ms| {
                serde_json::json!({
                    "date_ms":date_ms, "open_price":10.0, "high_price":11.0, "low_price":9.0,
                    "close_price":10.5, "volume":1200.0, "turnover":12000.0
                })
            })
            .to_vec()
    };
    let body = format!(" \n{}\n", serde_json::to_string_pretty(&serde_json::json!({
            "code":0, "message":"success", "request_id":"native-history",
            "data":{"thscode":"600519.SH", "interval":"1d", "adjust":"none", "timestamp":1787068800000_i64, "item":rows}
        })).unwrap()).into_bytes();
    let calls = Arc::new(AtomicUsize::new(0));
    let client = HithinkClient::with_transport(
        "test_key",
        HistoricalHithinkTransport {
            body: body.clone(),
            calls: calls.clone(),
        },
    )
    .unwrap();
    let mut registry = OperationRegistry::all_unadmitted("fixture operation is unavailable");
    registry
        .register_handler(
            admitted(
                Operation::HistoricalBars,
                "HithinkFinance",
                HITHINK_HISTORICAL_BARS_SCOPE,
            ),
            move |command| execute_hithink_historical_bars(command, &client, maximum_payload_bytes),
        )
        .unwrap();
    (registry, calls, body)
}

fn historical_hithink_command(version: u32, schema: &str, limit: u32) -> QueryCommand {
    let data = format!(r#"{{ "instrument":{{"exchange":"Shanghai","code":"600519","asset_class":"Equity"}}, "interval":"Day", "start":"2026-08-18", "end":"2026-08-19", "limit":{limit} }}"#).into_bytes();
    QueryCommand::new(
        "historical-coverage-request",
        Operation::HistoricalBars,
        Some("HithinkFinance".into()),
        CanonicalPayload::new(schema, version, data, 65_536).unwrap(),
    )
    .unwrap()
}

#[test]
fn hithink_historical_v2_binds_request_and_reports_observed_coverage_only() {
    let (registry, calls, body) = historical_hithink_registry(false, 65_536);
    let command = historical_hithink_command(2, HISTORICAL_BARS_REQUEST_SCHEMA, 1);
    let request_hash = format!("{:x}", Sha256::digest(command.payload().data()));
    let result = registry.execute(command).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!result.complete);
    assert_eq!(result.provider, "HithinkFinance");
    assert_eq!(result.batch_id, "native-history");
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.historical_bars.coverage");
    assert_eq!(payload.schema_version(), 2);
    let envelope: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(envelope["request_id"], "historical-coverage-request");
    assert_eq!(envelope["request_payload_sha256"], request_hash);
    assert_eq!(envelope["request"]["limit"], 1);
    assert_eq!(
        envelope["coverage_scope"],
        "HithinkNativeDateRangeResponseObservationOnly"
    );
    let coverage = &envelope["result"]["coverage"];
    assert_eq!(coverage["validated_source_rows"], 2);
    assert_eq!(coverage["returned_rows"], 1);
    assert_eq!(coverage["caller_limit_truncated"], true);
    assert_eq!(coverage["authority_calendar_coverage"], "Unknown");
    assert_eq!(coverage["source_revision"], "NotProvided");
    assert_eq!(coverage["pit_guarantee"], false);
    assert_eq!(
        coverage["response_receipt"]["body_sha256"],
        format!("{:x}", Sha256::digest(&body))
    );
    let batch = &envelope["result"]["batch"];
    assert_eq!(batch["quality"]["complete"], false);
    assert_eq!(batch["records"][0]["bar_start"], "2026-08-19");
    assert_eq!(batch["records"][0]["batch_id"], result.batch_id);
    assert_eq!(batch["provenance"]["batch_id"], result.batch_id);
}

#[test]
fn hithink_historical_v1_keeps_record_shape_and_corrected_truncation_quality() {
    for (limit, count, complete) in [(1, 1, false), (2, 2, true), (15, 2, true)] {
        let (registry, calls, _) = historical_hithink_registry(false, 65_536);
        let result = registry
            .execute(historical_hithink_command(
                1,
                HISTORICAL_BARS_REQUEST_SCHEMA,
                limit,
            ))
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(result.complete, complete);
        assert_eq!(result.records.len(), count);
        for payload in &result.records {
            assert_eq!(payload.schema(), HISTORICAL_BARS_RECORD_SCHEMA);
            assert_eq!(payload.schema_version(), 1);
            let record: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
            assert_eq!(record["provider"], "Tonghuashun");
            assert_eq!(record["volume"], 12.0);
            assert_eq!(record["amount"], 12000.0);
            assert_eq!(record["batch_id"], result.batch_id);
            assert!(record.get("result").is_none());
            assert!(record.get("coverage").is_none());
        }
    }
}

#[test]
fn hithink_historical_v2_no_deletion_or_empty_never_promotes_unknown_coverage() {
    for (empty, count) in [(false, 2), (true, 0)] {
        let (registry, _, _) = historical_hithink_registry(empty, 65_536);
        let result = registry
            .execute(historical_hithink_command(
                2,
                HISTORICAL_BARS_REQUEST_SCHEMA,
                15,
            ))
            .unwrap();
        assert!(result.complete);
        assert_eq!(result.records.len(), 1);
        let envelope: serde_json::Value = serde_json::from_slice(result.records[0].data()).unwrap();
        let coverage = &envelope["result"]["coverage"];
        assert_eq!(coverage["validated_source_rows"], count);
        assert_eq!(coverage["returned_rows"], count);
        assert_eq!(coverage["caller_limit_truncated"], false);
        assert_eq!(coverage["source_exhaustion"], "Unknown");
        assert_eq!(coverage["authority_calendar_coverage"], "Unknown");
        assert_eq!(coverage["missing_date_reasons"], "Unknown");
        assert_eq!(coverage["historical_publication_time"], "NotProvided");
        assert_eq!(coverage["pit_guarantee"], false);
        assert!(coverage.get("verified_empty").is_none());
        assert_eq!(
            envelope["result"]["batch"]["records"]
                .as_array()
                .unwrap()
                .len(),
            count
        );
    }
}

#[test]
fn hithink_historical_rejects_version_schema_and_payload_bounds() {
    for (version, schema) in [
        (3, HISTORICAL_BARS_REQUEST_SCHEMA),
        (2, "wrong.schema"),
        (1, "magic.market.ordinary_daily_change_window.request"),
        (2, "magic.market.ordinary_daily_change_window.request"),
    ] {
        let (registry, calls, _) = historical_hithink_registry(false, 65_536);
        assert!(matches!(
            registry.execute(historical_hithink_command(version, schema, 1)),
            Err(ServiceError::InvalidRequest(_))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
    let (registry, calls, _) = historical_hithink_registry(false, 64);
    assert!(matches!(
        registry.execute(historical_hithink_command(
            2,
            HISTORICAL_BARS_REQUEST_SCHEMA,
            1
        )),
        Err(ServiceError::ResourceExhausted(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

struct CoverageCninfoTransport {
    row_count: u64,
    calls: Arc<AtomicUsize>,
}

impl magic_cninfo_rs::CninfoTransport for CoverageCninfoTransport {
    fn execute(
        &self,
        request: &magic_cninfo_rs::HttpRequest,
    ) -> Result<magic_cninfo_rs::HttpResponse, CninfoError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let rows = (0..self.row_count)
            .map(|index| {
                serde_json::json!({
                    "secCode": "600396",
                    "secName": "华电辽能",
                    "orgId": "gssh0600396",
                    "announcementId": format!("coverage-{index}"),
                    "announcementTitle": format!("title {index}"),
                    "announcementTime": 1784822400000_i64,
                    "pageColumn": "SHMB",
                })
            })
            .collect::<Vec<_>>();
        Ok(magic_cninfo_rs::HttpResponse {
            status: 200,
            final_url: request.url.clone(),
            content_type: Some("application/json".into()),
            body: serde_json::to_vec(&serde_json::json!({
                "totalAnnouncement": self.row_count,
                "totalRecordNum": self.row_count,
                "totalpages": self.row_count / 30,
                "hasMore": false,
                "announcements": rows,
            }))
            .unwrap(),
        })
    }
}

fn coverage_cninfo_client(row_count: u64) -> (CninfoClient, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = CninfoClient::with_transport(
        magic_cninfo_rs::CninfoConfig::default(),
        CoverageCninfoTransport {
            row_count,
            calls: calls.clone(),
        },
    )
    .unwrap();
    (client, calls)
}

fn coverage_command(version: u32, schema: &str) -> QueryCommand {
    QueryCommand::new(
        "coverage-request-1",
        Operation::MarketAnnouncements,
        Some("Cninfo".into()),
        CanonicalPayload::new(
            schema,
            version,
            br#"{ "start": "2026-07-24", "end": "2026-07-24", "limit": 1 }"#.to_vec(),
            65_536,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn market_announcements_v2_binds_original_request_and_reports_incomplete_coverage() {
    let (client, calls) = coverage_cninfo_client(2);
    let command = coverage_command(2, MARKET_ANNOUNCEMENTS_REQUEST_SCHEMA);
    let expected_hash = format!("{:x}", Sha256::digest(command.payload().data()));
    let result = execute_market_announcements(command, &client, 65_536).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!result.complete);
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), MARKET_ANNOUNCEMENTS_COVERAGE_SCHEMA);
    assert_eq!(payload.schema_version(), 2);
    let envelope: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(envelope["request_id"], "coverage-request-1");
    assert_eq!(envelope["request_payload_sha256"], expected_hash);
    assert_eq!(envelope["request"]["limit"], 1);
    assert_eq!(envelope["pit_guarantee"], false);
    assert_eq!(envelope["exchange_event_universe_complete"], false);
    let coverage = &envelope["result"]["coverage"];
    assert_eq!(coverage["source_total"], 2);
    assert_eq!(coverage["inspected_raw_rows"], 2);
    assert_eq!(coverage["unique_rows"], 2);
    assert_eq!(coverage["returned_rows"], 1);
    assert_eq!(coverage["source_exhausted"], true);
    assert_eq!(coverage["caller_limit_truncated"], true);
    assert_eq!(envelope["result"]["batch"]["quality"]["complete"], false);
    assert_eq!(
        envelope["result"]["batch"]["records"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        envelope["result"]["batch"]["provenance"]["batch_id"],
        result.batch_id
    );
}

#[test]
fn market_announcements_v1_keeps_record_shape_and_incomplete_quality() {
    let (client, _) = coverage_cninfo_client(2);
    let result = execute_market_announcements(
        coverage_command(1, MARKET_ANNOUNCEMENTS_REQUEST_SCHEMA),
        &client,
        65_536,
    )
    .unwrap();
    assert!(!result.complete);
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].schema(), ANNOUNCEMENTS_RECORD_SCHEMA);
    assert_eq!(result.records[0].schema_version(), 1);
    let record: serde_json::Value = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(record["announcement_id"], "coverage-0");
    assert!(record.get("result").is_none());
    assert_eq!(record["evidence"]["batch_id"], result.batch_id);
}

#[test]
fn market_announcements_v2_verified_empty_keeps_one_coverage_envelope() {
    let (client, _) = coverage_cninfo_client(0);
    let result = execute_market_announcements(
        coverage_command(2, MARKET_ANNOUNCEMENTS_REQUEST_SCHEMA),
        &client,
        65_536,
    )
    .unwrap();
    assert!(result.complete);
    assert!(result.source_at.is_none());
    assert_eq!(result.records.len(), 1);
    let envelope: serde_json::Value = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(envelope["result"]["coverage"]["verified_empty"], true);
    assert_eq!(envelope["result"]["coverage"]["source_total"], 0);
    assert!(envelope["result"]["batch"]["records"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn market_announcements_rejects_unknown_version_or_schema_before_io() {
    for (version, schema) in [
        (3, MARKET_ANNOUNCEMENTS_REQUEST_SCHEMA),
        (2, "wrong.schema"),
    ] {
        let (client, calls) = coverage_cninfo_client(2);
        assert!(matches!(
            execute_market_announcements(coverage_command(version, schema), &client, 65_536),
            Err(ServiceError::InvalidRequest(_))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn market_announcements_v2_obeys_response_payload_bound() {
    let (client, _) = coverage_cninfo_client(2);
    assert!(matches!(
        execute_market_announcements(
            coverage_command(2, MARKET_ANNOUNCEMENTS_REQUEST_SCHEMA),
            &client,
            64,
        ),
        Err(ServiceError::ResourceExhausted(_))
    ));
}

#[test]
fn futures_delivery_v2_projection_marks_planned_rule_derived_dates() {
    let client = CffexClient::new().unwrap();
    let request = FuturesDeliveryRequest::new(
        PositiveU32::new(2026).unwrap(),
        PositiveU32::new(9).unwrap(),
    )
    .unwrap();
    let batch = client.futures_delivery_calendar(&request).unwrap();
    let result = futures_delivery_query_result(batch, 4096).unwrap();
    assert!(result.complete);
    assert_eq!(result.records.len(), 4);
    assert!(result.source_at.is_none());
    for (index, payload) in result.records.iter().enumerate() {
        assert_eq!(payload.schema(), FUTURES_DELIVERY_RECORD_SCHEMA);
        assert_eq!(payload.schema_version(), 2);
        let record: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(record["schedule_status"], "Planned");
        assert_eq!(record["date_basis"], "CffexRuleAndPublishedHolidays");
        assert_eq!(record["delivery_date"], "2026-09-18");
        assert_eq!(
            record["holiday_calendar_url"],
            FUTURES_DELIVERY_HOLIDAY_CALENDAR_URL
        );
        assert!(record.get("notice_url").is_none());
        assert!(record["rule_url"]
            .as_str()
            .unwrap()
            .starts_with("https://www.cffex.com.cn/"));
        assert_eq!(
            record["contract_code"],
            ["IF2609", "IH2609", "IC2609", "IM2609"][index]
        );
    }
}

fn news_record(
    id: &str,
    published_at: &str,
    source_at: Option<&str>,
    provider: ProviderId,
    observed_at: &str,
    batch_id: &str,
) -> NewsItem {
    let mut evidence = SourceEvidence::new(provider, observed_at, batch_id).unwrap();
    if let Some(source_at) = source_at {
        evidence = evidence.with_source_at(source_at).unwrap();
    }
    NewsItem {
        item_id: NonEmptyText::new(id).unwrap(),
        title: NonEmptyText::new(format!("news {id}")).unwrap(),
        summary: None,
        content: None,
        publisher: NonEmptyText::new("fixture").unwrap(),
        canonical_url: magic_market_core::HttpsUrl::new(format!("https://example.com/{id}"))
            .unwrap(),
        published_at: NonEmptyText::new(published_at).unwrap(),
        instruments: Vec::new(),
        topics: Vec::new(),
        language: NonEmptyText::new("zh-CN").unwrap(),
        evidence,
    }
}

fn news_batch(records: Vec<NewsItem>, source_at: &str, batch_id: &str) -> DataBatch<NewsItem> {
    DataBatch::strict(
        records,
        Provenance::new("jin10", "1787127606.533354000")
            .unwrap()
            .with_source_at(source_at)
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    )
}

#[test]
fn global_news_v2_preserves_two_distinct_record_evidence_times() {
    let batch_id = "TEST_GLOBAL_NEWS_BATCH";
    let batch = news_batch(
        vec![
            news_record(
                "NEWS_001",
                "2026-08-19T16:15:37+08:00",
                Some("2026-08-19 16:15:37"),
                ProviderId::Jin10,
                "1787127606.533354000",
                batch_id,
            ),
            news_record(
                "NEWS_002",
                "2026-08-19T16:14:00+08:00",
                Some("2026-08-19 16:14:00"),
                ProviderId::Jin10,
                "1787127605.000000000",
                batch_id,
            ),
        ],
        "2026-08-19 16:15:37",
        batch_id,
    );
    let result = global_news_query_result(&batch, "Jin10", ProviderId::Jin10, 16 * 1024).unwrap();
    assert_eq!(result.source_at.as_deref(), Some("2026-08-19 16:15:37"));
    assert!(result
        .records
        .iter()
        .all(|record| record.schema_version() == 2));
    let rows = result
        .records
        .iter()
        .map(|record| serde_json::from_slice::<serde_json::Value>(record.data()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        rows[0]
            .pointer("/evidence/source_at")
            .and_then(|v| v.as_str()),
        Some("2026-08-19 16:15:37")
    );
    assert_eq!(
        rows[1]
            .pointer("/evidence/source_at")
            .and_then(|v| v.as_str()),
        Some("2026-08-19 16:14:00")
    );
}

#[test]
fn global_news_v2_rejects_every_record_evidence_conflict_atomically() {
    let cases = [
        (
            news_record(
                "bad-provider",
                "2026-08-19T16:15:37+08:00",
                Some("2026-08-19 16:15:37"),
                ProviderId::Eastmoney,
                "1787127606.533354000",
                "batch",
            ),
            "record_provider_mismatch",
        ),
        (
            news_record(
                "bad-batch",
                "2026-08-19T16:15:37+08:00",
                Some("2026-08-19 16:15:37"),
                ProviderId::Jin10,
                "1787127606.533354000",
                "other-batch",
            ),
            "record_batch_mismatch",
        ),
        (
            news_record(
                "bad-source",
                "2026-08-19T16:14:00+08:00",
                Some("2026-08-19 16:15:37"),
                ProviderId::Jin10,
                "1787127606.533354000",
                "batch",
            ),
            "record_published_at_mismatch",
        ),
        (
            news_record(
                "future-source",
                "2026-08-19T16:30:00+08:00",
                Some("2026-08-19 16:30:00"),
                ProviderId::Jin10,
                "1787127606.533354000",
                "batch",
            ),
            "record_source_after_observation",
        ),
        (
            news_record(
                "missing-source",
                "2026-08-19T16:15:37+08:00",
                None,
                ProviderId::Jin10,
                "1787127606.533354000",
                "batch",
            ),
            "record_evidence_incomplete",
        ),
        (
            news_record(
                "observed-after-batch",
                "2026-08-19T16:15:37+08:00",
                Some("2026-08-19 16:15:37"),
                ProviderId::Jin10,
                "1787127607.000000000",
                "batch",
            ),
            "record_observed_after_batch",
        ),
    ];
    for (record, expected_code) in cases {
        let error = global_news_query_result(
            &news_batch(vec![record], "2026-08-19 16:15:37", "batch"),
            "Jin10",
            ProviderId::Jin10,
            16 * 1024,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ServiceError::InvalidEvidence { evidence_code, .. }
                if evidence_code == expected_code
        ));
    }

    let copied_batch_time = news_batch(
        vec![
            news_record(
                "newest",
                "2026-08-19T16:15:37+08:00",
                Some("2026-08-19 16:15:37"),
                ProviderId::Jin10,
                "1787127606.533354000",
                "batch",
            ),
            news_record(
                "older-with-copied-batch-time",
                "2026-08-19T16:14:00+08:00",
                Some("2026-08-19 16:15:37"),
                ProviderId::Jin10,
                "1787127606.533354000",
                "batch",
            ),
        ],
        "2026-08-19 16:15:37",
        "batch",
    );
    assert!(matches!(
        global_news_query_result(
            &copied_batch_time,
            "Jin10",
            ProviderId::Jin10,
            16 * 1024,
        ),
        Err(ServiceError::InvalidEvidence { evidence_code, .. })
            if evidence_code == "record_published_at_mismatch"
    ));
}

#[test]
fn instrument_news_v2_filters_at_the_callers_exact_cutoff() {
    let batch_id = "TEST_INSTRUMENT_NEWS_BATCH";
    let batch = news_batch(
        vec![
            news_record(
                "after-cutoff",
                "2026-08-19T16:16:00+08:00",
                Some("2026-08-19T16:16:00+08:00"),
                ProviderId::Sina,
                "1787127606.000000000",
                batch_id,
            ),
            news_record(
                "at-cutoff",
                "2026-08-19T16:15:37+08:00",
                Some("2026-08-19T16:15:37+08:00"),
                ProviderId::Sina,
                "1787127605.000000000",
                batch_id,
            ),
            news_record(
                "before-cutoff",
                "2026-08-19T16:14:00+08:00",
                Some("2026-08-19T16:14:00+08:00"),
                ProviderId::Sina,
                "1787127604.000000000",
                batch_id,
            ),
        ],
        "2026-08-19T16:16:00+08:00",
        batch_id,
    );

    let filtered = filter_instrument_news_batch(
        batch,
        "2026-08-19T16:15:37+08:00",
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();

    assert_eq!(filtered.records().len(), 2);
    assert_eq!(filtered.records()[0].item_id.as_str(), "at-cutoff");
    assert_eq!(
        filtered.provenance().source_at(),
        Some("2026-08-19T16:15:37+08:00")
    );
}

#[test]
fn instrument_news_v2_preserves_a_truthful_cutoff_empty_batch() {
    let batch_id = "TEST_INSTRUMENT_NEWS_CUTOFF_EMPTY";
    let batch = news_batch(
        vec![
            news_record(
                "after-cutoff-1",
                "2026-08-19T16:16:00+08:00",
                Some("2026-08-19T16:16:00+08:00"),
                ProviderId::Sina,
                "1787127606.000000000",
                batch_id,
            ),
            news_record(
                "after-cutoff-2",
                "2026-08-19T16:15:38+08:00",
                Some("2026-08-19T16:15:38+08:00"),
                ProviderId::Sina,
                "1787127605.000000000",
                batch_id,
            ),
        ],
        "2026-08-19T16:16:00+08:00",
        batch_id,
    );

    let result = instrument_news_query_result(
        batch,
        "2026-08-19T16:15:37+08:00",
        PositiveU32::new(5).unwrap(),
        16 * 1024,
    )
    .expect("a valid cutoff-empty result must remain admitted");

    assert!(result.repository_admitted);
    assert!(result.complete);
    assert!(result.records.is_empty());
    assert_eq!(result.provider, "Sina");
    assert_eq!(result.batch_id, batch_id);
    assert_eq!(result.source_at, None);
    assert_eq!(result.observed_at, "1787127606.533354000");
    assert_eq!(result.diagnostic_blocker, None);
}

#[test]
fn instrument_news_v2_cutoff_cannot_hide_invalid_upstream_evidence() {
    let batch_id = "TEST_INSTRUMENT_NEWS_INVALID_AFTER_CUTOFF";
    let batch = news_batch(
        vec![news_record(
            "invalid-after-cutoff",
            "2026-08-19T16:16:00+08:00",
            Some("2026-08-19T16:16:00+08:00"),
            ProviderId::Eastmoney,
            "1787127606.000000000",
            batch_id,
        )],
        "2026-08-19T16:16:00+08:00",
        batch_id,
    );

    assert!(matches!(
        instrument_news_query_result(
            batch,
            "2026-08-19T16:15:37+08:00",
            PositiveU32::new(5).unwrap(),
            16 * 1024,
        ),
        Err(ServiceError::InvalidEvidence {
            evidence_code,
            record_index: Some(0),
            ..
        }) if evidence_code == "record_provider_mismatch"
    ));
}

#[test]
fn instrument_news_v2_preserves_a_source_proven_empty_range() {
    let batch_id = "sina-company-news:sh600000:1787127606.533354000:pages-1";
    let batch = DataBatch::strict(
        Vec::new(),
        Provenance::new("sina-company-news", "1787127606.533354000")
            .unwrap()
            .with_source_at("2026-08-19T16:16:00+08:00")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    );

    let result = instrument_news_query_result(
        batch,
        "2026-08-19T16:15:37+08:00",
        PositiveU32::new(5).unwrap(),
        16 * 1024,
    )
    .expect("a source-proven empty date range must remain admitted");

    assert!(result.repository_admitted);
    assert!(result.complete);
    assert!(result.records.is_empty());
    assert_eq!(result.provider, "Sina");
    assert_eq!(result.batch_id, batch_id);
    assert_eq!(result.source_at, None);
    assert_eq!(result.observed_at, "1787127606.533354000");
}

#[test]
fn instrument_news_v2_rejects_unproved_empty_ranges() {
    let cases = [
        (
            Provenance::new("other-source", "1787127606.533354000")
                .unwrap()
                .with_source_at("2026-08-19T16:16:00+08:00")
                .unwrap()
                .with_batch_id("sina-company-news:sh600000:1787127606.533354000:pages-1")
                .unwrap(),
            "batch_source_mismatch",
        ),
        (
            Provenance::new("sina-company-news", "1787127606.533354000")
                .unwrap()
                .with_batch_id("sina-company-news:sh600000:1787127606.533354000:pages-1")
                .unwrap(),
            "batch_evidence_incomplete",
        ),
        (
            Provenance::new("sina-company-news", "1787127606.533354000")
                .unwrap()
                .with_source_at("2026-08-19T16:16:00+08:00")
                .unwrap()
                .with_batch_id("foreign-batch")
                .unwrap(),
            "batch_identity_invalid",
        ),
        (
            Provenance::new("sina-company-news", "2026-08-19T16:15:00+08:00")
                .unwrap()
                .with_source_at("2026-08-19T16:16:00+08:00")
                .unwrap()
                .with_batch_id("sina-company-news:sh600000:2026-08-19T16:15:00+08:00:pages-1")
                .unwrap(),
            "batch_source_after_observation",
        ),
    ];

    for (provenance, expected_code) in cases {
        let batch = DataBatch::<NewsItem>::strict(Vec::new(), provenance);
        assert!(matches!(
            instrument_news_query_result(
                batch,
                "2026-08-19T16:15:37+08:00",
                PositiveU32::new(5).unwrap(),
                16 * 1024,
            ),
            Err(ServiceError::InvalidEvidence { evidence_code, .. })
                if evidence_code == expected_code
        ));
    }
}

#[test]
fn consensus_schema_failures_keep_safe_structured_field_diagnostics() {
    let cases = [
        (
            "consensus title code 000001 does not match requested 600000",
            "consensus_instrument_identity_invalid",
            "consensus.instrument_identity",
        ),
        (
            "invalid EPS fiscal year xyz",
            "consensus_fiscal_year_invalid",
            "consensus.estimates.fiscal_year",
        ),
        (
            "EPS mean is above maximum",
            "consensus_maximum_invalid",
            "consensus.estimates.maximum",
        ),
        (
            "EPS table has no header and data rows",
            "consensus_table_invalid",
            "consensus.estimates.table",
        ),
    ];
    for (message, expected_code, expected_field) in cases {
        let error = map_ths_error(Operation::Consensus, &ThsError::Schema(message.into()));
        assert!(matches!(
            error,
            ServiceError::InvalidEvidence {
                provider,
                evidence_code,
                evidence_field,
                record_index: None,
                ..
            } if provider == "Tonghuashun"
                && evidence_code == expected_code
                && evidence_field == expected_field
        ));
    }
}

#[test]
fn t0_observation_clock_is_explicit_local_china_time() {
    let observed_at = current_china_observed_at().unwrap();
    assert_eq!(observed_at.len(), 25);
    assert_eq!(observed_at.as_bytes().get(10), Some(&b'T'));
    assert!(observed_at.ends_with("+08:00"));
}

#[test]
fn t0_external_decoder_rejects_v1_and_requires_v2_requested_at() {
    let data = serde_json::to_vec(&serde_json::json!({
        "instruments": [{
            "exchange": "Shanghai",
            "code": "600396",
            "asset_class": "Equity"
        }],
        "daily_bar_count": 20,
        "five_minute_bar_count": 20,
        "requested_at": "2026-08-27T09:24:10.123456+08:00"
    }))
    .unwrap();
    let command = |version| {
        QueryCommand::new(
            format!("t0-v{version}"),
            Operation::T0Evidence,
            Some("Tdx".to_owned()),
            CanonicalPayload::new(T0_EVIDENCE_REQUEST_SCHEMA, version, data.clone(), 4096).unwrap(),
        )
        .unwrap()
    };
    assert!(matches!(
        decode_request_version::<T0EvidenceRequest>(
            &command(1),
            T0_EVIDENCE_REQUEST_SCHEMA,
            T0_EVIDENCE_SCHEMA_VERSION,
        ),
        Err(ServiceError::InvalidRequest(message)) if message.contains("version 2")
    ));
    let request = decode_request_version::<T0EvidenceRequest>(
        &command(T0_EVIDENCE_SCHEMA_VERSION),
        T0_EVIDENCE_REQUEST_SCHEMA,
        T0_EVIDENCE_SCHEMA_VERSION,
    )
    .unwrap();
    assert_eq!(request.requested_at(), "2026-08-27T09:24:10.123456+08:00");
}

const QUOTE_RESPONSE: &str = "v_sh600396=\"1~ABC~600396~15.47~14.92~15.30~1775070~821130~950794~15.47~212~15.46~95~15.45~64~15.44~3~15.43~375~15.49~49~15.50~2721~15.51~241~15.52~450~15.53~86~~20260723094907~0.55~3.69~15.88~14.85~15.47/1775070/2729507908~1775070~272951~\";";
const INDEX_QUOTE_RESPONSE: &str = "v_sh000001=\"1~Shanghai Composite~000001~3560.47~3544.15~3551.30~1775070~821130~950794~3560.47~212~3560.46~95~3560.45~64~3560.44~3~3560.43~375~3560.49~49~3560.50~2721~3560.51~241~3560.52~450~3560.53~86~~20260723094907~16.32~0.46~3568.88~3538.85~3560.47/1775070/2729507908~1775070~272951~\";";
const SINA_AUCTION_NO_TRADE_RESPONSE: &str = "var hq_str_sh600396=\"ABC,0.000,14.920,0.000,0.000,0.000,16.410,16.420,0,0.000,6409200,16.410,72100,16.400,17600,16.390,3500,16.380,5000,16.370,1000,16.420,2000,16.430,3000,16.440,4000,16.450,5000,16.460,2026-08-31,09:20:00,00\";";

#[derive(Clone)]
struct SinaAuctionTransport;

impl SinaSnapshotTransport for SinaAuctionTransport {
    fn get(&self, _url: &str) -> Result<Vec<u8>, SinaError> {
        Ok(SINA_AUCTION_NO_TRADE_RESPONSE.as_bytes().to_vec())
    }
}

#[derive(Clone)]
struct StaticTransport {
    calls: Arc<AtomicUsize>,
}

impl SnapshotTransport for StaticTransport {
    fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(QUOTE_RESPONSE.as_bytes().to_vec())
    }
}

#[derive(Clone)]
struct IndexTransport {
    calls: Arc<AtomicUsize>,
}

impl SnapshotTransport for IndexTransport {
    fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(INDEX_QUOTE_RESPONSE.as_bytes().to_vec())
    }
}

#[derive(Clone)]
struct FailingIndexTransport;

impl SnapshotTransport for FailingIndexTransport {
    fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
        Err(TencentError::Transport("fixture TLS failure".to_owned()))
    }
}

#[derive(Clone)]
struct ShapeTransport {
    calls: Arc<AtomicUsize>,
}

impl SnapshotTransport for ShapeTransport {
    fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(br#"{"code":0,"data":{"sh600396":{"data":[{"date":"20260723","data":["0930 10.00 10 100000.00","0931 10.20 20 204000.00","1300 10.10 30 303000.00"]}]}}}"#.to_vec())
    }
}

#[derive(Clone)]
struct LimitPoolTransport {
    calls: Arc<AtomicUsize>,
}

impl EastmoneyTransport for LimitPoolTransport {
    fn get(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(br#"{"rc":0,"data":{"tc":1,"qdate":20260723,"pool":[{"c":"600396","m":1,"p":1308000,"zdp":9.97,"lbc":3}]}}"#.to_vec())
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Protocol(
            "unexpected POST in limit-pool test".into(),
        ))
    }
}

fn command(schema: &str, provider: Option<&str>) -> QueryCommand {
    command_for(Operation::RealtimeQuotes, schema, provider)
}

fn command_for(operation: Operation, schema: &str, provider: Option<&str>) -> QueryCommand {
    let data = serde_json::to_vec(&serde_json::json!({
        "instruments": [{
            "exchange": "Shanghai",
            "code": "600396",
            "asset_class": "Equity"
        }]
    }))
    .unwrap();
    QueryCommand::new(
        "quote-1",
        operation,
        provider.map(str::to_owned),
        CanonicalPayload::new(schema, SCHEMA_VERSION, data, 4096).unwrap(),
    )
    .unwrap()
}

#[test]
fn sina_auction_order_book_is_admitted_without_a_trade_price() {
    let mut registry = OperationRegistry::all_unadmitted("fixture");
    register_sina_parity(
        &mut registry,
        SinaClient::with_transport(SinaAuctionTransport),
        4096,
    )
    .unwrap();

    let result = registry
        .execute(command_for(
            Operation::OrderBooks,
            ORDER_BOOKS_REQUEST_SCHEMA,
            Some("Sina"),
        ))
        .unwrap();

    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.provider, "Sina");
    assert_eq!(
        result.source_at.as_deref(),
        Some("2026-08-31T09:20:00+08:00")
    );
    assert_eq!(result.records.len(), 1);
    let book: OrderBook = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(book.status(), DataStatus::Available);
    assert_eq!(
        book.bids()[0].quantity().map(|value| value.get()),
        Some(64_092.0)
    );
}

#[test]
fn production_registry_is_exhaustive_and_only_enables_evidence_backed_operations() {
    let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
    assert_eq!(
        registry.default_provider(Operation::GlobalNews),
        Some("WallstreetCn")
    );
    let quote = registry
        .capabilities()
        .into_iter()
        .find(|capability| capability.operation == Operation::RealtimeQuotes)
        .unwrap();
    assert!(quote.repository_admitted);
    assert!(quote.runtime_available);
    assert_eq!(quote.provider, TENCENT_PROVIDER);
    let capabilities = registry.capabilities();
    let covered = capabilities
        .iter()
        .map(|capability| capability.operation)
        .collect::<BTreeSet<_>>();
    assert_eq!(covered.len(), magic_market_service::ALL_OPERATIONS.len());
    assert!(capabilities.iter().all(|capability| capability
        .blocker
        .as_deref()
        .is_none_or(|blocker| !blocker.contains("no evidence-backed production handler"))));
    let admitted = capabilities
        .iter()
        .filter(|capability| capability.repository_admitted)
        .map(|capability| capability.operation)
        .collect::<BTreeSet<_>>();
    assert_eq!(admitted.len(), 63);
    let blocked = magic_market_service::ALL_OPERATIONS
        .iter()
        .copied()
        .filter(|operation| !admitted.contains(operation))
        .collect::<Vec<_>>();
    // `Auctions` has no admitted provider: the complete Core Level-2
    // contract was never admitted, and the narrow Miaoxiang observation
    // that was its only admitted route is now diagnostic only.
    assert_eq!(
        blocked,
        vec![Operation::Auctions, Operation::EconomicCalendar]
    );
    let t0 = capabilities
        .iter()
        .find(|capability| capability.operation == Operation::T0Evidence)
        .unwrap();
    assert!(t0.repository_admitted);
    assert!(t0.runtime_available);
    assert!(!t0.diagnostic_available);
    assert!(matches!(
        registry.execute(command_for(Operation::T0Evidence, "wrong.schema", None)),
        Err(ServiceError::InvalidRequest(_))
    ));
    assert!(matches!(
        registry.execute(
            command_for(Operation::T0Evidence, "wrong.schema", Some("Tdx"))
                .with_unadmitted_access(true)
        ),
        Err(ServiceError::InvalidRequest(_))
    ));
    let diagnostic = capabilities
        .iter()
        .filter(|capability| capability.diagnostic_available)
        .map(|capability| capability.operation)
        .collect::<BTreeSet<_>>();
    assert!(diagnostic.contains(&Operation::HistoricalBars));
    assert!(diagnostic.contains(&Operation::EconomicCalendar));
    for operation in [
        Operation::FuturesDelivery,
        Operation::MarketRankings,
        Operation::MarketBreadth,
    ] {
        assert!(!diagnostic.contains(&operation));
    }
}

#[test]
fn current_auction_observation_request_requires_explicit_known_stage() {
    let request: CurrentAuctionObservationsRequest = serde_json::from_value(serde_json::json!({
        "instruments": [{
            "exchange": "Shanghai",
            "code": "600519",
            "asset_class": "Equity"
        }],
        "stage": "live"
    }))
    .unwrap();
    assert_eq!(request.stage, CurrentAuctionStage::Live);

    for invalid in [
        serde_json::json!({"instruments": []}),
        serde_json::json!({"instruments": [], "stage": "opening"}),
        serde_json::json!({"instruments": [], "stage": "final", "trading_date": "2026-09-09"}),
    ] {
        assert!(serde_json::from_value::<CurrentAuctionObservationsRequest>(invalid).is_err());
    }
}

#[test]
fn production_registry_exposes_every_new_provider_parity_registration() {
    let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
    let capabilities = registry.capabilities();
    let expected_admitted = [
        (Operation::GlobalNews, "Cls"),
        (Operation::GlobalNews, "Cailianpress"),
        (Operation::GlobalNews, "ThePaper"),
        (Operation::GlobalNews, "XinhuaFinance"),
        (Operation::GlobalNews, "Yicai"),
        (Operation::GlobalNews, "Yonhap"),
        (Operation::EconomicSeries, "Nbs"),
        (Operation::EconomicSeries, "Pbc"),
        (Operation::EconomicSeries, "WorldBank"),
        (Operation::EconomicReleaseObservations, "Jin10"),
        (Operation::RealtimeQuotes, "Sina"),
        (Operation::HistoricalBars, "Sina"),
        (Operation::MinuteData, "Sina"),
        (Operation::OrderBooks, "Sina"),
        (Operation::SecurityMetadata, "Sina"),
        (Operation::InstrumentNews, "Sina"),
        (Operation::IndexQuotes, "Tencent"),
        (Operation::IntradayShape, "LocalAnalysis"),
        (Operation::UpperLimitPoolReview, "Eastmoney"),
        (Operation::MarketRankings, "Eastmoney"),
        (Operation::FuturesDelivery, "Cffex"),
        (Operation::FundFlowSeries, "Eastmoney"),
        (Operation::MoneyFlows, "Eastmoney"),
        (Operation::PostCloseFlows, "Eastmoney"),
        (Operation::TechnicalBars, "Baidu"),
        (Operation::OutcomeDailyBars, "Tdx"),
        (Operation::T0Evidence, "Tdx"),
        (Operation::RealtimeQuotes, "Tdx"),
        (Operation::HistoricalBars, "Tdx"),
        (Operation::MinuteData, "Tdx"),
        (Operation::OrderBooks, "Tdx"),
        (Operation::Trades, "Tdx"),
        (Operation::SecurityMetadata, "Tdx"),
        (Operation::RealtimeQuotes, "Szse"),
        (Operation::OrderBooks, "Szse"),
        (Operation::Announcements, "Sse"),
        (Operation::Announcements, "Szse"),
        (Operation::DragonTiger, "Sse"),
        (Operation::DragonTiger, "Szse"),
    ];
    for (operation, provider) in expected_admitted {
        assert!(
            capabilities.iter().any(|capability| {
                capability.operation == operation
                    && capability.provider == provider
                    && capability.repository_admitted
                    && capability.runtime_available
                    && !capability.diagnostic_available
            }),
            "missing admitted {provider} {} registration",
            operation.as_str()
        );
    }

    let fred_schedule = capabilities
        .iter()
        .find(|capability| {
            capability.operation == Operation::EconomicReleaseSchedule
                && capability.provider == "Fred"
        })
        .expect("missing FRED economic release schedule registration");
    assert!(fred_schedule.repository_admitted);
    assert_eq!(
        fred_schedule.runtime_available,
        env::var("FRED_API_KEY").is_ok_and(|value| !value.trim().is_empty())
    );
    assert!(!fred_schedule.diagnostic_available);

    let breadth = capabilities
        .iter()
        .find(|capability| {
            capability.operation == Operation::MarketBreadth
                && capability.provider == "EastmoneyMiaoxiang"
        })
        .expect("missing admitted Miaoxiang breadth registration");
    assert!(breadth.repository_admitted);
    assert_eq!(breadth.runtime_available, eastmoney_mx_key_is_configured());
    assert!(!breadth.diagnostic_available);

    let miaoxiang_auctions = capabilities
        .iter()
        .find(|capability| {
            capability.operation == Operation::Auctions
                && capability.provider == "EastmoneyMiaoxiang"
        })
        .expect("missing Miaoxiang auction diagnostic registration");
    assert!(!miaoxiang_auctions.repository_admitted);
    assert!(!miaoxiang_auctions.runtime_available);
    assert_eq!(
        miaoxiang_auctions.diagnostic_available,
        eastmoney_mx_key_is_configured()
    );
    assert!(miaoxiang_auctions
        .blocker
        .as_deref()
        .is_some_and(|blocker| blocker.contains("cardinality")));

    let emquant_bars = capabilities
        .iter()
        .find(|capability| {
            capability.operation == Operation::HistoricalBars && capability.provider == "EmQuant"
        })
        .expect("missing EmQuant daily-bar registration");
    assert!(emquant_bars.repository_admitted);
    assert_eq!(
        emquant_bars.runtime_available,
        EmQuantClient::discover().is_ok()
    );
    assert!(!emquant_bars.diagnostic_available);

    for operation in [
        Operation::HistoricalBars,
        Operation::RealtimeQuotes,
        Operation::MarketStatistics,
        Operation::LimitPools,
        Operation::Popularity,
        Operation::FinancialStatements,
        Operation::CorporateActions,
        Operation::SecurityMetadata,
        Operation::CurrentAuctionObservations,
    ] {
        let hithink = capabilities
            .iter()
            .find(|capability| {
                capability.operation == operation && capability.provider == "HithinkFinance"
            })
            .expect("missing official HITHINK Fuyao registration");
        assert!(hithink.repository_admitted);
        assert_eq!(hithink.runtime_available, hithink_key_is_configured());
        assert!(!hithink.diagnostic_available);
    }

    let hithink_auctions = capabilities
        .iter()
        .find(|capability| {
            capability.operation == Operation::Auctions && capability.provider == "HithinkFinance"
        })
        .expect("missing official HITHINK Fuyao auction diagnostic registration");
    assert!(!hithink_auctions.repository_admitted);
    assert!(!hithink_auctions.runtime_available);
    assert_eq!(
        hithink_auctions.diagnostic_available,
        hithink_key_is_configured()
    );

    // `Auctions` is deliberately absent: with the Miaoxiang observation
    // demoted, no provider holds an admitted `Auctions` capability, so the
    // operation has no admitted route to fall back to.
    let unadmitted_with_operation_route = [
        (Operation::EconomicSeries, "Imf", "WorldBank"),
        (Operation::FundFlowSeries, "EastmoneyMiaoxiang", "Eastmoney"),
        (Operation::HistoricalBars, "Baidu", "Tencent"),
        (Operation::MoneyFlows, "EastmoneyMiaoxiang", "Eastmoney"),
        (Operation::MoneyFlows, "EmQuant", "Eastmoney"),
        (Operation::OrderBooks, "EmQuant", "Tencent"),
        (Operation::RealtimeQuotes, "EmQuant", "Tencent"),
        (Operation::GlobalNews, "SecuritiesTimes", "Cls"),
        (Operation::OfficialPublications, "Gacc", "Nbs"),
        (Operation::OfficialPublication, "Gacc", "Nbs"),
    ];
    // Unadmitted capabilities whose own operation has no admitted route at
    // all, so they cannot join the list above: both `Auctions`
    // registrations, because the complete Core Level-2 contract was never
    // admitted and the narrow Miaoxiang observation that was its only
    // admitted route is now diagnostic only, plus `EconomicCalendar`.
    let unadmitted_without_an_admitted_route = [
        "auctions/EastmoneyMiaoxiang",
        "auctions/HithinkFinance",
        "economic_calendar/Jin10",
    ];
    let unadmitted = capabilities
        .iter()
        .filter(|capability| !capability.repository_admitted)
        .map(|capability| format!("{}/{}", capability.operation.as_str(), capability.provider))
        .collect::<Vec<_>>();
    assert_eq!(
        unadmitted.len(),
        unadmitted_with_operation_route.len() + unadmitted_without_an_admitted_route.len()
    );
    for expected in unadmitted_without_an_admitted_route {
        assert!(
            unadmitted.contains(&expected.to_owned()),
            "missing fail-closed {expected} registration"
        );
    }
    for (operation, provider, admitted_operation_provider) in unadmitted_with_operation_route {
        assert!(
            capabilities.iter().any(|capability| {
                capability.operation == operation
                    && capability.provider == provider
                    && !capability.repository_admitted
            }),
            "missing fail-closed {provider} {} registration",
            operation.as_str()
        );
        assert!(
            capabilities.iter().any(|capability| {
                capability.operation == operation
                    && capability.provider == admitted_operation_provider
                    && capability.repository_admitted
            }),
            "missing explicit admitted operation route {admitted_operation_provider} for {}",
            operation.as_str()
        );
    }
}

#[test]
fn economic_release_schedule_projection_preserves_date_only_evidence() {
    let observed_at = "2026-09-13T00:00:00Z";
    let batch_id = "FRED:economic-release-schedule:test";
    let record = magic_market_core::EconomicReleaseScheduleEntry::new(
        PositiveU32::new(10).unwrap(),
        "Consumer Price Index",
        IsoDate::new("2026-09-15").unwrap(),
        Some("2026-08-01 09:30:00-05".to_owned()),
        SourceEvidence::new(ProviderId::Fred, observed_at, batch_id).unwrap(),
    )
    .unwrap();
    let batch = DataBatch::strict(
        vec![record],
        Provenance::new("FRED release schedule", observed_at)
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    );

    let result =
        provider_query_result(batch, "Fred", ECONOMIC_RELEASE_SCHEDULE_RECORD_SCHEMA, 4096)
            .unwrap();
    assert!(result.complete);
    assert_eq!(result.source_at, None);
    assert!(result.repository_admitted);
    assert_eq!(result.records.len(), 1);
    assert_eq!(
        result.records[0].schema(),
        ECONOMIC_RELEASE_SCHEDULE_RECORD_SCHEMA
    );
    let value: serde_json::Value = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(value["release_id"], 10);
    assert_eq!(value["release_date"], "2026-09-15");
    assert_eq!(value["evidence"]["provider"], "Fred");
    assert!(value["evidence"]["source_at"].is_null());
}

#[test]
fn financial_statement_v2_adds_fiscal_period_without_changing_v1() {
    let observed_at = "1789453335.543000000";
    let batch_id = "hithink-financial-test";
    let statement = magic_market_core::FinancialStatement {
        instrument: InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            "600519",
            magic_market_core::AssetClass::Equity,
        )
        .unwrap(),
        kind: StatementKind::Income,
        report_period: IsoDate::new("2026-03-31").unwrap(),
        fiscal_period: Some(NonEmptyText::new("Q1").unwrap()),
        announced_on: Some(IsoDate::new("2026-04-30").unwrap()),
        currency: Some(NonEmptyText::new("CNY").unwrap()),
        lines: Vec::new(),
        evidence: SourceEvidence::new(ProviderId::Tonghuashun, observed_at, batch_id)
            .unwrap()
            .with_source_at("unix-ms:1777478400000")
            .unwrap(),
    };
    let batch = DataBatch::strict(
        vec![statement],
        Provenance::new("HithinkFinance", observed_at)
            .unwrap()
            .with_source_at("unix-ms:1777478400000")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    );

    let v1 =
        financial_statements_query_result(batch.clone(), "HithinkFinance", SCHEMA_VERSION, 4096)
            .unwrap();
    let v2 = financial_statements_query_result(
        batch.clone(),
        "HithinkFinance",
        FINANCIAL_STATEMENTS_SCHEMA_VERSION,
        4096,
    )
    .unwrap();
    assert!(matches!(
        financial_statements_query_result(batch, "HithinkFinance", 3, 4096),
        Err(ServiceError::InvalidRequest(message)) if message.contains("version 1 or 2")
    ));

    assert_eq!(v1.records[0].schema_version(), SCHEMA_VERSION);
    let v1_record: serde_json::Value = serde_json::from_slice(v1.records[0].data()).unwrap();
    assert!(v1_record.get("fiscal_period").is_none());
    assert_eq!(v2.records[0].schema_version(), 2);
    let v2_record: serde_json::Value = serde_json::from_slice(v2.records[0].data()).unwrap();
    assert_eq!(v2_record["fiscal_period"], "Q1");
}

#[test]
fn hithink_quote_projection_marks_missing_record_source_time() {
    let observed_at = "1789453335.543000000";
    let batch_id = "hithink-quote-test";
    let quote = Quote::from_parts(
        InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            "600396",
            magic_market_core::AssetClass::Equity,
        )
        .unwrap(),
        None,
        magic_market_core::Price::new(13.64).unwrap(),
        Some(magic_market_core::Price::new(13.40).unwrap()),
        Some(magic_market_core::Price::new(13.45).unwrap()),
        Some(magic_market_core::Price::new(13.75).unwrap()),
        Some(magic_market_core::Price::new(13.40).unwrap()),
        Some(magic_market_core::Ratio::new(1.79, magic_market_core::RatioUnit::Percent).unwrap()),
        magic_market_core::Quantity::new(1234.0).unwrap(),
        Some(magic_market_core::Money::new(1_680_000.0).unwrap()),
        DataStatus::Unavailable,
        None,
        observed_at,
        ProviderId::Tonghuashun,
        batch_id,
    )
    .unwrap();
    let batch = DataBatch::strict(
        vec![quote],
        Provenance::new("HithinkFinance", observed_at)
            .unwrap()
            .with_source_at("unix-ms:1789453334000")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    );

    let result =
        provider_query_result(batch, "HithinkFinance", REALTIME_QUOTES_RECORD_SCHEMA, 4096)
            .unwrap();

    assert_eq!(result.provider, "HithinkFinance");
    assert_eq!(result.source_at.as_deref(), Some("unix-ms:1789453334000"));
    assert!(result.complete);
    let record: serde_json::Value = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(record["provider"], "Tonghuashun");
    assert_eq!(record["status"], "Unavailable");
    assert!(record["source_at"].is_null());
    assert_eq!(record["observed_at"], observed_at);
}

#[test]
fn jin10_economic_calendar_is_diagnostic_after_public_calendar_retirement() {
    let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
    let calendar = registry
        .capabilities()
        .into_iter()
        .find(|capability| {
            capability.operation == Operation::EconomicCalendar && capability.provider == "Jin10"
        })
        .unwrap();

    assert!(!calendar.repository_admitted);
    assert!(!calendar.runtime_available);
    assert!(calendar.diagnostic_available);
    assert!(calendar
        .blocker
        .as_deref()
        .is_some_and(|blocker| blocker.contains("2025-12-01")));
}

#[test]
fn jin10_release_observation_verified_empty_is_admitted_only_for_the_same_request() {
    let observed_at = "2026-09-12T12:00:00+08:00";
    let batch_id = "jin10:test:economic-release-observations";
    let request_identity = r#"{"limit":20,"country":null}"#;
    let empty = magic_market_core::VerifiedEmpty::new(
        "economic_release_observations",
        request_identity,
        "current public flash window contains no eligible type-1 rows",
        SourceEvidence::new(ProviderId::Jin10, observed_at, batch_id).unwrap(),
        Provenance::new("jin10-flash-v1", observed_at)
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    )
    .unwrap();

    let admitted = economic_release_observations_query_result(
        Err(Jin10Error::VerifiedEmpty(Box::new(empty.clone()))),
        request_identity,
        4096,
    )
    .unwrap();
    assert!(admitted.complete);
    assert!(admitted.records.is_empty());
    assert_eq!(admitted.source_at, None);
    assert!(admitted.repository_admitted);

    assert!(matches!(
        economic_release_observations_query_result(
            Err(Jin10Error::VerifiedEmpty(Box::new(empty))),
            r#"{"limit":10,"country":null}"#,
            4096,
        ),
        Err(ServiceError::InvalidEvidence { evidence_code, .. })
            if evidence_code == "empty_request_identity_mismatch"
    ));
}

#[test]
fn securities_times_global_news_is_diagnostic_after_source_contract_drift() {
    let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
    let news = registry
        .capabilities()
        .into_iter()
        .find(|capability| {
            capability.operation == Operation::GlobalNews
                && capability.provider == "SecuritiesTimes"
        })
        .unwrap();

    assert!(!news.repository_admitted);
    assert!(!news.runtime_available);
    assert!(news.diagnostic_available);
    assert!(news
        .blocker
        .as_deref()
        .is_some_and(|blocker| blocker.contains("source")));
}

#[test]
fn emquant_production_scope_requires_daily_explicit_range() {
    let instrument = InstrumentId::new(
        magic_market_core::Exchange::Shanghai,
        "600396",
        AssetClass::Equity,
    )
    .unwrap();
    let valid = BarsRequest::new(instrument.clone(), BarInterval::Day, 5)
        .unwrap()
        .with_range("2026-08-14", "2026-08-20")
        .unwrap();
    assert!(validate_emquant_daily_bars_request(&valid).is_ok());

    let missing_range = BarsRequest::new(instrument.clone(), BarInterval::Day, 5).unwrap();
    assert!(validate_emquant_daily_bars_request(&missing_range).is_err());

    let weekly = BarsRequest::new(instrument.clone(), BarInterval::Week, 5)
        .unwrap()
        .with_range("2026-07-01", "2026-08-20")
        .unwrap();
    assert!(validate_emquant_daily_bars_request(&weekly).is_err());

    let oversized = BarsRequest::new(instrument, BarInterval::Day, 801)
        .unwrap()
        .with_range("2020-01-01", "2026-08-20")
        .unwrap();
    assert!(validate_emquant_daily_bars_request(&oversized).is_err());
}

#[test]
fn quote_handler_returns_tencent_core_records() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = TencentClient::with_transport(StaticTransport {
        calls: calls.clone(),
    });
    let registry = registry_with_tencent(client, Duration::from_secs(1), 4096).unwrap();
    let result = registry
        .execute(command(REALTIME_QUOTES_REQUEST_SCHEMA, Some("Tencent")))
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(result.provider, TENCENT_PROVIDER);
    assert!(result.complete);
    assert_eq!(result.records.len(), 1);
    let quote: Quote = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(quote.instrument().code(), "600396");
    assert_eq!(quote.provider(), magic_market_core::ProviderId::Tencent);
}

#[test]
fn index_quotes_enforce_typed_identity_and_strict_freshness() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = TencentClient::with_transport(IndexTransport {
        calls: calls.clone(),
    });
    let registry = registry_with_tencent(client, Duration::from_secs(1), 4096).unwrap();
    let index_request = serde_json::to_vec(&serde_json::json!({
        "indices": [{
            "exchange": "Shanghai",
            "code": "000001",
            "asset_class": "Index"
        }],
        "maximum_source_age_millis": 315_576_000_000_u64
    }))
    .unwrap();
    let command = QueryCommand::new(
        "index-quotes-1",
        Operation::IndexQuotes,
        Some(TENCENT_PROVIDER.to_owned()),
        CanonicalPayload::new(
            INDEX_QUOTES_REQUEST_SCHEMA,
            SCHEMA_VERSION,
            index_request,
            4096,
        )
        .unwrap(),
    )
    .unwrap();
    let result = registry.execute(command).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(result.repository_admitted);
    assert_eq!(result.records.len(), 1);
    let quote: Quote = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(quote.instrument().asset_class(), AssetClass::Index);

    let equity_request = serde_json::to_vec(&serde_json::json!({
        "indices": [{
            "exchange": "Shanghai",
            "code": "600396",
            "asset_class": "Equity"
        }],
        "maximum_source_age_millis": 5000
    }))
    .unwrap();
    let command = QueryCommand::new(
        "index-quotes-2",
        Operation::IndexQuotes,
        Some(TENCENT_PROVIDER.to_owned()),
        CanonicalPayload::new(
            INDEX_QUOTES_REQUEST_SCHEMA,
            SCHEMA_VERSION,
            equity_request,
            4096,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        registry.execute(command),
        Err(ServiceError::InvalidRequest(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn index_quote_route_failure_retains_its_safe_provider_attempt_trace() {
    let client = TencentClient::with_transport(FailingIndexTransport);
    let registry = registry_with_tencent(client, Duration::from_secs(1), 4096).unwrap();
    let payload = serde_json::to_vec(&serde_json::json!({
        "indices": [{
            "exchange": "Shanghai",
            "code": "000001",
            "asset_class": "Index"
        }],
        "maximum_source_age_millis": 5000
    }))
    .unwrap();
    let command = QueryCommand::new(
        "index-quotes-route-failure",
        Operation::IndexQuotes,
        Some(TENCENT_PROVIDER.to_owned()),
        CanonicalPayload::new(INDEX_QUOTES_REQUEST_SCHEMA, SCHEMA_VERSION, payload, 4096).unwrap(),
    )
    .unwrap();
    let error = registry.execute(command).unwrap_err();
    let ServiceError::ProviderRouteFailure {
        operation,
        exhausted,
        attempts,
    } = error
    else {
        panic!("expected typed provider route failure");
    };
    assert_eq!(operation, Operation::IndexQuotes);
    assert!(exhausted);
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].provider(), "Tencent");
    assert_eq!(attempts[0].outcome(), "failed");
    assert_eq!(attempts[0].reason_code(), "transport");
    assert!(attempts[0].retryable());
    assert!(!attempts[0].terminal());
}

#[test]
fn intraday_shape_uses_one_ordered_minute_series_and_deterministic_arithmetic() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = TencentClient::with_transport(ShapeTransport {
        calls: calls.clone(),
    });
    let registry = registry_with_tencent(client, Duration::from_secs(1), 65_536).unwrap();
    let request = IntradayShapeRequest::new(
        InstrumentId::new(
            magic_market_core::Exchange::Shanghai,
            "600396",
            AssetClass::Equity,
        )
        .unwrap(),
        Some(IsoDate::new("2026-07-23").unwrap()),
        PositiveU32::new(800).unwrap(),
    )
    .unwrap();
    let command = QueryCommand::new(
        "intraday-shape-1",
        Operation::IntradayShape,
        Some("LocalAnalysis".into()),
        CanonicalPayload::new(
            INTRADAY_SHAPE_REQUEST_SCHEMA,
            SCHEMA_VERSION,
            serde_json::to_vec(&request).unwrap(),
            65_536,
        )
        .unwrap(),
    )
    .unwrap();
    let result = registry.execute(command).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.provider, "LocalAnalysis");
    let record: IntradayShapeRecord = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(record.point_count().get(), 3);
    assert_eq!(record.trading_date().as_str(), "2026-07-23");
    assert_eq!(record.input_evidence().len(), 1);
}

#[test]
fn upper_limit_pool_review_is_one_atomic_checked_four_family_record() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = EastmoneyClient::with_transport(LimitPoolTransport {
        calls: calls.clone(),
    });
    let request = UpperLimitPoolReviewRequest::new(
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    let command = QueryCommand::new(
        "upper-review-1",
        Operation::UpperLimitPoolReview,
        Some("Eastmoney".into()),
        CanonicalPayload::new(
            UPPER_LIMIT_POOL_REVIEW_REQUEST_SCHEMA,
            SCHEMA_VERSION,
            serde_json::to_vec(&request).unwrap(),
            65_536,
        )
        .unwrap(),
    )
    .unwrap();
    let result = execute_upper_limit_pool_review(&client, command, 65_536).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.provider, "Eastmoney");
    assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
    assert_eq!(result.records.len(), 1);
    let record: UpperLimitPoolReviewRecord =
        serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(record.upper().len(), 1);
    assert_eq!(record.broken().len(), 1);
    assert_eq!(record.lower().len(), 1);
    assert_eq!(record.previous_upper().len(), 1);
    assert_eq!(record.maximum_streak(), Some(3));
    assert_eq!(record.input_evidence().len(), 4);
    assert_eq!(record.input_digest_sha256().len(), 64);
}

#[test]
fn metadata_handler_preserves_explicit_incomplete_fields() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = TencentClient::with_transport(StaticTransport {
        calls: calls.clone(),
    });
    let registry = registry_with_tencent(client, Duration::from_secs(1), 4096).unwrap();
    let result = registry
        .execute(command_for(
            Operation::SecurityMetadata,
            SECURITY_METADATA_REQUEST_SCHEMA,
            Some("Tencent"),
        ))
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!result.complete);
    assert_eq!(result.records.len(), 1);
    let metadata: SecurityMetadata = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(metadata.name(), Some("ABC"));
    assert!(metadata.listed_on().is_none());
    assert!(metadata.price_limit().version().is_none());
}

fn hithink_metadata_registry(row: serde_json::Value) -> OperationRegistry {
    let body = serde_json::to_vec(&serde_json::json!({
        "code": 0, "message": "success", "request_id": "native-metadata-fixture",
        "data": {"timestamp": 1716105600000_i64, "item": [row]}
    }))
    .unwrap();
    let client = HithinkClient::with_transport(
        "test_key",
        HistoricalHithinkTransport {
            body,
            calls: Arc::new(AtomicUsize::new(0)),
        },
    )
    .unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline fixture");
    register_hithink_security_metadata(&mut registry, Arc::new(client), 4096).unwrap();
    registry
}

fn hithink_metadata_row() -> serde_json::Value {
    serde_json::json!({
        "thscode": "600396.SH", "ticker": "600396", "name": "metadata fixture",
        "exchange": "SH", "asset_type": "a-share", "currency": "CNY"
    })
}

#[test]
fn hithink_metadata_handler_keeps_v1_projection_for_old_null_and_value_dates() {
    for dates in [
        None,
        Some(serde_json::Value::Null),
        Some(serde_json::json!("2007-03-01")),
    ] {
        let mut row = hithink_metadata_row();
        if let Some(value) = dates {
            for field in [
                "list_date",
                "end_date",
                "last_trade_date",
                "last_delivery_date",
            ] {
                row[field] = value.clone();
            }
        }
        let registry = hithink_metadata_registry(row);
        let result = registry
            .execute(command_for(
                Operation::SecurityMetadata,
                SECURITY_METADATA_REQUEST_SCHEMA,
                Some("HithinkFinance"),
            ))
            .unwrap();
        assert!(result.repository_admitted);
        assert!(result.complete);
        assert_eq!(result.provider, "HithinkFinance");
        assert_eq!(result.records.len(), 1);
        let payload = &result.records[0];
        assert_eq!(payload.schema(), "magic.market.security_metadata");
        assert_eq!(payload.schema_version(), 1);
        let metadata: SecurityMetadata = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(metadata.name(), Some("metadata fixture"));
        assert!(metadata.listed_on().is_none());
        assert_eq!(metadata.status(), DataStatus::Unavailable);
        assert_eq!(metadata.source_at(), Some("unix-ms:1716105600000"));
        assert!(registry.capabilities().iter().all(|capability| {
            !capability.exact_scope.contains("TEST_CODE_SYNTHETIC")
                && !capability
                    .exact_scope
                    .contains("ordinary_daily_change_window")
        }));
    }
}

#[test]
fn hithink_metadata_handler_rejects_bad_native_dates_without_partial_results() {
    for value in [serde_json::json!("2026-02-29"), serde_json::json!(20260228)] {
        let mut row = hithink_metadata_row();
        row["list_date"] = value;
        let result = hithink_metadata_registry(row).execute(command_for(
            Operation::SecurityMetadata,
            SECURITY_METADATA_REQUEST_SCHEMA,
            Some("HithinkFinance"),
        ));
        assert!(matches!(
            result,
            Err(ServiceError::ProviderFailure {
                kind: ProviderFailureKind::ResponseInvalid,
                ..
            })
        ));
    }
}

#[test]
fn security_profile_schema_rejects_before_tdx_io() {
    let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
    assert!(matches!(
        registry.execute(command_for(
            Operation::SecurityProfiles,
            "wrong.schema",
            Some("Tdx"),
        )),
        Err(ServiceError::InvalidRequest(_))
    ));
}

#[test]
fn coded_tdx_connectivity_errors_are_retryable_provider_outages() {
    for value in 2001..=2006 {
        let error = magic_tdx_rs::error_codes::ErrorCode::from_code(value)
            .unwrap()
            .err("transport fixture");
        assert!(
            matches!(
                map_tdx_error(Operation::SecurityProfiles, &error),
                ServiceError::Unavailable {
                    operation: Operation::SecurityProfiles,
                    ..
                }
            ),
            "TDX connectivity error E{value} must remain retryable"
        );
    }
}

#[test]
fn diagnostics_require_opt_in_and_exact_schema_while_absent_families_stay_blocked() {
    let registry = production_operation_registry(Duration::from_secs(1), 4096).unwrap();
    let technical = command_for(Operation::TechnicalBars, "wrong.schema", Some("Baidu"));
    assert!(matches!(
        registry.execute(technical),
        Err(ServiceError::InvalidRequest(_))
    ));

    let auction =
        command_for(Operation::Auctions, "wrong.schema", Some("Tdx")).with_unadmitted_access(true);
    assert!(matches!(
        registry.execute(auction),
        Err(ServiceError::Unsupported { .. })
    ));
}

#[test]
fn wrong_schema_and_provider_fail_before_provider_io() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = TencentClient::with_transport(StaticTransport {
        calls: calls.clone(),
    });
    let registry = registry_with_tencent(client, Duration::from_secs(1), 4096).unwrap();
    assert!(matches!(
        registry.execute(command("wrong.schema", None)),
        Err(ServiceError::InvalidRequest(_))
    ));
    assert!(matches!(
        registry.execute(command(
            REALTIME_QUOTES_REQUEST_SCHEMA,
            Some("NotRegistered")
        )),
        Err(ServiceError::Unsupported { .. })
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn provider_failures_preserve_retry_and_precondition_categories() {
    assert!(matches!(
        provider_error(
            Operation::MarketStatistics,
            HithinkError::Business {
                code: 3002,
                request_id: "hithink-request".into()
            }
        ),
        ServiceError::ProviderFailure {
            provider,
            kind: ProviderFailureKind::Unavailable,
            provider_reason,
            ..
        } if provider == "HithinkFinance"
            && provider_reason == "code=3002 request_id=hithink-request"
    ));
    assert!(matches!(
        provider_error(
            Operation::Popularity,
            HithinkError::Authentication {
                code: 2003,
                request_id: "hithink-auth".into()
            }
        ),
        ServiceError::ProviderFailure {
            kind: ProviderFailureKind::AuthenticationRejected,
            provider_reason,
            ..
        } if provider_reason == "code=2003 request_id=hithink-auth"
    ));
    assert!(matches!(
        provider_error(
            Operation::Auctions,
            HithinkError::NotReady {
                request_id: "hithink-not-ready".into()
            }
        ),
        ServiceError::ProviderFailure {
            kind: ProviderFailureKind::Unavailable,
            provider_reason,
            ..
        } if provider_reason == "category=not_ready request_id=hithink-not-ready"
    ));
    // A rejected Fuyao status keeps its own classification. Folding these into
    // `provider_unavailable` is what made a throttle look like an outage.
    assert!(matches!(
        provider_error(Operation::MarketStatistics, HithinkError::HttpStatus(429)),
        ServiceError::ProviderFailure {
            provider,
            kind: ProviderFailureKind::RateLimited,
            provider_reason,
            ..
        } if provider == "HithinkFinance" && provider_reason == "http_status=429"
    ));
    assert!(matches!(
        provider_error(Operation::MarketStatistics, HithinkError::HttpStatus(401)),
        ServiceError::ProviderFailure {
            kind: ProviderFailureKind::AuthenticationRejected,
            provider_reason,
            ..
        } if provider_reason == "http_status=401"
    ));
    assert!(matches!(
        provider_error(Operation::MarketStatistics, HithinkError::HttpStatus(403)),
        ServiceError::ProviderFailure {
            kind: ProviderFailureKind::AuthenticationRejected,
            provider_reason,
            ..
        } if provider_reason == "http_status=403"
    ));
    assert!(matches!(
        provider_error(Operation::MarketStatistics, HithinkError::HttpStatus(503)),
        ServiceError::ProviderFailure {
            kind: ProviderFailureKind::Unavailable,
            provider_reason,
            ..
        } if provider_reason == "http_status=503"
    ));
    assert!(matches!(
        provider_error(Operation::MarketStatistics, HithinkError::HttpStatus(404)),
        ServiceError::ProviderFailure {
            kind: ProviderFailureKind::QueryRejected,
            provider_reason,
            ..
        } if provider_reason == "http_status=404"
    ));
    assert!(matches!(
        provider_error(
            Operation::HistoricalBars,
            EmQuantError::Bridge("10001004 EQERR_ACCESS_EXPIRE".into())
        ),
        ServiceError::Unavailable {
            operation: Operation::HistoricalBars,
            ..
        }
    ));
    assert!(matches!(
        provider_error(
            Operation::GlobalIndices,
            SinaError::Transport("temporary TLS EOF".into())
        ),
        ServiceError::Unavailable {
            operation: Operation::GlobalIndices,
            ..
        }
    ));
    assert!(matches!(
        provider_error(
            Operation::GlobalNews,
            EastmoneyError::Protocol("unexpected article host".into())
        ),
        ServiceError::FailedPrecondition(_)
    ));
    assert!(matches!(
        provider_error(Operation::BoardDirectory, TdxError::ConnectionTimeout),
        ServiceError::Unavailable {
            operation: Operation::BoardDirectory,
            ..
        }
    ));
    assert!(matches!(
        provider_error(
            Operation::GlobalNews,
            ClsError::ProviderRejected {
                errno: 1001,
                message: "bad sign".into(),
            }
        ),
        ServiceError::ProviderFailure {
            operation: Operation::GlobalNews,
            provider,
            kind: ProviderFailureKind::QueryRejected,
            provider_reason,
        } if provider == "Cailianpress"
            && provider_reason == "errno=1001 message=bad sign"
    ));
    assert!(matches!(
        provider_error(Operation::GlobalNews, ClsError::HttpStatus(429)),
        ServiceError::ProviderFailure {
            provider,
            kind: ProviderFailureKind::RateLimited,
            provider_reason,
            ..
        } if provider == "Cailianpress" && provider_reason == "http_status=429"
    ));
    assert!(matches!(
        provider_error(
            Operation::GlobalNews,
            ClsError::Protocol("telegraph row identity changed".into())
        ),
        ServiceError::ProviderFailure {
            provider,
            kind: ProviderFailureKind::ResponseInvalid,
            provider_reason,
            ..
        } if provider == "Cailianpress"
            && provider_reason == "category=protocol message=telegraph row identity changed"
    ));
}
