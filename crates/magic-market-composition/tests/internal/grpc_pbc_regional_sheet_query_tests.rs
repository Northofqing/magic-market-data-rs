use super::register_economic_provider;
use magic_market_core::EvidenceTimestamp;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use magic_market_transport::{
    HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use magic_pbc_rs::PbcClient;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const ORIGINAL_XLSX: &[u8] =
    include_bytes!("../../../magic-pbc-rs/tests/fixtures/regional-social-financing-2025q1.xlsx");
// Future checked-in TEST-ONLY Rust byte-array fixture. The reviewed recipe emits
// UTF8 LF source to stdout only; ROOT apply_patch and fresh pins must precede test.
include!("../fixtures/pbc_regional_visible_template_2025q1.rs");
const URL: &str =
    "https://www.pbc.gov.cn/diaochatongjisi/fileDir/resource/cms/2025/05/2025051514404575389.xlsx";
const MIME: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

struct WorkbookFixture {
    body: &'static [u8],
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl HttpTransport for WorkbookFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        // Record EVERY external attempt before any guard, retaining all fields
        // and header multiplicity. No SDK, parser or provider result is mocked.
        self.attempts.lock().unwrap().push(request.clone());
        if request.method() != HttpMethod::Get
            || request.url() != URL
            || request.headers() != [("Accept".to_owned(), MIME.to_owned())]
            || !request.body().is_empty()
        {
            return Err(TransportError::InvalidRequest(
                "PBC workbook fixture requires the exact bodyless attachment GET".into(),
            ));
        }
        Ok(HttpResponse::new(
            200,
            URL,
            Some(MIME.into()),
            self.body.to_vec(),
        ))
    }
}

fn registry(body: &'static [u8], attempts: &Arc<Mutex<Vec<HttpRequest>>>) -> OperationRegistry {
    let client = PbcClient::with_transport(Arc::new(WorkbookFixture {
        body,
        attempts: Arc::clone(attempts),
    }))
    .unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline external workbook only");
    // Existing real production registrar is SETUP ONLY. Both assertions enter
    // through public Execute; no private parser/validator is a test entry.
    register_economic_provider(
        &mut registry,
        client,
        "Pbc",
        "admitted money-supply, social-financing and regional PBC series",
        65_536,
    )
    .unwrap();
    registry
}

fn command() -> QueryCommand {
    QueryCommand::new(
        "offline-pbc-regional-sheet-identity-v1",
        Operation::EconomicSeries,
        Some("Pbc".into()),
        CanonicalPayload::new(
            "magic.market.economic_series.request",
            1,
            br#"{"series":[{"provider":"Pbc","namespace":"regional-social-financing-flow","code":"AFRE_FLOW"}],"start":{"Quarterly":{"year":2025,"quarter":1}},"end":{"Quarterly":{"year":2025,"quarter":1}},"max_rows":31}"#.to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap()
}

fn assert_one_exact_get(attempts: &Arc<Mutex<Vec<HttpRequest>>>) {
    let attempts = attempts.lock().unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].method(), HttpMethod::Get);
    assert_eq!(attempts[0].url(), URL);
    assert_eq!(
        attempts[0].headers(),
        [("Accept".to_owned(), MIME.to_owned())]
    );
    assert_eq!(attempts[0].body(), b"");
}

#[test]
fn registered_pbc_regional_series_rejects_visible_template_without_publishing_rows() {
    let control_attempts = Arc::new(Mutex::new(Vec::new()));
    let control_registry = registry(ORIGINAL_XLSX, &control_attempts);
    let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let control = control_registry.execute(command()).unwrap();
    let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    assert_one_exact_get(&control_attempts);
    assert_eq!(control.provider, "Pbc");
    assert!(control.complete);
    assert!(control.repository_admitted);
    assert_eq!(control.diagnostic_blocker, None);
    assert_eq!(control.source_at, None);
    assert_eq!(control.records.len(), 31);
    let observed = EvidenceTimestamp::parse_instant(&control.observed_at).unwrap();
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
    assert_eq!(
        control.batch_id,
        format!(
            "pbc-regional-social-financing:2025-q1:{}",
            control.observed_at
        )
    );

    // Independently transcribed original worksheet A10:B40, including the
    // source's 陕西 Shanxi spelling. List order is an explicit literal expected
    // wire order; no production mapper/parser or expected-value sorting runs.
    let expected_regions = [
        ("上海 Shanghai", 4605.0),
        ("云南 Yunnan", 1956.0),
        ("内蒙古 Inner Mongolia", 1354.0),
        ("北京 Beijing", 8426.0),
        ("吉林 Jilin", 1080.0),
        ("四川 Sichuan", 8532.0),
        ("天津 Tianjin", 1412.0),
        ("宁夏 Ningxia", 486.0),
        ("安徽 Anhui", 5362.0),
        ("山东 Shandong", 11678.0),
        ("山西 Shanxi", 2770.0),
        ("广东 Guangdong", 10876.0),
        ("广西 Guangxi", 2365.0),
        ("新疆 Xinjiang", 1507.0),
        ("江苏 Jiangsu", 20646.0),
        ("江西 Jiangxi", 3739.0),
        ("河北 Hebei", 7007.0),
        ("河南 Henan", 5513.0),
        ("浙江 Zhejiang", 13686.0),
        ("海南 Hainan", 430.0),
        ("湖北 Hubei", 6054.0),
        ("湖南 Hunan", 3904.0),
        ("甘肃 Gansu", 1395.0),
        ("福建 Fujian", 3996.0),
        ("西藏 Tibet", 331.0),
        ("贵州 Guizhou", 1959.0),
        ("辽宁 Liaoning", 2126.0),
        ("重庆 Chongqing", 2734.0),
        ("陕西 Shanxi", 2345.0),
        ("青海 Qinghai", 76.0),
        ("黑龙江 Heilongjiang", 1081.0),
    ];
    let expected = expected_regions
        .into_iter()
        .map(|(region, value)| {
            serde_json::json!({
                "series": {"provider":"Pbc", "namespace":"regional-social-financing-flow", "code":"AFRE_FLOW"},
                "name": "地区社会融资规模增量",
                "region_code": null,
                "region_name": region,
                "period": {"Quarterly":{"year":2025,"quarter":1}},
                "value": value,
                "unit": "亿元人民币",
                "scale": "100 million yuan",
                "seasonal_adjustment": null,
                "status": "Present",
                "released_at": null,
                "revision": {"kind":"Preliminary", "label":"Note2: The statistics is provisional."},
                "evidence": {"provider":"Pbc", "source_at":null, "observed_at":control.observed_at, "batch_id":control.batch_id}
            })
        })
        .collect::<Vec<_>>();
    let actual = control
        .records
        .iter()
        .map(|payload| {
            assert_eq!(payload.schema(), "magic.market.economic_observation");
            assert_eq!(payload.schema_version(), 1);
            serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap()
        })
        .collect::<Vec<_>>();
    // Full 31-row / 13-field equality rejects omissions, extra fields, wrong
    // region/value/order, fabricated source time and altered source spelling.
    assert_eq!(actual, expected);

    let rejection_attempts = Arc::new(Mutex::new(Vec::new()));
    // Separate REAL SDK instance preserves its normal one-second gate without
    // sleeping, changing policy or comparing unrelated per-client pacing.
    let rejection_registry = registry(VISIBLE_TEMPLATE_XLSX, &rejection_attempts);
    let error = rejection_registry.execute(command()).unwrap_err();
    assert_eq!(
        error,
        ServiceError::FailedPrecondition(
            "PBC source contract failed: regional workbook sheet catalog or visibility differs from the audited two-sheet contract".into()
        )
    );
    assert_one_exact_get(&rejection_attempts);
    // Err publishes neither the normal source rows nor a successful empty or
    // partial QueryResult for the wrong-visible-template response.
}
