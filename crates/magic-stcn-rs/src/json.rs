use crate::StcnError;
use magic_market_core::{
    DataBatch, HttpsUrl, NewsItem, NonEmptyText, Provenance, ProviderId, SourceEvidence,
};
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashSet;
use std::fmt;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_SOURCE_ROWS: usize = 30;

#[derive(Deserialize)]
struct Envelope {
    state: i64,
    data: QuickNewsData,
    #[serde(default)]
    page_time: CursorField<u32>,
    #[serde(default)]
    last_time: CursorField<i64>,
}

enum QuickNewsData {
    Rows(Vec<Row>),
    TerminalEmpty,
}

impl<'de> Deserialize<'de> for QuickNewsData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct QuickNewsDataVisitor;

        impl<'de> Visitor<'de> for QuickNewsDataVisitor {
            type Value = QuickNewsData;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded quick-news row array or the empty terminal string")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                if sequence
                    .size_hint()
                    .is_some_and(|size| size > MAX_SOURCE_ROWS)
                {
                    return Err(de::Error::custom(
                        "quick-news data exceeds the 30-row source ceiling",
                    ));
                }
                let mut rows = Vec::with_capacity(
                    sequence
                        .size_hint()
                        .unwrap_or_default()
                        .min(MAX_SOURCE_ROWS),
                );
                while let Some(row) = sequence.next_element::<Row>()? {
                    if rows.len() == MAX_SOURCE_ROWS {
                        return Err(de::Error::custom(
                            "quick-news data exceeds the 30-row source ceiling",
                        ));
                    }
                    rows.push(row);
                }
                Ok(QuickNewsData::Rows(rows))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.is_empty() {
                    Ok(QuickNewsData::TerminalEmpty)
                } else {
                    Err(E::custom("quick-news terminal data string must be empty"))
                }
            }
        }

        deserializer.deserialize_any(QuickNewsDataVisitor)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
enum CursorField<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<'de, T> Deserialize<'de> for CursorField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<T>::deserialize(deserializer)?.map_or(Self::Null, Self::Value))
    }
}

#[derive(Deserialize)]
struct Row {
    id: String,
    url: String,
    web_url: String,
    title: String,
    source: String,
    time: i64,
    show_time: String,
    #[serde(rename = "pageTime")]
    page_time: String,
}

struct ParsedRow {
    id: String,
    title: String,
    publisher: String,
    canonical_url: String,
    published_at: String,
    epoch_millis: i64,
    epoch_seconds: i64,
}

/// Parses the complete quick-news envelope before applying `limit`.
pub fn parse_quick_news(body: &[u8], limit: u32) -> Result<DataBatch<NewsItem>, StcnError> {
    if body.len() > MAX_JSON_BYTES {
        return Err(protocol("JSON exceeds the 2 MiB source ceiling"));
    }
    if !(1..=30).contains(&limit) {
        return Err(StcnError::InvalidRequest(
            "Securities Times parser limit must be between 1 and 30".into(),
        ));
    }
    let envelope: Envelope = serde_json::from_slice(body)
        .map_err(|_| StcnError::Decode("quick-news JSON is invalid".into()))?;
    let Envelope {
        state,
        data,
        page_time,
        last_time,
    } = envelope;
    if state != 1 {
        return Err(protocol("quick-news state must equal 1"));
    }
    let rows = match data {
        QuickNewsData::TerminalEmpty => {
            if page_time != CursorField::Null || last_time != CursorField::Null {
                return Err(protocol(
                    "terminal envelope cursors must be present explicit nulls",
                ));
            }
            return Err(protocol(
                "initial quick-news empty data cannot prove a verified-empty result",
            ));
        }
        QuickNewsData::Rows(rows) => rows,
    };
    if rows.is_empty() {
        return Err(protocol(
            "non-terminal quick-news data must contain between 1 and 30 rows",
        ));
    }
    let page_time = match page_time {
        CursorField::Value(value) => value,
        CursorField::Missing | CursorField::Null => {
            return Err(protocol("non-terminal envelope page_time is required"));
        }
    };
    if page_time != 2 {
        return Err(protocol("initial envelope page_time must equal 2"));
    }
    let last_time = match last_time {
        CursorField::Value(value) => value,
        CursorField::Missing | CursorField::Null => {
            return Err(protocol("non-terminal envelope last_time is required"));
        }
    };
    if rows.len() > MAX_SOURCE_ROWS {
        return Err(protocol(
            "non-terminal quick-news data must contain between 1 and 30 rows",
        ));
    }

    let mut parsed = Vec::with_capacity(rows.len());
    let mut ids = HashSet::with_capacity(rows.len());
    let mut urls = HashSet::with_capacity(rows.len());
    let mut previous_time = None;
    for row in rows {
        let value = parse_row(row)?;
        if !ids.insert(value.id.clone()) || !urls.insert(value.canonical_url.clone()) {
            return Err(protocol("duplicate item ID or canonical URL"));
        }
        if previous_time.is_some_and(|previous| value.epoch_millis > previous) {
            return Err(protocol("source rows are not in non-increasing time order"));
        }
        previous_time = Some(value.epoch_millis);
        parsed.push(value);
    }
    if parsed.last().map(|row| row.epoch_seconds) != Some(last_time) {
        return Err(protocol("last_time does not match the final source row"));
    }

    let observed_at = now()?;
    let batch_id = format!("securities-times:{observed_at}");
    let returned = parsed.into_iter().take(limit as usize).collect::<Vec<_>>();
    let source_at = returned
        .last()
        .ok_or_else(|| protocol("quick-news data is empty"))?
        .published_at
        .clone();
    let mut records = Vec::with_capacity(returned.len());
    for row in returned {
        let evidence = SourceEvidence::new(
            ProviderId::SecuritiesTimes,
            observed_at.clone(),
            batch_id.clone(),
        )?
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
    let provenance = Provenance::new("securities-times", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn parse_row(row: Row) -> Result<ParsedRow, StcnError> {
    checked_text(&row.id, "id")?;
    if !row.id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(protocol("row ID must contain only ASCII digits"));
    }
    checked_text(&row.title, "title")?;
    let publisher = if row.source.is_empty() {
        "证券时报".to_owned()
    } else {
        checked_text(&row.source, "source")?;
        row.source
    };
    if row.page_time != row.id {
        return Err(protocol("row pageTime must exactly equal its row ID"));
    }
    let show_time = parse_epoch_string(&row.show_time)?;
    if row.time.div_euclid(1000) != show_time {
        return Err(protocol("row millisecond and second timestamps differ"));
    }
    let relative = format!("/article/detail/{}.html", row.id);
    let absolute = format!("https://www.stcn.com{relative}");
    if row.url != relative || row.web_url != relative {
        return Err(protocol("row canonical URLs do not exactly match its ID"));
    }
    let published_at = format_epoch(show_time)?;
    Ok(ParsedRow {
        id: row.id,
        title: row.title,
        publisher,
        canonical_url: absolute,
        published_at,
        epoch_millis: row.time,
        epoch_seconds: show_time,
    })
}

fn parse_epoch_string(value: &str) -> Result<i64, StcnError> {
    checked_text(value, "show_time")?;
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(protocol("show_time must contain only ASCII digits"));
    }
    let parsed = value
        .parse::<i64>()
        .map_err(|_| protocol("show_time is outside the supported integer range"))?;
    if parsed <= 0 || parsed.to_string() != value {
        return Err(protocol("show_time must be a canonical positive integer"));
    }
    Ok(parsed)
}

fn format_epoch(epoch: i64) -> Result<String, StcnError> {
    let offset = UtcOffset::from_hms(8, 0, 0).map_err(|_| protocol("Beijing offset is invalid"))?;
    OffsetDateTime::from_unix_timestamp(epoch)
        .map_err(|_| protocol("show_time is outside the supported timestamp range"))?
        .to_offset(offset)
        .format(&Rfc3339)
        .map_err(|_| protocol("show_time cannot be normalized"))
}

fn checked_text(value: &str, field: &str) -> Result<(), StcnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) || trimmed != value {
        return Err(protocol(&format!("{field} is empty, padded, or unsafe")));
    }
    Ok(())
}

fn now() -> Result<String, StcnError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| protocol("observation time cannot be formatted"))
}

fn protocol(message: &str) -> StcnError {
    StcnError::Protocol(message.into())
}
