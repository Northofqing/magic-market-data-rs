use crate::{EastmoneyClient, EastmoneyError};
use magic_market_core::{InstrumentDateRangeRequest, NewsItem, NewsProvider, PositiveU32};

#[cfg(test)]
use crate::mapping::{non_empty, optional_string, required_string, validate_date_or_datetime};
#[cfg(test)]
use crate::BatchContext;
#[cfg(test)]
use magic_market_core::{HttpsUrl, NonEmptyText};
#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
const CALLBACK: &str = "jQuery_news";

impl NewsProvider for EastmoneyClient {
    type Error = EastmoneyError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<magic_market_core::DataBatch<NewsItem>, Self::Error> {
        Err(EastmoneyError::Unsupported(
            "Eastmoney keyword-news search does not return a structured source instrument identity"
                .into(),
        ))
    }

    fn global_news(
        &self,
        _limit: PositiveU32,
    ) -> Result<magic_market_core::DataBatch<NewsItem>, Self::Error> {
        Err(EastmoneyError::Unsupported(
            "Eastmoney global-news endpoint shape has not been verified".into(),
        ))
    }
}

#[cfg(test)]
fn parse_news(
    bytes: &[u8],
    request: &InstrumentDateRangeRequest,
) -> Result<magic_market_core::DataBatch<NewsItem>, EastmoneyError> {
    let text =
        std::str::from_utf8(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    let payload = unwrap_jsonp(text)?;
    let root: Value =
        serde_json::from_str(payload).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    let rows = root
        .pointer("/result/cmsArticleWebOld")
        .or_else(|| root.pointer("/data/cmsArticleWebOld"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            EastmoneyError::Protocol(
                "news response is missing the cmsArticleWebOld result family".into(),
            )
        })?;
    let mut filtered = Vec::new();
    for row in rows {
        if in_requested_range(row, request)? {
            filtered.push(row);
        }
    }
    let source_at = filtered
        .iter()
        .filter_map(|row| optional_string(row.get("date")).ok().flatten())
        .max();
    let context = BatchContext::new("instrument-news", source_at.as_deref())?;
    let records = filtered
        .into_iter()
        .map(|row| map_news(row, &context))
        .collect::<Result<Vec<_>, _>>()?;
    context.finish(records)
}

#[cfg(test)]
fn unwrap_jsonp(text: &str) -> Result<&str, EastmoneyError> {
    let trimmed = text.trim();
    let prefix = format!("{CALLBACK}(");
    let payload = trimmed
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(')'))
        .or_else(|| {
            trimmed
                .strip_prefix(&prefix)
                .and_then(|value| value.strip_suffix(");"))
        })
        .ok_or_else(|| EastmoneyError::Protocol("unexpected news JSONP callback".into()))?;
    if payload.trim().is_empty() {
        return Err(EastmoneyError::Protocol("empty news JSONP payload".into()));
    }
    Ok(payload)
}

#[cfg(test)]
fn in_requested_range(
    row: &Value,
    request: &InstrumentDateRangeRequest,
) -> Result<bool, EastmoneyError> {
    let published = required_string(row, "date")?;
    validate_date_or_datetime(&published, "news date")?;
    let Some(start) = request.start() else {
        return Ok(true);
    };
    let end = request
        .end()
        .ok_or_else(|| EastmoneyError::InvalidRequest("news date range has no end".into()))?;
    let date = published
        .get(..10)
        .ok_or_else(|| EastmoneyError::Protocol("news date has no YYYY-MM-DD prefix".into()))?;
    Ok(date >= start.as_str() && date <= end.as_str())
}

#[cfg(test)]
fn map_news(row: &Value, context: &BatchContext) -> Result<NewsItem, EastmoneyError> {
    let published_at = required_string(row, "date")?;
    validate_date_or_datetime(&published_at, "news date")?;
    let content = optional_string(row.get("content"))?.map(strip_html);
    Ok(NewsItem {
        item_id: NonEmptyText::new(required_string(row, "code")?)?,
        title: NonEmptyText::new(strip_html(required_string(row, "title")?))?,
        summary: non_empty(content)?,
        content: None,
        publisher: NonEmptyText::new(required_string(row, "mediaName")?)?,
        canonical_url: normalize_article_url(&required_string(row, "url")?)?,
        published_at: NonEmptyText::new(published_at.clone())?,
        instruments: Vec::new(),
        topics: Vec::new(),
        language: NonEmptyText::new("zh-CN")?,
        evidence: context.evidence_at(Some(&published_at))?,
    })
}

#[cfg(test)]
fn normalize_article_url(url: &str) -> Result<HttpsUrl, EastmoneyError> {
    let (scheme, remainder) = if let Some(remainder) = url.strip_prefix("https://") {
        ("https", remainder)
    } else if let Some(remainder) = url.strip_prefix("http://") {
        ("http", remainder)
    } else {
        return Err(EastmoneyError::Protocol(
            "news canonical URL is not HTTP(S)".into(),
        ));
    };
    let host = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    const ALLOWED: &[&str] = &[
        "caifuhao.eastmoney.com",
        "data.eastmoney.com",
        "finance.eastmoney.com",
        "stock.eastmoney.com",
    ];
    if !ALLOWED.contains(&host) {
        return Err(EastmoneyError::Protocol(format!(
            "news URL host {host} is not an approved Eastmoney host"
        )));
    }
    let normalized = if scheme == "http" {
        format!("https://{remainder}")
    } else {
        url.to_owned()
    };
    Ok(HttpsUrl::new(normalized)?)
}

#[cfg(test)]
fn strip_html(value: String) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    output
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::parse_news;
    use magic_market_core::{
        AssetClass, Exchange, InstrumentDateRangeRequest, InstrumentId, IsoDate, NewsProvider,
        PositiveU32,
    };

    fn request() -> InstrumentDateRangeRequest {
        let instrument =
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
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
}
