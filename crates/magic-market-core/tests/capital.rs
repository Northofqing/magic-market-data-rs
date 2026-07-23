use magic_market_core::{
    AssetClass, BlockTrade, Board, BoardCategory, BoardFlow, CapitalCapabilities, DividendPlan,
    Exchange, FiniteNumber, FlowInterval, FlowScope, FundFlowPoint, FundFlowRequest, HolderCount,
    InstrumentDateRangeRequest, InstrumentId, IsoDate, LockupEvent, MarginBalance, Money,
    NonEmptyText, PositiveU32, PostCloseFlow, PostCloseFlowRequest, Price, PriceLimitRule,
    ProviderId, Quantity, Ratio, RatioUnit, SourceEvidence, SourcedRecord,
};

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn evidence() -> SourceEvidence {
    SourceEvidence::new(ProviderId::Eastmoney, "observed", "batch").unwrap()
}

#[test]
fn flow_series_distinguishes_missing_from_source_zero() {
    let point = FundFlowPoint {
        scope: FlowScope::Instrument(instrument()),
        interval: FlowInterval::Day1,
        period_at: NonEmptyText::new("2026-07-23").unwrap(),
        main_net: Some(Money::new(0.0).unwrap()),
        main_ratio: None,
        super_large_net: None,
        large_net: None,
        medium_net: None,
        small_net: None,
        evidence: evidence(),
    };
    let board = BoardFlow {
        board_code: NonEmptyText::new("BK0001").unwrap(),
        board_name: NonEmptyText::new("电力").unwrap(),
        category: BoardCategory::Industry,
        interval: FlowInterval::Day5,
        rank: PositiveU32::new(1).unwrap(),
        return_ratio: None,
        main_net: None,
        super_large_net: None,
        large_net: None,
        medium_net: None,
        small_net: None,
        leader_instrument: None,
        leader_name: None,
        leader_return_ratio: None,
        evidence: evidence(),
    };

    assert_eq!(point.main_net.unwrap().get(), 0.0);
    assert!(point.super_large_net.is_none());
    assert_eq!(board.provider_id(), ProviderId::Eastmoney);
}

#[test]
fn capital_records_round_trip_without_defaulting_absence() {
    let records = (
        MarginBalance {
            instrument: instrument(),
            trading_date: IsoDate::new("2026-07-23").unwrap(),
            financing_balance: None,
            financing_buy: Some(Money::new(0.0).unwrap()),
            financing_repayment: None,
            securities_lending_balance: None,
            securities_lending_sell: None,
            securities_lending_repayment: None,
            total_balance: None,
            evidence: evidence(),
        },
        BlockTrade {
            instrument: instrument(),
            trading_date: IsoDate::new("2026-07-23").unwrap(),
            traded_at: None,
            price: Price::new(4.0).unwrap(),
            close_price: None,
            premium_ratio: None,
            volume: Quantity::new(100.0).unwrap(),
            amount: Some(Money::new(400.0).unwrap()),
            buyer: None,
            seller: None,
            evidence: evidence(),
        },
        HolderCount {
            instrument: instrument(),
            report_date: IsoDate::new("2026-06-30").unwrap(),
            holders: Quantity::new(10_000.0).unwrap(),
            holder_change: None,
            change_ratio: Some(Ratio::new(-2.0, RatioUnit::Percent).unwrap()),
            average_shares_per_holder: None,
            evidence: evidence(),
        },
        LockupEvent {
            instrument: instrument(),
            listing_date: IsoDate::new("2026-08-01").unwrap(),
            share_type: NonEmptyText::new("首发原股东限售股份").unwrap(),
            shares: Quantity::new(1_000.0).unwrap(),
            able_shares: None,
            free_float_ratio: None,
            market_value: None,
            evidence: evidence(),
        },
        DividendPlan {
            instrument: instrument(),
            report_date: IsoDate::new("2025-12-31").unwrap(),
            ex_dividend_date: None,
            state: NonEmptyText::new("实施").unwrap(),
            cash_per_ten: Some(FiniteNumber::new(1.0).unwrap()),
            bonus_per_ten: None,
            transfer_per_ten: None,
            allotment_per_ten: None,
            reduction_ratio: None,
            evidence: evidence(),
        },
    );

    let json = serde_json::to_string(&records).unwrap();
    let decoded: (
        MarginBalance,
        BlockTrade,
        HolderCount,
        LockupEvent,
        DividendPlan,
    ) = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, records);
}

#[test]
fn range_requests_are_bounded_and_calendar_checked() {
    let request = InstrumentDateRangeRequest::new(instrument(), PositiveU32::new(100).unwrap())
        .unwrap()
        .with_range(
            IsoDate::new("2026-01-01").unwrap(),
            IsoDate::new("2026-07-23").unwrap(),
        )
        .unwrap();
    assert_eq!(request.limit().get(), 100);
    assert!(
        InstrumentDateRangeRequest::new(instrument(), PositiveU32::new(10_001).unwrap()).is_err()
    );
    assert!(
        InstrumentDateRangeRequest::new(instrument(), PositiveU32::new(1).unwrap())
            .unwrap()
            .with_range(
                IsoDate::new("2026-07-23").unwrap(),
                IsoDate::new("2026-01-01").unwrap()
            )
            .is_err()
    );
    assert!(FundFlowRequest::new(
        FlowScope::Instrument(instrument()),
        FlowInterval::Day120,
        PositiveU32::new(10_001).unwrap()
    )
    .is_err());
    assert!(serde_json::from_str::<FundFlowRequest>(
        r#"{"scope":{"Instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}},"interval":"Day120","limit":10001}"#
    )
    .is_err());
}

#[test]
fn capital_requests_round_trip_only_complete_validated_filters() {
    let current =
        InstrumentDateRangeRequest::new(instrument(), PositiveU32::new(100).unwrap()).unwrap();
    let decoded: InstrumentDateRangeRequest =
        serde_json::from_value(serde_json::to_value(&current).unwrap()).unwrap();
    assert_eq!(decoded.instrument(), &instrument());
    assert!(decoded.start().is_none());
    assert!(decoded.end().is_none());

    let ranged = current
        .with_range(
            IsoDate::new("2026-01-01").unwrap(),
            IsoDate::new("2026-07-23").unwrap(),
        )
        .unwrap();
    let decoded: InstrumentDateRangeRequest =
        serde_json::from_value(serde_json::to_value(&ranged).unwrap()).unwrap();
    assert_eq!(decoded.start().unwrap().as_str(), "2026-01-01");
    assert_eq!(decoded.end().unwrap().as_str(), "2026-07-23");
    assert_eq!(decoded.limit().get(), 100);

    let partial = serde_json::json!({
        "instrument": {
            "exchange": "Shanghai",
            "code": "600396",
            "asset_class": "Equity"
        },
        "start": "2026-01-01",
        "end": null,
        "limit": 100
    });
    assert!(serde_json::from_value::<InstrumentDateRangeRequest>(partial).is_err());

    let flow = FundFlowRequest::new(
        FlowScope::Instrument(instrument()),
        FlowInterval::Day5,
        PositiveU32::new(20).unwrap(),
    )
    .unwrap();
    let decoded: FundFlowRequest =
        serde_json::from_value(serde_json::to_value(&flow).unwrap()).unwrap();
    assert_eq!(decoded.scope(), &FlowScope::Instrument(instrument()));
    assert_eq!(decoded.interval(), FlowInterval::Day5);
    assert_eq!(decoded.limit().get(), 20);

    let post_close = PostCloseFlowRequest::new(
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_value::<PostCloseFlowRequest>(serde_json::to_value(&post_close).unwrap())
            .unwrap(),
        post_close
    );
}

#[test]
fn post_close_flow_preserves_rank_and_source_backed_limit_metadata() {
    let source_evidence = evidence().with_source_at("2026-07-23 15:35:00").unwrap();
    let record = PostCloseFlow::new(
        instrument(),
        Some(NonEmptyText::new("华电辽能").unwrap()),
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(1).unwrap(),
        Price::new(4.36).unwrap(),
        Ratio::new(2.1, RatioUnit::Percent).unwrap(),
        Money::new(12_000_000.0).unwrap(),
        Some(Board::Main),
        Some(
            PriceLimitRule::new(
                Some(Ratio::new(10.0, RatioUnit::Percent).unwrap()),
                Some("source-rule".into()),
            )
            .unwrap(),
        ),
        source_evidence,
    )
    .unwrap();
    let request = PostCloseFlowRequest::new(
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    assert_eq!(request.limit().get(), 10);
    assert_eq!(request.trading_date().as_str(), "2026-07-23");
    assert_eq!(record.instrument(), &instrument());
    assert_eq!(record.name().unwrap().as_str(), "华电辽能");
    assert_eq!(record.trading_date().as_str(), "2026-07-23");
    assert_eq!(record.rank().get(), 1);
    assert_eq!(record.close().get(), 4.36);
    assert_eq!(record.change().get(), 2.1);
    assert_eq!(record.main_net().get(), 12_000_000.0);
    assert_eq!(record.board(), Some(Board::Main));
    assert_eq!(
        record.price_limit_rule().unwrap().version(),
        Some("source-rule")
    );
    assert_eq!(record.evidence().provider(), ProviderId::Eastmoney);
    assert_eq!(
        serde_json::from_str::<PostCloseFlow>(&serde_json::to_string(&record).unwrap()).unwrap(),
        record
    );
    let mut missing_source_time = serde_json::to_value(&record).unwrap();
    missing_source_time["evidence"]["source_at"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<PostCloseFlow>(missing_source_time).is_err());
    assert!(PostCloseFlow::new(
        instrument(),
        None,
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(1).unwrap(),
        Price::new(4.36).unwrap(),
        Ratio::new(2.1, RatioUnit::Percent).unwrap(),
        Money::new(12_000_000.0).unwrap(),
        None,
        None,
        evidence(),
    )
    .is_err());
    assert!(PostCloseFlow::new(
        instrument(),
        None,
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(1).unwrap(),
        Price::new(4.36).unwrap(),
        Ratio::new(2.1, RatioUnit::Percent).unwrap(),
        Money::new(12_000_000.0).unwrap(),
        None,
        None,
        evidence().with_source_at("short").unwrap(),
    )
    .is_err());
    assert!(PostCloseFlow::new(
        instrument(),
        None,
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(1).unwrap(),
        Price::new(4.36).unwrap(),
        Ratio::new(2.1, RatioUnit::Percent).unwrap(),
        Money::new(12_000_000.0).unwrap(),
        None,
        None,
        evidence().with_source_at("2026-07-23x15:35:00").unwrap(),
    )
    .is_err());
    assert!(PostCloseFlow::new(
        instrument(),
        None,
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(1).unwrap(),
        Price::new(4.36).unwrap(),
        Ratio::new(2.1, RatioUnit::Percent).unwrap(),
        Money::new(12_000_000.0).unwrap(),
        None,
        None,
        evidence().with_source_at("2026-07-22 15:35:00").unwrap(),
    )
    .is_err());
    assert!(PostCloseFlow::new(
        instrument(),
        None,
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(1).unwrap(),
        Price::new(4.36).unwrap(),
        Ratio::new(2.1, RatioUnit::Percent).unwrap(),
        Money::new(12_000_000.0).unwrap(),
        None,
        None,
        evidence().with_source_at("not-a-date").unwrap(),
    )
    .is_err());
    assert!(PostCloseFlowRequest::new(
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(101).unwrap()
    )
    .is_err());
    let legacy_capabilities: CapitalCapabilities = serde_json::from_str(
        r#"{"fund_flow_series":true,"board_flow":true,"margin":true,"block_trades":true,"holder_count":true,"lockups":true,"dividends":true}"#,
    )
    .unwrap();
    assert!(!legacy_capabilities.post_close_flow);
}
