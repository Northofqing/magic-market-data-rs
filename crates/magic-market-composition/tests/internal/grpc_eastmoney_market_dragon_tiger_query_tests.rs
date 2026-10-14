use super::register_eastmoney_client;
use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::{DragonTigerDisclosure, ProviderId, SourcedRecord};
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// Synthetic external report bytes, not live admission evidence. The two source
// TRADE_IDs deliberately belong to one security/day; source row order is reverse
// net-amount order and the exact first entry repeats once.
const MARKET_PAGE: &str = r#"{"success":true,"code":0,"result":{"pages":1,"count":3,"data":[
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","SECURITY_TYPE_CODE":"058001001","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"EXPLANATION":"second reason","BILLBOARD_BUY_AMT":80,"BILLBOARD_SELL_AMT":40,"BILLBOARD_NET_AMT":40,"TURNOVERRATE":10},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","SECURITY_TYPE_CODE":"058001001","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"EXPLANATION":"first reason","BILLBOARD_BUY_AMT":100,"BILLBOARD_SELL_AMT":40,"BILLBOARD_NET_AMT":60,"TURNOVERRATE":12.5},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","SECURITY_TYPE_CODE":"058001001","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"EXPLANATION":"first reason","BILLBOARD_BUY_AMT":100,"BILLBOARD_SELL_AMT":40,"BILLBOARD_NET_AMT":60,"TURNOVERRATE":12.5}
]}}"#;

const BUY_ONE: &str = r#"{"success":true,"code":0,"result":{"pages":1,"count":5,"data":[
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"OPERATEDEPT_NAME":"Desk","BUY":100,"SELL":10,"NET":90},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"OPERATEDEPT_NAME":"Desk","BUY":100,"SELL":10,"NET":90},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"OPERATEDEPT_NAME":"Desk","BUY":100,"SELL":10,"NET":90},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"OPERATEDEPT_NAME":"Desk","BUY":100,"SELL":10,"NET":90},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"OPERATEDEPT_NAME":"Desk","BUY":100,"SELL":10,"NET":90}
]}}"#;

const SELL_ONE: &str = r#"{"success":true,"code":0,"result":{"pages":1,"count":5,"data":[
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1001,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75}
]}}"#;

const BUY_TWO: &str = r#"{"success":true,"code":0,"result":{"pages":1,"count":5,"data":[
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":100,"SELL":10,"NET":90},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":100,"SELL":10,"NET":90},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":100,"SELL":10,"NET":90},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":100,"SELL":10,"NET":90},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":100,"SELL":10,"NET":90}
]}}"#;

const SELL_TWO: &str = r#"{"success":true,"code":0,"result":{"pages":1,"count":5,"data":[
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75},
{"SECURITY_CODE":"600396","SECUCODE":"600396.SH","TRADE_DATE":"2026-07-23","TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75}
]}}"#;

// Independent full canonical wire oracle, with only observation/batch fields
// filled after separately validating their exact format and wall-clock bracket.
const EXPECTED: &str = r#"[
{"entry":{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"instrument_name":null,"trading_date":"2026-07-23","reason":"first reason","buy_amount":100.0,"sell_amount":40.0,"net_amount":60.0,"turnover_rate":{"value":12.5,"unit":"Percent"},"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},"seats":[
{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Buy","rank":1,"seat_name":"Desk","amount":100.0,"buy_amount":100.0,"sell_amount":10.0,"net_amount":90.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Buy","rank":2,"seat_name":"Desk","amount":100.0,"buy_amount":100.0,"sell_amount":10.0,"net_amount":90.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Buy","rank":3,"seat_name":"Desk","amount":100.0,"buy_amount":100.0,"sell_amount":10.0,"net_amount":90.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Buy","rank":4,"seat_name":"Desk","amount":100.0,"buy_amount":100.0,"sell_amount":10.0,"net_amount":90.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Buy","rank":5,"seat_name":"Desk","amount":100.0,"buy_amount":100.0,"sell_amount":10.0,"net_amount":90.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Sell","rank":1,"seat_name":"Desk","amount":80.0,"buy_amount":5.0,"sell_amount":80.0,"net_amount":-75.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Sell","rank":2,"seat_name":"Desk","amount":80.0,"buy_amount":5.0,"sell_amount":80.0,"net_amount":-75.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Sell","rank":3,"seat_name":"Desk","amount":80.0,"buy_amount":5.0,"sell_amount":80.0,"net_amount":-75.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Sell","rank":4,"seat_name":"Desk","amount":80.0,"buy_amount":5.0,"sell_amount":80.0,"net_amount":-75.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1001","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Sell","rank":5,"seat_name":"Desk","amount":80.0,"buy_amount":5.0,"sell_amount":80.0,"net_amount":-75.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}}
]},
{"entry":{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"instrument_name":null,"trading_date":"2026-07-23","reason":"second reason","buy_amount":80.0,"sell_amount":40.0,"net_amount":40.0,"turnover_rate":{"value":10.0,"unit":"Percent"},"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},"seats":[
{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Buy","rank":1,"seat_name":"Desk","amount":100.0,"buy_amount":100.0,"sell_amount":10.0,"net_amount":90.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Buy","rank":2,"seat_name":"Desk","amount":100.0,"buy_amount":100.0,"sell_amount":10.0,"net_amount":90.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Buy","rank":3,"seat_name":"Desk","amount":100.0,"buy_amount":100.0,"sell_amount":10.0,"net_amount":90.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Buy","rank":4,"seat_name":"Desk","amount":100.0,"buy_amount":100.0,"sell_amount":10.0,"net_amount":90.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Buy","rank":5,"seat_name":"Desk","amount":100.0,"buy_amount":100.0,"sell_amount":10.0,"net_amount":90.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Sell","rank":1,"seat_name":"Desk","amount":80.0,"buy_amount":5.0,"sell_amount":80.0,"net_amount":-75.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Sell","rank":2,"seat_name":"Desk","amount":80.0,"buy_amount":5.0,"sell_amount":80.0,"net_amount":-75.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Sell","rank":3,"seat_name":"Desk","amount":80.0,"buy_amount":5.0,"sell_amount":80.0,"net_amount":-75.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Sell","rank":4,"seat_name":"Desk","amount":80.0,"buy_amount":5.0,"sell_amount":80.0,"net_amount":-75.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}},
{"entry_id":"600396:2026-07-23:1002","instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-07-23","side":"Sell","rank":5,"seat_name":"Desk","amount":80.0,"buy_amount":5.0,"sell_amount":80.0,"net_amount":-75.0,"evidence":{"provider":"Eastmoney","source_at":"2026-07-23","observed_at":"","batch_id":""}}
]}
]"#;

const URLS: [&str; 5] = [
    "https://datacenter-web.eastmoney.com/api/data/v1/get?reportName=RPT_DAILYBILLBOARD_DETAILSNEW&columns=ALL&filter=%28TRADE_DATE%3D%272026-07-23%27%29%28SECURITY_TYPE_CODE%3D%22058001001%22%29&pageNumber=1&pageSize=500&sortColumns=BILLBOARD_NET_AMT&sortTypes=-1&source=WEB&client=WEB",
    "https://datacenter-web.eastmoney.com/api/data/v1/get?reportName=RPT_BILLBOARD_DAILYDETAILSBUY&columns=ALL&filter=%28SECURITY_CODE%3D%22600396%22%29%28TRADE_DATE%3D%272026-07-23%27%29%28TRADE_ID%3D%221001%22%29&pageNumber=1&pageSize=500&sortColumns=BUY&sortTypes=-1&source=WEB&client=WEB",
    "https://datacenter-web.eastmoney.com/api/data/v1/get?reportName=RPT_BILLBOARD_DAILYDETAILSSELL&columns=ALL&filter=%28SECURITY_CODE%3D%22600396%22%29%28TRADE_DATE%3D%272026-07-23%27%29%28TRADE_ID%3D%221001%22%29&pageNumber=1&pageSize=500&sortColumns=SELL&sortTypes=-1&source=WEB&client=WEB",
    "https://datacenter-web.eastmoney.com/api/data/v1/get?reportName=RPT_BILLBOARD_DAILYDETAILSBUY&columns=ALL&filter=%28SECURITY_CODE%3D%22600396%22%29%28TRADE_DATE%3D%272026-07-23%27%29%28TRADE_ID%3D%221002%22%29&pageNumber=1&pageSize=500&sortColumns=BUY&sortTypes=-1&source=WEB&client=WEB",
    "https://datacenter-web.eastmoney.com/api/data/v1/get?reportName=RPT_BILLBOARD_DAILYDETAILSSELL&columns=ALL&filter=%28SECURITY_CODE%3D%22600396%22%29%28TRADE_DATE%3D%272026-07-23%27%29%28TRADE_ID%3D%221002%22%29&pageNumber=1&pageSize=500&sortColumns=SELL&sortTypes=-1&source=WEB&client=WEB",
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceAttempt {
    method: &'static str,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    maximum_bytes: usize,
}

#[derive(Clone)]
struct DisclosureReports {
    replies: Arc<Mutex<Vec<Option<Vec<u8>>>>>,
    attempts: Arc<Mutex<Vec<SourceAttempt>>>,
}

impl DisclosureReports {
    fn new(last_sell: String) -> Self {
        Self {
            replies: Arc::new(Mutex::new(vec![
                Some(MARKET_PAGE.as_bytes().to_vec()),
                Some(BUY_ONE.as_bytes().to_vec()),
                Some(SELL_ONE.as_bytes().to_vec()),
                Some(BUY_TWO.as_bytes().to_vec()),
                Some(last_sell.into_bytes()),
            ])),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record(
        &self,
        method: &'static str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
        maximum_bytes: usize,
    ) -> usize {
        let mut attempts = self.attempts.lock().unwrap();
        let index = attempts.len();
        attempts.push(SourceAttempt {
            method,
            url: url.to_owned(),
            headers: headers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            body: body.map(<[u8]>::to_vec),
            maximum_bytes,
        });
        index
    }

    fn assert_complete_trace(&self) {
        let expected = URLS
            .iter()
            .map(|url| SourceAttempt {
                method: "GET",
                url: (*url).to_owned(),
                headers: vec![
                    ("Accept".into(), "application/json".into()),
                    ("Referer".into(), "https://data.eastmoney.com/".into()),
                ],
                body: None,
                maximum_bytes: 4 * 1024 * 1024,
            })
            .collect::<Vec<_>>();
        assert_eq!(*self.attempts.lock().unwrap(), expected);
        assert!(self.replies.lock().unwrap().iter().all(Option::is_none));
    }
}

impl EastmoneyTransport for DisclosureReports {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        // Every attempt is externally observed before *any* request guard.
        let index = self.record("GET", url, headers, None, maximum_bytes);
        if URLS.get(index).copied() != Some(url)
            || headers
                != [
                    ("Accept", "application/json"),
                    ("Referer", "https://data.eastmoney.com/"),
                ]
            || maximum_bytes != 4 * 1024 * 1024
        {
            return Err(EastmoneyError::Protocol(
                "offline market-disclosure request mismatch".into(),
            ));
        }
        self.replies
            .lock()
            .unwrap()
            .get_mut(index)
            .and_then(Option::take)
            .ok_or_else(|| {
                EastmoneyError::Protocol("offline market-disclosure reply exhausted".into())
            })
    }

    fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.record("POST", url, headers, Some(body), maximum_bytes);
        Err(EastmoneyError::Protocol(
            "offline market-disclosure POST forbidden".into(),
        ))
    }

    fn get_pdf(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.record("GET_PDF", url, headers, None, maximum_bytes);
        Err(EastmoneyError::Protocol(
            "offline market-disclosure PDF forbidden".into(),
        ))
    }

    fn get_html(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.record("GET_HTML", url, headers, None, maximum_bytes);
        Err(EastmoneyError::Protocol(
            "offline market-disclosure HTML forbidden".into(),
        ))
    }
}

fn registry(fixture: DisclosureReports) -> OperationRegistry {
    let client = EastmoneyClient::with_transport(fixture);
    let mut registry = OperationRegistry::all_unadmitted("offline raw reports only");
    register_eastmoney_client(&mut registry, client, 16_384).unwrap();
    registry
}

fn command() -> QueryCommand {
    QueryCommand::new(
        "offline-eastmoney-market-disclosures-v1",
        Operation::MarketDragonTiger,
        Some("Eastmoney".into()),
        CanonicalPayload::new(
            "magic.market.market_dragon_tiger.request",
            1,
            br#"{"trading_date":"2026-07-23","limit":2}"#.to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn registered_market_dragon_tiger_preserves_two_disclosures_and_rejects_late_sell_identity_atomically(
) {
    let normal = DisclosureReports::new(SELL_TWO.to_owned());
    let normal_registry = registry(normal.clone());
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let result = normal_registry.execute(command()).unwrap();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    normal.assert_complete_trace();
    assert_eq!(result.provider, "Eastmoney");
    assert!(result.complete && result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
    let millis_text = result.observed_at.strip_prefix("unix-ms:").unwrap();
    assert!(!millis_text.is_empty() && millis_text.bytes().all(|byte| byte.is_ascii_digit()));
    let millis = millis_text.parse::<u128>().unwrap();
    assert_eq!(millis.to_string(), millis_text);
    assert!(before <= millis && millis <= after);
    assert_eq!(
        result.batch_id,
        format!(
            "eastmoney-web:market-dragon-tiger-entries:{}",
            result.observed_at
        )
    );
    assert_eq!(result.records.len(), 2);

    let mut expected: serde_json::Value = serde_json::from_str(EXPECTED).unwrap();
    for disclosure in expected.as_array_mut().unwrap() {
        disclosure["entry"]["evidence"]["observed_at"] = result.observed_at.clone().into();
        disclosure["entry"]["evidence"]["batch_id"] = result.batch_id.clone().into();
        for seat in disclosure["seats"].as_array_mut().unwrap() {
            seat["evidence"]["observed_at"] = result.observed_at.clone().into();
            seat["evidence"]["batch_id"] = result.batch_id.clone().into();
        }
    }
    let mut actual = Vec::new();
    for payload in &result.records {
        assert_eq!(payload.schema(), "magic.market.dragon_tiger_disclosure");
        assert_eq!(payload.schema_version(), 1);
        assert!(payload.data().len() <= 16_384);
        actual.push(serde_json::from_slice::<serde_json::Value>(payload.data()).unwrap());
        let typed: DragonTigerDisclosure = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(typed.provider_id(), ProviderId::Eastmoney);
        assert_eq!(typed.evidence_batch_id(), result.batch_id);
        // The generic SourcedRecord contract exposes provider/batch only for
        // this type. Full source times reside in its public entry evidence.
        assert_eq!(typed.evidence_observed_at(), None);
        assert_eq!(typed.evidence_source_at(), None);
        assert_eq!(typed.entry().evidence().observed_at(), result.observed_at);
        assert_eq!(typed.entry().evidence().source_at(), Some("2026-07-23"));
    }
    assert_eq!(serde_json::Value::Array(actual), expected);

    // Change only the last row's TRADE_ID in the final sell response. All
    // preceding reports, including one complete disclosure, remain identical.
    let tail = r#""TRADE_ID":1002,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75}
]}}"#;
    let rejected_tail = r#""TRADE_ID":1999,"OPERATEDEPT_NAME":"Desk","BUY":5,"SELL":80,"NET":-75}
]}}"#;
    assert_eq!(SELL_TWO.matches(tail).count(), 1);
    let broken_sell = SELL_TWO.replacen(tail, rejected_tail, 1);
    assert_eq!(broken_sell.replacen(rejected_tail, tail, 1), SELL_TWO);
    let broken = DisclosureReports::new(broken_sell);
    let error = registry(broken.clone()).execute(command()).unwrap_err();
    broken.assert_complete_trace();
    assert!(
        matches!(error, ServiceError::FailedPrecondition(ref reason)
        if reason == "Eastmoney protocol error: dragon-tiger seat TRADE_ID 1999 does not match discovered entry TRADE_ID 1002"),
        "{error:?}"
    );
}
