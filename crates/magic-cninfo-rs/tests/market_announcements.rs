use magic_cninfo_rs::{
    CninfoClient, CninfoConfig, CninfoError, CninfoTransport, HttpRequest, HttpResponse,
};
use magic_market_core::{
    AssetClass, Exchange, IsoDate, MarketAnnouncementRequest, MarketAnnouncements, PositiveU32,
    ProviderId,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct FixtureTransport {
    body: Arc<Vec<u8>>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl FixtureTransport {
    fn new(body: serde_json::Value) -> Self {
        Self {
            body: Arc::new(serde_json::to_vec(&body).unwrap()),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl CninfoTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(HttpResponse {
            status: 200,
            final_url: request.url.clone(),
            content_type: Some("application/json;charset=UTF-8".into()),
            body: self.body.as_ref().clone(),
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

#[test]
fn native_market_page_preserves_source_identity_time_and_batch_evidence() {
    let rows = serde_json::json!([
        {
            "secCode": "600396",
            "orgId": "gssh0600396",
            "announcementId": "sh-1",
            "announcementTitle": "上海公告",
            "announcementTime": 1784822400000_i64,
            "adjunctUrl": "finalpage/2026-07-24/sh-1.PDF",
            "pageColumn": "SHMB",
            "announcementType": "01010503"
        },
        {
            "secCode": "300457",
            "orgId": "9900023940",
            "announcementId": "sz-1",
            "announcementTitle": "深圳公告",
            "announcementTime": 1784822400000_i64,
            "adjunctUrl": "finalpage/2026-07-24/sz-1.PDF",
            "pageColumn": "SZCY"
        },
        {
            "secCode": "920189",
            "orgId": "9900034066",
            "announcementId": "bj-1",
            "announcementTitle": "北京公告",
            "announcementTime": 1784822400000_i64,
            "adjunctUrl": "finalpage/2026-07-24/bj-1.PDF",
            "pageColumn": "BJS"
        }
    ]);
    let transport = FixtureTransport::new(serde_json::json!({
        "totalAnnouncement": 3,
        "totalRecordNum": 3,
        "totalpages": 0,
        "hasMore": false,
        "announcements": rows
    }));
    let requests = transport.requests.clone();
    let client = CninfoClient::with_transport(CninfoConfig::default(), transport).unwrap();

    let batch = client.market_announcements(&request(3)).unwrap();

    assert!(batch.quality().is_complete());
    assert_eq!(batch.records().len(), 3);
    assert_eq!(batch.records()[0].instrument.exchange(), Exchange::Shanghai);
    assert_eq!(batch.records()[1].instrument.exchange(), Exchange::Shenzhen);
    assert_eq!(batch.records()[2].instrument.exchange(), Exchange::Beijing);
    assert!(batch
        .records()
        .iter()
        .all(|record| record.instrument.asset_class() == AssetClass::Equity));
    assert!(batch
        .records()
        .iter()
        .all(|record| record.evidence.provider() == ProviderId::Cninfo));
    assert!(batch
        .records()
        .iter()
        .all(|record| record.evidence.source_at() == Some(record.published_at.as_str())));
    assert!(batch
        .records()
        .iter()
        .all(|record| record.evidence.batch_id() == batch.provenance().batch_id().unwrap()));
    assert_eq!(
        batch.records()[0].canonical_url.as_str(),
        "https://www.cninfo.com.cn/new/disclosure/detail?stockCode=600396&announcementId=sh-1&orgId=gssh0600396&announcementTime=2026-07-24"
    );
    assert_eq!(
        batch.records()[2]
            .pdf_url
            .as_ref()
            .map(magic_market_core::HttpsUrl::as_str),
        Some("https://static.cninfo.com.cn/finalpage/2026-07-24/bj-1.PDF")
    );

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let form = String::from_utf8(requests[0].body.clone()).unwrap();
    assert!(form.contains("stock=&"));
    assert!(form.contains("pageSize=30"));
    assert!(form.contains("pageNum=1"));
    assert!(form.contains("seDate=2026-07-24%7E2026-07-24"));
}
