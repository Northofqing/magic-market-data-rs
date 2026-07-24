use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;

#[test]
fn rejects_bar_ranges_instead_of_silently_ignoring_them() {
    let request = BarsRequest::new(instrument("600396"), BarInterval::Day, 5)
        .unwrap()
        .with_range("2026-07-01", "2026-07-22")
        .unwrap();
    assert!(matches!(
        reject_unsupported_bar_range(&request),
        Err(TdxError::Unsupported(_))
    ));
}

#[test]
fn maps_standard_one_minute_monthly_and_yearly_categories_exactly() {
    assert_eq!(category(BarInterval::Minute1).unwrap(), 8);
    assert_eq!(category(BarInterval::Month).unwrap(), 6);
    assert_eq!(category(BarInterval::Year).unwrap(), 11);
}

#[test]
fn order_book_levels_preserve_absence_atomically() {
    let absent = book_level(0.0, 0.0).unwrap();
    assert!(absent.price().is_none());
    assert!(absent.quantity().is_none());
    let half_present = book_level(0.0, 1.0).unwrap();
    assert!(half_present.price().is_none());
    assert!(half_present.quantity().is_none());
    assert!(book_level(10.0, -1.0).is_err());
}
use magic_market_core::{AssetClass, Exchange};

fn instrument(code: &str) -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap()
}

fn source_bar() -> SecurityBar {
    SecurityBar {
        open: 10.0,
        close: 11.0,
        high: 12.0,
        low: 9.0,
        vol: 100.0,
        amount: 1_000.0,
        year: 2026,
        month: 7,
        day: 23,
        hour: 0,
        minute: 0,
        datetime: "2026-07-23".into(),
    }
}

#[derive(Default)]
struct ScriptedBarsQuery {
    calls: RefCell<Vec<(u8, u8, String, u32, u16, u8)>>,
    response: RefCell<Option<Result<Vec<SecurityBar>, TdxError>>>,
    quote_calls: RefCell<Vec<Vec<(u8, String)>>>,
    quote_response: RefCell<Option<Result<Vec<SecurityQuote>, TdxError>>>,
    minute_calls: RefCell<Vec<(u8, String)>>,
    minute_response: RefCell<Option<Result<Vec<MinuteTimePrice>, TdxError>>>,
    history_minute_calls: RefCell<Vec<(u8, String, u32)>>,
    history_minute_response: RefCell<Option<Result<Vec<MinuteTimePrice>, TdxError>>>,
    transaction_calls: RefCell<Vec<(u8, String, u16, u16)>>,
    transaction_responses: RefCell<VecDeque<Result<Vec<TickData>, TdxError>>>,
    history_transaction_calls: RefCell<Vec<(u8, String, u16, u16, u32)>>,
    history_transaction_responses: RefCell<VecDeque<Result<Vec<TickData>, TdxError>>>,
    security_count_calls: RefCell<Vec<u8>>,
    security_count_responses: RefCell<VecDeque<Result<u16, TdxError>>>,
    security_list_calls: RefCell<Vec<(u8, u16)>>,
    security_list_responses: RefCell<VecDeque<Result<Vec<SecurityInfo>, TdxError>>>,
}

impl BlockingTdxQuery for ScriptedBarsQuery {
    fn security_bars(
        &self,
        category: u8,
        market: u8,
        code: &str,
        start: u32,
        count: u16,
        adjust: u8,
    ) -> Result<Vec<SecurityBar>, TdxError> {
        self.calls
            .borrow_mut()
            .push((category, market, code.to_owned(), start, count, adjust));
        self.response.borrow_mut().take().unwrap_or_else(|| {
            Err(TdxError::InvalidData(
                "scripted bars response is not configured".into(),
            ))
        })
    }

    fn security_quotes(
        &self,
        instruments: &[(u8, &str)],
    ) -> Result<Vec<SecurityQuote>, TdxError> {
        self.quote_calls.borrow_mut().push(
            instruments
                .iter()
                .map(|(market, code)| (*market, (*code).to_owned()))
                .collect(),
        );
        self.quote_response.borrow_mut().take().unwrap_or_else(|| {
            Err(TdxError::InvalidData(
                "scripted quote response is not configured".into(),
            ))
        })
    }

    fn minute_time_data(
        &self,
        market: u8,
        code: &str,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        self.minute_calls
            .borrow_mut()
            .push((market, code.to_owned()));
        self.minute_response.borrow_mut().take().unwrap_or_else(|| {
            Err(TdxError::InvalidData(
                "scripted current minute response is not configured".into(),
            ))
        })
    }

    fn history_minute_time_data(
        &self,
        market: u8,
        code: &str,
        date: u32,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        self.history_minute_calls
            .borrow_mut()
            .push((market, code.to_owned(), date));
        self.history_minute_response
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                Err(TdxError::InvalidData(
                    "scripted history minute response is not configured".into(),
                ))
            })
    }

    fn transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<Vec<TickData>, TdxError> {
        self.transaction_calls
            .borrow_mut()
            .push((market, code.to_owned(), start, count));
        self.transaction_responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| {
                Err(TdxError::InvalidData(
                    "scripted current transaction response is not configured".into(),
                ))
            })
    }

    fn history_transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
        date: u32,
    ) -> Result<Vec<TickData>, TdxError> {
        self.history_transaction_calls.borrow_mut().push((
            market,
            code.to_owned(),
            start,
            count,
            date,
        ));
        self.history_transaction_responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| {
                Err(TdxError::InvalidData(
                    "scripted history transaction response is not configured".into(),
                ))
            })
    }

    fn security_count(&self, market: u8) -> Result<u16, TdxError> {
        self.security_count_calls.borrow_mut().push(market);
        self.security_count_responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| {
                Err(TdxError::InvalidData(
                    "scripted security count response is not configured".into(),
                ))
            })
    }

    fn security_list(&self, market: u8, start: u16) -> Result<Vec<SecurityInfo>, TdxError> {
        self.security_list_calls.borrow_mut().push((market, start));
        self.security_list_responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| {
                Err(TdxError::InvalidData(
                    "scripted security list response is not configured".into(),
                ))
            })
    }
}

#[test]
fn blocking_bar_seam_uses_decoded_records_and_exact_request_parameters() {
    let query = ScriptedBarsQuery {
        response: RefCell::new(Some(Ok(vec![source_bar()]))),
        ..Default::default()
    };
    let request = BarsRequest::new(instrument("600396"), BarInterval::Day, 5).unwrap();

    let batch = historical_bars_with(&query, "tdx", &request).unwrap();

    assert_eq!(
        *query.calls.borrow(),
        vec![(KLINE_DAILY, 1, "600396".into(), 0, 5, 0)]
    );
    assert_eq!(batch.records().len(), 1);
    assert_eq!(batch.provenance().source(), "tdx");
    assert_eq!(batch.provenance().source_at(), Some("2026-07-23"));
}

#[test]
fn blocking_quote_seam_restores_order_and_preserves_source_label() {
    let query = ScriptedBarsQuery {
        quote_response: RefCell::new(Some(Ok(vec![
            source_quote("600002", 102.0),
            source_quote("600001", 101.0),
        ]))),
        ..Default::default()
    };
    let instruments = [instrument("600001"), instrument("600002")];

    let batch = realtime_quotes_with(&query, "tdx-smart", &instruments).unwrap();

    assert_eq!(
        *query.quote_calls.borrow(),
        vec![vec![(1, "600001".into()), (1, "600002".into())]]
    );
    assert_eq!(batch.records()[0].instrument().code(), "600001");
    assert_eq!(batch.records()[0].price().get(), 101.0);
    assert_eq!(batch.records()[1].instrument().code(), "600002");
    assert_eq!(batch.provenance().source(), "tdx-smart");
}

#[test]
fn blocking_historical_minute_seam_uses_explicit_source_date() {
    let query = ScriptedBarsQuery {
        history_minute_response: RefCell::new(Some(Ok(vec![MinuteTimePrice {
            time: "09:31".into(),
            price: 15.4,
            avg_price: 15.4,
            vol: 10.0,
        }]))),
        ..Default::default()
    };
    let request = MinuteDataRequest::new(instrument("600396"))
        .with_date("2026-07-23")
        .unwrap();

    let batch = minute_data_with(&query, "tdx", &request).unwrap();

    assert!(query.minute_calls.borrow().is_empty());
    assert_eq!(
        *query.history_minute_calls.borrow(),
        vec![(1, "600396".into(), 20260723)]
    );
    assert_eq!(batch.records()[0].minute_at(), "2026-07-23 09:31");
    assert_eq!(batch.provenance().source(), "tdx");
    assert_eq!(
        batch.provenance().source_at(),
        Some("2026-07-23T09:31:00+08:00")
    );
}

#[test]
fn blocking_trade_seam_uses_historical_query_and_stops_on_short_page() {
    let query = ScriptedBarsQuery {
        history_transaction_responses: RefCell::new(VecDeque::from([Ok(vec![
            source_trade(0, 0),
            source_trade(1, 1),
        ])])),
        ..Default::default()
    };
    let request = TradesRequest::new(instrument("600519"), 3)
        .unwrap()
        .with_date("2026-07-21")
        .unwrap();

    let batch = trades_with(&query, "tdx-current", "tdx-history", &request).unwrap();

    assert!(query.transaction_calls.borrow().is_empty());
    assert_eq!(
        *query.history_transaction_calls.borrow(),
        vec![(1, "600519".into(), 0, 3, 20260721)]
    );
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].trade_at(), "2026-07-21 10:00:00");
    assert_eq!(batch.provenance().source(), "tdx-history");
}

#[test]
fn blocking_metadata_seam_uses_declared_count_and_source_backed_page() {
    let query = ScriptedBarsQuery {
        security_count_responses: RefCell::new(VecDeque::from([Ok(2)])),
        security_list_responses: RefCell::new(VecDeque::from([Ok(vec![
            SecurityInfo {
                code: "600001".into(),
                volunit: 100,
                decimal_point: 2,
                name: "甲公司".into(),
                pre_close: 10.0,
            },
            SecurityInfo {
                code: "600002".into(),
                volunit: 100,
                decimal_point: 2,
                name: "乙公司".into(),
                pre_close: 20.0,
            },
        ])])),
        ..Default::default()
    };
    let instruments = [instrument("600002"), instrument("600001")];

    let batch = security_metadata_with(&query, "tdx", &instruments).unwrap();

    assert_eq!(*query.security_count_calls.borrow(), vec![1]);
    assert_eq!(*query.security_list_calls.borrow(), vec![(1, 0)]);
    assert_eq!(batch.records()[0].instrument().code(), "600002");
    assert_eq!(batch.records()[0].name(), Some("乙公司"));
    assert_eq!(batch.records()[1].name(), Some("甲公司"));
    assert_eq!(batch.provenance().source(), "tdx");
}

#[test]
fn blocking_order_book_seam_normalizes_five_levels_once() {
    let query = ScriptedBarsQuery {
        quote_response: RefCell::new(Some(Ok(vec![source_quote("600001", 101.0)]))),
        ..Default::default()
    };
    let instruments = [instrument("600001")];

    let batch = order_books_with(&query, "TDX smart", "tdx-smart", &instruments).unwrap();

    let book = &batch.records()[0];
    assert_eq!(book.bids()[0].price().unwrap().get(), 101.9);
    assert_eq!(book.asks()[4].quantity().unwrap().get(), 19.0);
    assert_eq!(book.total_bid_quantity().unwrap().get(), 60.0);
    assert_eq!(book.total_ask_quantity().unwrap().get(), 85.0);
    assert_eq!(book.status(), DataStatus::Unavailable);
    assert_eq!(book.provider(), ProviderId::Tdx);
    assert_eq!(batch.provenance().source(), "tdx-smart");
    assert_eq!(batch.quality().issues().len(), 1);
}

#[derive(Default)]
struct ScriptedAsyncBarsQuery {
    calls: RefCell<Vec<(u8, u8, String, u32, u16, u8)>>,
    response: RefCell<Option<Result<Vec<SecurityBar>, TdxError>>>,
    quote_calls: RefCell<Vec<Vec<(u8, String)>>>,
    quote_response: RefCell<Option<Result<Vec<SecurityQuote>, TdxError>>>,
    transaction_calls: RefCell<Vec<(u8, String, u16, u16)>>,
    transaction_responses: RefCell<VecDeque<Result<Vec<TickData>, TdxError>>>,
    history_transaction_calls: RefCell<Vec<(u8, String, u16, u16, u32)>>,
    history_transaction_responses: RefCell<VecDeque<Result<Vec<TickData>, TdxError>>>,
}

impl AsyncTdxQuery for ScriptedAsyncBarsQuery {
    async fn security_bars(
        &self,
        category: u8,
        market: u8,
        code: &str,
        start: u32,
        count: u16,
        adjust: u8,
    ) -> Result<Vec<SecurityBar>, TdxError> {
        self.calls
            .borrow_mut()
            .push((category, market, code.to_owned(), start, count, adjust));
        self.response.borrow_mut().take().unwrap_or_else(|| {
            Err(TdxError::InvalidData(
                "scripted async bars response is not configured".into(),
            ))
        })
    }

    async fn security_quotes(
        &self,
        instruments: &[(u8, &str)],
    ) -> Result<Vec<SecurityQuote>, TdxError> {
        self.quote_calls.borrow_mut().push(
            instruments
                .iter()
                .map(|(market, code)| (*market, (*code).to_owned()))
                .collect(),
        );
        self.quote_response.borrow_mut().take().unwrap_or_else(|| {
            Err(TdxError::InvalidData(
                "scripted async quote response is not configured".into(),
            ))
        })
    }

    async fn transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<Vec<TickData>, TdxError> {
        self.transaction_calls
            .borrow_mut()
            .push((market, code.to_owned(), start, count));
        self.transaction_responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| {
                Err(TdxError::InvalidData(
                    "scripted async current transaction response is not configured".into(),
                ))
            })
    }

    async fn history_transaction_data(
        &self,
        market: u8,
        code: &str,
        start: u16,
        count: u16,
        date: u32,
    ) -> Result<Vec<TickData>, TdxError> {
        self.history_transaction_calls.borrow_mut().push((
            market,
            code.to_owned(),
            start,
            count,
            date,
        ));
        self.history_transaction_responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| {
                Err(TdxError::InvalidData(
                    "scripted async history transaction response is not configured".into(),
                ))
            })
    }

    async fn security_count(&self, _market: u8) -> Result<u16, TdxError> {
        Err(TdxError::InvalidData(
            "scripted async security count response is not configured".into(),
        ))
    }

    async fn security_list(
        &self,
        _market: u8,
        _start: u16,
    ) -> Result<Vec<SecurityInfo>, TdxError> {
        Err(TdxError::InvalidData(
            "scripted async security list response is not configured".into(),
        ))
    }

    async fn minute_time_data(
        &self,
        _market: u8,
        _code: &str,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        Err(TdxError::InvalidData(
            "scripted async minute response is not configured".into(),
        ))
    }

    async fn history_minute_time_data(
        &self,
        _market: u8,
        _code: &str,
        _date: u32,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        Err(TdxError::InvalidData(
            "scripted async history minute response is not configured".into(),
        ))
    }
}

#[tokio::test]
async fn async_bar_seam_uses_decoded_records_and_exact_source_label() {
    let query = ScriptedAsyncBarsQuery {
        response: RefCell::new(Some(Ok(vec![source_bar()]))),
        ..Default::default()
    };
    let request = BarsRequest::new(instrument("600396"), BarInterval::Day, 5).unwrap();

    let batch = historical_bars_async_with(&query, "tdx-async", &request)
        .await
        .unwrap();

    assert_eq!(
        *query.calls.borrow(),
        vec![(KLINE_DAILY, 1, "600396".into(), 0, 5, 0)]
    );
    assert_eq!(batch.provenance().source(), "tdx-async");
    assert_eq!(batch.provenance().source_at(), Some("2026-07-23"));
}

#[tokio::test]
async fn async_quote_seam_reorders_decoded_records() {
    let query = ScriptedAsyncBarsQuery {
        quote_response: RefCell::new(Some(Ok(vec![
            source_quote("600002", 102.0),
            source_quote("600001", 101.0),
        ]))),
        ..Default::default()
    };
    let instruments = [instrument("600001"), instrument("600002")];

    let batch = realtime_quotes_async_with(&query, "tdx-async", &instruments)
        .await
        .unwrap();

    assert_eq!(
        *query.quote_calls.borrow(),
        vec![vec![(1, "600001".into()), (1, "600002".into())]]
    );
    assert_eq!(batch.records()[0].instrument().code(), "600001");
    assert_eq!(batch.records()[1].instrument().code(), "600002");
    assert_eq!(batch.provenance().source(), "tdx-async");
}

#[tokio::test]
async fn async_trade_seam_uses_historical_query_and_short_terminal_page() {
    let query = ScriptedAsyncBarsQuery {
        history_transaction_responses: RefCell::new(VecDeque::from([Ok(vec![
            source_trade(0, 0),
            source_trade(1, 1),
        ])])),
        ..Default::default()
    };
    let request = TradesRequest::new(instrument("600519"), 3)
        .unwrap()
        .with_date("2026-07-21")
        .unwrap();

    let batch = trades_async_with(&query, &request).await.unwrap();

    assert!(query.transaction_calls.borrow().is_empty());
    assert_eq!(
        *query.history_transaction_calls.borrow(),
        vec![(1, "600519".into(), 0, 3, 20260721)]
    );
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.provenance().source(), "tdx-async-history");
}

fn source_quote(code: &str, price: f64) -> SecurityQuote {
    SecurityQuote {
        market: 1,
        code: code.into(),
        active1: 0,
        price,
        last_close: 100.0,
        open: 101.0,
        high: 103.0,
        low: 99.0,
        servertime: "10:00:01".into(),
        vol: 1_000.0,
        cur_vol: 10.0,
        amount: 102_000.0,
        s_vol: 400.0,
        b_vol: 600.0,
        bid1: 101.9,
        bid_vol1: 10.0,
        bid2: 101.8,
        bid_vol2: 11.0,
        bid3: 101.7,
        bid_vol3: 12.0,
        bid4: 101.6,
        bid_vol4: 13.0,
        bid5: 101.5,
        bid_vol5: 14.0,
        ask1: 102.1,
        ask_vol1: 15.0,
        ask2: 102.2,
        ask_vol2: 16.0,
        ask3: 102.3,
        ask_vol3: 17.0,
        ask4: 102.4,
        ask_vol4: 18.0,
        ask5: 102.5,
        ask_vol5: 19.0,
        reversed_bytes0: 0,
        reversed_bytes1: 0,
        reversed_bytes2: 0,
        reversed_bytes3: 0,
        reversed_bytes4: 0,
        reversed_bytes5: 0,
        reversed_bytes6: 0,
        reversed_bytes7: 0,
        reversed_bytes8: 0,
        reversed_bytes9: 0,
        active2: 0,
    }
}

#[test]
fn order_book_quotes_are_keyed_by_market_and_code() {
    let instruments = [instrument("600001"), instrument("600002")];
    let ordered = ordered_order_book_quotes(
        &instruments,
        vec![source_quote("600002", 102.0), source_quote("600001", 101.0)],
        "test",
    )
    .unwrap();
    assert_eq!(ordered[0].0.code(), "600001");
    assert_eq!(ordered[0].1.price, 101.0);
    assert_eq!(ordered[1].0.code(), "600002");
    assert_eq!(ordered[1].1.price, 102.0);

    assert!(ordered_order_book_quotes(&[], Vec::new(), "test").is_err());
    assert!(ordered_order_book_quotes(
        &[instrument("600001"), instrument("600001")],
        vec![source_quote("600001", 101.0)],
        "test",
    )
    .is_err());
    assert!(ordered_order_book_quotes(
        &instruments,
        vec![source_quote("600001", 101.0), source_quote("600001", 102.0)],
        "test",
    )
    .is_err());
    assert!(ordered_order_book_quotes(
        &instruments,
        vec![source_quote("600001", 101.0), source_quote("600003", 103.0)],
        "test",
    )
    .is_err());
    assert!(
        ordered_order_book_quotes(&instruments, vec![source_quote("600001", 101.0)], "test",)
            .is_err()
    );
}

fn source_trade(index: u32, side: u32) -> TickData {
    TickData {
        time: format!("10:00:{index:02}"),
        price: 1_300.0 + f64::from(index),
        vol: 100.0 + f64::from(index),
        num: index + 1,
        buyorsell: side,
        reserved: 0,
    }
}

#[test]
fn normalized_quotes_restore_request_order_and_mark_missing_name() {
    let instruments = [instrument("600001"), instrument("600002")];
    let batch = normalize_quotes(
        "test",
        &instruments,
        vec![source_quote("600002", 101.0), source_quote("600001", 102.0)],
    )
    .unwrap();
    assert_eq!(batch.records()[0].instrument().code(), "600001");
    assert_eq!(batch.records()[0].price(), Price::new(102.0).unwrap());
    assert_eq!(
        batch.records()[0].change_percent(),
        Some(Ratio::new(2.0, RatioUnit::Percent).unwrap())
    );
    assert_eq!(batch.records()[0].status(), DataStatus::Unavailable);
    assert!(batch.records()[0].name().is_none());
    assert!(batch.records()[0].source_at().is_none());
    assert!(batch.provenance().source_at().is_none());
    assert_eq!(batch.quality().issues().len(), 6);
}

#[test]
fn normalized_quotes_reject_duplicates_and_missing_records() {
    let duplicated = [instrument("600001"), instrument("600001")];
    assert!(normalize_quotes("test", &duplicated, Vec::new()).is_err());

    let requested = [instrument("600001"), instrument("600002")];
    assert!(normalize_quotes("test", &requested, vec![source_quote("600001", 102.0)]).is_err());
}

#[test]
fn normalizes_only_source_backed_security_metadata() {
    let star = instrument("688001");
    let chinext = InstrumentId::new(Exchange::Shenzhen, "300001", AssetClass::Equity).unwrap();
    let records = vec![
        (
            0,
            SecurityInfo {
                code: "300001".into(),
                volunit: 100,
                decimal_point: 2,
                name: "*ST示例".into(),
                pre_close: 10.0,
            },
        ),
        (
            1,
            SecurityInfo {
                code: "688001".into(),
                volunit: 100,
                decimal_point: 2,
                name: "科创示例".into(),
                pre_close: 20.0,
            },
        ),
    ];

    let batch = normalize_security_metadata("test", &[star, chinext], records).unwrap();
    assert_eq!(batch.records()[0].board(), Some(Board::Star));
    assert_eq!(batch.records()[0].is_st(), Some(false));
    assert_eq!(batch.records()[1].board(), Some(Board::ChiNext));
    assert_eq!(batch.records()[1].is_st(), Some(true));
    assert!(batch
        .records()
        .iter()
        .all(|record| record.listed_on().is_none()
            && record.price_limit().percent().is_none()
            && record.price_limit().version().is_none()
            && record.status() == DataStatus::Unavailable));
    assert!(!batch.quality().is_complete());
}

#[test]
fn beijing_uses_the_live_verified_tdx_market_number() {
    let beijing = InstrumentId::new(Exchange::Beijing, "920001", AssetClass::Equity).unwrap();
    assert_eq!(market(&beijing).unwrap(), 2);
    assert_eq!(market(&instrument("600001")).unwrap(), 1);
    let shenzhen = InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap();
    assert_eq!(market(&shenzhen).unwrap(), 0);
}

#[test]
fn rejects_beijing_security_metadata_before_transport() {
    let beijing = InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap();
    let error = validate_security_metadata_request(&[beijing]).unwrap_err();
    assert!(matches!(error, TdxError::Unsupported(_)));
    assert!(error.to_string().contains("security-list"));
}

#[test]
fn normalizes_tdx_minute_rows_into_cumulative_chronological_points() {
    let records = vec![
        MinuteTimePrice {
            time: "09:32".into(),
            price: 15.6,
            avg_price: 15.5,
            vol: 20.0,
        },
        MinuteTimePrice {
            time: "09:31".into(),
            price: 15.4,
            avg_price: 15.4,
            vol: 10.0,
        },
    ];
    let batch =
        normalize_minute_records("test", &instrument("600396"), "2026-07-23", records).unwrap();
    assert_eq!(batch.records()[0].minute_at(), "2026-07-23 09:31");
    assert_eq!(batch.records()[0].cumulative_quantity().get(), 10.0);
    assert_eq!(batch.records()[1].cumulative_quantity().get(), 30.0);
    assert_eq!(
        batch.records()[1].source_at(),
        Some("2026-07-23T09:32:00+08:00")
    );
    assert!(batch.records()[1].cumulative_amount().is_none());
}

#[test]
fn paginates_and_normalizes_historical_trades() {
    let request = TradesRequest::new(instrument("600519"), 5)
        .unwrap()
        .with_date("2026-07-21")
        .unwrap();
    let mut calls = Vec::new();
    let batch = paginate_trades("test", &request, 2, |start, count| {
        calls.push((start, count));
        Ok((start..start + count)
            .map(|index| source_trade(u32::from(index), u32::from(index % 3)))
            .collect())
    })
    .unwrap();
    assert_eq!(calls, vec![(0, 2), (2, 2), (4, 1)]);
    assert_eq!(batch.records().len(), 5);
    assert_eq!(batch.records()[0].trade_at(), "2026-07-21 10:00:00");
    assert_eq!(batch.records()[0].source_at(), Some("2026-07-21 10:00:00"));
    assert_eq!(batch.records()[0].side(), TradeSide::Buy);
    assert_eq!(batch.records()[1].side(), TradeSide::Sell);
    assert_eq!(batch.records()[2].side(), TradeSide::Neutral);
}

#[test]
fn marks_unknown_trade_side_without_dropping_the_record() {
    let request = TradesRequest::new(instrument("600519"), 1).unwrap();
    let batch = normalize_trade_records("test", &request, vec![source_trade(0, 9)]).unwrap();
    assert_eq!(batch.records()[0].side(), TradeSide::Unknown(9));
    assert_eq!(batch.records()[0].status(), DataStatus::Unavailable);
    assert_eq!(batch.quality().issues().len(), 1);
}

#[test]
fn maps_every_core_bar_interval_and_accepts_requests_without_ranges() {
    let cases = [
        (BarInterval::Minute1, KLINE_1MIN),
        (BarInterval::Minute5, KLINE_5MIN),
        (BarInterval::Minute15, KLINE_15MIN),
        (BarInterval::Minute30, KLINE_30MIN),
        (BarInterval::Hour1, KLINE_1HOUR),
        (BarInterval::Day, KLINE_DAILY),
        (BarInterval::Week, KLINE_WEEKLY),
        (BarInterval::Month, KLINE_MONTHLY),
        (BarInterval::Year, KLINE_YEARLY),
    ];
    for (interval, expected) in cases {
        assert_eq!(category(interval).unwrap(), expected);
    }
    let request = BarsRequest::new(instrument("600396"), BarInterval::Day, 1).unwrap();
    assert!(reject_unsupported_bar_range(&request).is_ok());
}

#[test]
fn strict_bar_batches_require_records_and_preserve_source_time() {
    assert!(ensure_nonempty::<u8>(&[]).is_err());
    assert!(ensure_nonempty(&[1]).is_ok());
    assert!(fetched_at().unwrap().parse::<u64>().is_ok());

    let source = SecurityBar {
        open: 10.0,
        close: 11.0,
        high: 12.0,
        low: 9.0,
        vol: 100.0,
        amount: 1_000.0,
        year: 2026,
        month: 7,
        day: 23,
        hour: 0,
        minute: 0,
        datetime: "2026-07-23".into(),
    };
    let provenance = bars_provenance("test", std::slice::from_ref(&source)).unwrap();
    assert_eq!(provenance.source_at(), Some("2026-07-23"));
    assert!(bars_provenance("test", &[]).unwrap().source_at().is_none());

    let batch = strict_bars("test", vec![source]).unwrap();
    assert_eq!(batch.records().len(), 1);
    assert_eq!(batch.provenance().source_at(), Some("2026-07-23"));
    assert!(strict_bars("test", Vec::new()).is_err());
}

#[test]
fn date_and_session_helpers_reject_ambiguous_values() {
    assert_eq!(compact_date("2026-07-23").unwrap(), 20260723);
    assert_eq!(compact_date("20260723").unwrap(), 20260723);
    assert!(compact_date("2026/07/23").is_err());
    assert!(compact_date("2026-7-23").is_err());
    assert_eq!(display_date(20260723).unwrap(), "2026-07-23");
    assert!(display_date(u32::MAX).is_err());

    for value in ["09:31", "11:30", "13:01", "15:00"] {
        assert!(valid_tdx_minute(value));
    }
    for value in ["09:30", "11:31", "13:00", "15:01", "bad"] {
        assert!(!valid_tdx_minute(value));
    }
}

fn minute(time: &str, price: f64, volume: f64) -> MinuteTimePrice {
    MinuteTimePrice {
        time: time.into(),
        price,
        avg_price: price,
        vol: volume,
    }
}

#[test]
fn minute_normalization_rejects_empty_oversized_duplicate_and_bad_numeric_data() {
    let id = instrument("600396");
    assert!(normalize_minute_records("test", &id, "2026-07-23", Vec::new()).is_err());
    assert!(normalize_minute_records(
        "test",
        &id,
        "2026-07-23",
        vec![minute("09:31", 10.0, 1.0); 241],
    )
    .is_err());
    assert!(
        normalize_minute_records("test", &id, "2026-07-23", vec![minute("09:30", 10.0, 1.0)],)
            .is_err()
    );
    assert!(normalize_minute_records(
        "test",
        &id,
        "2026-07-23",
        vec![minute("09:31", 10.0, 1.0), minute("09:31", 10.0, 1.0)],
    )
    .is_err());
    for volume in [-1.0, f64::NAN] {
        assert!(normalize_minute_records(
            "test",
            &id,
            "2026-07-23",
            vec![minute("09:31", 10.0, volume)],
        )
        .is_err());
    }
    assert!(normalize_minute_records(
        "test",
        &id,
        "2026-07-23",
        vec![
            minute("09:31", 10.0, f64::MAX),
            minute("09:32", 10.0, f64::MAX),
        ],
    )
    .is_err());
    assert!(
        normalize_minute_records("test", &id, "2026-07-23", vec![minute("09:31", 0.0, 1.0)],)
            .is_err()
    );
}

#[test]
fn optional_quote_prices_and_quote_numeric_fields_are_validated() {
    assert!(optional_quote_price(0.0, "field").unwrap().is_none());
    assert_eq!(
        optional_quote_price(10.0, "field").unwrap(),
        Some(Price::new(10.0).unwrap())
    );
    assert!(optional_quote_price(-1.0, "field").is_err());
    assert!(optional_quote_price(f64::NAN, "field").is_err());

    let id = instrument("600396");
    assert!(normalize_quotes("test", &[], Vec::new()).is_err());

    let mut invalid = source_quote("600396", 0.0);
    assert!(normalize_quotes("test", std::slice::from_ref(&id), vec![invalid.clone()]).is_err());
    invalid.price = 10.0;
    invalid.last_close = -1.0;
    assert!(normalize_quotes("test", std::slice::from_ref(&id), vec![invalid.clone()]).is_err());
    invalid.last_close = 9.0;
    invalid.vol = -1.0;
    assert!(normalize_quotes("test", std::slice::from_ref(&id), vec![invalid.clone()]).is_err());
    invalid.vol = 1.0;
    invalid.amount = -1.0;
    assert!(normalize_quotes("test", std::slice::from_ref(&id), vec![invalid]).is_err());

    let ids = [instrument("600001"), instrument("600002")];
    assert!(normalize_quotes(
        "test",
        &ids,
        vec![source_quote("600001", 10.0), source_quote("600001", 11.0)],
    )
    .is_err());
}

#[test]
fn trade_dates_records_and_pages_fail_explicitly() {
    assert_eq!(tdx_trade_date("2026-07-23").unwrap(), 20260723);
    for value in ["20260723", "2026/07/23", "2026-0x-23"] {
        assert!(tdx_trade_date(value).is_err());
    }

    let request = TradesRequest::new(instrument("600519"), 2).unwrap();
    assert!(normalize_trade_records("test", &request, Vec::new()).is_err());
    let mut invalid = source_trade(0, 0);
    invalid.time.clear();
    assert!(normalize_trade_records("test", &request, vec![invalid]).is_err());
    let mut invalid = source_trade(0, 0);
    invalid.price = 0.0;
    assert!(normalize_trade_records("test", &request, vec![invalid]).is_err());
    let mut invalid = source_trade(0, 0);
    invalid.vol = -1.0;
    assert!(normalize_trade_records("test", &request, vec![invalid]).is_err());

    let oversized = paginate_trades("test", &request, 1, |_, _| Ok(vec![source_trade(0, 0); 2]));
    assert!(oversized.is_err());
    let empty = paginate_trades("test", &request, 2, |_, _| Ok(Vec::new()));
    assert!(empty.is_err());
    let short = paginate_trades("test", &request, 2, |_, _| Ok(vec![source_trade(0, 0)])).unwrap();
    assert_eq!(short.records().len(), 1);
}

#[test]
fn board_and_st_classification_covers_every_supported_shape() {
    assert_eq!(board(&instrument("689001")), Board::Star);
    let chinext = InstrumentId::new(Exchange::Shenzhen, "301001", AssetClass::Equity).unwrap();
    assert_eq!(board(&chinext), Board::ChiNext);
    assert_eq!(board(&instrument("600396")), Board::Main);
    let beijing = InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap();
    assert_eq!(board(&beijing), Board::Beijing);
    let fund = InstrumentId::new(Exchange::Shanghai, "510050", AssetClass::Fund).unwrap();
    assert_eq!(board(&fund), Board::Unknown);

    assert_eq!(st_flag(""), None);
    for name in ["ST示例", "*ST示例", "S*ST示例", "SST示例"] {
        assert_eq!(st_flag(name), Some(true));
    }
    assert_eq!(st_flag("普通公司"), Some(false));
}

fn security_info(code: &str, name: &str) -> SecurityInfo {
    SecurityInfo {
        code: code.into(),
        volunit: 100,
        decimal_point: 2,
        name: name.into(),
        pre_close: 10.0,
    }
}

#[test]
fn security_list_fetching_and_metadata_normalization_reject_bad_cardinality() {
    assert!(fetch_security_records(&[], |_| Ok(0), |_, _| Ok(Vec::new())).is_err());
    let duplicate = [instrument("600396"), instrument("600396")];
    assert!(fetch_security_records(&duplicate, |_| Ok(1), |_, _| Ok(Vec::new())).is_err());

    let requested = [instrument("600396")];
    let records = fetch_security_records(
        &requested,
        |_| Ok(2),
        |_, start| {
            assert_eq!(start, 0);
            Ok(vec![
                security_info("600396", "示例"),
                security_info("600397", "其他"),
            ])
        },
    )
    .unwrap();
    assert_eq!(records.len(), 1);
    assert!(fetch_security_records(&requested, |_| Ok(1), |_, _| Ok(Vec::new())).is_err());
    assert!(fetch_security_records(
        &requested,
        |_| Ok(1),
        |_, _| Ok(vec![
            security_info("600396", "示例"),
            security_info("600397", "其他")
        ]),
    )
    .is_err());

    assert!(validate_security_metadata_request(&requested).is_ok());
    assert!(normalize_security_metadata(
        "test",
        &requested,
        vec![
            (1, security_info("600396", "示例")),
            (1, security_info("600396", "重复")),
        ],
    )
    .is_err());
    assert!(normalize_security_metadata("test", &requested, Vec::new()).is_err());
    assert!(normalize_security_metadata(
        "test",
        &requested,
        vec![
            (1, security_info("600396", "示例")),
            (1, security_info("600397", "额外")),
        ],
    )
    .is_err());

    let fund = InstrumentId::new(Exchange::Shanghai, "510050", AssetClass::Fund).unwrap();
    let batch = normalize_security_metadata(
        "test",
        std::slice::from_ref(&fund),
        vec![(1, security_info("510050", "   "))],
    )
    .unwrap();
    assert_eq!(batch.records()[0].board(), Some(Board::Unknown));
    assert_eq!(batch.records()[0].is_st(), None);
    assert!(batch.records()[0].name().is_none());
}

#[test]
fn order_book_helpers_validate_levels_depth_and_request_identity() {
    assert!(book_level(f64::NAN, 1.0).is_err());
    assert!(book_level(1.0, f64::INFINITY).is_err());
    let complete = book_level(10.0, 2.0).unwrap();
    assert_eq!(complete.price(), Some(Price::new(10.0).unwrap()));
    assert_eq!(complete.quantity(), Some(Quantity::new(2.0).unwrap()));

    let unavailable = std::array::from_fn(|_| BookLevel::unavailable());
    assert_eq!(book_depth(&unavailable).unwrap(), None);
    let filled = std::array::from_fn(|_| book_level(10.0, 2.0).unwrap());
    assert_eq!(
        book_depth(&filled).unwrap(),
        Some(Quantity::new(10.0).unwrap())
    );
    let overflowing = std::array::from_fn(|_| book_level(10.0, f64::MAX).unwrap());
    assert!(book_depth(&overflowing).is_err());

    let ids = [instrument("600001"), instrument("600002")];
    assert_eq!(
        order_book_pairs(&ids, "test").unwrap(),
        vec![(1, "600001"), (1, "600002")]
    );
}

#[test]
fn disconnected_clients_still_execute_all_deterministic_preflight_failures() {
    let client = TdxHqClient::new();
    let ranged = BarsRequest::new(instrument("600396"), BarInterval::Day, 1)
        .unwrap()
        .with_range("2026-07-01", "2026-07-23")
        .unwrap();
    assert!(matches!(
        <TdxHqClient as HistoricalBars>::historical_bars(&client, &ranged),
        Err(TdxError::Unsupported(_))
    ));
    assert!(<TdxHqClient as OrderBooks>::order_books(&client, &[]).is_err());

    let beijing = InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap();
    assert!(matches!(
        <TdxHqClient as SecurityMetadataProvider>::security_metadata(&client, &[beijing]),
        Err(TdxError::Unsupported(_))
    ));

    assert!(matches!(
        <TdxHqClient as MoneyFlows>::money_flows(&client, &[]),
        Err(TdxError::Unsupported(_))
    ));
    assert!(matches!(
        <TdxHqClient as Auctions>::auction_snapshots(&client, &[]),
        Err(TdxError::Unsupported(_))
    ));

    let smart = crate::TdxSmartClient::new();
    assert!(matches!(
        <crate::TdxSmartClient as HistoricalBars>::historical_bars(&smart, &ranged),
        Err(TdxError::Unsupported(_))
    ));
    assert!(<crate::TdxSmartClient as OrderBooks>::order_books(&smart, &[]).is_err());
    assert!(matches!(
        <crate::TdxSmartClient as MoneyFlows>::money_flows(&smart, &[]),
        Err(TdxError::Unsupported(_))
    ));
    assert!(matches!(
        <crate::TdxSmartClient as Auctions>::auction_snapshots(&smart, &[]),
        Err(TdxError::Unsupported(_))
    ));

    let direct = crate::TdxDirectClient::new("127.0.0.1", 7709, 0.1);
    assert!(matches!(
        <crate::TdxDirectClient as HistoricalBars>::historical_bars(&direct, &ranged),
        Err(TdxError::Unsupported(_))
    ));
}

#[tokio::test]
async fn async_client_rejects_unsupported_ranges_before_transport() {
    let client = crate::AsyncTdxHqClient::new();
    let ranged = BarsRequest::new(instrument("600396"), BarInterval::Day, 1)
        .unwrap()
        .with_range("2026-07-01", "2026-07-23")
        .unwrap();
    assert!(matches!(
        <crate::AsyncTdxHqClient as AsyncHistoricalBars>::historical_bars_async(&client, &ranged)
            .await,
        Err(TdxError::Unsupported(_))
    ));
}

#[tokio::test]
async fn disconnected_async_client_preserves_transport_failures_for_each_mapping() {
    let client = crate::AsyncTdxHqClient::new();
    let id = instrument("600396");
    let bars = BarsRequest::new(id.clone(), BarInterval::Day, 1).unwrap();
    assert!(
        <crate::AsyncTdxHqClient as AsyncHistoricalBars>::historical_bars_async(&client, &bars)
            .await
            .is_err()
    );
    assert!(
        <crate::AsyncTdxHqClient as AsyncRealtimeQuotes>::realtime_quotes_async(
            &client,
            std::slice::from_ref(&id),
        )
        .await
        .is_err()
    );
    let current = TradesRequest::new(id.clone(), 1).unwrap();
    assert!(
        <crate::AsyncTdxHqClient as AsyncTrades>::trades_async(&client, &current)
            .await
            .is_err()
    );
    let historical = TradesRequest::new(id, 1)
        .unwrap()
        .with_date("2026-07-23")
        .unwrap();
    assert!(
        <crate::AsyncTdxHqClient as AsyncTrades>::trades_async(&client, &historical)
            .await
            .is_err()
    );
}
