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

fn dated_evidence(provider: ProviderId, batch: &str) -> SourceEvidence {
    evidence(provider, batch)
        .with_source_at("2026-07-23")
        .unwrap()
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
    let entry = DragonTigerEntry::new(
        NonEmptyText::new("entry-1").unwrap(),
        instrument(),
        IsoDate::new("2026-07-23").unwrap(),
        None,
        Some(Money::new(0.0).unwrap()),
        None,
        None,
        None,
        dated_evidence(ProviderId::Eastmoney, "entry"),
    )
    .unwrap();
    let seat = DragonTigerSeat::new(
        NonEmptyText::new("entry-1").unwrap(),
        instrument(),
        IsoDate::new("2026-07-23").unwrap(),
        DragonTigerSide::Buy,
        PositiveU32::new(1).unwrap(),
        NonEmptyText::new("机构专用").unwrap(),
        Money::new(0.0).unwrap(),
        Some(Money::new(0.0).unwrap()),
        None,
        None,
        dated_evidence(ProviderId::Eastmoney, "seat"),
    )
    .unwrap();

    assert_eq!(entry.buy_amount().unwrap().get(), 0.0);
    assert!(entry.sell_amount().is_none());
    assert_eq!(seat.rank().get(), 1);
    assert_eq!(seat.instrument(), &instrument());
    assert_eq!(seat.trading_date().as_str(), "2026-07-23");
    assert_eq!(
        serde_json::from_value::<DragonTigerEntry>(serde_json::to_value(&entry).unwrap()).unwrap(),
        entry
    );
    assert_eq!(
        serde_json::from_value::<DragonTigerSeat>(serde_json::to_value(&seat).unwrap()).unwrap(),
        seat
    );
}

#[test]
fn dragon_tiger_checked_deserialization_rejects_semantic_bypasses() {
    let entry = DragonTigerEntry::new(
        NonEmptyText::new("entry-1").unwrap(),
        instrument(),
        IsoDate::new("2026-07-23").unwrap(),
        None,
        Some(Money::new(100.0).unwrap()),
        Some(Money::new(40.0).unwrap()),
        Some(Money::new(60.0).unwrap()),
        None,
        dated_evidence(ProviderId::Eastmoney, "entry"),
    )
    .unwrap();
    let seat = DragonTigerSeat::new(
        NonEmptyText::new("entry-1").unwrap(),
        instrument(),
        IsoDate::new("2026-07-23").unwrap(),
        DragonTigerSide::Buy,
        PositiveU32::new(1).unwrap(),
        NonEmptyText::new("机构专用").unwrap(),
        Money::new(100.0).unwrap(),
        Some(Money::new(100.0).unwrap()),
        Some(Money::new(40.0).unwrap()),
        Some(Money::new(60.0).unwrap()),
        dated_evidence(ProviderId::Eastmoney, "seat"),
    )
    .unwrap();

    let mut wrong_date = serde_json::to_value(&entry).unwrap();
    wrong_date["evidence"]["source_at"] = serde_json::json!("2026-07-22");
    assert!(serde_json::from_value::<DragonTigerEntry>(wrong_date).is_err());

    let mut inconsistent_net = serde_json::to_value(&entry).unwrap();
    inconsistent_net["net_amount"] = serde_json::json!(61.0);
    assert!(serde_json::from_value::<DragonTigerEntry>(inconsistent_net).is_err());

    let mut rank_six = serde_json::to_value(&seat).unwrap();
    rank_six["rank"] = serde_json::json!(6);
    assert!(serde_json::from_value::<DragonTigerSeat>(rank_six).is_err());

    let mut wrong_side_amount = serde_json::to_value(&seat).unwrap();
    wrong_side_amount["amount"] = serde_json::json!(99.0);
    assert!(serde_json::from_value::<DragonTigerSeat>(wrong_side_amount).is_err());

    let mut large_amount_bypass = serde_json::to_value(&seat).unwrap();
    large_amount_bypass["amount"] = serde_json::json!(1_000_000_000_000_000.0);
    large_amount_bypass["buy_amount"] = serde_json::json!(1_000_000_000_000_100.0);
    large_amount_bypass["sell_amount"] = serde_json::Value::Null;
    large_amount_bypass["net_amount"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<DragonTigerSeat>(large_amount_bypass).is_err());
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
