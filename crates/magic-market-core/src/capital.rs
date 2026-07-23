use crate::{
    DataBatch, FiniteNumber, InstrumentId, IsoDate, Money, NonEmptyText, PositiveU32, Price,
    Quantity, Ratio, SourceEvidence, SourcedRecord,
};
use serde::{de, Deserialize, Deserializer, Serialize};

use crate::BoardCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowInterval {
    Minute1,
    Day1,
    Day5,
    Day10,
    Day120,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowScope {
    Instrument(InstrumentId),
    Board {
        code: NonEmptyText,
        name: NonEmptyText,
        category: BoardCategory,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundFlowPoint {
    pub scope: FlowScope,
    pub interval: FlowInterval,
    pub period_at: NonEmptyText,
    pub main_net: Option<Money>,
    pub main_ratio: Option<Ratio>,
    pub super_large_net: Option<Money>,
    pub large_net: Option<Money>,
    pub medium_net: Option<Money>,
    pub small_net: Option<Money>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardFlow {
    pub board_code: NonEmptyText,
    pub board_name: NonEmptyText,
    pub category: BoardCategory,
    pub interval: FlowInterval,
    pub rank: PositiveU32,
    pub return_ratio: Option<Ratio>,
    pub main_net: Option<Money>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarginBalance {
    pub instrument: InstrumentId,
    pub trading_date: IsoDate,
    pub financing_balance: Option<Money>,
    pub financing_buy: Option<Money>,
    pub securities_lending_balance: Option<Money>,
    pub total_balance: Option<Money>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockTrade {
    pub instrument: InstrumentId,
    pub trading_date: IsoDate,
    pub traded_at: Option<NonEmptyText>,
    pub price: Price,
    pub volume: Quantity,
    pub amount: Option<Money>,
    pub buyer: Option<NonEmptyText>,
    pub seller: Option<NonEmptyText>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HolderCount {
    pub instrument: InstrumentId,
    pub report_date: IsoDate,
    pub holders: Quantity,
    pub change_ratio: Option<Ratio>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockupEvent {
    pub instrument: InstrumentId,
    pub listing_date: IsoDate,
    pub share_type: NonEmptyText,
    pub shares: Quantity,
    pub market_value: Option<Money>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DividendPlan {
    pub instrument: InstrumentId,
    pub report_date: IsoDate,
    pub state: NonEmptyText,
    pub cash_per_ten: Option<FiniteNumber>,
    pub bonus_per_ten: Option<FiniteNumber>,
    pub transfer_per_ten: Option<FiniteNumber>,
    pub allotment_per_ten: Option<FiniteNumber>,
    pub reduction_ratio: Option<Ratio>,
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
    FundFlowPoint,
    BoardFlow,
    MarginBalance,
    BlockTrade,
    HolderCount,
    LockupEvent,
    DividendPlan,
);

/// Reusable bounded date-range request for one instrument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstrumentDateRangeRequest {
    instrument: InstrumentId,
    start: Option<IsoDate>,
    end: Option<IsoDate>,
    limit: PositiveU32,
}

impl InstrumentDateRangeRequest {
    pub fn new(instrument: InstrumentId, limit: PositiveU32) -> Result<Self, crate::CoreError> {
        if limit.get() > 10_000 {
            return Err(crate::CoreError::InvalidRequest(
                "date-range limit must be at most 10000".into(),
            ));
        }
        Ok(Self {
            instrument,
            start: None,
            end: None,
            limit,
        })
    }

    pub fn with_range(mut self, start: IsoDate, end: IsoDate) -> Result<Self, crate::CoreError> {
        if start > end {
            return Err(crate::CoreError::InvalidRequest(
                "date-range start must not exceed end".into(),
            ));
        }
        self.start = Some(start);
        self.end = Some(end);
        Ok(self)
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn start(&self) -> Option<&IsoDate> {
        self.start.as_ref()
    }

    pub fn end(&self) -> Option<&IsoDate> {
        self.end.as_ref()
    }

    pub fn limit(&self) -> PositiveU32 {
        self.limit
    }
}

#[derive(Deserialize)]
struct InstrumentDateRangeRequestWire {
    instrument: InstrumentId,
    start: Option<IsoDate>,
    end: Option<IsoDate>,
    limit: PositiveU32,
}

impl<'de> Deserialize<'de> for InstrumentDateRangeRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = InstrumentDateRangeRequestWire::deserialize(deserializer)?;
        let mut request = Self::new(wire.instrument, wire.limit).map_err(de::Error::custom)?;
        match (wire.start, wire.end) {
            (Some(start), Some(end)) => {
                request = request.with_range(start, end).map_err(de::Error::custom)?;
            }
            (None, None) => {}
            _ => return Err(de::Error::custom("date range requires both start and end")),
        }
        Ok(request)
    }
}

/// Request for an instrument or board fund-flow series.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FundFlowRequest {
    scope: FlowScope,
    interval: FlowInterval,
    limit: PositiveU32,
}

impl FundFlowRequest {
    pub fn new(
        scope: FlowScope,
        interval: FlowInterval,
        limit: PositiveU32,
    ) -> Result<Self, crate::CoreError> {
        if limit.get() > 10_000 {
            return Err(crate::CoreError::InvalidRequest(
                "fund-flow limit must be at most 10000".into(),
            ));
        }
        Ok(Self {
            scope,
            interval,
            limit,
        })
    }

    pub fn scope(&self) -> &FlowScope {
        &self.scope
    }

    pub fn interval(&self) -> FlowInterval {
        self.interval
    }

    pub fn limit(&self) -> PositiveU32 {
        self.limit
    }
}

#[derive(Deserialize)]
struct FundFlowRequestWire {
    scope: FlowScope,
    interval: FlowInterval,
    limit: PositiveU32,
}

impl<'de> Deserialize<'de> for FundFlowRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FundFlowRequestWire::deserialize(deserializer)?;
        Self::new(wire.scope, wire.interval, wire.limit).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CapitalCapabilities {
    pub fund_flow_series: bool,
    pub board_flow: bool,
    pub margin: bool,
    pub block_trades: bool,
    pub holder_count: bool,
    pub lockups: bool,
    pub dividends: bool,
}

pub trait FundFlowSeries {
    type Error: std::error::Error + Send + Sync + 'static;
    fn fund_flow_series(
        &self,
        request: &FundFlowRequest,
    ) -> Result<DataBatch<FundFlowPoint>, Self::Error>;
}

pub trait BoardFlows {
    type Error: std::error::Error + Send + Sync + 'static;
    fn board_flows(
        &self,
        category: BoardCategory,
        interval: FlowInterval,
        limit: PositiveU32,
    ) -> Result<DataBatch<BoardFlow>, Self::Error>;
}

pub trait MarginData {
    type Error: std::error::Error + Send + Sync + 'static;
    fn margin_data(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<MarginBalance>, Self::Error>;
}

pub trait BlockTrades {
    type Error: std::error::Error + Send + Sync + 'static;
    fn block_trades(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<BlockTrade>, Self::Error>;
}

pub trait HolderCounts {
    type Error: std::error::Error + Send + Sync + 'static;
    fn holder_counts(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<HolderCount>, Self::Error>;
}

pub trait LockupEvents {
    type Error: std::error::Error + Send + Sync + 'static;
    fn lockup_events(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<LockupEvent>, Self::Error>;
}

pub trait DividendPlans {
    type Error: std::error::Error + Send + Sync + 'static;
    fn dividend_plans(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<DividendPlan>, Self::Error>;
}
