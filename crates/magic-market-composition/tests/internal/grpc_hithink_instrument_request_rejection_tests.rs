use super::register_hithink_client;
use magic_hithink_rs::HithinkClient;
use magic_market_core::{
    AssetClass, DataStatus, Exchange, InstrumentId, ProviderId, Quote, RatioUnit,
};
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use magic_market_transport::{
    HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const QUOTE_URL: &str = "https://fuyao.aicubes.cn/api/a-share/prices/snapshot?thscodes=600396.SH";
const QUOTE_BODY: &[u8] = br#"{"code":0,"message":"success","request_id":"bounded-request-control","data":{"timestamp":null,"total":1,"item":[{"thscode":"600396.SH","ticker":"600396","last_price":13.64,"price_change":0.24,"price_change_ratio_pct":1.79,"open_price":13.45,"high_price":13.75,"low_price":13.4,"prev_price":13.4,"volume":123400.0,"turnover":1680000.0}]}}"#;

#[derive(Clone)]
struct RawSource {
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl HttpTransport for RawSource {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        // Lossless external-entry trace precedes every fixture guard, including rejection.
        self.attempts.lock().unwrap().push(request.clone());
        let headers = vec![
            ("Accept".to_owned(), "application/json".to_owned()),
            ("X-api-key".to_owned(), "offline-fixture-key".to_owned()),
            ("User-Agent".to_owned(), "magic-hithink-rs/0.2".to_owned()),
        ];
        if request.method() != HttpMethod::Get
            || request.url() != QUOTE_URL
            || request.headers() != headers.as_slice()
            || !request.body().is_empty()
        {
            return Err(TransportError::Network(
                "unexpected offline source request".into(),
            ));
        }
        Ok(HttpResponse::new(
            200,
            QUOTE_URL,
            Some("application/json".into()),
            QUOTE_BODY.to_vec(),
        ))
    }
}

fn registry(attempts: &Arc<Mutex<Vec<HttpRequest>>>) -> OperationRegistry {
    let client = HithinkClient::with_transport(
        "offline-fixture-key",
        RawSource {
            attempts: Arc::clone(attempts),
        },
    )
    .unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline request-bound contract");
    register_hithink_client(&mut registry, Arc::new(client), 32_768).unwrap();
    registry
}

fn command(
    operation: Operation,
    schema: &str,
    version: u32,
    instruments: Vec<InstrumentId>,
) -> QueryCommand {
    let body = if operation == Operation::FinancialStatements {
        json!({"instruments": instruments, "kind": "CashFlow"})
    } else {
        json!({"instruments": instruments})
    };
    QueryCommand::new(
        "bounded-instrument-request",
        operation,
        Some("HithinkFinance".into()),
        CanonicalPayload::new(schema, version, serde_json::to_vec(&body).unwrap(), 32_768).unwrap(),
    )
    .unwrap()
}

fn distinct_equities(count: usize) -> Vec<InstrumentId> {
    (0..count)
        .map(|index| {
            InstrumentId::new(
                Exchange::Shanghai,
                format!("{:06}", 600_000 + index),
                AssetClass::Equity,
            )
            .unwrap()
        })
        .collect()
}

fn canonical_nanos(value: &str) -> Option<u128> {
    let (seconds, fraction) = value.split_once('.')?;
    let seconds_value = seconds.parse::<u64>().ok()?;
    if seconds_value.to_string() != seconds
        || fraction.len() != 9
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    u128::from(seconds_value)
        .checked_mul(1_000_000_000)?
        .checked_add(u128::from(fraction.parse::<u32>().ok()?))
}

#[test]
fn registered_hithink_rejects_invalid_instrument_batches_before_source_io() {
    // One request preflight invariant, through real registration and public Execute.
    // The four operation-specific limits are independently fixed by BR-056 and the source contract.
    for (operation, schema, version, upper_plus_one, message) in [
        (
            Operation::RealtimeQuotes,
            "magic.market.realtime_quotes.request",
            1,
            61,
            "quote request must contain 1..=60 instruments",
        ),
        (
            Operation::MarketStatistics,
            "magic.market.market_statistics.request",
            1,
            101,
            "valuation request must contain 1..=100 instruments",
        ),
        (
            Operation::FinancialStatements,
            "magic.market.financial_statements.request",
            2,
            9,
            "financial statement request must contain 1..=8 instruments",
        ),
        (
            Operation::SecurityMetadata,
            "magic.market.security_metadata.request",
            1,
            33,
            "security metadata request must contain 1..=32 instruments",
        ),
    ] {
        for count in [0, upper_plus_one] {
            let attempts = Arc::new(Mutex::new(Vec::new()));
            let registry = registry(&attempts);
            let request = command(operation, schema, version, distinct_equities(count));
            assert!(!request.allows_unadmitted());
            let result = registry.execute(request);
            assert_eq!(
                result,
                Err(ServiceError::InvalidRequest(message.into())),
                "{operation:?}, count={count}"
            );
            assert!(
                attempts.lock().unwrap().is_empty(),
                "invalid {operation:?} count={count} reached external source"
            );
        }
    }

    // Only these SDK operations preflight the entire identity list before their first HTTP request.
    // FinancialStatements and Metadata fetch per instrument and are deliberately excluded here.
    for (operation, schema, message) in [
        (
            Operation::RealtimeQuotes,
            "magic.market.realtime_quotes.request",
            "quote instruments must be unique",
        ),
        (
            Operation::MarketStatistics,
            "magic.market.market_statistics.request",
            "valuation instruments must be unique",
        ),
    ] {
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let registry = registry(&attempts);
        let repeated = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
        let result = registry.execute(command(
            operation,
            schema,
            1,
            vec![repeated.clone(), repeated],
        ));
        assert_eq!(
            result,
            Err(ServiceError::InvalidRequest(message.into())),
            "{operation:?}, duplicate identity"
        );
        assert!(
            attempts.lock().unwrap().is_empty(),
            "duplicate {operation:?} request reached external source"
        );
    }

    // Positive control: a legal singleton is not rejected by an always-failing wrapper or an unavailable registration.
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let registry = registry(&attempts);
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let result = registry
        .execute(command(
            Operation::RealtimeQuotes,
            "magic.market.realtime_quotes.request",
            1,
            vec![instrument.clone()],
        ))
        .unwrap();
    let finished = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let observed =
        canonical_nanos(&result.observed_at).expect("canonical seconds.nanoseconds observation");
    assert!(started <= observed && observed <= finished);
    assert_eq!(result.provider, "HithinkFinance");
    assert_eq!(result.batch_id, "bounded-request-control");
    assert!(result.complete);
    assert_eq!(result.source_at, None);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.quote");
    assert_eq!(payload.schema_version(), 1);
    let wire: Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(wire.as_object().unwrap().len(), 15);
    assert_eq!(
        wire,
        json!({
            "instrument": {"exchange":"Shanghai", "code":"600396", "asset_class":"Equity"},
            "name":null, "price":13.64, "previous_close":13.4,
            "open":13.45, "high":13.75, "low":13.4,
            "change_percent":{"value":1.79,"unit":"Percent"},
            "volume":1234.0, "amount":1680000.0, "status":"Unavailable",
            "source_at":null, "observed_at":result.observed_at.as_str(),
            "provider":"Tonghuashun", "batch_id":"bounded-request-control"
        })
    );
    let typed: Quote = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(typed.instrument(), &instrument);
    assert_eq!(typed.provider(), ProviderId::Tonghuashun);
    assert_eq!(typed.status(), DataStatus::Unavailable);
    assert_eq!(typed.source_at(), None);
    assert_eq!(typed.volume().get(), 1234.0);
    assert_eq!(typed.change_percent().unwrap().unit(), RatioUnit::Percent);
    assert_eq!(
        *attempts.lock().unwrap(),
        vec![HttpRequest::new(
            HttpMethod::Get,
            "https://fuyao.aicubes.cn/api/a-share/prices/snapshot?thscodes=600396.SH",
            vec![
                ("Accept".into(), "application/json".into()),
                ("X-api-key".into(), "offline-fixture-key".into()),
                ("User-Agent".into(), "magic-hithink-rs/0.2".into()),
            ],
            Vec::new(),
        )
        .unwrap()]
    );
}
