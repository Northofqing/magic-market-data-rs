use crate::{ImfError, ImfParseContext};
use magic_market_core::{
    DataBatch, EconomicObservation, EconomicSeriesRequest, Provenance, ProviderId,
};
use magic_market_transport::{
    EndpointPolicy, HttpMethod, HttpRequest, HttpTransport, MediaType, RequestGate,
};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::collections::HashSet;
use std::fmt;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const BASE_URL: &str = "https://www.imf.org/external/datamapper/api/v2";
const USER_AGENT: &str = "magic-imf-rs/0.2";

pub(crate) fn policy() -> Result<EndpointPolicy, magic_market_transport::TransportError> {
    EndpointPolicy::new(
        "www.imf.org",
        vec!["/external/datamapper/api/v2".into()],
        vec!["periods".into()],
        vec![MediaType::Json],
        16 * 1024 * 1024,
        Duration::from_secs(30),
    )
}

pub(crate) fn fetch_series(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    request: &EconomicSeriesRequest,
) -> Result<DataBatch<EconomicObservation>, ImfError> {
    let mut namespaces = Vec::with_capacity(request.series().len());
    for key in request.series() {
        if key.provider() != ProviderId::Imf {
            return Err(ImfError::InvalidRequest("IMF provider mismatch".into()));
        }
        if !valid_component(key.code()) {
            return Err(ImfError::InvalidRequest(
                "invalid IMF indicator code".into(),
            ));
        }
        namespaces.push(crate::parse_namespace(key.namespace())?);
    }
    let start_year = request
        .start()
        .as_year()
        .ok_or_else(|| ImfError::InvalidRequest("annual start period required".into()))?;
    let end_year = request
        .end()
        .as_year()
        .ok_or_else(|| ImfError::InvalidRequest("annual end period required".into()))?;
    if end_year - start_year >= 50 {
        return Err(ImfError::InvalidRequest(
            "IMF request accepts at most 50 years".into(),
        ));
    }
    let distinct_areas: HashSet<String> = namespaces
        .iter()
        .map(|namespace| namespace.area().to_owned())
        .collect();
    if distinct_areas.len() > 20 {
        return Err(ImfError::InvalidRequest(
            "IMF request accepts at most 20 distinct areas".into(),
        ));
    }

    let observed_at = observed_at()?;
    let batch_id = format!("IMF:economic-series:{observed_at}");
    let catalog = execute(transport, gate, &format!("{BASE_URL}/indicators"))?;
    if catalog.len() > 8 * 1024 * 1024 {
        return Err(ImfError::Protocol(
            "IMF catalog exceeds the 8 MiB provider limit".into(),
        ));
    }
    let periods = (start_year..=end_year)
        .map(|year| year.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let mut records = Vec::new();
    for (key, namespace) in request.series().iter().zip(&namespaces) {
        let url = format!(
            "{BASE_URL}/{}/{}?periods={periods}",
            key.code(),
            namespace.area()
        );
        let series = execute(transport, gate, &url)?;
        let parsed = crate::parse_imf_responses(
            &catalog,
            &series,
            &ImfParseContext {
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
        return Err(ImfError::InvalidRequest(
            "IMF result exceeds requested max_rows".into(),
        ));
    }
    Ok(DataBatch::strict(
        records,
        Provenance::new("IMF DataMapper v2", observed_at)?.with_batch_id(batch_id)?,
    ))
}

fn execute(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    url: &str,
) -> Result<Vec<u8>, ImfError> {
    gate.wait_for_turn()?;
    let request = HttpRequest::new(
        HttpMethod::Get,
        url,
        vec![
            ("Accept".into(), "application/json".into()),
            ("User-Agent".into(), USER_AGENT.into()),
        ],
        vec![],
    )?;
    let policy = policy()?;
    policy.validate_request(&request)?;
    let response = policy.validate_response_for(&request, transport.execute(&request)?)?;
    Ok(response.body().to_vec())
}

fn valid_component(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn observed_at() -> Result<String, ImfError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ImfError::Protocol("failed to format observation timestamp".into()))
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
