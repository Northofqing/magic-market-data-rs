use crate::{validate_instruments, DocumentResponse, SinaClient, SinaError, MAX_RESPONSE_BYTES};
use encoding_rs::GB18030;
use magic_market_core::{
    AssetClass, ContentCapabilities, DataBatch, Exchange, HttpsUrl, InstrumentDateRangeRequest,
    IsoDate, NewsItem, NewsProvider, NonEmptyText, PositiveU32, Provenance, ProviderId,
    SourceEvidence,
};
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

#[derive(Debug)]
struct HtmlTag {
    start: usize,
    end: usize,
    name: String,
    closing: bool,
    self_closing: bool,
    attributes: HashMap<String, String>,
}

#[derive(Debug)]
struct PageAnchor {
    published_text: String,
    href: String,
    title: String,
}

impl SinaClient {
    /// Reports only content capabilities proved by deterministic tests and a
    /// bounded live probe.
    pub const fn content_capabilities() -> ContentCapabilities {
        ContentCapabilities {
            instrument_news: true,
            global_news: false,
            announcements: false,
            market_announcements: false,
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
        let mut previous_newest = None::<i64>;
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
            let page_newest = parsed.records.first().map(|record| record.published_unix);
            let page_oldest = parsed.records.last().map(|record| record.published_unix);
            if previous_newest
                .zip(page_newest)
                .is_some_and(|(previous, current)| current > previous)
                || previous_oldest
                    .zip(page_oldest)
                    .is_some_and(|(previous, current)| current > previous)
            {
                return Err(SinaError::Protocol(
                    "news page window increased across pages".into(),
                ));
            }
            previous_newest = page_newest;
            previous_oldest = page_oldest;
            let page_newest_date = parsed
                .records
                .first()
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

            selected.sort_by(|left, right| {
                unique[*right]
                    .published_unix
                    .cmp(&unique[*left].published_unix)
            });
            let limit_is_proven = selected.len() >= limit
                && page_newest
                    .is_some_and(|newest| newest <= unique[selected[limit - 1]].published_unix);
            let page_is_before_start = request.start().is_some_and(|start| {
                page_newest_date
                    .as_deref()
                    .is_some_and(|source_date| source_date < start.as_str())
            });
            if limit_is_proven || page_is_before_start || !has_next {
                break;
            }
            if page_number == MAX_NEWS_PAGES {
                return Err(SinaError::Protocol(format!(
                    "news pagination exceeds the {MAX_NEWS_PAGES}-page bound"
                )));
            }
        }

        selected.sort_by(|left, right| {
            unique[*right]
                .published_unix
                .cmp(&unique[*left].published_unix)
        });
        selected.truncate(limit);
        let observed_at = final_observed_at
            .ok_or_else(|| SinaError::Protocol("news observation time is missing".into()))?;
        let batch_id = format!("{SOURCE_NAME}:{symbol}:{observed_at}:pages-{pages_read}");
        let mut records = Vec::with_capacity(selected.len());
        for index in selected {
            records.push(normalize_news(
                &unique[index],
                request.instrument(),
                &batch_id,
            )?);
        }
        let source_at = records
            .first()
            .and_then(|record| record.evidence.source_at())
            .or(newest_source_at.as_deref())
            .ok_or_else(|| SinaError::Protocol("news source time is missing".into()))?
            .to_owned();
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
    let anchors = parse_page_anchors(html)?;
    let observed_unix = i64::try_from(response.observed_unix_seconds())
        .map_err(|_| SinaError::Protocol("news observation time is out of range".into()))?;
    let observed_at = format_observed(response.observed_unix_seconds())?;
    let mut records = Vec::new();
    for anchor in anchors {
        let (published_at, published_date, published_unix) =
            parse_published(&anchor.published_text)?;
        if published_unix > observed_unix {
            return Err(SinaError::Protocol(format!(
                "future instrument-news provider time {published_at}"
            )));
        }
        let canonical_url = normalize_sina_url(&anchor.href)?;
        if anchor.title.trim().is_empty() {
            return Err(SinaError::Protocol(format!(
                "instrument-news source title is empty for {canonical_url}"
            )));
        }
        records.push(RawNews {
            title: anchor.title,
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

fn parse_page_anchors(html: &str) -> Result<Vec<PageAnchor>, SinaError> {
    let mut cursor = 0;
    let mut datelist = None::<&str>;
    while let Some(tag) = next_html_tag(html, cursor)? {
        cursor = tag.end;
        if !tag.closing && !tag.self_closing && matches!(tag.name.as_str(), "script" | "style") {
            cursor = raw_text_element_end(html, cursor, &tag.name)?;
            continue;
        }
        if tag.closing || tag.self_closing || tag.name != "div" {
            continue;
        }
        let is_datelist = tag
            .attributes
            .get("class")
            .is_some_and(|class| class.split_whitespace().any(|value| value == "datelist"));
        if !is_datelist {
            continue;
        }
        if datelist.is_some() {
            return Err(SinaError::Protocol(
                "instrument-news page has multiple datelists".into(),
            ));
        }
        let (content_end, close_end) = matching_close(html, &tag, "div")?;
        datelist = Some(&html[tag.end..content_end]);
        cursor = close_end;
    }

    let content = datelist.ok_or_else(|| {
        SinaError::Protocol("empty instrument-news datelist: list is missing".into())
    })?;
    let ul = direct_datelist_ul(content)?;
    parse_anchor_rows(ul)
}

fn raw_text_element_end(
    html: &str,
    content_start: usize,
    tag_name: &str,
) -> Result<usize, SinaError> {
    let closing = format!("</{tag_name}>");
    let remainder = html.get(content_start..).ok_or_else(|| {
        SinaError::Protocol("instrument-news raw-text boundary is invalid".into())
    })?;
    let lower = remainder.to_ascii_lowercase();
    let relative = lower.find(&closing).ok_or_else(|| {
        SinaError::Protocol(format!(
            "instrument-news <{tag_name}> raw-text element is not closed"
        ))
    })?;
    content_start
        .checked_add(relative)
        .and_then(|value| value.checked_add(closing.len()))
        .ok_or_else(|| SinaError::Protocol("instrument-news raw-text boundary overflow".into()))
}

fn direct_datelist_ul(content: &str) -> Result<&str, SinaError> {
    let open = next_html_tag(content, 0)?.ok_or_else(|| {
        SinaError::Protocol("empty instrument-news datelist: list is missing".into())
    })?;
    if !content[..open.start].trim().is_empty()
        || open.closing
        || open.self_closing
        || open.name != "ul"
    {
        return Err(SinaError::Protocol(
            "instrument-news datelist must contain one direct ul".into(),
        ));
    }
    let (content_end, close_end) = matching_close(content, &open, "ul")?;
    if !content[close_end..].trim().is_empty() {
        return Err(SinaError::Protocol(
            "instrument-news datelist contains trailing structure".into(),
        ));
    }
    Ok(&content[open.end..content_end])
}

fn parse_anchor_rows(content: &str) -> Result<Vec<PageAnchor>, SinaError> {
    let mut records = Vec::new();
    let mut cursor = 0;
    let mut text_start = 0;
    while let Some(tag) = next_html_tag(content, cursor)? {
        if tag.name == "a" && !tag.closing {
            if tag.self_closing {
                return Err(SinaError::Protocol(
                    "instrument-news anchor must not be self-closing".into(),
                ));
            }
            let published_text = decode_html_entities(&content[text_start..tag.start])?;
            let href = tag
                .attributes
                .get("href")
                .ok_or_else(|| {
                    SinaError::Protocol("instrument-news row is missing canonical URL".into())
                })
                .and_then(|value| decode_html_entities(value))?;
            let (title_end, close_end) = matching_close(content, &tag, "a")?;
            let title = visible_text(&content[tag.end..title_end])?;
            if title.trim().is_empty() {
                cursor = close_end;
                text_start = close_end;
                continue;
            }
            records.push(PageAnchor {
                published_text,
                href,
                title,
            });
            if records.len() > MAX_PAGE_ROWS {
                return Err(SinaError::Protocol(format!(
                    "instrument-news page exceeds {MAX_PAGE_ROWS} rows"
                )));
            }
            cursor = close_end;
            text_start = close_end;
            continue;
        }
        if !tag.closing
            && !tag.self_closing
            && !matches!(
                tag.name.as_str(),
                "br" | "hr" | "img" | "input" | "meta" | "link"
            )
        {
            if matches!(tag.name.as_str(), "script" | "style") {
                cursor = raw_text_element_end(content, tag.end, &tag.name)?;
                text_start = cursor;
                continue;
            }
            let (_, close_end) = matching_close(content, &tag, &tag.name)?;
            cursor = close_end;
            text_start = close_end;
            continue;
        }
        if tag.name == "a" && tag.closing {
            return Err(SinaError::Protocol(
                "instrument-news page has an unmatched anchor".into(),
            ));
        }
        cursor = tag.end;
        text_start = tag.end;
    }
    Ok(records)
}

fn visible_text(content: &str) -> Result<String, SinaError> {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(tag) = next_html_tag(content, cursor)? {
        output.push_str(&decode_html_entities(&content[cursor..tag.start])?);
        if tag.name == "a" {
            return Err(SinaError::Protocol(
                "instrument-news title contains a nested anchor".into(),
            ));
        }
        if !tag.closing && !tag.self_closing && matches!(tag.name.as_str(), "script" | "style") {
            cursor = raw_text_element_end(content, tag.end, &tag.name)?;
            continue;
        }
        cursor = tag.end;
    }
    output.push_str(&decode_html_entities(&content[cursor..])?);
    Ok(output)
}

fn matching_close(
    html: &str,
    open: &HtmlTag,
    expected_name: &str,
) -> Result<(usize, usize), SinaError> {
    let mut depth = 1_u32;
    let mut cursor = open.end;
    while let Some(tag) = next_html_tag(html, cursor)? {
        cursor = tag.end;
        if !tag.closing && !tag.self_closing && matches!(tag.name.as_str(), "script" | "style") {
            cursor = raw_text_element_end(html, cursor, &tag.name)?;
            continue;
        }
        if tag.name != expected_name || tag.self_closing {
            continue;
        }
        if tag.closing {
            depth = depth.checked_sub(1).ok_or_else(|| {
                SinaError::Protocol(format!(
                    "instrument-news page has unmatched </{expected_name}>"
                ))
            })?;
            if depth == 0 {
                return Ok((tag.start, tag.end));
            }
        } else {
            if expected_name == "a" {
                return Err(SinaError::Protocol(
                    "instrument-news title contains a nested anchor".into(),
                ));
            }
            depth = depth.checked_add(1).ok_or_else(|| {
                SinaError::Protocol("instrument-news HTML nesting overflow".into())
            })?;
        }
    }
    Err(SinaError::Protocol(format!(
        "instrument-news page is missing </{expected_name}>"
    )))
}

fn next_html_tag(html: &str, mut cursor: usize) -> Result<Option<HtmlTag>, SinaError> {
    loop {
        let Some(relative_start) = html[cursor..].find('<') else {
            return Ok(None);
        };
        let start = cursor + relative_start;
        if html[start..].starts_with("<!--") {
            let relative_end = html[start + 4..].find("-->").ok_or_else(|| {
                SinaError::Protocol("instrument-news HTML comment is not closed".into())
            })?;
            cursor = start + 4 + relative_end + 3;
            continue;
        }
        let end = html_tag_end(html, start)?;
        let raw = html[start + 1..end - 1].trim();
        if raw.starts_with('!') || raw.starts_with('?') {
            cursor = end;
            continue;
        }
        return parse_html_tag(raw, start, end).map(Some);
    }
}

fn html_tag_end(html: &str, start: usize) -> Result<usize, SinaError> {
    let mut quote = None::<char>;
    for (relative, character) in html[start + 1..].char_indices() {
        match (quote, character) {
            (Some(expected), current) if current == expected => quote = None,
            (None, '\'' | '"') => quote = Some(character),
            (None, '>') => return Ok(start + 1 + relative + 1),
            _ => {}
        }
    }
    Err(SinaError::Protocol(
        "instrument-news HTML tag is not closed".into(),
    ))
}

fn parse_html_tag(raw: &str, start: usize, end: usize) -> Result<HtmlTag, SinaError> {
    let closing = raw.starts_with('/');
    let body = raw.strip_prefix('/').unwrap_or(raw).trim_start();
    let name_end = body
        .find(|character: char| character.is_ascii_whitespace() || character == '/')
        .unwrap_or(body.len());
    let name = body[..name_end].to_ascii_lowercase();
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(SinaError::Protocol(
            "instrument-news HTML tag name is invalid".into(),
        ));
    }
    let self_closing = !closing && body.trim_end().ends_with('/');
    let attributes = if closing {
        if !body[name_end..].trim().is_empty() {
            return Err(SinaError::Protocol(format!(
                "instrument-news closing tag </{name}> has trailing input"
            )));
        }
        HashMap::new()
    } else {
        parse_html_attributes(&body[name_end..], &name)?
    };
    Ok(HtmlTag {
        start,
        end,
        name,
        closing,
        self_closing,
        attributes,
    })
}

fn parse_html_attributes(
    input: &str,
    tag_name: &str,
) -> Result<HashMap<String, String>, SinaError> {
    let bytes = input.as_bytes();
    let mut attributes = HashMap::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == bytes.len() || bytes[cursor] == b'/' {
            break;
        }
        let name_start = cursor;
        while cursor < bytes.len()
            && (bytes[cursor].is_ascii_alphanumeric()
                || matches!(bytes[cursor], b'-' | b'_' | b':' | b'.'))
        {
            cursor += 1;
        }
        if cursor == name_start {
            return Err(SinaError::Protocol(format!(
                "instrument-news <{tag_name}> attribute name is invalid"
            )));
        }
        let name = input[name_start..cursor].to_ascii_lowercase();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let value = if cursor < bytes.len() && bytes[cursor] == b'=' {
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor == bytes.len() {
                return Err(SinaError::Protocol(format!(
                    "instrument-news <{tag_name}> attribute {name} has no value"
                )));
            }
            let quote = bytes[cursor];
            if quote == b'\'' || quote == b'"' {
                cursor += 1;
                let value_start = cursor;
                while cursor < bytes.len() && bytes[cursor] != quote {
                    cursor += 1;
                }
                if cursor == bytes.len() {
                    return Err(SinaError::Protocol(format!(
                        "instrument-news <{tag_name}> attribute {name} is not closed"
                    )));
                }
                let value = input[value_start..cursor].to_owned();
                cursor += 1;
                value
            } else {
                let value_start = cursor;
                while cursor < bytes.len()
                    && !bytes[cursor].is_ascii_whitespace()
                    && bytes[cursor] != b'/'
                {
                    cursor += 1;
                }
                input[value_start..cursor].to_owned()
            }
        } else {
            String::new()
        };
        if attributes.insert(name.clone(), value).is_some() {
            return Err(SinaError::Protocol(format!(
                "instrument-news <{tag_name}> repeats attribute {name}"
            )));
        }
    }
    Ok(attributes)
}

fn decode_html_entities(value: &str) -> Result<String, SinaError> {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(relative_start) = value[cursor..].find('&') {
        let start = cursor + relative_start;
        output.push_str(&value[cursor..start]);
        let relative_end = value[start + 1..].find(';').ok_or_else(|| {
            SinaError::Protocol("instrument-news HTML entity is not closed".into())
        })?;
        let end = start + 1 + relative_end;
        let entity = &value[start + 1..end];
        let decoded = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" | "#39" => '\'',
            "nbsp" => '\u{00a0}',
            _ => decode_numeric_entity(entity)?,
        };
        output.push(decoded);
        cursor = end + 1;
    }
    output.push_str(&value[cursor..]);
    Ok(output)
}

fn decode_numeric_entity(entity: &str) -> Result<char, SinaError> {
    let value = if let Some(hex) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        u32::from_str_radix(hex, 16)
    } else if let Some(decimal) = entity.strip_prefix('#') {
        decimal.parse::<u32>()
    } else {
        return Err(SinaError::Protocol(format!(
            "instrument-news HTML entity &{entity}; is unsupported"
        )));
    }
    .map_err(|_| {
        SinaError::Protocol(format!("instrument-news HTML entity &{entity}; is invalid"))
    })?;
    char::from_u32(value).ok_or_else(|| {
        SinaError::Protocol(format!("instrument-news HTML entity &{entity}; is invalid"))
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
