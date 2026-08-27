use std::collections::HashSet;

use magic_market_core::{
    AssetClass, Bar, BarInterval, DataStatus, EvidenceTimestamp, InstrumentId, IsoDate,
    LimitPoolEntry, LimitPoolKind, Money, NonEmptyText, OrderBook, PositiveU32, Price, ProviderId,
    Quantity, Quote, SourceEvidence,
};
use serde::{de, Deserialize, Deserializer, Serialize};
use thiserror::Error;

const MAX_INDEX_QUOTES: usize = 6;
const MAX_INTRADAY_POINTS: u32 = 800;
const MAX_T0_INSTRUMENTS: usize = 8;
const MAX_T0_BARS: u32 = 800;
const MAX_OUTCOME_BARS: u32 = 800;
const MAX_LIMIT_POOL_ROWS: u32 = 1_000;

#[derive(Debug, Error)]
pub enum DerivedProductContractError {
    #[error("invalid derived-product request: {0}")]
    InvalidRequest(String),
    #[error("invalid core value: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexQuotesRequest {
    indices: Vec<InstrumentId>,
    maximum_source_age_millis: u64,
}

impl IndexQuotesRequest {
    pub fn new(
        indices: Vec<InstrumentId>,
        maximum_source_age_millis: u64,
    ) -> Result<Self, DerivedProductContractError> {
        validate_cardinality("IndexQuotes indices", indices.len(), MAX_INDEX_QUOTES)?;
        if maximum_source_age_millis == 0 {
            return Err(invalid("maximum_source_age_millis must be positive"));
        }
        validate_unique(&indices, "index identity")?;
        for instrument in &indices {
            if instrument.asset_class() != AssetClass::Index {
                return Err(invalid(format!(
                    "{} is not an index identity",
                    instrument.code()
                )));
            }
            if instrument.exchange() == magic_market_core::Exchange::Beijing {
                return Err(invalid("Beijing index identity is not admitted"));
            }
            validate_six_digit_code(instrument)?;
        }
        Ok(Self {
            indices,
            maximum_source_age_millis,
        })
    }

    pub fn indices(&self) -> &[InstrumentId] {
        &self.indices
    }

    pub fn maximum_source_age_millis(&self) -> u64 {
        self.maximum_source_age_millis
    }
}

impl<'de> Deserialize<'de> for IndexQuotesRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            indices: Vec<InstrumentId>,
            maximum_source_age_millis: u64,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.indices, wire.maximum_source_age_millis).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntradayShapeRequest {
    instrument: InstrumentId,
    trading_date: Option<IsoDate>,
    maximum_points: PositiveU32,
}

impl IntradayShapeRequest {
    pub fn new(
        instrument: InstrumentId,
        trading_date: Option<IsoDate>,
        maximum_points: PositiveU32,
    ) -> Result<Self, DerivedProductContractError> {
        validate_a_share_equity(&instrument)?;
        validate_upper_bound("maximum_points", maximum_points.get(), MAX_INTRADAY_POINTS)?;
        Ok(Self {
            instrument,
            trading_date,
            maximum_points,
        })
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn trading_date(&self) -> Option<&IsoDate> {
        self.trading_date.as_ref()
    }

    pub fn maximum_points(&self) -> PositiveU32 {
        self.maximum_points
    }
}

impl<'de> Deserialize<'de> for IntradayShapeRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            instrument: InstrumentId,
            trading_date: Option<IsoDate>,
            maximum_points: PositiveU32,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.instrument, wire.trading_date, wire.maximum_points)
            .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct T0EvidenceRequest {
    instruments: Vec<InstrumentId>,
    daily_bar_count: PositiveU32,
    five_minute_bar_count: PositiveU32,
    requested_at: NonEmptyText,
}

impl T0EvidenceRequest {
    pub fn new(
        instruments: Vec<InstrumentId>,
        daily_bar_count: PositiveU32,
        five_minute_bar_count: PositiveU32,
        requested_at: impl Into<String>,
    ) -> Result<Self, DerivedProductContractError> {
        validate_cardinality(
            "T0Evidence instruments",
            instruments.len(),
            MAX_T0_INSTRUMENTS,
        )?;
        validate_unique(&instruments, "instrument identity")?;
        for instrument in &instruments {
            validate_a_share_equity(instrument)?;
        }
        validate_upper_bound("daily_bar_count", daily_bar_count.get(), MAX_T0_BARS)?;
        validate_upper_bound(
            "five_minute_bar_count",
            five_minute_bar_count.get(),
            MAX_T0_BARS,
        )?;
        let requested_at = NonEmptyText::new(requested_at)?;
        EvidenceTimestamp::parse_instant(requested_at.as_str())?;
        Ok(Self {
            instruments,
            daily_bar_count,
            five_minute_bar_count,
            requested_at,
        })
    }

    pub fn instruments(&self) -> &[InstrumentId] {
        &self.instruments
    }

    pub fn daily_bar_count(&self) -> PositiveU32 {
        self.daily_bar_count
    }

    pub fn five_minute_bar_count(&self) -> PositiveU32 {
        self.five_minute_bar_count
    }

    pub fn requested_at(&self) -> &str {
        self.requested_at.as_str()
    }
}

impl<'de> Deserialize<'de> for T0EvidenceRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            instruments: Vec<InstrumentId>,
            daily_bar_count: PositiveU32,
            five_minute_bar_count: PositiveU32,
            requested_at: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(
            wire.instruments,
            wire.daily_bar_count,
            wire.five_minute_bar_count,
            wire.requested_at,
        )
        .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutcomeDailyBarsRequest {
    instrument: InstrumentId,
    through: IsoDate,
    limit: PositiveU32,
    outcome_due_at: NonEmptyText,
}

impl OutcomeDailyBarsRequest {
    pub fn new(
        instrument: InstrumentId,
        through: IsoDate,
        limit: PositiveU32,
        outcome_due_at: impl Into<String>,
    ) -> Result<Self, DerivedProductContractError> {
        validate_a_share_equity(&instrument)?;
        validate_upper_bound("limit", limit.get(), MAX_OUTCOME_BARS)?;
        let outcome_due_at = NonEmptyText::new(outcome_due_at)?;
        let due = EvidenceTimestamp::parse_instant(outcome_due_at.as_str())?;
        let session_end =
            EvidenceTimestamp::parse_instant(&format!("{}T15:00:00+08:00", through.as_str()))?;
        if due < session_end {
            return Err(invalid(
                "outcome_due_at must not precede the requested through session end",
            ));
        }
        Ok(Self {
            instrument,
            through,
            limit,
            outcome_due_at,
        })
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn through(&self) -> &IsoDate {
        &self.through
    }

    pub fn limit(&self) -> PositiveU32 {
        self.limit
    }

    pub fn outcome_due_at(&self) -> &str {
        self.outcome_due_at.as_str()
    }
}

impl<'de> Deserialize<'de> for OutcomeDailyBarsRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            instrument: InstrumentId,
            through: IsoDate,
            limit: PositiveU32,
            outcome_due_at: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(
            wire.instrument,
            wire.through,
            wire.limit,
            wire.outcome_due_at,
        )
        .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpperLimitPoolReviewRequest {
    trading_date: IsoDate,
    per_pool_limit: PositiveU32,
}

impl UpperLimitPoolReviewRequest {
    pub fn new(
        trading_date: IsoDate,
        per_pool_limit: PositiveU32,
    ) -> Result<Self, DerivedProductContractError> {
        validate_upper_bound("per_pool_limit", per_pool_limit.get(), MAX_LIMIT_POOL_ROWS)?;
        Ok(Self {
            trading_date,
            per_pool_limit,
        })
    }

    pub fn trading_date(&self) -> &IsoDate {
        &self.trading_date
    }

    pub fn per_pool_limit(&self) -> PositiveU32 {
        self.per_pool_limit
    }
}

impl<'de> Deserialize<'de> for UpperLimitPoolReviewRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            trading_date: IsoDate,
            per_pool_limit: PositiveU32,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.trading_date, wire.per_pool_limit).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntradayShapeRecord {
    instrument: InstrumentId,
    trading_date: IsoDate,
    source_interval: BarInterval,
    first_at: NonEmptyText,
    last_at: NonEmptyText,
    point_count: PositiveU32,
    open: Price,
    high: Price,
    low: Price,
    latest: Price,
    vwap: Option<Price>,
    cumulative_volume: Option<Quantity>,
    cumulative_amount: Option<Money>,
    up_points: u32,
    down_points: u32,
    flat_points: u32,
    input_evidence: Vec<SourceEvidence>,
    algorithm_id: NonEmptyText,
    algorithm_revision: PositiveU32,
    input_digest_sha256: String,
}

impl IntradayShapeRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instrument: InstrumentId,
        trading_date: IsoDate,
        first_at: impl Into<String>,
        last_at: impl Into<String>,
        point_count: PositiveU32,
        open: Price,
        high: Price,
        low: Price,
        latest: Price,
        vwap: Option<Price>,
        cumulative_volume: Option<Quantity>,
        cumulative_amount: Option<Money>,
        up_points: u32,
        down_points: u32,
        flat_points: u32,
        input_evidence: Vec<SourceEvidence>,
        algorithm_revision: PositiveU32,
        input_digest_sha256: impl Into<String>,
    ) -> Result<Self, DerivedProductContractError> {
        validate_a_share_equity(&instrument)?;
        let first_at = NonEmptyText::new(first_at)?;
        let last_at = NonEmptyText::new(last_at)?;
        let first = EvidenceTimestamp::parse_instant(first_at.as_str())?;
        let last = EvidenceTimestamp::parse_instant(last_at.as_str())?;
        if first > last
            || !first_at.as_str().starts_with(trading_date.as_str())
            || !last_at.as_str().starts_with(trading_date.as_str())
        {
            return Err(invalid(
                "intraday endpoints must be ordered instants on trading_date",
            ));
        }
        if low.get() > open.get().min(latest.get())
            || high.get() < open.get().max(latest.get())
            || low.get() > high.get()
        {
            return Err(invalid("intraday OHLC/latest range is inconsistent"));
        }
        let classified = up_points
            .checked_add(down_points)
            .and_then(|value| value.checked_add(flat_points))
            .ok_or_else(|| invalid("intraday direction counts overflow u32"))?;
        if classified != point_count.get() {
            return Err(invalid("intraday direction counts must equal point_count"));
        }
        if cumulative_amount.is_some_and(|amount| amount.get() < 0.0) {
            return Err(invalid("cumulative_amount must not be negative"));
        }
        validate_single_provider_input(&input_evidence)?;
        let input_digest_sha256 = input_digest_sha256.into();
        validate_digest(&input_digest_sha256)?;
        Ok(Self {
            instrument,
            trading_date,
            source_interval: BarInterval::Minute1,
            first_at,
            last_at,
            point_count,
            open,
            high,
            low,
            latest,
            vwap,
            cumulative_volume,
            cumulative_amount,
            up_points,
            down_points,
            flat_points,
            input_evidence,
            algorithm_id: NonEmptyText::new("magic.intraday_shape")?,
            algorithm_revision,
            input_digest_sha256,
        })
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn trading_date(&self) -> &IsoDate {
        &self.trading_date
    }

    pub fn point_count(&self) -> PositiveU32 {
        self.point_count
    }

    pub fn source_interval(&self) -> BarInterval {
        self.source_interval
    }

    pub fn first_at(&self) -> &str {
        self.first_at.as_str()
    }

    pub fn last_at(&self) -> &str {
        self.last_at.as_str()
    }

    pub fn open(&self) -> Price {
        self.open
    }

    pub fn high(&self) -> Price {
        self.high
    }

    pub fn low(&self) -> Price {
        self.low
    }

    pub fn latest(&self) -> Price {
        self.latest
    }

    pub fn vwap(&self) -> Option<Price> {
        self.vwap
    }

    pub fn cumulative_volume(&self) -> Option<Quantity> {
        self.cumulative_volume
    }

    pub fn cumulative_amount(&self) -> Option<Money> {
        self.cumulative_amount
    }

    pub fn up_points(&self) -> u32 {
        self.up_points
    }

    pub fn down_points(&self) -> u32 {
        self.down_points
    }

    pub fn flat_points(&self) -> u32 {
        self.flat_points
    }

    pub fn input_evidence(&self) -> &[SourceEvidence] {
        &self.input_evidence
    }

    pub fn algorithm_revision(&self) -> PositiveU32 {
        self.algorithm_revision
    }

    pub fn input_digest_sha256(&self) -> &str {
        &self.input_digest_sha256
    }
}

impl<'de> Deserialize<'de> for IntradayShapeRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            instrument: InstrumentId,
            trading_date: IsoDate,
            source_interval: BarInterval,
            first_at: String,
            last_at: String,
            point_count: PositiveU32,
            open: Price,
            high: Price,
            low: Price,
            latest: Price,
            vwap: Option<Price>,
            cumulative_volume: Option<Quantity>,
            cumulative_amount: Option<Money>,
            up_points: u32,
            down_points: u32,
            flat_points: u32,
            input_evidence: Vec<SourceEvidence>,
            algorithm_id: String,
            algorithm_revision: PositiveU32,
            input_digest_sha256: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        validate_algorithm(&wire.algorithm_id, "magic.intraday_shape")
            .map_err(de::Error::custom)?;
        if wire.source_interval != BarInterval::Minute1 {
            return Err(de::Error::custom(
                "IntradayShape source_interval must be Minute1",
            ));
        }
        Self::new(
            wire.instrument,
            wire.trading_date,
            wire.first_at,
            wire.last_at,
            wire.point_count,
            wire.open,
            wire.high,
            wire.low,
            wire.latest,
            wire.vwap,
            wire.cumulative_volume,
            wire.cumulative_amount,
            wire.up_points,
            wire.down_points,
            wire.flat_points,
            wire.input_evidence,
            wire.algorithm_revision,
            wire.input_digest_sha256,
        )
        .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct T0EvidenceRecord {
    instrument: InstrumentId,
    requested_at: NonEmptyText,
    quote: Quote,
    order_book: OrderBook,
    daily_bar_count: PositiveU32,
    five_minute_bar_count: PositiveU32,
    daily_bars: Vec<Bar>,
    five_minute_bars: Vec<Bar>,
    input_evidence: Vec<SourceEvidence>,
    algorithm_id: NonEmptyText,
    algorithm_revision: PositiveU32,
    input_digest_sha256: String,
}

impl T0EvidenceRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instrument: InstrumentId,
        requested_at: impl Into<String>,
        quote: Quote,
        order_book: OrderBook,
        daily_bars: Vec<Bar>,
        five_minute_bars: Vec<Bar>,
        expected_daily_bar_count: PositiveU32,
        expected_five_minute_bar_count: PositiveU32,
        input_evidence: Vec<SourceEvidence>,
        algorithm_revision: PositiveU32,
        input_digest_sha256: impl Into<String>,
    ) -> Result<Self, DerivedProductContractError> {
        validate_a_share_equity(&instrument)?;
        let requested_at = NonEmptyText::new(requested_at)?;
        EvidenceTimestamp::parse_instant(requested_at.as_str())?;
        if quote.instrument() != &instrument || order_book.instrument() != &instrument {
            return Err(invalid("T0 quote/order_book instrument mismatch"));
        }
        if quote.provider() != ProviderId::Tdx || order_book.provider() != ProviderId::Tdx {
            return Err(invalid("T0 quote/order_book must come from TDX"));
        }
        if matches!(
            quote.status(),
            DataStatus::Stale | DataStatus::Conflicted | DataStatus::Unsupported
        ) || matches!(
            order_book.status(),
            DataStatus::Stale | DataStatus::Conflicted | DataStatus::Unsupported
        ) {
            return Err(invalid(
                "T0 quote/order_book must be Available or explicitly Unavailable",
            ));
        }
        validate_bars(
            &daily_bars,
            &instrument,
            BarInterval::Day,
            expected_daily_bar_count,
            ProviderId::Tdx,
        )?;
        validate_bars(
            &five_minute_bars,
            &instrument,
            BarInterval::Minute5,
            expected_five_minute_bar_count,
            ProviderId::Tdx,
        )?;
        validate_input_evidence(&input_evidence, Some(ProviderId::Tdx))?;
        if input_evidence.len() != 4 {
            return Err(invalid(
                "T0 input_evidence must contain quote, order-book, daily-bar and five-minute-bar evidence in that order",
            ));
        }
        validate_record_evidence(
            quote.batch_id(),
            quote.observed_at(),
            quote.source_at(),
            &input_evidence[0],
            "T0 quote",
        )?;
        validate_record_evidence(
            order_book.batch_id(),
            order_book.observed_at(),
            order_book.source_at(),
            &input_evidence[1],
            "T0 order-book",
        )?;
        validate_bar_evidence(&daily_bars, &input_evidence[2], "T0 daily bars")?;
        validate_bar_evidence(&five_minute_bars, &input_evidence[3], "T0 five-minute bars")?;
        let input_digest_sha256 = input_digest_sha256.into();
        validate_digest(&input_digest_sha256)?;
        Ok(Self {
            instrument,
            requested_at,
            quote,
            order_book,
            daily_bar_count: expected_daily_bar_count,
            five_minute_bar_count: expected_five_minute_bar_count,
            daily_bars,
            five_minute_bars,
            input_evidence,
            algorithm_id: NonEmptyText::new("magic.t0_evidence")?,
            algorithm_revision,
            input_digest_sha256,
        })
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn requested_at(&self) -> &str {
        self.requested_at.as_str()
    }

    pub fn quote(&self) -> &Quote {
        &self.quote
    }

    pub fn order_book(&self) -> &OrderBook {
        &self.order_book
    }

    pub fn daily_bars(&self) -> &[Bar] {
        &self.daily_bars
    }

    pub fn daily_bar_count(&self) -> PositiveU32 {
        self.daily_bar_count
    }

    pub fn five_minute_bars(&self) -> &[Bar] {
        &self.five_minute_bars
    }

    pub fn five_minute_bar_count(&self) -> PositiveU32 {
        self.five_minute_bar_count
    }

    pub fn input_evidence(&self) -> &[SourceEvidence] {
        &self.input_evidence
    }

    pub fn algorithm_revision(&self) -> PositiveU32 {
        self.algorithm_revision
    }

    pub fn input_digest_sha256(&self) -> &str {
        &self.input_digest_sha256
    }
}

impl<'de> Deserialize<'de> for T0EvidenceRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            instrument: InstrumentId,
            requested_at: String,
            quote: Quote,
            order_book: OrderBook,
            daily_bar_count: PositiveU32,
            five_minute_bar_count: PositiveU32,
            daily_bars: Vec<Bar>,
            five_minute_bars: Vec<Bar>,
            input_evidence: Vec<SourceEvidence>,
            algorithm_id: String,
            algorithm_revision: PositiveU32,
            input_digest_sha256: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        validate_algorithm(&wire.algorithm_id, "magic.t0_evidence").map_err(de::Error::custom)?;
        Self::new(
            wire.instrument,
            wire.requested_at,
            wire.quote,
            wire.order_book,
            wire.daily_bars,
            wire.five_minute_bars,
            wire.daily_bar_count,
            wire.five_minute_bar_count,
            wire.input_evidence,
            wire.algorithm_revision,
            wire.input_digest_sha256,
        )
        .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutcomeDailyBarsRecord {
    instrument: InstrumentId,
    requested_through: IsoDate,
    requested_limit: PositiveU32,
    outcome_due_at: NonEmptyText,
    bars: Vec<Bar>,
    input_evidence: Vec<SourceEvidence>,
    algorithm_id: NonEmptyText,
    algorithm_revision: PositiveU32,
    input_digest_sha256: String,
}

impl OutcomeDailyBarsRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instrument: InstrumentId,
        requested_through: IsoDate,
        outcome_due_at: impl Into<String>,
        bars: Vec<Bar>,
        expected_bar_count: PositiveU32,
        input_evidence: Vec<SourceEvidence>,
        algorithm_revision: PositiveU32,
        input_digest_sha256: impl Into<String>,
    ) -> Result<Self, DerivedProductContractError> {
        validate_a_share_equity(&instrument)?;
        let outcome_due_at = NonEmptyText::new(outcome_due_at)?;
        let due = EvidenceTimestamp::parse_instant(outcome_due_at.as_str())?;
        let session_end = EvidenceTimestamp::parse_instant(&format!(
            "{}T15:00:00+08:00",
            requested_through.as_str()
        ))?;
        if due < session_end {
            return Err(invalid(
                "outcome_due_at must not precede requested_through session end",
            ));
        }
        validate_bars(
            &bars,
            &instrument,
            BarInterval::Day,
            expected_bar_count,
            ProviderId::Tdx,
        )?;
        if bars
            .last()
            .is_none_or(|bar| bar.bar_end() != requested_through.as_str())
        {
            return Err(invalid(
                "newest outcome daily bar must equal requested_through",
            ));
        }
        validate_input_evidence(&input_evidence, Some(ProviderId::Tdx))?;
        if input_evidence.len() != 1 {
            return Err(invalid(
                "OutcomeDailyBars input_evidence must contain exactly one bar batch",
            ));
        }
        validate_bar_evidence(&bars, &input_evidence[0], "OutcomeDailyBars")?;
        let input_digest_sha256 = input_digest_sha256.into();
        validate_digest(&input_digest_sha256)?;
        Ok(Self {
            instrument,
            requested_through,
            requested_limit: expected_bar_count,
            outcome_due_at,
            bars,
            input_evidence,
            algorithm_id: NonEmptyText::new("magic.outcome_daily_bars")?,
            algorithm_revision,
            input_digest_sha256,
        })
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn requested_through(&self) -> &IsoDate {
        &self.requested_through
    }

    pub fn outcome_due_at(&self) -> &str {
        self.outcome_due_at.as_str()
    }

    pub fn requested_limit(&self) -> PositiveU32 {
        self.requested_limit
    }

    pub fn bars(&self) -> &[Bar] {
        &self.bars
    }

    pub fn input_evidence(&self) -> &[SourceEvidence] {
        &self.input_evidence
    }

    pub fn algorithm_revision(&self) -> PositiveU32 {
        self.algorithm_revision
    }

    pub fn input_digest_sha256(&self) -> &str {
        &self.input_digest_sha256
    }
}

impl<'de> Deserialize<'de> for OutcomeDailyBarsRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            instrument: InstrumentId,
            requested_through: IsoDate,
            requested_limit: PositiveU32,
            outcome_due_at: String,
            bars: Vec<Bar>,
            input_evidence: Vec<SourceEvidence>,
            algorithm_id: String,
            algorithm_revision: PositiveU32,
            input_digest_sha256: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        validate_algorithm(&wire.algorithm_id, "magic.outcome_daily_bars")
            .map_err(de::Error::custom)?;
        Self::new(
            wire.instrument,
            wire.requested_through,
            wire.outcome_due_at,
            wire.bars,
            wire.requested_limit,
            wire.input_evidence,
            wire.algorithm_revision,
            wire.input_digest_sha256,
        )
        .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpperLimitPoolReviewRecord {
    trading_date: IsoDate,
    upper: Vec<LimitPoolEntry>,
    broken: Vec<LimitPoolEntry>,
    lower: Vec<LimitPoolEntry>,
    previous_upper: Vec<LimitPoolEntry>,
    upper_count: u32,
    broken_count: u32,
    lower_count: u32,
    previous_upper_count: u32,
    maximum_streak: Option<u32>,
    input_evidence: Vec<SourceEvidence>,
    algorithm_id: NonEmptyText,
    algorithm_revision: PositiveU32,
    input_digest_sha256: String,
}

impl UpperLimitPoolReviewRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trading_date: IsoDate,
        upper: Vec<LimitPoolEntry>,
        broken: Vec<LimitPoolEntry>,
        lower: Vec<LimitPoolEntry>,
        previous_upper: Vec<LimitPoolEntry>,
        input_evidence: Vec<SourceEvidence>,
        algorithm_revision: PositiveU32,
        input_digest_sha256: impl Into<String>,
    ) -> Result<Self, DerivedProductContractError> {
        validate_pool(&upper, LimitPoolKind::Upper, &trading_date)?;
        validate_pool(&broken, LimitPoolKind::Broken, &trading_date)?;
        validate_pool(&lower, LimitPoolKind::Lower, &trading_date)?;
        validate_pool(&previous_upper, LimitPoolKind::PreviousUpper, &trading_date)?;
        validate_input_evidence(&input_evidence, Some(ProviderId::Eastmoney))?;
        let upper_count = checked_len("upper", upper.len())?;
        let broken_count = checked_len("broken", broken.len())?;
        let lower_count = checked_len("lower", lower.len())?;
        let previous_upper_count = checked_len("previous_upper", previous_upper.len())?;
        let maximum_streak = upper
            .iter()
            .chain(&broken)
            .chain(&lower)
            .chain(&previous_upper)
            .filter_map(|entry| entry.streak.map(PositiveU32::get))
            .max();
        let input_digest_sha256 = input_digest_sha256.into();
        validate_digest(&input_digest_sha256)?;
        Ok(Self {
            trading_date,
            upper,
            broken,
            lower,
            previous_upper,
            upper_count,
            broken_count,
            lower_count,
            previous_upper_count,
            maximum_streak,
            input_evidence,
            algorithm_id: NonEmptyText::new("magic.upper_limit_pool_review")?,
            algorithm_revision,
            input_digest_sha256,
        })
    }

    pub fn trading_date(&self) -> &IsoDate {
        &self.trading_date
    }

    pub fn upper(&self) -> &[LimitPoolEntry] {
        &self.upper
    }

    pub fn broken(&self) -> &[LimitPoolEntry] {
        &self.broken
    }

    pub fn lower(&self) -> &[LimitPoolEntry] {
        &self.lower
    }

    pub fn previous_upper(&self) -> &[LimitPoolEntry] {
        &self.previous_upper
    }

    pub fn maximum_streak(&self) -> Option<u32> {
        self.maximum_streak
    }

    pub fn upper_count(&self) -> u32 {
        self.upper_count
    }

    pub fn broken_count(&self) -> u32 {
        self.broken_count
    }

    pub fn lower_count(&self) -> u32 {
        self.lower_count
    }

    pub fn previous_upper_count(&self) -> u32 {
        self.previous_upper_count
    }

    pub fn input_evidence(&self) -> &[SourceEvidence] {
        &self.input_evidence
    }

    pub fn input_digest_sha256(&self) -> &str {
        &self.input_digest_sha256
    }

    pub fn algorithm_revision(&self) -> PositiveU32 {
        self.algorithm_revision
    }
}

impl<'de> Deserialize<'de> for UpperLimitPoolReviewRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            trading_date: IsoDate,
            upper: Vec<LimitPoolEntry>,
            broken: Vec<LimitPoolEntry>,
            lower: Vec<LimitPoolEntry>,
            previous_upper: Vec<LimitPoolEntry>,
            upper_count: u32,
            broken_count: u32,
            lower_count: u32,
            previous_upper_count: u32,
            maximum_streak: Option<u32>,
            input_evidence: Vec<SourceEvidence>,
            algorithm_id: String,
            algorithm_revision: PositiveU32,
            input_digest_sha256: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        if wire.algorithm_id != "magic.upper_limit_pool_review" {
            return Err(de::Error::custom(
                "UpperLimitPoolReview algorithm_id is invalid",
            ));
        }
        let record = Self::new(
            wire.trading_date,
            wire.upper,
            wire.broken,
            wire.lower,
            wire.previous_upper,
            wire.input_evidence,
            wire.algorithm_revision,
            wire.input_digest_sha256,
        )
        .map_err(de::Error::custom)?;
        if record.upper_count != wire.upper_count
            || record.broken_count != wire.broken_count
            || record.lower_count != wire.lower_count
            || record.previous_upper_count != wire.previous_upper_count
            || record.maximum_streak != wire.maximum_streak
        {
            return Err(de::Error::custom(
                "UpperLimitPoolReview derived counts contradict its pools",
            ));
        }
        Ok(record)
    }
}

fn validate_cardinality(
    field: &str,
    length: usize,
    maximum: usize,
) -> Result<(), DerivedProductContractError> {
    if length == 0 || length > maximum {
        return Err(invalid(format!("{field} must contain 1..={maximum} items")));
    }
    Ok(())
}

fn validate_upper_bound(
    field: &str,
    value: u32,
    maximum: u32,
) -> Result<(), DerivedProductContractError> {
    if value > maximum {
        return Err(invalid(format!("{field} must be at most {maximum}")));
    }
    Ok(())
}

fn validate_unique(
    instruments: &[InstrumentId],
    label: &str,
) -> Result<(), DerivedProductContractError> {
    let mut seen = HashSet::with_capacity(instruments.len());
    for instrument in instruments {
        if !seen.insert(instrument) {
            return Err(invalid(format!("duplicate {label} {}", instrument.code())));
        }
    }
    Ok(())
}

fn validate_a_share_equity(instrument: &InstrumentId) -> Result<(), DerivedProductContractError> {
    if instrument.asset_class() != AssetClass::Equity {
        return Err(invalid(format!(
            "{} is not an A-share equity identity",
            instrument.code()
        )));
    }
    validate_six_digit_code(instrument)
}

fn validate_six_digit_code(instrument: &InstrumentId) -> Result<(), DerivedProductContractError> {
    if instrument.code().len() != 6 || !instrument.code().bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid(format!(
            "{} must be a six-digit market code",
            instrument.code()
        )));
    }
    Ok(())
}

fn validate_pool(
    entries: &[LimitPoolEntry],
    expected_kind: LimitPoolKind,
    expected_date: &IsoDate,
) -> Result<(), DerivedProductContractError> {
    validate_upper_bound(
        "limit-pool response rows",
        u32::try_from(entries.len())
            .map_err(|_| invalid("limit-pool response row count exceeds u32"))?,
        MAX_LIMIT_POOL_ROWS,
    )?;
    for entry in entries {
        if entry.kind != expected_kind {
            return Err(invalid(
                "limit-pool entry kind contradicts its response family",
            ));
        }
        if &entry.trading_date != expected_date {
            return Err(invalid(
                "limit-pool entry trading_date contradicts the requested date",
            ));
        }
        if entry.evidence.provider() != ProviderId::Eastmoney {
            return Err(invalid(
                "limit-pool entry evidence is not from the Eastmoney provider",
            ));
        }
    }
    Ok(())
}

fn validate_input_evidence(
    input_evidence: &[SourceEvidence],
    expected_provider: Option<ProviderId>,
) -> Result<(), DerivedProductContractError> {
    if input_evidence.is_empty() {
        return Err(invalid("input_evidence must not be empty"));
    }
    if let Some(expected_provider) = expected_provider {
        for evidence in input_evidence {
            if evidence.provider() != expected_provider {
                return Err(invalid("input_evidence provider is inconsistent"));
            }
        }
    }
    Ok(())
}

fn validate_single_provider_input(
    input_evidence: &[SourceEvidence],
) -> Result<(), DerivedProductContractError> {
    validate_input_evidence(input_evidence, None)?;
    let provider = input_evidence[0].provider();
    if input_evidence
        .iter()
        .any(|evidence| evidence.provider() != provider)
    {
        return Err(invalid(
            "input_evidence must come from exactly one Provider",
        ));
    }
    Ok(())
}

fn validate_bars(
    bars: &[Bar],
    instrument: &InstrumentId,
    interval: BarInterval,
    expected_count: PositiveU32,
    provider: ProviderId,
) -> Result<(), DerivedProductContractError> {
    let actual_count = checked_len("bars", bars.len())?;
    if actual_count != expected_count.get() {
        return Err(invalid("bar count contradicts the requested count"));
    }
    if actual_count > MAX_T0_BARS {
        return Err(invalid(format!("bar count must be at most {MAX_T0_BARS}")));
    }
    let mut previous_start: Option<&str> = None;
    for bar in bars {
        if bar.instrument() != instrument {
            return Err(invalid("bar instrument mismatch"));
        }
        if bar.interval() != interval {
            return Err(invalid("bar interval mismatch"));
        }
        if bar.provider() != provider {
            return Err(invalid("bar Provider mismatch"));
        }
        if bar.source_at().is_none() || bar.observed_at().is_none() {
            return Err(invalid(
                "derived-product bars require source_at and observed_at evidence",
            ));
        }
        if previous_start.is_some_and(|previous| previous >= bar.bar_start()) {
            return Err(invalid("bars must be strictly oldest-to-newest"));
        }
        previous_start = Some(bar.bar_start());
    }
    Ok(())
}

fn validate_record_evidence(
    batch_id: &str,
    observed_at: &str,
    source_at: Option<&str>,
    evidence: &SourceEvidence,
    family: &str,
) -> Result<(), DerivedProductContractError> {
    if batch_id != evidence.batch_id()
        || observed_at != evidence.observed_at()
        || source_at != evidence.source_at()
    {
        return Err(invalid(format!(
            "{family} record evidence contradicts input_evidence"
        )));
    }
    Ok(())
}

fn validate_bar_evidence(
    bars: &[Bar],
    evidence: &SourceEvidence,
    family: &str,
) -> Result<(), DerivedProductContractError> {
    for bar in bars {
        if bar.batch_id() != evidence.batch_id()
            || bar.observed_at() != Some(evidence.observed_at())
        {
            return Err(invalid(format!(
                "{family} record evidence contradicts input_evidence"
            )));
        }
    }
    if bars.last().and_then(Bar::source_at) != evidence.source_at() {
        return Err(invalid(format!(
            "{family} latest source time contradicts input_evidence"
        )));
    }
    Ok(())
}

fn validate_algorithm(actual: &str, expected: &str) -> Result<(), DerivedProductContractError> {
    if actual != expected {
        return Err(invalid(format!("algorithm_id must be exactly {expected}")));
    }
    Ok(())
}

fn checked_len(field: &str, length: usize) -> Result<u32, DerivedProductContractError> {
    u32::try_from(length).map_err(|_| invalid(format!("{field} row count exceeds u32")))
}

fn validate_digest(value: &str) -> Result<(), DerivedProductContractError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "input_digest_sha256 must be exactly 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> DerivedProductContractError {
    DerivedProductContractError::InvalidRequest(message.into())
}

#[cfg(test)]
mod tests {
    use magic_market_core::{Adjustment, BookLevel, Exchange, Ratio, RatioUnit};

    use super::*;

    fn equity() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
    }

    fn index() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).unwrap()
    }

    fn evidence(provider: ProviderId, batch: &str) -> SourceEvidence {
        SourceEvidence::new(provider, "2026-08-14T15:01:00+08:00", batch)
            .unwrap()
            .with_source_at("2026-08-14T15:00:00+08:00")
            .unwrap()
    }

    fn bar(interval: BarInterval, batch: &str) -> Bar {
        let (start, end) = match interval {
            BarInterval::Day => ("2026-08-14", "2026-08-14"),
            BarInterval::Minute5 => ("2026-08-14T09:30:00", "2026-08-14T09:35:00"),
            _ => unreachable!(),
        };
        Bar::new(
            equity(),
            interval,
            start,
            end,
            Price::new(10.0).unwrap(),
            Price::new(11.0).unwrap(),
            Price::new(9.0).unwrap(),
            Price::new(10.5).unwrap(),
            Quantity::new(100.0).unwrap(),
            Some(Money::new(1_000.0).unwrap()),
            Adjustment::Unadjusted,
            ProviderId::Tdx,
            batch,
        )
        .unwrap()
        .with_source_at("2026-08-14T15:00:00+08:00")
        .unwrap()
        .with_observed_at("2026-08-14T15:01:00+08:00")
        .unwrap()
    }

    fn quote() -> Quote {
        Quote::from_parts(
            equity(),
            Some("Example".into()),
            Price::new(10.5).unwrap(),
            Some(Price::new(10.0).unwrap()),
            Some(Price::new(10.0).unwrap()),
            Some(Price::new(11.0).unwrap()),
            Some(Price::new(9.0).unwrap()),
            Some(Ratio::new(5.0, RatioUnit::Percent).unwrap()),
            Quantity::new(100.0).unwrap(),
            Some(Money::new(1_000.0).unwrap()),
            DataStatus::Available,
            Some("2026-08-14T15:00:00+08:00".into()),
            "2026-08-14T15:01:00+08:00",
            ProviderId::Tdx,
            "quote-batch",
        )
        .unwrap()
    }

    fn order_book() -> OrderBook {
        let level = BookLevel::new(
            Some(Price::new(10.0).unwrap()),
            Some(Quantity::new(100.0).unwrap()),
        )
        .unwrap();
        OrderBook::new(
            equity(),
            [level; 5],
            [level; 5],
            Some(Quantity::new(500.0).unwrap()),
            Some(Quantity::new(500.0).unwrap()),
            DataStatus::Available,
            Some("2026-08-14T15:00:00+08:00".into()),
            "2026-08-14T15:01:00+08:00",
            ProviderId::Tdx,
            "book-batch",
        )
        .unwrap()
    }

    #[test]
    fn all_five_request_contracts_round_trip_through_their_checked_constructors() {
        let index_request = IndexQuotesRequest::new(vec![index()], 5_000).unwrap();
        let decoded: IndexQuotesRequest =
            serde_json::from_slice(&serde_json::to_vec(&index_request).unwrap()).unwrap();
        assert_eq!(decoded, index_request);

        let shape = IntradayShapeRequest::new(
            equity(),
            None,
            PositiveU32::new(MAX_INTRADAY_POINTS).unwrap(),
        )
        .unwrap();
        let decoded: IntradayShapeRequest =
            serde_json::from_slice(&serde_json::to_vec(&shape).unwrap()).unwrap();
        assert_eq!(decoded, shape);

        let t0 = T0EvidenceRequest::new(
            vec![equity()],
            PositiveU32::new(20).unwrap(),
            PositiveU32::new(48).unwrap(),
            "2026-08-14T15:01:00+08:00",
        )
        .unwrap();
        let decoded: T0EvidenceRequest =
            serde_json::from_slice(&serde_json::to_vec(&t0).unwrap()).unwrap();
        assert_eq!(decoded, t0);

        let outcome = OutcomeDailyBarsRequest::new(
            equity(),
            IsoDate::new("2026-08-14").unwrap(),
            PositiveU32::new(20).unwrap(),
            "2026-08-14T15:35:00+08:00",
        )
        .unwrap();
        let decoded: OutcomeDailyBarsRequest =
            serde_json::from_slice(&serde_json::to_vec(&outcome).unwrap()).unwrap();
        assert_eq!(decoded, outcome);

        let review = UpperLimitPoolReviewRequest::new(
            IsoDate::new("2026-08-14").unwrap(),
            PositiveU32::new(MAX_LIMIT_POOL_ROWS).unwrap(),
        )
        .unwrap();
        let decoded: UpperLimitPoolReviewRequest =
            serde_json::from_slice(&serde_json::to_vec(&review).unwrap()).unwrap();
        assert_eq!(decoded, review);
    }

    #[test]
    fn t0_evidence_v2_requires_and_preserves_the_caller_requested_at_instant() {
        let request: T0EvidenceRequest = serde_json::from_value(serde_json::json!({
            "instruments": [equity()],
            "daily_bar_count": 20,
            "five_minute_bar_count": 48,
            "requested_at": "2026-08-27T09:57:40+08:00"
        }))
        .unwrap();

        assert_eq!(
            serde_json::to_value(request).unwrap()["requested_at"],
            "2026-08-27T09:57:40+08:00"
        );
        assert!(
            serde_json::from_value::<T0EvidenceRequest>(serde_json::json!({
                "instruments": [equity()],
                "daily_bar_count": 20,
                "five_minute_bar_count": 48
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<T0EvidenceRequest>(serde_json::json!({
                "instruments": [equity()],
                "daily_bar_count": 20,
                "five_minute_bar_count": 48,
                "requested_at": "2026-08-27 09:57:40"
            }))
            .is_err()
        );
    }

    #[test]
    fn invalid_identity_bounds_unknown_fields_and_due_time_fail_closed() {
        assert!(IndexQuotesRequest::new(vec![equity()], 5_000).is_err());
        assert!(IndexQuotesRequest::new(vec![index(), index()], 5_000).is_err());
        assert!(IntradayShapeRequest::new(
            equity(),
            None,
            PositiveU32::new(MAX_INTRADAY_POINTS + 1).unwrap()
        )
        .is_err());
        assert!(T0EvidenceRequest::new(
            vec![equity(), equity()],
            PositiveU32::new(20).unwrap(),
            PositiveU32::new(48).unwrap(),
            "2026-08-14T15:01:00+08:00"
        )
        .is_err());
        assert!(OutcomeDailyBarsRequest::new(
            equity(),
            IsoDate::new("2026-08-14").unwrap(),
            PositiveU32::new(20).unwrap(),
            "2026-08-14T14:59:59+08:00"
        )
        .is_err());
        assert!(serde_json::from_str::<UpperLimitPoolReviewRequest>(
            r#"{"trading_date":"2026-08-14","per_pool_limit":1,"extra":true}"#
        )
        .is_err());
    }

    #[test]
    fn derived_response_contracts_reject_tampered_counts_and_preserve_exact_inputs() {
        let shape = IntradayShapeRecord::new(
            equity(),
            IsoDate::new("2026-08-14").unwrap(),
            "2026-08-14T09:30:00+08:00",
            "2026-08-14T15:00:00+08:00",
            PositiveU32::new(3).unwrap(),
            Price::new(10.0).unwrap(),
            Price::new(11.0).unwrap(),
            Price::new(9.0).unwrap(),
            Price::new(10.5).unwrap(),
            Some(Price::new(10.2).unwrap()),
            Some(Quantity::new(100.0).unwrap()),
            Some(Money::new(1_000.0).unwrap()),
            1,
            1,
            1,
            vec![evidence(ProviderId::Tencent, "minute-batch")],
            PositiveU32::new(1).unwrap(),
            "a".repeat(64),
        )
        .unwrap();
        let decoded: IntradayShapeRecord =
            serde_json::from_slice(&serde_json::to_vec(&shape).unwrap()).unwrap();
        assert_eq!(decoded.point_count().get(), 3);

        let t0 = T0EvidenceRecord::new(
            equity(),
            "2026-08-14T15:00:30+08:00",
            quote(),
            order_book(),
            vec![bar(BarInterval::Day, "day-batch")],
            vec![bar(BarInterval::Minute5, "minute5-batch")],
            PositiveU32::new(1).unwrap(),
            PositiveU32::new(1).unwrap(),
            vec![
                evidence(ProviderId::Tdx, "quote-batch"),
                evidence(ProviderId::Tdx, "book-batch"),
                evidence(ProviderId::Tdx, "day-batch"),
                evidence(ProviderId::Tdx, "minute5-batch"),
            ],
            PositiveU32::new(1).unwrap(),
            "b".repeat(64),
        )
        .unwrap();
        let t0_round_trip: T0EvidenceRecord =
            serde_json::from_slice(&serde_json::to_vec(&t0).unwrap()).unwrap();
        assert_eq!(t0_round_trip.requested_at(), "2026-08-14T15:00:30+08:00");
        let mut t0_json = serde_json::to_value(&t0).unwrap();
        t0_json["daily_bar_count"] = serde_json::json!(2);
        assert!(serde_json::from_value::<T0EvidenceRecord>(t0_json).is_err());

        let outcome = OutcomeDailyBarsRecord::new(
            equity(),
            IsoDate::new("2026-08-14").unwrap(),
            "2026-08-14T15:35:00+08:00",
            vec![bar(BarInterval::Day, "outcome-batch")],
            PositiveU32::new(1).unwrap(),
            vec![evidence(ProviderId::Tdx, "outcome-batch")],
            PositiveU32::new(1).unwrap(),
            "c".repeat(64),
        )
        .unwrap();
        let mut outcome_json = serde_json::to_value(&outcome).unwrap();
        outcome_json["requested_limit"] = serde_json::json!(2);
        assert!(serde_json::from_value::<OutcomeDailyBarsRecord>(outcome_json).is_err());
    }
}
