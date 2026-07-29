use crate::ImfError;
use magic_market_core::{
    DataBatch, EconomicObservation, EconomicObservationStatus, EconomicPeriod, EconomicRevision,
    EconomicRevisionKind, EconomicSeriesKey, FiniteNumber, NonEmptyText, Provenance, ProviderId,
    SourceEvidence,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

const MAX_DECODED_CELLS: usize = 20_000;

#[derive(Debug, Clone)]
pub struct ImfParseContext<'a> {
    pub key: &'a EconomicSeriesKey,
    pub start_year: u32,
    pub end_year: u32,
    pub observed_at: &'a str,
    pub batch_id: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImfNamespace {
    dataset: String,
    area: String,
}

impl ImfNamespace {
    pub fn dataset(&self) -> &str {
        &self.dataset
    }

    pub fn area(&self) -> &str {
        &self.area
    }
}

#[derive(Deserialize)]
struct CatalogEnvelope {
    indicators: HashMap<String, CatalogIndicator>,
}

#[derive(Deserialize)]
struct CatalogIndicator {
    label: String,
    source: String,
    unit: String,
    dataset: String,
    #[serde(rename = "projection-year")]
    projection_year: Option<u32>,
    #[serde(rename = "last-modified")]
    last_modified: Option<String>,
}

pub fn parse_namespace(value: &str) -> Result<ImfNamespace, ImfError> {
    let mut parts = value.split('/');
    let dataset = parts.next().unwrap_or_default();
    let area = parts.next().unwrap_or_default();
    if parts.next().is_some() || !valid_component(dataset) || !valid_component(area) {
        return Err(ImfError::InvalidRequest(
            "IMF namespace must be DATASET/AREA".into(),
        ));
    }
    Ok(ImfNamespace {
        dataset: dataset.to_owned(),
        area: area.to_owned(),
    })
}

pub fn parse_imf_responses(
    catalog_body: &[u8],
    series_body: &[u8],
    context: &ImfParseContext<'_>,
) -> Result<DataBatch<EconomicObservation>, ImfError> {
    super::transport::ensure_no_duplicate_json_keys(catalog_body)
        .map_err(|error| ImfError::Decode(error.to_string()))?;
    super::transport::ensure_no_duplicate_json_keys(series_body)
        .map_err(|error| ImfError::Decode(error.to_string()))?;
    if context.key.provider() != ProviderId::Imf || !valid_component(context.key.code()) {
        return Err(ImfError::InvalidRequest(
            "invalid provider-native IMF series key".into(),
        ));
    }
    if context.start_year > context.end_year || context.end_year - context.start_year >= 50 {
        return Err(ImfError::InvalidRequest(
            "IMF requests accept at most 50 annual periods".into(),
        ));
    }
    let namespace = parse_namespace(context.key.namespace())?;
    let catalog: CatalogEnvelope = serde_json::from_slice(catalog_body)
        .map_err(|error| ImfError::Decode(error.to_string()))?;
    let metadata = catalog
        .indicators
        .get(context.key.code())
        .ok_or_else(|| ImfError::Protocol("requested indicator missing from catalog".into()))?;
    if metadata.dataset != namespace.dataset {
        return Err(ImfError::Protocol(
            "catalog dataset does not match requested namespace".into(),
        ));
    }
    if metadata.label.trim().is_empty()
        || metadata.unit.trim().is_empty()
        || metadata.source.trim().is_empty()
    {
        return Err(ImfError::Protocol(
            "catalog label, unit and source are mandatory".into(),
        ));
    }

    let root: Value =
        serde_json::from_slice(series_body).map_err(|error| ImfError::Decode(error.to_string()))?;
    validate_api(&root)?;
    let response_indicators = object_at(&root, "indicators")?;
    if !response_indicators.contains_key(context.key.code()) {
        return Err(ImfError::Protocol(
            "requested indicator missing from response metadata".into(),
        ));
    }
    for (code, value) in response_indicators {
        let object = value.as_object().ok_or_else(|| {
            ImfError::Protocol("response indicator metadata must be an object".into())
        })?;
        let label = object.get("label").and_then(Value::as_str).ok_or_else(|| {
            ImfError::Protocol("response indicator metadata requires a label".into())
        })?;
        if !valid_component(code) || label.trim().is_empty() {
            return Err(ImfError::Protocol(
                "response indicator metadata shape is invalid".into(),
            ));
        }
        if code == context.key.code() && label != metadata.label {
            return Err(ImfError::Protocol(
                "response indicator label conflicts with catalog metadata".into(),
            ));
        }
    }
    let values = object_at(&root, "values")?;
    if response_indicators.len() != values.len()
        || response_indicators
            .keys()
            .any(|indicator| !values.contains_key(indicator))
    {
        return Err(ImfError::Protocol(
            "indicator metadata and values identities differ".into(),
        ));
    }
    let mut decoded_cells = 0usize;
    let mut sentinel_count = 0usize;
    let mut selected = Vec::new();
    let mut requested_area_seen = false;
    for (indicator, areas_value) in values {
        if !valid_component(indicator) {
            return Err(ImfError::Protocol(
                "response contains invalid indicator identity".into(),
            ));
        }
        if !response_indicators.contains_key(indicator) {
            return Err(ImfError::Protocol(
                "values contain an indicator absent from response metadata".into(),
            ));
        }
        let areas = areas_value.as_object().ok_or_else(|| {
            ImfError::Protocol("indicator values must contain an area object".into())
        })?;
        for (area, years_value) in areas {
            if area.is_empty() {
                if years_value.is_null() {
                    sentinel_count = sentinel_count
                        .checked_add(1)
                        .ok_or_else(|| ImfError::Protocol("sentinel counter overflow".into()))?;
                    continue;
                }
                return Err(ImfError::Protocol(
                    "empty IMF area key is allowed only for the null sentinel".into(),
                ));
            }
            if !valid_component(area) {
                return Err(ImfError::Protocol(
                    "response contains invalid area identity".into(),
                ));
            }
            let years = years_value.as_object().ok_or_else(|| {
                ImfError::Protocol("non-sentinel area values must be objects".into())
            })?;
            for (year_text, value) in years {
                decoded_cells = decoded_cells
                    .checked_add(1)
                    .ok_or_else(|| ImfError::Protocol("decoded cell counter overflow".into()))?;
                if decoded_cells > MAX_DECODED_CELLS {
                    return Err(ImfError::Protocol(
                        "response exceeds 20000 decoded area-year cells".into(),
                    ));
                }
                let year = parse_year(year_text)?;
                let number = value.as_f64().ok_or_else(|| {
                    ImfError::Protocol("IMF area-year value must be numeric".into())
                })?;
                let finite = FiniteNumber::new(number)?;
                if indicator == context.key.code() && area.as_str() == namespace.area() {
                    requested_area_seen = true;
                    if (context.start_year..=context.end_year).contains(&year) {
                        selected.push((year, finite));
                    }
                }
            }
        }
    }
    if sentinel_count != 1 {
        return Err(ImfError::Protocol(
            "IMF response requires exactly one empty-key/null sentinel".into(),
        ));
    }
    if !requested_area_seen {
        return Err(ImfError::Protocol(
            "requested area is absent from response".into(),
        ));
    }
    selected.sort_by_key(|(year, _)| *year);
    let mut seen = HashSet::with_capacity(selected.len());
    if selected.iter().any(|(year, _)| !seen.insert(*year)) {
        return Err(ImfError::Protocol(
            "duplicate requested area-year observation".into(),
        ));
    }

    let mut records = Vec::with_capacity(selected.len());
    for (year, value) in selected {
        let evidence = evidence(context)?;
        let revision_kind = if metadata
            .projection_year
            .is_some_and(|projection_year| year >= projection_year)
        {
            "IMF projection"
        } else {
            "IMF catalog revision"
        };
        let revision_label = match &metadata.last_modified {
            Some(last_modified) if !last_modified.trim().is_empty() => {
                format!("{}; last-modified={last_modified}", metadata.source)
            }
            _ => metadata.source.clone(),
        };
        let revision = Some(EconomicRevision {
            kind: EconomicRevisionKind::SourceDefined(NonEmptyText::new(revision_kind)?),
            label: Some(NonEmptyText::new(revision_label)?),
        });
        records.push(EconomicObservation::new(
            context.key.clone(),
            metadata.label.clone(),
            Some(NonEmptyText::new(namespace.area.clone())?),
            None,
            EconomicPeriod::year(year)?,
            Some(value),
            metadata.unit.clone(),
            None,
            None,
            EconomicObservationStatus::Present,
            None,
            revision,
            evidence,
        )?);
    }
    let provenance = Provenance::new("IMF DataMapper v2", context.observed_at)?
        .with_batch_id(context.batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn evidence(context: &ImfParseContext<'_>) -> Result<SourceEvidence, ImfError> {
    Ok(SourceEvidence::new(
        ProviderId::Imf,
        context.observed_at,
        context.batch_id,
    )?)
}

fn validate_api(root: &Value) -> Result<(), ImfError> {
    let api = object_at(root, "api")?;
    if api.get("version").and_then(Value::as_str) != Some("2")
        || api.get("output-method").and_then(Value::as_str) != Some("json")
    {
        return Err(ImfError::Protocol(
            "IMF API version or output method is not exact".into(),
        ));
    }
    Ok(())
}

fn object_at<'a>(root: &'a Value, key: &str) -> Result<&'a Map<String, Value>, ImfError> {
    root.get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| ImfError::Protocol(format!("IMF response missing {key} object")))
}

fn valid_component(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn parse_year(value: &str) -> Result<u32, ImfError> {
    if value.len() != 4 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ImfError::Protocol("invalid IMF year key".into()));
    }
    let year = value
        .parse()
        .map_err(|_| ImfError::Protocol("invalid IMF year key".into()))?;
    if !(1900..=9999).contains(&year) {
        return Err(ImfError::Protocol("IMF year is out of range".into()));
    }
    Ok(year)
}

#[cfg(test)]
mod tests;
