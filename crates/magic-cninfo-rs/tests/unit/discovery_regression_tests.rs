use super::*;
use magic_market_core::{MarketAnnouncementRequest, MarketAnnouncements};
use std::collections::VecDeque;

#[derive(Clone)]
struct FixtureTransport {
    responses: Arc<Mutex<VecDeque<HttpResponse>>>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

#[derive(Debug, Clone, Copy)]
enum PagedFixtureKind {
    Announcements,
    Questions,
}

#[derive(Clone)]
struct PagedFixtureTransport {
    kind: PagedFixtureKind,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl PagedFixtureTransport {
    fn new(kind: PagedFixtureKind) -> Self {
        Self {
            kind,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn page_response(
        request: &HttpRequest,
        page: usize,
        page_size: usize,
        kind: PagedFixtureKind,
    ) -> Result<HttpResponse, CninfoError> {
        let offset = page
            .checked_sub(1)
            .and_then(|page| page.checked_mul(page_size))
            .ok_or_else(|| CninfoError::Transport("fixture page offset overflow".into()))?;
        let end = offset.saturating_add(page_size).min(50);
        let rows = (offset..end)
            .map(|index| match kind {
                PagedFixtureKind::Announcements => serde_json::json!({
                    "announcementId": format!("announcement-{index:02}"),
                    "secCode": "600396",
                    "announcementTitle": format!("announcement {index}"),
                    "announcementTime": 1_784_822_400_000_i64,
                    "adjunctUrl": format!("finalpage/2026-07-24/{index}.PDF")
                }),
                PagedFixtureKind::Questions => serde_json::json!({
                    "indexId": format!("question-{index:02}"),
                    "stockCode": "002594",
                    "companyShortName": "比亚迪",
                    "mainContent": format!("question {index}"),
                    "attachedContent": null,
                    "attachedAuthor": null,
                    "pubDate": 1_784_822_400_000_i64,
                    "attachedPubDate": null
                }),
            })
            .collect::<Vec<_>>();
        let body = match kind {
            PagedFixtureKind::Announcements => {
                serde_json::json!({"hasMore": end < 50, "announcements": rows})
            }
            PagedFixtureKind::Questions => {
                serde_json::json!({"total": 50, "rows": rows})
            }
        };
        Ok(HttpResponse {
            status: 200,
            final_url: request.url.clone(),
            content_type: Some("application/json".into()),
            body: serde_json::to_vec(&body)
                .map_err(|error| CninfoError::Transport(error.to_string()))?,
        })
    }
}

impl CninfoTransport for PagedFixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError> {
        self.requests
            .lock()
            .map_err(|_| CninfoError::Transport("fixture lock poisoned".into()))?
            .push(request.clone());
        match self.kind {
            PagedFixtureKind::Announcements if request.url == DEFAULT_MAPPING_URL => Ok(response(
                DEFAULT_MAPPING_URL,
                include_str!("../fixtures/organizations.json"),
            )),
            PagedFixtureKind::Questions if request.url == DEFAULT_IRM_LOOKUP_URL => Ok(response(
                DEFAULT_IRM_LOOKUP_URL,
                include_str!("../fixtures/irm_lookup.json"),
            )),
            PagedFixtureKind::Announcements => {
                let values = form_urlencoded::parse(&request.body)
                    .into_owned()
                    .collect::<HashMap<_, _>>();
                let page = values
                    .get("pageNum")
                    .and_then(|value| value.parse::<usize>().ok())
                    .ok_or_else(|| CninfoError::Transport("fixture pageNum missing".into()))?;
                let page_size = values
                    .get("pageSize")
                    .and_then(|value| value.parse::<usize>().ok())
                    .ok_or_else(|| CninfoError::Transport("fixture pageSize missing".into()))?;
                Self::page_response(request, page, page_size, self.kind)
            }
            PagedFixtureKind::Questions => {
                let url = Url::parse(&request.url)
                    .map_err(|error| CninfoError::Transport(error.to_string()))?;
                let values = url.query_pairs().into_owned().collect::<HashMap<_, _>>();
                let page = values
                    .get("pageNum")
                    .and_then(|value| value.parse::<usize>().ok())
                    .ok_or_else(|| CninfoError::Transport("fixture pageNum missing".into()))?;
                let page_size = values
                    .get("pageSize")
                    .and_then(|value| value.parse::<usize>().ok())
                    .ok_or_else(|| CninfoError::Transport("fixture pageSize missing".into()))?;
                Self::page_response(request, page, page_size, self.kind)
            }
        }
    }
}

impl FixtureTransport {
    fn new(responses: Vec<HttpResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl CninfoTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError> {
        self.requests.lock().unwrap().push(request.clone());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| CninfoError::Transport("fixture response exhausted".into()))
    }
}

#[derive(Clone)]
struct CompletionTransport {
    inner: FixtureTransport,
    completed_at: Arc<Mutex<Option<u128>>>,
}

impl CompletionTransport {
    fn new(responses: Vec<HttpResponse>) -> Self {
        Self {
            inner: FixtureTransport::new(responses),
            completed_at: Arc::new(Mutex::new(None)),
        }
    }
}

impl CninfoTransport for CompletionTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError> {
        let response = self.inner.execute(request)?;
        let completed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| CninfoError::Transport(error.to_string()))?
            .as_nanos();
        *self
            .completed_at
            .lock()
            .map_err(|_| CninfoError::Transport("completion lock poisoned".into()))? =
            Some(completed_at);
        Ok(response)
    }
}

fn response(url: &str, body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        final_url: url.into(),
        content_type: Some("application/json;charset=UTF-8".into()),
        body: body.as_bytes().to_vec(),
    }
}

fn instrument(code: &str) -> InstrumentId {
    InstrumentId::new(
        magic_market_core::Exchange::Shanghai,
        code,
        AssetClass::Equity,
    )
    .unwrap()
}

fn request(code: &str, limit: u32) -> InstrumentDateRangeRequest {
    InstrumentDateRangeRequest::new(
        instrument(code),
        magic_market_core::PositiveU32::new(limit).unwrap(),
    )
    .unwrap()
}

fn timestamp_nanos(value: &str) -> u128 {
    let (seconds, nanos) = value.split_once('.').unwrap();
    seconds.parse::<u128>().unwrap() * 1_000_000_000 + nanos.parse::<u128>().unwrap()
}

#[test]
fn organization_mapping_and_announcement_preserve_optional_category_and_pdf() {
    let transport = FixtureTransport::new(vec![
        response(
            DEFAULT_MAPPING_URL,
            include_str!("../fixtures/organizations.json"),
        ),
        response(
            DEFAULT_ANNOUNCEMENT_URL,
            include_str!("../fixtures/announcements_page.json"),
        ),
    ]);
    let observed = transport.clone();
    let client = CninfoClient::with_test_transport(transport);
    let batch = client.announcements(&request("600396", 2)).unwrap();
    assert_eq!(batch.records().len(), 2);
    let first = &batch.records()[0];
    assert_eq!(first.announcement_id.as_str(), "1225438962");
    assert!(first.category.is_none());
    assert_eq!(
        first.pdf_url.as_ref().map(HttpsUrl::as_str),
        Some("https://static.cninfo.com.cn/finalpage/2026-07-24/1225438962.PDF")
    );
    assert_eq!(
            first.canonical_url.as_str(),
            "https://www.cninfo.com.cn/new/disclosure/detail?stockCode=600396&announcementId=1225438962&orgId=gssh0600396&announcementTime=2026-07-24"
        );
    assert_eq!(first.evidence.provider(), ProviderId::Cninfo);
    assert!(batch.quality().is_complete());
    let requests = observed.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(String::from_utf8_lossy(&requests[1].body).contains("isHLtitle=false"));
    assert!(!String::from_utf8_lossy(&requests[1].body).contains("Cookie"));
}

#[test]
fn full_market_announcements_keep_source_identity_and_name() {
    let body = r#"{
          "hasMore": false,
          "totalAnnouncement": 2,
          "totalRecordNum": 2,
          "totalpages": 0,
          "announcements": [
            {
              "announcementId": "A-SH",
              "secCode": "600396",
              "secName": "华电辽能",
              "orgId": "gssh0600396",
              "pageColumn": "SHMB",
              "announcementTitle": "上海公告",
              "announcementTypeName": "公司公告",
              "announcementTime": 1784822400000,
              "adjunctUrl": "finalpage/2026-07-24/A-SH.PDF"
            },
            {
              "announcementId": "A-SZ",
              "secCode": "002594",
              "secName": "比亚迪",
              "orgId": "gssz0002594",
              "pageColumn": "SZMB",
              "announcementTitle": "深圳公告",
              "announcementTypeName": "公司公告",
              "announcementTime": 1784822400000,
              "adjunctUrl": "finalpage/2026-07-24/A-SZ.PDF"
            }
          ]
        }"#;
    let client = CninfoClient::with_test_transport(FixtureTransport::new(vec![response(
        DEFAULT_ANNOUNCEMENT_URL,
        body,
    )]));
    let request = MarketAnnouncementRequest::new(
        magic_market_core::IsoDate::new("2026-07-24").unwrap(),
        magic_market_core::IsoDate::new("2026-07-24").unwrap(),
        magic_market_core::PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    let batch = client.market_announcements(&request).unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].instrument.code(), "600396");
    assert_eq!(
        batch.records()[0]
            .instrument_name
            .as_ref()
            .unwrap()
            .as_str(),
        "华电辽能"
    );
    assert_eq!(batch.records()[1].instrument.code(), "002594");
    assert_eq!(
        batch.records()[1]
            .instrument_name
            .as_ref()
            .unwrap()
            .as_str(),
        "比亚迪"
    );
}

#[test]
fn investor_questions_keep_answer_absence_and_only_source_answer_time() {
    let transport = FixtureTransport::new(vec![
            response(
                DEFAULT_IRM_LOOKUP_URL,
                include_str!("../fixtures/irm_lookup.json"),
            ),
            response(
                "https://irm.cninfo.com.cn/newircs/company/question?_t=1&stockcode=002594&orgId=gshk0001211&pageSize=30&pageNum=1&keyWord=&startDay=&endDay=",
                include_str!("../fixtures/irm_questions.json"),
            ),
        ]);
    let client = CninfoClient::with_test_transport(transport);
    let shenzhen = InstrumentId::new(
        magic_market_core::Exchange::Shenzhen,
        "002594",
        AssetClass::Equity,
    )
    .unwrap();
    let request =
        InstrumentDateRangeRequest::new(shenzhen, magic_market_core::PositiveU32::new(2).unwrap())
            .unwrap();
    let batch = client.investor_questions(&request).unwrap();
    assert_eq!(batch.records().len(), 2);
    assert!(batch.records()[0].answer().is_none());
    assert!(batch.records()[0].answerer().is_none());
    assert!(batch.records()[1].answer().is_some());
    assert_eq!(
        batch.records()[1].answerer().map(NonEmptyText::as_str),
        Some("比亚迪")
    );
    assert!(batch.records()[1].answer_at().is_none());
    assert_eq!(
        batch.records()[1]
            .source_question_id()
            .map(NonEmptyText::as_str),
        Some("2310153346199089152")
    );
}

#[test]
fn pagination_failure_and_identity_mismatch_are_explicit() {
    let config = CninfoConfig {
        max_pages: 1,
        ..CninfoConfig::default()
    };
    let transport = FixtureTransport::new(vec![
        response(
            DEFAULT_MAPPING_URL,
            include_str!("../fixtures/organizations.json"),
        ),
        response(
            DEFAULT_ANNOUNCEMENT_URL,
            r#"{"hasMore":true,"announcements":[]}"#,
        ),
    ]);
    let client = CninfoClient::from_parts(Duration::ZERO, config, Arc::new(transport));
    assert!(matches!(
        client.announcements(&request("600396", 2)),
        Err(CninfoError::Schema(message)) if message.contains("hasMore")
    ));
}

#[test]
fn empty_announcement_and_question_results_are_explicitly_incomplete() {
    let announcements = CninfoClient::with_test_transport(FixtureTransport::new(vec![
        response(
            DEFAULT_MAPPING_URL,
            include_str!("../fixtures/organizations.json"),
        ),
        response(
            DEFAULT_ANNOUNCEMENT_URL,
            r#"{"hasMore":false,"announcements":[]}"#,
        ),
    ]));
    assert!(matches!(
        announcements.announcements(&request("600396", 1)),
        Err(CninfoError::Incomplete(message)) if message.contains("no announcements")
    ));

    let questions = CninfoClient::with_test_transport(FixtureTransport::new(vec![
            response(
                DEFAULT_IRM_LOOKUP_URL,
                include_str!("../fixtures/irm_lookup.json"),
            ),
            response(
                "https://irm.cninfo.com.cn/newircs/company/question?_t=1&stockcode=002594&orgId=gshk0001211&pageSize=30&pageNum=1&keyWord=&startDay=&endDay=",
                r#"{"total":0,"rows":[]}"#,
            ),
        ]));
    let instrument = InstrumentId::new(
        magic_market_core::Exchange::Shenzhen,
        "002594",
        AssetClass::Equity,
    )
    .unwrap();
    let request = InstrumentDateRangeRequest::new(
        instrument,
        magic_market_core::PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        questions.investor_questions(&request),
        Err(CninfoError::Incomplete(message)) if message.contains("no investor questions")
    ));
}

#[test]
fn remote_page_width_stays_fixed_for_fifty_record_requests() {
    let announcements = PagedFixtureTransport::new(PagedFixtureKind::Announcements);
    let observed_announcements = announcements.clone();
    let client = CninfoClient::with_test_transport(announcements);
    let batch = client
        .announcements(&request("600396", 50))
        .expect("two fixed-width announcement pages should not overlap");
    assert_eq!(batch.records().len(), 50);
    assert_eq!(
        batch
            .records()
            .iter()
            .map(|record| record.announcement_id.as_str())
            .collect::<HashSet<_>>()
            .len(),
        50
    );
    let requests = observed_announcements.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[1]
        .body
        .windows(11)
        .any(|part| part == b"pageSize=30"));
    assert!(requests[2]
        .body
        .windows(11)
        .any(|part| part == b"pageSize=30"));

    let questions = PagedFixtureTransport::new(PagedFixtureKind::Questions);
    let observed_questions = questions.clone();
    let client = CninfoClient::with_test_transport(questions);
    let instrument = InstrumentId::new(
        magic_market_core::Exchange::Shenzhen,
        "002594",
        AssetClass::Equity,
    )
    .unwrap();
    let request = InstrumentDateRangeRequest::new(
        instrument,
        magic_market_core::PositiveU32::new(50).unwrap(),
    )
    .unwrap();
    let batch = client
        .investor_questions(&request)
        .expect("two fixed-width question pages should not overlap");
    assert_eq!(batch.records().len(), 50);
    assert_eq!(
        batch
            .records()
            .iter()
            .map(|record| record.question_id().as_str())
            .collect::<HashSet<_>>()
            .len(),
        50
    );
    let requests = observed_questions.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[1].url.contains("pageSize=30"));
    assert!(requests[2].url.contains("pageSize=30"));
}

#[test]
fn code_prefix_must_match_the_declared_exchange() {
    let mismatches = [
        (magic_market_core::Exchange::Shanghai, "002594"),
        (magic_market_core::Exchange::Shenzhen, "600396"),
        (magic_market_core::Exchange::Beijing, "300001"),
    ];
    for (exchange, code) in mismatches {
        let instrument = InstrumentId::new(exchange, code, AssetClass::Equity).unwrap();
        assert!(matches!(
            validate_instrument(&instrument),
            Err(CninfoError::InvalidRequest(message)) if message.contains("exchange")
        ));
    }
    let unsupported = instrument("100001");
    assert!(matches!(
        validate_instrument(&unsupported),
        Err(CninfoError::Unsupported(message)) if message.contains("prefix")
    ));

    let verified_beijing = InstrumentId::new(
        magic_market_core::Exchange::Beijing,
        "920001",
        AssetClass::Equity,
    )
    .unwrap();
    assert!(validate_instrument(&verified_beijing).is_ok());

    let unverified_nine_prefix = InstrumentId::new(
        magic_market_core::Exchange::Shanghai,
        "900901",
        AssetClass::Equity,
    )
    .unwrap();
    assert!(matches!(
        validate_instrument(&unverified_nine_prefix),
        Err(CninfoError::Unsupported(message)) if message.contains("prefix")
    ));
}

#[test]
fn batch_observation_time_is_not_before_the_final_response() {
    let announcements = CompletionTransport::new(vec![
        response(
            DEFAULT_MAPPING_URL,
            include_str!("../fixtures/organizations.json"),
        ),
        response(
            DEFAULT_ANNOUNCEMENT_URL,
            include_str!("../fixtures/announcements_page.json"),
        ),
    ]);
    let observed_announcements = announcements.clone();
    let batch = CninfoClient::with_test_transport(announcements)
        .announcements(&request("600396", 2))
        .unwrap();
    let completed_at = observed_announcements.completed_at.lock().unwrap().unwrap();
    assert!(timestamp_nanos(batch.provenance().fetched_at()) >= completed_at);

    let questions = CompletionTransport::new(vec![
            response(
                DEFAULT_IRM_LOOKUP_URL,
                include_str!("../fixtures/irm_lookup.json"),
            ),
            response(
                "https://irm.cninfo.com.cn/newircs/company/question?_t=1&stockcode=002594&orgId=gshk0001211&pageSize=30&pageNum=1&keyWord=&startDay=&endDay=",
                include_str!("../fixtures/irm_questions.json"),
            ),
        ]);
    let observed_questions = questions.clone();
    let instrument = InstrumentId::new(
        magic_market_core::Exchange::Shenzhen,
        "002594",
        AssetClass::Equity,
    )
    .unwrap();
    let request = InstrumentDateRangeRequest::new(
        instrument,
        magic_market_core::PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    let batch = CninfoClient::with_test_transport(questions)
        .investor_questions(&request)
        .unwrap();
    let completed_at = observed_questions.completed_at.lock().unwrap().unwrap();
    assert!(timestamp_nanos(batch.provenance().fetched_at()) >= completed_at);
}

#[test]
fn strict_hosts_content_type_and_body_caps_are_enforced() {
    let config = CninfoConfig {
        mapping_url: "https://example.com/map.json".into(),
        ..CninfoConfig::default()
    };
    assert!(matches!(
        CninfoClient::with_transport(config, FixtureTransport::new(Vec::new())),
        Err(CninfoError::InvalidRequest(message)) if message.contains("allowlisted")
    ));

    let oversized = HttpResponse {
        status: 200,
        final_url: DEFAULT_MAPPING_URL.into(),
        content_type: Some("application/json".into()),
        body: vec![b' '; MAX_RESPONSE_BYTES + 1],
    };
    let client = CninfoClient::with_test_transport(FixtureTransport::new(vec![oversized]));
    assert!(matches!(
        client.organization_mapping(&instrument("600396")),
        Err(CninfoError::Incomplete(_))
    ));
}

#[test]
fn timestamp_conversion_is_timezone_explicit() {
    assert_eq!(
        unix_seconds_to_china_iso(1_784_822_400).as_deref(),
        Some("2026-07-24T00:00:00+08:00")
    );
}

#[test]
fn capabilities_are_conservative() {
    let capabilities = CninfoClient::capabilities();
    assert!(capabilities.announcements);
    assert!(capabilities.investor_questions);
    assert!(!capabilities.instrument_news);
    assert!(!capabilities.global_news);
}
