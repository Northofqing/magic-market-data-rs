use super::register_baidu_technical_bars_client;
use magic_baidu_rs::{BaiduClient, BaiduError, BaiduTransport, HttpRequest};
use magic_market_core::TechnicalBar;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MAXIMUM_PAYLOAD_BYTES: usize = 16 * 1024;
const MAXIMUM_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const BAIDU_URL: &str = "https://finance.pae.baidu.com/selfselect/getstockquotation?all=1&isIndex=false&isBk=false&isBlock=false&isFutures=false&isStock=true&newFormat=1&group=quotation_kline_ab&finClientType=pc&code=600396&start_time=&ktype=1";

// Existing SDK raw fixture: both selected source rows are valid, not locally computed.
const NORMAL_JSON: &str = r#"{
      "ResultCode": "0",
      "Result": {"newMarketData": {
        "keys": ["timestamp","time","open","close","volume","high","low","amount","ma5avgprice","ma10avgprice","ma20avgprice"],
        "marketData": "1784649600,2026-07-22,14.92,14.92,10000,15.00,14.80,149200.00,--,--,--;1784736000,2026-07-23,15.30,16.41,341780059,16.41,14.85,5352355411.00,13.87,13.02,13.40"
      }}
    }"#;
// Only the later row's source high changes. No keys, dates, count or first-row changes.
const INVALID_TAIL_JSON: &str = r#"{
      "ResultCode": "0",
      "Result": {"newMarketData": {
        "keys": ["timestamp","time","open","close","volume","high","low","amount","ma5avgprice","ma10avgprice","ma20avgprice"],
        "marketData": "1784649600,2026-07-22,14.92,14.92,10000,15.00,14.80,149200.00,--,--,--;1784736000,2026-07-23,15.30,16.41,341780059,16.00,14.85,5352355411.00,13.87,13.02,13.40"
      }}
    }"#;

#[derive(Clone, Debug, PartialEq, Eq)]
struct RequestAttempt {
    method: &'static str,
    url: String,
    headers: Vec<(String, String)>,
}

fn expected_attempt() -> RequestAttempt {
    RequestAttempt {
        method: "GET",
        url: BAIDU_URL.to_owned(),
        headers: vec![
            (
                "Accept".to_owned(),
                "application/vnd.finance-web.v1+json".to_owned(),
            ),
            (
                "Origin".to_owned(),
                "https://gushitong.baidu.com".to_owned(),
            ),
            (
                "Referer".to_owned(),
                "https://gushitong.baidu.com/".to_owned(),
            ),
            ("User-Agent".to_owned(), "magic-baidu-rs/0.2".to_owned()),
        ],
    }
}

#[derive(Clone)]
struct TechnicalBarsFixture {
    body: Vec<u8>,
    attempts: Arc<Mutex<Vec<RequestAttempt>>>,
}

impl TechnicalBarsFixture {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl BaiduTransport for TechnicalBarsFixture {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, BaiduError> {
        // The trait exposes GET, raw URL, ordered headers and returned bytes only.
        let attempt = RequestAttempt {
            method: "GET",
            url: request.url().to_owned(),
            headers: request.headers().to_vec(),
        };
        // Preserve every external entry, including rejected attempts, before any guard.
        self.attempts
            .lock()
            .map_err(|_| BaiduError::Transport("offline technical-bars trace poisoned".to_owned()))?
            .push(attempt.clone());
        if attempt != expected_attempt() {
            return Err(BaiduError::Transport(
                "unexpected offline technical-bars method, raw URL or ordered headers".to_owned(),
            ));
        }
        if self.body.len() > MAXIMUM_SOURCE_BYTES {
            return Err(BaiduError::Transport(
                "offline technical-bars fixture exceeds response bound".to_owned(),
            ));
        }
        Ok(self.body.clone())
    }
}

fn registry(fixture: TechnicalBarsFixture) -> OperationRegistry {
    // with_transport's existing ZERO interval is fixture setup, not production pacing proof.
    let client = BaiduClient::with_transport(fixture);
    let mut registry = OperationRegistry::all_unadmitted("offline Baidu technical-bars contract");
    // Real concrete registrar is setup only; every observation crosses public Execute.
    register_baidu_technical_bars_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::TechnicalBars,
        Some("Baidu".to_owned()),
        CanonicalPayload::new(
            "magic.market.technical_bars.request",
            1,
            br#"{"instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"interval":"Day","start":null,"end":null,"limit":2}"#.to_vec(),
            MAXIMUM_PAYLOAD_BYTES,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!command.allows_unadmitted());
    command
}

fn clock_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

#[test]
fn registered_technical_bars_rejects_invalid_later_ohlc_without_publishing_prefix() {
    assert_eq!(NORMAL_JSON.len(), 388);
    assert_eq!(
        format!("{:x}", Sha256::digest(NORMAL_JSON.as_bytes())),
        "f3f2ccbc5610fc3d829355c4808e8e3a902486a64e547464280ae393430cab47"
    );
    assert_eq!(INVALID_TAIL_JSON.len(), 388);
    assert_eq!(
        format!("{:x}", Sha256::digest(INVALID_TAIL_JSON.as_bytes())),
        "ece7af9b94d055543eba54fe01c6e4bdadd0b3b6d40b2970eab7ceac79daad1c"
    );
    assert_eq!(NORMAL_JSON.matches("341780059,16.41,14.85").count(), 1);
    assert_eq!(
        INVALID_TAIL_JSON,
        NORMAL_JSON.replace("341780059,16.41,14.85", "341780059,16.00,14.85")
    );

    let normal_fixture = TechnicalBarsFixture::new(NORMAL_JSON.as_bytes());
    let normal_registry = registry(normal_fixture.clone());
    let before = clock_nanos();
    let result = normal_registry
        .execute(command("baidu-technical-bars-complete-control"))
        .unwrap();
    let after = clock_nanos();
    assert_eq!(result.provider, "Baidu");
    assert!(result.complete);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
    assert_eq!(result.records.len(), 2);
    assert_eq!(
        normal_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_attempt()]
    );

    // Independent syntax, checked epoch conversion and actual observation-time window.
    let (seconds, fraction) = result.observed_at.split_once('.').unwrap();
    assert!(!seconds.is_empty());
    assert!(seconds.bytes().all(|byte| byte.is_ascii_digit()));
    assert_eq!(fraction.len(), 9);
    assert!(fraction.bytes().all(|byte| byte.is_ascii_digit()));
    let observed_nanos = seconds
        .parse::<u128>()
        .unwrap()
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(fraction.parse::<u128>().unwrap()))
        .unwrap();
    assert!(before <= observed_nanos && observed_nanos <= after);
    let expected_batch_id = format!("baidu-pae:{}:technical-bars", result.observed_at);
    assert_eq!(result.batch_id, expected_batch_id);

    // All source numbers are literals; the SDK's parser does not construct this oracle.
    let expected_rows = [
        serde_json::json!({
            "bar": {
                "instrument": {"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
                "interval": "Day",
                "bar_start": "2026-07-22",
                "bar_end": "2026-07-22",
                "open": 14.92,
                "high": 15.0,
                "low": 14.80,
                "close": 14.92,
                "volume": 100.0,
                "amount": 149200.0,
                "adjustment": "Unadjusted",
                "source_at": "2026-07-22",
                "observed_at": result.observed_at.clone(),
                "provider": "Baidu",
                "batch_id": expected_batch_id.clone()
            },
            "ma5": null,
            "ma10": null,
            "ma20": null,
            "evidence": {
                "provider": "Baidu",
                "source_at": "2026-07-22",
                "observed_at": result.observed_at.clone(),
                "batch_id": expected_batch_id.clone()
            }
        }),
        serde_json::json!({
            "bar": {
                "instrument": {"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
                "interval": "Day",
                "bar_start": "2026-07-23",
                "bar_end": "2026-07-23",
                "open": 15.30,
                "high": 16.41,
                "low": 14.85,
                "close": 16.41,
                "volume": 3417800.59,
                "amount": 5352355411.0,
                "adjustment": "Unadjusted",
                "source_at": "2026-07-23",
                "observed_at": result.observed_at.clone(),
                "provider": "Baidu",
                "batch_id": expected_batch_id.clone()
            },
            "ma5": 13.87,
            "ma10": 13.02,
            "ma20": 13.40,
            "evidence": {
                "provider": "Baidu",
                "source_at": "2026-07-23",
                "observed_at": result.observed_at.clone(),
                "batch_id": expected_batch_id.clone()
            }
        }),
    ];
    for (payload, expected) in result.records.iter().zip(expected_rows.iter()) {
        assert_eq!(payload.schema(), "magic.market.technical_bar");
        assert_eq!(payload.schema_version(), 1);
        let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(row.as_object().unwrap().len(), 5);
        assert_eq!(row["bar"].as_object().unwrap().len(), 15);
        assert_eq!(row["bar"]["instrument"].as_object().unwrap().len(), 3);
        assert_eq!(row["evidence"].as_object().unwrap().len(), 4);
        assert_eq!(row, *expected);
        let typed: TechnicalBar = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(serde_json::to_value(&typed).unwrap(), *expected);
    }

    let rejected_fixture = TechnicalBarsFixture::new(INVALID_TAIL_JSON.as_bytes());
    let rejected_registry = registry(rejected_fixture.clone());
    let error = rejected_registry
        .execute(command("baidu-technical-bars-invalid-later-source-high"))
        .unwrap_err();
    assert_eq!(
        error,
        ServiceError::FailedPrecondition(
            "Baidu protocol error: technical-bar OHLC values have an inconsistent range".to_owned()
        )
    );
    assert_eq!(
        rejected_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_attempt()]
    );
}
