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
