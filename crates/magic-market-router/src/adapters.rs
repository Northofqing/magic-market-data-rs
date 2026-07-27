use crate::{
    AcceptancePolicy, FailoverChain, FailureKind, RouteAttempt, RouteOutcome, RoutedSource,
    RouterError, SourceError, SourceFn,
};
use magic_market_core::{
    Announcement, Announcements, AuctionSnapshot, Auctions, Bar, BarsRequest, BlockTrade,
    BlockTrades, BoardCategory, BoardFlow, BoardFlows, BoardMembership, BoardMembershipProvider,
    ConceptHit, ConceptHits, ConsensusData, ConsensusSnapshot, ContractMonth, CorporateAction,
    CorporateActionRequest, CorporateActions, DataBatch, DividendPlan, DividendPlans,
    DragonTigerData, DragonTigerDisclosure, DragonTigerEntry, DragonTigerSeat, EvidenceTimestamp,
    Exchange, FinancialStatement, FinancialStatements, FlowInterval, FundFlowPoint,
    FundFlowRequest, FundFlowSeries, HistoricalBars, HolderCount, HolderCounts,
    InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest, InvestorQuestion,
    InvestorQuestions, IsoDate, LimitPoolEntry, LimitPoolRequest, LimitPools, LockupEvent,
    LockupEvents, MarginBalance, MarginData, MarketDragonTigerData, MarketDragonTigerRequest,
    MarketRankingEntry, MarketRankingKind, MarketRankings, MarketStatistics,
    MarketStatisticsProvider, MinuteData, MinuteDataRequest, MinutePoint, MoneyFlow, MoneyFlows,
    NewsItem, NewsProvider, NonEmptyText, NorthboundDailyRequest, NorthboundDailyStat,
    NorthboundDailyStatistics, OptionContract, OptionData, OptionGreeks, OptionQuote, OrderBook,
    OrderBooks, PopularityData, PopularityRank, PositiveU32, PostCloseFlow, PostCloseFlowRequest,
    PostCloseFlows, ProviderId, Quote, RealtimeQuotes, ResearchReport, ResearchReports,
    ResearchRequest, SecurityMetadata, SecurityMetadataProvider, SecurityProfile, SecurityProfiles,
    SemanticSearch, SemanticSearchDocument, SemanticSearchRequest, StatementKind,
    StrongStockReason, StrongStockReasons, TargetPriceConsensus, TargetPriceData,
    TargetPriceRequest, TechnicalBar, TechnicalBarsProvider, Trade, Trades, TradesRequest,
};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::Arc;

pub type QuoteRouter = FailoverChain<[InstrumentId], Quote>;
pub type BarsRouter = FailoverChain<BarsRequest, Bar>;
pub type MinuteRouter = FailoverChain<MinuteDataRequest, MinutePoint>;
pub type TradesRouter = FailoverChain<TradesRequest, Trade>;
pub type MoneyFlowRouter = FailoverChain<[InstrumentId], MoneyFlow>;
pub type OrderBookRouter = FailoverChain<[InstrumentId], OrderBook>;
pub type AuctionRouter = FailoverChain<[InstrumentId], AuctionSnapshot>;
pub type SecurityMetadataRouter = FailoverChain<[InstrumentId], SecurityMetadata>;
pub type MarketStatisticsRouter = FailoverChain<[InstrumentId], MarketStatistics>;
pub type TechnicalBarsRouter = FailoverChain<BarsRequest, TechnicalBar>;
pub type ResearchRouter = FailoverChain<ResearchRequest, ResearchReport>;
pub type ConsensusRouter = FailoverChain<[InstrumentId], ConsensusSnapshot>;
pub type TargetPriceRouter = FailoverChain<TargetPriceRequest, TargetPriceConsensus>;
pub type SemanticSearchRouter = FailoverChain<SemanticSearchRequest, SemanticSearchDocument>;
pub type BoardMembershipRouter = FailoverChain<[InstrumentId], BoardMembership>;
pub type StrongStockReasonRouter = FailoverChain<InstrumentSignalRequest, StrongStockReason>;
pub type DragonTigerEntryRouter = FailoverChain<InstrumentSignalRequest, DragonTigerEntry>;
pub type DragonTigerSeatRouter = FailoverChain<InstrumentSignalRequest, DragonTigerSeat>;
pub type MarketDragonTigerRouter = FailoverChain<MarketDragonTigerRequest, DragonTigerDisclosure>;
pub type MarketRankingRequest = (MarketRankingKind, PositiveU32);
pub type MarketRankingRouter = FailoverChain<MarketRankingRequest, MarketRankingEntry>;
pub type PopularityRouter = FailoverChain<PositiveU32, PopularityRank>;
pub type ConceptHitRouter = FailoverChain<[InstrumentId], ConceptHit>;
pub type FundFlowSeriesRouter = FailoverChain<FundFlowRequest, FundFlowPoint>;
pub type BoardFlowRequest = (BoardCategory, FlowInterval, PositiveU32);
pub type BoardFlowRouter = FailoverChain<BoardFlowRequest, BoardFlow>;
pub type MarginRouter = FailoverChain<InstrumentDateRangeRequest, MarginBalance>;
pub type BlockTradeRouter = FailoverChain<InstrumentDateRangeRequest, BlockTrade>;
pub type HolderCountRouter = FailoverChain<InstrumentDateRangeRequest, HolderCount>;
pub type LockupRouter = FailoverChain<InstrumentDateRangeRequest, LockupEvent>;
pub type DividendRouter = FailoverChain<InstrumentDateRangeRequest, DividendPlan>;
pub type PostCloseFlowRouter = FailoverChain<PostCloseFlowRequest, PostCloseFlow>;
pub type NorthboundDailyRouter = FailoverChain<NorthboundDailyRequest, NorthboundDailyStat>;
pub type InstrumentNewsRouter = FailoverChain<InstrumentDateRangeRequest, NewsItem>;
pub type GlobalNewsRouter = FailoverChain<PositiveU32, NewsItem>;
pub type AnnouncementRouter = FailoverChain<InstrumentDateRangeRequest, Announcement>;
pub type InvestorQuestionRouter = FailoverChain<InstrumentDateRangeRequest, InvestorQuestion>;
pub type SecurityProfileRouter = FailoverChain<[InstrumentId], SecurityProfile>;
pub type FinancialStatementRequest = (Vec<InstrumentId>, StatementKind);
pub type FinancialStatementRouter = FailoverChain<FinancialStatementRequest, FinancialStatement>;
pub type LimitPoolRouter = FailoverChain<LimitPoolRequest, LimitPoolEntry>;
pub type OptionContractsRequest = (InstrumentId, Option<ContractMonth>);
pub type OptionContractRouter = FailoverChain<OptionContractsRequest, OptionContract>;
pub type OptionQuoteRouter = FailoverChain<[NonEmptyText], OptionQuote>;
pub type OptionGreeksRouter = FailoverChain<[NonEmptyText], OptionGreeks>;

/// One corporate-action request carrying the router's single admission date.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CorporateActionRouteRequest {
    request: CorporateActionRequest,
    admission_as_of: IsoDate,
}

impl CorporateActionRouteRequest {
    fn request(&self) -> &CorporateActionRequest {
        &self.request
    }

    fn admission_as_of(&self) -> &IsoDate {
        &self.admission_as_of
    }
}

/// Sealed, response-validating source accepted by [`CorporateActionRouter`].
///
/// Construct this only through [`corporate_action_source`]. The private route
/// request prevents a generic [`SourceFn`] from bypassing the router's single
/// admission date or exact response-coverage checks.
pub struct CorporateActionSource {
    source: SourceFn<CorporateActionRouteRequest, CorporateAction>,
}

impl std::fmt::Debug for CorporateActionSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CorporateActionSource")
            .field("provider_id", &self.source.provider_id())
            .finish()
    }
}

impl RoutedSource<CorporateActionRouteRequest, CorporateAction> for CorporateActionSource {
    fn provider_id(&self) -> ProviderId {
        self.source.provider_id()
    }

    fn fetch(
        &self,
        request: &CorporateActionRouteRequest,
    ) -> Result<DataBatch<CorporateAction>, SourceError> {
        self.source.fetch(request)
    }
}

/// Corporate-action failover with one immutable admission date for every source.
#[derive(Debug)]
pub struct CorporateActionRouter {
    admission_as_of: IsoDate,
    chain: FailoverChain<CorporateActionRouteRequest, CorporateAction>,
}

impl CorporateActionRouter {
    pub fn new(policy: AcceptancePolicy, admission_as_of: IsoDate) -> Self {
        Self {
            admission_as_of,
            chain: FailoverChain::new(policy),
        }
    }

    pub fn policy(&self) -> AcceptancePolicy {
        self.chain.policy()
    }

    pub fn admission_as_of(&self) -> &IsoDate {
        &self.admission_as_of
    }

    pub fn register(&mut self, source: CorporateActionSource) -> Result<&mut Self, RouterError> {
        self.chain.register(source)?;
        Ok(self)
    }

    pub fn provider_ids(&self) -> Vec<ProviderId> {
        self.chain.provider_ids()
    }

    pub fn route(
        &self,
        request: &CorporateActionRequest,
    ) -> Result<CorporateActionRouteOutcome, RouterError> {
        let routed_request = CorporateActionRouteRequest {
            request: request.clone(),
            admission_as_of: self.admission_as_of.clone(),
        };
        self.chain
            .route(&routed_request)
            .map(|outcome| CorporateActionRouteOutcome {
                admission_as_of: self.admission_as_of.clone(),
                outcome,
            })
    }
}

/// Accepted corporate-action batch retaining the immutable admission boundary.
#[derive(Debug)]
pub struct CorporateActionRouteOutcome {
    admission_as_of: IsoDate,
    outcome: RouteOutcome<CorporateAction>,
}

impl CorporateActionRouteOutcome {
    pub fn admission_as_of(&self) -> &IsoDate {
        &self.admission_as_of
    }

    pub fn selected_provider(&self) -> ProviderId {
        self.outcome.selected_provider()
    }

    pub fn batch(&self) -> &DataBatch<CorporateAction> {
        self.outcome.batch()
    }

    pub fn attempts(&self) -> &[RouteAttempt] {
        self.outcome.attempts()
    }

    pub fn into_batch(self) -> DataBatch<CorporateAction> {
        self.outcome.into_batch()
    }

    pub fn into_parts(self) -> (IsoDate, DataBatch<CorporateAction>, Vec<RouteAttempt>) {
        let (batch, attempts) = self.outcome.into_parts();
        (self.admission_as_of, batch, attempts)
    }
}

pub fn quote_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], Quote>
where
    Provider: RealtimeQuotes<Quote = Quote> + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.realtime_quotes(request).map_err(&classify)
    })
}

pub fn bars_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<BarsRequest, Bar>
where
    Provider: HistoricalBars<Bar = Bar> + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.historical_bars(request).map_err(&classify)
    })
}

pub fn minute_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<MinuteDataRequest, MinutePoint>
where
    Provider: MinuteData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.minute_data(request).map_err(&classify)
    })
}

pub fn trades_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<TradesRequest, Trade>
where
    Provider: Trades + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.trades(request).map_err(&classify)
    })
}

pub fn money_flow_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], MoneyFlow>
where
    Provider: MoneyFlows + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.money_flows(request).map_err(&classify)
    })
}

pub fn order_book_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], OrderBook>
where
    Provider: OrderBooks + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.order_books(request).map_err(&classify)
    })
}

pub fn auction_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], AuctionSnapshot>
where
    Provider: Auctions + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.auction_snapshots(request).map_err(&classify)
    })
}

pub fn security_metadata_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], SecurityMetadata>
where
    Provider: SecurityMetadataProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &[InstrumentId]| {
        if request.is_empty() {
            return Err(SourceError::stop(
                FailureKind::InvalidRequest,
                "security metadata requires at least one instrument",
            ));
        }
        let requested = request.iter().cloned().collect::<HashSet<_>>();
        if requested.len() != request.len() {
            return Err(SourceError::stop(
                FailureKind::InvalidRequest,
                "security metadata request contains duplicate instruments",
            ));
        }
        let batch = provider.security_metadata(request).map_err(&classify)?;
        if batch.records().len() != request.len() {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "security metadata response cardinality does not match request",
            ));
        }
        let mut returned = HashSet::with_capacity(batch.records().len());
        for record in batch.records() {
            if !requested.contains(record.instrument()) {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "security metadata record instrument was not requested",
                ));
            }
            if !returned.insert(record.instrument().clone()) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "security metadata response contains a duplicate instrument",
                ));
            }
        }
        Ok(batch)
    })
}

pub fn corporate_action_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> CorporateActionSource
where
    Provider: CorporateActions + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    CorporateActionSource {
        source: SourceFn::new(
            provider_id,
            move |routed_request: &CorporateActionRouteRequest| {
                let request = routed_request.request();
                let admission_as_of = routed_request.admission_as_of();
                if request.start().is_some_and(|start| start > admission_as_of)
                    || request.end().is_some_and(|end| end > admission_as_of)
                {
                    return Err(SourceError::stop(
                        FailureKind::InvalidRequest,
                        "corporate-action request range extends beyond router admission_as_of",
                    ));
                }
                let response = provider.corporate_actions(request).map_err(&classify)?;
                if response.evidence().provider() != provider_id {
                    return Err(SourceError::try_next(
                        FailureKind::Evidence,
                        "corporate-action response provider does not match registered provider",
                    ));
                }
                if response.admission_as_of() != admission_as_of {
                    return Err(SourceError::try_next(
                        FailureKind::Evidence,
                        "corporate-action response admission_as_of does not match router policy",
                    ));
                }
                let response_observed = EvidenceTimestamp::parse_instant(
                    response.evidence().observed_at(),
                )
                .map_err(|_| {
                    SourceError::try_next(
                        FailureKind::Evidence,
                        "corporate-action response observation timestamp is malformed",
                    )
                })?;
                let response_source = response
                    .evidence()
                    .source_at()
                    .map(EvidenceTimestamp::parse)
                    .transpose()
                    .map_err(|_| {
                        SourceError::try_next(
                            FailureKind::Evidence,
                            "corporate-action response source timestamp is malformed",
                        )
                    })?;
                if response_source.is_some_and(|source_time| {
                    response_observed.duration_since(source_time).is_none()
                }) {
                    return Err(SourceError::try_next(
                FailureKind::Evidence,
                "corporate-action response source timestamp is later than observation timestamp",
            ));
                }
                if response.coverage() != request {
                    return Err(SourceError::try_next(
                        FailureKind::Evidence,
                        "corporate-action response coverage does not match request",
                    ));
                }
                let batch = response.into_batch();
                if !batch.quality().is_complete() {
                    return Err(SourceError::try_next(
                        FailureKind::Quality,
                        format!(
                            "corporate-action batch is incomplete: {}",
                            batch.quality().issues().join("; ")
                        ),
                    ));
                }
                let provenance = batch.provenance();
                let batch_id = provenance.batch_id().ok_or_else(|| {
                    SourceError::try_next(
                        FailureKind::Evidence,
                        "corporate-action batch provenance has no batch ID",
                    )
                })?;
                let observed_time = EvidenceTimestamp::parse_instant(provenance.fetched_at())
                    .map_err(|_| {
                        SourceError::try_next(
                            FailureKind::Evidence,
                            "corporate-action batch observation timestamp is malformed",
                        )
                    })?;
                let batch_source_time = provenance
                    .source_at()
                    .map(EvidenceTimestamp::parse)
                    .transpose()
                    .map_err(|_| {
                        SourceError::try_next(
                            FailureKind::Evidence,
                            "corporate-action batch source timestamp is malformed",
                        )
                    })?;
                if batch_source_time
                    .is_some_and(|source_time| observed_time.duration_since(source_time).is_none())
                {
                    return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "corporate-action batch source timestamp is later than observation timestamp",
                ));
                }
                let mut identities = HashSet::with_capacity(batch.records().len());
                let mut previous = None;
                for record in batch.records() {
                    if record.status() != magic_market_core::CorporateActionStatus::Implemented {
                        return Err(SourceError::try_next(
                            FailureKind::Quality,
                            "corporate-action batch contains a non-implemented action",
                        ));
                    }
                    if record.evidence().provider() != provider_id {
                        return Err(SourceError::try_next(
                            FailureKind::Evidence,
                            "corporate-action record provider does not match registered provider",
                        ));
                    }
                    if record.evidence().batch_id() != batch_id {
                        return Err(SourceError::try_next(
                            FailureKind::Evidence,
                            "corporate-action record batch ID does not match batch provenance",
                        ));
                    }
                    if record.evidence().observed_at() != provenance.fetched_at() {
                        return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "corporate-action record observation timestamp does not match batch provenance",
                ));
                    }
                    if record.evidence().source_at() != provenance.source_at() {
                        return Err(SourceError::try_next(
                        FailureKind::Evidence,
                        "corporate-action record source timestamp does not match batch provenance",
                    ));
                    }
                    let record_observed = EvidenceTimestamp::parse_instant(
                        record.evidence().observed_at(),
                    )
                    .map_err(|_| {
                        SourceError::try_next(
                            FailureKind::Evidence,
                            "corporate-action record observation timestamp is malformed",
                        )
                    })?;
                    let record_source = record
                        .evidence()
                        .source_at()
                        .map(EvidenceTimestamp::parse)
                        .transpose()
                        .map_err(|_| {
                            SourceError::try_next(
                                FailureKind::Evidence,
                                "corporate-action record source timestamp is malformed",
                            )
                        })?;
                    if record_source.is_some_and(|source_time| {
                        record_observed.duration_since(source_time).is_none()
                    }) {
                        return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "corporate-action record source timestamp is later than observation timestamp",
                ));
                    }
                    if record.instrument() != request.instrument() {
                        return Err(SourceError::try_next(
                            FailureKind::Evidence,
                            "corporate-action record instrument does not match request",
                        ));
                    }
                    if record.effective_on() > admission_as_of {
                        return Err(SourceError::try_next(
                            FailureKind::Evidence,
                            "corporate-action effective date is later than router admission_as_of",
                        ));
                    }
                    if request
                        .start()
                        .is_some_and(|start| record.effective_on() < start)
                        || request.end().is_some_and(|end| record.effective_on() > end)
                    {
                        return Err(SourceError::try_next(
                            FailureKind::Evidence,
                            "corporate-action effective date is outside the requested range",
                        ));
                    }
                    let identity = (record.effective_on().clone(), record.category());
                    if !identities.insert(identity.clone()) {
                        return Err(SourceError::try_next(
                            FailureKind::Quality,
                            "corporate-action batch contains a duplicate date/category identity",
                        ));
                    }
                    if previous
                        .as_ref()
                        .is_some_and(|previous| previous > &identity)
                    {
                        return Err(SourceError::try_next(
                            FailureKind::Quality,
                            "corporate-action batch is not ordered by effective date and category",
                        ));
                    }
                    previous = Some(identity);
                }
                Ok(batch)
            },
        ),
    }
}

pub fn market_statistics_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], MarketStatistics>
where
    Provider: MarketStatisticsProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.market_statistics(request).map_err(&classify)
    })
}

pub fn technical_bars_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<BarsRequest, TechnicalBar>
where
    Provider: TechnicalBarsProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.technical_bars(request).map_err(&classify)
    })
}

pub fn research_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<ResearchRequest, ResearchReport>
where
    Provider: ResearchReports + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.research_reports(request).map_err(&classify)
    })
}

pub fn consensus_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], ConsensusSnapshot>
where
    Provider: ConsensusData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.consensus(request).map_err(&classify)
    })
}

pub fn target_price_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<TargetPriceRequest, TargetPriceConsensus>
where
    Provider: TargetPriceData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &TargetPriceRequest| {
        if request.from() > request.through() {
            return Err(SourceError::stop(
                FailureKind::InvalidRequest,
                "target-price request start exceeds end",
            ));
        }
        let batch = provider
            .target_price_consensus(request)
            .map_err(&classify)?;
        validate_target_price_batch(provider_id, request, &batch)?;
        Ok(batch)
    })
}

fn validate_target_price_batch(
    provider_id: ProviderId,
    request: &TargetPriceRequest,
    batch: &DataBatch<TargetPriceConsensus>,
) -> Result<(), SourceError> {
    if !batch.quality().is_complete() {
        return Err(SourceError::try_next(
            FailureKind::Quality,
            format!(
                "target-price batch is incomplete: {}",
                batch.quality().issues().join("; ")
            ),
        ));
    }
    let [consensus] = batch.records() else {
        return Err(SourceError::try_next(
            FailureKind::Quality,
            format!(
                "target-price provider must return exactly one consensus record, got {}",
                batch.records().len()
            ),
        ));
    };
    if consensus.instrument() != request.instrument() {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            "target-price consensus instrument does not match request",
        ));
    }
    if consensus.requested_from() != request.from()
        || consensus.requested_through() != request.through()
    {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            "target-price consensus requested range does not match request",
        ));
    }
    if consensus.observation_start() < request.from()
        || consensus.observation_end() > request.through()
        || consensus.observation_start() > consensus.observation_end()
    {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            "target-price observation range is outside the requested range",
        ));
    }

    let provenance = batch.provenance();
    let batch_id = provenance.batch_id().ok_or_else(|| {
        SourceError::try_next(
            FailureKind::Evidence,
            "target-price batch provenance has no batch ID",
        )
    })?;
    if consensus.evidence().provider() != provider_id {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            "target-price consensus provider does not match registered provider",
        ));
    }
    if consensus.evidence().batch_id() != batch_id {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            "target-price consensus batch ID does not match batch provenance",
        ));
    }
    if consensus.evidence().observed_at() != provenance.fetched_at() {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            "target-price consensus observation timestamp does not match batch provenance",
        ));
    }
    if consensus.evidence().source_at() != provenance.source_at() {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            "target-price consensus source timestamp does not match batch provenance",
        ));
    }

    let observed_at = EvidenceTimestamp::parse_instant(provenance.fetched_at()).map_err(|_| {
        SourceError::try_next(
            FailureKind::Evidence,
            "target-price batch observation timestamp is malformed",
        )
    })?;
    let source_at = provenance
        .source_at()
        .ok_or_else(|| {
            SourceError::try_next(
                FailureKind::Evidence,
                "target-price batch source timestamp is absent",
            )
        })
        .and_then(|value| {
            EvidenceTimestamp::parse(value).map_err(|_| {
                SourceError::try_next(
                    FailureKind::Evidence,
                    "target-price batch source timestamp is malformed",
                )
            })
        })?;
    if observed_at.duration_since(source_at).is_none() {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            "target-price batch source timestamp is later than its observation timestamp",
        ));
    }

    if consensus.observations().iter().any(|observation| {
        observation.evidence().provider() != provider_id
            || observation.evidence().batch_id() != batch_id
            || observation.evidence().observed_at() != provenance.fetched_at()
    }) {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            "target-price observation evidence does not match batch provenance",
        ));
    }

    // The Core constructor is the single source of truth for observation
    // identity, range, canonical date/report ordering, deduplication, counts,
    // input evidence and derived aggregate values. Rebuild at this trust
    // boundary so the Router does not maintain a second, drifting rule set.
    let rebuilt = TargetPriceConsensus::new(
        request,
        consensus.observations().to_vec(),
        consensus.evidence().clone(),
    )
    .map_err(|error| {
        SourceError::try_next(
            FailureKind::Quality,
            format!("target-price consensus violates the Core contract: {error}"),
        )
    })?;
    if &rebuilt != consensus {
        return Err(SourceError::try_next(
            FailureKind::Quality,
            "target-price consensus derived fields contradict its observations",
        ));
    }
    Ok(())
}

pub fn semantic_search_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<SemanticSearchRequest, SemanticSearchDocument>
where
    Provider: SemanticSearch + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.semantic_search(request).map_err(&classify)
    })
}

pub fn board_membership_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], BoardMembership>
where
    Provider: BoardMembershipProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &[InstrumentId]| {
        let requested: HashSet<_> = request.iter().cloned().collect();
        let batch = provider.board_memberships(request).map_err(&classify)?;
        let mut identities = HashSet::with_capacity(batch.records().len());
        for record in batch.records() {
            if !requested.contains(&record.instrument) {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "board-membership instrument was not requested",
                ));
            }
            if !identities.insert((record.instrument.clone(), record.board_code.as_str())) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "board-membership batch contains duplicate identities",
                ));
            }
        }
        Ok(batch)
    })
}

pub fn strong_stock_reason_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<InstrumentSignalRequest, StrongStockReason>
where
    Provider: StrongStockReasons + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.strong_stock_reasons(request).map_err(&classify)
    })
}

pub fn dragon_tiger_entry_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<InstrumentSignalRequest, DragonTigerEntry>
where
    Provider: DragonTigerData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.dragon_tiger_entries(request).map_err(&classify)?;
        if batch.records().len() > request.limit().get() as usize {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "dragon-tiger entry batch exceeds requested limit",
            ));
        }
        validate_signal_batch_date(
            batch.provenance().source_at(),
            request.trading_date(),
            "dragon-tiger entry batch",
        )?;
        let mut entry_ids = HashSet::with_capacity(batch.records().len());
        for record in batch.records() {
            if record.instrument() != request.instrument() {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "dragon-tiger entry instrument does not match requested instrument",
                ));
            }
            if request
                .trading_date()
                .is_some_and(|date| record.trading_date() != date)
            {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "dragon-tiger entry date does not match requested date",
                ));
            }
            validate_signal_batch_date(
                record.evidence().source_at(),
                request.trading_date(),
                "dragon-tiger entry record",
            )?;
            if !entry_ids.insert(record.entry_id().as_str()) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "dragon-tiger entry batch contains a duplicate entry ID",
                ));
            }
        }
        Ok(batch)
    })
}

pub fn dragon_tiger_seat_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<InstrumentSignalRequest, DragonTigerSeat>
where
    Provider: DragonTigerData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.dragon_tiger_seats(request).map_err(&classify)?;
        if batch.records().len() > request.limit().get() as usize {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "dragon-tiger seat batch exceeds requested limit",
            ));
        }
        validate_signal_batch_date(
            batch.provenance().source_at(),
            request.trading_date(),
            "dragon-tiger seat batch",
        )?;
        let mut identities = HashSet::with_capacity(batch.records().len());
        let mut groups = std::collections::HashMap::<&str, ([bool; 5], [bool; 5])>::new();
        for record in batch.records() {
            if record.instrument() != request.instrument() {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "dragon-tiger seat instrument does not match requested instrument",
                ));
            }
            if request
                .trading_date()
                .is_some_and(|date| record.trading_date() != date)
            {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "dragon-tiger seat date does not match requested date",
                ));
            }
            validate_signal_batch_date(
                record.evidence().source_at(),
                request.trading_date(),
                "dragon-tiger seat record",
            )?;
            let rank = record.rank().get() as usize;
            if !(1..=5).contains(&rank) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "dragon-tiger seat rank must be between 1 and 5",
                ));
            }
            let side = match record.side() {
                magic_market_core::DragonTigerSide::Buy => 0_u8,
                magic_market_core::DragonTigerSide::Sell => 1_u8,
            };
            if !identities.insert((record.entry_id().as_str(), side, rank)) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "dragon-tiger seat batch contains a duplicate entry/side/rank",
                ));
            }
            let group = groups
                .entry(record.entry_id().as_str())
                .or_insert(([false; 5], [false; 5]));
            match record.side() {
                magic_market_core::DragonTigerSide::Buy => group.0[rank - 1] = true,
                magic_market_core::DragonTigerSide::Sell => group.1[rank - 1] = true,
            }
        }
        if groups.is_empty()
            || groups.values().any(|(buy, sell)| {
                !buy.iter().all(|present| *present) || !sell.iter().all(|present| *present)
            })
        {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "dragon-tiger seat batch must contain complete buy-five and sell-five groups",
            ));
        }
        Ok(batch)
    })
}

pub fn market_dragon_tiger_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<MarketDragonTigerRequest, DragonTigerDisclosure>
where
    Provider: MarketDragonTigerData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.market_dragon_tiger(request).map_err(&classify)?;
        if batch.records().is_empty() {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "market dragon-tiger batch is empty without verified-empty evidence",
            ));
        }
        if batch.records().len() > request.limit().get() as usize {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "market dragon-tiger batch exceeds requested limit",
            ));
        }
        validate_signal_batch_date(
            batch.provenance().source_at(),
            Some(request.trading_date()),
            "market dragon-tiger batch",
        )?;

        let mut entry_ids = HashSet::with_capacity(batch.records().len());
        for disclosure in batch.records() {
            let entry = disclosure.entry();
            if entry.trading_date() != request.trading_date() {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "market dragon-tiger entry date does not match requested date",
                ));
            }
            validate_signal_batch_date(
                entry.evidence().source_at(),
                Some(request.trading_date()),
                "market dragon-tiger entry",
            )?;
            if !entry_ids.insert(entry.entry_id().as_str()) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "market dragon-tiger batch contains a duplicate entry ID",
                ));
            }
        }
        if batch
            .records()
            .windows(2)
            .any(|pair| market_dragon_tiger_order(&pair[0], &pair[1]) == Ordering::Greater)
        {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "market dragon-tiger batch is not in canonical order",
            ));
        }
        Ok(batch)
    })
}

fn market_dragon_tiger_order(
    left: &DragonTigerDisclosure,
    right: &DragonTigerDisclosure,
) -> Ordering {
    let left = left.entry();
    let right = right.entry();
    match (left.net_amount(), right.net_amount()) {
        (Some(left), Some(right)) => right.get().total_cmp(&left.get()),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| {
        market_exchange_order(left.instrument().exchange())
            .cmp(&market_exchange_order(right.instrument().exchange()))
    })
    .then_with(|| left.instrument().code().cmp(right.instrument().code()))
    .then_with(|| left.entry_id().as_str().cmp(right.entry_id().as_str()))
}

const fn market_exchange_order(exchange: Exchange) -> u8 {
    match exchange {
        Exchange::Shanghai => 0,
        Exchange::Shenzhen => 1,
        Exchange::Beijing => 2,
    }
}

fn validate_signal_batch_date(
    source_at: Option<&str>,
    requested_date: Option<&IsoDate>,
    context: &str,
) -> Result<(), SourceError> {
    let Some(requested_date) = requested_date else {
        return Ok(());
    };
    let source_at = source_at.ok_or_else(|| {
        SourceError::try_next(
            FailureKind::Evidence,
            format!("{context} evidence is missing source_at"),
        )
    })?;
    let requested = requested_date.as_str();
    if source_at.get(..10) != Some(requested)
        || !matches!(source_at.as_bytes().get(10), None | Some(b' ') | Some(b'T'))
    {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            format!("{context} source date does not match requested date {requested}"),
        ));
    }
    Ok(())
}

pub fn market_ranking_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<MarketRankingRequest, MarketRankingEntry>
where
    Provider: MarketRankings + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &MarketRankingRequest| {
        provider
            .market_rankings(&request.0, request.1)
            .map_err(&classify)
    })
}

pub fn popularity_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<PositiveU32, PopularityRank>
where
    Provider: PopularityData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.popularity(*request).map_err(&classify)
    })
}

pub fn concept_hit_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], ConceptHit>
where
    Provider: ConceptHits + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.concept_hits(request).map_err(&classify)
    })
}

pub fn fund_flow_series_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<FundFlowRequest, FundFlowPoint>
where
    Provider: FundFlowSeries + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.fund_flow_series(request).map_err(&classify)
    })
}

pub fn board_flow_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<BoardFlowRequest, BoardFlow>
where
    Provider: BoardFlows + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &BoardFlowRequest| {
        provider
            .board_flows(request.0, request.1, request.2)
            .map_err(&classify)
    })
}

macro_rules! date_range_source {
    ($function:ident, $trait:ident, $method:ident, $record:ty) => {
        pub fn $function<Provider, Classify>(
            provider_id: ProviderId,
            provider: Arc<Provider>,
            classify: Classify,
        ) -> SourceFn<InstrumentDateRangeRequest, $record>
        where
            Provider: $trait + Send + Sync + 'static,
            Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
        {
            SourceFn::new(provider_id, move |request| {
                provider.$method(request).map_err(&classify)
            })
        }
    };
}

date_range_source!(margin_source, MarginData, margin_data, MarginBalance);
date_range_source!(block_trade_source, BlockTrades, block_trades, BlockTrade);
date_range_source!(
    holder_count_source,
    HolderCounts,
    holder_counts,
    HolderCount
);
date_range_source!(lockup_source, LockupEvents, lockup_events, LockupEvent);
date_range_source!(dividend_source, DividendPlans, dividend_plans, DividendPlan);

pub fn post_close_flow_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<PostCloseFlowRequest, PostCloseFlow>
where
    Provider: PostCloseFlows + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.post_close_flows(request).map_err(&classify)?;
        let requested_date = request.trading_date().as_str();
        let batch_source_at = batch.provenance().source_at().ok_or_else(|| {
            SourceError::try_next(
                FailureKind::Evidence,
                "post-close flow batch provenance is missing source_at",
            )
        })?;
        if batch_source_at.get(..10) != Some(requested_date)
            || !matches!(
                batch_source_at.as_bytes().get(10),
                None | Some(b' ') | Some(b'T')
            )
        {
            return Err(SourceError::try_next(
                FailureKind::Evidence,
                format!(
                    "post-close flow batch source date does not match requested date {requested_date}"
                ),
            ));
        }
        if batch.records().len() != request.limit().get() as usize {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "post-close flow batch cardinality does not match requested limit",
            ));
        }
        let mut ranks = HashSet::with_capacity(batch.records().len());
        let mut instruments = HashSet::with_capacity(batch.records().len());
        let mut previous_main_net = None;
        for (index, record) in batch.records().iter().enumerate() {
            if record.trading_date() != request.trading_date() {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "post-close flow record date does not match requested date",
                ));
            }
            if !ranks.insert(record.rank().get()) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "post-close flow batch contains a duplicate rank",
                ));
            }
            if record.rank().get() != index as u32 + 1 {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "post-close flow ranks are not contiguous source order",
                ));
            }
            if !instruments.insert(record.instrument().clone()) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "post-close flow batch contains a duplicate instrument",
                ));
            }
            if record.name().is_none() {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "post-close flow record is missing its stock name",
                ));
            }
            if previous_main_net.is_some_and(|value| value < record.main_net().get()) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "post-close flow main net values are not descending",
                ));
            }
            previous_main_net = Some(record.main_net().get());
        }
        Ok(batch)
    })
}

pub fn northbound_daily_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<NorthboundDailyRequest, NorthboundDailyStat>
where
    Provider: NorthboundDailyStatistics + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider
            .northbound_daily_statistics(request)
            .map_err(&classify)?;
        let requested_date = request.trading_date().as_str();
        let batch_source_at = batch.provenance().source_at().ok_or_else(|| {
            SourceError::try_next(
                FailureKind::Evidence,
                "northbound daily batch provenance is missing source_at",
            )
        })?;
        if batch_source_at.get(..10) != Some(requested_date)
            || !matches!(
                batch_source_at.as_bytes().get(10),
                None | Some(b' ') | Some(b'T')
            )
        {
            return Err(SourceError::try_next(
                FailureKind::Evidence,
                format!(
                    "northbound daily batch source date does not match requested date {requested_date}"
                ),
            ));
        }
        if batch.records().len() != 1 {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "northbound daily response must contain exactly one requested channel",
            ));
        }
        let record = &batch.records()[0];
        if record.trading_date() != request.trading_date() || record.channel() != request.channel()
        {
            return Err(SourceError::try_next(
                FailureKind::Evidence,
                "northbound daily record does not match requested date and channel",
            ));
        }
        Ok(batch)
    })
}

date_range_source!(
    instrument_news_source,
    NewsProvider,
    instrument_news,
    NewsItem
);
pub fn announcement_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<InstrumentDateRangeRequest, Announcement>
where
    Provider: Announcements + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.announcements(request).map_err(&classify)?;
        if batch.records().len() > request.limit().get() as usize {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "announcement batch exceeds requested limit",
            ));
        }
        let mut ids = HashSet::with_capacity(batch.records().len());
        for record in batch.records() {
            if &record.instrument != request.instrument() {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "announcement record instrument does not match requested instrument",
                ));
            }
            if !ids.insert(record.announcement_id.as_str()) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "announcement batch contains a duplicate announcement ID",
                ));
            }
            let published_date = announcement_date(record.published_at.as_str(), "published_at")?;
            if request.start().is_some_and(|start| published_date < *start)
                || request.end().is_some_and(|end| published_date > *end)
            {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "announcement publication date is outside the requested range",
                ));
            }
            let source_at = record.evidence.source_at().ok_or_else(|| {
                SourceError::try_next(
                    FailureKind::Evidence,
                    "announcement record evidence is missing source_at",
                )
            })?;
            if announcement_date(source_at, "evidence source_at")? != published_date {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "announcement evidence source date does not match publication date",
                ));
            }
        }
        Ok(batch)
    })
}

fn announcement_date(value: &str, field: &str) -> Result<IsoDate, SourceError> {
    let date = value.get(..10).ok_or_else(|| {
        SourceError::try_next(
            FailureKind::Evidence,
            format!("announcement {field} must start with YYYY-MM-DD"),
        )
    })?;
    if !matches!(value.as_bytes().get(10), None | Some(b' ') | Some(b'T')) {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            format!("announcement {field} has an invalid date/time separator"),
        ));
    }
    IsoDate::new(date).map_err(|error| {
        SourceError::try_next(
            FailureKind::Evidence,
            format!("announcement {field} is invalid: {error}"),
        )
    })
}
date_range_source!(
    investor_question_source,
    InvestorQuestions,
    investor_questions,
    InvestorQuestion
);

pub fn global_news_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<PositiveU32, NewsItem>
where
    Provider: NewsProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.global_news(*request).map_err(&classify)?;
        if batch.records().len() > request.get() as usize {
            return Err(SourceError::try_next(
                FailureKind::Evidence,
                "global-news batch exceeds requested limit",
            ));
        }
        let mut item_ids = HashSet::with_capacity(batch.records().len());
        for record in batch.records() {
            if !item_ids.insert(record.item_id.as_str()) {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "global-news batch contains a duplicate item ID",
                ));
            }
        }
        Ok(batch)
    })
}

pub fn security_profile_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[InstrumentId], SecurityProfile>
where
    Provider: SecurityProfiles + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.security_profiles(request).map_err(&classify)
    })
}

pub fn financial_statement_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<FinancialStatementRequest, FinancialStatement>
where
    Provider: FinancialStatements + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &FinancialStatementRequest| {
        provider
            .financial_statements(&request.0, request.1)
            .map_err(&classify)
    })
}

pub fn limit_pool_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<LimitPoolRequest, LimitPoolEntry>
where
    Provider: LimitPools + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.limit_pool(request).map_err(&classify)
    })
}

pub fn option_contract_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<OptionContractsRequest, OptionContract>
where
    Provider: OptionData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &OptionContractsRequest| {
        provider
            .option_contracts(&request.0, request.1.as_ref())
            .map_err(&classify)
    })
}

pub fn option_quote_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[NonEmptyText], OptionQuote>
where
    Provider: OptionData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.option_quotes(request).map_err(&classify)
    })
}

pub fn option_greeks_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<[NonEmptyText], OptionGreeks>
where
    Provider: OptionData + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        provider.option_greeks(request).map_err(&classify)
    })
}
