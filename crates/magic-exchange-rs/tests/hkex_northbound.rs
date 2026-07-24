use magic_exchange_rs::{
    ExchangeError, ExchangeTransport, HkexClient, HkexConfig, HttpMethod, HttpRequest, HttpResponse,
};
use magic_market_core::{
    Exchange, NorthboundChannel, NorthboundDailyRequest, NorthboundDailyStatistics,
    NorthboundQuotaBalance, ProviderId,
};
use std::sync::{Arc, Mutex};

const DAILY: &[u8] = include_bytes!("../fixtures/hkex_daily_20260722.js");

#[derive(Clone)]
struct Scripted {
    body: Arc<Mutex<Vec<u8>>>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl Scripted {
    fn new(body: &[u8]) -> Self {
        Self {
            body: Arc::new(Mutex::new(body.to_vec())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ExchangeTransport for Scripted {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(HttpResponse {
            status: 200,
            final_url: request.url.clone(),
            content_type: Some("application/javascript".into()),
            body: self.body.lock().unwrap().clone(),
        })
    }
}

fn request() -> NorthboundDailyRequest {
    NorthboundDailyRequest::new(
        magic_market_core::IsoDate::new("2026-07-22").unwrap(),
        NorthboundChannel::Shenzhen,
    )
}

#[test]
fn maps_official_summary_sentinel_top_ten_and_source_units() {
    let transport = Scripted::new(DAILY);
    let client =
        HkexClient::with_transport(HkexConfig::default(), transport.clone()).expect("client");
    let batch = client
        .northbound_daily_statistics(&request())
        .expect("daily statistics");

    assert_eq!(batch.records().len(), 1);
    let record = &batch.records()[0];
    assert_eq!(record.channel(), NorthboundChannel::Shenzhen);
    assert_eq!(record.total_turnover().get(), 200_283_710_000.0);
    assert_eq!(record.total_trade_count().get(), 9_188_740.0);
    assert_eq!(record.quota_balance(), NorthboundQuotaBalance::Unavailable);
    assert_eq!(record.etf_turnover().get(), 2_696_070_000.0);
    assert_eq!(record.top_turnover().len(), 10);
    assert_eq!(record.top_turnover()[1].instrument().code(), "002371");
    assert_eq!(
        record.top_turnover()[1].instrument().exchange(),
        Exchange::Shenzhen
    );
    assert_eq!(
        record.top_turnover()[1].total_turnover().get(),
        4_742_018_935.0
    );
    assert_eq!(record.evidence().provider(), ProviderId::Hkex);
    assert_eq!(record.evidence().source_at(), Some("2026-07-22"));
    assert_eq!(batch.provenance().source(), "hkex-official");
    assert_eq!(batch.provenance().source_at(), Some("2026-07-22"));

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, HttpMethod::Get);
    assert_eq!(
        requests[0].url,
        "https://www.hkex.com.hk/eng/csm/DailyStat/data_tab_daily_20260722e.js"
    );
    assert!(!requests[0]
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("cookie")
            || name.eq_ignore_ascii_case("authorization")));
}

#[test]
fn rejects_date_channel_calendar_schema_and_incomplete_top_ten() {
    let wrong_date = String::from_utf8(DAILY.to_vec())
        .unwrap()
        .replace("2026-07-22", "2026-07-21");
    let client =
        HkexClient::with_transport(HkexConfig::default(), Scripted::new(wrong_date.as_bytes()))
            .unwrap();
    assert!(matches!(
        client.northbound_daily_statistics(&request()),
        Err(ExchangeError::Schema(_))
    ));

    let closed = String::from_utf8(DAILY.to_vec())
        .unwrap()
        .replace("\"tradingDay\": 1", "\"tradingDay\": 0");
    let client =
        HkexClient::with_transport(HkexConfig::default(), Scripted::new(closed.as_bytes()))
            .unwrap();
    assert!(matches!(
        client.northbound_daily_statistics(&request()),
        Err(ExchangeError::Incomplete(_))
    ));

    let wrong_schema = String::from_utf8(DAILY.to_vec())
        .unwrap()
        .replace("\"DQB\"", "\"Net Inflow\"");
    let client = HkexClient::with_transport(
        HkexConfig::default(),
        Scripted::new(wrong_schema.as_bytes()),
    )
    .unwrap();
    assert!(matches!(
        client.northbound_daily_statistics(&request()),
        Err(ExchangeError::Schema(_))
    ));

    let incomplete = String::from_utf8(DAILY.to_vec())
        .unwrap()
        .replace(
            ",\n            {\"td\": [[\"10\", \"2156\", \"TONGFU MICROELECTRONICS\", \"1,878,077,820\"]]}",
            "",
        );
    let client =
        HkexClient::with_transport(HkexConfig::default(), Scripted::new(incomplete.as_bytes()))
            .unwrap();
    assert!(matches!(
        client.northbound_daily_statistics(&request()),
        Err(ExchangeError::Incomplete(_))
    ));

    let shanghai = NorthboundDailyRequest::new(
        magic_market_core::IsoDate::new("2026-07-22").unwrap(),
        NorthboundChannel::Shanghai,
    );
    let client = HkexClient::with_transport(HkexConfig::default(), Scripted::new(DAILY)).unwrap();
    assert!(matches!(
        client.northbound_daily_statistics(&shanghai),
        Err(ExchangeError::Incomplete(_))
    ));
}

#[test]
fn rejects_duplicate_codes_negative_values_and_non_javascript() {
    let duplicate = String::from_utf8(DAILY.to_vec())
        .unwrap()
        .replace("\"2371\"", "\"300308\"");
    let client =
        HkexClient::with_transport(HkexConfig::default(), Scripted::new(duplicate.as_bytes()))
            .unwrap();
    assert!(matches!(
        client.northbound_daily_statistics(&request()),
        Err(ExchangeError::Schema(_))
    ));

    let negative = String::from_utf8(DAILY.to_vec())
        .unwrap()
        .replace("\"200,283.71\"", "\"-1\"");
    let client =
        HkexClient::with_transport(HkexConfig::default(), Scripted::new(negative.as_bytes()))
            .unwrap();
    assert!(matches!(
        client.northbound_daily_statistics(&request()),
        Err(ExchangeError::Schema(_))
    ));

    #[derive(Clone)]
    struct Html;
    impl ExchangeTransport for Html {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
            Ok(HttpResponse {
                status: 200,
                final_url: request.url.clone(),
                content_type: Some("text/html".into()),
                body: DAILY.to_vec(),
            })
        }
    }
    let client = HkexClient::with_transport(HkexConfig::default(), Html).unwrap();
    assert!(matches!(
        client.northbound_daily_statistics(&request()),
        Err(ExchangeError::Schema(_))
    ));
}
