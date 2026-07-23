use magic_market_core::{
    AssetClass, BoardCategory, BoardMembership, ConceptHit, DragonTigerEntry, DragonTigerSeat,
    DragonTigerSide, Exchange, InstrumentId, IsoDate, Money, NonEmptyText, PopularityRank,
    PositiveU32, ProviderId, SourceEvidence, SourcedRecord, StrongStockReason,
};

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn evidence(provider: ProviderId, batch: &str) -> SourceEvidence {
    SourceEvidence::new(provider, "observed", batch).unwrap()
}

#[test]
fn board_and_reason_contracts_preserve_unknown_categories() {
    let board = BoardMembership {
        instrument: instrument(),
        board_code: NonEmptyText::new("BK0001").unwrap(),
        board_name: NonEmptyText::new("混合板块").unwrap(),
        category: BoardCategory::Unknown,
        evidence: evidence(ProviderId::Eastmoney, "board"),
    };
    let reason = StrongStockReason {
        instrument: instrument(),
        trading_date: IsoDate::new("2026-07-23").unwrap(),
        reason: NonEmptyText::new("电力板块走强").unwrap(),
        subjects: vec![NonEmptyText::new("电力").unwrap()],
        limit_state: None,
        evidence: evidence(ProviderId::Tonghuashun, "reason"),
    };

    assert_eq!(board.category, BoardCategory::Unknown);
    assert_eq!(reason.provider_id(), ProviderId::Tonghuashun);
    assert_eq!(
        serde_json::from_str::<BoardMembership>(&serde_json::to_string(&board).unwrap()).unwrap(),
        board
    );
}

#[test]
fn dragon_tiger_missing_amounts_remain_absent() {
    let entry = DragonTigerEntry {
        entry_id: NonEmptyText::new("entry-1").unwrap(),
        instrument: instrument(),
        trading_date: IsoDate::new("2026-07-23").unwrap(),
        reason: None,
        buy_amount: Some(Money::new(0.0).unwrap()),
        sell_amount: None,
        net_amount: None,
        turnover_rate: None,
        evidence: evidence(ProviderId::Eastmoney, "entry"),
    };
    let seat = DragonTigerSeat {
        entry_id: NonEmptyText::new("entry-1").unwrap(),
        side: DragonTigerSide::Buy,
        rank: PositiveU32::new(1).unwrap(),
        seat_name: NonEmptyText::new("机构专用").unwrap(),
        amount: Money::new(0.0).unwrap(),
        buy_amount: Some(Money::new(0.0).unwrap()),
        sell_amount: None,
        net_amount: None,
        evidence: evidence(ProviderId::Eastmoney, "seat"),
    };

    assert_eq!(entry.buy_amount.unwrap().get(), 0.0);
    assert!(entry.sell_amount.is_none());
    assert_eq!(seat.rank.get(), 1);
}

#[test]
fn popularity_join_retains_both_evidence_records() {
    let rank = PopularityRank {
        instrument: instrument(),
        rank: PositiveU32::new(3).unwrap(),
        price: None,
        name: None,
        rank_change: None,
        return_ratio: None,
        heat: None,
        concepts: vec![],
        tag: None,
        quote_evidence: Some(evidence(ProviderId::Tencent, "quote")),
        evidence: evidence(ProviderId::Eastmoney, "rank"),
    };
    let hit = ConceptHit {
        instrument: instrument(),
        concept: NonEmptyText::new("绿色电力").unwrap(),
        detail: None,
        evidence: evidence(ProviderId::Eastmoney, "concept"),
    };

    assert_eq!(rank.provider_id(), ProviderId::Eastmoney);
    assert_eq!(
        rank.quote_evidence.as_ref().unwrap().provider(),
        ProviderId::Tencent
    );
    let legacy_json = serde_json::to_string(&rank)
        .unwrap()
        .replace(",\"concepts\":[]", "");
    let restored: PopularityRank = serde_json::from_str(&legacy_json).unwrap();
    assert!(restored.concepts.is_empty());
    assert_eq!(hit.evidence_batch_id(), "concept");
}
