use magic_market_core::{AssetClass, Exchange};
use magic_tencent_rs::SnapshotTransport;

use super::*;

struct OfflineTencentTransport {
    body: Vec<u8>,
    calls: Arc<AtomicUsize>,
}

impl SnapshotTransport for OfflineTencentTransport {
    fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.body.clone())
    }
}

fn equity() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn tencent_registry(body: Vec<u8>, ceiling: usize) -> (OperationRegistry, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = TencentClient::with_transport(OfflineTencentTransport {
        body,
        calls: calls.clone(),
    });
    let registry = registry_with_tencent(client, Duration::from_secs(1), ceiling).unwrap();
    (registry, calls)
}

fn shape_command(maximum_points: u32) -> QueryCommand {
    let request = IntradayShapeRequest::new(
        equity(),
        Some(IsoDate::new("2026-07-23").unwrap()),
        PositiveU32::new(maximum_points).unwrap(),
    )
    .unwrap();
    QueryCommand::new(
        "offline-upstream-shape",
        Operation::IntradayShape,
        Some("LocalAnalysis".into()),
        CanonicalPayload::new(
            INTRADAY_SHAPE_REQUEST_SCHEMA,
            1,
            serde_json::to_vec(&request).unwrap(),
            65_536,
        )
        .unwrap(),
    )
    .unwrap()
}

fn historical_minutes(rows: &[&str]) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "code": 0,
        "data": {"sh600396": {"data": [{"date": "20260723", "data": rows}]}}
    }))
    .unwrap()
}

#[test]
fn registered_intraday_shape_preserves_worked_prices_direction_and_source_lot_vwap() {
    let (registry, calls) = tencent_registry(
        historical_minutes(&[
            "0930 10.00 10 10000.00",
            "0931 11.00 20 21000.00",
            "0932 11.00 30 32000.00",
            "1300 9.00 40 36000.00",
        ]),
        65_536,
    );
    let result = registry.execute(shape_command(4)).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(result.repository_admitted && result.complete);
    assert_eq!(result.provider, "LocalAnalysis");
    assert_eq!(
        result.source_at.as_deref(),
        Some("2026-07-23T13:00:00+08:00")
    );
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].schema(), INTRADAY_SHAPE_RECORD_SCHEMA);
    let record: IntradayShapeRecord = serde_json::from_slice(result.records[0].data()).unwrap();
    assert_eq!(record.point_count().get(), 4);
    assert_eq!(record.open().get(), 10.0);
    assert_eq!(record.high().get(), 11.0);
    assert_eq!(record.low().get(), 9.0);
    assert_eq!(record.latest().get(), 9.0);
    assert_eq!(
        (
            record.up_points(),
            record.down_points(),
            record.flat_points()
        ),
        (1, 1, 2)
    );
    assert_eq!(record.cumulative_volume().unwrap().get(), 40.0);
    assert_eq!(record.cumulative_amount().unwrap().get(), 36_000.0);
    // Source quantity is 40 lots = 4000 shares: CNY 36000 / 4000 = CNY 9.
    assert_eq!(record.vwap().unwrap().get(), 9.0);
    assert_eq!(record.input_evidence()[0].provider(), ProviderId::Tencent);
    assert_eq!(
        record.input_evidence()[0].source_at(),
        result.source_at.as_deref()
    );
}

#[test]
fn registered_intraday_shape_refuses_mixed_amount_evidence_without_partial_output() {
    let (registry, calls) = tencent_registry(
        historical_minutes(&["0930 10.00 10", "0931 11.00 20 21000.00"]),
        65_536,
    );
    let error = registry.execute(shape_command(2)).unwrap_err();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        matches!(
            error,
            ServiceError::FailedPrecondition(ref reason)
                if reason == "IntradayShape cumulative amount presence changes within one source series"
        ),
        "{error:?}"
    );
}

fn typed_command<T: Serialize>(operation: Operation, schema: &str, request: &T) -> QueryCommand {
    QueryCommand::new(
        "offline-upstream-request",
        operation,
        Some("Tencent".into()),
        CanonicalPayload::new(schema, 1, serde_json::to_vec(request).unwrap(), 65_536).unwrap(),
    )
    .unwrap()
}

#[test]
fn registered_minute_and_shape_handlers_do_not_invent_missing_amount_or_vwap() {
    let (registry, calls) = tencent_registry(
        historical_minutes(&["0930 10.00 10", "0931 11.00 20"]),
        65_536,
    );
    let minute_request = MinuteDataRequest::new(equity())
        .with_date("2026-07-23")
        .unwrap();
    let minute = registry
        .execute(typed_command(
            Operation::MinuteData,
            MINUTE_DATA_REQUEST_SCHEMA,
            &minute_request,
        ))
        .unwrap();
    assert!(!minute.complete);
    assert_eq!(minute.provider, "Tencent");
    assert_eq!(minute.records.len(), 2);
    for payload in &minute.records {
        assert_eq!(payload.schema(), MINUTE_DATA_RECORD_SCHEMA);
        let record: MinutePoint = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(record.status(), DataStatus::Unavailable);
        assert!(record.cumulative_amount().is_none());
        assert!(record.source_at().is_some());
    }
    let shape = registry.execute(shape_command(2)).unwrap();
    let record: IntradayShapeRecord = serde_json::from_slice(shape.records[0].data()).unwrap();
    assert_eq!(record.point_count().get(), 2);
    assert_eq!(record.cumulative_volume().unwrap().get(), 20.0);
    assert!(record.cumulative_amount().is_none() && record.vwap().is_none());
    assert_eq!(record.input_evidence()[0].provider(), ProviderId::Tencent);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn registered_intraday_request_refuses_invalid_bounds_and_identity_before_source() {
    let (registry, calls) = tencent_registry(Vec::new(), 65_536);
    let cases = [
        (
            "zero bound",
            serde_json::json!({"instrument": equity(), "maximum_points": 0}),
        ),
        (
            "above bound",
            serde_json::json!({"instrument": equity(), "maximum_points": 801}),
        ),
        (
            "index identity",
            serde_json::json!({
                "instrument": {"exchange":"Shanghai", "code":"000001", "asset_class":"Index"},
                "maximum_points": 2
            }),
        ),
        (
            "wrong exchange",
            serde_json::json!({
                "instrument": {"exchange":"HongKong", "code":"00700", "asset_class":"Equity"},
                "maximum_points": 2
            }),
        ),
        (
            "invalid date",
            serde_json::json!({
                "instrument": equity(), "trading_date": "2026-02-29", "maximum_points": 2
            }),
        ),
        (
            "uncontracted cutoff",
            serde_json::json!({
                "instrument": equity(), "maximum_points": 2, "cutoff": "2026-07-23"
            }),
        ),
    ];
    for (name, data) in cases {
        let command = QueryCommand::new(
            "offline-bad-shape-request",
            Operation::IntradayShape,
            Some("LocalAnalysis".into()),
            CanonicalPayload::new(
                INTRADAY_SHAPE_REQUEST_SCHEMA,
                1,
                serde_json::to_vec(&data).unwrap(),
                4096,
            )
            .unwrap(),
        )
        .unwrap();
        let error = registry.execute(command).unwrap_err();
        assert!(
            matches!(error, ServiceError::InvalidRequest(_)),
            "{name}: {error:?}"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "{name} reached source transport"
        );
    }
}

#[derive(Clone, Copy)]
enum TencentFailure {
    Transport,
    Protocol,
}

struct FailingTencentTransport {
    failure: TencentFailure,
    calls: Arc<AtomicUsize>,
}

impl SnapshotTransport for FailingTencentTransport {
    fn get(&self, _url: &str) -> Result<Vec<u8>, TencentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.failure {
            TencentFailure::Transport => {
                Err(TencentError::Transport("offline source outage".into()))
            }
            TencentFailure::Protocol => Err(TencentError::Protocol(
                "offline source schema conflict".into(),
            )),
        }
    }
}

fn tencent_source_request_examples() -> Vec<(Operation, &'static str, serde_json::Value)> {
    let instruments = serde_json::json!({"instruments": [equity()]});
    vec![
        (
            Operation::RealtimeQuotes,
            REALTIME_QUOTES_REQUEST_SCHEMA,
            instruments.clone(),
        ),
        (
            Operation::OrderBooks,
            ORDER_BOOKS_REQUEST_SCHEMA,
            instruments.clone(),
        ),
        (
            Operation::SecurityMetadata,
            SECURITY_METADATA_REQUEST_SCHEMA,
            instruments.clone(),
        ),
        (
            Operation::MarketStatistics,
            MARKET_STATISTICS_REQUEST_SCHEMA,
            instruments,
        ),
        (
            Operation::HistoricalBars,
            HISTORICAL_BARS_REQUEST_SCHEMA,
            serde_json::to_value(BarsRequest::new(equity(), BarInterval::Day, 2).unwrap()).unwrap(),
        ),
        (
            Operation::MinuteData,
            MINUTE_DATA_REQUEST_SCHEMA,
            serde_json::to_value(MinuteDataRequest::new(equity())).unwrap(),
        ),
        (
            Operation::Trades,
            TRADES_REQUEST_SCHEMA,
            serde_json::to_value(TradesRequest::new(equity(), 2).unwrap()).unwrap(),
        ),
        (
            Operation::IntradayShape,
            INTRADAY_SHAPE_REQUEST_SCHEMA,
            serde_json::json!({"instrument": equity(), "trading_date": "2026-07-23", "maximum_points": 2}),
        ),
    ]
}

#[test]
fn registered_tencent_handlers_keep_source_outages_and_schema_conflicts_as_errors() {
    for failure in [TencentFailure::Transport, TencentFailure::Protocol] {
        let calls = Arc::new(AtomicUsize::new(0));
        let client = TencentClient::with_transport(FailingTencentTransport {
            failure,
            calls: calls.clone(),
        });
        let registry = registry_with_tencent(client, Duration::from_secs(1), 65_536).unwrap();
        for (index, (operation, schema, data)) in
            tencent_source_request_examples().into_iter().enumerate()
        {
            let provider = if operation == Operation::IntradayShape {
                "LocalAnalysis"
            } else {
                "Tencent"
            };
            let command = fixture_command(
                operation,
                provider,
                schema,
                1,
                &serde_json::to_vec(&data).unwrap(),
            );
            let error = registry.execute(command).unwrap_err();
            match failure {
                TencentFailure::Transport => assert!(
                    matches!(
                        error, ServiceError::Unavailable { operation: failed_operation, ref reason }
                            if failed_operation == operation && reason == "offline source outage"
                    ),
                    "{operation:?}: {error:?}"
                ),
                TencentFailure::Protocol => assert!(
                    matches!(
                        error, ServiceError::FailedPrecondition(ref reason)
                            if reason == "offline source schema conflict"
                    ),
                    "{operation:?}: {error:?}"
                ),
            }
            assert_eq!(
                calls.load(Ordering::SeqCst),
                index + 1,
                "{operation:?} did not reach the single pinned offline source"
            );
        }
    }
}

struct FailingSinaTransport {
    calls: Arc<AtomicUsize>,
}

impl magic_sina_rs::SnapshotTransport for FailingSinaTransport {
    fn get(&self, _url: &str) -> Result<Vec<u8>, SinaError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(SinaError::Transport("offline Sina source outage".into()))
    }

    fn get_document(&self, _url: &str) -> Result<magic_sina_rs::DocumentResponse, SinaError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(SinaError::Transport("offline Sina source outage".into()))
    }
}

#[test]
fn registered_sina_handlers_do_not_turn_acquisition_outages_into_empty_success() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = SinaClient::with_transport(FailingSinaTransport {
        calls: calls.clone(),
    });
    let mut registry = OperationRegistry::all_unadmitted("offline source fixture only");
    register_sina_parity(&mut registry, client, 65_536).unwrap();
    let instruments = serde_json::json!({"instruments": [equity()]});
    let cases = [
        (
            Operation::RealtimeQuotes,
            REALTIME_QUOTES_REQUEST_SCHEMA,
            1,
            instruments.clone(),
        ),
        (
            Operation::OrderBooks,
            ORDER_BOOKS_REQUEST_SCHEMA,
            1,
            instruments.clone(),
        ),
        (
            Operation::SecurityMetadata,
            SECURITY_METADATA_REQUEST_SCHEMA,
            1,
            instruments,
        ),
        (
            Operation::HistoricalBars,
            HISTORICAL_BARS_REQUEST_SCHEMA,
            1,
            serde_json::to_value(BarsRequest::new(equity(), BarInterval::Day, 2).unwrap()).unwrap(),
        ),
        (
            Operation::MinuteData,
            MINUTE_DATA_REQUEST_SCHEMA,
            1,
            serde_json::to_value(MinuteDataRequest::new(equity())).unwrap(),
        ),
        (
            Operation::InstrumentNews,
            INSTRUMENT_NEWS_REQUEST_SCHEMA,
            2,
            serde_json::json!({"instrument": equity(), "limit": 2,
                           "captured_through": "2026-07-23T10:00:00+08:00"}),
        ),
    ];
    for (index, (operation, schema, version, data)) in cases.into_iter().enumerate() {
        let command = fixture_command(
            operation,
            "Sina",
            schema,
            version,
            &serde_json::to_vec(&data).unwrap(),
        );
        let error = registry.execute(command).unwrap_err();
        assert!(
            matches!(
                error, ServiceError::Unavailable { operation: failed_operation, ref reason }
                    if failed_operation == operation && reason == "offline Sina source outage"
            ),
            "{operation:?}: {error:?}"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            index + 1,
            "{operation:?} did not reach the pinned offline source once"
        );
    }
}

fn daily_bars_body() -> Vec<u8> {
    br#"{"code":0,"data":{"sh600396":{"day":[
        ["2026-07-22","9.0","10.0","11.0","8.0","100"],
        ["2026-07-23","10.0","11.0","12.0","9.0","200"]
    ]}}}"#
        .to_vec()
}

#[test]
fn registered_tencent_bars_preserve_caller_bound_date_precision_and_absent_amount() {
    let (registry, calls) = tencent_registry(daily_bars_body(), 65_536);
    for limit in [1_u16, 2] {
        let request = BarsRequest::new(equity(), BarInterval::Day, limit).unwrap();
        let result = registry
            .execute(typed_command(
                Operation::HistoricalBars,
                HISTORICAL_BARS_REQUEST_SCHEMA,
                &request,
            ))
            .unwrap();
        assert_eq!(result.records.len(), usize::from(limit));
        assert!(result.repository_admitted && result.complete);
        assert_eq!(result.provider, "Tencent");
        assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
        let last = result.records.last().unwrap();
        assert_eq!(last.schema(), HISTORICAL_BARS_RECORD_SCHEMA);
        let bar: Bar = serde_json::from_slice(last.data()).unwrap();
        assert_eq!(bar.instrument(), &equity());
        assert_eq!(bar.provider(), ProviderId::Tencent);
        assert_eq!(bar.interval(), BarInterval::Day);
        assert_eq!(bar.open().get(), 10.0);
        assert_eq!(bar.high().get(), 12.0);
        assert_eq!(bar.low().get(), 9.0);
        assert_eq!(bar.close().get(), 11.0);
        assert_eq!(bar.volume().get(), 200.0);
        assert!(bar.amount().is_none());
        assert_eq!(bar.source_at(), Some("2026-07-23"));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn registered_tencent_handlers_reject_unsupported_scopes_and_bounds_before_source() {
    let (registry, calls) = tencent_registry(Vec::new(), 65_536);
    let beijing = InstrumentId::new(Exchange::Beijing, "836077", AssetClass::Equity).unwrap();
    let examples = [
        (
            "bar range",
            Operation::HistoricalBars,
            HISTORICAL_BARS_REQUEST_SCHEMA,
            false,
            serde_json::to_value(
                BarsRequest::new(equity(), BarInterval::Day, 2)
                    .unwrap()
                    .with_range("2026-07-22", "2026-07-23")
                    .unwrap(),
            )
            .unwrap(),
        ),
        (
            "bar count",
            Operation::HistoricalBars,
            HISTORICAL_BARS_REQUEST_SCHEMA,
            true,
            serde_json::to_value(BarsRequest::new(equity(), BarInterval::Day, 801).unwrap())
                .unwrap(),
        ),
        (
            "year bars",
            Operation::HistoricalBars,
            HISTORICAL_BARS_REQUEST_SCHEMA,
            false,
            serde_json::to_value(BarsRequest::new(equity(), BarInterval::Year, 2).unwrap())
                .unwrap(),
        ),
        (
            "Beijing intraday",
            Operation::HistoricalBars,
            HISTORICAL_BARS_REQUEST_SCHEMA,
            false,
            serde_json::to_value(
                BarsRequest::new(beijing.clone(), BarInterval::Minute5, 2).unwrap(),
            )
            .unwrap(),
        ),
        (
            "trade date",
            Operation::Trades,
            TRADES_REQUEST_SCHEMA,
            false,
            serde_json::to_value(
                TradesRequest::new(equity(), 2)
                    .unwrap()
                    .with_date("2026-07-23")
                    .unwrap(),
            )
            .unwrap(),
        ),
        (
            "trade count",
            Operation::Trades,
            TRADES_REQUEST_SCHEMA,
            true,
            serde_json::to_value(TradesRequest::new(equity(), 2001).unwrap()).unwrap(),
        ),
        (
            "Beijing trades",
            Operation::Trades,
            TRADES_REQUEST_SCHEMA,
            false,
            serde_json::to_value(TradesRequest::new(beijing, 2).unwrap()).unwrap(),
        ),
    ];
    for (name, operation, schema, invalid, data) in examples {
        let error = registry
            .execute(typed_command(operation, schema, &data))
            .unwrap_err();
        if invalid {
            assert!(
                matches!(error, ServiceError::InvalidRequest(_)),
                "{name}: {error:?}"
            );
        } else {
            assert!(
                matches!(error, ServiceError::Unsupported { operation: failed, .. }
                            if failed == operation),
                "{name}: {error:?}"
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0, "{name} reached source");
    }
    for (operation, schema) in [
        (Operation::RealtimeQuotes, REALTIME_QUOTES_REQUEST_SCHEMA),
        (Operation::OrderBooks, ORDER_BOOKS_REQUEST_SCHEMA),
        (
            Operation::SecurityMetadata,
            SECURITY_METADATA_REQUEST_SCHEMA,
        ),
        (
            Operation::MarketStatistics,
            MARKET_STATISTICS_REQUEST_SCHEMA,
        ),
    ] {
        for instruments in [Vec::<InstrumentId>::new(), vec![equity(), equity()]] {
            let error = registry
                .execute(typed_command(
                    operation,
                    schema,
                    &serde_json::json!({"instruments": instruments}),
                ))
                .unwrap_err();
            assert!(
                matches!(error, ServiceError::InvalidRequest(_)),
                "{operation:?}: {error:?}"
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
    }
}

#[test]
fn registered_intraday_shape_refuses_windows_that_cannot_support_the_requested_summary() {
    let examples = [
        (
            "post-market only",
            historical_minutes(&["1510 10.00 10 10000.00"]),
            Some("2026-07-23"),
            2,
            "input has no regular-session points",
        ),
        (
            "requested point ceiling",
            historical_minutes(&["0930 10.00 10 10000.00", "0931 11.00 20 21000.00"]),
            Some("2026-07-23"),
            1,
            "exceeds requested maximum_points 1",
        ),
        (
            "unordered source",
            historical_minutes(&["0931 10.00 10 10000.00", "0930 11.00 20 21000.00"]),
            Some("2026-07-23"),
            2,
            "minute rows are duplicated or unordered",
        ),
        (
            "stale current source",
            br#"{"code":0,"data":{"sh600396":{"data":{
            "date":"19990104","data":["0930 10.00 10 10000.00"]
         }}}}"#
                .to_vec(),
            None,
            2,
            "is not the current China date",
        ),
    ];
    for (name, body, date, maximum_points, expected_reason) in examples {
        let (registry, calls) = tencent_registry(body, 65_536);
        let data = serde_json::json!({
            "instrument": equity(), "trading_date": date, "maximum_points": maximum_points
        });
        let command = fixture_command(
            Operation::IntradayShape,
            "LocalAnalysis",
            INTRADAY_SHAPE_REQUEST_SCHEMA,
            1,
            &serde_json::to_vec(&data).unwrap(),
        );
        let error = registry.execute(command).unwrap_err();
        assert!(
            matches!(error, ServiceError::FailedPrecondition(ref reason)
                        if reason.contains(expected_reason)),
            "{name}: {error:?}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1, "{name}");
    }
}

struct OfflineSinaTransport {
    body: Vec<u8>,
    calls: Arc<AtomicUsize>,
}

impl magic_sina_rs::SnapshotTransport for OfflineSinaTransport {
    fn get(&self, _url: &str) -> Result<Vec<u8>, SinaError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.body.clone())
    }
}

fn sina_registry(body: Vec<u8>, ceiling: usize) -> (OperationRegistry, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = SinaClient::with_transport(OfflineSinaTransport {
        body,
        calls: calls.clone(),
    });
    let mut registry = OperationRegistry::all_unadmitted("offline source fixture only");
    register_sina_parity(&mut registry, client, ceiling).unwrap();
    (registry, calls)
}

#[test]
fn registered_sina_snapshot_handlers_keep_lot_units_and_unproved_metadata_absent() {
    let body = br#"var hq_str_sh600396="Offline,9.50,9.00,10.00,11.00,9.00,9.99,10.01,1000,10000.00,1000,9.99,1000,9.98,1000,9.97,1000,9.96,1000,9.95,2000,10.01,2000,10.02,2000,10.03,2000,10.04,2000,10.05,2026-07-23,09:49:07,00";"#;
    let (registry, calls) = sina_registry(body.to_vec(), 65_536);
    let data = serde_json::to_vec(&serde_json::json!({"instruments": [equity()]})).unwrap();
    let quote_result = registry
        .execute(fixture_command(
            Operation::RealtimeQuotes,
            "Sina",
            REALTIME_QUOTES_REQUEST_SCHEMA,
            1,
            &data,
        ))
        .unwrap();
    assert!(quote_result.repository_admitted && quote_result.complete);
    assert_eq!(quote_result.provider, "Sina");
    assert_eq!(
        quote_result.source_at.as_deref(),
        Some("2026-07-23T09:49:07+08:00")
    );
    let quote: Quote = serde_json::from_slice(quote_result.records[0].data()).unwrap();
    assert_eq!(quote.provider(), ProviderId::Sina);
    assert_eq!(quote.price().get(), 10.0);
    assert_eq!(quote.volume().get(), 10.0);
    assert_eq!(quote.amount().unwrap().get(), 10_000.0);

    let book_result = registry
        .execute(fixture_command(
            Operation::OrderBooks,
            "Sina",
            ORDER_BOOKS_REQUEST_SCHEMA,
            1,
            &data,
        ))
        .unwrap();
    assert!(book_result.complete);
    let book: OrderBook = serde_json::from_slice(book_result.records[0].data()).unwrap();
    assert_eq!(book.provider(), ProviderId::Sina);
    assert_eq!(book.bids()[0].price().unwrap().get(), 9.99);
    assert_eq!(book.bids()[0].quantity().unwrap().get(), 10.0);
    assert_eq!(book.asks()[0].price().unwrap().get(), 10.01);
    assert_eq!(book.asks()[0].quantity().unwrap().get(), 20.0);

    let metadata_result = registry
        .execute(fixture_command(
            Operation::SecurityMetadata,
            "Sina",
            SECURITY_METADATA_REQUEST_SCHEMA,
            1,
            &data,
        ))
        .unwrap();
    assert!(metadata_result.repository_admitted && !metadata_result.complete);
    assert_eq!(
        metadata_result.source_at.as_deref(),
        Some("2026-07-23T09:49:07+08:00")
    );
    let metadata: magic_market_core::SecurityMetadata =
        serde_json::from_slice(metadata_result.records[0].data()).unwrap();
    assert_eq!(metadata.name(), Some("Offline"));
    assert!(metadata.listed_on().is_none());
    assert!(metadata.price_limit().version().is_none());
    assert_eq!(metadata.status(), DataStatus::Unavailable);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn registered_sina_kline_handlers_preserve_date_precision_and_latest_session_totals() {
    let daily = br#"[
        {"day":"2026-07-22","open":"9","high":"10","low":"8","close":"9","volume":"1000","amount":"9000"},
        {"day":"2026-07-23","open":"10","high":"11","low":"9","close":"10","volume":"2000","amount":"20000"}
    ]"#;
    let (registry, calls) = sina_registry(daily.to_vec(), 65_536);
    let request = BarsRequest::new(equity(), BarInterval::Day, 2).unwrap();
    let result = registry
        .execute(fixture_command(
            Operation::HistoricalBars,
            "Sina",
            HISTORICAL_BARS_REQUEST_SCHEMA,
            1,
            &serde_json::to_vec(&request).unwrap(),
        ))
        .unwrap();
    assert!(result.repository_admitted && result.complete);
    assert_eq!(result.records.len(), 2);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
    let bar: Bar = serde_json::from_slice(result.records[1].data()).unwrap();
    assert_eq!(bar.provider(), ProviderId::Sina);
    assert_eq!(bar.close().get(), 10.0);
    assert_eq!(bar.volume().get(), 20.0);
    assert_eq!(bar.amount().unwrap().get(), 20_000.0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let minute = br#"[
        {"day":"2026-07-22 15:00:00","open":"9","high":"9","low":"9","close":"9","volume":"100","amount":"900"},
        {"day":"2026-07-23 09:30:00","open":"10","high":"10","low":"10","close":"10","volume":"1000","amount":"10000"},
        {"day":"2026-07-23 09:31:00","open":"11","high":"11","low":"11","close":"11","volume":"2000","amount":"22000"}
    ]"#;
    let (registry, calls) = sina_registry(minute.to_vec(), 65_536);
    let result = registry
        .execute(fixture_command(
            Operation::MinuteData,
            "Sina",
            MINUTE_DATA_REQUEST_SCHEMA,
            1,
            &serde_json::to_vec(&MinuteDataRequest::new(equity())).unwrap(),
        ))
        .unwrap();
    assert!(result.repository_admitted && result.complete);
    assert_eq!(result.provider, "Sina");
    assert_eq!(result.records.len(), 2);
    assert_eq!(
        result.source_at.as_deref(),
        Some("2026-07-23T09:31:00+08:00")
    );
    let point: MinutePoint = serde_json::from_slice(result.records[1].data()).unwrap();
    assert_eq!(point.provider(), ProviderId::Sina);
    assert_eq!(point.minute_at(), "2026-07-23 09:31");
    assert_eq!(point.cumulative_quantity().get(), 30.0);
    assert_eq!(point.cumulative_amount().unwrap().get(), 32_000.0);
    assert_eq!(point.status(), DataStatus::Available);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn registered_tencent_handlers_fail_atomically_when_the_response_payload_ceiling_is_too_small() {
    let examples = [
        (
            Operation::HistoricalBars,
            HISTORICAL_BARS_REQUEST_SCHEMA,
            "Tencent",
            daily_bars_body(),
            serde_json::to_value(BarsRequest::new(equity(), BarInterval::Day, 2).unwrap()).unwrap(),
        ),
        (
            Operation::MinuteData,
            MINUTE_DATA_REQUEST_SCHEMA,
            "Tencent",
            historical_minutes(&["0930 10.00 10 10000.00", "0931 11.00 20 21000.00"]),
            serde_json::to_value(
                MinuteDataRequest::new(equity())
                    .with_date("2026-07-23")
                    .unwrap(),
            )
            .unwrap(),
        ),
        (
            Operation::IntradayShape,
            INTRADAY_SHAPE_REQUEST_SCHEMA,
            "LocalAnalysis",
            historical_minutes(&["0930 10.00 10 10000.00", "0931 11.00 20 21000.00"]),
            serde_json::json!({"instrument": equity(), "trading_date": "2026-07-23", "maximum_points": 2}),
        ),
    ];
    for (operation, schema, provider, body, data) in examples {
        let (registry, calls) = tencent_registry(body, 1);
        let command = fixture_command(
            operation,
            provider,
            schema,
            1,
            &serde_json::to_vec(&data).unwrap(),
        );
        let error = registry.execute(command).unwrap_err();
        assert!(
            matches!(error, ServiceError::ResourceExhausted(ref reason)
                        if reason.contains("exceeds maximum 1")),
            "{operation:?}: {error:?}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
