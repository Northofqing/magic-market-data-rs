use super::*;
use magic_market_core::{
    AssetClass, BarInterval, Exchange, InstrumentId, MinuteDataRequest, TradesRequest,
};
use std::cell::RefCell;
use std::collections::VecDeque;

fn instrument(exchange: Exchange, code: &str) -> InstrumentId {
    InstrumentId::new(exchange, code, AssetClass::Equity).unwrap()
}

#[derive(Default)]
struct ScriptedBlockingServiceQuery {
    count_calls: RefCell<Vec<u8>>,
    count_responses: RefCell<VecDeque<Result<u16, TdxError>>>,
    list_calls: RefCell<Vec<(u8, u16)>>,
    list_responses: RefCell<VecDeque<Result<Vec<SecurityInfo>, TdxError>>>,
}

fn unconfigured<T>(operation: &str) -> Result<T, TdxError> {
    Err(TdxError::InvalidData(format!(
        "scripted {operation} response is not configured"
    )))
}

impl crate::adapter::BlockingTdxQuery for ScriptedBlockingServiceQuery {
    fn security_bars(
        &self,
        _category: u8,
        _market: u8,
        _code: &str,
        _start: u32,
        _count: u16,
        _adjust: u8,
    ) -> Result<Vec<SecurityBar>, TdxError> {
        unconfigured("bars")
    }

    fn security_quotes(
        &self,
        _instruments: &[(u8, &str)],
    ) -> Result<Vec<SecurityQuote>, TdxError> {
        unconfigured("quotes")
    }

    fn minute_time_data(
        &self,
        _market: u8,
        _code: &str,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        unconfigured("current minute")
    }

    fn history_minute_time_data(
        &self,
        _market: u8,
        _code: &str,
        _date: u32,
    ) -> Result<Vec<MinuteTimePrice>, TdxError> {
        unconfigured("history minute")
    }

    fn transaction_data(
        &self,
        _market: u8,
        _code: &str,
        _start: u16,
        _count: u16,
    ) -> Result<Vec<TickData>, TdxError> {
        unconfigured("current transactions")
    }

    fn history_transaction_data(
        &self,
        _market: u8,
        _code: &str,
        _start: u16,
        _count: u16,
        _date: u32,
    ) -> Result<Vec<TickData>, TdxError> {
        unconfigured("history transactions")
    }

    fn security_count(&self, market: u8) -> Result<u16, TdxError> {
        self.count_calls.borrow_mut().push(market);
        self.count_responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| unconfigured("security count"))
    }

    fn security_list(&self, market: u8, start: u16) -> Result<Vec<SecurityInfo>, TdxError> {
        self.list_calls.borrow_mut().push((market, start));
        self.list_responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| unconfigured("security list"))
    }
}

fn security(code: usize) -> SecurityInfo {
    SecurityInfo {
        code: format!("{code:06}"),
        volunit: 100,
        decimal_point: 2,
        name: format!("证券{code}"),
        pre_close: 10.0,
    }
}

#[test]
fn blocking_service_security_list_seam_assembles_declared_count_atomically() {
    let query = ScriptedBlockingServiceQuery {
        count_responses: RefCell::new(VecDeque::from([Ok(2001)])),
        list_responses: RefCell::new(VecDeque::from([
            Ok((0..1000).map(security).collect()),
            Ok((1000..2000).map(security).collect()),
            Ok(vec![security(2000)]),
        ])),
        ..Default::default()
    };

    let records = security_list_all_with(&query, 1).unwrap();

    assert_eq!(records.len(), 2001);
    assert_eq!(*query.count_calls.borrow(), vec![1]);
    assert_eq!(
        *query.list_calls.borrow(),
        vec![(1, 0), (1, 1000), (1, 2000)]
    );
    assert_eq!(records[2000].code, "002000");
}

#[test]
fn service_market_mapping_and_construction_are_explicit() {
    assert_eq!(
        market(&instrument(Exchange::Shanghai, "600396")).unwrap(),
        1
    );
    assert_eq!(
        market(&instrument(Exchange::Shenzhen, "000001")).unwrap(),
        0
    );
    assert!(matches!(
        market(&instrument(Exchange::Beijing, "920118")),
        Err(TdxError::Unsupported(_))
    ));
    assert!(fetched_epoch().unwrap().parse::<u64>().is_ok());

    let blocking = TdxService::default();
    let _ = blocking.client();
    let asynchronous = AsyncTdxService::default();
    let _ = asynchronous.client();
}

#[test]
fn blocking_service_rejects_requests_before_any_transport_call() {
    let service = TdxService::new();
    let ranged = BarsRequest::new(
        instrument(Exchange::Shanghai, "600396"),
        BarInterval::Day,
        1,
    )
    .unwrap()
    .with_range("2026-07-01", "2026-07-23")
    .unwrap();
    assert!(matches!(
        service.bars(&ranged),
        Err(TdxError::Unsupported(_))
    ));
    assert!(service.quotes_chunked(&[]).is_err());

    let beijing = instrument(Exchange::Beijing, "920118");
    assert!(matches!(
        service.quotes_chunked(std::slice::from_ref(&beijing)),
        Err(TdxError::Unsupported(_))
    ));
    assert!(matches!(
        service.security_metadata(std::slice::from_ref(&beijing)),
        Err(TdxError::Unsupported(_))
    ));
}

#[tokio::test]
async fn async_service_maps_disconnected_failures_for_every_facade_family() {
    let service = AsyncTdxService::new();
    let id = instrument(Exchange::Shanghai, "600396");
    let ranged = BarsRequest::new(id.clone(), BarInterval::Day, 1)
        .unwrap()
        .with_range("2026-07-01", "2026-07-23")
        .unwrap();
    assert!(matches!(
        service.bars(&ranged).await,
        Err(TdxError::Unsupported(_))
    ));
    assert!(service.order_books(&[]).await.is_err());

    let trades = TradesRequest::new(id.clone(), 1).unwrap();
    let historical_trades = TradesRequest::new(id.clone(), 1)
        .unwrap()
        .with_date("2026-07-23")
        .unwrap();
    let minute_request = MinuteDataRequest::new(id.clone());
    assert_eq!(minute_request.instrument(), &id);

    let (
        quotes,
        current_trades,
        historical_trade_batch,
        count,
        list,
        all,
        minute,
        history_minute,
        transactions,
        history_transactions,
        finance,
        actions,
    ) = tokio::join!(
        service.quotes(std::slice::from_ref(&id)),
        service.trades(&trades),
        service.trades(&historical_trades),
        service.security_count(1),
        service.security_list(1, 0),
        service.security_list_all(1),
        service.minute_data(1, "600396"),
        service.history_minute_data(1, "600396", 20260723),
        service.transactions(1, "600396", 0, 1),
        service.history_transactions(1, "600396", 0, 1, 20260723),
        service.finance(1, "600396"),
        service.corporate_actions(1, "600396"),
    );
    for failed in [
        quotes.is_err(),
        current_trades.is_err(),
        historical_trade_batch.is_err(),
        count.is_err(),
        list.is_err(),
        all.is_err(),
        minute.is_err(),
        history_minute.is_err(),
        transactions.is_err(),
        history_transactions.is_err(),
        finance.is_err(),
        actions.is_err(),
    ] {
        assert!(failed);
    }
}
