use super::*;
use serde_json::json;
use std::collections::VecDeque;

#[derive(Clone, Default)]
pub(crate) struct FixtureTransport {
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl FixtureTransport {
    pub(crate) fn new(responses: Vec<serde_json::Value>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(
                responses
                    .into_iter()
                    .map(|value| serde_json::to_vec(&value).unwrap())
                    .collect(),
            )),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn requested_urls(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.url().to_owned())
            .collect()
    }
}

impl HttpTransport for FixtureTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.requests.lock().unwrap().push(request.clone());
        let body = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| TransportError::Internal("missing fixture response".into()))?;
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body,
        ))
    }
}

pub(crate) fn success(request_id: &str, data: serde_json::Value) -> serde_json::Value {
    json!({
        "code": 0,
        "message": "success",
        "request_id": request_id,
        "data": data
    })
}

fn instrument(exchange: Exchange, code: &str) -> InstrumentId {
    InstrumentId::new(exchange, code, AssetClass::Equity).unwrap()
}

fn historical_request() -> BarsRequest {
    BarsRequest::new(
        instrument(Exchange::Shanghai, "600519"),
        BarInterval::Day,
        10,
    )
    .unwrap()
    .with_range("2026-08-18", "2026-08-19")
    .unwrap()
}

#[test]
fn debug_and_http_request_redact_api_key() {
    let transport = FixtureTransport::new(vec![success(
        "valuation-request",
        json!({
            "timestamp": null,
            "total": 1,
            "item": [{
                "thscode": "600519.SH",
                "ticker": "600519",
                "name": "贵州茅台",
                "pe_ttm": null,
                "pe_mrq": null,
                "pb_mrq": null,
                "ps_ttm": null,
                "pcf_ttm": null
            }]
        }),
    )]);
    let observed = transport.clone();
    let client = HithinkClient::with_transport("top_secret_key", transport).unwrap();
    let debug = format!("{client:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("top_secret_key"));

    client
        .probe_market_statistics(&[instrument(Exchange::Shanghai, "600519")])
        .unwrap();
    let requests = observed.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request_debug = format!("{:?}", requests[0]);
    assert!(!request_debug.contains("top_secret_key"));
    assert!(requests[0]
        .headers()
        .iter()
        .any(|(name, value)| name == "X-api-key" && value == "top_secret_key"));
}

#[test]
fn historical_bars_preserve_distinct_dates_and_convert_shares_to_lots() {
    let first = shanghai_millis(parse_date("2026-08-18").unwrap(), Time::MIDNIGHT).unwrap();
    let second = shanghai_millis(parse_date("2026-08-19").unwrap(), Time::MIDNIGHT).unwrap();
    let client = HithinkClient::with_transport(
        "test_key",
        FixtureTransport::new(vec![success(
            "history-request",
            json!({
                "thscode": "600519.SH",
                "interval": "1d",
                "adjust": "none",
                "timestamp": second,
                "item": [
                    {
                        "date_ms": second,
                        "open_price": 11.0,
                        "high_price": 12.0,
                        "low_price": 10.5,
                        "close_price": 11.5,
                        "volume": 2300.0,
                        "turnover": 26000.0
                    },
                    {
                        "date_ms": first,
                        "open_price": 10.0,
                        "high_price": 11.0,
                        "low_price": 9.5,
                        "close_price": 10.5,
                        "volume": 1200.0,
                        "turnover": 12000.0
                    }
                ]
            }),
        )]),
    )
    .unwrap();

    let batch = client.probe_historical_bars(&historical_request()).unwrap();
    assert!(batch.quality().is_complete());
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].bar_start(), "2026-08-18");
    assert_eq!(batch.records()[1].bar_start(), "2026-08-19");
    assert_eq!(batch.records()[0].source_at(), Some("2026-08-18"));
    assert_eq!(batch.records()[1].source_at(), Some("2026-08-19"));
    assert_eq!(batch.records()[0].volume().get(), 12.0);
    assert_eq!(batch.records()[1].volume().get(), 23.0);
    assert_eq!(
        batch.provenance().source_at(),
        Some(format!("unix-ms:{second}").as_str())
    );
}

#[test]
fn historical_bars_route_standard_indices_and_etfs_to_exact_endpoints() {
    let day = shanghai_millis(parse_date("2026-08-19").unwrap(), Time::MIDNIGHT).unwrap();
    for (asset_class, code, path, adjust) in [
        (
            AssetClass::Index,
            "000300",
            INDEX_HISTORICAL_PATH,
            Some(serde_json::Value::Null),
        ),
        (AssetClass::Fund, "510300", FUND_HISTORICAL_PATH, None),
    ] {
        let mut data = json!({
            "thscode": format!("{code}.SH"),
            "interval": "1d",
            "timestamp": day,
            "item": [{
                "date_ms": day,
                "open_price": 4.0,
                "high_price": 4.1,
                "low_price": 3.9,
                "close_price": 4.05,
                "volume": 10000.0,
                "turnover": 40500.0
            }]
        });
        if let Some(adjust) = adjust {
            data.as_object_mut()
                .unwrap()
                .insert("adjust".to_owned(), adjust);
        }
        let transport = FixtureTransport::new(vec![success("history-asset", data)]);
        let observed = transport.clone();
        let client = HithinkClient::with_transport("test_key", transport).unwrap();
        let instrument = InstrumentId::new(Exchange::Shanghai, code, asset_class).unwrap();
        let request = BarsRequest::new(instrument.clone(), BarInterval::Day, 1)
            .unwrap()
            .with_range("2026-08-19", "2026-08-19")
            .unwrap();
        let batch = client.probe_historical_bars(&request).unwrap();
        assert_eq!(batch.records().len(), 1);
        assert_eq!(batch.records()[0].instrument(), &instrument);
        assert!(observed.requested_urls()[0].contains(path));
        assert!(!observed.requested_urls()[0].contains("adjust="));
    }
}

#[test]
fn historical_bars_fail_closed_on_duplicate_date_or_timestamp_conflict() {
    let date = shanghai_millis(parse_date("2026-08-18").unwrap(), Time::MIDNIGHT).unwrap();
    let row = json!({
        "date_ms": date,
        "open_price": 10.0,
        "high_price": 11.0,
        "low_price": 9.0,
        "close_price": 10.5,
        "volume": 100.0,
        "turnover": 1000.0
    });
    for data in [
        json!({
            "thscode": "600519.SH",
            "interval": "1d",
            "adjust": "none",
            "timestamp": date,
            "item": [row.clone(), row.clone()]
        }),
        json!({
            "thscode": "600519.SH",
            "interval": "1d",
            "adjust": "none",
            "timestamp": shanghai_millis(parse_date("2026-08-19").unwrap(), Time::MIDNIGHT).unwrap(),
            "item": [row.clone()]
        }),
        json!({
            "thscode": "600519.SH",
            "interval": "1d",
            "adjust": "forward",
            "timestamp": date,
            "item": [row.clone()]
        }),
    ] {
        let client = HithinkClient::with_transport(
            "test_key",
            FixtureTransport::new(vec![success("history-conflict", data)]),
        )
        .unwrap();
        assert!(matches!(
            client.probe_historical_bars(&historical_request()),
            Err(HithinkError::Protocol(_))
        ));
    }
}

#[test]
fn valuations_preserve_nulls_negatives_and_request_order() {
    let instruments = [
        instrument(Exchange::Shanghai, "600519"),
        instrument(Exchange::Shenzhen, "000001"),
    ];
    let client = HithinkClient::with_transport(
        "test_key",
        FixtureTransport::new(vec![success(
            "valuation-request",
            json!({
                "timestamp": 1787068800000_i64,
                "total": 2,
                "item": [
                    {
                        "thscode": "600519.SH",
                        "ticker": "600519",
                        "name": "贵州茅台",
                        "pe_ttm": 18.5,
                        "pe_mrq": null,
                        "pb_mrq": 6.2,
                        "ps_ttm": 8.1,
                        "pcf_ttm": null
                    },
                    {
                        "thscode": "000001.SZ",
                        "ticker": "000001",
                        "name": null,
                        "pe_ttm": -2.0,
                        "pe_mrq": 4.0,
                        "pb_mrq": null,
                        "ps_ttm": null,
                        "pcf_ttm": -3.0
                    }
                ]
            }),
        )]),
    )
    .unwrap();
    let batch = client.probe_market_statistics(&instruments).unwrap();
    assert_eq!(batch.records()[0].instrument(), &instruments[0]);
    assert_eq!(batch.records()[0].trailing_pe().unwrap().get(), 18.5);
    assert!(batch.records()[0].static_pe().is_none());
    assert_eq!(batch.records()[1].trailing_pe().unwrap().get(), -2.0);
    assert!(batch.records()[1].pb().is_none());
}

#[test]
fn valuation_identity_conflict_rejects_entire_batch() {
    let client = HithinkClient::with_transport(
        "test_key",
        FixtureTransport::new(vec![success(
            "valuation-conflict",
            json!({
                "timestamp": null,
                "total": 1,
                "item": [{
                    "thscode": "000001.SZ",
                    "ticker": "000001",
                    "name": null,
                    "pe_ttm": null,
                    "pe_mrq": null,
                    "pb_mrq": null,
                    "ps_ttm": null,
                    "pcf_ttm": null
                }]
            }),
        )]),
    )
    .unwrap();
    assert!(matches!(
        client.probe_market_statistics(&[instrument(Exchange::Shanghai, "600519")]),
        Err(HithinkError::Protocol(_))
    ));
}

#[test]
fn limit_up_pool_maps_only_proved_fields_and_keeps_exact_date() {
    let request = LimitPoolRequest::new(
        LimitPoolKind::Upper,
        IsoDate::new("2026-08-21").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    let client = HithinkClient::with_transport(
        "test_key",
        FixtureTransport::new(vec![success(
            "limit-up-request",
            json!({
                "timestamp": 1787302800000_i64,
                "pagination": {"total": 1, "pages": 1, "size": 200, "page": 1},
                "item": [{
                    "thscode": "920403.BJ",
                    "ticker": "920403",
                    "name": "北交样本",
                    "is_st": false,
                    "is_new": true,
                    "last_price": 22.0,
                    "price_change_ratio_pct": 29.98,
                    "limit_up_time": "09:31",
                    "limit_up_reason": "测试原因",
                    "continue_day_text": "2 连板",
                    "continue_day_cnt": 2,
                    "seal_money": 1000000.0,
                    "max_seal_money": 1200000.0
                }]
            }),
        )]),
    )
    .unwrap();
    let batch = client.probe_limit_pool(&request).unwrap();
    let row = &batch.records()[0];
    assert_eq!(row.instrument.code(), "920403");
    assert_eq!(row.instrument.exchange(), Exchange::Beijing);
    assert_eq!(row.trading_date.as_str(), "2026-08-21");
    assert_eq!(row.evidence.source_at(), Some("2026-08-21"));
    assert_eq!(row.streak.unwrap().get(), 2);
    assert!(row.last_seal_at.is_none());
}

#[test]
fn limit_pool_rejects_incomplete_pagination() {
    let request = LimitPoolRequest::new(
        LimitPoolKind::Lower,
        IsoDate::new("2026-08-21").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    let client = HithinkClient::with_transport(
        "test_key",
        FixtureTransport::new(vec![success(
            "limit-down-request",
            json!({
                "timestamp": 1787302800000_i64,
                "pagination": {"total": 2, "pages": 1, "size": 200, "page": 1},
                "item": []
            }),
        )]),
    )
    .unwrap();
    assert!(matches!(
        client.probe_limit_pool(&request),
        Err(HithinkError::Protocol(_))
    ));
}

#[test]
fn limit_pool_reads_every_declared_page_before_applying_caller_limit() {
    let item = |index: u32| {
        let ticker = format!("{:06}", 100_000 + index);
        json!({
            "thscode": format!("{ticker}.SZ"),
            "ticker": ticker,
            "name": format!("样本{index}"),
            "last_price": 10.0,
            "price_change_ratio_pct": 5.0,
            "open_times": index,
            "turnover_ratio_pct": 2.0,
            "turnover": 1000.0
        })
    };
    let first_page = (0..200).map(item).collect::<Vec<_>>();
    let second_page = vec![item(200)];
    let timestamp = 1787302800000_i64;
    let transport = FixtureTransport::new(vec![
        success(
            "page-one",
            json!({
                "timestamp": timestamp,
                "pagination": {"total": 201, "pages": 2, "size": 200, "page": 1},
                "item": first_page
            }),
        ),
        success(
            "page-two",
            json!({
                "timestamp": timestamp,
                "pagination": {"total": 201, "pages": 2, "size": 200, "page": 2},
                "item": second_page
            }),
        ),
    ]);
    let observed = transport.clone();
    let client = HithinkClient::with_transport("test_key", transport).unwrap();
    let request = LimitPoolRequest::new(
        LimitPoolKind::Broken,
        IsoDate::new("2026-08-21").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    let batch = client.probe_limit_pool(&request).unwrap();
    assert_eq!(batch.records().len(), 10);
    assert_eq!(observed.requests.lock().unwrap().len(), 2);
    assert_eq!(
        batch.provenance().batch_id(),
        Some("hithink-pages:page-one,page-two")
    );
}

#[test]
fn broken_pool_preserves_null_open_times_as_field_level_absence() {
    let client = HithinkClient::with_transport(
        "test_key",
        FixtureTransport::new(vec![success(
            "broken-null-open-times",
            json!({
                "timestamp": 1787367878853_i64,
                "pagination": {"total": 1, "pages": 1, "size": 200, "page": 1},
                "item": [{
                    "thscode": "600519.SH",
                    "ticker": "600519",
                    "name": "贵州茅台",
                    "last_price": 10.0,
                    "price_change_ratio_pct": 5.0,
                    "open_times": null,
                    "turnover_ratio_pct": 2.0,
                    "turnover": 1000.0
                }]
            }),
        )]),
    )
    .unwrap();
    let request = LimitPoolRequest::new(
        LimitPoolKind::Broken,
        IsoDate::new("2026-08-21").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    let batch = client.probe_limit_pool(&request).unwrap();
    assert_eq!(batch.records().len(), 1);
    assert_eq!(batch.records()[0].break_count, None);
}

#[test]
fn popularity_preserves_source_timestamp_and_bj_identity() {
    let timestamp = 1787302800000_i64;
    let client = HithinkClient::with_transport(
        "test_key",
        FixtureTransport::new(vec![success(
            "hot-request",
            json!({
                "timestamp": timestamp,
                "item": [
                    {"thscode":"920344.BJ","ticker":"920344","name":"北交二","rank":2,"heat":"88.0","rank_change":-1,"rank_trend":"down"},
                    {"thscode":"600519.SH","ticker":"600519","name":"贵州茅台","rank":1,"heat":99.0,"rank_change":2,"rank_trend":"up"}
                ]
            }),
        )]),
    )
    .unwrap();
    let batch = client
        .probe_popularity(PositiveU32::new(2).unwrap())
        .unwrap();
    assert_eq!(batch.records()[0].rank.get(), 1);
    assert_eq!(batch.records()[1].instrument.code(), "920344");
    assert_eq!(
        batch.records()[1].evidence.source_at(),
        Some(format!("unix-ms:{timestamp}").as_str())
    );
}

#[test]
fn popularity_rejects_non_finite_or_loose_heat_strings() {
    for heat in [json!("NaN"), json!(" 88"), json!("88\n")] {
        let client = HithinkClient::with_transport(
            "test_key",
            FixtureTransport::new(vec![success(
                "hot-invalid",
                json!({
                    "timestamp": 1787302800000_i64,
                    "item": [{
                        "thscode":"600519.SH",
                        "ticker":"600519",
                        "name":"贵州茅台",
                        "rank":1,
                        "heat":heat,
                        "rank_change":0,
                        "rank_trend":"flat"
                    }]
                }),
            )]),
        )
        .unwrap();
        assert!(matches!(
            client.probe_popularity(PositiveU32::new(1).unwrap()),
            Err(HithinkError::Decode(_))
        ));
    }
}

#[test]
fn business_errors_are_typed_and_do_not_echo_server_message() {
    let client = HithinkClient::with_transport(
        "test_key",
        FixtureTransport::new(vec![json!({
            "code": 2003,
            "message": "secret upstream detail",
            "request_id": "auth-request",
            "data": null
        })]),
    )
    .unwrap();
    let error = client
        .probe_market_statistics(&[instrument(Exchange::Shanghai, "600519")])
        .unwrap_err();
    assert!(matches!(
        error,
        HithinkError::Authentication {
            code: 2003,
            ref request_id
        } if request_id == "auth-request"
    ));
    assert!(!error.to_string().contains("secret upstream detail"));
}

#[test]
fn unsupported_families_and_shapes_fail_before_transport() {
    let transport = FixtureTransport::default();
    let observed = transport.clone();
    let client = HithinkClient::with_transport("test_key", transport).unwrap();
    let index = InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).unwrap();
    assert!(matches!(
        client.probe_market_statistics(&[index]),
        Err(HithinkError::Unsupported(_))
    ));
    for asset_class in [AssetClass::Index, AssetClass::Fund] {
        let request = BarsRequest::new(
            InstrumentId::new(Exchange::Beijing, "920403", asset_class).unwrap(),
            BarInterval::Day,
            1,
        )
        .unwrap()
        .with_range("2026-08-18", "2026-08-19")
        .unwrap();
        assert!(matches!(
            client.probe_historical_bars(&request),
            Err(HithinkError::Unsupported(_))
        ));
    }
    let previous = LimitPoolRequest::new(
        LimitPoolKind::PreviousUpper,
        IsoDate::new("2026-08-21").unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        client.probe_limit_pool(&previous),
        Err(HithinkError::Unsupported(_))
    ));
    assert!(observed.requests.lock().unwrap().is_empty());
}
