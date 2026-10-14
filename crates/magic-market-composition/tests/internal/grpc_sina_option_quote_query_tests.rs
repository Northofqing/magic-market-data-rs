use super::super::register_sina_option_client;
use magic_market_core::{OptionQuote, ProviderId, RatioUnit};
use magic_market_service::{CanonicalPayload, Operation, OperationRegistry, QueryCommand};
use magic_sina_rs::{SinaClient, SinaError, SnapshotTransport};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Native 51-slot Sina source response. The name uses literal GB18030 bytes,
// not a production encoder; deeper source slots are not additional wire claims.
const NATIVE_QUOTE: &[u8] = b"var hq_str_CON_OP_10012127=\"1,0.3241,0.3268,0.3273,2,396,-2.74,2.7500,0.3281,0.3292,0.6446,0.0274,0.3330,16,0.3328,1,0.3292,1,0.3278,1,0.3273,1,0.3241,1,0.3218,1,0.3172,1,0.3171,1,0.3155,1,2026-07-23 14:48:38,0,E 00,EBS,510050,50ETF\xB9\xBA8\xD4\xC22750,6.77,0.3375,0.3153,89,289374.00,M,0.3360,C,2026-08-26,34,2,0.334,-0.0072\";";

// All business values and source evidence are independent literal expectations.
// Only O and B are replaced after their independent clock and grammar checks.
const EXPECTED_WIRE: &str = r#"{
  "contract_code": "10012127",
  "name": "50ETF购8月2750",
  "bid": 0.3241,
  "bid_quantity": 1.0,
  "ask": 0.3273,
  "ask_quantity": 2.0,
  "last": 0.3268,
  "previous_close": 0.3281,
  "open": 0.3292,
  "high": 0.3375,
  "low": 0.3153,
  "upper_limit": 0.6446,
  "lower_limit": 0.0274,
  "strike": 2.75,
  "volume": 89.0,
  "open_interest": 396.0,
  "amount": 289374.0,
  "change": {"value": -2.74, "unit": "Percent"},
  "amplitude": {"value": 6.77, "unit": "Percent"},
  "quote_at": "2026-07-23T14:48:38+08:00",
  "evidence": {
    "provider": "Sina",
    "source_at": "2026-07-23T14:48:38+08:00",
    "observed_at": "O",
    "batch_id": "B"
  }
}"#;

#[derive(Debug, PartialEq, Eq)]
enum SnapshotCall {
    Get { url: String },
    GetWithReferer { url: String, referer: String },
}

struct OneOptionQuoteTransport {
    calls: Arc<Mutex<Vec<SnapshotCall>>>,
}

impl SnapshotTransport for OneOptionQuoteTransport {
    fn get(&self, url: &str) -> Result<Vec<u8>, SinaError> {
        self.calls
            .lock()
            .unwrap()
            .push(SnapshotCall::Get { url: url.into() });
        Err(SinaError::Transport(
            "option fixture requires the exact referer-bearing request".into(),
        ))
    }

    fn get_with_referer(&self, url: &str, referer: &str) -> Result<Vec<u8>, SinaError> {
        let mut calls = self.calls.lock().unwrap();
        calls.push(SnapshotCall::GetWithReferer {
            url: url.into(),
            referer: referer.into(),
        });
        if calls.len() != 1
            || url != "https://hq.sinajs.cn/list=CON_OP_10012127"
            || referer != "https://stock.finance.sina.com.cn/"
        {
            return Err(SinaError::Transport(
                "option fixture received an unexpected or repeated source request".into(),
            ));
        }
        Ok(NATIVE_QUOTE.to_vec())
    }
}

#[test]
fn registered_sina_option_quote_preserves_gb18030_percent_and_source_evidence() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let client = SinaClient::with_transport(OneOptionQuoteTransport {
        calls: Arc::clone(&calls),
    });
    let mut registry = OperationRegistry::all_unadmitted("isolated offline Sina quote fixture");
    // The real private registrar is setup only. No handler, parser, validator,
    // or typed provider result is replaced by the test.
    register_sina_option_client(&mut registry, client, 4096).unwrap();
    let command = QueryCommand::new(
        "sina-option-quote-one",
        Operation::OptionData,
        Some("Sina".into()),
        CanonicalPayload::new(
            "magic.market.option_data.request",
            1,
            br#"{"kind":"quotes","contracts":["10012127"]}"#.to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!command.allows_unadmitted());

    let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let result = registry.execute(command).unwrap();
    let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[SnapshotCall::GetWithReferer {
            url: "https://hq.sinajs.cn/list=CON_OP_10012127".into(),
            referer: "https://stock.finance.sina.com.cn/".into(),
        }]
    );
    assert_eq!(result.provider, "Sina");
    assert!(result.complete);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    // The record has quote source time; the SDK batch does not promote it
    // into outer provenance. This old offline source is not a live witness.
    assert_eq!(result.source_at, None);
    assert_eq!(result.records.len(), 1);

    let (seconds, nanos) = result.observed_at.split_once('.').unwrap();
    assert!(!seconds.is_empty() && seconds.bytes().all(|byte| byte.is_ascii_digit()));
    assert_eq!(nanos.len(), 9);
    assert!(nanos.bytes().all(|byte| byte.is_ascii_digit()));
    let observed = Duration::new(
        seconds.parse::<u64>().unwrap(),
        nanos.parse::<u32>().unwrap(),
    );
    assert!(before <= observed && observed <= after);
    assert_eq!(
        result.batch_id,
        format!("sina-web:{}:option-quotes", result.observed_at)
    );

    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.option_quote");
    assert_eq!(payload.schema_version(), 1);
    assert!(payload.data().len() <= 4096);
    let actual: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(actual.as_object().unwrap().len(), 21);
    let mut expected: serde_json::Value = serde_json::from_str(EXPECTED_WIRE).unwrap();
    expected["evidence"]["observed_at"] = serde_json::Value::String(result.observed_at.clone());
    expected["evidence"]["batch_id"] = serde_json::Value::String(result.batch_id.clone());
    assert_eq!(actual, expected);

    let quote: OptionQuote = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(quote.contract_code().as_str(), "10012127");
    assert_eq!(quote.name().unwrap().as_str(), "50ETF购8月2750");
    assert_eq!(quote.bid_quantity().unwrap().get(), 1.0);
    assert_eq!(quote.ask_quantity().unwrap().get(), 2.0);
    assert_eq!(quote.amount().unwrap().get(), 289374.0);
    assert_eq!(quote.change().unwrap().get(), -2.74);
    assert_eq!(quote.change().unwrap().unit(), RatioUnit::Percent);
    assert_eq!(quote.amplitude().unwrap().get(), 6.77);
    assert_eq!(quote.amplitude().unwrap().unit(), RatioUnit::Percent);
    assert_eq!(
        quote.quote_at().unwrap().as_str(),
        "2026-07-23T14:48:38+08:00"
    );
    assert_eq!(quote.evidence().provider(), ProviderId::Sina);
    assert_eq!(
        quote.evidence().source_at(),
        Some("2026-07-23T14:48:38+08:00")
    );
    assert_eq!(quote.evidence().observed_at(), result.observed_at);
    assert_eq!(quote.evidence().batch_id(), result.batch_id);
    assert_eq!(serde_json::to_value(&quote).unwrap(), expected);
}
