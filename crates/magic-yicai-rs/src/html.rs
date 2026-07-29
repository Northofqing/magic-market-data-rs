use crate::YicaiError;
use magic_market_core::{
    DataBatch, HttpsUrl, NewsItem, NonEmptyText, Provenance, ProviderId, SourceEvidence,
};
use serde::Deserialize;
use std::collections::HashSet;
use time::format_description::well_known::Rfc3339;
use time::{PrimitiveDateTime, UtcOffset};

const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_EMBEDDED_JSON_BYTES: usize = 512 * 1024;
const MAX_SOURCE_ROWS: usize = 300;

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct SourceRow {
    NewsID: u64,
    NewsTitle: String,
    CreateDate: String,
    NewsSource: String,
    url: String,
}

struct ParsedRow {
    id: String,
    title: String,
    publisher: String,
    canonical_url: String,
    published_at: String,
    epoch: i64,
}

/// Parses and validates the complete bounded embedded list before applying `limit`.
pub fn parse_listing(html: &str, limit: u32) -> Result<DataBatch<NewsItem>, YicaiError> {
    if html.len() > MAX_HTML_BYTES {
        return Err(protocol("HTML exceeds the 2 MiB source ceiling"));
    }
    if !(1..=50).contains(&limit) {
        return Err(YicaiError::InvalidRequest(
            "Yicai parser limit must be between 1 and 50".into(),
        ));
    }
    let payload = extract_firstlist(html)?;
    let rows: Vec<SourceRow> = serde_json::from_str(payload)
        .map_err(|_| YicaiError::Decode("firstlist JSON is invalid".into()))?;
    if rows.is_empty() || rows.len() > MAX_SOURCE_ROWS {
        return Err(protocol(
            "firstlist must contain between 1 and 300 source objects",
        ));
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
    let batch_id = format!("yicai:{observed_at}");
    let returned = parsed.into_iter().take(limit as usize).collect::<Vec<_>>();
    let source_at = returned
        .last()
        .ok_or_else(|| protocol("firstlist is empty"))?
        .published_at
        .clone();
    let mut records = Vec::with_capacity(returned.len());
    for row in returned {
        let evidence =
            SourceEvidence::new(ProviderId::Yicai, observed_at.clone(), batch_id.clone())?
                .with_source_at(source_at.clone())?;
        records.push(NewsItem {
            item_id: NonEmptyText::new(row.id)?,
            title: NonEmptyText::new(row.title)?,
            summary: None,
            content: None,
            publisher: NonEmptyText::new(row.publisher)?,
            canonical_url: HttpsUrl::new(row.canonical_url)?,
            published_at: NonEmptyText::new(row.published_at)?,
            instruments: Vec::new(),
            topics: Vec::new(),
            language: NonEmptyText::new("zh-CN")?,
            evidence,
        });
    }
    let provenance = Provenance::new("yicai", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn extract_firstlist(html: &str) -> Result<&str, YicaiError> {
    let mut assignment = None;
    let mut cursor = html;
    while let Some(script_start) = cursor.find("<script") {
        let after_name = &cursor[script_start + "<script".len()..];
        if !after_name
            .chars()
            .next()
            .is_some_and(|character| character == '>' || character.is_whitespace())
        {
            cursor = after_name;
            continue;
        }
        let open_end = after_name
            .find('>')
            .ok_or_else(|| protocol("script start tag is unclosed"))?;
        let after_open = &after_name[open_end + 1..];
        let close = after_open
            .find("</script>")
            .ok_or_else(|| protocol("script element is unclosed"))?;
        let script = &after_open[..close];
        if executable_script(&after_name[..open_end])? {
            for offset in executable_assignment_offsets(script) {
                if assignment.replace(&script[offset..]).is_some() {
                    return Err(protocol("multiple firstlist assignments are forbidden"));
                }
            }
        }
        cursor = &after_open[close + "</script>".len()..];
    }
    let rest = assignment
        .ok_or_else(|| protocol("firstlist assignment is missing"))?
        .trim_start();
    if !rest.starts_with('[') {
        return Err(protocol(
            "firstlist assignment does not start with an array",
        ));
    }
    let bytes = rest.as_bytes();
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if index >= MAX_EMBEDDED_JSON_BYTES {
            return Err(protocol("firstlist JSON exceeds 512 KiB"));
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'[' => {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| protocol("firstlist nesting overflow"))?
            }
            b']' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| protocol("unbalanced firstlist array"))?;
                if depth == 0 {
                    return rest
                        .get(..=index)
                        .ok_or_else(|| protocol("firstlist JSON boundary is invalid"));
                }
            }
            _ => {}
        }
    }
    Err(protocol("firstlist array is unclosed"))
}

fn executable_script(attributes: &str) -> Result<bool, YicaiError> {
    let mut cursor = attributes;
    let mut script_type = None;
    loop {
        cursor = cursor.trim_start_matches(char::is_whitespace);
        if cursor.is_empty() {
            break;
        }
        let name_end = cursor
            .find(|character: char| character.is_whitespace() || character == '=')
            .unwrap_or(cursor.len());
        if name_end == 0 {
            return Err(protocol("script attribute name is invalid"));
        }
        let name = &cursor[..name_end];
        if !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':'))
        {
            return Err(protocol("script attribute name is invalid"));
        }
        cursor = cursor[name_end..].trim_start_matches(char::is_whitespace);
        let value = if let Some(after_equals) = cursor.strip_prefix('=') {
            cursor = after_equals.trim_start_matches(char::is_whitespace);
            let quote = *cursor
                .as_bytes()
                .first()
                .filter(|byte| matches!(byte, b'\'' | b'"'))
                .ok_or_else(|| protocol("script attribute value must be quoted"))?;
            cursor = &cursor[1..];
            let end = cursor
                .as_bytes()
                .iter()
                .position(|byte| *byte == quote)
                .ok_or_else(|| protocol("script attribute value is unclosed"))?;
            let value = &cursor[..end];
            cursor = &cursor[end + 1..];
            Some(value)
        } else {
            None
        };
        if name == "type" && script_type.replace(value.unwrap_or_default()).is_some() {
            return Err(protocol("duplicate script type attribute"));
        }
    }
    Ok(script_type.is_none_or(|value| {
        matches!(
            value,
            "text/javascript" | "application/javascript" | "module"
        )
    }))
}

fn executable_assignment_offsets(script: &str) -> Vec<usize> {
    let bytes = script.as_bytes();
    let mut offsets = Vec::new();
    let mut index = 0;
    let mut statement_start = true;
    let mut regex_allowed = true;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if bytes[index..].starts_with(b"//") {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            index += 2;
            while index + 1 < bytes.len() && !bytes[index..].starts_with(b"*/") {
                index += 1;
            }
            index = (index + 2).min(bytes.len());
            continue;
        }
        if matches!(bytes[index], b'\'' | b'"' | b'`') {
            index = skip_js_string(bytes, index);
            statement_start = false;
            regex_allowed = false;
            continue;
        }
        if bytes[index] == b'/' && regex_allowed {
            index = skip_js_regex(bytes, index);
            statement_start = false;
            regex_allowed = false;
            continue;
        }
        if is_identifier_start(bytes[index]) {
            let end = identifier_end(bytes, index);
            let identifier = &script[index..end];
            if statement_start && identifier == "var" {
                let firstlist_start = skip_js_trivia(bytes, end);
                let firstlist_end = identifier_end(bytes, firstlist_start);
                if firstlist_start < bytes.len()
                    && &script[firstlist_start..firstlist_end] == "firstlist"
                {
                    let equals = skip_js_trivia(bytes, firstlist_end);
                    if bytes.get(equals) == Some(&b'=')
                        && !matches!(bytes.get(equals + 1), Some(b'=' | b'>'))
                    {
                        offsets.push(equals + 1);
                    }
                }
            }
            index = end;
            statement_start = false;
            regex_allowed = matches!(
                identifier,
                "return"
                    | "throw"
                    | "case"
                    | "delete"
                    | "void"
                    | "typeof"
                    | "new"
                    | "in"
                    | "instanceof"
                    | "yield"
                    | "await"
            );
            continue;
        }
        if bytes[index].is_ascii_digit() {
            index += 1;
            while bytes
                .get(index)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'.')
            {
                index += 1;
            }
            statement_start = false;
            regex_allowed = false;
            continue;
        }
        statement_start = matches!(bytes[index], b';' | b'{' | b'}');
        regex_allowed = match bytes[index] {
            b')' | b']' | b'.' => false,
            b';' | b'{' | b'}' | b'(' | b'[' | b',' | b':' | b'?' | b'=' | b'!' | b'&' | b'|'
            | b'+' | b'-' | b'*' | b'%' | b'<' | b'>' | b'/' => true,
            _ => regex_allowed,
        };
        index += 1;
    }
    offsets
}

fn skip_js_trivia(bytes: &[u8], mut index: usize) -> usize {
    loop {
        while bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            index += 1;
        }
        if bytes
            .get(index..)
            .is_some_and(|rest| rest.starts_with(b"//"))
        {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes
            .get(index..)
            .is_some_and(|rest| rest.starts_with(b"/*"))
        {
            index += 2;
            while index + 1 < bytes.len() && !bytes[index..].starts_with(b"*/") {
                index += 1;
            }
            index = (index + 2).min(bytes.len());
            continue;
        }
        return index;
    }
}

fn skip_js_string(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut index = start + 1;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            break;
        }
    }
    index
}

fn skip_js_regex(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    let mut escaped = false;
    let mut in_class = false;
    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'[' {
            in_class = true;
        } else if byte == b']' {
            in_class = false;
        } else if byte == b'/' && !in_class {
            while bytes
                .get(index)
                .is_some_and(|flag| flag.is_ascii_alphabetic())
            {
                index += 1;
            }
            break;
        } else if matches!(byte, b'\r' | b'\n') {
            return bytes.len();
        }
    }
    index
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$')
}

fn identifier_end(bytes: &[u8], start: usize) -> usize {
    if !bytes
        .get(start)
        .is_some_and(|byte| is_identifier_start(*byte))
    {
        return start;
    }
    let mut end = start + 1;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
    {
        end += 1;
    }
    end
}

fn parse_row(row: SourceRow) -> Result<ParsedRow, YicaiError> {
    if row.NewsID == 0 {
        return Err(protocol("NewsID must be positive"));
    }
    let id = row.NewsID.to_string();
    let title = normalized_display_text(row.NewsTitle, "NewsTitle")?;
    let publisher = normalized_display_text(row.NewsSource, "NewsSource")?;
    let expected_path = format!("/news/{id}.html");
    if row.url != expected_path {
        return Err(protocol("row URL does not exactly match NewsID"));
    }
    let (published_at, epoch) = parse_beijing(&row.CreateDate)?;
    Ok(ParsedRow {
        id,
        title,
        publisher,
        canonical_url: format!("https://www.yicai.com{expected_path}"),
        published_at,
        epoch,
    })
}

fn normalized_display_text(value: String, field: &str) -> Result<String, YicaiError> {
    if value.chars().any(char::is_control) {
        return Err(protocol(&format!("{field} contains a control character")));
    }
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(protocol(&format!("{field} is empty after trimming")));
    }
    Ok(trimmed.to_owned())
}

fn parse_beijing(value: &str) -> Result<(String, i64), YicaiError> {
    let format = time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");
    let local =
        PrimitiveDateTime::parse(value, format).map_err(|_| protocol("CreateDate is malformed"))?;
    let offset = UtcOffset::from_hms(8, 0, 0).map_err(|_| protocol("Beijing offset is invalid"))?;
    let timestamp = local.assume_offset(offset);
    let formatted = timestamp
        .format(&Rfc3339)
        .map_err(|_| protocol("CreateDate cannot be normalized"))?;
    Ok((formatted, timestamp.unix_timestamp()))
}

fn now() -> Result<String, YicaiError> {
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| protocol("observation time cannot be formatted"))
}

fn protocol(message: &str) -> YicaiError {
    YicaiError::Protocol(message.into())
}

#[cfg(test)]
mod tests;
