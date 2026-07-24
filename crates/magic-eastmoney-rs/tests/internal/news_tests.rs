use super::parse_news;
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
