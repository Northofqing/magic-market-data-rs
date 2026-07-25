use crate::{validate_instruments, DocumentResponse, SinaClient, SinaError, MAX_RESPONSE_BYTES};
use encoding_rs::GB18030;
use magic_market_core::{
    AssetClass, ContentCapabilities, DataBatch, Exchange, HttpsUrl, InstrumentDateRangeRequest,
    IsoDate, NewsItem, NewsProvider, NonEmptyText, PositiveU32, Provenance, ProviderId,
    SourceEvidence,
};
use scraper::{Html, Selector};
use std::collections::HashMap;
use url::Url;

const NEWS_ENDPOINT: &str = "https://vip.stock.finance.sina.com.cn/corp/view/vCB_AllNewsStock.php";
const MAX_NEWS_LIMIT: u32 = 200;
const MAX_NEWS_PAGES: u32 = 5;
const MAX_PAGE_ROWS: usize = 50;
const MIN_SOURCE_DATE: &str = "2000-01-01";
const SOURCE_NAME: &str = "sina-company-news";
const PUBLISHER: &str = "新浪财经";

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawNews {
    title: String,
    canonical_url: String,
    published_at: String,
    published_date: String,
    published_unix: i64,
    observed_at: String,
}

#[derive(Debug)]
struct ParsedPage {
    records: Vec<RawNews>,
    has_next: bool,
}

impl SinaClient {
    /// Reports only content capabilities proved by deterministic tests and a
    /// bounded live probe.
    pub const fn content_capabilities() -> ContentCapabilities {
        ContentCapabilities {
            instrument_news: true,
            global_news: false,
            announcements: false,
            investor_questions: false,
        }
    }
}

impl NewsProvider for SinaClient {
    type Error = SinaError;

    fn instrument_news(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        validate_news_request(request)?;
        let symbol = validate_instruments(std::slice::from_ref(request.instrument()))?
            .into_iter()
            .next()
            .ok_or_else(|| SinaError::Protocol("validated news symbol is missing".into()))?;
        let limit = request.limit().get() as usize;
        let mut unique = Vec::<RawNews>::new();
        let mut identities = HashMap::<String, usize>::new();
        let mut selected = Vec::<usize>::new();
        let mut previous_oldest = None::<i64>;
        let mut newest_source_at = None::<String>;
        let mut final_observed_at = None::<String>;
        let mut pages_read = 0_u32;

        for page_number in 1..=MAX_NEWS_PAGES {
            let response = self
                .transport
                .get_document(&news_url(&symbol, page_number))?;
            let parsed = parse_page(&response, &symbol, page_number)?;
            pages_read = page_number;
            final_observed_at = Some(format_observed(response.observed_unix_seconds())?);
            if newest_source_at.is_none() {
                newest_source_at = parsed
                    .records
                    .first()
                    .map(|record| record.published_at.clone());
            }

            if let (Some(previous), Some(current)) = (
                previous_oldest,
                parsed.records.first().map(|record| record.published_unix),
            ) {
                if current > previous {
                    return Err(SinaError::Protocol(
                        "news ordering increased across pages".into(),
                    ));
                }
            }
            previous_oldest = parsed.records.last().map(|record| record.published_unix);
            let page_oldest_date = parsed
                .records
                .last()
                .map(|record| record.published_date.clone());
            let has_next = parsed.has_next;

            for record in parsed.records {
                if let Some(index) = identities.get(&record.canonical_url).copied() {
                    if !same_source_record(&unique[index], &record) {
                        return Err(SinaError::Protocol(format!(
                            "conflicting duplicate news canonical URL {}",
                            record.canonical_url
                        )));
                    }
                    continue;
                }
                let index = unique.len();
                identities.insert(record.canonical_url.clone(), index);
                if in_requested_range(request, &record.published_date) {
                    selected.push(index);
                }
                unique.push(record);
            }

            if selected.len() >= limit {
                break;
            }
            let crossed_start = request.start().is_some_and(|start| {
                page_oldest_date
                    .as_deref()
                    .is_some_and(|source_date| source_date < start.as_str())
            });
            if crossed_start || !has_next {
                break;
            }
            if page_number == MAX_NEWS_PAGES {
                return Err(SinaError::Protocol(format!(
                    "news pagination exceeds the {MAX_NEWS_PAGES}-page bound"
                )));
            }
        }

        selected.truncate(limit);
        let observed_at = final_observed_at
            .ok_or_else(|| SinaError::Protocol("news observation time is missing".into()))?;
        let source_at = newest_source_at
            .ok_or_else(|| SinaError::Protocol("news source time is missing".into()))?;
        let batch_id = format!("{SOURCE_NAME}:{symbol}:{observed_at}:pages-{pages_read}");
        let mut records = Vec::with_capacity(selected.len());
        for index in selected {
            records.push(normalize_news(
                &unique[index],
                request.instrument(),
                &batch_id,
            )?);
        }
        let provenance = Provenance::new(SOURCE_NAME, &observed_at)?
            .with_source_at(source_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::strict(records, provenance))
    }

    fn global_news(&self, _limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(SinaError::Unsupported(
            "Sina global news has no independently verified provider contract".into(),
        ))
    }
}

fn validate_news_request(request: &InstrumentDateRangeRequest) -> Result<(), SinaError> {
    if request.limit().get() > MAX_NEWS_LIMIT {
        return Err(SinaError::InvalidRequest(format!(
            "instrument-news limit must be at most {MAX_NEWS_LIMIT}"
        )));
    }
    if request.instrument().asset_class() != AssetClass::Equity {
        return Err(SinaError::Unsupported(
            "Sina company news supports only A-share equities".into(),
        ));
    }
    if request.instrument().exchange() == Exchange::Beijing {
        return Err(SinaError::Unsupported(
            "Sina Beijing company-news page is not live-probe admitted".into(),
        ));
    }
    Ok(())
}

fn news_url(symbol: &str, page: u32) -> String {
    format!("{NEWS_ENDPOINT}?symbol={symbol}&Page={page}")
}

fn parse_page(
    response: &DocumentResponse,
    symbol: &str,
    page_number: u32,
) -> Result<ParsedPage, SinaError> {
    if response.status() != 200 {
        return Err(SinaError::Transport(format!(
            "instrument-news HTTP status {}",
            response.status()
        )));
    }
    validate_html_mime(response.content_type())?;
    if response.body().is_empty() {
        return Err(SinaError::Protocol(
            "empty instrument-news response body".into(),
        ));
    }
    if response.body().len() > MAX_RESPONSE_BYTES {
        return Err(SinaError::Protocol(format!(
            "instrument-news response exceeds {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    let (decoded, _, had_errors) = GB18030.decode(response.body());
    if had_errors {
        return Err(SinaError::Decode(
            "instrument-news response contains invalid GB18030 bytes".into(),
        ));
    }
    let html = decoded.as_ref();
    validate_page_identity(html, symbol, page_number)?;
    let document = Html::parse_document(html);
    let list_selector = Selector::parse("div.datelist > ul")
        .map_err(|error| SinaError::Protocol(error.to_string()))?;
    let anchor_selector =
        Selector::parse("a").map_err(|error| SinaError::Protocol(error.to_string()))?;
    let mut lists = document.select(&list_selector);
    let list = lists.next().ok_or_else(|| {
        SinaError::Protocol("empty instrument-news datelist: list is missing".into())
    })?;
    if lists.next().is_some() {
        return Err(SinaError::Protocol(
            "instrument-news page has multiple datelists".into(),
        ));
    }
    let observed_unix = i64::try_from(response.observed_unix_seconds())
        .map_err(|_| SinaError::Protocol("news observation time is out of range".into()))?;
    let observed_at = format_observed(response.observed_unix_seconds())?;
    let mut records = Vec::new();
    for anchor in list.select(&anchor_selector) {
        let timestamp = anchor
            .prev_sibling()
            .and_then(|node| node.value().as_text())
            .ok_or_else(|| {
                SinaError::Protocol("instrument-news row is missing provider published time".into())
            })?;
        let (published_at, published_date, published_unix) = parse_published(timestamp.as_ref())?;
        if published_unix > observed_unix {
            return Err(SinaError::Protocol(format!(
                "future instrument-news provider time {published_at}"
            )));
        }
        let source_url = anchor.value().attr("href").ok_or_else(|| {
            SinaError::Protocol("instrument-news row is missing canonical URL".into())
        })?;
        let canonical_url = normalize_sina_url(source_url)?;
        let title = anchor.text().collect::<String>();
        NonEmptyText::new(title.clone())?;
        records.push(RawNews {
            title,
            canonical_url,
            published_at,
            published_date,
            published_unix,
            observed_at: observed_at.clone(),
        });
        if records.len() > MAX_PAGE_ROWS {
            return Err(SinaError::Protocol(format!(
                "instrument-news page exceeds {MAX_PAGE_ROWS} rows"
            )));
        }
    }
    if records.is_empty() {
        return Err(SinaError::Protocol("empty instrument-news datelist".into()));
    }
    if records
        .windows(2)
        .any(|window| window[1].published_unix > window[0].published_unix)
    {
        return Err(SinaError::Protocol(
            "instrument-news page is not newest-first".into(),
        ));
    }
    let next_marker = format!(
        "vCB_AllNewsStock.php?symbol={symbol}&amp;Page={}",
        page_number + 1
    );
    let raw_next_marker = format!(
        "vCB_AllNewsStock.php?symbol={symbol}&Page={}",
        page_number + 1
    );
    Ok(ParsedPage {
        records,
        has_next: html.contains(&next_marker) || html.contains(&raw_next_marker),
    })
}

fn validate_html_mime(content_type: &str) -> Result<(), SinaError> {
    let mut parts = content_type.split(';').map(str::trim);
    if !parts
        .next()
        .is_some_and(|value| value.eq_ignore_ascii_case("text/html"))
    {
        return Err(SinaError::Protocol(format!(
            "instrument-news MIME must be text/html with GBK charset, got {content_type:?}"
        )));
    }
    let charset = parts.find_map(|part| {
        part.split_once('=')
            .filter(|(key, _)| key.trim().eq_ignore_ascii_case("charset"))
            .map(|(_, value)| value.trim().trim_matches('"'))
    });
    if !charset.is_some_and(|value| {
        value.eq_ignore_ascii_case("gbk")
            || value.eq_ignore_ascii_case("gb2312")
            || value.eq_ignore_ascii_case("gb18030")
    }) {
        return Err(SinaError::Protocol(format!(
            "instrument-news MIME has an unsupported charset: {content_type:?}"
        )));
    }
    Ok(())
}

fn validate_page_identity(html: &str, symbol: &str, page_number: u32) -> Result<(), SinaError> {
    let marker_name = "var page_symbol";
    let expected = format!("var page_symbol = \"{symbol}\";");
    if html.matches(marker_name).count() != 1 || html.matches(&expected).count() != 1 {
        return Err(SinaError::Protocol(format!(
            "instrument-news identity does not match {symbol}"
        )));
    }
    let page_marker = format!("第{page_number}页");
    if !html.contains(&page_marker) {
        return Err(SinaError::Protocol(format!(
            "instrument-news page marker does not match Page={page_number}"
        )));
    }
    Ok(())
}

fn parse_published(value: &str) -> Result<(String, String, i64), SinaError> {
    let mut fields = value.split_whitespace();
    let date = fields
        .next()
        .ok_or_else(|| SinaError::Protocol("instrument-news published date is missing".into()))?;
    let time = fields
        .next()
        .ok_or_else(|| SinaError::Protocol("instrument-news published time is missing".into()))?;
    if fields.next().is_some() {
        return Err(SinaError::Protocol(
            "instrument-news published time has trailing input".into(),
        ));
    }
    IsoDate::new(date.to_owned())?;
    if date < MIN_SOURCE_DATE {
        return Err(SinaError::Protocol(format!(
            "instrument-news published date precedes {MIN_SOURCE_DATE}"
        )));
    }
    let time_bytes = time.as_bytes();
    if time_bytes.len() != 5
        || time_bytes[2] != b':'
        || !time_bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 2 || byte.is_ascii_digit())
    {
        return Err(SinaError::Protocol(format!(
            "instrument-news published time is invalid: {time:?}"
        )));
    }
    let hour = time[0..2]
        .parse::<u32>()
        .map_err(|_| SinaError::Protocol("published hour is invalid".into()))?;
    let minute = time[3..5]
        .parse::<u32>()
        .map_err(|_| SinaError::Protocol("published minute is invalid".into()))?;
    if hour >= 24 || minute >= 60 {
        return Err(SinaError::Protocol(format!(
            "instrument-news published time is invalid: {time:?}"
        )));
    }
    let published_unix = china_time_to_unix(date, hour, minute)?;
    Ok((
        format!("{date}T{time}:00+08:00"),
        date.to_owned(),
        published_unix,
    ))
}

fn china_time_to_unix(date: &str, hour: u32, minute: u32) -> Result<i64, SinaError> {
    let year = date[0..4]
        .parse::<i64>()
        .map_err(|_| SinaError::Protocol("published year is invalid".into()))?;
    let month = date[5..7]
        .parse::<i64>()
        .map_err(|_| SinaError::Protocol("published month is invalid".into()))?;
    let day = date[8..10]
        .parse::<i64>()
        .map_err(|_| SinaError::Protocol("published day is invalid".into()))?;
    let days = days_from_civil(year, month, day)?;
    days.checked_mul(86_400)
        .and_then(|seconds| seconds.checked_add(i64::from(hour) * 3_600))
        .and_then(|seconds| seconds.checked_add(i64::from(minute) * 60))
        .and_then(|seconds| seconds.checked_sub(8 * 3_600))
        .ok_or_else(|| SinaError::Protocol("instrument-news published time overflow".into()))
}

fn days_from_civil(year: i64, month: i64, day: i64) -> Result<i64, SinaError> {
    let adjusted_year = year - i64::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era.checked_mul(146_097)
        .and_then(|value| value.checked_add(day_of_era))
        .and_then(|value| value.checked_sub(719_468))
        .ok_or_else(|| SinaError::Protocol("instrument-news date is out of range".into()))
}

fn format_observed(seconds: u64) -> Result<String, SinaError> {
    if seconds == 0 {
        return Err(SinaError::Protocol(
            "instrument-news observation time is invalid".into(),
        ));
    }
    Ok(format!("{seconds}.000000000"))
}

fn normalize_sina_url(value: &str) -> Result<String, SinaError> {
    let mut parsed = Url::parse(value).map_err(|error| {
        SinaError::Protocol(format!("instrument-news canonical URL is invalid: {error}"))
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(SinaError::Protocol(format!(
            "instrument-news canonical URL has unsupported scheme: {value}"
        )));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(SinaError::Protocol(
            "instrument-news canonical URL must not contain credentials".into(),
        ));
    }
    let authority = value
        .split_once("://")
        .map(|(_, remainder)| remainder)
        .unwrap_or_default()
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.contains(':') {
        return Err(SinaError::Protocol(
            "instrument-news canonical URL must not contain an explicit port".into(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| SinaError::Protocol("instrument-news canonical URL host is missing".into()))?
        .to_ascii_lowercase();
    let is_sina = host == "sina.com.cn"
        || host.ends_with(".sina.com.cn")
        || host == "sina.cn"
        || host.ends_with(".sina.cn");
    if !is_sina {
        return Err(SinaError::Protocol(format!(
            "instrument-news canonical URL is not a Sina host: {value}"
        )));
    }
    if parsed.scheme() == "http" {
        parsed.set_scheme("https").map_err(|()| {
            SinaError::Protocol("instrument-news canonical URL scheme upgrade failed".into())
        })?;
    }
    let normalized = parsed.to_string();
    HttpsUrl::new(normalized.clone())?;
    Ok(normalized)
}

fn in_requested_range(request: &InstrumentDateRangeRequest, source_date: &str) -> bool {
    match (request.start(), request.end()) {
        (Some(start), Some(end)) => source_date >= start.as_str() && source_date <= end.as_str(),
        (None, None) => true,
        _ => false,
    }
}

fn same_source_record(left: &RawNews, right: &RawNews) -> bool {
    left.title == right.title
        && left.canonical_url == right.canonical_url
        && left.published_at == right.published_at
        && left.published_date == right.published_date
        && left.published_unix == right.published_unix
}

fn normalize_news(
    raw: &RawNews,
    instrument: &magic_market_core::InstrumentId,
    batch_id: &str,
) -> Result<NewsItem, SinaError> {
    let evidence = SourceEvidence::new(
        ProviderId::Sina,
        raw.observed_at.clone(),
        batch_id.to_owned(),
    )?
    .with_source_at(raw.published_at.clone())?;
    Ok(NewsItem {
        item_id: NonEmptyText::new(raw.canonical_url.clone())?,
        title: NonEmptyText::new(raw.title.clone())?,
        summary: None,
        content: None,
        publisher: NonEmptyText::new(PUBLISHER)?,
        canonical_url: HttpsUrl::new(raw.canonical_url.clone())?,
        published_at: NonEmptyText::new(raw.published_at.clone())?,
        instruments: vec![instrument.clone()],
        topics: Vec::new(),
        language: NonEmptyText::new("zh-CN")?,
        evidence,
    })
}
