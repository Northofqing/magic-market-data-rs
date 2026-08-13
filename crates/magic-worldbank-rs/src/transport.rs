use crate::{WorldBankError, WorldBankParseContext};
use magic_market_core::{
    DataBatch, EconomicFrequency, EconomicObservation, EconomicSeriesRequest, Provenance,
    ProviderId,
};
use magic_market_transport::{
    EndpointPolicy, HttpMethod, HttpRequest, HttpTransport, MediaType, RequestGate,
};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const BASE_URL: &str = "https://api.worldbank.org/v2";

pub(crate) fn policy() -> Result<EndpointPolicy, magic_market_transport::TransportError> {
    EndpointPolicy::new(
        "api.worldbank.org",
        vec![
            "/v2/indicator".into(),
            "/v2/country".into(),
            "/v2/sources/2/series".into(),
        ],
        vec![
            "format".into(),
            "date".into(),
            "page".into(),
            "per_page".into(),
            "source".into(),
        ],
        vec![MediaType::Json],
        8 * 1024 * 1024,
        Duration::from_secs(30),
    )
}

pub(crate) fn fetch_series(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    request: &EconomicSeriesRequest,
) -> Result<DataBatch<EconomicObservation>, WorldBankError> {
    let mut namespaces = Vec::with_capacity(request.series().len());
    for key in request.series() {
        if key.provider() != ProviderId::WorldBank {
            return Err(WorldBankError::InvalidRequest(
                "World Bank provider mismatch".into(),
            ));
        }
        if !valid_indicator_code(key.code()) {
            return Err(WorldBankError::InvalidRequest(
                "invalid World Bank indicator code".into(),
            ));
        }
        namespaces.push(crate::parse_world_bank_namespace(key.namespace())?);
    }
    if namespaces
        .iter()
        .any(|namespace| namespace.source_id() != "2")
    {
        return Err(WorldBankError::Unsupported(
            "only World Development Indicators source 2 has an audited metadata contract".into(),
        ));
    }
    if request.start().frequency() != EconomicFrequency::Annual {
        return Err(WorldBankError::Unsupported(
            "World Bank production path supports annual periods".into(),
        ));
    }
    if request.series().len() > 60 {
        return Err(WorldBankError::InvalidRequest(
            "World Bank accepts at most 60 indicators".into(),
        ));
    }
    let start_year = request
        .start()
        .as_year()
        .ok_or_else(|| WorldBankError::InvalidRequest("annual start required".into()))?;
    let end_year = request
        .end()
        .as_year()
        .ok_or_else(|| WorldBankError::InvalidRequest("annual end required".into()))?;
    let observed_at = observed_at()?;
    let batch_id = format!("WorldBank:economic-series:{observed_at}");
    let mut records = Vec::new();
    for (key, namespace) in request.series().iter().zip(&namespaces) {
        let indicator_url = format!(
            "{BASE_URL}/indicator/{}?format=json&source={}",
            key.code(),
            namespace.source_id()
        );
        let indicator = execute(transport, gate, &indicator_url)?;
        let metadata_url = format!(
            "{BASE_URL}/sources/{}/series/{}/metadata?format=json",
            namespace.source_id(),
            key.code()
        );
        let series_metadata = execute(transport, gate, &metadata_url)?;
        // Validate identity, official unit and annual frequency before issuing
        // any data-page request.
        match crate::parse_world_bank_responses_with_metadata(
            &indicator,
            &series_metadata,
            &[],
            &WorldBankParseContext {
                key,
                start_year,
                end_year,
                observed_at: &observed_at,
                batch_id: &batch_id,
            },
        ) {
            Err(WorldBankError::Protocol(message))
                if message == "World Bank response contains no data pages" => {}
            Err(error) => return Err(error),
            Ok(_) => {
                return Err(WorldBankError::Protocol(
                    "World Bank metadata-only validation unexpectedly returned records".into(),
                ))
            }
        }
        let first_url = data_url(
            namespace.economy(),
            key.code(),
            namespace.source_id(),
            start_year,
            end_year,
            1,
        );
        let first = execute(transport, gate, &first_url)?;
        let pages = page_count(&first)?;
        if pages > 100 {
            return Err(WorldBankError::Protocol(
                "World Bank response exceeds 100 pages".into(),
            ));
        }
        let mut bodies = vec![first];
        for page in 2..=pages {
            bodies.push(execute(
                transport,
                gate,
                &data_url(
                    namespace.economy(),
                    key.code(),
                    namespace.source_id(),
                    start_year,
                    end_year,
                    page,
                ),
            )?);
        }
        let refs = bodies.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let parsed = crate::parse_world_bank_responses_with_metadata(
            &indicator,
            &series_metadata,
            &refs,
            &WorldBankParseContext {
                key,
                start_year,
                end_year,
                observed_at: &observed_at,
                batch_id: &batch_id,
            },
        )?;
        records.extend(parsed.into_records());
    }
    records.sort_by(|left, right| {
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
    if records.len() > request.max_rows().get() as usize {
        return Err(WorldBankError::InvalidRequest(
            "World Bank result exceeds max_rows".into(),
        ));
    }
    Ok(DataBatch::strict(
        records,
        Provenance::new("World Bank Indicators v2", observed_at)?.with_batch_id(batch_id)?,
    ))
}

fn valid_indicator_code(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn data_url(
    economy: &str,
    indicator: &str,
    source: &str,
    start: u32,
    end: u32,
    page: usize,
) -> String {
    format!(
        "{BASE_URL}/country/{economy}/indicator/{indicator}?format=json&date={start}:{end}&page={page}&per_page=1000&source={source}"
    )
}

fn execute(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    url: &str,
) -> Result<Vec<u8>, WorldBankError> {
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

fn page_count(body: &[u8]) -> Result<usize, WorldBankError> {
    ensure_no_duplicate_json_keys(body)
        .map_err(|error| WorldBankError::Decode(error.to_string()))?;
    let value: Value =
        serde_json::from_slice(body).map_err(|error| WorldBankError::Decode(error.to_string()))?;
    let metadata = value
        .as_array()
        .and_then(|array| array.first())
        .and_then(Value::as_object)
        .ok_or_else(|| WorldBankError::Protocol("invalid first page metadata".into()))?;
    match metadata.get("pages") {
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| WorldBankError::Protocol("invalid pages field".into())),
        Some(Value::String(value)) => value
            .parse()
            .map_err(|_| WorldBankError::Protocol("invalid pages field".into())),
        _ => Err(WorldBankError::Protocol("missing pages field".into())),
    }
}

fn observed_at() -> Result<String, WorldBankError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| WorldBankError::Protocol("failed to format observation timestamp".into()))
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
        let mut seen = HashSet::new();
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
