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
#[path = "../tests/internal/news_tests.rs"]
mod tests;
