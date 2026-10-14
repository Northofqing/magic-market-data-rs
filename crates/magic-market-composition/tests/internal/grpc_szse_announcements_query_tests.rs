use super::register_szse_announcements_client;
use magic_exchange_rs::{
    ExchangeError, ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, SzseClient, SzseConfig,
};
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAXIMUM_PAYLOAD_BYTES: usize = 16 * 1024;
const PAGE: &[u8] =
    include_bytes!("../../../magic-exchange-rs/fixtures/szse_announcements_page1.json");
const PAGE_URL: &str = "https://www.szse.cn/api/disc/announcement/annList";
const NORMAL_REQUEST: &[u8] = br#"{"instrument":{"exchange":"Shenzhen","code":"000858","asset_class":"Equity"},"start":"2026-07-01","end":"2026-07-23","limit":2}"#;
const REJECTED_REQUEST: &[u8] = br#"{"instrument":{"exchange":"Shenzhen","code":"000858","asset_class":"Equity"},"start":"2026-07-01","end":"2026-07-23","limit":1}"#;

fn expected_request() -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Post,
        url: PAGE_URL.to_owned(),
        headers: vec![
            (
                "User-Agent".to_owned(),
                "Mozilla/5.0 (compatible; magic-exchange-rs/0.2; read-only official-data probe)"
                    .to_owned(),
            ),
            ("Accept".to_owned(), "application/json".to_owned()),
            (
                "Content-Type".to_owned(),
                "application/json;charset=UTF-8".to_owned(),
            ),
            (
                "Referer".to_owned(),
                "https://www.szse.cn/disclosure/listed/notice/index.html".to_owned(),
            ),
        ],
        body: br#"{"channelCode":["listedNotice_disc"],"pageSize":50,"pageNum":1,"stock":["000858"],"seDate":["2026-07-01","2026-07-23"]}"#.to_vec(),
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
        // Record every external attempt before guards, including raw bytes and
        // duplicate/order-sensitive headers; never decode it through the SDK.
        self.attempts
            .lock()
            .map_err(|_| ExchangeError::Transport("offline SZSE trace poisoned".to_owned()))?
            .push(request.clone());
        if request != &expected_request() {
            return Err(ExchangeError::Transport(
                "unexpected offline SZSE method, URL, ordered headers or body".to_owned(),
            ));
        }
        if self.body.len() > 8 * 1024 * 1024 {
            return Err(ExchangeError::Transport(
                "offline SZSE response exceeds 8 MiB".to_owned(),
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
    let config = SzseConfig::default();
    assert_eq!(config.timeout, Duration::from_secs(15));
    assert_eq!(config.minimum_interval, Duration::from_secs(1));
    assert_eq!(config.max_pages, 10);
    let client = SzseClient::with_transport(config, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline SZSE public contract");
    // The real concrete registrar is setup only; assertions enter public Execute.
    register_szse_announcements_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str, request_bytes: &[u8]) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::Announcements,
        Some("Szse".to_owned()),
        CanonicalPayload::new(
            "magic.market.announcements.request",
            1,
            request_bytes.to_vec(),
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
fn registered_szse_announcements_reject_second_identity_without_publishing_rows() {
    assert_eq!(PAGE.len(), 642);
    assert_eq!(
        format!("{:x}", Sha256::digest(PAGE)),
        "b238b2a9064c22f62bfd556d9aa6d5aa24a1d1cbcea246f3ae8b84c3954d4efa"
    );
    let source = std::str::from_utf8(PAGE).unwrap();
    let second_identity = r#""secCode":["000858"],"secName":["五 粮 液"]}]}"#;
    let wrong_second_identity = r#""secCode":["000651"],"secName":["五 粮 液"]}]}"#;
    assert_eq!(source.matches(second_identity).count(), 1);
    assert_eq!(source.matches(wrong_second_identity).count(), 0);
    let rejected_source = source.replace(second_identity, wrong_second_identity);
    assert_eq!(rejected_source.len(), 642);
    assert_eq!(
        format!("{:x}", Sha256::digest(rejected_source.as_bytes())),
        "bbc59e6ff2acfe49300d86938bff2659ebcf5547e5333d52bdb91cc665e83e40"
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
        .execute(command(
            "szse-announcements-complete-control",
            NORMAL_REQUEST,
        ))
        .unwrap();
    let after = clock_nanos();
    assert_eq!(result.provider, "Szse");
    assert!(result.complete);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-18"));
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.records.len(), 2);
    assert_eq!(
        normal_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );

    // Independently bracket observation; do not compare it to the source dates.
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
    let batch = format!("szse:{}:announcements:000858", result.observed_at);
    assert_eq!(result.batch_id, batch);

    // Literal record facts from the original specimen, not mapper-generated
    // expectations. secName is not exposed by this existing announcement mapper.
    let expected = [
        serde_json::json!({
            "announcement_id": "1225429654",
            "instrument": {"exchange":"Shenzhen", "code":"000858", "asset_class":"Equity"},
            "instrument_name": null,
            "category": null,
            "title": "五 粮 液：关于董事辞职的公告",
            "published_at": "2026-07-18",
            "canonical_url": "https://www.szse.cn/disclosure/listed/bulletinDetail/index.html?1225429654",
            "pdf_url": "https://disc.static.szse.cn/download/disc/disk03/finalpage/2026-07-18/b3904d77-234f-46cb-a427-c5f700550cd8.PDF",
            "evidence": {
                "provider": "Szse", "source_at": "2026-07-18",
                "observed_at": result.observed_at.clone(), "batch_id": batch.clone()
            }
        }),
        serde_json::json!({
            "announcement_id": "1225426598",
            "instrument": {"exchange":"Shenzhen", "code":"000858", "asset_class":"Equity"},
            "instrument_name": null,
            "category": null,
            "title": "五 粮 液：分红派息公告",
            "published_at": "2026-07-16",
            "canonical_url": "https://www.szse.cn/disclosure/listed/bulletinDetail/index.html?1225426598",
            "pdf_url": "https://disc.static.szse.cn/download/disc/disk03/finalpage/2026-07-16/2b971232-4152-4fe8-998e-1e2771ea0709.PDF",
            "evidence": {
                "provider": "Szse", "source_at": "2026-07-16",
                "observed_at": result.observed_at.clone(), "batch_id": batch.clone()
            }
        }),
    ];
    for (payload, expected_row) in result.records.iter().zip(expected.iter()) {
        assert_eq!(payload.schema(), "magic.market.announcement");
        assert_eq!(payload.schema_version(), 1);
        let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(&row, expected_row);
    }

    let rejected_fixture = AnnouncementPageFixture::new(rejected_source.as_bytes());
    let rejected_registry = registry(rejected_fixture.clone());
    let error = rejected_registry
        .execute(command(
            "szse-announcements-wrong-second-identity",
            REJECTED_REQUEST,
        ))
        .unwrap_err();
    assert_eq!(
        error,
        ServiceError::FailedPrecondition(
            "official exchange schema drift: SZSE source identity [\"000651\"] does not exactly match requested 000858"
                .to_owned(),
        )
    );
    assert_eq!(
        rejected_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );
}
