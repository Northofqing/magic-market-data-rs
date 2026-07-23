pub use magic_exchange_rs::ExchangeError;

#[path = "../src/szse_quote.rs"]
mod szse_quote;

use magic_exchange_rs::{
    ExchangeTransport, HttpMethod, HttpRequest, HttpResponse, SzseClient, SzseConfig,
};
use magic_market_core::{
    AssetClass, DataStatus, Exchange, InstrumentId, OrderBooks, ProviderId, RatioUnit,
    RealtimeQuotes,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use szse_quote::{
    build_quote_url, parse_quote_snapshot, MAX_QUOTE_RESPONSE_BYTES, SZSE_QUOTE_ENDPOINT,
};

const FIXTURE: &[u8] = include_bytes!("../fixtures/szse_quote_000858.json");

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shenzhen, "000858", AssetClass::Equity).unwrap()
}

fn fixture_value() -> Value {
    serde_json::from_slice(FIXTURE).unwrap()
}

fn encoded(mutator: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut value = fixture_value();
    mutator(&mut value);
    serde_json::to_vec(&value).unwrap()
}

fn parse(body: &[u8]) -> Result<szse_quote::SzseQuoteSnapshot, ExchangeError> {
    parse_quote_snapshot(
        &instrument(),
        body,
        "2026-07-23T22:52:17+08:00",
        "szse:quote:test",
    )
}

#[derive(Clone)]
struct Scripted {
    requests: Arc<Mutex<Vec<HttpRequest>>>,
    body: Arc<Vec<u8>>,
}

impl ExchangeTransport for Scripted {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(HttpResponse {
            status: 200,
            final_url: request.url.clone(),
            content_type: Some("application/json;charset=UTF-8".into()),
            body: self.body.as_ref().clone(),
        })
    }
}

fn scripted() -> (SzseClient, Scripted) {
    scripted_with_body(FIXTURE.to_vec())
}

fn scripted_with_body(body: Vec<u8>) -> (SzseClient, Scripted) {
    let transport = Scripted {
        requests: Arc::new(Mutex::new(Vec::new())),
        body: Arc::new(body),
    };
    let client =
        SzseClient::with_transport(SzseConfig::default(), transport.clone()).expect("client");
    (client, transport)
}

#[test]
fn builds_only_the_verified_official_request() {
    assert_eq!(
        build_quote_url(&instrument()).unwrap(),
        format!("{SZSE_QUOTE_ENDPOINT}?marketId=1&code=000858")
    );

    for invalid in [
        InstrumentId::new(Exchange::Shanghai, "000858", AssetClass::Equity).unwrap(),
        InstrumentId::new(Exchange::Shenzhen, "000858", AssetClass::Fund).unwrap(),
        InstrumentId::new(Exchange::Shenzhen, "200858", AssetClass::Equity).unwrap(),
        InstrumentId::new(Exchange::Shenzhen, "00085", AssetClass::Equity).unwrap(),
    ] {
        assert!(matches!(
            build_quote_url(&invalid),
            Err(ExchangeError::InvalidRequest(_))
        ));
    }
}

#[test]
fn public_client_traits_use_the_verified_get_request_and_strict_batches() {
    let (client, transport) = scripted();
    let quote_batch = client.realtime_quotes(&[instrument()]).unwrap();
    assert_eq!(quote_batch.records().len(), 1);
    assert_eq!(quote_batch.records()[0].provider(), ProviderId::Szse);
    assert_eq!(
        quote_batch.provenance().source_at(),
        Some("2026-07-23T15:30:00+08:00")
    );
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, HttpMethod::Get);
    assert_eq!(
        requests[0].url,
        "https://www.szse.cn/api/market/ssjjhq/getTimeData?marketId=1&code=000858"
    );
    drop(requests);

    let (client, _) = scripted();
    let book_batch = client.order_books(&[instrument()]).unwrap();
    assert_eq!(book_batch.records().len(), 1);
    assert_eq!(book_batch.records()[0].provider(), ProviderId::Szse);
    assert!(book_batch.quality().is_complete());

    let (client, _) = scripted();
    assert!(matches!(
        client.realtime_quotes(&[]),
        Err(ExchangeError::InvalidRequest(_))
    ));
    assert!(matches!(
        client.order_books(&[instrument(), instrument()]),
        Err(ExchangeError::InvalidRequest(_))
    ));
}

#[test]
fn public_order_book_batch_reports_missing_tail_levels_as_incomplete() {
    let partial = encoded(|value| {
        value["data"]["sellbuy5"][4] = json!({"price":"0.00","volume":0});
        value["data"]["sellbuy5"][9] = json!({"price":null,"volume":null});
    });
    let (client, _) = scripted_with_body(partial);
    let batch = client.order_books(&[instrument()]).unwrap();
    assert_eq!(batch.records().len(), 1);
    assert_eq!(batch.records()[0].status(), DataStatus::Unavailable);
    assert!(!batch.quality().is_complete());
    assert_eq!(batch.quality().issues().len(), 1);
}

#[test]
fn maps_real_quote_and_five_level_book_without_changing_lots() {
    let (quote, book) = parse(FIXTURE).unwrap().into_parts();
    assert_eq!(quote.instrument(), &instrument());
    assert_eq!(quote.name(), Some("五 粮 液"));
    assert_eq!(quote.price().get(), 74.84);
    assert_eq!(quote.previous_close().unwrap().get(), 74.75);
    assert_eq!(quote.open().unwrap().get(), 74.0);
    assert_eq!(quote.high().unwrap().get(), 74.95);
    assert_eq!(quote.low().unwrap().get(), 73.32);
    assert_eq!(quote.change_percent().unwrap().get(), 0.12);
    assert_eq!(quote.change_percent().unwrap().unit(), RatioUnit::Percent);
    assert_eq!(quote.volume().get(), 370_321.0);
    assert_eq!(quote.amount().unwrap().get(), 2_746_776_997.16);
    assert_eq!(quote.status(), DataStatus::Available);
    assert_eq!(quote.source_at(), Some("2026-07-23T15:30:00+08:00"));
    assert_eq!(quote.observed_at(), "2026-07-23T22:52:17+08:00");
    assert_eq!(quote.provider(), ProviderId::Szse);

    assert_eq!(book.status(), DataStatus::Available);
    assert_eq!(book.asks()[0].price().unwrap().get(), 74.84);
    assert_eq!(book.asks()[0].quantity().unwrap().get(), 506.0);
    assert_eq!(book.asks()[4].price().unwrap().get(), 74.88);
    assert_eq!(book.bids()[0].price().unwrap().get(), 74.83);
    assert_eq!(book.bids()[0].quantity().unwrap().get(), 122.0);
    assert_eq!(book.bids()[4].price().unwrap().get(), 74.79);
    assert_eq!(book.total_ask_quantity().unwrap().get(), 1_239.0);
    assert_eq!(book.total_bid_quantity().unwrap().get(), 475.0);
    assert_eq!(book.source_at(), Some("2026-07-23T15:30:00+08:00"));
    assert_eq!(book.provider(), ProviderId::Szse);
}

#[test]
fn rejects_response_identity_status_and_asset_group_mismatches() {
    for body in [
        encoded(|value| value["code"] = json!("1")),
        encoded(|value| value["message"] = json!("")),
        encoded(|value| value["data"]["code"] = json!("000001")),
        encoded(|value| value["data"]["groupId"] = json!(8)),
        encoded(|value| value["data"]["name"] = json!("\n")),
    ] {
        assert!(matches!(parse(&body), Err(ExchangeError::Schema(_))));
    }
}

#[test]
fn requires_a_valid_consistent_source_market_time() {
    for body in [
        encoded(|value| value["datetime"] = json!("2026-07-23 15:29")),
        encoded(|value| value["data"]["marketTime"] = json!("2026-02-30 15:30:00")),
        encoded(|value| value["data"]["marketTime"] = json!("2026-07-23 25:30:00")),
        encoded(|value| value["data"]["marketTime"] = json!("中中中中")),
    ] {
        assert!(matches!(parse(&body), Err(ExchangeError::Schema(_))));
    }
}

#[test]
fn rejects_invalid_ohlc_delta_percent_amount_and_volume() {
    for body in [
        encoded(|value| value["data"]["high"] = json!("74.80")),
        encoded(|value| value["data"]["low"] = json!("74.10")),
        encoded(|value| value["data"]["delta"] = json!("0.10")),
        encoded(|value| value["data"]["delta"] = json!("0")),
        encoded(|value| value["data"]["deltaPercent"] = json!("0.20")),
        encoded(|value| value["data"]["deltaPercent"] = json!("0")),
        encoded(|value| value["data"]["amount"] = json!(-1)),
        encoded(|value| value["data"]["volume"] = json!(-1)),
        encoded(|value| value["data"]["now"] = json!("not-a-number")),
    ] {
        assert!(matches!(parse(&body), Err(ExchangeError::Schema(_))));
    }
}

#[test]
fn requires_exactly_ten_ordered_non_crossed_levels() {
    let mut nine = fixture_value();
    nine["data"]["sellbuy5"].as_array_mut().unwrap().pop();
    let mut eleven = fixture_value();
    eleven["data"]["sellbuy5"]
        .as_array_mut()
        .unwrap()
        .push(json!({"price":"74.78","volume":1}));

    for body in [
        serde_json::to_vec(&nine).unwrap(),
        serde_json::to_vec(&eleven).unwrap(),
        encoded(|value| value["data"]["sellbuy5"][1]["price"] = json!("74.83")),
        encoded(|value| value["data"]["sellbuy5"][6]["price"] = json!("74.84")),
        encoded(|value| value["data"]["sellbuy5"][5]["price"] = json!("74.85")),
    ] {
        assert!(matches!(parse(&body), Err(ExchangeError::Schema(_))));
    }
}

#[test]
fn maps_atomic_zero_tail_levels_but_rejects_zero_contradictions_and_gaps() {
    let partial = encoded(|value| {
        value["data"]["sellbuy5"][4] = json!({"price":"0.00","volume":0});
        value["data"]["sellbuy5"][9] = json!({"price":null,"volume":null});
    });
    let (_, order_book) = parse(&partial).unwrap().into_parts();
    assert_eq!(order_book.status(), DataStatus::Unavailable);
    assert_eq!(order_book.asks()[4].price(), None);
    assert_eq!(order_book.bids()[4].quantity(), None);
    assert_eq!(order_book.total_ask_quantity().unwrap().get(), 1_153.0);
    assert_eq!(order_book.total_bid_quantity().unwrap().get(), 402.0);

    for body in [
        encoded(|value| value["data"]["sellbuy5"][4] = json!({"price":"0.00","volume":1})),
        encoded(|value| value["data"]["sellbuy5"][4] = json!({"price":"74.88","volume":0})),
        encoded(|value| value["data"]["sellbuy5"][1] = json!({"price":"0.00","volume":0})),
    ] {
        assert!(matches!(parse(&body), Err(ExchangeError::Schema(_))));
    }
}

#[test]
fn rejects_empty_invalid_and_oversized_bodies_without_fixture_fallback() {
    assert!(matches!(parse(b""), Err(ExchangeError::Decode(_))));
    assert!(matches!(parse(b"{not json"), Err(ExchangeError::Decode(_))));
    let oversized = vec![b' '; MAX_QUOTE_RESPONSE_BYTES + 1];
    assert!(matches!(parse(&oversized), Err(ExchangeError::Schema(_))));
}
