use super::*;
use magic_market_core::{AssetClass, Exchange};
use magic_sina_rs::{DocumentResponse, SnapshotTransport};
use std::collections::HashMap;

const OBSERVED_UNIX: u64 = 1_784_912_800;
const NEWS_ONE: &str = "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml";
const NEWS_TWO: &str = "https://finance.sina.com.cn/roll/2026-07-24/doc-two.shtml";

struct NewsDocuments {
    pages: HashMap<String, DocumentResponse>,
    calls: Arc<AtomicUsize>,
}

impl SnapshotTransport for NewsDocuments {
    fn get(&self, _url: &str) -> Result<Vec<u8>, SinaError> {
        panic!("pinned news handler must use the document transport")
    }

    fn get_document(&self, url: &str) -> Result<DocumentResponse, SinaError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.pages
            .get(url)
            .cloned()
            .ok_or_else(|| SinaError::Transport("offline later news page unavailable".into()))
    }
}

fn page_url(page: u32) -> String {
    format!("https://vip.stock.finance.sina.com.cn/corp/view/vCB_AllNewsStock.php?symbol=sh600396&Page={page}")
}

fn news_page(page: u32, rows: &[(&str, &str, &str)], has_next: bool) -> DocumentResponse {
    let mut html =
        br#"<html><body><script>var page_symbol = "sh600396";</script><div class="datelist"><ul>"#
            .to_vec();
    for (time, url, title) in rows {
        html.extend_from_slice(
            format!("{time}&nbsp;&nbsp;<a href='{url}'>{title}</a><br>").as_bytes(),
        );
    }
    html.extend_from_slice(b"</ul></div><div>");
    // Exact GBK bytes for the source's page marker: 第, decimal page, 页.
    html.extend_from_slice(b"\xb5\xda");
    html.extend_from_slice(page.to_string().as_bytes());
    html.extend_from_slice(b"\xd2\xb3");
    if has_next {
        html.extend_from_slice(format!("<a href='{}'>next</a>", page_url(page + 1)).as_bytes());
    }
    html.extend_from_slice(b"</div></body></html>");
    DocumentResponse::new(200, "text/html; charset=gbk", html, OBSERVED_UNIX)
}

fn news_registry(
    pages: Vec<(u32, DocumentResponse)>,
    maximum_payload_bytes: usize,
) -> (OperationRegistry, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = SinaClient::with_transport(NewsDocuments {
        pages: pages
            .into_iter()
            .map(|(page, value)| (page_url(page), value))
            .collect(),
        calls: calls.clone(),
    });
    let mut registry = OperationRegistry::all_unadmitted("offline news fixture only");
    register_sina_parity(&mut registry, client, maximum_payload_bytes).unwrap();
    (registry, calls)
}

fn news_command(data: serde_json::Value) -> QueryCommand {
    fixture_command(
        Operation::InstrumentNews,
        "Sina",
        INSTRUMENT_NEWS_REQUEST_SCHEMA,
        2,
        &serde_json::to_vec(&data).unwrap(),
    )
}

fn request(cutoff: &str) -> serde_json::Value {
    serde_json::json!({
        "instrument": InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
        "limit": 2, "captured_through": cutoff
    })
}

#[test]
fn registered_sina_news_preserves_source_fields_after_cutoff_without_inventing_body() {
    let (registry, calls) = news_registry(
        vec![(
            1,
            news_page(
                1,
                &[
                    ("2026-07-24 14:00", NEWS_ONE, "Newer source title"),
                    ("2026-07-24 12:00", NEWS_TWO, "Retained source title"),
                ],
                false,
            ),
        )],
        65_536,
    );
    let result = registry
        .execute(news_command(request("2026-07-24T13:00:00+08:00")))
        .unwrap();
    assert!(result.complete && result.repository_admitted);
    assert_eq!(result.provider, "Sina");
    assert_eq!(
        result.source_at.as_deref(),
        Some("2026-07-24T12:00:00+08:00")
    );
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].schema(), GLOBAL_NEWS_RECORD_SCHEMA);
    assert_eq!(result.records[0].schema_version(), 2);
    let record: serde_json::Value = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(record["title"], "Retained source title");
    assert_eq!(record["url"], NEWS_TWO);
    assert_eq!(record["published_at"], "2026-07-24T12:00:00+08:00");
    assert_eq!(record["publisher"], "新浪财经");
    assert_eq!(record["language"], "zh-CN");
    assert!(record
        .get("summary")
        .is_some_and(serde_json::Value::is_null));
    assert!(record
        .get("content")
        .is_some_and(serde_json::Value::is_null));
    assert_eq!(record["evidence"]["batch_id"], result.batch_id);
    assert_eq!(record["evidence"]["source_at"], "2026-07-24T12:00:00+08:00");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_sina_news_cutoff_empty_window_has_no_fabricated_source_instant() {
    let (registry, calls) = news_registry(
        vec![(
            1,
            news_page(
                1,
                &[
                    ("2026-07-24 14:00", NEWS_ONE, "Newer source title"),
                    ("2026-07-24 12:00", NEWS_TWO, "Other source title"),
                ],
                false,
            ),
        )],
        65_536,
    );
    let result = registry
        .execute(news_command(request("2026-07-24T11:00:00+08:00")))
        .unwrap();
    assert!(result.complete && result.repository_admitted);
    assert_eq!(result.provider, "Sina");
    assert!(result.records.is_empty());
    assert_eq!(result.source_at, None);
    assert!(result.batch_id.starts_with("sina-company-news:sh600396:"));
    assert!(result.diagnostic_blocker.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_sina_news_source_date_filtered_empty_is_not_an_acquisition_failure() {
    let (registry, calls) = news_registry(
        vec![(
            1,
            news_page(
                1,
                &[(
                    "2026-07-24 14:00",
                    NEWS_ONE,
                    "Outside requested source date",
                )],
                false,
            ),
        )],
        65_536,
    );
    let mut data = request("2026-07-23T23:59:00+08:00");
    data["start"] = serde_json::json!("2026-07-23");
    data["end"] = serde_json::json!("2026-07-23");
    let result = registry.execute(news_command(data)).unwrap();
    assert!(result.complete && result.repository_admitted);
    assert_eq!(result.provider, "Sina");
    assert!(result.records.is_empty());
    assert_eq!(result.source_at, None);
    assert!(result.batch_id.starts_with("sina-company-news:sh600396:"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_sina_news_future_source_time_cannot_be_hidden_by_caller_cutoff() {
    let (registry, calls) = news_registry(
        vec![(
            1,
            news_page(
                1,
                &[("2026-07-25 14:00", NEWS_ONE, "Future source row")],
                false,
            ),
        )],
        65_536,
    );
    let error = registry
        .execute(news_command(request("2026-07-24T13:00:00+08:00")))
        .unwrap_err();
    assert!(
        matches!(error, ServiceError::FailedPrecondition(ref reason)
        if reason.contains("future instrument-news provider time")),
        "{error:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_sina_news_refuses_invalid_contract_requests_before_source_access() {
    let (registry, calls) = news_registry(Vec::new(), 65_536);
    let mut too_many = request("2026-07-24T13:00:00+08:00");
    too_many["limit"] = serde_json::json!(201);
    let invalid_time = request("2026-07-24 13:00");
    let mut missing_end = request("2026-07-24T13:00:00+08:00");
    missing_end["start"] = serde_json::json!("2026-07-23");
    let mut wrong_china_end = request("2026-07-23T17:00:00Z");
    wrong_china_end["start"] = serde_json::json!("2026-07-23");
    wrong_china_end["end"] = serde_json::json!("2026-07-23");
    let mut invented_keyword = request("2026-07-24T13:00:00+08:00");
    invented_keyword["keyword"] = serde_json::json!("Rubin");
    for data in [
        too_many,
        invalid_time,
        missing_end,
        wrong_china_end,
        invented_keyword,
    ] {
        let error = registry.execute(news_command(data)).unwrap_err();
        assert!(
            matches!(error, ServiceError::InvalidRequest(_)),
            "{error:?}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn registered_sina_news_payload_ceiling_refuses_whole_source_batch_before_cutoff() {
    let (registry, calls) = news_registry(
        vec![(
            1,
            news_page(
                1,
                &[(
                    "2026-07-24 14:00",
                    NEWS_ONE,
                    "Source row outside caller cutoff",
                )],
                false,
            ),
        )],
        1,
    );
    let error = registry
        .execute(news_command(request("2026-07-24T13:00:00+08:00")))
        .unwrap_err();
    assert!(
        matches!(error, ServiceError::ResourceExhausted(ref reason)
        if reason.contains("exceeds maximum 1")),
        "{error:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_sina_news_later_page_outage_refuses_prior_successful_page_atomically() {
    let (registry, calls) = news_registry(
        vec![(
            1,
            news_page(
                1,
                &[(
                    "2026-07-24 14:00",
                    NEWS_ONE,
                    "First page is valid but not the full request",
                )],
                true,
            ),
        )],
        65_536,
    );
    let error = registry
        .execute(news_command(request("2026-07-24T15:00:00+08:00")))
        .unwrap_err();
    assert!(
        matches!(error, ServiceError::Unavailable { operation: Operation::InstrumentNews, ref reason }
        if reason == "offline later news page unavailable"),
        "{error:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
