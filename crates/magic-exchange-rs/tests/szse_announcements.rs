use magic_exchange_rs::{
    ExchangeError, ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, SzseClient, SzseConfig,
};
use magic_market_core::{
    Announcements, AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, IsoDate,
    PositiveU32, ProviderId,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const PAGE_1: &[u8] = include_bytes!("../fixtures/szse_announcements_page1.json");
#[derive(Clone)]
struct Scripted {
    responses: Arc<Mutex<VecDeque<HttpResponse>>>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl Scripted {
    fn new(bodies: &[&[u8]]) -> Self {
        Self {
            responses: Arc::new(Mutex::new(
                bodies
                    .iter()
                    .map(|body| HttpResponse {
                        status: 200,
                        final_url: "https://www.szse.cn/api/disc/announcement/annList".into(),
                        content_type: Some("application/json;charset=UTF-8".into()),
                        body: body.to_vec(),
                    })
                    .collect(),
            )),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ExchangeTransport for Scripted {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(self.responses.lock().unwrap().pop_front().unwrap())
    }
}

fn request(limit: u32) -> InstrumentDateRangeRequest {
    InstrumentDateRangeRequest::new(
        InstrumentId::new(Exchange::Shenzhen, "000858", AssetClass::Equity).unwrap(),
        PositiveU32::new(limit).unwrap(),
    )
    .unwrap()
    .with_range(
        IsoDate::new("2026-07-01").unwrap(),
        IsoDate::new("2026-07-23").unwrap(),
    )
    .unwrap()
}

fn generated_page(start: u64, count: usize, total: u32) -> Vec<u8> {
    let rows = (0..count)
        .map(|offset| {
            let id = start + offset as u64;
            serde_json::json!({
                "id": format!("fixture-{id}"),
                "annId": id,
                "title": format!("五 粮 液：公告 {id}"),
                "publishTime": "2026-07-18 00:00:00",
                "attachPath": format!("/disc/disk03/finalpage/2026-07-18/{id}.PDF"),
                "attachFormat": "PDF",
                "secCode": ["000858"]
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&serde_json::json!({
        "announceCount": total,
        "data": rows
    }))
    .unwrap()
}

#[test]
fn maps_canonical_detail_pdf_and_provenance() {
    let transport = Scripted::new(&[PAGE_1]);
    let client =
        SzseClient::with_transport(SzseConfig::default(), transport.clone()).expect("client");
    let batch = client.announcements(&request(1)).expect("announcements");
    let record = &batch.records()[0];
    assert_eq!(record.evidence.provider(), ProviderId::Szse);
    assert_eq!(record.evidence.source_at(), Some("2026-07-18"));
    assert_eq!(
        record.canonical_url.as_str(),
        "https://www.szse.cn/disclosure/listed/bulletinDetail/index.html?1225429654"
    );
    assert_eq!(
        record.pdf_url.as_ref().unwrap().as_str(),
        "https://disc.static.szse.cn/download/disc/disk03/finalpage/2026-07-18/b3904d77-234f-46cb-a427-c5f700550cd8.PDF"
    );
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests[0].method, HttpMethod::Post);
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["pageSize"], 50);
    assert_eq!(body["pageNum"], 1);
    assert_eq!(body["stock"][0], "000858");
    assert_eq!(body["seDate"][0], "2026-07-01");
    assert_eq!(body["seDate"][1], "2026-07-23");
}

#[test]
fn paginates_at_actual_maximum_and_truncates_locally() {
    let first = generated_page(1, 50, 51);
    let second = generated_page(51, 1, 51);
    let transport = Scripted::new(&[&first, &second]);
    let client =
        SzseClient::with_transport(SzseConfig::default(), transport.clone()).expect("client");
    let batch = client.announcements(&request(51)).expect("announcements");
    assert_eq!(batch.records().len(), 51);
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(body["pageSize"], 50);
    assert_eq!(body["pageNum"], 2);
}

#[test]
fn rejects_wrong_exchange_identity_date_and_partial_pages() {
    let wrong = InstrumentDateRangeRequest::new(
        InstrumentId::new(Exchange::Shanghai, "000858", AssetClass::Equity).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let client =
        SzseClient::with_transport(SzseConfig::default(), Scripted::new(&[PAGE_1])).unwrap();
    assert!(matches!(
        client.announcements(&wrong),
        Err(ExchangeError::InvalidRequest(_))
    ));

    for body in [
        String::from_utf8(PAGE_1.to_vec())
            .unwrap()
            .replace("\"000858\"", "\"000001\""),
        String::from_utf8(PAGE_1.to_vec())
            .unwrap()
            .replace("2026-07-18 00:00:00", "2026-06-18 00:00:00"),
        String::from_utf8(PAGE_1.to_vec())
            .unwrap()
            .replace("\"announceCount\":2", "\"announceCount\":60"),
        String::from_utf8(PAGE_1.to_vec())
            .unwrap()
            .replace("2026-07-18 00:00:00", "中中中中"),
        String::from_utf8(PAGE_1.to_vec())
            .unwrap()
            .replace("2026-07-18 00:00:00", "2026-02-30 00:00:00"),
    ] {
        let client =
            SzseClient::with_transport(SzseConfig::default(), Scripted::new(&[body.as_bytes()]))
                .unwrap();
        assert!(client.announcements(&request(1)).is_err());
    }
}

#[test]
fn rejects_duplicate_ids_and_schema_drift() {
    let first = generated_page(1, 50, 51);
    let duplicate = generated_page(1, 1, 51);
    let client =
        SzseClient::with_transport(SzseConfig::default(), Scripted::new(&[&first, &duplicate]))
            .unwrap();
    assert!(matches!(
        client.announcements(&request(51)),
        Err(ExchangeError::Schema(_))
    ));
    let client = SzseClient::with_transport(
        SzseConfig::default(),
        Scripted::new(&[br#"{"announceCount":1}"#]),
    )
    .unwrap();
    assert!(matches!(
        client.announcements(&request(1)),
        Err(ExchangeError::Schema(_))
    ));
}
