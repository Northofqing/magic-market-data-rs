use super::register_economic_provider;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use magic_market_transport::{
    HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use magic_worldbank_rs::WorldBankClient;
use std::sync::Arc;

const INDICATOR: &[u8] =
    include_bytes!("../../../magic-worldbank-rs/tests/fixtures/indicator.json");

// Synthetic external metadata: two individually well-formed Series variables
// make the requested GDP unit/name ambiguous. This is not live source evidence.
const AMBIGUOUS_SERIES_METADATA: &[u8] = br#"{
    "page":1,"pages":1,"per_page":"5000","total":3,
    "source":[{"id":"2","name":"World Development Indicators",
        "concept":[{"id":"Series","variable":[
            {"id":"NY.GDP.MKTP.CD","metatype":[
                {"id":"IndicatorName","value":"GDP (current US$)"},
                {"id":"Unitofmeasure","value":"current US$"},
                {"id":"Periodicity","value":"Annual"}]},
            {"id":"SP.POP.TOTL","metatype":[
                {"id":"IndicatorName","value":"Population, total"},
                {"id":"Unitofmeasure","value":"people"},
                {"id":"Periodicity","value":"Annual"}]}
        ]}]}]
}"#;

struct AmbiguousMetadataFixture;

impl HttpTransport for AmbiguousMetadataFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        if request.method() != HttpMethod::Get || !request.body().is_empty() {
            return Err(TransportError::InvalidRequest(
                "ambiguous WorldBank fixture requires bodyless GET".into(),
            ));
        }
        let body = match request.url() {
            "https://api.worldbank.org/v2/indicator/NY.GDP.MKTP.CD?format=json&source=2" => {
                INDICATOR
            }
            "https://api.worldbank.org/v2/sources/2/series/NY.GDP.MKTP.CD/metadata?format=json" => {
                AMBIGUOUS_SERIES_METADATA
            }
            _ => {
                // A data-page request would become a different public error,
                // so it cannot satisfy the expected metadata rejection below.
                return Err(TransportError::InvalidRequest(
                    "ambiguous WorldBank fixture does not serve data pages".into(),
                ));
            }
        };
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body.to_vec(),
        ))
    }
}

#[test]
fn registered_worldbank_rejects_ambiguous_metadata_without_publishing_data() {
    let client = WorldBankClient::with_transport(Arc::new(AmbiguousMetadataFixture)).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline external source only");
    // Real production registration is setup; Execute is the public test seam.
    register_economic_provider(
        &mut registry,
        client,
        "WorldBank",
        "exact admitted World Bank source 2 USA annual GDP observation",
        4096,
    )
    .unwrap();
    let command = QueryCommand::new(
        "offline-worldbank-ambiguous-metadata-v1",
        Operation::EconomicSeries,
        Some("WorldBank".into()),
        CanonicalPayload::new(
            "magic.market.economic_series.request",
            1,
            br#"{
                "series":[{"provider":"WorldBank","namespace":"source:2/country:USA",
                           "code":"NY.GDP.MKTP.CD"}],
                "start":{"Annual":{"year":2024}},
                "end":{"Annual":{"year":2024}},"max_rows":1
            }"#
            .to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap();

    match registry.execute(command) {
        Err(ServiceError::FailedPrecondition(reason)) => assert_eq!(
            reason,
            "World Bank protocol violation: series metadata requires exactly one variable"
        ),
        other => panic!("ambiguous source metadata must reject the public query: {other:?}"),
    }
}

use std::sync::Mutex;

const WB_INTEGRITY_INDICATOR_URL: &str =
    "https://api.worldbank.org/v2/indicator/NY.GDP.MKTP.CD?format=json&source=2";
const WB_INTEGRITY_METADATA_URL: &str =
    "https://api.worldbank.org/v2/sources/2/series/NY.GDP.MKTP.CD/metadata?format=json";
const WB_INTEGRITY_DATA_URL: &str =
    "https://api.worldbank.org/v2/country/USA/indicator/NY.GDP.MKTP.CD?format=json&date=2024:2024&page=1&per_page=1000&source=2";
const WB_INTEGRITY_METADATA: &[u8] = br#"{
    "page":1,"pages":1,"per_page":"5000","total":3,
    "source":[{"id":"2","name":"World Development Indicators",
        "concept":[{"id":"Series","variable":[
            {"id":"NY.GDP.MKTP.CD","metatype":[
                {"id":"IndicatorName","value":"GDP (current US$)"},
                {"id":"Unitofmeasure","value":"current US$"},
                {"id":"Periodicity","value":"Annual"}]}]}]}]
}"#;
const WB_INTEGRITY_ZERO_DATA: &[u8] = br#"[
    {"page":1,"pages":1,"per_page":1000,"total":1,
     "sourceid":"2","lastupdated":"2026-07-13"},
    [{"indicator":{"id":"NY.GDP.MKTP.CD","value":"GDP (current US$)"},
      "country":{"id":"US","value":"United States"},
      "countryiso3code":"USA","date":"2024","value":0,
      "unit":"","obs_status":"","decimal":0}]
]"#;

// Only the external HTTP response is substituted. Clone preserves every public
// request field, including header multiplicity/order and raw body bytes, BEFORE
// any fixture guard can reject an attempt. No SDK/provider result is mocked.
struct WorldBankIntegrityFixture {
    indicator: Vec<u8>,
    metadata: Vec<u8>,
    serve_data: bool,
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl HttpTransport for WorldBankIntegrityFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let index = {
            let mut attempts = self.attempts.lock().unwrap();
            let index = attempts.len();
            attempts.push(request.clone());
            index
        };
        let expected_headers = vec![("Accept".to_owned(), "application/json".to_owned())];
        if request.method() != HttpMethod::Get
            || !request.body().is_empty()
            || request.headers() != expected_headers.as_slice()
        {
            return Err(TransportError::InvalidRequest(
                "WorldBank integrity fixture requires exact bodyless GET headers".into(),
            ));
        }
        let body = match (index, request.url()) {
            (0, WB_INTEGRITY_INDICATOR_URL) => self.indicator.clone(),
            (1, WB_INTEGRITY_METADATA_URL) => self.metadata.clone(),
            (2, WB_INTEGRITY_DATA_URL) if self.serve_data => WB_INTEGRITY_ZERO_DATA.to_vec(),
            _ => {
                return Err(TransportError::InvalidRequest(
                    "WorldBank integrity fixture refuses extra, reordered or forbidden data attempts"
                        .into(),
                ));
            }
        };
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body,
        ))
    }
}

#[test]
fn registered_worldbank_metadata_integrity_is_required_before_any_data_request() {
    use magic_market_core::EvidenceTimestamp;
    use serde_json::{json, Value};
    use std::time::{SystemTime, UNIX_EPOCH};

    let seed = || -> (Value, Value) {
        (
            serde_json::from_slice(INDICATOR).unwrap(),
            serde_json::from_slice(WB_INTEGRITY_METADATA).unwrap(),
        )
    };
    let execute = |indicator: &Value, metadata: &Value, serve_data| {
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let client = WorldBankClient::with_transport(Arc::new(WorldBankIntegrityFixture {
            indicator: serde_json::to_vec(indicator).unwrap(),
            metadata: serde_json::to_vec(metadata).unwrap(),
            serve_data,
            attempts: Arc::clone(&attempts),
        }))
        .unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline external source only");
        // Existing real registrar is setup; only public Execute is asserted.
        register_economic_provider(
            &mut registry,
            client,
            "WorldBank",
            "exact admitted World Bank source 2 USA annual GDP observation",
            4096,
        )
        .unwrap();
        let command = QueryCommand::new(
            "offline-worldbank-metadata-integrity-v1",
            Operation::EconomicSeries,
            Some("WorldBank".into()),
            CanonicalPayload::new(
                "magic.market.economic_series.request",
                1,
                br#"{
                    "series":[{"provider":"WorldBank","namespace":"source:2/country:USA",
                               "code":"NY.GDP.MKTP.CD"}],
                    "start":{"Annual":{"year":2024}},
                    "end":{"Annual":{"year":2024}},"max_rows":1
                }"#
                .to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();
        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let result = registry.execute(command);
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        (result, attempts, before, after)
    };
    // These expected requests are independent external protocol literals, not
    // derived from returned SDK requests or public business result values.
    let expected_request = |url| {
        HttpRequest::new(
            HttpMethod::Get,
            url,
            vec![("Accept".into(), "application/json".into())],
            vec![],
        )
        .unwrap()
    };

    // Same-wiring full literal control qualifies the new response-owning seam.
    // Source zero remains Present/0, not Missing/null or a dropped observation.
    let (indicator, metadata) = seed();
    let (result, attempts, before, after) = execute(&indicator, &metadata, true);
    let result = result.unwrap();
    assert_eq!(
        *attempts.lock().unwrap(),
        vec![
            expected_request(WB_INTEGRITY_INDICATOR_URL),
            expected_request(WB_INTEGRITY_METADATA_URL),
            expected_request(WB_INTEGRITY_DATA_URL),
        ]
    );
    assert_eq!(result.provider, "WorldBank");
    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at, None);
    let observed = EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
    let lower = EvidenceTimestamp::parse_instant(&format!(
        "{}.{:09}",
        before.as_secs(),
        before.subsec_nanos()
    ))
    .unwrap();
    let upper = EvidenceTimestamp::parse_instant(&format!(
        "{}.{:09}",
        after.as_secs(),
        after.subsec_nanos()
    ))
    .unwrap();
    assert!(lower <= observed && observed <= upper);
    let expected_batch = format!("WorldBank:economic-series:{}", result.observed_at);
    assert_eq!(result.batch_id, expected_batch);
    assert_eq!(result.records.len(), 1);
    assert_eq!(
        result.records[0].schema(),
        "magic.market.economic_observation"
    );
    assert_eq!(result.records[0].schema_version(), 1);
    let wire: Vec<Value> = result
        .records
        .iter()
        .map(|record| serde_json::from_slice(record.data()).unwrap())
        .collect();
    assert_eq!(
        wire,
        vec![json!({
            "series":{"provider":"WorldBank","namespace":"source:2/country:USA",
                      "code":"NY.GDP.MKTP.CD"},
            "name":"GDP (current US$)","region_code":"USA","region_name":"United States",
            "period":{"Annual":{"year":2024}},"value":0.0,"unit":"current US$",
            "scale":null,"seasonal_adjustment":null,"status":"Present",
            "released_at":"2026-07-13","revision":null,
            "evidence":{"provider":"WorldBank","source_at":"2026-07-13",
                        "observed_at":result.observed_at.as_str(),"batch_id":expected_batch.as_str()}
        })]
    );

    // One raw alteration per row. Earlier envelopes stay valid, so each full
    // reason names the intended first metadata gate rather than a decode error.
    type Case = (&'static str, fn(&mut Value, &mut Value), &'static str);
    let cases: [Case; 16] = [
        (
            "metadata root", |_, m| *m = json!([]),
            "World Bank protocol violation: World Bank series metadata must be an object",
        ),
        (
            "metadata page", |_, m| m["page"] = json!(2),
            "World Bank protocol violation: World Bank series metadata pagination is inconsistent: page=2 pages=1 per_page=5000 total=3",
        ),
        (
            "metadata pages", |_, m| m["pages"] = json!(2),
            "World Bank protocol violation: World Bank series metadata pagination is inconsistent: page=1 pages=2 per_page=5000 total=3",
        ),
        (
            "metadata per_page lower bound", |_, m| m["per_page"] = json!(0),
            "World Bank protocol violation: World Bank series metadata pagination is inconsistent: page=1 pages=1 per_page=0 total=3",
        ),
        (
            "metadata per_page upper bound", |_, m| m["per_page"] = json!(5001),
            "World Bank protocol violation: World Bank series metadata pagination is inconsistent: page=1 pages=1 per_page=5001 total=3",
        ),
        (
            "metadata total lower bound", |_, m| m["total"] = json!(0),
            "World Bank protocol violation: World Bank series metadata pagination is inconsistent: page=1 pages=1 per_page=5000 total=0",
        ),
        (
            "metadata total upper bound", |_, m| m["total"] = json!(129),
            "World Bank protocol violation: World Bank series metadata pagination is inconsistent: page=1 pages=1 per_page=5000 total=129",
        ),
        (
            "no source", |_, m| m["source"] = json!([]),
            "World Bank protocol violation: series metadata requires exactly one source",
        ),
        (
            "two sources", |_, m| {
                let sources = m["source"].as_array_mut().unwrap();
                sources.push(sources[0].clone());
            },
            "World Bank protocol violation: series metadata requires exactly one source",
        ),
        (
            "no Series concept", |_, m| m["source"][0]["concept"] = json!([]),
            "World Bank protocol violation: series metadata requires exactly one Series concept",
        ),
        (
            "two Series concepts", |_, m| {
                let concepts = m["source"][0]["concept"].as_array_mut().unwrap();
                concepts.push(concepts[0].clone());
            },
            "World Bank protocol violation: series metadata requires exactly one Series concept",
        ),
        (
            "variable identity", |_, m| m["source"][0]["concept"][0]["variable"][0]["id"] = json!("OTHER"),
            "World Bank protocol violation: series metadata variable identity mismatch",
        ),
        (
            "129 received metatypes", |_, m| {
                let fields = m["source"][0]["concept"][0]["variable"][0]["metatype"]
                    .as_array_mut().unwrap();
                // Keep the three authoritative fields and earlier bounded
                // declared total; cardinality fails before field/total checks.
                fields.extend((0..126).map(|n| json!({"id":format!("Extra{n}"),"value":""})));
            },
            "World Bank protocol violation: series metadata exceeds 128 metatypes",
        ),
        (
            "declared metatype total", |_, m| m["total"] = json!(4),
            "World Bank protocol violation: series metadata metatypes do not cover the declared total",
        ),
        (
            "indicator name identity", |_, m| m["source"][0]["concept"][0]["variable"][0]["metatype"][0]["value"] = json!("OTHER"),
            "World Bank protocol violation: series metadata indicator name mismatch",
        ),
        (
            "conflicting structured units", |i, _| i[1][0]["unit"] = json!("USD"),
            "World Bank protocol violation: World Bank indicator and series-metadata units conflict",
        ),
    ];
    for (name, defect, expected_reason) in cases {
        let (mut indicator, mut metadata) = seed();
        defect(&mut indicator, &mut metadata);
        let (result, attempts, _, _) = execute(&indicator, &metadata, false);
        // Every attempt was recorded before the fixture could reject it. Even
        // a swallowed retry or forbidden data request would break this oracle.
        assert_eq!(
            *attempts.lock().unwrap(),
            vec![
                expected_request(WB_INTEGRITY_INDICATOR_URL),
                expected_request(WB_INTEGRITY_METADATA_URL),
            ],
            "{name}: complete external attempt sequence"
        );
        match result {
            Err(ServiceError::FailedPrecondition(reason)) => {
                assert_eq!(reason, expected_reason, "{name}")
            }
            Err(other) => panic!("{name}: wrong public failure stage: {other:?}"),
            Ok(result) => {
                panic!("{name}: metadata defect published partial/empty/success data: {result:?}")
            }
        }
    }
}

#[test]
fn registered_worldbank_rejects_malformed_nested_metadata_before_requesting_data() {
    use magic_market_core::{EconomicObservation, ProviderId, SourcedRecord};
    use serde_json::{json, Value};
    use std::time::{SystemTime, UNIX_EPOCH};
    use time::{format_description::well_known::Rfc3339, Date, Month};

    // Independent lexical/calendar oracle, not the SDK's clock or Core parser.
    // Only the canonical UTC form emitted by this SDK qualifies the echo below.
    fn canonical_utc_nanos(value: &str) -> Option<i128> {
        if !(20..=30).contains(&value.len()) || !value.is_ascii() {
            return None;
        }
        let text = value.strip_suffix('Z')?;
        if text.get(4..5) != Some("-")
            || text.get(7..8) != Some("-")
            || text.get(10..11) != Some("T")
            || text.get(13..14) != Some(":")
            || text.get(16..17) != Some(":")
        {
            return None;
        }
        let year = text.get(..4)?.parse::<i32>().ok()?;
        let month = Month::try_from(text.get(5..7)?.parse::<u8>().ok()?).ok()?;
        let day = text.get(8..10)?.parse::<u8>().ok()?;
        let hour = text.get(11..13)?.parse::<u8>().ok()?;
        let minute = text.get(14..16)?.parse::<u8>().ok()?;
        let second = text.get(17..19)?.parse::<u8>().ok()?;
        let tail = text.get(19..)?;
        let nanos = if tail.is_empty() {
            0
        } else {
            let fraction = tail.strip_prefix('.')?;
            if !(1..=9).contains(&fraction.len())
                || !fraction.bytes().all(|byte| byte.is_ascii_digit())
            {
                return None;
            }
            fraction
                .parse::<u32>()
                .ok()?
                .checked_mul(10_u32.pow(9 - fraction.len() as u32))?
        };
        let instant = Date::from_calendar_date(year, month, day)
            .ok()?
            .with_hms_nano(hour, minute, second, nanos)
            .ok()?
            .assume_utc();
        let canonical = instant.format(&Rfc3339).ok()?;
        (canonical == value).then(|| instant.unix_timestamp_nanos())
    }

    assert_eq!(canonical_utc_nanos("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(
        canonical_utc_nanos("1970-01-01T00:00:01.000000001Z"),
        Some(1_000_000_001)
    );
    for invalid in [
        "1970-01-01T00:00:01.0000000010Z",
        "1970-01-01T00:00:01.10Z",
        "1970-01-01T00:00:01.0Z",
        "1970-01-01T00:00:01+00:00",
        "1970-01-01 00:00:01Z",
        "1970-02-30T00:00:00Z",
        "1970-01-01T00:00:60Z",
        "+970-01-01T00:00:01Z",
    ] {
        assert_eq!(canonical_utc_nanos(invalid), None, "{invalid}");
    }

    let expected_request = |url| {
        HttpRequest::new(
            HttpMethod::Get,
            url,
            vec![("Accept".into(), "application/json".into())],
            vec![],
        )
        .unwrap()
    };
    let execute = |metadata: &Value, serve_data| {
        let attempts = Arc::new(Mutex::new(Vec::new()));
        // This existing external fixture logs a clone at entry, before EVERY
        // guard. Its ordered Vec preserves duplicates, headers and raw body.
        let client = WorldBankClient::with_transport(Arc::new(WorldBankIntegrityFixture {
            indicator: INDICATOR.to_vec(),
            metadata: serde_json::to_vec(metadata).unwrap(),
            serve_data,
            attempts: Arc::clone(&attempts),
        }))
        .unwrap();
        let mut registry = OperationRegistry::all_unadmitted("offline external source only");
        // Existing production registrar is setup; public Execute is the seam.
        register_economic_provider(
            &mut registry,
            client,
            "WorldBank",
            "exact admitted World Bank source 2 USA annual GDP observation",
            4096,
        )
        .unwrap();
        let command = QueryCommand::new(
            "offline-worldbank-nested-metadata-v1",
            Operation::EconomicSeries,
            Some("WorldBank".into()),
            CanonicalPayload::new(
                "magic.market.economic_series.request",
                1,
                br#"{
                    "series":[{"provider":"WorldBank","namespace":"source:2/country:USA",
                               "code":"NY.GDP.MKTP.CD"}],
                    "start":{"Annual":{"year":2024}},
                    "end":{"Annual":{"year":2024}},"max_rows":1
                }"#
                .to_vec(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();
        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let result = registry.execute(command);
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        (result, attempts, before, after)
    };

    // Same fixture/client/registration as every fault. Expected wire is a full
    // independent literal; zero MUST stay Present rather than missing/dropped.
    let metadata: Value = serde_json::from_slice(WB_INTEGRITY_METADATA).unwrap();
    let (result, attempts, before, after) = execute(&metadata, true);
    assert_eq!(
        *attempts.lock().unwrap(),
        vec![
            expected_request(WB_INTEGRITY_INDICATOR_URL),
            expected_request(WB_INTEGRITY_METADATA_URL),
            expected_request(WB_INTEGRITY_DATA_URL),
        ]
    );
    let result = result.unwrap();
    assert_eq!(result.provider, "WorldBank");
    assert!(result.repository_admitted);
    assert!(result.complete);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at, None);
    assert!(before <= after, "system clock reversed during the query");
    let observed = u128::try_from(canonical_utc_nanos(&result.observed_at).unwrap()).unwrap();
    assert!(before.as_nanos() <= observed && observed <= after.as_nanos());
    let expected_batch = format!("WorldBank:economic-series:{}", result.observed_at);
    assert_eq!(result.batch_id, expected_batch);
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.economic_observation");
    assert_eq!(payload.schema_version(), 1);
    let wire: Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(
        wire,
        json!({
            "series":{"provider":"WorldBank","namespace":"source:2/country:USA",
                      "code":"NY.GDP.MKTP.CD"},
            "name":"GDP (current US$)","region_code":"USA","region_name":"United States",
            "period":{"Annual":{"year":2024}},"value":0.0,"unit":"current US$",
            "scale":null,"seasonal_adjustment":null,"status":"Present",
            "released_at":"2026-07-13","revision":null,
            "evidence":{"provider":"WorldBank","source_at":"2026-07-13",
                        "observed_at":result.observed_at.as_str(),"batch_id":expected_batch.as_str()}
        })
    );
    let typed: EconomicObservation = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(typed.provider_id(), ProviderId::WorldBank);
    assert_eq!(typed.evidence_batch_id(), expected_batch);
    assert_eq!(typed.evidence_source_at(), Some("2026-07-13"));
    assert_eq!(
        typed.evidence_observed_at(),
        Some(result.observed_at.as_str())
    );

    // One malformed external value per case; earlier pagination and identity
    // remain valid. The final case preserves the two earlier metatype fields.
    type Case = (&'static str, fn(&mut Value), &'static str);
    let cases: [Case; 7] = [
        (
            "source array",
            |m| m["source"] = json!({}),
            "World Bank protocol violation: missing metadata source array",
        ),
        (
            "source object",
            |m| m["source"][0] = Value::Null,
            "World Bank protocol violation: invalid metadata source object",
        ),
        (
            "concept array",
            |m| m["source"][0]["concept"] = json!({}),
            "World Bank protocol violation: missing metadata concept array",
        ),
        (
            "variable array",
            |m| m["source"][0]["concept"][0]["variable"] = json!({}),
            "World Bank protocol violation: missing metadata variable array",
        ),
        (
            "variable object",
            |m| m["source"][0]["concept"][0]["variable"][0] = Value::Null,
            "World Bank protocol violation: invalid metadata variable object",
        ),
        (
            "metatype array",
            |m| m["source"][0]["concept"][0]["variable"][0]["metatype"] = json!({}),
            "World Bank protocol violation: missing metadata metatype array",
        ),
        (
            "late metatype object",
            |m| m["source"][0]["concept"][0]["variable"][0]["metatype"][2] = Value::Null,
            "World Bank protocol violation: invalid metadata metatype object",
        ),
    ];
    for (name, defect, expected_reason) in cases {
        let mut metadata: Value = serde_json::from_slice(WB_INTEGRITY_METADATA).unwrap();
        defect(&mut metadata);
        let (result, attempts, _, _) = execute(&metadata, false);
        assert_eq!(
            *attempts.lock().unwrap(),
            vec![
                expected_request(WB_INTEGRITY_INDICATOR_URL),
                expected_request(WB_INTEGRITY_METADATA_URL),
            ],
            "{name}: all external attempts, including rejected/retried requests"
        );
        match result {
            Err(ServiceError::FailedPrecondition(reason)) => {
                assert_eq!(reason, expected_reason, "{name}")
            }
            Err(other) => panic!("{name}: wrong public failure stage: {other:?}"),
            Ok(result) => panic!("{name}: malformed metadata published data: {result:?}"),
        }
    }
}
