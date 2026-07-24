use magic_exchange_rs::{
    parse_sse_response, parse_szse_detail_response, parse_szse_list_response, ExchangeError,
    ExchangeTransport, HttpRequest, HttpResponse, OfficialDragonTigerRequest, SseClient, SseConfig,
    SzseClient, SzseConfig, SzseDragonTigerDetailKey, MAX_DRAGON_TIGER_RESPONSE_BYTES,
};
use magic_market_core::{
    AssetClass, DragonTigerData, DragonTigerSide, Exchange, InstrumentId, InstrumentSignalRequest,
    IsoDate, PositiveU32, ProviderId,
};
use std::io::Read;
use std::sync::{Arc, Mutex};

const OBSERVED_AT: &str = "2026-07-23T14:00:00+08:00";
const BATCH_ID: &str = "official-dragon-tiger-test";

fn request(exchange: Exchange, code: &str, date: &str) -> OfficialDragonTigerRequest {
    OfficialDragonTigerRequest::new(
        InstrumentId::new(exchange, code, AssetClass::Equity).unwrap(),
        IsoDate::new(date).unwrap(),
    )
    .unwrap()
}

fn signal_request(
    exchange: Exchange,
    code: &str,
    date: Option<&str>,
    limit: u32,
) -> InstrumentSignalRequest {
    let request = InstrumentSignalRequest::new(
        InstrumentId::new(exchange, code, AssetClass::Equity).unwrap(),
        PositiveU32::new(limit).unwrap(),
    )
    .unwrap();
    match date {
        Some(date) => request.with_trading_date(IsoDate::new(date).unwrap()),
        None => request,
    }
}

#[derive(Clone, Default)]
struct OfficialFixtureTransport {
    requests: Arc<Mutex<Vec<HttpRequest>>>,
    paginated: bool,
    duplicate_last: bool,
}

impl ExchangeTransport for OfficialFixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        self.requests.lock().unwrap().push(request.clone());
        let (content_type, body) = if request.url.contains("query.sse.com.cn") {
            (
                "application/javascript",
                include_bytes!("../fixtures/dragon_tiger_sse_600396.jsonp").to_vec(),
            )
        } else if request.url.contains("CATALOGID=1842_detal") {
            (
                "application/json",
                include_bytes!("../fixtures/dragon_tiger_szse_detail_000603_0901.json").to_vec(),
            )
        } else if request.url.contains("CATALOGID=1842_xxpl_after") && self.paginated {
            let page = if request.url.contains("PAGENO=2") {
                2
            } else {
                1
            };
            (
                "application/json",
                paginated_szse_list(page, self.duplicate_last),
            )
        } else if request.url.contains("CATALOGID=1842_xxpl_after") {
            (
                "application/json",
                include_bytes!("../fixtures/dragon_tiger_szse_list_000603.json").to_vec(),
            )
        } else {
            return Err(ExchangeError::Transport(format!(
                "unexpected fixture request {}",
                request.url
            )));
        };
        Ok(HttpResponse {
            status: 200,
            final_url: request.url.clone(),
            content_type: Some(content_type.into()),
            body,
        })
    }
}

fn paginated_szse_list(page: u32, duplicate_last: bool) -> Vec<u8> {
    let mut document: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../fixtures/dragon_tiger_szse_list_000603.json"
    ))
    .unwrap();
    document[0]["metadata"]["pageno"] = serde_json::json!(page);
    document[0]["metadata"]["pagecount"] = serde_json::json!(2);
    document[0]["metadata"]["recordcount"] = serde_json::json!(11);
    let template = document[0]["data"][0].clone();
    let range = if page == 1 { 0..10 } else { 10..11 };
    let rows = range
        .map(|index| {
            let mut row = template.clone();
            let indicator = if duplicate_last && page == 2 {
                "0909".to_owned()
            } else {
                format!("{:04}", 900 + index)
            };
            let link = row["bz"]
                .as_str()
                .unwrap()
                .replace("ZBDM=0901", &format!("ZBDM={indicator}"));
            row["bz"] = serde_json::json!(link);
            row["plyy"] = serde_json::json!(format!("fixture reason {indicator}"));
            row
        })
        .collect::<Vec<_>>();
    document[0]["data"] = serde_json::Value::Array(rows);
    serde_json::to_vec(&document).unwrap()
}

#[test]
fn request_models_are_exact_bounded_official_https_queries() {
    let sse = request(Exchange::Shanghai, "600396", "2026-07-22");
    assert_eq!(sse.instrument().code(), "600396");
    assert_eq!(sse.trading_date().as_str(), "2026-07-22");
    assert_eq!(
        sse.sse_url().unwrap(),
        "https://query.sse.com.cn/infodisplay/showTradePublicFile.do?jsonCallBack=magicExchange&isPagination=false&dateTx=2026-07-22"
    );
    assert!(sse.szse_list_url(1).is_err());

    let szse = request(Exchange::Shenzhen, "000603", "2026-07-23");
    let list = szse.szse_list_url(2).unwrap();
    assert!(list.starts_with("https://www.szse.cn/api/report/ShowReport/data?"));
    let parsed = url::Url::parse(&list).unwrap();
    let pairs = parsed
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(pairs.get("SHOWTYPE").map(|v| v.as_ref()), Some("JSON"));
    assert_eq!(
        pairs.get("CATALOGID").map(|v| v.as_ref()),
        Some("1842_xxpl_after")
    );
    assert_eq!(pairs.get("TABKEY").map(|v| v.as_ref()), Some("tab1"));
    assert_eq!(pairs.get("PAGENO").map(|v| v.as_ref()), Some("2"));
    assert_eq!(pairs.get("tab1PAGESIZE").map(|v| v.as_ref()), Some("10"));
    assert_eq!(pairs.get("txtDMorJC").map(|v| v.as_ref()), Some("000603"));
    assert_eq!(
        pairs.get("txtStart").map(|v| v.as_ref()),
        Some("2026-07-23")
    );
    assert_eq!(pairs.get("txtEnd").map(|v| v.as_ref()), Some("2026-07-23"));
    assert!(szse.szse_list_url(0).is_err());

    let key = SzseDragonTigerDetailKey::new(IsoDate::new("2026-07-23").unwrap(), "000603", "0901")
        .unwrap();
    assert_eq!(key.trading_date().as_str(), "2026-07-23");
    assert_eq!(key.instrument_code(), "000603");
    let detail = key.url().unwrap();
    assert!(detail.starts_with("https://www.szse.cn/api/report/ShowReport/data?"));
    let parsed = url::Url::parse(&detail).unwrap();
    let pairs = parsed
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        pairs.get("CATALOGID").map(|v| v.as_ref()),
        Some("1842_detal")
    );
    assert_eq!(pairs.get("TABKEY").map(|v| v.as_ref()), Some("tab1,tab2"));
    assert_eq!(pairs.get("DQRQ").map(|v| v.as_ref()), Some("2026-07-23"));
    assert_eq!(pairs.get("ZQDM").map(|v| v.as_ref()), Some("000603"));
    assert_eq!(pairs.get("ZBDM").map(|v| v.as_ref()), Some("0901"));
}

#[test]
fn sse_text_state_maps_exact_entry_and_complete_top_five_seats() {
    let parsed = parse_sse_response(
        include_bytes!("../fixtures/dragon_tiger_sse_600396.jsonp"),
        &request(Exchange::Shanghai, "600396", "2026-07-22"),
        OBSERVED_AT,
        BATCH_ID,
    )
    .unwrap();
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.seats.len(), 10);
    let entry = &parsed.entries[0];
    assert_eq!(entry.instrument().code(), "600396");
    assert_eq!(entry.trading_date().as_str(), "2026-07-22");
    assert_eq!(
        entry.reason().unwrap().as_str(),
        "有价格涨跌幅限制的日收盘价格涨幅偏离值达到7%的前五只证券"
    );
    assert_eq!(entry.entry_id().as_str(), "sse:2026-07-22:600396:1");
    assert!(entry.buy_amount().is_none());
    assert!(entry.sell_amount().is_none());
    assert!(entry.net_amount().is_none());
    assert!(entry.turnover_rate().is_none());
    assert_eq!(entry.evidence().provider(), ProviderId::Sse);
    assert_eq!(entry.evidence().source_at(), Some("2026-07-22"));

    let buy = &parsed.seats[0];
    assert_eq!(buy.entry_id(), entry.entry_id());
    assert_eq!(buy.side(), DragonTigerSide::Buy);
    assert_eq!(buy.rank().get(), 1);
    assert_eq!(buy.seat_name().as_str(), "沪股通专用");
    assert_eq!(buy.amount().get(), 204_844_204.54);
    assert_eq!(buy.buy_amount().unwrap().get(), 204_844_204.54);
    assert!(buy.sell_amount().is_none());
    assert!(buy.net_amount().is_none());
    let sell = &parsed.seats[5];
    assert_eq!(sell.side(), DragonTigerSide::Sell);
    assert_eq!(sell.rank().get(), 1);
    assert_eq!(sell.amount().get(), 81_243_566.0);
    assert!(sell.buy_amount().is_none());
    assert_eq!(sell.sell_amount().unwrap().get(), 81_243_566.0);
}

#[test]
fn sse_rejects_wrong_identity_date_units_or_incomplete_rank_state() {
    let fixture = include_bytes!("../fixtures/dragon_tiger_sse_600396.jsonp");
    assert!(parse_sse_response(
        fixture,
        &request(Exchange::Shanghai, "600396", "2026-07-21"),
        OBSERVED_AT,
        BATCH_ID,
    )
    .is_err());
    assert!(parse_sse_response(
        fixture,
        &request(Exchange::Shanghai, "600000", "2026-07-22"),
        OBSERVED_AT,
        BATCH_ID,
    )
    .is_err());

    let text = std::str::from_utf8(fixture).unwrap();
    for corrupted in [
        text.replace("累计买入金额(元):", "累计买入金额(万元):"),
        text.replace(
            "  (5) 国泰海通证券股份有限公司湛江万豪世家证券营业部                                             77178105.00",
            "",
        ),
        text.replace(
            "  (4) 国新证券股份有限公司北京分公司                                                             87790025.00",
            "  (3) 国新证券股份有限公司北京分公司                                                             87790025.00",
        ),
        text.replace("204844204.54", "204844204元"),
    ] {
        assert!(parse_sse_response(
            corrupted.as_bytes(),
            &request(Exchange::Shanghai, "600396", "2026-07-22"),
            OBSERVED_AT,
            BATCH_ID,
        )
        .is_err());
    }
}

#[test]
fn szse_list_enforces_identity_date_pagination_and_detail_keys() {
    let parsed = parse_szse_list_response(
        include_bytes!("../fixtures/dragon_tiger_szse_list_000603.json"),
        &request(Exchange::Shenzhen, "000603", "2026-07-23"),
        1,
        OBSERVED_AT,
        BATCH_ID,
    )
    .unwrap();
    assert_eq!(parsed.page_no, 1);
    assert_eq!(parsed.page_count, 1);
    assert_eq!(parsed.record_count, 2);
    assert_eq!(parsed.items.len(), 2);
    assert_eq!(parsed.items[0].detail_key.indicator_code(), "0901");
    assert_eq!(
        parsed.items[0].entry.entry_id().as_str(),
        "szse:2026-07-23:000603:0901"
    );
    assert_eq!(
        parsed.items[0].entry.reason().unwrap().as_str(),
        "日价格涨幅偏离值达到9.07%"
    );
    assert_eq!(
        parsed.items[0].entry.evidence().provider(),
        ProviderId::Szse
    );

    let fixture = std::str::from_utf8(include_bytes!(
        "../fixtures/dragon_tiger_szse_list_000603.json"
    ))
    .unwrap();
    for corrupted in [
        fixture.replace("\"zqdm\":\"000603\"", "\"zqdm\":\"000001\""),
        fixture.replace("\"dqrq\":\"2026-07-23\"", "\"dqrq\":\"2026-07-22\""),
        fixture.replace("\"pageno\": 1", "\"pageno\": 2"),
        fixture.replace("\"recordcount\": 2", "\"recordcount\": 3"),
        fixture.replace("ZQDM=000603", "ZQDM=000001"),
        fixture.replace("ZBDM=0901", "ZBDM="),
    ] {
        assert!(parse_szse_list_response(
            corrupted.as_bytes(),
            &request(Exchange::Shenzhen, "000603", "2026-07-23"),
            1,
            OBSERVED_AT,
            BATCH_ID,
        )
        .is_err());
    }
}

#[test]
fn szse_detail_preserves_both_amounts_side_rank_and_yuan_units() {
    let list = parse_szse_list_response(
        include_bytes!("../fixtures/dragon_tiger_szse_list_000603.json"),
        &request(Exchange::Shenzhen, "000603", "2026-07-23"),
        1,
        OBSERVED_AT,
        BATCH_ID,
    )
    .unwrap();
    let key = &list.items[0].detail_key;
    let seats = parse_szse_detail_response(
        include_bytes!("../fixtures/dragon_tiger_szse_detail_000603_0901.json"),
        key,
        OBSERVED_AT,
        BATCH_ID,
    )
    .unwrap();
    assert_eq!(seats.len(), 10);
    let buy = &seats[0];
    assert_eq!(buy.side(), DragonTigerSide::Buy);
    assert_eq!(buy.rank().get(), 1);
    assert_eq!(buy.amount().get(), 136_235_364.0);
    assert_eq!(buy.buy_amount().unwrap().get(), 136_235_364.0);
    assert_eq!(buy.sell_amount().unwrap().get(), 185_329_440.0);
    assert_eq!(buy.net_amount().unwrap().get(), -49_094_076.0);
    let sell = &seats[5];
    assert_eq!(sell.side(), DragonTigerSide::Sell);
    assert_eq!(sell.rank().get(), 1);
    assert_eq!(sell.amount().get(), 185_329_440.0);
    assert_eq!(sell.entry_id().as_str(), "szse:2026-07-23:000603:0901");
    assert_eq!(sell.evidence().source_at(), Some("2026-07-23"));
}

#[test]
fn szse_detail_rejects_identity_unit_schema_or_incomplete_top_five() {
    let key = SzseDragonTigerDetailKey::new(IsoDate::new("2026-07-23").unwrap(), "000603", "0901")
        .unwrap();
    let fixture = std::str::from_utf8(include_bytes!(
        "../fixtures/dragon_tiger_szse_detail_000603_0901.json"
    ))
    .unwrap();
    for corrupted in [
        fixture.replace("\"defaultValue\":\"000603\"", "\"defaultValue\":\"000001\""),
        fixture.replace("\"dqrq\":\"2026-07-23\"", "\"dqrq\":\"2026-07-22\""),
        fixture.replace("买入金额<br>（元）", "买入金额<br>（万元）"),
        fixture.replace(
            "{\"mmlb\":\"买5\",\"zsmc\":\"机构专用\",\"mrje\":\"26,201,294\",\"mcje\":\"33,471,072\"},",
            "",
        ),
        fixture.replace("\"mmlb\":\"卖4\"", "\"mmlb\":\"卖3\""),
        fixture.replace("\"mrje\":\"48,330\"", "\"mrje\":\"48,330元\""),
        fixture.replace("\"mrje\":\"48,330\"", "\"mrje\":\"48,33\""),
    ] {
        assert!(parse_szse_detail_response(
            corrupted.as_bytes(),
            &key,
            OBSERVED_AT,
            BATCH_ID,
        )
        .is_err());
    }
}

#[test]
fn request_rejects_non_equity_wrong_venue_and_invalid_codes() {
    let date = IsoDate::new("2026-07-23").unwrap();
    for instrument in [
        InstrumentId::new(Exchange::Shanghai, "000603", AssetClass::Equity).unwrap(),
        InstrumentId::new(Exchange::Shenzhen, "600396", AssetClass::Equity).unwrap(),
        InstrumentId::new(Exchange::Shenzhen, "000603", AssetClass::Fund).unwrap(),
        InstrumentId::new(Exchange::Shenzhen, "00060中", AssetClass::Equity).unwrap(),
    ] {
        assert!(OfficialDragonTigerRequest::new(instrument, date.clone()).is_err());
    }
}

#[test]
fn parsers_reject_oversized_bodies_before_decoding() {
    let oversized = vec![b'x'; MAX_DRAGON_TIGER_RESPONSE_BYTES + 1];
    assert!(parse_sse_response(
        &oversized,
        &request(Exchange::Shanghai, "600396", "2026-07-22"),
        OBSERVED_AT,
        BATCH_ID,
    )
    .is_err());
    assert!(parse_szse_list_response(
        &oversized,
        &request(Exchange::Shenzhen, "000603", "2026-07-23"),
        1,
        OBSERVED_AT,
        BATCH_ID,
    )
    .is_err());
    let key = SzseDragonTigerDetailKey::new(IsoDate::new("2026-07-23").unwrap(), "000603", "0901")
        .unwrap();
    assert!(parse_szse_detail_response(&oversized, &key, OBSERVED_AT, BATCH_ID).is_err());
}

#[test]
fn public_provider_traits_return_strict_entries_and_atomic_top_five_seats() {
    let sse_request = signal_request(Exchange::Shanghai, "600396", Some("2026-07-22"), 10);
    let sse = SseClient::with_transport(SseConfig::default(), OfficialFixtureTransport::default())
        .unwrap();
    let entries = sse.dragon_tiger_entries(&sse_request).unwrap();
    assert_eq!(entries.records().len(), 1);
    assert!(entries.quality().is_complete());
    assert_eq!(
        entries.records()[0].evidence().batch_id(),
        entries.provenance().batch_id().unwrap()
    );
    let sse = SseClient::with_transport(SseConfig::default(), OfficialFixtureTransport::default())
        .unwrap();
    let seats = sse.dragon_tiger_seats(&sse_request).unwrap();
    assert_eq!(seats.records().len(), 10);
    assert!(seats.quality().is_complete());

    let szse_request = signal_request(Exchange::Shenzhen, "000603", Some("2026-07-23"), 10);
    let szse =
        SzseClient::with_transport(SzseConfig::default(), OfficialFixtureTransport::default())
            .unwrap();
    let entries = szse.dragon_tiger_entries(&szse_request).unwrap();
    assert_eq!(entries.records().len(), 2);
    assert!(entries.quality().is_complete());
    let szse =
        SzseClient::with_transport(SzseConfig::default(), OfficialFixtureTransport::default())
            .unwrap();
    let seats = szse.dragon_tiger_seats(&szse_request).unwrap();
    assert_eq!(seats.records().len(), 10);
    assert!(seats.quality().is_complete());
    assert!(seats
        .records()
        .iter()
        .all(|seat| seat.entry_id().as_str() == "szse:2026-07-23:000603:0901"));
}

#[test]
fn public_provider_traits_require_date_venue_and_complete_seat_limit() {
    let sse = SseClient::with_transport(SseConfig::default(), OfficialFixtureTransport::default())
        .unwrap();
    assert!(matches!(
        sse.dragon_tiger_entries(&signal_request(Exchange::Shanghai, "600396", None, 1)),
        Err(ExchangeError::InvalidRequest(_))
    ));
    assert!(matches!(
        sse.dragon_tiger_entries(&signal_request(
            Exchange::Shenzhen,
            "000603",
            Some("2026-07-23"),
            1
        )),
        Err(ExchangeError::InvalidRequest(_))
    ));
    assert!(matches!(
        sse.dragon_tiger_seats(&signal_request(
            Exchange::Shanghai,
            "600396",
            Some("2026-07-22"),
            9
        )),
        Err(ExchangeError::InvalidRequest(_))
    ));
}

#[test]
fn szse_provider_fetches_every_page_before_truncating_and_rejects_cross_page_duplicates() {
    let request = signal_request(Exchange::Shenzhen, "000603", Some("2026-07-23"), 1);
    let transport = OfficialFixtureTransport {
        paginated: true,
        ..Default::default()
    };
    let requests = Arc::clone(&transport.requests);
    let client = SzseClient::with_transport(SzseConfig::default(), transport).unwrap();
    let batch = client.dragon_tiger_entries(&request).unwrap();
    assert_eq!(batch.records().len(), 1);
    assert_eq!(requests.lock().unwrap().len(), 2);

    let transport = OfficialFixtureTransport {
        paginated: true,
        duplicate_last: true,
        ..Default::default()
    };
    let client = SzseClient::with_transport(SzseConfig::default(), transport).unwrap();
    assert!(matches!(
        client.dragon_tiger_entries(&request),
        Err(ExchangeError::Schema(_))
    ));
}

fn fetch_official(url: &str, referer: &str) -> Vec<u8> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(30))
        .redirects(0)
        .build();
    let response = agent
        .get(url)
        .set("Accept", "application/json, text/javascript;q=0.9")
        .set("Referer", referer)
        .set(
            "User-Agent",
            "Mozilla/5.0 (magic-exchange-rs live verification)",
        )
        .call()
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response
        .header("Content-Type")
        .is_some_and(|value| value.contains("json") || value.contains("javascript")));
    assert_eq!(response.get_url(), url);
    let mut body = Vec::new();
    response
        .into_reader()
        .take((magic_exchange_rs::MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .unwrap();
    assert!(!body.is_empty());
    assert!(body.len() <= magic_exchange_rs::MAX_RESPONSE_BYTES);
    body
}

#[test]
#[ignore = "requires real SSE HTTPS"]
fn live_sse_official_response_parses_without_fixture_fallback() {
    let request = request(Exchange::Shanghai, "600396", "2026-07-22");
    let url = request.sse_url().unwrap();
    let body = fetch_official(
        &url,
        "https://www.sse.com.cn/disclosure/diclosure/public/dailydata/",
    );
    let parsed = parse_sse_response(&body, &request, OBSERVED_AT, "live-sse").unwrap();
    assert!(!parsed.entries.is_empty());
    assert_eq!(parsed.seats.len(), parsed.entries.len() * 10);
}

#[test]
#[ignore = "requires real SZSE HTTPS"]
fn live_szse_official_list_and_detail_parse_without_fixture_fallback() {
    let request = request(Exchange::Shenzhen, "000603", "2026-07-23");
    let list_url = request.szse_list_url(1).unwrap();
    let list_body = fetch_official(
        &list_url,
        "https://www.szse.cn/disclosure/deal/public/index.html",
    );
    let list =
        parse_szse_list_response(&list_body, &request, 1, OBSERVED_AT, "live-szse-list").unwrap();
    assert!(!list.items.is_empty());
    let detail_url = list.items[0].detail_key.url().unwrap();
    let detail_body = fetch_official(
        &detail_url,
        "https://www.szse.cn/disclosure/deal/public/index.html",
    );
    let seats = parse_szse_detail_response(
        &detail_body,
        &list.items[0].detail_key,
        OBSERVED_AT,
        "live-szse-detail",
    )
    .unwrap();
    assert_eq!(seats.len(), 10);
}
