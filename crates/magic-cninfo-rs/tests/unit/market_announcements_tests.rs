use super::*;
use crate::{CninfoConfig, CninfoTransport, HttpResponse};
use magic_market_core::{IsoDate, PositiveU32};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
struct SequenceTransport {
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl SequenceTransport {
    fn new(responses: Vec<serde_json::Value>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(
                responses
                    .into_iter()
                    .map(|value| serde_json::to_vec(&value).unwrap())
                    .collect(),
            )),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl CninfoTransport for SequenceTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError> {
        self.requests.lock().unwrap().push(request.clone());
        let body = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| CninfoError::Transport("fixture response exhausted".into()))?;
        Ok(HttpResponse {
            status: 200,
            final_url: request.url.clone(),
            content_type: Some("application/json".into()),
            body,
        })
    }
}

fn request(limit: u32) -> MarketAnnouncementRequest {
    MarketAnnouncementRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(limit).unwrap(),
    )
    .unwrap()
}

fn row(id: &str, title: &str) -> serde_json::Value {
    serde_json::json!({
        "secCode": "600396",
        "orgId": "gssh0600396",
        "announcementId": id,
        "announcementTitle": title,
        "announcementTime": 1784822400000_i64,
        "adjunctUrl": format!("finalpage/2026-07-24/{id}.PDF"),
        "pageColumn": "SHMB"
    })
}

fn page(
    total: u64,
    total_pages: u64,
    has_more: bool,
    rows: Vec<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "totalAnnouncement": total,
        "totalRecordNum": total,
        "totalpages": total_pages,
        "hasMore": has_more,
        "announcements": rows
    })
}

fn client(transport: impl CninfoTransport + 'static) -> CninfoClient {
    CninfoClient::from_parts(Duration::ZERO, CninfoConfig::default(), Arc::new(transport))
}

#[test]
fn complete_pages_continue_until_unique_limit_or_declared_total() {
    let first_rows = (0..30)
        .map(|index| row(&format!("id-{index:02}"), &format!("title {index:02}")))
        .collect();
    let duplicate = row("id-29", "title 29");
    let transport = SequenceTransport::new(vec![
        page(31, 1, true, first_rows),
        page(31, 1, false, vec![duplicate]),
    ]);
    let requests = transport.requests.clone();

    let batch = client(transport)
        .market_announcements(&request(31))
        .unwrap();

    assert_eq!(batch.records().len(), 30);
    assert_eq!(batch.records()[0].announcement_id.as_str(), "id-00");
    assert_eq!(batch.records()[29].announcement_id.as_str(), "id-29");
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(String::from_utf8_lossy(&requests[1].body).contains("pageNum=2"));
    assert!(
        !batch.quality().is_complete(),
        "overlap cannot prove complete source coverage"
    );
}

#[test]
fn unexhausted_market_prefix_must_not_claim_complete() {
    let rows = (0..30)
        .map(|index| row(&format!("id-{index:02}"), &format!("title {index:02}")))
        .collect();
    let batch = client(SequenceTransport::new(vec![page(31, 1, true, rows)]))
        .market_announcements(&request(30))
        .unwrap();

    assert_eq!(batch.records().len(), 30);
    assert!(
        !batch.quality().is_complete(),
        "30 of 31 is not complete coverage"
    );
    assert!(!batch.quality().issues().is_empty());
}

#[test]
fn caller_truncation_must_not_claim_complete_even_after_source_exhaustion() {
    let batch = client(SequenceTransport::new(vec![page(
        2,
        0,
        false,
        vec![row("id-00", "title 00"), row("id-01", "title 01")],
    )]))
    .market_announcements(&request(1))
    .unwrap();

    assert_eq!(batch.records().len(), 1);
    assert!(
        !batch.quality().is_complete(),
        "a caller-truncated exhausted page is not complete output"
    );
}

#[test]
fn caller_limit_does_not_hide_an_invalid_row_on_the_complete_source_page() {
    let invalid = serde_json::json!({
        "secCode": "600396",
        "orgId": "gssh0600396",
        "announcementId": "invalid-board",
        "announcementTitle": "unknown board",
        "announcementTime": 1784822400000_i64,
        "pageColumn": "UNKNOWN"
    });
    let error = client(SequenceTransport::new(vec![page(
        2,
        0,
        false,
        vec![row("valid", "valid"), invalid],
    )]))
    .market_announcements(&request(1))
    .unwrap_err();

    assert!(matches!(
        error,
        CninfoError::Unsupported(message) if message.contains("pageColumn")
    ));
}

#[test]
fn coverage_retains_all_ten_page_hashes_for_the_300_of_722_prefix() {
    let documents = (0..10)
        .map(|page_index| {
            let rows = (page_index * 30..page_index * 30 + 30)
                .map(|index| row(&format!("id-{index:03}"), &format!("title {index:03}")))
                .collect();
            page(722, 24, true, rows)
        })
        .collect::<Vec<_>>();
    let transport = SequenceTransport::new(documents.clone());
    let requests = transport.requests.clone();
    let outcome = client(transport)
        .market_announcements_with_coverage(&request(300))
        .unwrap();
    let coverage = outcome.coverage();

    assert!(!outcome.batch().quality().is_complete());
    assert_eq!(coverage.source_total, 722);
    assert_eq!(coverage.expected_request_pages, 25);
    assert_eq!(coverage.pages_read, 10);
    assert_eq!(coverage.inspected_raw_rows, 300);
    assert_eq!(coverage.unique_rows, 300);
    assert_eq!(coverage.returned_rows, 300);
    assert!(!coverage.source_exhausted);
    assert!(coverage.terminal_has_more);
    assert!(!coverage.caller_limit_truncated);
    assert!(!coverage.verified_empty);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 10);
    for (index, evidence) in coverage.pages.iter().enumerate() {
        assert_eq!(evidence.requested_page, index as u32 + 1);
        assert_eq!(evidence.source_total, 722);
        assert_eq!(evidence.source_total_pages, 24);
        assert_eq!(evidence.row_count, 30);
        assert!(evidence.has_more);
        let raw = serde_json::to_vec(&documents[index]).unwrap();
        assert_eq!(evidence.response_bytes, raw.len() as u64);
        assert_eq!(
            evidence.response_body_sha256,
            format!("{:x}", Sha256::digest(raw))
        );
        assert_eq!(
            evidence.request_body_sha256,
            format!("{:x}", Sha256::digest(&requests[index].body))
        );
    }
    let batch_id = outcome.batch().provenance().batch_id().unwrap();
    assert!(batch_id.contains("total=722:limit=300:raw=300:unique=300:returned=300"));
    for record in outcome.batch().records() {
        assert_eq!(record.evidence.batch_id(), batch_id);
        assert_eq!(
            record.evidence.observed_at(),
            outcome.batch().provenance().fetched_at()
        );
    }
}

#[test]
fn complete_source_pagination_and_verified_empty_have_explicit_terminals() {
    let first_rows = (0..30)
        .map(|index| row(&format!("id-{index:02}"), &format!("title {index:02}")))
        .collect();
    let outcome = client(SequenceTransport::new(vec![
        page(31, 1, true, first_rows),
        page(31, 1, false, vec![row("id-30", "title 30")]),
    ]))
    .market_announcements_with_coverage(&request(31))
    .unwrap();
    assert!(outcome.batch().quality().is_complete());
    assert!(outcome.coverage().source_exhausted);
    assert!(!outcome.coverage().terminal_has_more);
    assert!(!outcome.coverage().caller_limit_truncated);
    assert_eq!(outcome.coverage().equivalent_duplicate_rows, 0);
    assert_eq!(outcome.coverage().returned_rows, 31);
    assert_eq!(outcome.coverage().pages_read, 2);

    let empty = client(SequenceTransport::new(vec![page(0, 0, false, Vec::new())]))
        .market_announcements_with_coverage(&request(3))
        .unwrap();
    assert!(empty.batch().quality().is_complete());
    assert!(empty.coverage().verified_empty);
    assert!(empty.coverage().source_exhausted);
    assert!(!empty.coverage().terminal_has_more);
    assert_eq!(empty.coverage().expected_request_pages, 1);
    assert_eq!(empty.coverage().pages[0].source_total_pages, 0);
    assert_eq!(empty.coverage().pages_read, 1);
    assert_eq!(empty.coverage().inspected_raw_rows, 0);
    assert!(empty.batch().provenance().source_at().is_none());
}

#[test]
fn coverage_distinguishes_exhaustion_from_limit_truncation_and_overlap() {
    let truncated = client(SequenceTransport::new(vec![page(
        2,
        0,
        false,
        vec![row("id-00", "title 00"), row("id-01", "title 01")],
    )]))
    .market_announcements_with_coverage(&request(1))
    .unwrap();
    assert!(truncated.coverage().source_exhausted);
    assert!(truncated.coverage().caller_limit_truncated);
    assert_eq!(truncated.coverage().unique_rows, 2);
    assert_eq!(truncated.coverage().returned_rows, 1);
    assert!(!truncated.batch().quality().is_complete());

    let overlap = client(SequenceTransport::new(vec![page(
        2,
        0,
        false,
        vec![row("id-00", "title 00"), row("id-00", "title 00")],
    )]))
    .market_announcements_with_coverage(&request(2))
    .unwrap();
    assert!(overlap.coverage().source_exhausted);
    assert!(!overlap.coverage().caller_limit_truncated);
    assert_eq!(overlap.coverage().equivalent_duplicate_rows, 1);
    assert_eq!(overlap.coverage().inspected_raw_rows, 2);
    assert_eq!(overlap.coverage().unique_rows, 1);
    assert!(!overlap.batch().quality().is_complete());
}

#[test]
fn legacy_shenzhen_main_board_column_is_a_verified_equity_board() {
    let mut legacy_main_board = row("szzb", "深圳主板公告");
    legacy_main_board["secCode"] = serde_json::json!("000001");
    legacy_main_board["orgId"] = serde_json::json!("gssz0000001");
    legacy_main_board["pageColumn"] = serde_json::json!("SZZB");

    let batch = client(SequenceTransport::new(vec![page(
        1,
        0,
        false,
        vec![legacy_main_board],
    )]))
    .market_announcements(&request(1))
    .expect("SZZB is a verified Shenzhen main-board identity");

    assert_eq!(batch.records()[0].instrument.code(), "000001");
    assert_eq!(batch.records()[0].instrument.exchange(), Exchange::Shenzhen);
}

#[test]
fn cninfo_star_board_column_is_a_verified_equity_board() {
    let mut star_board = row("shkcb", "科创板公告");
    star_board["secCode"] = serde_json::json!("688001");
    star_board["orgId"] = serde_json::json!("gssh0688001");
    star_board["pageColumn"] = serde_json::json!("SHKCB");

    let batch = client(SequenceTransport::new(vec![page(
        1,
        0,
        false,
        vec![star_board],
    )]))
    .market_announcements(&request(1))
    .expect("SHKCB is a verified STAR Market identity");

    assert_eq!(batch.records()[0].instrument.code(), "688001");
    assert_eq!(batch.records()[0].instrument.exchange(), Exchange::Shanghai);
}

#[test]
fn cninfo_shanghai_main_board_column_is_a_verified_equity_board() {
    let mut main_board = row("shzb", "上海主板公告");
    main_board["pageColumn"] = serde_json::json!("SHZB");

    let batch = client(SequenceTransport::new(vec![page(
        1,
        0,
        false,
        vec![main_board],
    )]))
    .market_announcements(&request(1))
    .expect("SHZB is a verified Shanghai main-board identity");

    assert_eq!(batch.records()[0].instrument.code(), "600396");
    assert_eq!(batch.records()[0].instrument.exchange(), Exchange::Shanghai);
}

#[test]
fn conflicting_duplicate_identity_fails_the_atomic_batch() {
    let first_rows = (0..30)
        .map(|index| row(&format!("id-{index:02}"), &format!("title {index:02}")))
        .collect();
    let transport = SequenceTransport::new(vec![
        page(31, 1, true, first_rows),
        page(31, 1, false, vec![row("id-29", "changed title")]),
    ]);

    let error = client(transport)
        .market_announcements(&request(31))
        .unwrap_err();

    assert!(matches!(
        error,
        CninfoError::Schema(message) if message.contains("conflicting")
    ));
}

#[test]
fn pagination_totals_and_page_boundaries_must_remain_complete() {
    let first_rows = (0..30)
        .map(|index| row(&format!("id-{index:02}"), &format!("title {index:02}")))
        .collect();
    let drift = SequenceTransport::new(vec![
        page(31, 1, true, first_rows),
        page(
            32,
            1,
            false,
            vec![row("id-30", "title 30"), row("id-31", "title 31")],
        ),
    ]);
    assert!(matches!(
        client(drift).market_announcements(&request(31)),
        Err(CninfoError::Incomplete(message)) if message.contains("total changed")
    ));

    let short_page =
        SequenceTransport::new(vec![page(2, 0, false, vec![row("id-00", "title 00")])]);
    assert!(matches!(
        client(short_page).market_announcements(&request(1)),
        Err(CninfoError::Incomplete(message)) if message.contains("expected 2")
    ));

    let wrong_has_more =
        SequenceTransport::new(vec![page(1, 0, true, vec![row("id-00", "title 00")])]);
    assert!(matches!(
        client(wrong_has_more).market_announcements(&request(1)),
        Err(CninfoError::Incomplete(message)) if message.contains("hasMore")
    ));
}

#[test]
fn exact_zero_metadata_is_verified_empty_but_invalid_empty_is_not() {
    let empty = client(SequenceTransport::new(vec![page(0, 0, false, Vec::new())]))
        .market_announcements(&request(3))
        .unwrap();

    assert!(empty.records().is_empty());
    assert!(empty.quality().is_complete());
    assert_eq!(empty.provenance().source(), "cninfo-market");
    assert!(empty.provenance().source_at().is_none());
    assert!(empty.provenance().batch_id().unwrap().contains("total=0"));

    let incomplete = SequenceTransport::new(vec![page(1, 0, false, Vec::new())]);
    assert!(matches!(
        client(incomplete).market_announcements(&request(1)),
        Err(CninfoError::Incomplete(message)) if message.contains("expected 1")
    ));
}

#[test]
fn source_time_must_be_newest_first_and_configured_page_bound_is_terminal() {
    let mut newer_second = row("id-01", "title 01");
    newer_second["announcementTime"] = serde_json::json!(1784822401000_i64);
    let order_error = SequenceTransport::new(vec![page(
        2,
        0,
        false,
        vec![row("id-00", "title 00"), newer_second],
    )]);
    assert!(matches!(
        client(order_error).market_announcements(&request(2)),
        Err(CninfoError::Incomplete(message)) if message.contains("source order")
    ));

    let first_rows = (0..30)
        .map(|index| row(&format!("id-{index:02}"), &format!("title {index:02}")))
        .collect();
    let config = CninfoConfig {
        max_pages: 1,
        ..CninfoConfig::default()
    };
    let bounded = CninfoClient::from_parts(
        Duration::ZERO,
        config,
        Arc::new(SequenceTransport::new(vec![page(31, 1, true, first_rows)])),
    );
    assert!(matches!(
        bounded.market_announcements(&request(31)),
        Err(CninfoError::Incomplete(message)) if message.contains("more than 1")
    ));
}

#[test]
fn source_metadata_boundaries_and_row_identity_fail_closed() {
    let mut conflicting_totals = page(1, 0, false, vec![row("id-00", "title 00")]);
    conflicting_totals["totalRecordNum"] = serde_json::json!(2);
    assert!(matches!(
        client(SequenceTransport::new(vec![conflicting_totals]))
            .market_announcements(&request(1)),
        Err(CninfoError::Incomplete(message)) if message.contains("totals disagree")
    ));

    let wrong_page_total = page(
        31,
        0,
        true,
        (0..30)
            .map(|index| row(&format!("id-{index:02}"), &format!("title {index:02}")))
            .collect(),
    );
    assert!(matches!(
        client(SequenceTransport::new(vec![wrong_page_total]))
            .market_announcements(&request(31)),
        Err(CninfoError::Incomplete(message)) if message.contains("totalpages")
    ));

    let invalid_zero = page(0, 0, true, Vec::new());
    assert!(matches!(
        client(SequenceTransport::new(vec![invalid_zero]))
            .market_announcements(&request(1)),
        Err(CninfoError::Incomplete(message)) if message.contains("zero-total")
    ));

    let beyond_total = validate_market_page(
        serde_json::from_value(page(1, 0, false, vec![row("id-00", "title 00")])).unwrap(),
        2,
        Some(1),
        0,
    );
    assert!(matches!(
        beyond_total,
        Err(CninfoError::Incomplete(message)) if message.contains("beyond")
    ));

    let mut invalid_code = row("bad-code", "invalid code");
    invalid_code["secCode"] = serde_json::json!("60039A");
    assert!(matches!(
        client(SequenceTransport::new(vec![page(
            1,
            0,
            false,
            vec![invalid_code],
        )]))
        .market_announcements(&request(1)),
        Err(CninfoError::Schema(message)) if message.contains("six ASCII digits")
    ));

    let mut outside_range = row("outside", "outside range");
    outside_range["announcementTime"] = serde_json::json!(1784736000000_i64);
    assert!(matches!(
        client(SequenceTransport::new(vec![page(
            1,
            0,
            false,
            vec![outside_range],
        )]))
        .market_announcements(&request(1)),
        Err(CninfoError::Schema(message)) if message.contains("outside the requested range")
    ));
}
