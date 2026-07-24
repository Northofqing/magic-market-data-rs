use crate::{
    DataBatch, FiniteNumber, InstrumentId, IsoDate, Money, NonEmptyText, PositiveU32, Price, Ratio,
    RatioUnit, SourceEvidence, SourcedRecord,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DragonTigerEntry {
    entry_id: NonEmptyText,
    instrument: InstrumentId,
    trading_date: IsoDate,
    reason: Option<NonEmptyText>,
    buy_amount: Option<Money>,
    sell_amount: Option<Money>,
    net_amount: Option<Money>,
    turnover_rate: Option<Ratio>,
    evidence: SourceEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DragonTigerSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DragonTigerSeat {
    entry_id: NonEmptyText,
    instrument: InstrumentId,
    trading_date: IsoDate,
    side: DragonTigerSide,
    rank: PositiveU32,
    seat_name: NonEmptyText,
    amount: Money,
    buy_amount: Option<Money>,
    sell_amount: Option<Money>,
    net_amount: Option<Money>,
    evidence: SourceEvidence,
}

impl DragonTigerEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        entry_id: NonEmptyText,
        instrument: InstrumentId,
        trading_date: IsoDate,
        reason: Option<NonEmptyText>,
        buy_amount: Option<Money>,
        sell_amount: Option<Money>,
        net_amount: Option<Money>,
        turnover_rate: Option<Ratio>,
        evidence: SourceEvidence,
    ) -> Result<Self, crate::CoreError> {
        validate_dragon_tiger_evidence(&trading_date, &evidence)?;
        validate_non_negative_money("dragon-tiger buy amount", buy_amount)?;
        validate_non_negative_money("dragon-tiger sell amount", sell_amount)?;
        validate_net_amount(buy_amount, sell_amount, net_amount)?;
        if turnover_rate.is_some_and(|ratio| {
            ratio.unit() != RatioUnit::Percent || ratio.get().is_sign_negative()
        }) {
            return Err(crate::CoreError::InvalidRequest(
                "dragon-tiger turnover rate must be a non-negative percentage".into(),
            ));
        }
        Ok(Self {
            entry_id,
            instrument,
            trading_date,
            reason,
            buy_amount,
            sell_amount,
            net_amount,
            turnover_rate,
            evidence,
        })
    }

    pub fn entry_id(&self) -> &NonEmptyText {
        &self.entry_id
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn trading_date(&self) -> &IsoDate {
        &self.trading_date
    }

    pub fn reason(&self) -> Option<&NonEmptyText> {
        self.reason.as_ref()
    }

    pub fn buy_amount(&self) -> Option<Money> {
        self.buy_amount
    }

    pub fn sell_amount(&self) -> Option<Money> {
        self.sell_amount
    }

    pub fn net_amount(&self) -> Option<Money> {
        self.net_amount
    }

    pub fn turnover_rate(&self) -> Option<Ratio> {
        self.turnover_rate
    }

    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }
}

impl DragonTigerSeat {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        entry_id: NonEmptyText,
        instrument: InstrumentId,
        trading_date: IsoDate,
        side: DragonTigerSide,
        rank: PositiveU32,
        seat_name: NonEmptyText,
        amount: Money,
        buy_amount: Option<Money>,
        sell_amount: Option<Money>,
        net_amount: Option<Money>,
        evidence: SourceEvidence,
    ) -> Result<Self, crate::CoreError> {
        validate_dragon_tiger_evidence(&trading_date, &evidence)?;
        if rank.get() > 5 {
            return Err(crate::CoreError::InvalidRequest(
                "dragon-tiger seat rank must be between 1 and 5".into(),
            ));
        }
        validate_non_negative_money("dragon-tiger seat amount", Some(amount))?;
        validate_non_negative_money("dragon-tiger seat buy amount", buy_amount)?;
        validate_non_negative_money("dragon-tiger seat sell amount", sell_amount)?;
        validate_net_amount(buy_amount, sell_amount, net_amount)?;
        let side_amount = match side {
            DragonTigerSide::Buy => buy_amount,
            DragonTigerSide::Sell => sell_amount,
        };
        if !side_amount.is_some_and(|value| money_values_match(value, amount)) {
            return Err(crate::CoreError::InvalidRequest(
                "dragon-tiger seat amount must match its side amount".into(),
            ));
        }
        Ok(Self {
            entry_id,
            instrument,
            trading_date,
            side,
            rank,
            seat_name,
            amount,
            buy_amount,
            sell_amount,
            net_amount,
            evidence,
        })
    }

    pub fn entry_id(&self) -> &NonEmptyText {
        &self.entry_id
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn trading_date(&self) -> &IsoDate {
        &self.trading_date
    }

    pub fn side(&self) -> DragonTigerSide {
        self.side
    }

    pub fn rank(&self) -> PositiveU32 {
        self.rank
    }

    pub fn seat_name(&self) -> &NonEmptyText {
        &self.seat_name
    }

    pub fn amount(&self) -> Money {
        self.amount
    }

    pub fn buy_amount(&self) -> Option<Money> {
        self.buy_amount
    }

    pub fn sell_amount(&self) -> Option<Money> {
        self.sell_amount
    }

    pub fn net_amount(&self) -> Option<Money> {
        self.net_amount
    }

    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }
}

#[derive(Deserialize)]
struct DragonTigerEntryWire {
    entry_id: NonEmptyText,
    instrument: InstrumentId,
    trading_date: IsoDate,
    reason: Option<NonEmptyText>,
    buy_amount: Option<Money>,
    sell_amount: Option<Money>,
    net_amount: Option<Money>,
    turnover_rate: Option<Ratio>,
    evidence: SourceEvidence,
}

impl<'de> Deserialize<'de> for DragonTigerEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DragonTigerEntryWire::deserialize(deserializer)?;
        Self::new(
            wire.entry_id,
            wire.instrument,
            wire.trading_date,
            wire.reason,
            wire.buy_amount,
            wire.sell_amount,
            wire.net_amount,
            wire.turnover_rate,
            wire.evidence,
        )
        .map_err(de::Error::custom)
    }
}

#[derive(Deserialize)]
struct DragonTigerSeatWire {
    entry_id: NonEmptyText,
    instrument: InstrumentId,
    trading_date: IsoDate,
    side: DragonTigerSide,
    rank: PositiveU32,
    seat_name: NonEmptyText,
    amount: Money,
    buy_amount: Option<Money>,
    sell_amount: Option<Money>,
    net_amount: Option<Money>,
    evidence: SourceEvidence,
}

impl<'de> Deserialize<'de> for DragonTigerSeat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DragonTigerSeatWire::deserialize(deserializer)?;
        Self::new(
            wire.entry_id,
            wire.instrument,
            wire.trading_date,
            wire.side,
            wire.rank,
            wire.seat_name,
            wire.amount,
            wire.buy_amount,
            wire.sell_amount,
            wire.net_amount,
            wire.evidence,
        )
        .map_err(de::Error::custom)
    }
}

fn validate_dragon_tiger_evidence(
    trading_date: &IsoDate,
    evidence: &SourceEvidence,
) -> Result<(), crate::CoreError> {
    let source_at = evidence.source_at().ok_or_else(|| {
        crate::CoreError::InvalidRequest(
            "dragon-tiger record evidence must include source_at".into(),
        )
    })?;
    let remainder = source_at
        .strip_prefix(trading_date.as_str())
        .ok_or_else(|| {
            crate::CoreError::InvalidRequest(
                "dragon-tiger record evidence date must match trading date".into(),
            )
        })?;
    if !remainder.is_empty() && !remainder.starts_with('T') && !remainder.starts_with(' ') {
        return Err(crate::CoreError::InvalidRequest(
            "dragon-tiger record evidence date must match trading date".into(),
        ));
    }
    Ok(())
}

fn validate_non_negative_money(name: &str, value: Option<Money>) -> Result<(), crate::CoreError> {
    if value.is_some_and(|money| money.get().is_sign_negative()) {
        return Err(crate::CoreError::InvalidRequest(format!(
            "{name} must be non-negative"
        )));
    }
    Ok(())
}

fn validate_net_amount(
    buy_amount: Option<Money>,
    sell_amount: Option<Money>,
    net_amount: Option<Money>,
) -> Result<(), crate::CoreError> {
    if let (Some(buy), Some(sell), Some(net)) = (buy_amount, sell_amount, net_amount) {
        let expected = Money::new(buy.get() - sell.get())?;
        if !money_values_match(net, expected) {
            return Err(crate::CoreError::InvalidRequest(
                "dragon-tiger net amount must equal buy amount minus sell amount".into(),
            ));
        }
    }
    Ok(())
}

fn money_values_match(left: Money, right: Money) -> bool {
    (left.get() - right.get()).abs() <= 0.01
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
