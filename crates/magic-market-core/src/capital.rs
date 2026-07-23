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
    pub super_large_net: Option<Money>,
    pub large_net: Option<Money>,
    pub medium_net: Option<Money>,
    pub small_net: Option<Money>,
    pub leader_instrument: Option<InstrumentId>,
    pub leader_name: Option<NonEmptyText>,
    pub leader_return_ratio: Option<Ratio>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarginBalance {
    pub instrument: InstrumentId,
    pub trading_date: IsoDate,
    pub financing_balance: Option<Money>,
    pub financing_buy: Option<Money>,
    pub financing_repayment: Option<Money>,
    pub securities_lending_balance: Option<Money>,
    pub securities_lending_sell: Option<Quantity>,
    pub securities_lending_repayment: Option<Quantity>,
    pub total_balance: Option<Money>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockTrade {
    pub instrument: InstrumentId,
    pub trading_date: IsoDate,
    pub traded_at: Option<NonEmptyText>,
    pub price: Price,
    pub close_price: Option<Price>,
    pub premium_ratio: Option<Ratio>,
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
    pub holder_change: Option<FiniteNumber>,
    pub change_ratio: Option<Ratio>,
    pub average_shares_per_holder: Option<Quantity>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockupEvent {
    pub instrument: InstrumentId,
    pub listing_date: IsoDate,
    pub share_type: NonEmptyText,
    pub shares: Quantity,
    pub able_shares: Option<Quantity>,
    pub free_float_ratio: Option<Ratio>,
    pub market_value: Option<Money>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DividendPlan {
    pub instrument: InstrumentId,
    pub report_date: IsoDate,
    pub ex_dividend_date: Option<IsoDate>,
    pub state: NonEmptyText,
    pub cash_per_ten: Option<FiniteNumber>,
    pub bonus_per_ten: Option<FiniteNumber>,
    pub transfer_per_ten: Option<FiniteNumber>,
    pub allotment_per_ten: Option<FiniteNumber>,
    pub reduction_ratio: Option<Ratio>,
    pub evidence: SourceEvidence,
}

/// One source-ranked post-close main-fund-flow record.
///
/// Board and price-limit metadata remain optional because they must come from a
/// source record rather than code-based inference.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PostCloseFlow {
    instrument: InstrumentId,
    name: Option<NonEmptyText>,
    trading_date: IsoDate,
    rank: PositiveU32,
    close: Price,
    change: Ratio,
    main_net: Money,
    board: Option<crate::Board>,
    price_limit_rule: Option<crate::PriceLimitRule>,
    evidence: SourceEvidence,
}

impl PostCloseFlow {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instrument: InstrumentId,
        name: Option<NonEmptyText>,
        trading_date: IsoDate,
        rank: PositiveU32,
        close: Price,
        change: Ratio,
        main_net: Money,
        board: Option<crate::Board>,
        price_limit_rule: Option<crate::PriceLimitRule>,
        evidence: SourceEvidence,
    ) -> Result<Self, crate::CoreError> {
        let source_at = evidence.source_at().ok_or_else(|| {
            crate::CoreError::InvalidRequest(
                "post-close flow evidence must include source_at".into(),
            )
        })?;
        let source_date_text = source_at.get(..10).ok_or_else(|| {
            crate::CoreError::InvalidRequest(
                "post-close flow source_at must start with YYYY-MM-DD".into(),
            )
        })?;
        if !matches!(source_at.as_bytes().get(10), None | Some(b' ') | Some(b'T')) {
            return Err(crate::CoreError::InvalidRequest(
                "post-close flow source_at date must end or be followed by a time separator".into(),
            ));
        }
        let source_date = IsoDate::new(source_date_text)?;
        if source_date != trading_date {
            return Err(crate::CoreError::InvalidRequest(format!(
                "post-close flow source date {} does not match trading date {}",
                source_date.as_str(),
                trading_date.as_str()
            )));
        }
        Ok(Self {
            instrument,
            name,
            trading_date,
            rank,
            close,
            change,
            main_net,
            board,
            price_limit_rule,
            evidence,
        })
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn name(&self) -> Option<&NonEmptyText> {
        self.name.as_ref()
    }

    pub fn trading_date(&self) -> &IsoDate {
        &self.trading_date
    }

    pub fn rank(&self) -> PositiveU32 {
        self.rank
    }

    pub fn close(&self) -> Price {
        self.close
    }

    pub fn change(&self) -> Ratio {
        self.change
    }

    pub fn main_net(&self) -> Money {
        self.main_net
    }

    pub fn board(&self) -> Option<crate::Board> {
        self.board
    }

    pub fn price_limit_rule(&self) -> Option<&crate::PriceLimitRule> {
        self.price_limit_rule.as_ref()
    }

    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }
}

#[derive(Deserialize)]
struct PostCloseFlowWire {
    instrument: InstrumentId,
    name: Option<NonEmptyText>,
    trading_date: IsoDate,
    rank: PositiveU32,
    close: Price,
    change: Ratio,
    main_net: Money,
    board: Option<crate::Board>,
    price_limit_rule: Option<crate::PriceLimitRule>,
    evidence: SourceEvidence,
}

impl<'de> Deserialize<'de> for PostCloseFlow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = PostCloseFlowWire::deserialize(deserializer)?;
        Self::new(
            wire.instrument,
            wire.name,
            wire.trading_date,
            wire.rank,
            wire.close,
            wire.change,
            wire.main_net,
            wire.board,
            wire.price_limit_rule,
            wire.evidence,
        )
        .map_err(de::Error::custom)
    }
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
    PostCloseFlow,
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

/// Bounded request for a source-ranked post-close flow snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PostCloseFlowRequest {
    trading_date: IsoDate,
    limit: PositiveU32,
}

impl PostCloseFlowRequest {
    pub fn new(trading_date: IsoDate, limit: PositiveU32) -> Result<Self, crate::CoreError> {
        if limit.get() > 100 {
            return Err(crate::CoreError::InvalidRequest(
                "post-close flow limit must be at most 100".into(),
            ));
        }
        Ok(Self {
            trading_date,
            limit,
        })
    }

    pub fn trading_date(&self) -> &IsoDate {
        &self.trading_date
    }

    pub fn limit(&self) -> PositiveU32 {
        self.limit
    }
}

#[derive(Deserialize)]
struct PostCloseFlowRequestWire {
    trading_date: IsoDate,
    limit: PositiveU32,
}

impl<'de> Deserialize<'de> for PostCloseFlowRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = PostCloseFlowRequestWire::deserialize(deserializer)?;
        Self::new(wire.trading_date, wire.limit).map_err(de::Error::custom)
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
    #[serde(default)]
    pub post_close_flow: bool,
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

pub trait PostCloseFlows {
    type Error: std::error::Error + Send + Sync + 'static;
    fn post_close_flows(
        &self,
        request: &PostCloseFlowRequest,
    ) -> Result<DataBatch<PostCloseFlow>, Self::Error>;
}
