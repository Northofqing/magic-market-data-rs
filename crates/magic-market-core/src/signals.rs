use crate::{
    DataBatch, FiniteNumber, InstrumentId, IsoDate, Money, NonEmptyText, PositiveU32, Price, Ratio,
    SourceEvidence, SourcedRecord,
};
use serde::{de, Deserialize, Deserializer, Serialize};

/// Classification supplied for a board membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoardCategory {
    Industry,
    Concept,
    Region,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardMembership {
    pub instrument: InstrumentId,
    pub board_code: NonEmptyText,
    pub board_name: NonEmptyText,
    pub category: BoardCategory,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrongStockReason {
    pub instrument: InstrumentId,
    pub trading_date: IsoDate,
    pub reason: NonEmptyText,
    pub subjects: Vec<NonEmptyText>,
    pub limit_state: Option<NonEmptyText>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DragonTigerEntry {
    pub entry_id: NonEmptyText,
    pub instrument: InstrumentId,
    pub trading_date: IsoDate,
    pub reason: Option<NonEmptyText>,
    pub buy_amount: Option<Money>,
    pub sell_amount: Option<Money>,
    pub net_amount: Option<Money>,
    pub turnover_rate: Option<Ratio>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DragonTigerSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DragonTigerSeat {
    pub entry_id: NonEmptyText,
    pub side: DragonTigerSide,
    pub rank: PositiveU32,
    pub seat_name: NonEmptyText,
    pub amount: Money,
    pub buy_amount: Option<Money>,
    pub sell_amount: Option<Money>,
    pub net_amount: Option<Money>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketRankingKind {
    Industry,
    Concept,
    Region,
    Popularity,
    Custom(NonEmptyText),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketRankingEntry {
    pub kind: MarketRankingKind,
    pub rank: PositiveU32,
    pub instrument: Option<InstrumentId>,
    pub label: NonEmptyText,
    pub return_ratio: Option<Ratio>,
    pub value: Option<FiniteNumber>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopularityRank {
    pub instrument: InstrumentId,
    pub rank: PositiveU32,
    pub price: Option<Price>,
    pub name: Option<NonEmptyText>,
    pub rank_change: Option<FiniteNumber>,
    pub return_ratio: Option<Ratio>,
    pub heat: Option<FiniteNumber>,
    #[serde(default)]
    pub concepts: Vec<NonEmptyText>,
    pub tag: Option<NonEmptyText>,
    /// Evidence for an optional second-source quote join.
    pub quote_evidence: Option<SourceEvidence>,
    /// Evidence for the ranking response itself.
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConceptHit {
    pub instrument: InstrumentId,
    pub concept: NonEmptyText,
    pub detail: Option<NonEmptyText>,
    pub evidence: SourceEvidence,
}

macro_rules! impl_sourced {
    ($($record:ty),+ $(,)?) => {
        $(
            impl SourcedRecord for $record {
                fn provider_id(&self) -> crate::ProviderId {
                    self.evidence.provider()
                }

                fn evidence_batch_id(&self) -> &str {
                    self.evidence.batch_id()
                }
            }
        )+
    };
}

impl_sourced!(
    BoardMembership,
    StrongStockReason,
    DragonTigerEntry,
    DragonTigerSeat,
    MarketRankingEntry,
    PopularityRank,
    ConceptHit,
);

/// Bounded request for an instrument signal family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstrumentSignalRequest {
    instrument: InstrumentId,
    trading_date: Option<IsoDate>,
    limit: PositiveU32,
}

impl InstrumentSignalRequest {
    pub fn new(instrument: InstrumentId, limit: PositiveU32) -> Result<Self, crate::CoreError> {
        if limit.get() > 10_000 {
            return Err(crate::CoreError::InvalidRequest(
                "signal limit must be at most 10000".into(),
            ));
        }
        Ok(Self {
            instrument,
            trading_date: None,
            limit,
        })
    }

    pub fn with_trading_date(mut self, trading_date: IsoDate) -> Self {
        self.trading_date = Some(trading_date);
        self
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn trading_date(&self) -> Option<&IsoDate> {
        self.trading_date.as_ref()
    }

    pub fn limit(&self) -> PositiveU32 {
        self.limit
    }
}

#[derive(Deserialize)]
struct InstrumentSignalRequestWire {
    instrument: InstrumentId,
    trading_date: Option<IsoDate>,
    limit: PositiveU32,
}

impl<'de> Deserialize<'de> for InstrumentSignalRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = InstrumentSignalRequestWire::deserialize(deserializer)?;
        let mut request = Self::new(wire.instrument, wire.limit).map_err(de::Error::custom)?;
        if let Some(trading_date) = wire.trading_date {
            request = request.with_trading_date(trading_date);
        }
        Ok(request)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SignalCapabilities {
    pub board_memberships: bool,
    pub strong_stock_reasons: bool,
    pub dragon_tiger: bool,
    pub market_rankings: bool,
    pub popularity: bool,
    pub concept_hits: bool,
}

pub trait BoardMembershipProvider {
    type Error: std::error::Error + Send + Sync + 'static;
    fn board_memberships(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<BoardMembership>, Self::Error>;
}

pub trait StrongStockReasons {
    type Error: std::error::Error + Send + Sync + 'static;
    fn strong_stock_reasons(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<StrongStockReason>, Self::Error>;
}

pub trait DragonTigerData {
    type Error: std::error::Error + Send + Sync + 'static;
    fn dragon_tiger_entries(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerEntry>, Self::Error>;
    fn dragon_tiger_seats(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerSeat>, Self::Error>;
}

pub trait MarketRankings {
    type Error: std::error::Error + Send + Sync + 'static;
    fn market_rankings(
        &self,
        kind: &MarketRankingKind,
        limit: PositiveU32,
    ) -> Result<DataBatch<MarketRankingEntry>, Self::Error>;
}

pub trait PopularityData {
    type Error: std::error::Error + Send + Sync + 'static;
    fn popularity(&self, limit: PositiveU32) -> Result<DataBatch<PopularityRank>, Self::Error>;
}

pub trait ConceptHits {
    type Error: std::error::Error + Send + Sync + 'static;
    fn concept_hits(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<ConceptHit>, Self::Error>;
}
