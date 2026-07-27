use magic_market_core::{
    AssetClass, AuctionSnapshot, Auctions, DataBatch, DataStatus, Exchange, FiniteNumber,
    InstrumentId, IsoDate, MarketRankingEntry, MarketRankingKind, MarketRankingUnit,
    MarketRankings, MarketSession, NonEmptyText, PositiveU32, Provenance, ProviderId,
    SourceEvidence,
};
use magic_market_router::{
    auction_source, market_ranking_source, AcceptancePolicy, AttemptStatus, AuctionRouter,
    FailureKind, MarketRankingRouter, SourceError,
};
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
#[error("fixture family failure")]
struct FixtureError;

#[derive(Clone)]
struct AuctionProvider(Result<DataBatch<AuctionSnapshot>, FixtureError>);

impl Auctions for AuctionProvider {
    type Error = FixtureError;

    fn auction_snapshots(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<AuctionSnapshot>, Self::Error> {
        self.0.clone()
    }
}

#[derive(Clone)]
struct RankingProvider(Result<DataBatch<MarketRankingEntry>, FixtureError>);

impl MarketRankings for RankingProvider {
    type Error = FixtureError;

    fn market_rankings(
        &self,
        _kind: &MarketRankingKind,
        _limit: PositiveU32,
    ) -> Result<DataBatch<MarketRankingEntry>, Self::Error> {
        self.0.clone()
    }
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap()
}

fn provenance(batch_id: &str) -> Provenance {
    Provenance::new("fixture", "2026-07-27T10:00:00+08:00")
        .unwrap()
        .with_source_at("2026-07-27T09:30:00+08:00")
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap()
}

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Transport, "fixture transport failure")
}

#[test]
fn auction_router_fails_over_to_an_evidence_matching_snapshot() {
    let batch_id = "auction-batch";
    let record = AuctionSnapshot::new(
        instrument(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        DataStatus::Unavailable,
        Some("2026-07-27T09:30:00+08:00".into()),
        "2026-07-27T10:00:00+08:00",
        ProviderId::Tencent,
        batch_id,
    )
    .unwrap();
    let mut router = AuctionRouter::new(AcceptancePolicy::new());
    router
        .register(auction_source(
            ProviderId::Tdx,
            Arc::new(AuctionProvider(Err(FixtureError))),
            classify,
        ))
        .unwrap()
        .register(auction_source(
            ProviderId::Tencent,
            Arc::new(AuctionProvider(Ok(DataBatch::strict(
                vec![record],
                provenance(batch_id),
            )))),
            classify,
        ))
        .unwrap();

    let outcome = router.route(&[instrument()]).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Tencent);
    assert_eq!(outcome.batch().records()[0].instrument(), &instrument());
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Transport,
            ..
        }
    ));
    assert_eq!(outcome.attempts()[1].status(), &AttemptStatus::Selected);
}

#[test]
fn market_ranking_router_fails_over_and_keeps_kind_rank_and_name() {
    let batch_id = "ranking-batch";
    let kind = MarketRankingKind::VolumeRatio;
    let one = PositiveU32::new(1).unwrap();
    let record = MarketRankingEntry::new(
        kind.clone(),
        one,
        Some(instrument()),
        NonEmptyText::new("贵州茅台").unwrap(),
        FiniteNumber::new(2.5).unwrap(),
        MarketRankingUnit::Multiple,
        IsoDate::new("2026-07-27").unwrap(),
        MarketSession::Continuous,
        NonEmptyText::new("A股全市场").unwrap(),
        one,
        one,
        0,
        SourceEvidence::new(ProviderId::Eastmoney, "2026-07-27T10:00:00+08:00", batch_id)
            .unwrap()
            .with_source_at("2026-07-27T09:30:00+08:00")
            .unwrap(),
    )
    .unwrap();
    let mut router = MarketRankingRouter::new(AcceptancePolicy::new());
    router
        .register(market_ranking_source(
            ProviderId::Tdx,
            Arc::new(RankingProvider(Err(FixtureError))),
            classify,
        ))
        .unwrap()
        .register(market_ranking_source(
            ProviderId::Eastmoney,
            Arc::new(RankingProvider(Ok(DataBatch::strict(
                vec![record],
                provenance(batch_id),
            )))),
            classify,
        ))
        .unwrap();

    let outcome = router.route(&(kind.clone(), one)).unwrap();
    let selected = &outcome.batch().records()[0];
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert_eq!(selected.kind(), &kind);
    assert_eq!(selected.rank(), one);
    assert_eq!(selected.label().as_str(), "贵州茅台");
    assert_eq!(outcome.attempts().len(), 2);
}
