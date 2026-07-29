use crate::{FredError, FredParseContext};
use magic_market_core::{
    DataBatch, EconomicFrequency, EconomicObservation, EconomicPeriod, EconomicSeriesRequest,
    ProviderId,
};
use magic_market_transport::{
    EndpointPolicy, HttpMethod, HttpRequest, HttpTransport, MediaType, RequestGate,
};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::fmt;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const SERIES_URL: &str = "https://api.stlouisfed.org/fred/series";
const OBSERVATIONS_URL: &str = "https://api.stlouisfed.org/fred/series/observations";

pub(crate) fn policy() -> Result<EndpointPolicy, magic_market_transport::TransportError> {
    EndpointPolicy::new(
        "api.stlouisfed.org",
        vec!["/fred/series".into(), "/fred/series/observations".into()],
        vec![
            "api_key".into(),
            "file_type".into(),
            "series_id".into(),
            "observation_start".into(),
            "observation_end".into(),
            "offset".into(),
            "limit".into(),
            "sort_order".into(),
        ],
        vec![MediaType::Json],
        4 * 1024 * 1024,
        Duration::from_secs(30),
    )
}

pub(crate) fn fetch_series(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    api_key: &str,
    request: &EconomicSeriesRequest,
) -> Result<DataBatch<EconomicObservation>, FredError> {
    for key in request.series() {
        validate_key(key)?;
    }
    let frequency = request.start().frequency();
    let start = period_date(request.start(), true)?;
    let end = period_date(request.end(), false)?;
    let observed_at = observed_at()?;
    let batch_id = format!("FRED:economic-series:{observed_at}");
    let mut all = Vec::new();
    for key in request.series() {
        let metadata_url = query_url(
            SERIES_URL,
            &[
                ("api_key", api_key),
                ("file_type", "json"),
                ("series_id", key.code()),
            ],
        );
        let observations_url = query_url(
            OBSERVATIONS_URL,
            &[
                ("api_key", api_key),
                ("file_type", "json"),
                ("series_id", key.code()),
                ("observation_start", &start),
                ("observation_end", &end),
                ("offset", "0"),
                ("limit", "100000"),
                ("sort_order", "asc"),
            ],
        );
        let metadata = execute(transport, gate, &metadata_url)?;
        let observations = execute(transport, gate, &observations_url)?;
        let parsed = crate::parse_fred_responses(
            &metadata,
            &observations,
            &FredParseContext {
                key,
                frequency,
                start: request.start(),
                end: request.end(),
                query_start: &start,
                query_end: &end,
                observed_at: &observed_at,
                batch_id: &batch_id,
            },
        )?;
        all.extend(parsed.into_records());
    }
    all.sort_by(|left, right| {
        let series_order = request
            .series()
            .iter()
            .position(|key| key == left.series())
            .cmp(
                &request
                    .series()
                    .iter()
                    .position(|key| key == right.series()),
            );
        series_order.then_with(|| left.period().cmp(right.period()))
    });
    if all.len() > request.max_rows().get() as usize {
        return Err(FredError::InvalidRequest(
            "FRED result exceeds requested max_rows".into(),
        ));
    }
    let provenance =
        magic_market_core::Provenance::new("FRED", observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::strict(all, provenance))
}

fn validate_key(key: &magic_market_core::EconomicSeriesKey) -> Result<(), FredError> {
    if key.provider() != ProviderId::Fred || key.namespace() != "fred" {
        return Err(FredError::InvalidRequest(
            "FRED keys require provider Fred and namespace fred".into(),
        ));
    }
    if !valid_code(key.code()) {
        return Err(FredError::InvalidRequest(
            "FRED series code contains forbidden characters".into(),
        ));
    }
    Ok(())
}

fn execute(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    url: &str,
) -> Result<Vec<u8>, FredError> {
    gate.wait_for_turn()?;
    let request = HttpRequest::new(
        HttpMethod::Get,
        url,
        vec![("Accept".into(), "application/json".into())],
        vec![],
    )?;
    let policy = policy()?;
    policy.validate_request(&request)?;
    let response = policy.validate_response_for(&request, transport.execute(&request)?)?;
    Ok(response.body().to_vec())
}

fn period_date(period: &EconomicPeriod, start: bool) -> Result<String, FredError> {
    match period.frequency() {
        EconomicFrequency::Daily => Ok(period
            .as_day()
            .ok_or_else(|| FredError::InvalidRequest("daily period expected".into()))?
            .to_owned()),
        EconomicFrequency::Monthly => {
            let (year, month) = period
                .as_month()
                .ok_or_else(|| FredError::InvalidRequest("monthly period expected".into()))?;
            let day = if start { 1 } else { days_in_month(year, month) };
            Ok(format!("{year:04}-{month:02}-{day:02}"))
        }
        EconomicFrequency::Quarterly => {
            let (year, quarter) = period
                .as_quarter()
                .ok_or_else(|| FredError::InvalidRequest("quarterly period expected".into()))?;
            let first_month = (quarter - 1) * 3 + 1;
            let month = if start { first_month } else { first_month + 2 };
            let day = if start { 1 } else { days_in_month(year, month) };
            Ok(format!("{year:04}-{month:02}-{day:02}"))
        }
        EconomicFrequency::Annual => {
            let year = period
                .as_year()
                .ok_or_else(|| FredError::InvalidRequest("annual period expected".into()))?;
            Ok(format!(
                "{year:04}-{}-{}",
                if start { "01" } else { "12" },
                if start { "01" } else { "31" }
            ))
        }
        EconomicFrequency::Weekly => {
            let (year, week) = period
                .as_iso_week()
                .ok_or_else(|| FredError::InvalidRequest("weekly period expected".into()))?;
            let weekday = if start {
                time::Weekday::Monday
            } else {
                time::Weekday::Sunday
            };
            let date = time::Date::from_iso_week_date(year as i32, week as u8, weekday)
                .map_err(|_| FredError::InvalidRequest("invalid ISO week".into()))?;
            Ok(format!(
                "{:04}-{:02}-{:02}",
                date.year(),
                u8::from(date.month()),
                date.day()
            ))
        }
        EconomicFrequency::Irregular => Err(FredError::Unsupported(
            "FRED irregular ranges are not admitted".into(),
        )),
    }
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => 31,
    }
}

fn valid_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn query_url(base: &str, params: &[(&str, &str)]) -> String {
    let query = params
        .iter()
        .map(|(key, value)| format!("{key}={}", percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{base}?{query}")
}

fn percent_encode(value: &str) -> String {
    use std::fmt::Write;
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn observed_at() -> Result<String, FredError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| FredError::Protocol("failed to format observation timestamp".into()))
}

pub(crate) fn ensure_no_duplicate_json_keys(body: &[u8]) -> Result<(), serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_slice(body);
    NoDuplicates.deserialize(&mut deserializer)?;
    deserializer.end()
}

struct NoDuplicates;

impl<'de> DeserializeSeed<'de> for NoDuplicates {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(NoDuplicatesVisitor)
    }
}

struct NoDuplicatesVisitor;

impl<'de> Visitor<'de> for NoDuplicatesVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen = std::collections::HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            map.next_value_seed(NoDuplicates)?;
        }
        Ok(())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<(), A::Error>
    where
        A: SeqAccess<'de>,
    {
        while seq.next_element_seed(NoDuplicates)?.is_some() {}
        Ok(())
    }

    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_string<E>(self, _: String) -> Result<(), E> {
        Ok(())
    }
    fn visit_none<E>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_some<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        NoDuplicates.deserialize(deserializer)
    }
    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
