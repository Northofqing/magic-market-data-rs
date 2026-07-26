use magic_exchange_rs::{
    ExchangeError, ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, SseClient, SseConfig,
};
use magic_market_core::{
    Announcements, AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, IsoDate,
    PositiveU32, ProviderId,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const PAGE_1: &[u8] = include_bytes!("../fixtures/sse_announcements_page1.jsonp");
#[derive(Clone)]
struct Scripted {
    responses: Arc<Mutex<VecDeque<HttpResponse>>>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl Scripted {
    fn new(bodies: &[&[u8]]) -> Self {
        let responses = bodies
            .iter()
            .map(|body| HttpResponse {
                status: 200,
                final_url: String::new(),
                content_type: Some("application/json;charset=UTF-8".into()),
                body: body.to_vec(),
            })
            .collect();
        Self {
            responses: Arc::new(Mutex::new(responses)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ExchangeTransport for Scripted {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        self.requests.lock().unwrap().push(request.clone());
        let mut response = self.responses.lock().unwrap().pop_front().unwrap();
        response.final_url = request.url.clone();
        Ok(response)
    }
}

fn request(limit: u32) -> InstrumentDateRangeRequest {
    InstrumentDateRangeRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
        PositiveU32::new(limit).unwrap(),
    )
    .unwrap()
    .with_range(
        IsoDate::new("2026-07-01").unwrap(),
        IsoDate::new("2026-07-23").unwrap(),
    )
    .unwrap()
}

fn generated_page(page: u32, start: u64, count: usize, total: u32) -> Vec<u8> {
    let rows = (0..count)
        .map(|offset| {
            let id = start + offset as u64;
            serde_json::json!({
                "SECURITY_CODE": "600396",
                "SSEDATE": "2026-07-18",
                "TITLE": format!("公告 {id}"),
                "BULLETIN_TYPE": "其它",
                "URL": format!(
                    "/disclosure/listedinfo/announcement/c/new/2026-07-18/600396_20260718_{id}.pdf"
                )
            })
        })
        .collect::<Vec<_>>();
    format!(
        "magicExchange({})",
        serde_json::json!({
            "productId": "600396",
            "beginDate": "2026-07-01",
            "endDate": "2026-07-23",
            "pageHelp": {
                "pageNo": page,
                "pageSize": 50,
                "pageCount": total.div_ceil(50),
                "total": total,
                "data": rows
            }
        })
    )
    .into_bytes()
}

#[test]
fn maps_official_records_and_truncates_after_fixed_remote_page() {
    let transport = Scripted::new(&[PAGE_1]);
    let client =
        SseClient::with_transport(SseConfig::default(), transport.clone()).expect("client");
    let batch = client.announcements(&request(1)).expect("announcements");
    assert_eq!(batch.records().len(), 1);
    let record = &batch.records()[0];
    assert_eq!(record.instrument.code(), "600396");
    assert_eq!(record.evidence.provider(), ProviderId::Sse);
    assert_eq!(record.evidence.source_at(), Some("2026-07-23"));
    assert_eq!(
        record.pdf_url.as_ref().unwrap().as_str(),
        "https://static.sse.com.cn/disclosure/listedinfo/announcement/c/new/2026-07-23/600396_20260723_A001.pdf"
    );
    assert_eq!(batch.provenance().source(), "sse-official");
    assert_eq!(batch.provenance().source_at(), Some("2026-07-23"));
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, HttpMethod::Get);
    assert!(requests[0].url.contains("pageHelp.pageSize=50"));
    assert!(requests[0].headers.iter().any(|(name, value)| {
        name == "User-Agent"
            && value == "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
    }));
    assert!(requests[0]
        .headers
        .iter()
        .any(|(name, value)| name == "X-Requested-With" && value == "XMLHttpRequest"));
}

#[test]
fn paginates_without_overlap_and_uses_local_limit() {
    let first = generated_page(1, 1, 50, 51);
    let second = generated_page(2, 51, 1, 51);
    let transport = Scripted::new(&[&first, &second]);
    let client =
        SseClient::with_transport(SseConfig::default(), transport.clone()).expect("client");
    let batch = client.announcements(&request(51)).expect("announcements");
    assert_eq!(batch.records().len(), 51);
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].url.contains("pageHelp.pageNo=1"));
    assert!(requests[1].url.contains("pageHelp.pageNo=2"));
}

#[test]
fn rejects_wrong_exchange_and_source_identity() {
    let wrong = InstrumentDateRangeRequest::new(
        InstrumentId::new(Exchange::Shenzhen, "600396", AssetClass::Equity).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let client = SseClient::with_transport(SseConfig::default(), Scripted::new(&[PAGE_1])).unwrap();
    assert!(matches!(
        client.announcements(&wrong),
        Err(ExchangeError::InvalidRequest(_))
    ));
    let bad = String::from_utf8(PAGE_1.to_vec())
        .unwrap()
        .replace("\"600396\"", "\"600000\"");
    let client =
        SseClient::with_transport(SseConfig::default(), Scripted::new(&[bad.as_bytes()])).unwrap();
    assert!(matches!(
        client.announcements(&request(1)),
        Err(ExchangeError::Schema(_))
    ));
}

#[test]
fn rejects_malformed_jsonp_duplicate_ids_and_pagination_gap() {
    let client =
        SseClient::with_transport(SseConfig::default(), Scripted::new(&[b"not_jsonp"])).unwrap();
    assert!(matches!(
        client.announcements(&request(1)),
        Err(ExchangeError::Decode(_))
    ));

    let first = generated_page(1, 1, 50, 51);
    let duplicate = generated_page(2, 1, 1, 51);
    let client =
        SseClient::with_transport(SseConfig::default(), Scripted::new(&[&first, &duplicate]))
            .unwrap();
    assert!(matches!(
        client.announcements(&request(51)),
        Err(ExchangeError::Schema(_))
    ));

    let gap = String::from_utf8(generated_page(2, 51, 1, 51))
        .unwrap()
        .replace("\"pageNo\":2", "\"pageNo\":3");
    let client = SseClient::with_transport(
        SseConfig::default(),
        Scripted::new(&[&first, gap.as_bytes()]),
    )
    .unwrap();
    assert!(matches!(
        client.announcements(&request(51)),
        Err(ExchangeError::Incomplete(_))
    ));
}
