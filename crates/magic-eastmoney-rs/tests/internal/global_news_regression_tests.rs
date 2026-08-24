use super::{
    exact_attribute, normalize_global_article_url, parse_complete_global_news_page,
    parse_global_news, parse_global_news_row, parse_news,
};
use crate::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::{
    AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, IsoDate, NewsProvider,
    PositiveU32,
};

#[derive(Clone)]
struct HtmlFixture {
    body: Vec<u8>,
}

impl EastmoneyTransport for HtmlFixture {
    fn get(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Ok(self.body.clone())
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Transport(
            "fixture does not support POST".into(),
        ))
    }
}

fn global_fixture() -> String {
    r#"
          <html><body>
          <div id="artList" class="contain">
            <ul>
              <li>
                <span> 2026-07-25 08:40 </span>[<a href="finance.html">财经</a>]<a href="http://finance.eastmoney.com/a/202607253821086055.html" title="美迪西：采用更加灵活的报价策略" target="_blank">美迪西：采用更加灵活的报价策略</a>
              </li>
              <li>
                <span>2026-07-25 08:38</span>[<a href="finance.html">财经</a>]<a href="https://finance.eastmoney.com/a/202607253821083017.html" title="美股牛市四周年近在眼前" target="_blank">美股牛市四周年近在眼前</a>
              </li>
              <li>
                <span>2026-07-25 08:38</span>[<a href="finance.html">财经</a>]<a href="http://finance.eastmoney.com/a/202607253821081234.html" title="央行发布最新政策" target="_blank">央行发布最新政策</a>
              </li>
            </ul>
          </div>
          <div class="PageBox"><div class="Page">1</div></div>
          </body></html>
        "#
        .to_owned()
}

fn request() -> InstrumentDateRangeRequest {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
    InstrumentDateRangeRequest::new(instrument, PositiveU32::new(5).unwrap())
        .unwrap()
        .with_range(
            IsoDate::new("2026-07-01").unwrap(),
            IsoDate::new("2026-07-31").unwrap(),
        )
        .unwrap()
}

#[test]
fn global_news_maps_the_verified_rolling_page() {
    let client = EastmoneyClient::with_transport(HtmlFixture {
        body: global_fixture().into_bytes(),
    });
    let batch = client.global_news(PositiveU32::new(2).unwrap()).unwrap();
    assert_eq!(batch.records().len(), 2);
    let item = &batch.records()[0];
    assert_eq!(item.item_id.as_str(), "202607253821086055");
    assert_eq!(item.title.as_str(), "美迪西：采用更加灵活的报价策略");
    assert!(item.summary.is_none());
    assert!(item.content.is_none());
    assert_eq!(item.publisher.as_str(), "东方财富网");
    assert_eq!(
        item.canonical_url.as_str(),
        "https://finance.eastmoney.com/a/202607253821086055.html"
    );
    assert_eq!(item.published_at.as_str(), "2026-07-25T08:40:00+08:00");
    assert_eq!(item.evidence.source_at(), Some("2026-07-25 08:40"));
    assert!(item.instruments.is_empty());
    assert_eq!(item.topics[0].as_str(), "财经");
    assert_eq!(item.language.as_str(), "zh-CN");
    assert_eq!(batch.provenance().source_at(), Some("2026-07-25 08:40"));
}

#[test]
fn global_news_accepts_exact_first_party_metadata_links() {
    for host in [
        "futures.eastmoney.com",
        "bond.eastmoney.com",
        "hk.eastmoney.com",
    ] {
        let expected = format!("https://{host}/a/202607253821086055.html");
        let fixture = global_fixture().replace(
            "http://finance.eastmoney.com/a/202607253821086055.html",
            &expected,
        );
        let batch = parse_global_news(fixture.as_bytes(), 1).unwrap();
        assert_eq!(batch.records()[0].canonical_url.as_str(), expected);
    }
}

#[test]
fn global_news_limit_is_bounded_before_transport() {
    let client = EastmoneyClient::with_transport(HtmlFixture { body: Vec::new() });
    assert!(matches!(
        client.global_news(PositiveU32::new(21).unwrap()),
        Err(EastmoneyError::InvalidRequest(message)) if message.contains("at most 20")
    ));
}

#[test]
fn global_news_requires_a_complete_strict_page_before_truncation() {
    let valid = global_fixture();
    let malformed_cases = [
        valid.replace(r#"<div id="artList" class="contain">"#, "<div>"),
        valid.replacen("财经", "股票", 1),
        valid.replacen("2026-07-25 08:40", "2026-07-25 25:00", 1),
        valid.replacen("2026-07-25 08:38", "2026-07-25 08:41", 1),
        valid.replacen(">央行发布最新政策</a>", ">与 title 不一致</a>", 1),
    ];
    for fixture in malformed_cases {
        assert!(parse_global_news(fixture.as_bytes(), 2).is_err());
    }

    let one_row = valid.replacen(
            r#"
              <li>
                <span>2026-07-25 08:38</span>[<a href="finance.html">财经</a>]<a href="https://finance.eastmoney.com/a/202607253821083017.html" title="美股牛市四周年近在眼前" target="_blank">美股牛市四周年近在眼前</a>
              </li>
              <li>
                <span>2026-07-25 08:38</span>[<a href="finance.html">财经</a>]<a href="http://finance.eastmoney.com/a/202607253821081234.html" title="央行发布最新政策" target="_blank">央行发布最新政策</a>
              </li>"#,
            "",
            1,
        );
    assert!(parse_global_news(one_row.as_bytes(), 2).is_err());
}

#[test]
fn global_news_accepts_multiple_complete_source_list_groups() {
    let fixture = global_fixture().replacen(
        "</li>\n              <li>",
        "</li>\n            </ul>\n            <ul>\n              <li>",
        1,
    );
    let batch = parse_global_news(fixture.as_bytes(), 3).unwrap();
    assert_eq!(batch.records().len(), 3);
}

#[test]
fn global_news_rejects_duplicate_id_url_and_noncanonical_urls() {
    let duplicate = global_fixture().replacen("202607253821081234", "202607253821086055", 1);
    assert!(parse_global_news(duplicate.as_bytes(), 2).is_err());
    for invalid in [
        "https://news.eastmoney.com/a/202607253821086055.html",
        "https://finance.eastmoney.com/b/202607253821086055.html",
        "https://finance.eastmoney.com/a/not-digits.html",
        "https://finance.eastmoney.com/a/202607253821086055.html?x=1",
        "https://finance.eastmoney.com.example/a/202607253821086055.html",
    ] {
        assert!(normalize_global_article_url(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn global_news_accepts_exact_official_hosts_and_preserves_them_in_canonical_url() {
    for host in [
        "finance.eastmoney.com",
        "global.eastmoney.com",
        "biz.eastmoney.com",
        "stock.eastmoney.com",
    ] {
        let input = format!("http://{host}/a/202607253821086055.html");
        let (item_id, canonical_url) = normalize_global_article_url(&input).unwrap();
        assert_eq!(item_id, "202607253821086055");
        assert_eq!(
            canonical_url,
            format!("https://{host}/a/202607253821086055.html")
        );
        assert!(normalize_global_article_url(&format!(
            "https://{host}.example/a/202607253821086055.html"
        ))
        .is_err());
    }
}

#[test]
fn global_news_html_boundaries_fail_closed_for_every_incomplete_shape() {
    let valid = global_fixture();
    let malformed_pages = [
        valid.replacen(
            r#"<div id="artList" class="contain">"#,
            concat!(
                r#"<div id="artList" class="contain"></div>"#,
                r#"<div id="artList" class="contain">"#
            ),
            1,
        ),
        valid.replace(r#"<div class="PageBox">"#, r#"<div class="Other">"#),
        valid.replacen(
            "</div>\n          <div class=\"PageBox\">",
            "<div class=\"PageBox\">",
            1,
        ),
        valid.replacen("<ul>", "outside<ul>", 1),
        valid.replacen("</ul>", "", 1),
        valid.replacen("<li>", "outside<li>", 1),
        valid.replacen("</li>", "", 1),
        valid.replacen(
            "<ul>\n              <li>",
            "<ul></ul><ul>\n              <li>",
            1,
        ),
    ];
    for page in malformed_pages {
        assert!(parse_complete_global_news_page(&page).is_err());
    }
    assert!(parse_complete_global_news_page(
        r#"<div id="artList" class="contain"></div><div class="PageBox">x</div>"#
    )
    .is_err());
    assert!(parse_global_news(&[0xff], 1).is_err());
}

#[test]
fn global_news_row_and_attribute_parser_reject_every_ambiguous_shape() {
    let prefix = r#"<span>2026-07-25 08:40</span>[<a href="finance.html">财经</a>]"#;
    let valid_link = r#"<a href="https://finance.eastmoney.com/a/123.html" title="标题">标题</a>"#;
    let malformed_rows = [
        "not-a-row".to_owned(),
        "<span>2026-07-25 08:40".to_owned(),
        format!("<span><b>2026-07-25 08:40</b></span>{valid_link}"),
        format!(r#"<span>2026-07-25 08:40</span>[股票]{valid_link}"#),
        format!("{prefix}<a"),
        format!("{prefix}<strong>标题</strong>"),
        format!(r#"{prefix}<a title="标题">标题</a>"#),
        format!(r#"{prefix}<a href="https://finance.eastmoney.com/a/123.html">标题</a>"#),
        format!(r#"{prefix}<a href="https://finance.eastmoney.com/a/123.html" title="标题">标题"#),
        format!("{prefix}{valid_link} trailing"),
    ];
    for row in malformed_rows {
        assert!(parse_global_news_row(&row).is_err(), "{row}");
    }

    assert!(exact_attribute(r#"<a href="">"#, "href").is_err());
    assert!(exact_attribute(r#"<a href="one" href="two">"#, "href").is_err());
    for invalid in [
        "relative",
        "https://finance.eastmoney.com",
        "https://finance.eastmoney.com/a/123.html#fragment",
        "https://finance.eastmoney.com/a/.html",
    ] {
        assert!(normalize_global_article_url(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn maps_and_filters_verified_news_jsonp() {
    let fixture = r#"jQuery_news({"result":{"cmsArticleWebOld":[
          {"code":"202607231234","title":"<em>华电辽能</em>公告",
           "content":"主营业务&nbsp;进展","date":"2026-07-23 10:01:00",
           "mediaName":"东方财富网","url":"http://stock.eastmoney.com/a/202607231234.html"},
          {"code":"old","title":"旧闻","content":"","date":"2026-06-01 10:00:00",
           "mediaName":"东方财富网","url":"https://stock.eastmoney.com/a/old.html"}
        ]}})"#
        .as_bytes();
    let batch = parse_news(fixture, &request()).unwrap();
    assert_eq!(batch.records().len(), 1);
    let item = &batch.records()[0];
    assert_eq!(item.item_id.as_str(), "202607231234");
    assert_eq!(item.title.as_str(), "华电辽能公告");
    assert_eq!(item.summary.as_ref().unwrap().as_str(), "主营业务 进展");
    assert!(item.content.is_none());
    assert_eq!(item.publisher.as_str(), "东方财富网");
    assert_eq!(
        item.canonical_url.as_str(),
        "https://stock.eastmoney.com/a/202607231234.html"
    );
    assert_eq!(item.published_at.as_str(), "2026-07-23 10:01:00");
    assert!(item.instruments.is_empty());
    assert!(item.topics.is_empty());
    assert_eq!(item.language.as_str(), "zh-CN");
    assert_eq!(item.evidence.source_at(), Some("2026-07-23 10:01:00"));
}

#[test]
fn instrument_news_is_an_explicit_unsupported_boundary() {
    let client = crate::EastmoneyClient::new().unwrap();
    assert!(matches!(
        client.instrument_news(&request()),
        Err(crate::EastmoneyError::Unsupported(message))
            if message.contains("structured source instrument identity")
    ));
}

#[test]
fn absent_family_and_unapproved_hosts_are_errors() {
    assert!(parse_news(br#"jQuery_news({"result":{"passportWeb":[]}})"#, &request()).is_err());
    let fixture = br#"jQuery_news({"result":{"cmsArticleWebOld":[{
          "code":"1","title":"x","date":"2026-07-23","mediaName":"x",
          "url":"https://stock.eastmoney.com.example/x"
        }]}})"#;
    assert!(parse_news(fixture, &request()).is_err());
}

#[test]
fn news_date_must_be_a_real_date_and_time() {
    for published_at in [
        "2026-02-30 10:01:00",
        "2026-07-23T10:01:00",
        "2026-07-23 24:01:00",
        "2026-07-23 10:60:00",
        "2026-07-23 10:01:60",
        "2026-07-23 10:01:00.bad",
    ] {
        let fixture = format!(
            r#"jQuery_news({{"result":{{"cmsArticleWebOld":[{{
                  "code":"1","title":"x","date":"{published_at}",
                  "mediaName":"x","url":"https://stock.eastmoney.com/a/1.html"
                }}]}}}})"#
        );
        assert!(
            parse_news(fixture.as_bytes(), &request()).is_err(),
            "{published_at}"
        );
    }
}
