use super::register_sse_announcements_client;
use magic_exchange_rs::{
    ExchangeError, ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, SseClient, SseConfig,
};
use magic_market_core::Announcement;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAXIMUM_PAYLOAD_BYTES: usize = 16 * 1024;
const PAGE: &[u8] =
    include_bytes!("../../../magic-exchange-rs/fixtures/sse_announcements_page1.jsonp");
const PAGE_URL: &str = "https://query.sse.com.cn/security/stock/queryCompanyBulletin.do?jsonCallBack=magicExchange&isPagination=true&productId=600396&securityType=0101&reportType2=DQGG&reportType=ALL&beginDate=2026-07-01&endDate=2026-07-23&pageHelp.pageSize=50&pageHelp.pageNo=1&pageHelp.beginPage=1&pageHelp.endPage=1&pageHelp.cacheSize=1";

fn expected_request() -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: PAGE_URL.to_owned(),
        headers: vec![
            (
                "User-Agent".to_owned(),
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
                    .to_owned(),
            ),
            (
                "Accept".to_owned(),
                "application/json, text/javascript;q=0.9".to_owned(),
            ),
            ("Accept-Language".to_owned(), "zh-CN,zh;q=0.9,en;q=0.8".to_owned()),
            (
                "Referer".to_owned(),
                "https://www.sse.com.cn/disclosure/listedinfo/announcement/".to_owned(),
            ),
            ("X-Requested-With".to_owned(), "XMLHttpRequest".to_owned()),
        ],
        body: Vec::new(),
    }
}

#[derive(Clone)]
struct AnnouncementPageFixture {
    body: Vec<u8>,
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl AnnouncementPageFixture {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ExchangeTransport for AnnouncementPageFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        // Every external entry is recorded before any fixture guard, without
        // dropping duplicate headers, reordering them or decoding the raw URL.
        self.attempts
            .lock()
            .map_err(|_| ExchangeError::Transport("offline SSE trace poisoned".to_owned()))?
            .push(request.clone());
        if request != &expected_request() {
            return Err(ExchangeError::Transport(
                "unexpected offline SSE method, URL, ordered headers or body".to_owned(),
            ));
        }
        if self.body.len() > 8 * 1024 * 1024 {
            return Err(ExchangeError::Transport(
                "offline SSE response exceeds 8 MiB".to_owned(),
            ));
        }
        Ok(HttpResponse {
            status: 200,
            final_url: PAGE_URL.to_owned(),
            content_type: Some("application/json;charset=UTF-8".to_owned()),
            body: self.body.clone(),
        })
    }
}

fn registry(fixture: AnnouncementPageFixture) -> OperationRegistry {
    let config = SseConfig::default();
    assert_eq!(config.timeout, Duration::from_secs(15));
    assert_eq!(config.minimum_interval, Duration::from_secs(1));
    assert_eq!(config.max_pages, 10);
    let client = SseClient::with_transport(config, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline SSE public contract");
    // Real concrete registrar is setup, not the assertion entry.
    register_sse_announcements_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::Announcements,
        Some("Sse".to_owned()),
        CanonicalPayload::new(
            "magic.market.announcements.request",
            1,
            br#"{"instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"start":"2026-07-01","end":"2026-07-23","limit":2}"#.to_vec(),
            MAXIMUM_PAYLOAD_BYTES,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!command.allows_unadmitted());
    command
}

fn clock_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

#[test]
fn registered_sse_announcements_reject_second_identity_without_publishing_rows() {
    assert_eq!(PAGE.len(), 555);
    assert_eq!(
        format!("{:x}", Sha256::digest(PAGE)),
        "6065457291c8ba7519ef307c09d2866c254e6a24cf062f85bfdd60a0bf97d7fb"
    );
    let source = std::str::from_utf8(PAGE).unwrap();
    let second_identity = r#""SECURITY_CODE":"600396","SSEDATE":"2026-07-11""#;
    let wrong_second_identity = r#""SECURITY_CODE":"600000","SSEDATE":"2026-07-11""#;
    assert_eq!(source.matches(second_identity).count(), 1);
    assert_eq!(source.matches(wrong_second_identity).count(), 0);
    let rejected_source = source.replace(second_identity, wrong_second_identity);
    assert_eq!(rejected_source.len(), 555);
    assert_eq!(
        format!("{:x}", Sha256::digest(rejected_source.as_bytes())),
        "774d8a53438bc4c62cfe563855e9d0dc220dac7034619de2877228c359af31c8"
    );
    assert_eq!(rejected_source.matches(wrong_second_identity).count(), 1);
    assert_eq!(
        rejected_source
            .replace(wrong_second_identity, second_identity)
            .as_bytes(),
        PAGE
    );

    let normal_fixture = AnnouncementPageFixture::new(PAGE);
    let normal_registry = registry(normal_fixture.clone());
    let before = clock_nanos();
    let result = normal_registry
        .execute(command("sse-announcements-complete-control"))
        .unwrap();
    let after = clock_nanos();
    assert_eq!(result.provider, "Sse");
    assert!(result.complete);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.records.len(), 2);
    assert_eq!(
        normal_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );

    // Independently constrain actual observation, not the date-only source time.
    let (seconds, fraction) = result.observed_at.split_once('.').unwrap();
    assert!(!seconds.is_empty());
    assert!(seconds.bytes().all(|byte| byte.is_ascii_digit()));
    assert_eq!(fraction.len(), 9);
    assert!(fraction.bytes().all(|byte| byte.is_ascii_digit()));
    let whole_seconds = seconds.parse::<u64>().unwrap();
    let fractional_nanos = fraction.parse::<u32>().unwrap();
    assert_eq!(seconds, whole_seconds.to_string());
    assert_eq!(fraction, format!("{fractional_nanos:09}"));
    let observed_nanos = u128::from(whole_seconds) * 1_000_000_000 + u128::from(fractional_nanos);
    assert!(before <= observed_nanos && observed_nanos <= after);
    let batch = format!("sse:{}:announcements:600396", result.observed_at);
    assert_eq!(result.batch_id, batch);

    // Complete literal wire facts come from the original public-source specimen,
    // never from a production mapper or a fabricated typed provider result.
    let expected = [
        serde_json::json!({
            "announcement_id": "600396_20260723_A001",
            "instrument": {"exchange":"Shanghai", "code":"600396", "asset_class":"Equity"},
            "instrument_name": null,
            "category": "其它",
            "title": "股票交易异常波动公告",
            "published_at": "2026-07-23",
            "canonical_url": "https://static.sse.com.cn/disclosure/listedinfo/announcement/c/new/2026-07-23/600396_20260723_A001.pdf",
            "pdf_url": "https://static.sse.com.cn/disclosure/listedinfo/announcement/c/new/2026-07-23/600396_20260723_A001.pdf",
            "evidence": {
                "provider": "Sse", "source_at": "2026-07-23",
                "observed_at": result.observed_at.clone(), "batch_id": batch.clone()
            }
        }),
        serde_json::json!({
            "announcement_id": "600396_20260711_A002",
            "instrument": {"exchange":"Shanghai", "code":"600396", "asset_class":"Equity"},
            "instrument_name": null,
            "category": "其它",
            "title": "股东减持计划公告",
            "published_at": "2026-07-11",
            "canonical_url": "https://static.sse.com.cn/disclosure/listedinfo/announcement/c/new/2026-07-11/600396_20260711_A002.pdf",
            "pdf_url": "https://static.sse.com.cn/disclosure/listedinfo/announcement/c/new/2026-07-11/600396_20260711_A002.pdf",
            "evidence": {
                "provider": "Sse", "source_at": "2026-07-11",
                "observed_at": result.observed_at.clone(), "batch_id": batch.clone()
            }
        }),
    ];
    for (payload, expected_row) in result.records.iter().zip(expected.iter()) {
        assert_eq!(payload.schema(), "magic.market.announcement");
        assert_eq!(payload.schema_version(), 1);
        let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(&row, expected_row);
        let typed: Announcement = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(&serde_json::to_value(&typed).unwrap(), expected_row);
    }

    let rejected_fixture = AnnouncementPageFixture::new(rejected_source.as_bytes());
    let rejected_registry = registry(rejected_fixture.clone());
    let error = rejected_registry
        .execute(command("sse-announcements-wrong-second-identity"))
        .unwrap_err();
    assert_eq!(
        error,
        ServiceError::FailedPrecondition(
            "official exchange schema drift: SSE source identity 600000 does not match requested 600396"
                .to_owned(),
        )
    );
    assert_eq!(
        rejected_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );
}
