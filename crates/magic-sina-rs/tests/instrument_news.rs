use encoding_rs::GB18030;
use magic_market_core::{
    AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, IsoDate, NewsProvider,
    PositiveU32, ProviderId,
};
use magic_sina_rs::{DocumentResponse, SinaClient, SinaError, SnapshotTransport};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const OBSERVED_UNIX: u64 = 1_784_912_800;

#[test]
fn instrument_news_has_no_unmaintained_general_html_parser_dependency() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        !manifest.lines().any(|line| {
            line.split_once('=')
                .is_some_and(|(name, _)| name.trim() == "scraper")
        }),
        "BR-025 requires the bounded Sina page parser; scraper is not release-admitted"
    );
}

#[derive(Clone)]
struct FixtureTransport {
    pages: Arc<HashMap<String, DocumentResponse>>,
    requested: Arc<Mutex<Vec<String>>>,
}

impl FixtureTransport {
    fn new(pages: HashMap<String, DocumentResponse>) -> Self {
        Self {
            pages: Arc::new(pages),
            requested: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requested(&self) -> Vec<String> {
        self.requested.lock().unwrap().clone()
    }
}

impl SnapshotTransport for FixtureTransport {
    fn get(&self, _url: &str) -> Result<Vec<u8>, SinaError> {
        Err(SinaError::Transport(
            "byte-only transport must not serve news".into(),
        ))
    }

    fn get_document(&self, url: &str) -> Result<DocumentResponse, SinaError> {
        self.requested.lock().unwrap().push(url.to_owned());
        self.pages
            .get(url)
            .cloned()
            .ok_or_else(|| SinaError::Transport(format!("unexpected fixture URL {url}")))
    }
}

fn sh() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn sz() -> InstrumentId {
    InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap()
}

fn bj() -> InstrumentId {
    InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap()
}

fn request(limit: u32) -> InstrumentDateRangeRequest {
    InstrumentDateRangeRequest::new(sh(), PositiveU32::new(limit).unwrap()).unwrap()
}

fn url(page: u32) -> String {
    url_for("sh600396", page)
}

fn url_for(symbol: &str, page: u32) -> String {
    format!(
        "https://vip.stock.finance.sina.com.cn/corp/view/vCB_AllNewsStock.php?symbol={symbol}&Page={page}"
    )
}

fn row(published: &str, canonical_url: &str, title: &str) -> String {
    format!(
        "&nbsp;&nbsp;&nbsp;&nbsp;{published}&nbsp;&nbsp;<a target='_blank' href='{canonical_url}'>{title}</a><br>"
    )
}

fn page(symbol: &str, page: u32, rows: &[String], has_next: bool) -> Vec<u8> {
    let next = if has_next {
        format!(
            "<a href='http://vip.stock.finance.sina.com.cn/corp/view/vCB_AllNewsStock.php?symbol={symbol}&Page={}'>下一页</a>",
            page + 1
        )
    } else {
        String::new()
    };
    let html = format!(
        r#"<html><head><title>公司(600396)公司资讯_新浪财经</title></head>
<body><script>var page_symbol = "{symbol}";</script>
<div class="datelist"><ul>{}</ul></div>
<div>第{page}页 {next}</div></body></html>"#,
        rows.join("")
    );
    let (encoded, _, had_errors) = GB18030.encode(&html);
    assert!(!had_errors);
    encoded.into_owned()
}

fn response(body: Vec<u8>) -> DocumentResponse {
    response_at(body, OBSERVED_UNIX)
}

fn response_at(body: Vec<u8>, observed_unix: u64) -> DocumentResponse {
    DocumentResponse::new(200, "text/html; charset=gbk", body, observed_unix)
}

#[test]
fn instrument_news_uses_exact_symbol_url_and_complete_evidence() {
    let canonical = "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml";
    let transport = FixtureTransport::new(HashMap::from([(
        url(1),
        response(page(
            "sh600396",
            1,
            &[row("2026-07-24 22:35", canonical, "来源标题")],
            false,
        )),
    )]));
    let client = SinaClient::with_transport(transport.clone());

    let batch = client.instrument_news(&request(1)).unwrap();

    assert_eq!(transport.requested(), vec![url(1)]);
    assert_eq!(batch.records().len(), 1);
    let item = &batch.records()[0];
    assert_eq!(item.item_id.as_str(), canonical);
    assert_eq!(item.title.as_str(), "来源标题");
    assert_eq!(item.publisher.as_str(), "新浪财经");
    assert_eq!(item.canonical_url.as_str(), canonical);
    assert_eq!(item.published_at.as_str(), "2026-07-24T22:35:00+08:00");
    assert_eq!(item.instruments, vec![sh()]);
    assert_eq!(item.language.as_str(), "zh-CN");
    assert_eq!(item.evidence.provider(), ProviderId::Sina);
    assert_eq!(item.evidence.source_at(), Some("2026-07-24T22:35:00+08:00"));
    assert_eq!(
        item.evidence.batch_id(),
        batch.provenance().batch_id().unwrap()
    );
    assert!(batch.quality().is_complete());

    let capabilities = SinaClient::content_capabilities();
    assert!(capabilities.instrument_news);
    assert!(
        !capabilities.global_news
            && !capabilities.announcements
            && !capabilities.investor_questions
    );
}

#[test]
fn outer_script_comparisons_are_not_parsed_as_html_tags() {
    let canonical = "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml";
    let mut body = page(
        "sh600396",
        1,
        &[row("2026-07-24 22:35", canonical, "来源标题")],
        false,
    );
    let (needle, _, _) = GB18030.encode("<script>var page_symbol");
    let position = body
        .windows(needle.len())
        .position(|window| window == needle.as_ref())
        .unwrap();
    let (replacement, _, had_errors) =
        GB18030.encode("<script>if (width < wrapWidth) { width += 1; } var page_symbol");
    assert!(!had_errors);
    body.splice(
        position..position + needle.len(),
        replacement.iter().copied(),
    );
    let client = SinaClient::with_transport(FixtureTransport::new(HashMap::from([(
        url(1),
        response(body),
    )])));

    let batch = client.instrument_news(&request(1)).unwrap();
    assert_eq!(batch.records().len(), 1);
    assert_eq!(batch.records()[0].canonical_url.as_str(), canonical);
}

#[test]
fn script_between_news_rows_does_not_create_an_empty_source_title() {
    let one = "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml";
    let two = "https://finance.sina.com.cn/roll/2026-07-24/doc-two.shtml";
    let rows = vec![
        row("2026-07-24 22:35", one, "一"),
        "<script>if (width < wrapWidth) { width += 1; }</script>".into(),
        row("2026-07-24 22:34", two, "二"),
    ];
    let client = SinaClient::with_transport(FixtureTransport::new(HashMap::from([(
        url(1),
        response(page("sh600396", 1, &rows, false)),
    )])));

    let batch = client.instrument_news(&request(2)).unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].title.as_str(), "一");
    assert_eq!(batch.records()[1].title.as_str(), "二");
}

#[test]
fn unrelated_link_with_nested_markup_between_rows_is_not_a_news_record() {
    let one = "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml";
    let two = "https://finance.sina.com.cn/roll/2026-07-24/doc-two.shtml";
    let rows = vec![
        row("2026-07-24 22:35", one, "一"),
        "<span><a href='https://cj.sina.cn/articles/view/1/2'><img src='x'></a></span>".into(),
        row("2026-07-24 22:34", two, "二"),
    ];
    let client = SinaClient::with_transport(FixtureTransport::new(HashMap::from([(
        url(1),
        response(page("sh600396", 1, &rows, false)),
    )])));

    let batch = client.instrument_news(&request(2)).unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].title.as_str(), "一");
    assert_eq!(batch.records()[1].title.as_str(), "二");
}

#[test]
fn shenzhen_request_uses_exact_sz_symbol_identity() {
    let request = InstrumentDateRangeRequest::new(sz(), PositiveU32::new(1).unwrap()).unwrap();
    let expected_url = url_for("sz000001", 1);
    let canonical = "https://finance.sina.com.cn/roll/2026-07-24/doc-sz.shtml";
    let transport = FixtureTransport::new(HashMap::from([(
        expected_url.clone(),
        response(page(
            "sz000001",
            1,
            &[row("2026-07-24 22:35", canonical, "深圳来源标题")],
            false,
        )),
    )]));
    let client = SinaClient::with_transport(transport.clone());

    let batch = client.instrument_news(&request).unwrap();

    assert_eq!(transport.requested(), vec![expected_url]);
    assert_eq!(batch.records().len(), 1);
    assert_eq!(batch.records()[0].instruments, vec![sz()]);
}

#[test]
fn official_http_article_url_is_upgraded_to_the_same_https_identity() {
    let source_url =
        "http://stock.finance.sina.com.cn/stock/go.php/vReport_Show/kind/search/rptid/1/index.phtml";
    let expected =
        "https://stock.finance.sina.com.cn/stock/go.php/vReport_Show/kind/search/rptid/1/index.phtml";
    let client = SinaClient::with_transport(FixtureTransport::new(HashMap::from([(
        url(1),
        response(page(
            "sh600396",
            1,
            &[row("2026-07-24 22:35", source_url, "来源标题")],
            false,
        )),
    )])));

    let batch = client.instrument_news(&request(1)).unwrap();

    assert_eq!(batch.records()[0].item_id.as_str(), expected);
    assert_eq!(batch.records()[0].canonical_url.as_str(), expected);
}

#[test]
fn official_article_url_accepts_sina_source_query_separators_without_entity_rewriting() {
    let source_url = "http://vip.stock.finance.sina.com.cn/corp/view/vCB_AllBulletinDetail.php?stockid=600396&id=12535311";
    let expected = "https://vip.stock.finance.sina.com.cn/corp/view/vCB_AllBulletinDetail.php?stockid=600396&id=12535311";
    let client = SinaClient::with_transport(FixtureTransport::new(HashMap::from([(
        url(1),
        response(page(
            "sh600396",
            1,
            &[row("2026-07-24 22:35", source_url, "来源公告")],
            false,
        )),
    )])));

    let batch = client.instrument_news(&request(1)).unwrap();

    assert_eq!(batch.records()[0].canonical_url.as_str(), expected);
}

#[test]
fn known_html_entities_still_require_a_semicolon() {
    let source_url = "http://vip.stock.finance.sina.com.cn/corp/view/detail.php?stockid=600396&amp";
    let client = SinaClient::with_transport(FixtureTransport::new(HashMap::from([(
        url(1),
        response(page(
            "sh600396",
            1,
            &[row("2026-07-24 22:35", source_url, "错误实体")],
            false,
        )),
    )])));

    assert!(matches!(
        client.instrument_news(&request(1)),
        Err(SinaError::Protocol(message)) if message.contains("HTML entity is not closed")
    ));
}

#[test]
fn range_filter_and_limit_run_after_stable_cross_page_deduplication() {
    let one = "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml";
    let two = "https://finance.sina.com.cn/roll/2026-07-23/doc-two.shtml";
    let first = response(page(
        "sh600396",
        1,
        &[row("2026-07-24 22:35", one, "一")],
        true,
    ));
    let second = response_at(
        page(
            "sh600396",
            2,
            &[
                row("2026-07-24 22:35", one, "一"),
                row("2026-07-23 10:24", two, "二"),
            ],
            false,
        ),
        OBSERVED_UNIX + 1,
    );
    let transport = FixtureTransport::new(HashMap::from([(url(1), first), (url(2), second)]));
    let client = SinaClient::with_transport(transport.clone());
    let request = request(1)
        .with_range(
            IsoDate::new("2026-07-23").unwrap(),
            IsoDate::new("2026-07-23").unwrap(),
        )
        .unwrap();

    let batch = client.instrument_news(&request).unwrap();

    assert_eq!(transport.requested(), vec![url(1), url(2)]);
    assert_eq!(batch.records().len(), 1);
    assert_eq!(batch.records()[0].canonical_url.as_str(), two);
}

#[test]
fn overlapping_page_windows_are_merged_and_stably_sorted_before_limit() {
    let one = "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml";
    let two = "https://finance.sina.com.cn/roll/2026-07-24/doc-two.shtml";
    let three = "https://finance.sina.com.cn/roll/2026-07-24/doc-three.shtml";
    let four = "https://finance.sina.com.cn/roll/2026-07-24/doc-four.shtml";
    let transport = FixtureTransport::new(HashMap::from([
        (
            url(1),
            response(page(
                "sh600396",
                1,
                &[
                    row("2026-07-24 22:35", one, "一"),
                    row("2026-07-24 22:30", three, "三"),
                ],
                true,
            )),
        ),
        (
            url(2),
            response_at(
                page(
                    "sh600396",
                    2,
                    &[
                        row("2026-07-24 22:34", two, "二"),
                        row("2026-07-24 22:29", four, "四"),
                    ],
                    false,
                ),
                OBSERVED_UNIX + 1,
            ),
        ),
    ]));
    let client = SinaClient::with_transport(transport.clone());

    let batch = client.instrument_news(&request(3)).unwrap();

    assert_eq!(transport.requested(), vec![url(1), url(2)]);
    assert_eq!(
        batch
            .records()
            .iter()
            .map(|record| record.canonical_url.as_str())
            .collect::<Vec<_>>(),
        vec![one, two, three]
    );
}

#[test]
fn later_page_window_extrema_cannot_move_forward() {
    let first = &[
        row(
            "2026-07-24 22:35",
            "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml",
            "一",
        ),
        row(
            "2026-07-24 22:30",
            "https://finance.sina.com.cn/roll/2026-07-24/doc-two.shtml",
            "二",
        ),
    ];
    let invalid_pages = [
        vec![
            row(
                "2026-07-24 22:36",
                "https://finance.sina.com.cn/roll/2026-07-24/doc-three.shtml",
                "三",
            ),
            row(
                "2026-07-24 22:29",
                "https://finance.sina.com.cn/roll/2026-07-24/doc-four.shtml",
                "四",
            ),
        ],
        vec![
            row(
                "2026-07-24 22:34",
                "https://finance.sina.com.cn/roll/2026-07-24/doc-five.shtml",
                "五",
            ),
            row(
                "2026-07-24 22:31",
                "https://finance.sina.com.cn/roll/2026-07-24/doc-six.shtml",
                "六",
            ),
        ],
    ];

    for invalid in invalid_pages {
        let client = SinaClient::with_transport(FixtureTransport::new(HashMap::from([
            (url(1), response(page("sh600396", 1, first, true))),
            (url(2), response(page("sh600396", 2, &invalid, false))),
        ])));

        assert!(matches!(
            client.instrument_news(&request(3)),
            Err(SinaError::Protocol(message))
                if message.contains("news page window increased across pages")
        ));
    }
}

#[test]
fn valid_filtered_empty_is_complete_but_malformed_empty_page_fails() {
    let canonical = "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml";
    let valid_client = SinaClient::with_transport(FixtureTransport::new(HashMap::from([(
        url(1),
        response(page(
            "sh600396",
            1,
            &[row("2026-07-24 22:35", canonical, "来源标题")],
            false,
        )),
    )])));
    let outside_range = request(1)
        .with_range(
            IsoDate::new("2026-07-20").unwrap(),
            IsoDate::new("2026-07-20").unwrap(),
        )
        .unwrap();

    let batch = valid_client.instrument_news(&outside_range).unwrap();

    assert!(batch.records().is_empty());
    assert!(batch.quality().is_complete());
    assert_eq!(
        batch.provenance().source_at(),
        Some("2026-07-24T22:35:00+08:00")
    );
    assert!(batch.provenance().batch_id().is_some());

    let malformed_client = SinaClient::with_transport(FixtureTransport::new(HashMap::from([(
        url(1),
        response(page("sh600396", 1, &[], false)),
    )])));
    assert!(matches!(
        malformed_client.instrument_news(&outside_range),
        Err(SinaError::Protocol(message)) if message.contains("empty instrument-news datelist")
    ));
}

#[test]
fn conflicting_duplicate_fails_the_atomic_batch() {
    let canonical = "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml";
    let transport = FixtureTransport::new(HashMap::from([
        (
            url(1),
            response(page(
                "sh600396",
                1,
                &[row("2026-07-24 22:35", canonical, "标题一")],
                true,
            )),
        ),
        (
            url(2),
            response(page(
                "sh600396",
                2,
                &[row("2026-07-24 22:35", canonical, "标题二")],
                false,
            )),
        ),
    ]));
    let client = SinaClient::with_transport(transport);

    assert!(matches!(
        client.instrument_news(&request(2)),
        Err(SinaError::Protocol(message)) if message.contains("conflicting duplicate")
    ));
}

#[test]
fn response_contract_failures_are_explicit() {
    let valid_row = row(
        "2026-07-24 22:35",
        "https://finance.sina.com.cn/roll/2026-07-24/doc-one.shtml",
        "标题",
    );
    let cases = [
        (
            "status",
            DocumentResponse::new(
                503,
                "text/html; charset=gbk",
                page("sh600396", 1, std::slice::from_ref(&valid_row), false),
                OBSERVED_UNIX,
            ),
        ),
        (
            "MIME",
            DocumentResponse::new(
                200,
                "application/json",
                page("sh600396", 1, std::slice::from_ref(&valid_row), false),
                OBSERVED_UNIX,
            ),
        ),
        (
            "identity",
            response(page("sz000001", 1, std::slice::from_ref(&valid_row), false)),
        ),
        ("empty", response(page("sh600396", 1, &[], false))),
        (
            "future",
            response(page(
                "sh600396",
                1,
                &[row(
                    "2099-01-01 00:00",
                    "https://finance.sina.com.cn/roll/2099-01-01/doc-future.shtml",
                    "未来",
                )],
                false,
            )),
        ),
        (
            "canonical",
            response(page(
                "sh600396",
                1,
                &[row(
                    "2026-07-24 22:35",
                    "https://example.com/not-sina",
                    "外部",
                )],
                false,
            )),
        ),
        (
            "credentials",
            response(page(
                "sh600396",
                1,
                &[row(
                    "2026-07-24 22:35",
                    "https://user@finance.sina.com.cn/roll/doc.shtml",
                    "带凭据",
                )],
                false,
            )),
        ),
        (
            "explicit port",
            response(page(
                "sh600396",
                1,
                &[row(
                    "2026-07-24 22:35",
                    "https://finance.sina.com.cn:443/roll/doc.shtml",
                    "带端口",
                )],
                false,
            )),
        ),
    ];

    for (expected, fixture) in cases {
        let client =
            SinaClient::with_transport(FixtureTransport::new(HashMap::from([(url(1), fixture)])));
        let error = client.instrument_news(&request(1)).unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in {error}"
        );
    }
}

#[test]
fn unsupported_and_request_bounds_fail_before_network() {
    let transport = FixtureTransport::new(HashMap::new());
    let client = SinaClient::with_transport(transport.clone());
    let too_large = InstrumentDateRangeRequest::new(sh(), PositiveU32::new(201).unwrap()).unwrap();
    let beijing = InstrumentDateRangeRequest::new(bj(), PositiveU32::new(1).unwrap()).unwrap();

    assert!(matches!(
        client.instrument_news(&too_large),
        Err(SinaError::InvalidRequest(_))
    ));
    assert!(matches!(
        client.instrument_news(&beijing),
        Err(SinaError::Unsupported(_))
    ));
    assert!(matches!(
        client.global_news(PositiveU32::new(1).unwrap()),
        Err(SinaError::Unsupported(_))
    ));
    assert!(transport.requested().is_empty());
}
