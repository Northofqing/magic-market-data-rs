use crate::FredError;
use magic_market_core::{
    DataBatch, EconomicFrequency, EconomicObservation, EconomicObservationStatus, EconomicPeriod,
    EconomicRevision, EconomicRevisionKind, EconomicSeriesKey, FiniteNumber, NonEmptyText,
    Provenance, ProviderId, SourceEvidence,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct FredParseContext<'a> {
    pub key: &'a EconomicSeriesKey,
    pub frequency: EconomicFrequency,
    pub start: &'a EconomicPeriod,
    pub end: &'a EconomicPeriod,
    pub query_start: &'a str,
    pub query_end: &'a str,
    pub observed_at: &'a str,
    pub batch_id: &'a str,
}

#[derive(Deserialize)]
struct SeriesEnvelope {
    seriess: Vec<SeriesMetadata>,
}

#[derive(Deserialize)]
struct SeriesMetadata {
    id: String,
    title: String,
    observation_start: String,
    observation_end: String,
    frequency: String,
    frequency_short: String,
    units: String,
    seasonal_adjustment: String,
    last_updated: String,
}

#[derive(Deserialize)]
struct ObservationsEnvelope {
    realtime_start: String,
    realtime_end: String,
    observation_start: String,
    observation_end: String,
    units: String,
    output_type: u8,
    file_type: String,
    order_by: String,
    count: usize,
    offset: usize,
    limit: usize,
    sort_order: String,
    observations: Vec<RawObservation>,
}

#[derive(Deserialize)]
struct RawObservation {
    realtime_start: String,
    realtime_end: String,
    date: String,
    value: String,
}

pub fn parse_fred_responses(
    metadata_body: &[u8],
    observations_body: &[u8],
    context: &FredParseContext<'_>,
) -> Result<DataBatch<EconomicObservation>, FredError> {
    ensure_no_duplicate_keys(metadata_body)?;
    ensure_no_duplicate_keys(observations_body)?;
    reject_api_error(metadata_body)?;
    reject_api_error(observations_body)?;
    if context.key.provider() != ProviderId::Fred || context.key.namespace() != "fred" {
        return Err(FredError::InvalidRequest(
            "FRED key namespace must be exactly fred".into(),
        ));
    }

    let metadata: SeriesEnvelope = serde_json::from_slice(metadata_body)
        .map_err(|error| FredError::Decode(error.to_string()))?;
    if metadata.seriess.len() != 1 {
        return Err(FredError::Protocol(
            "series metadata must contain exactly one row".into(),
        ));
    }
    let metadata = &metadata.seriess[0];
    if metadata.id != context.key.code() {
        return Err(FredError::Protocol(
            "series metadata identity does not match request".into(),
        ));
    }
    let metadata_start = EconomicPeriod::day(&metadata.observation_start)?;
    let metadata_end = EconomicPeriod::day(&metadata.observation_end)?;
    if metadata_start > metadata_end {
        return Err(FredError::Protocol(
            "series metadata date range is reversed".into(),
        ));
    }
    let frequency = parse_frequency(&metadata.frequency, &metadata.frequency_short)?;
    if frequency != context.frequency {
        return Err(FredError::Protocol(
            "series frequency does not match request".into(),
        ));
    }
    if metadata.units.trim().is_empty() || metadata.seasonal_adjustment.trim().is_empty() {
        return Err(FredError::Protocol(
            "series unit and seasonal adjustment are mandatory".into(),
        ));
    }
    let source_at = parse_fred_timestamp(&metadata.last_updated)?;

    let observations: ObservationsEnvelope = serde_json::from_slice(observations_body)
        .map_err(|error| FredError::Decode(error.to_string()))?;
    let realtime_start = EconomicPeriod::day(&observations.realtime_start)?;
    let realtime_end = EconomicPeriod::day(&observations.realtime_end)?;
    let observation_start = EconomicPeriod::day(&observations.observation_start)?;
    let observation_end = EconomicPeriod::day(&observations.observation_end)?;
    let requested_start = EconomicPeriod::day(context.query_start)?;
    let requested_end = EconomicPeriod::day(context.query_end)?;
    if observations.observation_start != context.query_start
        || observations.observation_end != context.query_end
        || requested_start > requested_end
        || observations.offset != 0
        || observations.count != observations.observations.len()
        || observations.limit != 100_000
        || observations.observations.len() > observations.limit
        || observations.sort_order != "asc"
        || observations.order_by != "observation_date"
        || observations.units != "lin"
        || observations.output_type != 1
        || observations.file_type != "json"
        || realtime_start != realtime_end
        || observation_start > observation_end
    {
        return Err(FredError::Protocol(
            "observation pagination or ordering metadata is inconsistent".into(),
        ));
    }

    let mut records = Vec::with_capacity(observations.observations.len());
    let mut seen = HashSet::with_capacity(observations.observations.len());
    let mut previous = None;
    for raw in observations.observations {
        if raw.realtime_start != raw.realtime_end
            || raw.realtime_start != observations.realtime_start
            || raw.realtime_end != observations.realtime_end
            || EconomicPeriod::day(&raw.realtime_start).is_err()
        {
            return Err(FredError::Protocol(
                "observation realtime bounds must match".into(),
            ));
        }
        let raw_date = EconomicPeriod::day(&raw.date)?;
        if raw_date < observation_start
            || raw_date > observation_end
            || raw_date < requested_start
            || raw_date > requested_end
            || raw_date < metadata_start
            || raw_date > metadata_end
        {
            return Err(FredError::Protocol(
                "raw observation date falls outside bound response metadata".into(),
            ));
        }
        let period = parse_period(&raw.date, frequency)?;
        if !seen.insert(period.clone()) {
            return Err(FredError::Protocol("duplicate observation period".into()));
        }
        if previous.as_ref().is_some_and(|value| value >= &period) {
            return Err(FredError::Protocol(
                "observations are not strictly ascending".into(),
            ));
        }
        previous = Some(period.clone());
        let (value, status) = if raw.value == "." {
            (None, EconomicObservationStatus::Missing)
        } else {
            let parsed = raw
                .value
                .parse::<f64>()
                .map_err(|_| FredError::Protocol("invalid observation value".into()))?;
            (
                Some(FiniteNumber::new(parsed)?),
                EconomicObservationStatus::Present,
            )
        };
        if period < *context.start || period > *context.end {
            continue;
        }
        let evidence =
            SourceEvidence::new(ProviderId::Fred, context.observed_at, context.batch_id)?
                .with_source_at(source_at.clone())?;
        let revision = EconomicRevision {
            kind: EconomicRevisionKind::SourceDefined(NonEmptyText::new("FRED realtime")?),
            label: Some(NonEmptyText::new(raw.realtime_start)?),
        };
        records.push(EconomicObservation::new(
            context.key.clone(),
            metadata.title.clone(),
            None,
            None,
            period,
            value,
            metadata.units.clone(),
            None,
            Some(NonEmptyText::new(metadata.seasonal_adjustment.clone())?),
            status,
            Some(NonEmptyText::new(source_at.clone())?),
            Some(revision),
            evidence,
        )?);
    }
    let provenance = Provenance::new("FRED", context.observed_at)?
        .with_batch_id(context.batch_id)?
        .with_source_at(source_at)?;
    Ok(DataBatch::strict(records, provenance))
}

fn reject_api_error(body: &[u8]) -> Result<(), FredError> {
    let value: Value =
        serde_json::from_slice(body).map_err(|error| FredError::Decode(error.to_string()))?;
    if value.get("error_code").is_some() || value.get("error_message").is_some() {
        return Err(FredError::Authentication(
            "official API rejected the supplied credentials".into(),
        ));
    }
    Ok(())
}

fn parse_frequency(long: &str, short: &str) -> Result<EconomicFrequency, FredError> {
    match (long, short) {
        ("Daily", "D") => Ok(EconomicFrequency::Daily),
        ("Weekly", "W") => Ok(EconomicFrequency::Weekly),
        ("Monthly", "M") => Ok(EconomicFrequency::Monthly),
        ("Quarterly", "Q") => Ok(EconomicFrequency::Quarterly),
        ("Annual", "A") => Ok(EconomicFrequency::Annual),
        _ => Err(FredError::Protocol(
            "unsupported or inconsistent FRED frequency".into(),
        )),
    }
}

fn parse_period(date: &str, frequency: EconomicFrequency) -> Result<EconomicPeriod, FredError> {
    let (year, month, day) = parse_date_parts(date)?;
    match frequency {
        EconomicFrequency::Daily => Ok(EconomicPeriod::day(date)?),
        EconomicFrequency::Monthly if day == 1 => Ok(EconomicPeriod::month(year, month)?),
        EconomicFrequency::Quarterly if day == 1 && matches!(month, 1 | 4 | 7 | 10) => {
            Ok(EconomicPeriod::quarter(year, (month - 1) / 3 + 1)?)
        }
        EconomicFrequency::Annual if month == 1 && day == 1 => Ok(EconomicPeriod::year(year)?),
        EconomicFrequency::Weekly => {
            let month = time::Month::try_from(month as u8)
                .map_err(|_| FredError::Protocol("invalid weekly observation date".into()))?;
            let date = time::Date::from_calendar_date(year as i32, month, day as u8)
                .map_err(|_| FredError::Protocol("invalid weekly observation date".into()))?;
            let (iso_year, week, _) = date.to_iso_week_date();
            Ok(EconomicPeriod::iso_week(iso_year as u32, u32::from(week))?)
        }
        _ => Err(FredError::Protocol(
            "observation date is not aligned to its frequency".into(),
        )),
    }
}

fn parse_date_parts(value: &str) -> Result<(u32, u32, u32), FredError> {
    if value.len() != 10 || value.as_bytes()[4] != b'-' || value.as_bytes()[7] != b'-' {
        return Err(FredError::Protocol("invalid observation date".into()));
    }
    let year = value[0..4]
        .parse()
        .map_err(|_| FredError::Protocol("invalid observation date".into()))?;
    let month = value[5..7]
        .parse()
        .map_err(|_| FredError::Protocol("invalid observation date".into()))?;
    let day = value[8..10]
        .parse()
        .map_err(|_| FredError::Protocol("invalid observation date".into()))?;
    EconomicPeriod::day(value)?;
    Ok((year, month, day))
}

fn parse_fred_timestamp(value: &str) -> Result<String, FredError> {
    if value.len() != 22
        || value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || value.as_bytes()[10] != b' '
        || value.as_bytes()[13] != b':'
        || value.as_bytes()[16] != b':'
        || !matches!(value.as_bytes()[19], b'+' | b'-')
        || !value
            .bytes()
            .enumerate()
            .filter(|(index, _)| !matches!(index, 4 | 7 | 10 | 13 | 16 | 19))
            .all(|(_, byte)| byte.is_ascii_digit())
    {
        return Err(FredError::Protocol(
            "invalid FRED last_updated timestamp".into(),
        ));
    }
    let year = value[0..4]
        .parse::<i32>()
        .map_err(|_| FredError::Protocol("invalid FRED last_updated timestamp".into()))?;
    let month = value[5..7]
        .parse::<u8>()
        .ok()
        .and_then(|value| time::Month::try_from(value).ok())
        .ok_or_else(|| FredError::Protocol("invalid FRED last_updated timestamp".into()))?;
    let day = value[8..10]
        .parse::<u8>()
        .map_err(|_| FredError::Protocol("invalid FRED last_updated timestamp".into()))?;
    time::Date::from_calendar_date(year, month, day)
        .map_err(|_| FredError::Protocol("invalid FRED last_updated timestamp".into()))?;
    let hour = value[11..13]
        .parse::<u8>()
        .map_err(|_| FredError::Protocol("invalid FRED last_updated timestamp".into()))?;
    let minute = value[14..16]
        .parse::<u8>()
        .map_err(|_| FredError::Protocol("invalid FRED last_updated timestamp".into()))?;
    let second = value[17..19]
        .parse::<u8>()
        .map_err(|_| FredError::Protocol("invalid FRED last_updated timestamp".into()))?;
    time::Time::from_hms(hour, minute, second)
        .map_err(|_| FredError::Protocol("invalid FRED last_updated timestamp".into()))?;
    let offset_hours = value[20..22]
        .parse::<i8>()
        .map_err(|_| FredError::Protocol("invalid FRED last_updated timestamp".into()))?;
    let signed_offset = if value.as_bytes()[19] == b'-' {
        -offset_hours
    } else {
        offset_hours
    };
    time::UtcOffset::from_hms(signed_offset, 0, 0)
        .map_err(|_| FredError::Protocol("invalid FRED last_updated timestamp".into()))?;
    Ok(format!(
        "{}T{}{}:00",
        &value[..10],
        &value[11..19],
        &value[19..]
    ))
}

fn ensure_no_duplicate_keys(body: &[u8]) -> Result<(), FredError> {
    super::transport::ensure_no_duplicate_json_keys(body)
        .map_err(|error| FredError::Decode(error.to_string()))
}

#[cfg(test)]
mod tests;
