use crate::XinhuaError;
use magic_market_core::{
    DataBatch, HttpsUrl, NewsItem, NonEmptyText, Provenance, ProviderId, SourceEvidence,
};
use std::collections::HashSet;
use time::format_description::well_known::Rfc3339;
use time::{PrimitiveDateTime, UtcOffset};

const MAX_SOURCE_ROWS: usize = 13;
const ROW_START: &str = "<div class=\"ui-zxlist-item\">";
const LIST_END: &str = "<div id=\"listPage\"";
const SOURCE_INFO: &str = "资讯<span> | </span>要闻";

struct ParsedRow {
    id: String,
    title: String,
    category: String,
    canonical_url: String,
    published_at: String,
    epoch: i64,
}

/// Parses and validates the complete bounded source page before applying `limit`.
pub fn parse_listing(html: &str, limit: u32) -> Result<DataBatch<NewsItem>, XinhuaError> {
    if html.len() > 1024 * 1024 {
        return Err(protocol("HTML exceeds the 1 MiB source ceiling"));
    }
    if !(1..=13).contains(&limit) {
        return Err(XinhuaError::InvalidRequest(
            "Xinhua Finance parser limit must be between 1 and 13".into(),
        ));
    }
    let rows = split_rows(html)?;
    if rows.is_empty() || rows.len() > MAX_SOURCE_ROWS {
        return Err(protocol("source page must contain between 1 and 13 rows"));
    }

    let mut parsed = Vec::with_capacity(rows.len());
    let mut ids = HashSet::with_capacity(rows.len());
    let mut urls = HashSet::with_capacity(rows.len());
    let mut previous_epoch = None;
    for row in rows {
        let value = parse_row(row)?;
        if !ids.insert(value.id.clone()) || !urls.insert(value.canonical_url.clone()) {
            return Err(protocol("duplicate item ID or canonical URL"));
        }
        if previous_epoch.is_some_and(|previous| value.epoch > previous) {
            return Err(protocol("source rows are not in non-increasing time order"));
        }
        previous_epoch = Some(value.epoch);
        parsed.push(value);
    }

    let observed_at = now()?;
    let batch_id = format!("xinhua-finance:{observed_at}");
    let returned = parsed.into_iter().take(limit as usize).collect::<Vec<_>>();
    let source_at = returned
        .last()
        .ok_or_else(|| protocol("source page is empty"))?
        .published_at
        .clone();
    let mut records = Vec::with_capacity(returned.len());
    for row in returned {
        let evidence = SourceEvidence::new(
            ProviderId::XinhuaFinance,
            observed_at.clone(),
            batch_id.clone(),
        )?
        .with_source_at(source_at.clone())?;
        records.push(NewsItem {
            item_id: NonEmptyText::new(row.id)?,
            title: NonEmptyText::new(row.title)?,
            summary: None,
            content: None,
            publisher: NonEmptyText::new("新华财经")?,
            canonical_url: HttpsUrl::new(row.canonical_url)?,
            published_at: NonEmptyText::new(row.published_at)?,
            instruments: Vec::new(),
            topics: vec![NonEmptyText::new(row.category)?],
            language: NonEmptyText::new("zh-CN")?,
            evidence,
        });
    }
    let provenance = Provenance::new("xinhua-finance", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn split_rows(html: &str) -> Result<Vec<&str>, XinhuaError> {
    let first = html
        .find(ROW_START)
        .ok_or_else(|| protocol("source page is missing news rows"))?;
    let list_end = html[first..]
        .find(LIST_END)
        .map(|offset| first + offset)
        .ok_or_else(|| protocol("source page is missing the exact list boundary"))?;
    if html[list_end + LIST_END.len()..].contains(ROW_START) {
        return Err(protocol("news rows appear outside the exact list boundary"));
    }

    let mut rows = Vec::new();
    let mut rest = &html[first + ROW_START.len()..list_end];
    loop {
        let next = rest.find(ROW_START);
        let row = next.map_or(rest, |offset| &rest[..offset]);
        if row.trim().is_empty() {
            return Err(protocol("news row is empty"));
        }
        rows.push(row);
        if rows.len() > MAX_SOURCE_ROWS {
            return Err(protocol("source page exceeds 13 rows"));
        }
        let Some(next) = next else {
            break;
        };
        rest = &rest[next + ROW_START.len()..];
    }
    Ok(rows)
}

fn parse_row(row: &str) -> Result<ParsedRow, XinhuaError> {
    let (href, title) = canonical_anchor(row)?;
    let time = exact_tag_text(
        row,
        "<div class=\"ui-publish\">",
        "</div>",
        "publication time",
    )?;
    let source_info = exact_tag_raw(
        row,
        "<div class=\"ui-sourceinfo\">",
        "</div>",
        "source information",
    )?;
    if source_info != SOURCE_INFO {
        return Err(protocol("source information identity is not exact"));
    }
    let (published_at, epoch) = parse_beijing(&time)?;
    let (date, id) = parse_path(&href)?;
    let expected_date = published_at
        .get(..10)
        .ok_or_else(|| protocol("normalized publication date is invalid"))?
        .replace('-', "");
    if date != expected_date {
        return Err(protocol(
            "canonical link date differs from publication date",
        ));
    }
    Ok(ParsedRow {
        id,
        title,
        category: "要闻".into(),
        canonical_url: format!("https:{href}"),
        published_at,
        epoch,
    })
}

fn canonical_anchor(row: &str) -> Result<(String, String), XinhuaError> {
    let mut cursor = row;
    let mut found = None;
    while let Some(start) = cursor.find("<h3><a ") {
        cursor = &cursor[start + "<h3><a ".len()..];
        let close = cursor
            .find('>')
            .ok_or_else(|| protocol("unclosed anchor start tag"))?;
        let attributes = &cursor[..close];
        let after = &cursor[close + 1..];
        let end = after
            .find("</a></h3>")
            .ok_or_else(|| protocol("unclosed anchor"))?;
        let attributes = parse_attributes(attributes)?;
        if attributes.len() != 2 {
            return Err(protocol(
                "canonical anchor must contain exactly href and target attributes",
            ));
        }
        let href = unique_attribute(&attributes, "href")?;
        let target = unique_attribute(&attributes, "target")?;
        if target != "_blank" {
            return Err(protocol("canonical anchor target is not exact"));
        }
        parse_path(href)?;
        if found.is_some() {
            return Err(protocol("row contains multiple canonical anchors"));
        }
        found = Some((href.to_owned(), decode_text(&after[..end])?));
        cursor = &after[end + "</a></h3>".len()..];
    }
    found.ok_or_else(|| protocol("row is missing its canonical anchor"))
}

fn parse_attributes(attributes: &str) -> Result<Vec<(String, String)>, XinhuaError> {
    let mut cursor = attributes;
    let mut parsed = Vec::new();
    loop {
        cursor = cursor.trim_start_matches(char::is_whitespace);
        if cursor.is_empty() {
            break;
        }
        let name_end = cursor
            .find(|character: char| character.is_whitespace() || character == '=')
            .unwrap_or(cursor.len());
        if name_end == 0 {
            return Err(protocol("anchor attribute name is invalid"));
        }
        let attribute_name = &cursor[..name_end];
        if !attribute_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
        {
            return Err(protocol("anchor attribute name is invalid"));
        }
        cursor = cursor[name_end..].trim_start_matches(char::is_whitespace);
        cursor = cursor
            .strip_prefix('=')
            .ok_or_else(|| protocol("anchor attribute must have a quoted value"))?
            .trim_start_matches(char::is_whitespace);
        cursor = cursor
            .strip_prefix('"')
            .ok_or_else(|| protocol("anchor attribute value must use double quotes"))?;
        let value_end = cursor
            .find('"')
            .ok_or_else(|| protocol("unclosed anchor attribute"))?;
        let value = &cursor[..value_end];
        parsed.push((attribute_name.to_owned(), value.to_owned()));
        cursor = &cursor[value_end + 1..];
    }
    Ok(parsed)
}

fn unique_attribute<'a>(
    attributes: &'a [(String, String)],
    name: &str,
) -> Result<&'a str, XinhuaError> {
    let mut values = attributes
        .iter()
        .filter(|(attribute, _)| attribute == name)
        .map(|(_, value)| value.as_str());
    let value = values
        .next()
        .ok_or_else(|| protocol("required anchor attribute is missing"))?;
    if values.next().is_some() {
        return Err(protocol("duplicate anchor attribute"));
    }
    Ok(value)
}

fn exact_tag_text(row: &str, start: &str, end: &str, field: &str) -> Result<String, XinhuaError> {
    let mut matches = row.match_indices(start);
    let (offset, _) = matches
        .next()
        .ok_or_else(|| protocol(&format!("missing {field}")))?;
    if matches.next().is_some() {
        return Err(protocol(&format!("duplicate {field}")));
    }
    let value = &row[offset + start.len()..];
    let close = value
        .find(end)
        .ok_or_else(|| protocol(&format!("unclosed {field}")))?;
    decode_text(&value[..close])
}

fn exact_tag_raw(row: &str, start: &str, end: &str, field: &str) -> Result<String, XinhuaError> {
    let mut matches = row.match_indices(start);
    let (offset, _) = matches
        .next()
        .ok_or_else(|| protocol(&format!("missing {field}")))?;
    if matches.next().is_some() {
        return Err(protocol(&format!("duplicate {field}")));
    }
    let value = &row[offset + start.len()..];
    let close = value
        .find(end)
        .ok_or_else(|| protocol(&format!("unclosed {field}")))?;
    Ok(value[..close].trim().to_owned())
}

fn parse_path(href: &str) -> Result<(String, String), XinhuaError> {
    if href.contains(['?', '#', '\\']) || href.chars().any(char::is_whitespace) {
        return Err(protocol("canonical link contains unsafe URL components"));
    }
    let value = href
        .strip_prefix("//www.cnfin.com/yw-lb/detail/")
        .and_then(|value| value.strip_suffix("_1.html"))
        .ok_or_else(|| protocol("canonical link path is invalid"))?;
    let (date, id) = value
        .split_once('/')
        .ok_or_else(|| protocol("canonical link path is incomplete"))?;
    if date.len() != 8
        || !date.bytes().all(|byte| byte.is_ascii_digit())
        || id.is_empty()
        || !id.bytes().all(|byte| byte.is_ascii_digit())
        || id.contains('/')
    {
        return Err(protocol("canonical link date or ID is invalid"));
    }
    Ok((date.to_owned(), id.to_owned()))
}

fn parse_beijing(value: &str) -> Result<(String, i64), XinhuaError> {
    let format = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    let local = PrimitiveDateTime::parse(value, format)
        .map_err(|_| protocol("publication time is malformed"))?;
    let offset = UtcOffset::from_hms(8, 0, 0).map_err(|_| protocol("Beijing offset is invalid"))?;
    let timestamp = local.assume_offset(offset);
    let formatted = timestamp
        .format(&Rfc3339)
        .map_err(|_| protocol("publication time cannot be normalized"))?;
    Ok((formatted, timestamp.unix_timestamp()))
}

fn decode_text(value: &str) -> Result<String, XinhuaError> {
    if value.contains('<') {
        return Err(protocol("nested markup is forbidden in metadata text"));
    }
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(index) = rest.find('&') {
        output.push_str(&rest[..index]);
        let entity = &rest[index + 1..];
        let end = entity
            .find(';')
            .ok_or_else(|| protocol("unclosed HTML entity"))?;
        let name = &entity[..end];
        let decoded = match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            numeric if numeric.starts_with("#x") || numeric.starts_with("#X") => {
                let value = u32::from_str_radix(&numeric[2..], 16)
                    .map_err(|_| protocol("invalid numeric HTML entity"))?;
                char::from_u32(value).ok_or_else(|| protocol("invalid Unicode scalar"))?
            }
            numeric if numeric.starts_with('#') => {
                let value = numeric[1..]
                    .parse::<u32>()
                    .map_err(|_| protocol("invalid numeric HTML entity"))?;
                char::from_u32(value).ok_or_else(|| protocol("invalid Unicode scalar"))?
            }
            _ => return Err(protocol("unsupported named HTML entity")),
        };
        output.push(decoded);
        rest = &entity[end + 1..];
    }
    output.push_str(rest);
    let trimmed = output.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return Err(protocol("metadata text is empty or contains controls"));
    }
    Ok(trimmed.to_owned())
}

fn now() -> Result<String, XinhuaError> {
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| protocol("observation time cannot be formatted"))
}

fn protocol(message: &str) -> XinhuaError {
    XinhuaError::Protocol(message.into())
}
