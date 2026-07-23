use magic_market_core::{
    AssetClass, BlockTrade, BoardCategory, BoardFlow, DividendPlan, Exchange, FiniteNumber,
    FlowInterval, FlowScope, FundFlowPoint, FundFlowRequest, HolderCount,
    InstrumentDateRangeRequest, InstrumentId, IsoDate, LockupEvent, MarginBalance, Money,
    NonEmptyText, PositiveU32, Price, ProviderId, Quantity, Ratio, RatioUnit, SourceEvidence,
    SourcedRecord,
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
            securities_lending_balance: None,
            total_balance: None,
            evidence: evidence(),
        },
        BlockTrade {
            instrument: instrument(),
            trading_date: IsoDate::new("2026-07-23").unwrap(),
            traded_at: None,
            price: Price::new(4.0).unwrap(),
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
            change_ratio: Some(Ratio::new(-2.0, RatioUnit::Percent).unwrap()),
            evidence: evidence(),
        },
        LockupEvent {
            instrument: instrument(),
            listing_date: IsoDate::new("2026-08-01").unwrap(),
            share_type: NonEmptyText::new("首发原股东限售股份").unwrap(),
            shares: Quantity::new(1_000.0).unwrap(),
            market_value: None,
            evidence: evidence(),
        },
        DividendPlan {
            instrument: instrument(),
            report_date: IsoDate::new("2025-12-31").unwrap(),
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
