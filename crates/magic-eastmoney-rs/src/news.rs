use crate::mapping::validate_minute_timestamp;
use crate::{BatchContext, EastmoneyClient, EastmoneyError};
use magic_market_core::{
    DataBatch, HttpsUrl, InstrumentDateRangeRequest, NewsItem, NewsProvider, NonEmptyText,
    PositiveU32,
};
use std::collections::HashSet;

#[cfg(test)]
use crate::mapping::{non_empty, optional_string, required_string, validate_date_or_datetime};
#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
const CALLBACK: &str = "jQuery_news";
const GLOBAL_NEWS_URL: &str = "https://roll.eastmoney.com/finance.html";
const MAX_GLOBAL_NEWS_LIMIT: u32 = 20;
const ARTICLE_PREFIX: &str = "/a/";
const ARTICLE_SUFFIX: &str = ".html";
const CATEGORY_MARKUP: &str = r#"[<a href="finance.html">财经</a>]"#;

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

    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        if limit.get() > MAX_GLOBAL_NEWS_LIMIT {
            return Err(EastmoneyError::InvalidRequest(format!(
                "Eastmoney global-news limit must be at most {MAX_GLOBAL_NEWS_LIMIT}"
            )));
        }
        let body = self.get_html(
            GLOBAL_NEWS_URL,
            &[
                ("Accept", "text/html"),
                ("Referer", "https://www.eastmoney.com/"),
            ],
        )?;
        parse_global_news(&body, limit.get() as usize)
    }
}

#[derive(Debug)]
struct ParsedGlobalNews {
    item_id: String,
    title: String,
    canonical_url: String,
    published_at: String,
}

fn parse_global_news(bytes: &[u8], limit: usize) -> Result<DataBatch<NewsItem>, EastmoneyError> {
    let html =
        std::str::from_utf8(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    let rows = parse_complete_global_news_page(html)?;
    if rows.len() < limit {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney rolling page has {} rows, fewer than requested {limit}",
            rows.len()
        )));
    }
    let source_at = rows
        .first()
        .map(|row| row.published_at.as_str())
        .ok_or_else(|| {
            EastmoneyError::Protocol("Eastmoney rolling page contains no finance rows".into())
        })?;
    let context = BatchContext::new("global-news", Some(source_at))?;
    let records = rows
        .into_iter()
        .take(limit)
        .map(|row| {
            Ok(NewsItem {
                item_id: NonEmptyText::new(row.item_id)?,
                title: NonEmptyText::new(row.title)?,
                summary: None,
                content: None,
                publisher: NonEmptyText::new("东方财富网")?,
                canonical_url: HttpsUrl::new(row.canonical_url)?,
                published_at: NonEmptyText::new(row.published_at.clone())?,
                instruments: Vec::new(),
                topics: vec![NonEmptyText::new("财经")?],
                language: NonEmptyText::new("zh-CN")?,
                evidence: context.evidence_at(Some(&row.published_at))?,
            })
        })
        .collect::<Result<Vec<_>, EastmoneyError>>()?;
    context.finish(records)
}

fn parse_complete_global_news_page(html: &str) -> Result<Vec<ParsedGlobalNews>, EastmoneyError> {
    const ART_LIST: &str = r#"<div id="artList" class="contain">"#;
    if html.match_indices(ART_LIST).count() != 1 {
        return Err(EastmoneyError::Protocol(
            "Eastmoney rolling page must contain exactly one #artList container".into(),
        ));
    }
    let after_container = html
        .split_once(ART_LIST)
        .map(|(_, after)| after)
        .ok_or_else(|| EastmoneyError::Protocol("missing Eastmoney #artList container".into()))?;
    let before_pager = after_container
        .split_once(r#"<div class="PageBox">"#)
        .map(|(before, _)| before)
        .ok_or_else(|| {
            EastmoneyError::Protocol("Eastmoney rolling page is missing its pager boundary".into())
        })?;
    let mut list_groups = before_pager
        .trim()
        .strip_suffix("</div>")
        .ok_or_else(|| {
            EastmoneyError::Protocol("Eastmoney #artList container is not closed".into())
        })?
        .trim();
    let mut rows = Vec::new();
    let mut item_ids = HashSet::new();
    let mut urls = HashSet::new();
    let mut previous_time: Option<String> = None;
    while !list_groups.is_empty() {
        let after_opening = list_groups.strip_prefix("<ul>").ok_or_else(|| {
            EastmoneyError::Protocol(
                "Eastmoney #artList contains content outside complete list groups".into(),
            )
        })?;
        let (mut list, remaining_groups) = after_opening.split_once("</ul>").ok_or_else(|| {
            EastmoneyError::Protocol("Eastmoney #artList has an incomplete list group".into())
        })?;
        let first_row = rows.len();
        loop {
            list = list.trim_start();
            if list.is_empty() {
                break;
            }
            let row = list.strip_prefix("<li>").ok_or_else(|| {
                EastmoneyError::Protocol("Eastmoney #artList contains content outside a row".into())
            })?;
            let (row, remainder) = row.split_once("</li>").ok_or_else(|| {
                EastmoneyError::Protocol("Eastmoney #artList contains an incomplete row".into())
            })?;
            let parsed = parse_global_news_row(row)?;
            if previous_time
                .as_deref()
                .is_some_and(|previous| parsed.published_at.as_str() > previous)
            {
                return Err(EastmoneyError::Protocol(
                    "Eastmoney rolling news is not newest-first".into(),
                ));
            }
            previous_time = Some(parsed.published_at.clone());
            if !item_ids.insert(parsed.item_id.clone()) {
                return Err(EastmoneyError::Protocol(format!(
                    "duplicate Eastmoney news item ID {}",
                    parsed.item_id
                )));
            }
            if !urls.insert(parsed.canonical_url.clone()) {
                return Err(EastmoneyError::Protocol(format!(
                    "duplicate Eastmoney news URL {}",
                    parsed.canonical_url
                )));
            }
            rows.push(parsed);
            list = remainder;
        }
        if rows.len() == first_row {
            return Err(EastmoneyError::Protocol(
                "Eastmoney #artList contains an empty list group".into(),
            ));
        }
        list_groups = remaining_groups.trim();
    }
    if rows.is_empty() {
        return Err(EastmoneyError::Protocol(
            "Eastmoney #artList contains no finance rows".into(),
        ));
    }
    Ok(rows)
}

fn parse_global_news_row(row: &str) -> Result<ParsedGlobalNews, EastmoneyError> {
    let row = row.trim();
    let after_span = row.strip_prefix("<span>").ok_or_else(|| {
        EastmoneyError::Protocol("Eastmoney news row must start with a timestamp span".into())
    })?;
    let (published_markup, remainder) = after_span.split_once("</span>").ok_or_else(|| {
        EastmoneyError::Protocol("Eastmoney news row has an incomplete timestamp span".into())
    })?;
    if published_markup.contains(['<', '>']) {
        return Err(EastmoneyError::Protocol(
            "Eastmoney news timestamp contains nested markup".into(),
        ));
    }
    let published_at = collapse_whitespace(published_markup);
    validate_minute_timestamp(&published_at, "Eastmoney news published_at")?;

    let article = remainder
        .trim_start()
        .strip_prefix(CATEGORY_MARKUP)
        .ok_or_else(|| {
            EastmoneyError::Protocol("Eastmoney rolling row is not in the 财经 category".into())
        })?
        .trim_start();
    let opening_end = article.find('>').ok_or_else(|| {
        EastmoneyError::Protocol("Eastmoney news article link has no closing bracket".into())
    })?;
    let opening = &article[..=opening_end];
    if !opening.starts_with("<a ") {
        return Err(EastmoneyError::Protocol(
            "Eastmoney news row has no article link".into(),
        ));
    }
    let href = exact_attribute(opening, "href")?;
    let title_attribute = normalize_html_text(&exact_attribute(opening, "title")?);
    let after_opening = &article[opening_end + 1..];
    let (visible_markup, after_article) = after_opening.split_once("</a>").ok_or_else(|| {
        EastmoneyError::Protocol("Eastmoney news article link is incomplete".into())
    })?;
    if !after_article.trim().is_empty() {
        return Err(EastmoneyError::Protocol(
            "Eastmoney news row contains content after its article link".into(),
        ));
    }
    let visible_title = normalize_html_text(visible_markup);
    if title_attribute != visible_title {
        return Err(EastmoneyError::Protocol(
            "Eastmoney news title attribute and visible title disagree".into(),
        ));
    }
    let (item_id, canonical_url) = normalize_global_article_url(&href)?;
    Ok(ParsedGlobalNews {
        item_id,
        title: visible_title,
        canonical_url,
        published_at,
    })
}

fn exact_attribute(opening: &str, name: &str) -> Result<String, EastmoneyError> {
    let needle = format!(r#" {name}=""#);
    if opening.match_indices(&needle).count() != 1 {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney news link must contain exactly one {name} attribute"
        )));
    }
    let value = opening
        .split_once(&needle)
        .map(|(_, after)| after)
        .and_then(|after| after.split_once('"').map(|(value, _)| value))
        .ok_or_else(|| {
            EastmoneyError::Protocol(format!(
                "Eastmoney news link has an invalid {name} attribute"
            ))
        })?;
    if value.is_empty() {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney news link has an empty {name} attribute"
        )));
    }
    Ok(value.to_owned())
}

fn normalize_global_article_url(url: &str) -> Result<(String, String), EastmoneyError> {
    let remainder = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| {
            EastmoneyError::Protocol("Eastmoney news article URL is not HTTP(S)".into())
        })?;
    if remainder.contains(['?', '#', '@']) {
        return Err(EastmoneyError::Protocol(
            "Eastmoney news article URL contains forbidden authority or suffix data".into(),
        ));
    }
    let (host, path) = remainder
        .split_once('/')
        .ok_or_else(|| EastmoneyError::Protocol("Eastmoney news article URL has no path".into()))?;
    if host != "finance.eastmoney.com" {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney news article host {host:?} is not finance.eastmoney.com"
        )));
    }
    let path = format!("/{path}");
    let item_id = path
        .strip_prefix(ARTICLE_PREFIX)
        .and_then(|value| value.strip_suffix(ARTICLE_SUFFIX))
        .ok_or_else(|| {
            EastmoneyError::Protocol(
                "Eastmoney news article path must be /a/<numeric-id>.html".into(),
            )
        })?;
    if item_id.is_empty() || !item_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EastmoneyError::Protocol(
            "Eastmoney news article ID must contain only digits".into(),
        ));
    }
    Ok((
        item_id.to_owned(),
        format!("https://finance.eastmoney.com{path}"),
    ))
}

fn normalize_html_text(value: &str) -> String {
    let mut without_tags = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => without_tags.push(character),
            _ => {}
        }
    }
    collapse_whitespace(
        &without_tags
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'"),
    )
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
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
    normalize_html_text(&value)
}

#[cfg(test)]
mod tests {
    use super::{normalize_global_article_url, parse_global_news, parse_news};
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
        assert_eq!(item.published_at.as_str(), "2026-07-25 08:40");
        assert!(item.instruments.is_empty());
        assert_eq!(item.topics[0].as_str(), "财经");
        assert_eq!(item.language.as_str(), "zh-CN");
        assert_eq!(batch.provenance().source_at(), Some("2026-07-25 08:40"));
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
            "https://stock.eastmoney.com/a/202607253821086055.html",
            "https://finance.eastmoney.com/b/202607253821086055.html",
            "https://finance.eastmoney.com/a/not-digits.html",
            "https://finance.eastmoney.com/a/202607253821086055.html?x=1",
            "https://finance.eastmoney.com.example/a/202607253821086055.html",
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
}
