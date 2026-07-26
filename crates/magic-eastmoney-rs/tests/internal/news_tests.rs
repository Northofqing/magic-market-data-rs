use super::{normalize_article_url, parse_news, strip_html, unwrap_jsonp};
use magic_market_core::{
    AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, IsoDate, NewsProvider,
    PositiveU32,
};

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

#[test]
fn global_news_jsonp_and_url_boundaries_are_explicit() {
    let capabilities = crate::EastmoneyClient::content_capabilities();
    assert!(capabilities.global_news);
    assert!(!capabilities.instrument_news);
    assert_eq!(unwrap_jsonp(" jQuery_news({}); ").unwrap(), "{}");
    assert_eq!(unwrap_jsonp("jQuery_news({})").unwrap(), "{}");
    assert!(unwrap_jsonp("other({})").is_err());
    assert!(unwrap_jsonp("jQuery_news( )").is_err());
    assert!(normalize_article_url("ftp://stock.eastmoney.com/a").is_err());
    assert!(normalize_article_url("https://example.com/a").is_err());
    assert_eq!(
        normalize_article_url("https://finance.eastmoney.com/a")
            .unwrap()
            .as_str(),
        "https://finance.eastmoney.com/a"
    );
    for host in [
        "caifuhao.eastmoney.com",
        "data.eastmoney.com",
        "stock.eastmoney.com",
    ] {
        assert!(normalize_article_url(&format!("http://{host}/a")).is_ok());
    }
    assert_eq!(
        strip_html("<p>A&nbsp;&amp;&lt;&gt;&quot;&#39;</p><script>ignored</script>".into()),
        "A &<>\"'ignored"
    );
}

#[test]
fn parser_accepts_data_family_and_no_range_but_rejects_decode_and_required_fields() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
    let no_range =
        InstrumentDateRangeRequest::new(instrument, PositiveU32::new(5).unwrap()).unwrap();
    let fixture = r#"jQuery_news({"data":{"cmsArticleWebOld":[{
      "code":"old","title":"标题","content":null,"date":"2026-06-01",
      "mediaName":"东方财富网","url":"https://data.eastmoney.com/a"
    }]}})"#
        .as_bytes();
    let batch = parse_news(fixture, &no_range).unwrap();
    assert_eq!(batch.records().len(), 1);
    assert!(batch.records()[0].summary.is_none());

    assert!(parse_news(&[0xff], &no_range).is_err());
    assert!(parse_news(b"jQuery_news(not-json)", &no_range).is_err());
    for row in [
        r#"{"title":"x","date":"2026-07-23","mediaName":"x","url":"https://stock.eastmoney.com/a"}"#,
        r#"{"code":"1","date":"2026-07-23","mediaName":"x","url":"https://stock.eastmoney.com/a"}"#,
        r#"{"code":"1","title":"x","date":"2026-07-23","url":"https://stock.eastmoney.com/a"}"#,
        r#"{"code":"1","title":"x","date":"2026-07-23","mediaName":"x"}"#,
    ] {
        let fixture = format!(r#"jQuery_news({{"result":{{"cmsArticleWebOld":[{row}]}}}})"#);
        assert!(parse_news(fixture.as_bytes(), &no_range).is_err(), "{row}");
    }
}
