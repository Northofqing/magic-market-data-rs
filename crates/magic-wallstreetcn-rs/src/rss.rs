use crate::{validate_returned_limit, WallstreetCnError};
use magic_market_core::{
    DataBatch, HttpsUrl, NewsItem, NonEmptyText, Provenance, ProviderId, SourceEvidence,
};
use quick_xml::encoding::Decoder;
use quick_xml::events::{BytesDecl, BytesRef, BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::XmlVersion;
use std::collections::HashSet;
use time::format_description::well_known::{Rfc2822, Rfc3339};
use time::OffsetDateTime;

const MAX_SOURCE_ITEMS: usize = 100;
const ARTICLE_PREFIX: &str = "https://wallstreetcn.com/articles/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    ChannelTitle,
    ChannelLink,
    ChannelLanguage,
    ItemTitle,
    ItemLink,
    ItemSource,
    ItemPubDate,
}

impl Field {
    const fn name(self) -> &'static str {
        match self {
            Self::ChannelTitle | Self::ItemTitle => "title",
            Self::ChannelLink | Self::ItemLink => "link",
            Self::ChannelLanguage => "language",
            Self::ItemSource => "source",
            Self::ItemPubDate => "pubDate",
        }
    }
}

#[derive(Debug, Default)]
struct RawItem {
    title: Option<String>,
    link: Option<String>,
    source: Option<String>,
    published_at: Option<String>,
}

impl RawItem {
    fn has(&self, field: Field) -> bool {
        match field {
            Field::ItemTitle => self.title.is_some(),
            Field::ItemLink => self.link.is_some(),
            Field::ItemSource => self.source.is_some(),
            Field::ItemPubDate => self.published_at.is_some(),
            Field::ChannelTitle | Field::ChannelLink | Field::ChannelLanguage => false,
        }
    }

    fn set(&mut self, field: Field, value: String) -> Result<(), WallstreetCnError> {
        match field {
            Field::ItemTitle => self.title = Some(value),
            Field::ItemLink => self.link = Some(value),
            Field::ItemSource => self.source = Some(value),
            Field::ItemPubDate => self.published_at = Some(value),
            Field::ChannelTitle | Field::ChannelLink | Field::ChannelLanguage => {
                return Err(WallstreetCnError::Protocol(
                    "channel field was assigned to an RSS item".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct RssState {
    stack: Vec<String>,
    ignored_depth: usize,
    active_field: Option<Field>,
    active_text: String,
    saw_rss: bool,
    closed_rss: bool,
    saw_channel: bool,
    closed_channel: bool,
    channel_title: Option<String>,
    channel_link: Option<String>,
    channel_language: Option<String>,
    current_item: Option<RawItem>,
    items: Vec<RawItem>,
}

impl RssState {
    fn start(&mut self, name: String) -> Result<(), WallstreetCnError> {
        if self.ignored_depth > 0 {
            self.ignored_depth += 1;
            self.stack.push(name);
            return Ok(());
        }
        if self.active_field.is_some() {
            return Err(WallstreetCnError::Protocol(
                "recognized RSS fields must not contain nested elements".into(),
            ));
        }

        match self.stack.as_slice() {
            [] => {
                if name != "rss" || self.saw_rss {
                    return Err(WallstreetCnError::Protocol(
                        "RSS document must contain exactly one rss root".into(),
                    ));
                }
                self.saw_rss = true;
            }
            [root] if root == "rss" => {
                if name != "channel" || self.saw_channel || self.closed_channel {
                    return Err(WallstreetCnError::Protocol(
                        "RSS root must contain exactly one channel".into(),
                    ));
                }
                self.saw_channel = true;
            }
            [root, channel] if root == "rss" && channel == "channel" => {
                if name == "item" {
                    if self.current_item.is_some() {
                        return Err(WallstreetCnError::Protocol(
                            "nested RSS items are not permitted".into(),
                        ));
                    }
                    self.current_item = Some(RawItem::default());
                } else if let Some(field) = channel_field(&name) {
                    if self.channel_has(field) {
                        return Err(WallstreetCnError::Protocol(format!(
                            "RSS channel contains duplicate {} field",
                            field.name()
                        )));
                    }
                    self.active_field = Some(field);
                    self.active_text.clear();
                } else {
                    self.ignored_depth = 1;
                }
            }
            [root, channel, item] if root == "rss" && channel == "channel" && item == "item" => {
                if let Some(field) = item_field(&name) {
                    let current = self.current_item.as_ref().ok_or_else(|| {
                        WallstreetCnError::Protocol("RSS item parser state is missing".into())
                    })?;
                    if current.has(field) {
                        return Err(WallstreetCnError::Protocol(format!(
                            "RSS item contains duplicate {} field",
                            field.name()
                        )));
                    }
                    self.active_field = Some(field);
                    self.active_text.clear();
                } else {
                    self.ignored_depth = 1;
                }
            }
            _ if name == "item" || name == "channel" || name == "rss" => {
                return Err(WallstreetCnError::Protocol(format!(
                    "RSS structural element {name} is nested incorrectly"
                )));
            }
            _ => {
                self.ignored_depth = 1;
            }
        }
        self.stack.push(name);
        Ok(())
    }

    fn end(&mut self, name: &str) -> Result<(), WallstreetCnError> {
        if self.stack.last().map(String::as_str) != Some(name) {
            return Err(WallstreetCnError::Decode(format!(
                "RSS closing element {name} does not match parser state"
            )));
        }
        if self.ignored_depth > 0 {
            self.ignored_depth -= 1;
            self.stack.pop();
            return Ok(());
        }

        if let Some(field) = self.active_field.take() {
            if field.name() != name {
                return Err(WallstreetCnError::Protocol(
                    "RSS field parser state disagrees with its closing element".into(),
                ));
            }
            let value = std::mem::take(&mut self.active_text);
            self.set_field(field, value)?;
        } else if self.stack.len() == 3 && name == "item" {
            let item = self.current_item.take().ok_or_else(|| {
                WallstreetCnError::Protocol("RSS item closed without parser state".into())
            })?;
            self.items.push(item);
            if self.items.len() > MAX_SOURCE_ITEMS {
                return Err(WallstreetCnError::Protocol(format!(
                    "WallstreetCN RSS exceeds the {MAX_SOURCE_ITEMS}-item source bound"
                )));
            }
        } else if self.stack.len() == 2 && name == "channel" {
            self.closed_channel = true;
        } else if self.stack.len() == 1 && name == "rss" {
            self.closed_rss = true;
        }
        self.stack.pop();
        Ok(())
    }

    fn append_text(&mut self, value: &str) -> Result<(), WallstreetCnError> {
        if self.ignored_depth > 0 {
            return Ok(());
        }
        if self.active_field.is_some() {
            self.active_text.push_str(value);
            Ok(())
        } else if value.trim().is_empty() {
            Ok(())
        } else {
            Err(WallstreetCnError::Protocol(
                "RSS structural elements must not contain text".into(),
            ))
        }
    }

    fn channel_has(&self, field: Field) -> bool {
        match field {
            Field::ChannelTitle => self.channel_title.is_some(),
            Field::ChannelLink => self.channel_link.is_some(),
            Field::ChannelLanguage => self.channel_language.is_some(),
            Field::ItemTitle | Field::ItemLink | Field::ItemSource | Field::ItemPubDate => false,
        }
    }

    fn set_field(&mut self, field: Field, value: String) -> Result<(), WallstreetCnError> {
        match field {
            Field::ChannelTitle => self.channel_title = Some(value),
            Field::ChannelLink => self.channel_link = Some(value),
            Field::ChannelLanguage => self.channel_language = Some(value),
            Field::ItemTitle | Field::ItemLink | Field::ItemSource | Field::ItemPubDate => self
                .current_item
                .as_mut()
                .ok_or_else(|| {
                    WallstreetCnError::Protocol("RSS item parser state is missing".into())
                })?
                .set(field, value)?,
        }
        Ok(())
    }

    fn finish(self) -> Result<(String, String, String, Vec<RawItem>), WallstreetCnError> {
        if !self.saw_rss
            || !self.closed_rss
            || !self.saw_channel
            || !self.closed_channel
            || !self.stack.is_empty()
            || self.current_item.is_some()
            || self.active_field.is_some()
            || self.ignored_depth != 0
        {
            return Err(WallstreetCnError::Protocol(
                "RSS document structure is incomplete".into(),
            ));
        }
        if self.items.is_empty() {
            return Err(WallstreetCnError::Protocol(
                "WallstreetCN returned an empty RSS feed".into(),
            ));
        }
        Ok((
            required_text("channel title", self.channel_title)?,
            required_text("channel link", self.channel_link)?,
            required_text("channel language", self.channel_language)?,
            self.items,
        ))
    }
}

fn channel_field(name: &str) -> Option<Field> {
    match name {
        "title" => Some(Field::ChannelTitle),
        "link" => Some(Field::ChannelLink),
        "language" => Some(Field::ChannelLanguage),
        _ => None,
    }
}

fn item_field(name: &str) -> Option<Field> {
    match name {
        "title" => Some(Field::ItemTitle),
        "link" => Some(Field::ItemLink),
        "source" => Some(Field::ItemSource),
        "pubDate" => Some(Field::ItemPubDate),
        _ => None,
    }
}

pub(crate) fn parse_response(
    body: &[u8],
    limit: u32,
    observed_at: &str,
) -> Result<DataBatch<NewsItem>, WallstreetCnError> {
    validate_returned_limit(limit)?;
    crate::transport::ensure_body_size(body)?;
    let document = std::str::from_utf8(body)
        .map_err(|error| WallstreetCnError::Decode(format!("RSS is not UTF-8: {error}")))?;
    validate_xml10_characters("RSS document", document)?;
    if body.iter().all(u8::is_ascii_whitespace) {
        return Err(WallstreetCnError::Protocol(
            "WallstreetCN returned an empty RSS body".into(),
        ));
    }

    let mut reader = Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    reader.config_mut().check_end_names = true;
    reader.config_mut().check_comments = true;
    let mut state = RssState::default();
    let mut saw_declaration = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| WallstreetCnError::Decode(format!("RSS XML: {error}")))?;
        match event {
            Event::Start(element) => {
                let name = xml_name(element.name().as_ref())?;
                validate_attributes(&element, reader.decoder(), &state, &name)?;
                state.start(name)?;
            }
            Event::Empty(element) => {
                let name = xml_name(element.name().as_ref())?;
                validate_attributes(&element, reader.decoder(), &state, &name)?;
                state.start(name.clone())?;
                state.end(&name)?;
            }
            Event::End(element) => {
                let name = xml_name(element.name().as_ref())?;
                state.end(&name)?;
            }
            Event::Text(text) => {
                if state.ignored_depth == 0 {
                    let decoded = text
                        .xml10_content()
                        .map_err(|error| WallstreetCnError::Decode(format!("RSS text: {error}")))?;
                    state.append_text(&decoded)?;
                }
            }
            Event::CData(text) => {
                if state.ignored_depth == 0 {
                    let decoded = text.xml10_content().map_err(|error| {
                        WallstreetCnError::Decode(format!("RSS CDATA: {error}"))
                    })?;
                    state.append_text(&decoded)?;
                }
            }
            Event::GeneralRef(reference) => {
                let resolved = resolve_reference(&reference)?;
                state.append_text(&resolved)?;
            }
            Event::DocType(_) => {
                return Err(WallstreetCnError::Protocol(
                    "WallstreetCN RSS must not contain a DOCTYPE".into(),
                ));
            }
            Event::Decl(declaration) => {
                if saw_declaration || !state.stack.is_empty() || state.saw_rss {
                    return Err(WallstreetCnError::Protocol(
                        "RSS XML declaration must be unique and precede the document root".into(),
                    ));
                }
                validate_xml_declaration(&declaration)?;
                saw_declaration = true;
            }
            Event::Comment(_) | Event::PI(_) => {}
            Event::Eof => break,
        }
    }

    let (channel_title, channel_link, channel_language, raw_items) = state.finish()?;
    if channel_title != "华尔街见闻"
        || channel_link != "https://wallstreetcn.com"
        || channel_language != "zh-hans"
    {
        return Err(WallstreetCnError::Protocol(
            "WallstreetCN RSS channel identity does not match the admitted public feed".into(),
        ));
    }

    let batch_id = format!("wallstreetcn:{observed_at}");
    let mut seen_ids = HashSet::with_capacity(raw_items.len());
    let mut seen_numeric_ids = HashSet::with_capacity(raw_items.len());
    let mut seen_urls = HashSet::with_capacity(raw_items.len());
    let mut previous_time = None;
    let mut parsed = Vec::with_capacity(raw_items.len());
    for raw in raw_items {
        let title = required_text("item title", raw.title)?;
        let canonical_url = required_text("item link", raw.link)?;
        let article_id = validate_article_url(&canonical_url)?;
        let numeric_identity = article_id.trim_start_matches('0');
        let numeric_identity = if numeric_identity.is_empty() {
            "0"
        } else {
            numeric_identity
        };
        let source = required_text("item source", raw.source)?;
        if source != "华尔街见闻" {
            return Err(WallstreetCnError::Protocol(
                "WallstreetCN RSS item source must be exactly 华尔街见闻".into(),
            ));
        }
        let published_source = required_text("item pubDate", raw.published_at)?;
        let source_time = OffsetDateTime::parse(&published_source, &Rfc2822).map_err(|error| {
            WallstreetCnError::Protocol(format!("invalid RSS pubDate: {error}"))
        })?;
        if previous_time.is_some_and(|previous| previous < source_time) {
            return Err(WallstreetCnError::Protocol(
                "WallstreetCN RSS items are not newest-first".into(),
            ));
        }
        previous_time = Some(source_time);
        let published_at = source_time
            .format(&Rfc3339)
            .map_err(|error| WallstreetCnError::Protocol(format!("RSS time format: {error}")))?;
        if !seen_ids.insert(article_id.clone())
            || !seen_numeric_ids.insert(numeric_identity.to_owned())
            || !seen_urls.insert(canonical_url.clone())
        {
            return Err(WallstreetCnError::Protocol(format!(
                "duplicate WallstreetCN article identity {article_id}"
            )));
        }
        let evidence = SourceEvidence::new(ProviderId::WallstreetCn, observed_at, &batch_id)?
            .with_source_at(&published_at)?;
        parsed.push(NewsItem {
            item_id: NonEmptyText::new(article_id)?,
            title: NonEmptyText::new(title)?,
            summary: None,
            content: None,
            publisher: NonEmptyText::new("华尔街见闻")?,
            canonical_url: HttpsUrl::new(canonical_url)?,
            published_at: NonEmptyText::new(published_at)?,
            instruments: Vec::new(),
            topics: vec![NonEmptyText::new("华尔街见闻")?],
            language: NonEmptyText::new("zh-CN")?,
            evidence,
        });
    }

    parsed.truncate(limit as usize);
    let latest_source_at = parsed
        .first()
        .map(|item| item.published_at.as_str())
        .ok_or_else(|| WallstreetCnError::Protocol("latest source time is missing".into()))?;
    let provenance = Provenance::new("wallstreetcn-rss-v1", observed_at)?
        .with_source_at(latest_source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(parsed, provenance))
}

fn validate_attributes(
    element: &BytesStart<'_>,
    decoder: Decoder,
    state: &RssState,
    name: &str,
) -> Result<(), WallstreetCnError> {
    let root = state.stack.is_empty() && name == "rss";
    let strict = state.ignored_depth == 0
        && (root
            || name == "channel"
            || name == "item"
            || channel_field(name).is_some()
            || item_field(name).is_some());
    let mut version = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| WallstreetCnError::Decode(format!("RSS XML attribute: {error}")))?;
        let key = xml_name(attribute.key.as_ref())?;
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
            .map_err(|error| WallstreetCnError::Decode(format!("RSS XML attribute: {error}")))?;
        validate_xml10_characters("RSS XML attribute", &value)?;
        if root && key == "version" {
            if version.replace(value.into_owned()).is_some() {
                return Err(WallstreetCnError::Protocol(
                    "RSS root contains duplicate version attributes".into(),
                ));
            }
        } else if key == "xmlns" || key.starts_with("xmlns:") {
            if value.is_empty() {
                return Err(WallstreetCnError::Protocol(
                    "RSS namespace attributes must not be empty".into(),
                ));
            }
        } else if strict {
            return Err(WallstreetCnError::Protocol(format!(
                "unexpected RSS attribute {key} on {name}"
            )));
        }
    }
    if root && version.as_deref() != Some("2.0") {
        return Err(WallstreetCnError::Protocol(
            "RSS root must declare version 2.0".into(),
        ));
    }
    Ok(())
}

fn validate_xml_declaration(declaration: &BytesDecl<'_>) -> Result<(), WallstreetCnError> {
    let version = declaration.version().map_err(|error| {
        WallstreetCnError::Protocol(format!("invalid RSS XML declaration: {error}"))
    })?;
    if version.as_ref() != b"1.0" {
        return Err(WallstreetCnError::Protocol(
            "WallstreetCN RSS XML declaration must use version 1.0".into(),
        ));
    }

    let raw = std::str::from_utf8(declaration.as_ref()).map_err(|error| {
        WallstreetCnError::Decode(format!("RSS XML declaration is not UTF-8: {error}"))
    })?;
    let start = BytesStart::from_content(raw, 3);
    let mut position = 0_u8;
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| {
            WallstreetCnError::Protocol(format!("invalid RSS XML declaration: {error}"))
        })?;
        let key = xml_name(attribute.key.as_ref())?;
        let value = std::str::from_utf8(attribute.value.as_ref()).map_err(|error| {
            WallstreetCnError::Decode(format!("RSS XML declaration is not UTF-8: {error}"))
        })?;
        validate_xml10_characters("RSS XML declaration", value)?;
        match (position, key.as_str()) {
            (0, "version") if value == "1.0" => position = 1,
            (1, "encoding") if value.eq_ignore_ascii_case("utf-8") => position = 2,
            (1 | 2, "standalone") if value == "yes" || value == "no" => {
                position = 3;
            }
            _ => {
                return Err(WallstreetCnError::Protocol(format!(
                    "unexpected or out-of-order RSS XML declaration attribute {key}"
                )));
            }
        }
    }
    if position == 0 {
        return Err(WallstreetCnError::Protocol(
            "WallstreetCN RSS XML declaration is missing version 1.0".into(),
        ));
    }
    Ok(())
}

fn xml_name(bytes: &[u8]) -> Result<String, WallstreetCnError> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|error| {
            WallstreetCnError::Decode(format!("RSS element name is not UTF-8: {error}"))
        })
}

fn validate_xml10_characters(context: &str, value: &str) -> Result<(), WallstreetCnError> {
    if value.chars().all(is_xml10_character) {
        return Ok(());
    }
    Err(WallstreetCnError::Protocol(format!(
        "{context} contains characters forbidden by XML 1.0"
    )))
}

fn is_xml10_character(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{A}' | '\u{D}')
        || ('\u{20}'..='\u{D7FF}').contains(&character)
        || ('\u{E000}'..='\u{FFFD}').contains(&character)
        || ('\u{10000}'..='\u{10FFFF}').contains(&character)
}

fn resolve_reference(reference: &BytesRef<'_>) -> Result<String, WallstreetCnError> {
    if let Some(character) = reference
        .resolve_char_ref()
        .map_err(|error| WallstreetCnError::Protocol(format!("invalid numeric entity: {error}")))?
    {
        if !is_xml10_character(character) || character.is_control() {
            return Err(WallstreetCnError::Protocol(
                "RSS numeric entities must resolve to permitted XML 1.0 characters".into(),
            ));
        }
        return Ok(character.to_string());
    }
    let name = reference
        .decode()
        .map_err(|error| WallstreetCnError::Decode(format!("RSS entity name: {error}")))?;
    let value = match name.as_ref() {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "apos" => "'",
        "quot" => "\"",
        _ => {
            return Err(WallstreetCnError::Protocol(format!(
                "custom RSS entity &{name}; is not permitted"
            )));
        }
    };
    Ok(value.to_owned())
}

fn required_text(field: &'static str, value: Option<String>) -> Result<String, WallstreetCnError> {
    let value = value.ok_or_else(|| {
        WallstreetCnError::Protocol(format!("WallstreetCN RSS is missing required {field}"))
    })?;
    let normalized = normalize_text(&value);
    if normalized.is_empty() {
        return Err(WallstreetCnError::Protocol(format!(
            "WallstreetCN RSS {field} must not be empty"
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(WallstreetCnError::Protocol(format!(
            "WallstreetCN RSS {field} contains control characters"
        )));
    }
    Ok(normalized)
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_article_url(url: &str) -> Result<String, WallstreetCnError> {
    let article_id = url.strip_prefix(ARTICLE_PREFIX).ok_or_else(|| {
        WallstreetCnError::Protocol(
            "WallstreetCN article URL must use the exact official HTTPS path".into(),
        )
    })?;
    if article_id.is_empty()
        || article_id.len() > 20
        || !article_id.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(WallstreetCnError::Protocol(
            "WallstreetCN article ID must contain 1 through 20 ASCII digits".into(),
        ));
    }
    Ok(article_id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use magic_market_core::ProviderId;

    const OBSERVED_AT: &str = "2026-07-26T10:31:00+08:00";

    fn item(id: &str, title: &str, published_at: &str) -> String {
        format!(
            "<item>\
               <title>{title}</title>\
               <link>https://wallstreetcn.com/articles/{id}</link>\
               <source>华尔街见闻</source>\
               <pubDate>{published_at}</pubDate>\
             </item>"
        )
    }

    fn feed(items: &str) -> String {
        feed_with_identity("华尔街见闻", "https://wallstreetcn.com", "zh-hans", items)
    }

    fn feed_with_identity(title: &str, link: &str, language: &str, items: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
             <rss version=\"2.0\" xmlns:content=\"urn:synthetic-content\">\
               <channel>\
                 <title>{title}</title>\
                 <link>{link}</link>\
                 <description></description>\
                 <language>{language}</language>\
                 {items}\
               </channel>\
             </rss>"
        )
    }

    fn valid_feed() -> String {
        let first = "<item>\
               <title><![CDATA[ 合成财经标题一 ]]></title>\
               <link>https://wallstreetcn.com/articles/3779002</link>\
               <description><![CDATA[ NEVER_EXPOSE_DESCRIPTION ]]></description>\
               <content:encoded><![CDATA[ NEVER_EXPOSE_BODY ]]></content:encoded>\
               <source>华尔街见闻</source>\
               <pubDate>Sun, 26 Jul 2026 10:30:00 +0800</pubDate>\
             </item>";
        let second = item(
            "3779001",
            "合成财经标题二",
            "Sun, 26 Jul 2026 10:20:00 +0800",
        );
        feed(&format!("{first}{second}"))
    }

    fn parse(
        body: &[u8],
        limit: u32,
    ) -> Result<magic_market_core::DataBatch<magic_market_core::NewsItem>, crate::WallstreetCnError>
    {
        parse_response(body, limit, OBSERVED_AT)
    }

    #[test]
    fn parser_maps_only_strict_metadata() {
        let batch = parse(valid_feed().as_bytes(), 2).unwrap();
        assert_eq!(batch.records().len(), 2);
        let first = &batch.records()[0];
        assert_eq!(first.item_id.as_str(), "3779002");
        assert_eq!(first.title.as_str(), "合成财经标题一");
        assert_eq!(first.publisher.as_str(), "华尔街见闻");
        assert_eq!(
            first.canonical_url.as_str(),
            "https://wallstreetcn.com/articles/3779002"
        );
        assert_eq!(first.published_at.as_str(), "2026-07-26T10:30:00+08:00");
        assert!(first.summary.is_none());
        assert!(first.content.is_none());
        assert!(first.instruments.is_empty());
        assert_eq!(first.topics[0].as_str(), "华尔街见闻");
        assert_eq!(first.language.as_str(), "zh-CN");
        assert_eq!(first.evidence.provider(), ProviderId::WallstreetCn);
        assert_eq!(batch.provenance().source(), "wallstreetcn-rss-v1");
        assert_eq!(
            batch.provenance().batch_id(),
            Some(first.evidence.batch_id())
        );
        let json = serde_json::to_string(&batch).unwrap();
        assert!(!json.contains("NEVER_EXPOSE_DESCRIPTION"));
        assert!(!json.contains("NEVER_EXPOSE_BODY"));
    }

    #[test]
    fn parser_rejects_invalid_document_boundaries() {
        for invalid in [
            Vec::new(),
            b" \n\t".to_vec(),
            vec![0xff],
            b"<rss".to_vec(),
            b"<channel/>".to_vec(),
            b"<rss version=\"2.0\"><channel/><channel/></rss>".to_vec(),
            feed("").into_bytes(),
            b"<!DOCTYPE rss><rss version=\"2.0\"><channel/></rss>".to_vec(),
        ] {
            assert!(parse(&invalid, 1).is_err());
        }
    }

    #[test]
    fn parser_rejects_noncanonical_article_urls() {
        for url in [
            "http://wallstreetcn.com/articles/1",
            "https://www.wallstreetcn.com/articles/1",
            "https://news.wallstreetcn.com/articles/1",
            "https://user@wallstreetcn.com/articles/1",
            "https://wallstreetcn.com:443/articles/1",
            "https://wallstreetcn.com/articles/1?x=1",
            "https://wallstreetcn.com/articles/1#x",
            "https://wallstreetcn.com/articles/1/extra",
            "https://wallstreetcn.com/articles/not-digits",
            "https://wallstreetcn.com/articles/",
            "https://wallstreetcn.com/articles/123456789012345678901",
        ] {
            let row = format!(
                "<item><title>合成标题</title><link>{url}</link><source>华尔街见闻</source><pubDate>Sun, 26 Jul 2026 10:30:00 +0800</pubDate></item>"
            );
            assert!(parse(feed(&row).as_bytes(), 1).is_err(), "{url}");
        }
    }

    #[test]
    fn parser_requires_exact_channel_identity() {
        let row = item("1", "合成标题", "Sun, 26 Jul 2026 10:30:00 +0800");
        for body in [
            feed_with_identity("别的来源", "https://wallstreetcn.com", "zh-hans", &row),
            feed_with_identity("华尔街见闻", "https://wallstreetcn.com/", "zh-hans", &row),
            feed_with_identity("华尔街见闻", "https://wallstreetcn.com", "zh-cn", &row),
            format!(
                "<rss version=\"2.0\"><channel><link>https://wallstreetcn.com</link><language>zh-hans</language>{row}</channel></rss>"
            ),
            format!(
                "<rss version=\"2.0\"><channel><title>华尔街见闻</title><title>华尔街见闻</title><link>https://wallstreetcn.com</link><language>zh-hans</language>{row}</channel></rss>"
            ),
        ] {
            assert!(parse(body.as_bytes(), 1).is_err());
        }
    }

    #[test]
    fn parser_requires_every_exact_item_field() {
        let valid_time = "Sun, 26 Jul 2026 10:30:00 +0800";
        for row in [
            format!(
                "<item><link>https://wallstreetcn.com/articles/1</link><source>华尔街见闻</source><pubDate>{valid_time}</pubDate></item>"
            ),
            format!(
                "<item><title></title><link>https://wallstreetcn.com/articles/1</link><source>华尔街见闻</source><pubDate>{valid_time}</pubDate></item>"
            ),
            format!(
                "<item><title>合成</title><title>重复</title><link>https://wallstreetcn.com/articles/1</link><source>华尔街见闻</source><pubDate>{valid_time}</pubDate></item>"
            ),
            format!(
                "<item><title><b>嵌套</b></title><link>https://wallstreetcn.com/articles/1</link><source>华尔街见闻</source><pubDate>{valid_time}</pubDate></item>"
            ),
            format!(
                "<item><title>合成</title><link>https://wallstreetcn.com/articles/1</link><source>其他来源</source><pubDate>{valid_time}</pubDate></item>"
            ),
            "<item><title>合成</title><link>https://wallstreetcn.com/articles/1</link><source>华尔街见闻</source><pubDate>not-a-date</pubDate></item>".into(),
        ] {
            assert!(parse(feed(&row).as_bytes(), 1).is_err());
        }
    }

    #[test]
    fn parser_rejects_duplicate_and_out_of_order_identities() {
        let first = item("1", "合成标题一", "Sun, 26 Jul 2026 10:30:00 +0800");
        let duplicate = item("1", "合成标题二", "Sun, 26 Jul 2026 10:20:00 +0800");
        assert!(parse(feed(&format!("{first}{duplicate}")).as_bytes(), 2).is_err());

        let leading_zero = item("01", "合成标题二", "Sun, 26 Jul 2026 10:20:00 +0800");
        assert!(parse(feed(&format!("{first}{leading_zero}")).as_bytes(), 2).is_err());

        let older = item("2", "合成标题一", "Sun, 26 Jul 2026 10:20:00 +0800");
        let newer = item("3", "合成标题二", "Sun, 26 Jul 2026 10:30:00 +0800");
        assert!(parse(feed(&format!("{older}{newer}")).as_bytes(), 2).is_err());
    }

    #[test]
    fn parser_rejects_dtd_entities_controls_and_bad_attributes() {
        let valid = item("1", "合成标题", "Sun, 26 Jul 2026 10:30:00 +0800");
        for body in [
            format!("<!DOCTYPE rss>{}", feed(&valid)),
            feed(&valid.replace("合成标题", "合成&custom;标题")),
            feed(&valid.replace("合成标题", "合成&#xZZ;标题")),
            feed(&valid.replace("合成标题", "合成&#1;标题")),
            feed(&valid.replace("合成标题", "合成&#xFFFE;标题")),
            feed(&valid.replace("合成标题", "合成&#xFFFF;标题")),
            feed(&valid.replace("<item>", "<item unexpected=\"x\">")),
            feed(&valid).replace("<rss version=\"2.0\"", "<rss version=\"1.0\""),
            feed(&valid).replace(
                "<rss version=\"2.0\"",
                "<rss version=\"2.0\" unexpected=\"x\"",
            ),
        ] {
            assert!(parse(body.as_bytes(), 1).is_err());
        }
    }

    #[test]
    fn parser_rejects_invalid_xml_inside_ignored_content() {
        let valid = item("1", "合成标题", "Sun, 26 Jul 2026 10:30:00 +0800");
        for body in [
            feed(&valid).replace(
                "<description></description>",
                "<description>忽略\u{1}正文</description>",
            ),
            feed(&valid).replace(
                "<description></description>",
                "<description hidden=\"&#1;\"></description>",
            ),
            feed(&valid).replace(
                "<description></description>",
                "<description>忽略&#xFFFE;正文</description>",
            ),
            feed(&valid).replace(
                "<description></description>",
                "<description>忽略&#xFFFF;正文</description>",
            ),
            feed(&valid).replace(
                "<description></description>",
                "<description><!--invalid--comment--></description>",
            ),
            feed(&valid).replace(
                "<description></description>",
                "<description><?ignored value\u{1}?></description>",
            ),
        ] {
            assert!(parse(body.as_bytes(), 1).is_err());
        }
    }

    #[test]
    fn parser_rejects_invalid_or_duplicate_xml_declarations() {
        let valid = item("1", "合成标题", "Sun, 26 Jul 2026 10:30:00 +0800");
        let document = feed(&valid);
        let rss = document
            .strip_prefix("<?xml version=\"1.0\" encoding=\"utf-8\"?>")
            .unwrap();
        for declaration in [
            "<?xml encoding=\"utf-8\"?>",
            "<?xml version=\"1.1\" encoding=\"utf-8\"?>",
            "<?xml version=\"1.0\" version=\"1.0\"?>",
            "<?xml version=\"1.0\" unexpected=\"x\"?>",
            "<?xml version=\"1.0\" standalone=\"maybe\"?>",
            "<?xml encoding=\"utf-8\" version=\"1.0\"?>",
            "<?xml version=\"1.0\" encoding=\"UTF&#45;8\"?>",
            "<?xml version=\"1.0\" standalone=\"y&#101;s\"?>",
        ] {
            let body = format!("{declaration}{rss}");
            assert!(parse(body.as_bytes(), 1).is_err(), "{declaration}");
        }

        let duplicate =
            format!("<?xml version=\"1.0\"?><?xml version=\"1.0\" encoding=\"utf-8\"?>{rss}");
        assert!(parse(duplicate.as_bytes(), 1).is_err());

        for declaration in [
            "",
            "<?xml version='1.0'?>",
            "<?xml version='1.0' encoding='UTF-8'?>",
            "<?xml version='1.0' standalone='yes'?>",
            "<?xml version='1.0' encoding='utf-8' standalone='no'?>",
        ] {
            let body = format!("{declaration}{rss}");
            assert!(parse(body.as_bytes(), 1).is_ok(), "{declaration}");
        }
    }

    #[test]
    fn parser_ignores_extension_subtrees_without_interpreting_structure_or_text() {
        let row = "<item>\
               <title>合成标题</title>\
               <link>https://wallstreetcn.com/articles/1</link>\
               <description><item><title>NEVER_EXPOSE_DESCRIPTION</title></item></description>\
               <media:group xmlns:media=\"urn:synthetic-media\"><channel><source>NEVER_EXPOSE_BODY</source></channel></media:group>\
               <source>华尔街见闻</source>\
               <pubDate>Sun, 26 Jul 2026 10:30:00 +0800</pubDate>\
             </item>";
        let batch = parse(feed(row).as_bytes(), 1).unwrap();
        let serialized = serde_json::to_string(&batch).unwrap();
        assert!(!serialized.contains("NEVER_EXPOSE_DESCRIPTION"));
        assert!(!serialized.contains("NEVER_EXPOSE_BODY"));
    }

    #[test]
    fn parser_enforces_source_bound_before_caller_truncation() {
        let mut one_hundred = String::new();
        for id in 1..=100 {
            one_hundred.push_str(&item(
                &id.to_string(),
                "合成标题",
                "Sun, 26 Jul 2026 10:30:00 +0800",
            ));
        }
        assert_eq!(
            parse(feed(&one_hundred).as_bytes(), 50)
                .unwrap()
                .records()
                .len(),
            50
        );

        let one_hundred_one = format!(
            "{}{}",
            one_hundred,
            item("101", "合成标题", "Sun, 26 Jul 2026 10:30:00 +0800")
        );
        assert!(parse(feed(&one_hundred_one).as_bytes(), 50).is_err());

        let mut first_fifty = String::new();
        for id in 1..=50 {
            first_fifty.push_str(&item(
                &id.to_string(),
                "合成标题",
                "Sun, 26 Jul 2026 10:30:00 +0800",
            ));
        }
        let malformed_fifty_first = format!(
            "{first_fifty}<item><title>坏行</title><link>https://wallstreetcn.com/articles/51</link><pubDate>Sun, 26 Jul 2026 10:30:00 +0800</pubDate></item>"
        );
        assert!(parse(feed(&malformed_fifty_first).as_bytes(), 50).is_err());
    }
}
