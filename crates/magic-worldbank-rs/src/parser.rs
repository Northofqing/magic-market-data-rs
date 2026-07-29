use crate::WorldBankError;
use magic_market_core::{
    DataBatch, EconomicObservation, EconomicObservationStatus, EconomicPeriod, EconomicSeriesKey,
    FiniteNumber, NonEmptyText, Provenance, ProviderId, SourceEvidence,
};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldBankNamespace {
    source_id: String,
    economy: String,
}

impl WorldBankNamespace {
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    pub fn economy(&self) -> &str {
        &self.economy
    }
}

#[derive(Debug, Clone)]
pub struct WorldBankParseContext<'a> {
    pub key: &'a EconomicSeriesKey,
    pub start_year: u32,
    pub end_year: u32,
    pub observed_at: &'a str,
    pub batch_id: &'a str,
}

struct IndicatorMetadata {
    name: String,
    unit: String,
    source_id: String,
}

struct PaginationMetadata {
    page: usize,
    pages: usize,
    per_page: usize,
    total: usize,
}

struct PageMetadata {
    page: usize,
    pages: usize,
    per_page: usize,
    total: usize,
    source_id: String,
    last_updated: String,
}

pub fn parse_world_bank_namespace(value: &str) -> Result<WorldBankNamespace, WorldBankError> {
    let mut parts = value.split('/');
    let source = parts.next().unwrap_or_default();
    let country = parts.next().unwrap_or_default();
    if parts.next().is_some() || !source.starts_with("source:") || !country.starts_with("country:")
    {
        return Err(WorldBankError::InvalidRequest(
            "World Bank namespace must be source:SOURCE_ID/country:AREA_CODE".into(),
        ));
    }
    let source_id = &source["source:".len()..];
    let economy = &country["country:".len()..];
    if !valid_source_id(source_id) || !valid_code(economy, 3, 32) {
        return Err(WorldBankError::InvalidRequest(
            "World Bank namespace contains unsafe identifiers".into(),
        ));
    }
    Ok(WorldBankNamespace {
        source_id: source_id.to_owned(),
        economy: economy.to_owned(),
    })
}

pub fn parse_world_bank_responses(
    indicator_body: &[u8],
    data_pages: &[&[u8]],
    context: &WorldBankParseContext<'_>,
) -> Result<DataBatch<EconomicObservation>, WorldBankError> {
    super::transport::ensure_no_duplicate_json_keys(indicator_body)
        .map_err(|error| WorldBankError::Decode(error.to_string()))?;
    if context.key.provider() != ProviderId::WorldBank
        || !valid_code(context.key.code(), 1, 64)
        || context.start_year > context.end_year
    {
        return Err(WorldBankError::InvalidRequest(
            "invalid World Bank series request".into(),
        ));
    }
    let namespace = parse_world_bank_namespace(context.key.namespace())?;
    let indicator = parse_indicator(indicator_body, context.key.code(), namespace.source_id())?;
    if indicator.source_id != namespace.source_id {
        return Err(WorldBankError::Protocol(
            "indicator source ID does not match namespace".into(),
        ));
    }
    if indicator.unit.trim().is_empty() {
        return Err(WorldBankError::Protocol(
            "official structured indicator unit is empty; unit inference is forbidden".into(),
        ));
    }
    if data_pages.is_empty() {
        return Err(WorldBankError::Protocol(
            "World Bank response contains no data pages".into(),
        ));
    }

    let mut stable: Option<PageMetadata> = None;
    let mut rows = Vec::new();
    let mut seen_periods = HashMap::new();
    let mut seen_pages = HashSet::new();
    let mut stable_economy: Option<(String, String, String)> = None;
    for body in data_pages {
        super::transport::ensure_no_duplicate_json_keys(body)
            .map_err(|error| WorldBankError::Decode(error.to_string()))?;
        let value: Value = serde_json::from_slice(body)
            .map_err(|error| WorldBankError::Decode(error.to_string()))?;
        let (metadata_value, data_value) = two_element_page(&value)?;
        let metadata = parse_page_metadata(metadata_value)?;
        if metadata.pages == 0
            || metadata.pages > 100
            || metadata.page == 0
            || metadata.page > metadata.pages
            || metadata.per_page == 0
            || metadata.per_page > 1_000
            || metadata.total > 10_000
            || metadata.source_id != namespace.source_id
            || EconomicPeriod::day(&metadata.last_updated).is_err()
        {
            return Err(WorldBankError::Protocol(
                "World Bank page metadata exceeds or violates bounds".into(),
            ));
        }
        if !seen_pages.insert(metadata.page) {
            return Err(WorldBankError::Protocol(
                "World Bank response contains a duplicate page".into(),
            ));
        }
        if let Some(first) = &stable {
            if metadata.pages != first.pages
                || metadata.per_page != first.per_page
                || metadata.total != first.total
                || metadata.source_id != first.source_id
                || metadata.last_updated != first.last_updated
            {
                return Err(WorldBankError::Protocol(
                    "World Bank pagination metadata changed between pages".into(),
                ));
            }
        } else {
            stable = Some(PageMetadata {
                page: metadata.page,
                pages: metadata.pages,
                per_page: metadata.per_page,
                total: metadata.total,
                source_id: metadata.source_id.clone(),
                last_updated: metadata.last_updated.clone(),
            });
        }
        let data = data_value.as_array().ok_or_else(|| {
            WorldBankError::Protocol("World Bank data page rows must be an array".into())
        })?;
        if data.len() > metadata.per_page {
            return Err(WorldBankError::Protocol(
                "World Bank page row count exceeds per_page".into(),
            ));
        }
        for raw in data {
            let object = raw.as_object().ok_or_else(|| {
                WorldBankError::Protocol("World Bank row must be an object".into())
            })?;
            let indicator_id = nested_string(object, "indicator", "id")?;
            let indicator_name = nested_string(object, "indicator", "value")?;
            let country_id = nested_string(object, "country", "id")?;
            let country_name = nested_string(object, "country", "value")?;
            let iso3 = string_field(object, "countryiso3code")?;
            let year = parse_year(string_field(object, "date")?)?;
            if indicator_id != context.key.code()
                || indicator_name != indicator.name
                || !economy_matches(&namespace.economy, country_id, iso3)
            {
                return Err(WorldBankError::Protocol(
                    "World Bank row identity does not match request".into(),
                ));
            }
            let economy_identity = (
                country_id.to_owned(),
                iso3.to_owned(),
                country_name.to_owned(),
            );
            if let Some(first) = &stable_economy {
                if first != &economy_identity {
                    return Err(WorldBankError::Protocol(
                        "World Bank economy identity changed across pages".into(),
                    ));
                }
            } else {
                stable_economy = Some(economy_identity);
            }
            if !(context.start_year..=context.end_year).contains(&year) {
                return Err(WorldBankError::Protocol(
                    "World Bank row period is outside the requested range".into(),
                ));
            }
            let value = match object.get("value") {
                Some(Value::Null) => None,
                Some(Value::Number(number)) => {
                    Some(FiniteNumber::new(number.as_f64().ok_or_else(|| {
                        WorldBankError::Protocol("non-finite World Bank value".into())
                    })?)?)
                }
                _ => {
                    return Err(WorldBankError::Protocol(
                        "World Bank row value must be numeric or null".into(),
                    ))
                }
            };
            if let Some(previous) = seen_periods.insert(year, value) {
                if previous != value {
                    return Err(WorldBankError::Protocol(
                        "conflicting duplicate World Bank period".into(),
                    ));
                }
                return Err(WorldBankError::Protocol(
                    "duplicate World Bank period".into(),
                ));
            }
            rows.push((year, value, iso3.to_owned(), country_name.to_owned()));
        }
    }
    let stable = stable.ok_or_else(|| {
        WorldBankError::Protocol("World Bank response contains no page metadata".into())
    })?;
    if data_pages.len() != stable.pages
        || seen_periods.len() != stable.total
        || !(1..=stable.pages).all(|page| seen_pages.contains(&page))
    {
        return Err(WorldBankError::Protocol(
            "World Bank pages do not cover the declared total".into(),
        ));
    }
    rows.sort_by_key(|(year, _, _, _)| *year);
    let mut records = Vec::with_capacity(rows.len());
    for (year, value, iso3, country_name) in rows {
        let evidence =
            SourceEvidence::new(ProviderId::WorldBank, context.observed_at, context.batch_id)?
                .with_source_at(stable.last_updated.clone())?;
        records.push(EconomicObservation::new(
            context.key.clone(),
            indicator.name.clone(),
            Some(NonEmptyText::new(iso3)?),
            Some(NonEmptyText::new(country_name)?),
            EconomicPeriod::year(year)?,
            value,
            indicator.unit.clone(),
            None,
            None,
            if value.is_some() {
                EconomicObservationStatus::Present
            } else {
                EconomicObservationStatus::Missing
            },
            Some(NonEmptyText::new(stable.last_updated.clone())?),
            None,
            evidence,
        )?);
    }
    let provenance = Provenance::new("World Bank Indicators v2", context.observed_at)?
        .with_batch_id(context.batch_id)?
        .with_source_at(stable.last_updated)?;
    Ok(DataBatch::strict(records, provenance))
}

fn parse_indicator(
    body: &[u8],
    requested: &str,
    expected_source_id: &str,
) -> Result<IndicatorMetadata, WorldBankError> {
    let value: Value =
        serde_json::from_slice(body).map_err(|error| WorldBankError::Decode(error.to_string()))?;
    let (page, rows) = two_element_page(&value)?;
    let page = parse_pagination_metadata(page)?;
    if page.page != 1
        || page.pages != 1
        || page.per_page == 0
        || page.per_page > 1_000
        || page.total != 1
    {
        return Err(WorldBankError::Protocol(
            "indicator metadata page is inconsistent".into(),
        ));
    }
    let rows = rows.as_array().ok_or_else(|| {
        WorldBankError::Protocol("indicator metadata rows must be an array".into())
    })?;
    if rows.len() != 1 {
        return Err(WorldBankError::Protocol(
            "indicator metadata must contain exactly one row".into(),
        ));
    }
    let row = rows[0].as_object().ok_or_else(|| {
        WorldBankError::Protocol("indicator metadata row must be an object".into())
    })?;
    if string_field(row, "id")? != requested {
        return Err(WorldBankError::Protocol(
            "indicator metadata identity mismatch".into(),
        ));
    }
    let source_id = nested_string(row, "source", "id")?;
    if source_id != expected_source_id {
        return Err(WorldBankError::Protocol(
            "indicator source ID does not match namespace".into(),
        ));
    }
    Ok(IndicatorMetadata {
        name: string_field(row, "name")?.to_owned(),
        unit: string_field_allow_empty(row, "unit")?.to_owned(),
        source_id: source_id.to_owned(),
    })
}

fn parse_page_metadata(value: &Value) -> Result<PageMetadata, WorldBankError> {
    let pagination = parse_pagination_metadata(value)?;
    let object = value.as_object().ok_or_else(|| {
        WorldBankError::Protocol("World Bank page metadata must be an object".into())
    })?;
    Ok(PageMetadata {
        page: pagination.page,
        pages: pagination.pages,
        per_page: pagination.per_page,
        total: pagination.total,
        source_id: string_or_number_field(object, "sourceid")?,
        last_updated: string_field(object, "lastupdated")?.to_owned(),
    })
}

fn parse_pagination_metadata(value: &Value) -> Result<PaginationMetadata, WorldBankError> {
    let object = value.as_object().ok_or_else(|| {
        WorldBankError::Protocol("World Bank page metadata must be an object".into())
    })?;
    Ok(PaginationMetadata {
        page: usize_field(object, "page")?,
        pages: usize_field(object, "pages")?,
        per_page: usize_field(object, "per_page")?,
        total: usize_field(object, "total")?,
    })
}

fn two_element_page(value: &Value) -> Result<(&Value, &Value), WorldBankError> {
    let array = value.as_array().ok_or_else(|| {
        WorldBankError::Protocol("World Bank response must be a two-element array".into())
    })?;
    if array.len() != 2 {
        return Err(WorldBankError::Protocol(
            "World Bank response must contain metadata and rows".into(),
        ));
    }
    Ok((&array[0], &array[1]))
}

fn usize_field(object: &Map<String, Value>, key: &str) -> Result<usize, WorldBankError> {
    match object.get(key) {
        Some(Value::Number(value)) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| WorldBankError::Protocol(format!("invalid {key} field"))),
        Some(Value::String(value)) => value
            .parse()
            .map_err(|_| WorldBankError::Protocol(format!("invalid {key} field"))),
        _ => Err(WorldBankError::Protocol(format!("missing {key} field"))),
    }
}

fn string_or_number_field(
    object: &Map<String, Value>,
    key: &str,
) -> Result<String, WorldBankError> {
    match object.get(key) {
        Some(Value::String(value)) if !value.is_empty() => Ok(value.clone()),
        Some(Value::Number(value)) => Ok(value.to_string()),
        _ => Err(WorldBankError::Protocol(format!("invalid {key} field"))),
    }
}

fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, WorldBankError> {
    let value = string_field_allow_empty(object, key)?;
    if value.is_empty() {
        return Err(WorldBankError::Protocol(format!("empty {key} field")));
    }
    Ok(value)
}

fn string_field_allow_empty<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, WorldBankError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| WorldBankError::Protocol(format!("invalid {key} field")))
}

fn nested_string<'a>(
    object: &'a Map<String, Value>,
    outer: &str,
    inner: &str,
) -> Result<&'a str, WorldBankError> {
    let nested = object
        .get(outer)
        .and_then(Value::as_object)
        .ok_or_else(|| WorldBankError::Protocol(format!("invalid {outer} object")))?;
    string_field(nested, inner)
}

fn economy_matches(requested: &str, source_id: &str, iso3: &str) -> bool {
    requested == source_id || requested == iso3
}

fn valid_source_id(value: &str) -> bool {
    (1..=10).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_code(value: &str, minimum: usize, maximum: usize) -> bool {
    (minimum..=maximum).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn parse_year(value: &str) -> Result<u32, WorldBankError> {
    if value.len() != 4 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(WorldBankError::Protocol("invalid World Bank year".into()));
    }
    let year = value
        .parse()
        .map_err(|_| WorldBankError::Protocol("invalid World Bank year".into()))?;
    if !(1900..=9999).contains(&year) {
        return Err(WorldBankError::Protocol(
            "World Bank year out of range".into(),
        ));
    }
    Ok(year)
}
