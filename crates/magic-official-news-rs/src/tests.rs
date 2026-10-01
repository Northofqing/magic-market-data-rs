use super::*;
use magic_market_transport::{HttpResponse, TransportError};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn caller_timeout_can_shorten_but_cannot_widen_admitted_http_bound() {
    assert!(bounded_timeout(Duration::ZERO).is_err());
    assert_eq!(
        bounded_timeout(Duration::from_secs(1)).unwrap(),
        Duration::from_secs(1)
    );
    assert_eq!(
        bounded_timeout(Duration::from_secs(60)).unwrap(),
        Duration::from_secs(15)
    );
    assert!(OfficialNewsClient::with_timeout(OfficialSource::Nbs, Duration::ZERO).is_err());
    assert!(
        OfficialNewsClient::with_timeout(OfficialSource::Nbs, Duration::from_millis(500)).is_err()
    );
}

struct FixtureTransport {
    body: Vec<u8>,
    calls: Arc<AtomicUsize>,
}
impl HttpTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(
            HttpResponse::new(
                200,
                request.url(),
                Some(
                    if request.url().contains("/api-gateway/")
                        || request.url().contains("/searchList/")
                    {
                        "application/json; charset=utf-8"
                    } else {
                        "text/html; charset=utf-8"
                    }
                    .into(),
                ),
                self.body.clone(),
            ),
        )
    }
}

fn fixture(source: OfficialSource, html: &str) -> (OfficialNewsClient, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    (
        OfficialNewsClient::with_transport(
            source,
            FixtureTransport {
                body: html.as_bytes().to_vec(),
                calls: calls.clone(),
            },
        )
        .unwrap(),
        calls,
    )
}

fn nbs_article(label: &str) -> String {
    format!(
        r#"<meta name="ArticleTitle" content="统计发布"><meta name="PubDate" content="{label}"><meta name="createDate" content="2099-12-31 23:59:59"><div class="detail-text-content"><div class="TRS_UEDITOR"><p>数值 0 保留</p><script>do_not_include_secret_script()</script><SCRIPT>upper_script()</SCRIPT><style>.hidden{{}}</style><p>原文第二段</p></div></div>"#
    )
}

#[test]
fn original_publication_preserves_actual_precision_and_ignores_page_build_time() {
    for (label, precision) in [
        ("2026-09-30", PublicationPrecision::Date),
        ("2026/09/30 09:30", PublicationPrecision::Minute),
        ("2026-09-30 09:30:15", PublicationPrecision::Second),
    ] {
        let (client, _) = fixture(OfficialSource::Nbs, &nbs_article(label));
        let article = client
            .probe_article("https://www.stats.gov.cn/sj/zxfb/202609/t20260930_1965449.html")
            .unwrap();
        assert_eq!(article.publication_label, label);
        assert_eq!(article.precision, precision);
        assert_eq!(
            article.publication_label_origin,
            PublicationLabelOrigin::ArticleMetadata
        );
        assert_eq!(article.published_date.as_str(), "2026-09-30");
        assert_eq!(article.content, "数值 0 保留 原文第二段");
        assert_eq!(article.response_sha256.len(), 64);
        assert!(!article.observed_at.is_empty());
    }
}

#[test]
fn unsafe_original_urls_fail_before_transport() {
    let (client, calls) = fixture(OfficialSource::Nbs, &nbs_article("2026-09-30"));
    for url in [
        "http://www.stats.gov.cn/sj/zxfb/202609/t20260930_1965449.html",
        "https://www.stats.gov.cn.evil.test/sj/zxfb/202609/t20260930_1965449.html",
        "https://www.stats.gov.cn/other/202609/t20260930_1965449.html",
        "https://user@www.stats.gov.cn/sj/zxfb/202609/t20260930_1965449.html",
        "https://www.stats.gov.cn/sj/zxfb/202609/t20260930_1965449.html?next=evil",
        "https://www.stats.gov.cn/sj/zxfb/202609/t20260930_1965449.html#body",
        "https://www.stats.gov.cn/sj/zxfb/202609/%7420260930_1965449.html",
        "https://www.stats.gov.cn/other/../sj/zxfb/202609/t20260930_1965449.html",
    ] {
        assert!(
            matches!(
                client.probe_article(url),
                Err(OfficialNewsError::InvalidRequest(_))
            ),
            "{url}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn source_admission_blocks_formal_calls_before_transport() {
    let (client, calls) = fixture(OfficialSource::Gacc, "");
    assert!(matches!(
        client.latest(1),
        Err(OfficialNewsError::Unadmitted(OfficialSource::Gacc))
    ));
    assert!(matches!(
        client.article("https://www.customs.gov.cn/customs/302249/302425/1/index.html"),
        Err(OfficialNewsError::Unadmitted(OfficialSource::Gacc))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn source_metadata_conflicts_and_empty_body_are_not_replaced_by_page_text() {
    for html in [
        nbs_article("2026-02-30"),
        nbs_article("2026-09-30 24:00"),
        nbs_article("2026-09-30 09:30:60"),
        nbs_article("2026-09-30 09:+3"),
        nbs_article("2099-09-30"),
        nbs_article("2026-09-30").replace("name=\"PubDate\"", "name=\"MakeTime\""),
        nbs_article("2026-09-30") + "<meta name=\"PubDate\" content=\"2026-09-29\">",
        nbs_article("2026-09-30").replace("TRS_UEDITOR", "unknown_template"),
        nbs_article("2026-09-30")
            .replace("数值 0 保留", "")
            .replace("原文第二段", ""),
    ] {
        let (client, _) = fixture(OfficialSource::Nbs, &html);
        assert!(client
            .probe_article("https://www.stats.gov.cn/sj/zxfb/202609/t20260930_1965449.html")
            .is_err());
    }
}

#[test]
fn pbc_parent_dates_ndrc_separators_and_original_bodies_use_exact_containers() {
    let pbc = r#"<table><tr><td><font class="newslist_style"><a href="./1/index.html">央行发布</a></font><span>2026-09-29</span></td></tr></table>"#;
    let (client, _) = fixture(OfficialSource::Pbc, pbc);
    let page = client.probe_latest(1).unwrap();
    assert_eq!(page.items[0].published_date.as_str(), "2026-09-29");
    let ndrc = r#"<ul class="u-list"><li><a href="./202609/t20260930_1.html">发改委发布</a><span>2026-09-30</span></li><li class="empty"></li></ul>"#;
    let (client, _) = fixture(OfficialSource::Ndrc, ndrc);
    assert_eq!(client.probe_latest(2).unwrap().source_rows, 1);
    let (client, _) = fixture(
        OfficialSource::Ndrc,
        &ndrc.replace("class=\"empty\"></li>", "class=\"empty\">隐藏条目</li>"),
    );
    assert!(client.probe_latest(1).is_err());
    for (source, url, container, label, precision) in [
        (OfficialSource::Pbc, "https://www.pbc.gov.cn/goutongjiaoliu/113456/113469/1/index.html", "<div id=\"zoom\">原文 &amp; 数值&nbsp;0</div>", "2026-09-29", PublicationPrecision::Date),
        (OfficialSource::Ndrc, "https://www.ndrc.gov.cn/xwdt/xwfb/202609/t20260930_1.html", "<div class=\"article_con\"><div class=\"TRS_Editor\">原文 &amp; 数值&nbsp;0</div></div>", "2026-09-30 16:43:37", PublicationPrecision::Second),
        (OfficialSource::Mofcom, "https://www.mofcom.gov.cn/xwfb/rcxwfb/art/2026/art_0123456789abcdef0123456789abcdef.html", "<div class=\"art-con\" ergodic=\"article\">原文 &amp; 数值&nbsp;0</div>", "2026-09-29 17:03", PublicationPrecision::Minute),
    ] {
        let html = format!("<meta name=\"ArticleTitle\" content=\"官方发布\"><meta name=\"PubDate\" content=\"{label}\">{container}");
        let (client, _) = fixture(source, &html);
        let article = client.probe_article(url).unwrap();
        assert_eq!(article.precision, precision);
        assert_eq!(article.content, "原文 & 数值 0");
    }
}

#[test]
fn mofcom_unit_envelope_pagination_and_full_page_validation_are_strict() {
    let html = r#"<ul class="txtList_01"><li><a href="/xwfb/rcxwfb/art/2026/art_0123456789abcdef0123456789abcdef.html">商务发布</a><span>[2026-09-29]</span></li></ul><div class="pagination" rows="1" count="8242" pageNo="1"></div>"#;
    let response = |html: &str| {
        serde_json::json!({"code":"200","success":true,"data":{"html":html}}).to_string()
    };
    let (client, _) = fixture(OfficialSource::Mofcom, &response(html));
    let page = client.probe_latest(1).unwrap();
    assert_eq!(page.source_rows, 1);
    assert_eq!(page.items[0].publication_label, "[2026-09-29]");
    assert!(page.response_url.as_str().contains("/api-gateway/"));
    assert!(!page.listing_url.as_str().contains("/api-gateway/"));
    for body in [
        response(&html.replace("rows=\"1\"", "rows=\"2\"")),
        response(&html.replace("pageNo=\"1\"", "pageNo=\"2\"")),
        response(&html.replace("count=\"8242\"", "count=\"0\"")),
        response(&html.replace("<div class=\"pagination\"", "<div class=\"unrecognized\"")),
        response(&html.replace("href=\"/xwfb", "href=\"http://www.mofcom.gov.cn/xwfb")),
        r#"{"code":200,"success":true,"data":{"html":"unexpected"}}"#.to_owned(),
        r#"{"code":"200","success":false,"data":{"html":"unexpected"}}"#.to_owned(),
    ] {
        let (client, _) = fixture(OfficialSource::Mofcom, &body);
        assert!(client.probe_latest(1).is_err());
    }
}

#[test]
fn narrowed_mof_and_miit_profiles_keep_original_titles_and_visible_publication_time() {
    let (client, _) = fixture(
        OfficialSource::Mof,
        r#"<ul class="liBox"><li><a href="./202608/t20260826_3996112.htm" title="财政部政策完整标题">截断</a><span>2026-08-26</span></li></ul>"#,
    );
    let page = client.probe_latest(1).unwrap();
    assert_eq!(
        page.items[0].canonical_url.as_str(),
        "https://zhs.mof.gov.cn/zhengcefabu/202608/t20260826_3996112.htm"
    );
    assert_eq!(page.items[0].title, "财政部政策完整标题");
    let html = r#"<meta name="ArticleTitle" content="财政部政策"><meta name="PubDate" content="2026-08-26 10:22:00"><h2 class="title_con">财政部政策</h2><div class="docreltime"><span>发布日期：2026年08月26日</span></div><div class="my_doccontent"><div class="TRS_Editor">财政部政策原文</div></div>"#;
    let (client, _) = fixture(OfficialSource::Mof, html);
    let article = client
        .probe_article("https://zhs.mof.gov.cn/zhengcefabu/202608/t20260826_3996112.htm")
        .unwrap();
    assert_eq!(article.published_date.as_str(), "2026-08-26");
    assert_eq!(article.publication_label, "发布日期：2026年08月26日");
    assert_eq!(article.precision, PublicationPrecision::Date);
    assert_eq!(
        article.publication_label_origin,
        PublicationLabelOrigin::VisibleArticleDate
    );
    let (client, calls) = fixture(OfficialSource::Mof, html);
    assert!(client
        .probe_article(
            "https://www.mof.gov.cn/zhengwuxinxi/caizhengxinwen/202608/t20260826_3996112.htm"
        )
        .is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let (client, _) = fixture(
        OfficialSource::Mof,
        &html.replace("发布日期：2026年08月26日", "发布日期：2026年02月30日"),
    );
    assert!(client
        .probe_article("https://zhs.mof.gov.cn/zhengcefabu/202608/t20260826_3996112.htm")
        .is_err());
    let html = r#"<meta name="ArticleTitle" content="部领导活动"><meta name="PubDate" content="2026-09-29 14:46"><meta name="MakeTime" content="2026-09-29 14:46"><h1 id="con_title">部领导活动</h1><div class="cinfo center"><span id="con_time">发布时间：2026-09-28 19:19</span></div><div id="con_con" class="ccontent"><p>工信部原文</p></div>"#;
    let (client, _) = fixture(OfficialSource::Miit, html);
    let article = client
        .probe_article(
            "https://www.miit.gov.cn/xwfb/bldhd/art/2026/art_b6f2e487b7a74338aa4999e8fd91ecc2.html",
        )
        .unwrap();
    assert_eq!(article.published_date.as_str(), "2026-09-28");
    assert_eq!(article.publication_label, "发布时间：2026-09-28 19:19");
    assert_eq!(article.precision, PublicationPrecision::Minute);
    assert_eq!(
        article.publication_label_origin,
        PublicationLabelOrigin::VisibleArticleDate
    );
    let (client, calls) = fixture(OfficialSource::Miit, html);
    assert!(client
        .probe_article(
            "https://www.miit.gov.cn/xwfb/zxzc/art/2026/art_b6f2e487b7a74338aa4999e8fd91ecc2.html"
        )
        .is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let (client, _) = fixture(
        OfficialSource::Miit,
        &html.replace("id=\"con_time\"", "id=\"unknown\""),
    );
    assert!(client
        .probe_article(
            "https://www.miit.gov.cn/xwfb/bldhd/art/2026/art_b6f2e487b7a74338aa4999e8fd91ecc2.html"
        )
        .is_err());
}

#[test]
fn miit_unit_validates_all_rows_before_limit_and_exact_page_identity() {
    let row = r#"<li class="cf"><a class="fl" href="/xwfb/bldhd/art/2026/art_c37cffd9af9a4b3aa6379d153ed83620.html" title="工信部完整标题">截断</a><span class="fr">2026-09-30</span></li>"#;
    let html = format!(
        r#"<div id="右侧内容"><div class="page-content"><ul>{row}</ul><div class="pagination" rows="1" count="2725" pageNo="1"></div></div></div>"#
    );
    let response = |html: &str| {
        serde_json::json!({"code":"200","success":true,"data":{"html":html}}).to_string()
    };
    let (client, _) = fixture(OfficialSource::Miit, &response(&html));
    let page = client.probe_latest(1).unwrap();
    assert_eq!(page.items[0].title, "工信部完整标题");
    assert!(page
        .response_url
        .as_str()
        .contains("pageId=d3e2bede1bc045e2875fc7161c01db7d"));
    for broken in [
        html.replace("rows=\"1\"", "rows=\"24\""),
        html.replace("pageNo=\"1\"", "pageNo=\"2\""),
        html.replace("count=\"2725\"", "count=\"0\""),
        html.replace(
            "</ul>",
            &format!(
                "{}</ul>",
                row.replace("href=\"/xwfb", "href=\"http://www.miit.gov.cn/xwfb")
            ),
        )
        .replace("rows=\"1\"", "rows=\"2\""),
    ] {
        let (client, _) = fixture(OfficialSource::Miit, &response(&broken));
        assert!(client.probe_latest(1).is_err());
    }
}

#[test]
fn nea_work_updates_validate_both_source_groups_and_use_visible_date_without_metadata_fallback() {
    let mut html =
        "<div class=\"xwzx-page01\"><div class=\"xwzx-yw-right\"><div class=\"xwzx-yw-box\">"
            .to_owned();
    for (group, id) in [
        "91fb3999d1964141b668e4a4cef4ed98",
        "64763711c745408cb6f3bc0895f37649",
    ]
    .iter()
    .enumerate()
    {
        html.push_str(&format!("<ul class=\"list01\" data=\"datasource:{id}\">"));
        for index in 0..5 {
            html.push_str(&format!("<li><a href=\"../20260930/{:032x}/c.html\">能源局发布{index}</a><span class=\"date\">(2026-09-30)</span></li>", 1 + group * 5 + index));
        }
        html.push_str("</ul>");
    }
    html.push_str("</div><div class=\"more more_dw\"><a href=\"/news/jwzdt.htm\">更多</a></div></div></div><div class=\"other\"><a href=\"http://outside.test\">范围外其他栏目</a></div>");
    let (client, _) = fixture(OfficialSource::Nea, &html);
    assert_eq!(client.probe_latest(1).unwrap().source_rows, 10);
    for broken in [
        html.replace(
            "datasource:64763711c745408cb6f3bc0895f37649",
            "datasource:unknown",
        ),
        html.replace(
            "../20260930/0000000000000000000000000000000a/c.html",
            "http://www.nea.gov.cn/20260930/0000000000000000000000000000000a/c.html",
        ),
        html.replace("/news/jwzdt.htm", "/other.htm"),
        html.replacen("<span class=\"date\">(2026-09-30)</span>", "", 1),
    ] {
        let (client, _) = fixture(OfficialSource::Nea, &broken);
        assert!(client.probe_latest(1).is_err());
    }
    let article = r#"<meta name="ArticleTitle" content="能源局发布"><meta name="PublishDate" content="2026-09-30"><meta name="PubDate" content="2026-09-30 14:15:39"><div class="article-title"><div class="titles">能源局发布</div><span class="times">发布时间：2026-09-30</span></div><span id="detailContent"><p>能源局原文</p></span>"#;
    let url = "https://www.nea.gov.cn/20260930/9ca69805239243efad85690747000ff9/c.html";
    let (client, _) = fixture(OfficialSource::Nea, article);
    let original = client.probe_article(url).unwrap();
    assert_eq!(original.precision, PublicationPrecision::Date);
    assert_eq!(
        original.publication_label_origin,
        PublicationLabelOrigin::VisibleArticleDate
    );
    for broken in [
        article.replace("class=\"times\"", "class=\"unknown\""),
        article.replace("发布时间：2026-09-30", "发布时间：2026-09-30 14:15:39"),
        article.replace(
            "</div><span id=\"detailContent\">",
            "<span class=\"times\">发布时间：2026-09-29</span></div><span id=\"detailContent\">",
        ),
    ] {
        let (client, _) = fixture(OfficialSource::Nea, &broken);
        assert!(client.probe_article(url).is_err());
    }
}

#[test]
fn csrc_list_time_and_visible_original_date_keep_distinct_precision() {
    let body = serde_json::json!({"data":{"page":1,"rows":18,"channelId":"a1a078ee0bc54721ab6b148884c784a8","relateSubChannels":"true","total":2,"results":[{"title":"监管发布","url":"//www.csrc.gov.cn/csrc/c100028/c7661513/content.shtml","publishedTimeStr":"2026-09-30 15:15:26"},{"title":"监管发布","url":"//www.csrc.gov.cn/csrc/c106311/c7657131/content.shtml","publishedTimeStr":"2026-09-30 19:33:27"}]}}).to_string();
    let (client, _) = fixture(OfficialSource::Csrc, &body);
    let page = client.probe_latest(1).unwrap();
    assert_eq!(page.items[0].publication_label, "2026-09-30 15:15:26");
    assert_eq!(page.items[0].precision, PublicationPrecision::Second);
    assert_eq!(
        page.items[0].publication_label_origin,
        PublicationLabelOrigin::ListingApi
    );
    let html = r#"<meta name="ArticleTitle" content="监管发布"><meta name="PubDate" content="2026-09-30 15:35:13"><meta name="others" content="页面生成时间 2026-09-30 15:35:13"><div class="main"><div class="content"><h2>监管发布</h2><div class="info"><p class="fl">日期：2026-09-30     来源：证监会</p></div><div class="detail-news">监管原文</div></div></div>"#;
    let (client, _) = fixture(OfficialSource::Csrc, html);
    let article = client
        .probe_article("https://www.csrc.gov.cn/csrc/c100028/c7661513/content.shtml")
        .unwrap();
    assert_eq!(article.publication_label, "日期：2026-09-30 来源：证监会");
    assert_eq!(article.precision, PublicationPrecision::Date);
    assert_eq!(
        article.publication_label_origin,
        PublicationLabelOrigin::VisibleArticleDate
    );
    assert_eq!(article.published_date, page.items[0].published_date);
    assert_eq!(article.content, "监管原文");
    let (client, _) = fixture(OfficialSource::Csrc, html);
    assert!(client
        .probe_article("https://www.csrc.gov.cn/csrc/c106311/c7657131/content.shtml")
        .is_ok());
    for broken in [
        html.replace("日期：2026-09-30", "日期：2026-02-30"),
        html.replace("日期：2026-09-30", "日期：2026-09-30 15:35"),
        html.replace("日期：2026-09-30", "更新时间：2026-09-30"),
        html.replace("<p class=\"fl\">", "<p class=\"unknown\">"),
        html.replace(
            "</div><div class=\"detail-news\">",
            "<p class=\"fl\">日期：2026-09-29 来源：证监会</p></div><div class=\"detail-news\">",
        ),
        html.replace("<h2>监管发布</h2>", "<h2>不同标题</h2>"),
    ] {
        let (client, _) = fixture(OfficialSource::Csrc, &broken);
        assert!(client
            .probe_article("https://www.csrc.gov.cn/csrc/c100028/c7661513/content.shtml")
            .is_err());
    }
}

#[test]
fn csrc_aggregate_rejects_wrong_page_channel_partial_page_and_unknown_original_paths() {
    let row = serde_json::json!({"title":"监管发布","url":"//www.csrc.gov.cn/csrc/c100028/c7661513/content.shtml","publishedTimeStr":"2026-09-30 15:15:26"});
    let body = serde_json::json!({"data":{"page":1,"rows":18,"channelId":"a1a078ee0bc54721ab6b148884c784a8","relateSubChannels":"true","total":1,"results":[row]}});
    let (client, _) = fixture(OfficialSource::Csrc, &body.to_string());
    assert_eq!(client.probe_latest(1).unwrap().source_rows, 1);
    for (field, value) in [
        ("page", serde_json::json!(2)),
        ("rows", serde_json::json!(20)),
        ("channelId", serde_json::json!("other_channel")),
        ("relateSubChannels", serde_json::json!("false")),
        ("total", serde_json::json!(3197)),
    ] {
        let mut changed = body.clone();
        changed["data"][field] = value;
        let (client, _) = fixture(OfficialSource::Csrc, &changed.to_string());
        assert!(client.probe_latest(1).is_err());
    }
    let mut changed = body;
    changed["data"]["total"] = serde_json::json!(2);
    let mut unsafe_row = row;
    unsafe_row["url"] = serde_json::json!("//www.csrc.gov.cn/csrc/c100029/c7657131/content.shtml");
    changed["data"]["results"]
        .as_array_mut()
        .unwrap()
        .push(unsafe_row);
    let (client, _) = fixture(OfficialSource::Csrc, &changed.to_string());
    assert!(client.probe_latest(1).is_err());
}

#[test]
fn request_bounds_and_observation_date_reject_invalid_inputs() {
    let (client, calls) = fixture(OfficialSource::Nbs, "");
    for limit in [0, 21, u32::MAX] {
        assert!(client.probe_latest(limit).is_err());
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(parse::article(
        OfficialSource::Nbs,
        "https://www.stats.gov.cn/sj/zxfb/202609/t20260930_1.html",
        &nbs_article("2026-10-03"),
        "2026-10-01T12:00:00Z".into(),
        "fixture".into()
    )
    .is_err());
}

#[test]
fn client_clones_share_serialized_request_pacing() {
    struct TimedTransport(Arc<Mutex<Vec<Instant>>>);
    impl HttpTransport for TimedTransport {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            self.0.lock().unwrap().push(Instant::now());
            Ok(HttpResponse::new(
                200,
                request.url(),
                Some("text/html".into()),
                listing("").into_bytes(),
            ))
        }
    }
    let starts = Arc::new(Mutex::new(Vec::new()));
    let client =
        OfficialNewsClient::with_transport(OfficialSource::Nbs, TimedTransport(starts.clone()))
            .unwrap();
    let other = client.clone();
    let first = std::thread::spawn(move || client.probe_latest(1).unwrap());
    let second = std::thread::spawn(move || other.probe_latest(1).unwrap());
    first.join().unwrap();
    second.join().unwrap();
    let starts = starts.lock().unwrap();
    assert_eq!(starts.len(), 2);
    assert!(starts[1].duration_since(starts[0]) >= Duration::from_millis(990));
}

fn listing(extra: &str) -> String {
    format!(
        r#"<div class="list-content"><ul><li><a class="pc_1600" href="./202609/t20260930_1.html" title="完整标题">完整标题</a><a class="pchide" href="./202609/t20260930_1.html">截断…</a><span>2026-09-30</span></li>{extra}</ul></div>"#
    )
}

#[test]
fn validates_source_rows_after_limit_and_keeps_desktop_title_evidence() {
    let (client, _) = fixture(OfficialSource::Nbs, &listing(""));
    let page = client.probe_latest(1).unwrap();
    assert_eq!(page.source_rows, 1);
    assert_eq!(page.items[0].title, "完整标题");
    assert_eq!(page.items[0].published_date.as_str(), "2026-09-30");
    assert_eq!(
        page.items[0].publication_label_origin,
        PublicationLabelOrigin::ListingHtml
    );
    for extra in [
        r#"<li><a class="pc_1600" href="http://www.stats.gov.cn/sj/zxfb/202609/t20260929_2.html">unsafe</a><span>2026-09-29</span></li>"#,
        r#"<li><a class="pc_1600" href="./202609/t20260930_1.html">duplicate</a><span>2026-09-30</span></li>"#,
        r#"<li><a class="pc_1600" href="./202609/t20260929_2.html">missing date</a></li>"#,
    ] {
        let (client, _) = fixture(OfficialSource::Nbs, &listing(extra));
        assert!(client.probe_latest(1).is_err());
    }
}

#[test]
fn injected_redirect_media_and_body_violations_are_rejected() {
    struct BadResponse {
        kind: u8,
    }
    impl HttpTransport for BadResponse {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            Ok(match self.kind {
                0 => HttpResponse::new(
                    200,
                    "https://www.stats.gov.cn/sj/zxfb/202609/t20260929_2.html",
                    Some("text/html".into()),
                    nbs_article("2026-09-30").into_bytes(),
                ),
                1 => HttpResponse::new(
                    200,
                    request.url(),
                    Some("application/json".into()),
                    b"{}".to_vec(),
                ),
                2 => HttpResponse::new(
                    200,
                    request.url(),
                    Some("text/html".into()),
                    vec![b'a'; MAX_BODY_BYTES + 1],
                ),
                _ => HttpResponse::new(200, request.url(), Some("text/html".into()), vec![255]),
            })
        }
    }
    for kind in 0..4 {
        let client =
            OfficialNewsClient::with_transport(OfficialSource::Nbs, BadResponse { kind }).unwrap();
        assert!(client
            .probe_article("https://www.stats.gov.cn/sj/zxfb/202609/t20260930_1965449.html")
            .is_err());
    }
}
