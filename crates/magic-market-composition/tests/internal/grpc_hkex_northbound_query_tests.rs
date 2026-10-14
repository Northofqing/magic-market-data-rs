use super::register_hkex_northbound_client;
use magic_exchange_rs::{
    ExchangeError, ExchangeTransport, HkexClient, HkexConfig, HttpMethod, HttpRequest, HttpResponse,
};
use magic_market_core::NorthboundDailyStat;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAXIMUM_PAYLOAD_BYTES: usize = 16 * 1024;
const DAILY: &[u8] = include_bytes!("../../../magic-exchange-rs/fixtures/hkex_daily_20260722.js");
const DAILY_URL: &str = "https://www.hkex.com.hk/eng/csm/DailyStat/data_tab_daily_20260722e.js";

fn expected_request() -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: DAILY_URL.to_owned(),
        headers: vec![
            (
                "User-Agent".to_owned(),
                "Mozilla/5.0 (compatible; magic-exchange-rs/0.2; read-only official-data probe)"
                    .to_owned(),
            ),
            ("Accept".to_owned(), "application/javascript".to_owned()),
        ],
        body: Vec::new(),
    }
}

#[derive(Clone)]
struct NorthboundJsFixture {
    body: Vec<u8>,
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl NorthboundJsFixture {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ExchangeTransport for NorthboundJsFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        // Record every external entry losslessly, before any fixture guard.
        self.attempts
            .lock()
            .map_err(|_| ExchangeError::Transport("offline HKEX trace poisoned".to_owned()))?
            .push(request.clone());
        if request != &expected_request() {
            return Err(ExchangeError::Transport(
                "unexpected offline HKEX method, URL, ordered headers or body".to_owned(),
            ));
        }
        if self.body.len() > 8 * 1024 * 1024 {
            return Err(ExchangeError::Transport(
                "offline HKEX response exceeds 8 MiB".to_owned(),
            ));
        }
        Ok(HttpResponse {
            status: 200,
            final_url: DAILY_URL.to_owned(),
            content_type: Some("application/javascript; charset=UTF-8".to_owned()),
            body: self.body.clone(),
        })
    }
}

fn registry(fixture: NorthboundJsFixture) -> OperationRegistry {
    let config = HkexConfig::default();
    assert_eq!(config.timeout, Duration::from_secs(15));
    assert_eq!(config.minimum_interval, Duration::from_secs(1));
    let client = HkexClient::with_transport(config, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline HKEX public contract");
    // Same concrete production registrar is setup, not the assertion entry.
    register_hkex_northbound_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::NorthboundDaily,
        Some("Hkex".to_owned()),
        CanonicalPayload::new(
            "magic.market.northbound_daily.request",
            1,
            br#"{"trading_date":"2026-07-22","channel":"Shenzhen"}"#.to_vec(),
            MAXIMUM_PAYLOAD_BYTES,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!command.allows_unadmitted());
    command
}

fn clock_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

#[test]
fn registered_northbound_daily_rejects_tail_rank_without_publishing_rows() {
    assert_eq!(DAILY.len(), 1534);
    assert_eq!(
        format!("{:x}", Sha256::digest(DAILY)),
        "6e14c066c91c478af66132b18893fea61a82eb9c55e82af883166f16c448b36e"
    );
    let source = std::str::from_utf8(DAILY).unwrap();
    let tail = r#"[["10", "2156", "TONGFU MICROELECTRONICS", "1,878,077,820"]]"#;
    let repeated_rank = r#"[["9", "2156", "TONGFU MICROELECTRONICS", "1,878,077,820"]]"#;
    assert_eq!(source.matches(tail).count(), 1);
    let rejected_source = source.replace(tail, repeated_rank);
    assert_eq!(rejected_source.len(), 1533);
    assert_eq!(
        format!("{:x}", Sha256::digest(rejected_source.as_bytes())),
        "cb2d782a947dab60b1097fca578dfa90444befbb82132e1aa66e889a613ce22b"
    );
    assert_eq!(rejected_source.matches(repeated_rank).count(), 1);
    assert_eq!(
        rejected_source.replace(repeated_rank, tail).as_bytes(),
        DAILY
    );

    let normal_fixture = NorthboundJsFixture::new(DAILY);
    let normal_registry = registry(normal_fixture.clone());
    let before = clock_millis();
    let result = normal_registry
        .execute(command("hkex-northbound-complete-control"))
        .unwrap();
    let after = clock_millis();
    assert_eq!(result.provider, "Hkex");
    assert_eq!(result.batch_id, "hkex-northbound-2026-07-22-szse");
    assert!(result.complete);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-22"));
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.records.len(), 1);
    assert_eq!(
        normal_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );

    // Source date is not an invented intraday instant. O is actual observation.
    let digits = result.observed_at.strip_prefix("unix:").unwrap();
    assert!(!digits.is_empty());
    assert!(digits.bytes().all(|byte| byte.is_ascii_digit()));
    let observed_millis = digits.parse::<u128>().unwrap();
    assert_eq!(digits, observed_millis.to_string());
    assert!(before <= observed_millis && observed_millis <= after);

    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.northbound_daily_stat");
    assert_eq!(payload.schema_version(), 1);
    let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    // Known literal CNY/quantity facts, names, order and missing-quota semantics;
    // neither the production mapper nor a typed provider object builds the oracle.
    let expected = serde_json::json!({
        "trading_date": "2026-07-22",
        "channel": "Shenzhen",
        "total_turnover": 200283710000.0,
        "total_trade_count": 9188740.0,
        "quota_balance": "Unavailable",
        "etf_turnover": 2696070000.0,
        "top_turnover": [
            {"rank":1, "instrument":{"exchange":"Shenzhen", "code":"300308", "asset_class":"Equity"}, "name":"ZHONGJI INNOLIGHT", "total_turnover":4878825523.0},
            {"rank":2, "instrument":{"exchange":"Shenzhen", "code":"002371", "asset_class":"Equity"}, "name":"NAURA TECHNOLOGY GROUP", "total_turnover":4742018935.0},
            {"rank":3, "instrument":{"exchange":"Shenzhen", "code":"000938", "asset_class":"Equity"}, "name":"UNISPLENDOUR CORP", "total_turnover":2987950632.0},
            {"rank":4, "instrument":{"exchange":"Shenzhen", "code":"000977", "asset_class":"Equity"}, "name":"IEIT SYSTEMS", "total_turnover":2867287250.0},
            {"rank":5, "instrument":{"exchange":"Shenzhen", "code":"300408", "asset_class":"Equity"}, "name":"CHAOZHOU THREE-CIRCLE GROUP", "total_turnover":2741869984.0},
            {"rank":6, "instrument":{"exchange":"Shenzhen", "code":"002384", "asset_class":"Equity"}, "name":"SUZHOU DONGSHAN PRECISION MANUFACTURING", "total_turnover":2461993715.0},
            {"rank":7, "instrument":{"exchange":"Shenzhen", "code":"300750", "asset_class":"Equity"}, "name":"CONTEMPORARY AMPEREX TECHNOLOGY", "total_turnover":2184005425.0},
            {"rank":8, "instrument":{"exchange":"Shenzhen", "code":"300502", "asset_class":"Equity"}, "name":"EOPTOLINK TECHNOLOGY", "total_turnover":1939910246.0},
            {"rank":9, "instrument":{"exchange":"Shenzhen", "code":"002281", "asset_class":"Equity"}, "name":"ACCELINK TECHNOLOGIES", "total_turnover":1879164309.0},
            {"rank":10, "instrument":{"exchange":"Shenzhen", "code":"002156", "asset_class":"Equity"}, "name":"TONGFU MICROELECTRONICS", "total_turnover":1878077820.0}
        ],
        "evidence": {
            "provider": "Hkex",
            "source_at": "2026-07-22",
            "observed_at": result.observed_at.clone(),
            "batch_id": "hkex-northbound-2026-07-22-szse"
        }
    });
    assert_eq!(row, expected);
    let typed: NorthboundDailyStat = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), expected);

    let rejected_fixture = NorthboundJsFixture::new(rejected_source.as_bytes());
    let rejected_registry = registry(rejected_fixture.clone());
    let error = rejected_registry
        .execute(command("hkex-northbound-invalid-tail-rank"))
        .unwrap_err();
    assert_eq!(
        error,
        ServiceError::FailedPrecondition(
            "official exchange schema drift: HKEX northbound ranks must be ordered 1 through 10"
                .to_owned(),
        )
    );
    assert_eq!(
        rejected_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request()]
    );
}
