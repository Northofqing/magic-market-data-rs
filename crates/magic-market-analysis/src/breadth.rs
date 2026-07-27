use crate::AnalysisError;
use magic_market_core::{
    DataBatch, EvidenceTimestamp, IsoDate, LimitPoolEntry, LimitPoolKind, MarketBreadth,
    MarketBreadthRequest, MarketBreadthSnapshot, MarketSession, NonEmptyText, Provenance,
    ProviderId, Quote, SecurityMetadata, SourceEvidence,
};
use std::collections::HashSet;

/// Marker used by callers that want deterministic local breadth composition.
pub trait BreadthAnalysis: MarketBreadth {}

impl<T: MarketBreadth> BreadthAnalysis for T {}

/// Complete, source-versioned membership of the named breadth universe.
#[derive(Debug, Clone)]
pub struct BreadthUniverse {
    name: NonEmptyText,
    as_of_date: IsoDate,
    version: NonEmptyText,
    evidence: SourceEvidence,
    members: DataBatch<SecurityMetadata>,
}

impl BreadthUniverse {
    pub fn new(
        name: NonEmptyText,
        as_of_date: IsoDate,
        version: NonEmptyText,
        provider: ProviderId,
        members: DataBatch<SecurityMetadata>,
    ) -> Result<Self, AnalysisError> {
        if members.records().is_empty() {
            return Err(AnalysisError::InvalidInput(
                "breadth universe membership must not be empty".into(),
            ));
        }
        if !members.quality().is_complete() || !members.quality().issues().is_empty() {
            return Err(AnalysisError::InvalidInput(
                "breadth universe membership must be a complete source batch".into(),
            ));
        }
        let provenance = members.provenance();
        let batch_id = provenance.batch_id().ok_or_else(|| {
            AnalysisError::InvalidInput("breadth universe provenance is missing batch_id".into())
        })?;
        let source_at = provenance.source_at().ok_or_else(|| {
            AnalysisError::InvalidInput("breadth universe provenance is missing source_at".into())
        })?;
        if source_at.get(..10) != Some(as_of_date.as_str()) {
            return Err(AnalysisError::InvalidInput(
                "breadth universe source date does not match its as-of date".into(),
            ));
        }
        let observed = EvidenceTimestamp::parse_instant(provenance.fetched_at())?;
        let source = EvidenceTimestamp::parse(source_at)?;
        if observed.duration_since(source).is_none() {
            return Err(AnalysisError::InvalidInput(
                "breadth universe source_at is later than fetched_at".into(),
            ));
        }
        let evidence = SourceEvidence::new(provider, provenance.fetched_at(), batch_id)?
            .with_source_at(source_at)?;
        let mut instruments = HashSet::with_capacity(members.records().len());
        for member in members.records() {
            if member.name().is_none()
                || member.provider() != provider
                || member.batch_id() != batch_id
                || member.observed_at() != provenance.fetched_at()
                || member.source_at() != provenance.source_at()
            {
                return Err(AnalysisError::InvalidInput(
                    "breadth universe member must carry code, name, and atomic batch evidence"
                        .into(),
                ));
            }
            if !instruments.insert(member.instrument().clone()) {
                return Err(AnalysisError::InvalidInput(format!(
                    "breadth universe contains duplicate instrument {}",
                    member.instrument().code()
                )));
            }
        }
        Ok(Self {
            name,
            as_of_date,
            version,
            evidence,
            members,
        })
    }

    pub fn name(&self) -> &NonEmptyText {
        &self.name
    }

    pub fn as_of_date(&self) -> &IsoDate {
        &self.as_of_date
    }

    pub fn version(&self) -> &NonEmptyText {
        &self.version
    }

    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }

    pub fn members(&self) -> &DataBatch<SecurityMetadata> {
        &self.members
    }
}

/// One complete, request-identified limit pool used by breadth composition.
///
/// The explicit provider and kind keep legitimate empty source batches
/// attributable; an empty `DataBatch<LimitPoolEntry>` alone cannot prove which
/// pool request produced it.
#[derive(Debug, Clone)]
pub struct BreadthLimitPool {
    kind: LimitPoolKind,
    trading_date: IsoDate,
    evidence: SourceEvidence,
    entries: DataBatch<LimitPoolEntry>,
}

impl BreadthLimitPool {
    pub fn new(
        kind: LimitPoolKind,
        trading_date: IsoDate,
        provider: ProviderId,
        entries: DataBatch<LimitPoolEntry>,
    ) -> Result<Self, AnalysisError> {
        if !matches!(kind, LimitPoolKind::Upper | LimitPoolKind::Lower) {
            return Err(AnalysisError::InvalidInput(
                "breadth accepts only complete upper or lower limit pools".into(),
            ));
        }
        if !entries.quality().is_complete() || !entries.quality().issues().is_empty() {
            return Err(AnalysisError::InvalidInput(
                "breadth limit pool must be a complete source batch".into(),
            ));
        }
        let provenance = entries.provenance();
        let batch_id = provenance.batch_id().ok_or_else(|| {
            AnalysisError::InvalidInput("breadth limit-pool provenance is missing batch_id".into())
        })?;
        let source_at = provenance.source_at().ok_or_else(|| {
            AnalysisError::InvalidInput("breadth limit-pool provenance is missing source_at".into())
        })?;
        if source_at != trading_date.as_str() {
            return Err(AnalysisError::InvalidInput(
                "breadth limit-pool provenance source_at must equal its trading date".into(),
            ));
        }
        let observed = EvidenceTimestamp::parse_instant(provenance.fetched_at())?;
        let source = EvidenceTimestamp::parse(source_at)?;
        if observed.duration_since(source).is_none() {
            return Err(AnalysisError::InvalidInput(
                "breadth limit-pool source_at is later than fetched_at".into(),
            ));
        }
        let evidence = SourceEvidence::new(provider, provenance.fetched_at(), batch_id)?
            .with_source_at(trading_date.as_str())?;
        let mut instruments = HashSet::with_capacity(entries.records().len());
        for entry in entries.records() {
            if entry.kind != kind
                || entry.trading_date != trading_date
                || entry.evidence.provider() != provider
                || entry.evidence.batch_id() != batch_id
                || entry.evidence.observed_at() != provenance.fetched_at()
                || entry.evidence.source_at() != provenance.source_at()
            {
                return Err(AnalysisError::InvalidInput(
                    "breadth limit-pool record does not atomically match its request and provenance"
                        .into(),
                ));
            }
            if !instruments.insert(entry.instrument.clone()) {
                return Err(AnalysisError::InvalidInput(format!(
                    "breadth limit-pool contains duplicate instrument {}",
                    entry.instrument.code()
                )));
            }
        }
        Ok(Self {
            kind,
            trading_date,
            evidence,
            entries,
        })
    }

    pub fn kind(&self) -> LimitPoolKind {
        self.kind
    }

    pub fn trading_date(&self) -> &IsoDate {
        &self.trading_date
    }

    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }

    pub fn entries(&self) -> &DataBatch<LimitPoolEntry> {
        &self.entries
    }
}

/// Network-free breadth computation over an explicitly identified quote universe.
///
/// Source skew is measured over the dynamic quote records. The universe and
/// limit pools retain their date-level source evidence without converting it
/// into a fabricated intraday source instant.
#[derive(Debug, Clone)]
pub struct MarketBreadthAnalysis {
    universe: BreadthUniverse,
    quotes: DataBatch<Quote>,
    upper_pool: BreadthLimitPool,
    lower_pool: BreadthLimitPool,
}

impl MarketBreadthAnalysis {
    pub fn new(
        universe: BreadthUniverse,
        quotes: DataBatch<Quote>,
        upper_pool: BreadthLimitPool,
        lower_pool: BreadthLimitPool,
    ) -> Result<Self, AnalysisError> {
        if quotes.records().is_empty() {
            return Err(AnalysisError::InvalidInput(
                "breadth quote universe must not be empty".into(),
            ));
        }
        let universe_identities = universe
            .members()
            .records()
            .iter()
            .map(|member| member.instrument().clone())
            .collect::<HashSet<_>>();
        let mut identities = HashSet::with_capacity(quotes.records().len());
        for quote in quotes.records() {
            if !universe_identities.contains(quote.instrument()) {
                return Err(AnalysisError::InvalidInput(format!(
                    "breadth quote {} is outside the proved universe",
                    quote.instrument().code()
                )));
            }
            if !identities.insert(quote.instrument().clone()) {
                return Err(AnalysisError::InvalidInput(format!(
                    "breadth quote universe contains duplicate instrument {}",
                    quote.instrument().code()
                )));
            }
        }
        if upper_pool.kind() != LimitPoolKind::Upper || lower_pool.kind() != LimitPoolKind::Lower {
            return Err(AnalysisError::InvalidInput(
                "breadth requires one upper and one lower limit pool".into(),
            ));
        }
        for (label, pool) in [("limit-up", &upper_pool), ("limit-down", &lower_pool)] {
            if let Some(unknown) = pool
                .entries()
                .records()
                .iter()
                .map(|entry| &entry.instrument)
                .find(|instrument| !universe_identities.contains(*instrument))
            {
                return Err(AnalysisError::InvalidInput(format!(
                    "breadth {label} set contains unknown instrument {}",
                    unknown.code()
                )));
            }
        }
        let limit_up = upper_pool
            .entries()
            .records()
            .iter()
            .map(|entry| &entry.instrument)
            .collect::<HashSet<_>>();
        let limit_down = lower_pool
            .entries()
            .records()
            .iter()
            .map(|entry| &entry.instrument)
            .collect::<HashSet<_>>();
        if let Some(instrument) = limit_up.intersection(&limit_down).next() {
            return Err(AnalysisError::InvalidInput(format!(
                "instrument {} cannot be both limit-up and limit-down",
                instrument.code()
            )));
        }
        Ok(Self {
            universe,
            quotes,
            upper_pool,
            lower_pool,
        })
    }
}

impl MarketBreadth for MarketBreadthAnalysis {
    type Error = AnalysisError;

    fn market_breadth(
        &self,
        request: &MarketBreadthRequest,
    ) -> Result<DataBatch<MarketBreadthSnapshot>, Self::Error> {
        if request.universe() != self.universe.name() {
            return Err(AnalysisError::InvalidInput(format!(
                "breadth request universe {} does not match configured {}",
                request.universe(),
                self.universe.name()
            )));
        }
        if self.universe.as_of_date() != request.source_date() {
            return Err(AnalysisError::InvalidInput(format!(
                "breadth universe as-of date {} does not match requested {}",
                self.universe.as_of_date(),
                request.source_date()
            )));
        }

        let evidence = quote_evidence(&self.quotes)?;
        if evidence.source_date.as_str() != request.source_date().as_str() {
            return Err(AnalysisError::InvalidInput(format!(
                "breadth source date {} does not match requested {}",
                evidence.source_date,
                request.source_date()
            )));
        }
        if evidence.source_session != request.source_session() {
            return Err(AnalysisError::InvalidInput(format!(
                "breadth source session {:?} does not match requested {:?}",
                evidence.source_session,
                request.source_session()
            )));
        }
        let QuoteEvidence {
            mut input_evidence,
            earliest,
            latest,
            conservative_source_at,
            ..
        } = evidence;
        input_evidence.insert(0, self.universe.evidence().clone());
        for pool in [&self.upper_pool, &self.lower_pool] {
            if pool.trading_date() != request.source_date() {
                return Err(AnalysisError::InvalidInput(format!(
                    "breadth {:?} pool date {} does not match requested {}",
                    pool.kind(),
                    pool.trading_date(),
                    request.source_date()
                )));
            }
            input_evidence.push(pool.evidence().clone());
        }
        let derived_observed_at = latest_input_observed_at(&input_evidence)?;
        for input in &input_evidence {
            let source_at = input.source_at().ok_or_else(|| {
                AnalysisError::InvalidInput("breadth input evidence must preserve source_at".into())
            })?;
            let source = EvidenceTimestamp::parse(source_at)?;
            let observed = EvidenceTimestamp::parse_instant(&derived_observed_at)?;
            if observed.duration_since(source).is_none() {
                return Err(AnalysisError::InvalidInput(
                    "breadth input source_at is later than the derived observation".into(),
                ));
            }
        }
        let source_skew = latest
            .duration_since(earliest)
            .ok_or_else(|| {
                AnalysisError::InvalidInput("breadth source time moved backwards".into())
            })?
            .as_millis();
        let source_skew = u64::try_from(source_skew).map_err(|_| {
            AnalysisError::InvalidInput("breadth source skew exceeds u64 milliseconds".into())
        })?;
        if source_skew > request.maximum_source_skew_millis() {
            return Err(AnalysisError::InvalidInput(format!(
                "breadth source skew {source_skew}ms exceeds requested {}ms",
                request.maximum_source_skew_millis()
            )));
        }

        let limit_up = self
            .upper_pool
            .entries()
            .records()
            .iter()
            .map(|entry| &entry.instrument)
            .collect::<HashSet<_>>();
        let limit_down = self
            .lower_pool
            .entries()
            .records()
            .iter()
            .map(|entry| &entry.instrument)
            .collect::<HashSet<_>>();
        let mut valid = 0_u32;
        let mut up = 0_u32;
        let mut down = 0_u32;
        let mut flat = 0_u32;
        let mut limit_up_count = 0_u32;
        let mut limit_down_count = 0_u32;
        for quote in self.quotes.records() {
            let Some(previous) = quote.previous_close() else {
                continue;
            };
            valid = valid.checked_add(1).ok_or_else(|| {
                AnalysisError::InvalidInput("breadth valid count overflow".into())
            })?;
            if quote.price().get() > previous.get() {
                up = up.checked_add(1).ok_or_else(|| {
                    AnalysisError::InvalidInput("breadth up count overflow".into())
                })?;
                if limit_up.contains(quote.instrument()) {
                    limit_up_count = limit_up_count.checked_add(1).ok_or_else(|| {
                        AnalysisError::InvalidInput("breadth limit-up count overflow".into())
                    })?;
                }
                if limit_down.contains(quote.instrument()) {
                    return Err(AnalysisError::InvalidInput(format!(
                        "limit-down instrument {} has an advancing quote",
                        quote.instrument().code()
                    )));
                }
            } else if quote.price().get() < previous.get() {
                down = down.checked_add(1).ok_or_else(|| {
                    AnalysisError::InvalidInput("breadth down count overflow".into())
                })?;
                if limit_down.contains(quote.instrument()) {
                    limit_down_count = limit_down_count.checked_add(1).ok_or_else(|| {
                        AnalysisError::InvalidInput("breadth limit-down count overflow".into())
                    })?;
                }
                if limit_up.contains(quote.instrument()) {
                    return Err(AnalysisError::InvalidInput(format!(
                        "limit-up instrument {} has a declining quote",
                        quote.instrument().code()
                    )));
                }
            } else {
                flat = flat.checked_add(1).ok_or_else(|| {
                    AnalysisError::InvalidInput("breadth flat count overflow".into())
                })?;
                if limit_up.contains(quote.instrument()) || limit_down.contains(quote.instrument())
                {
                    return Err(AnalysisError::InvalidInput(format!(
                        "limit-state instrument {} has an unchanged quote",
                        quote.instrument().code()
                    )));
                }
            }
        }
        if usize::try_from(limit_up_count).ok() != Some(limit_up.len()) {
            return Err(AnalysisError::InvalidInput(
                "every upper-limit pool member must have a valid advancing quote".into(),
            ));
        }
        if usize::try_from(limit_down_count).ok() != Some(limit_down.len()) {
            return Err(AnalysisError::InvalidInput(
                "every lower-limit pool member must have a valid declining quote".into(),
            ));
        }

        let total = u32::try_from(self.universe.members().records().len())
            .map_err(|_| AnalysisError::InvalidInput("breadth total count overflow".into()))?;
        let coverage = valid as f64 / total as f64;
        if coverage < request.minimum_coverage().get() {
            return Err(AnalysisError::InvalidInput(format!(
                "breadth coverage {coverage:.6} is below requested {:.6}",
                request.minimum_coverage().get()
            )));
        }

        let batch_id = format!(
            "local-breadth:v1:universe={}:{}",
            self.universe.version(),
            input_evidence
                .iter()
                .map(|evidence| format!("{:?}/{}", evidence.provider(), evidence.batch_id()))
                .collect::<Vec<_>>()
                .join(",")
        );
        let evidence = SourceEvidence::new(
            ProviderId::LocalAnalysis,
            derived_observed_at.clone(),
            batch_id.clone(),
        )?
        .with_source_at(conservative_source_at.clone())?;
        let snapshot = MarketBreadthSnapshot::new(
            self.universe.name().clone(),
            request.source_date().clone(),
            request.source_session(),
            total,
            valid,
            up,
            down,
            flat,
            limit_up_count,
            limit_down_count,
            source_skew,
            input_evidence,
            evidence,
        )?;
        let provenance = Provenance::new("local-analysis", derived_observed_at)?
            .with_source_at(conservative_source_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::strict(vec![snapshot], provenance))
    }
}

struct QuoteEvidence {
    input_evidence: Vec<SourceEvidence>,
    earliest: EvidenceTimestamp,
    latest: EvidenceTimestamp,
    conservative_source_at: String,
    source_date: magic_market_core::IsoDate,
    source_session: MarketSession,
}

fn quote_evidence(quotes: &DataBatch<Quote>) -> Result<QuoteEvidence, AnalysisError> {
    let provenance = quotes.provenance();
    let provenance_batch = provenance.batch_id().ok_or_else(|| {
        AnalysisError::InvalidInput("breadth quote batch provenance is missing batch_id".into())
    })?;
    let observed = EvidenceTimestamp::parse_instant(provenance.fetched_at())?;
    let mut times = Vec::with_capacity(quotes.records().len());
    let mut provider = None;
    let mut source_date = None;
    let mut source_session = None;
    for quote in quotes.records() {
        match provider {
            Some(expected) if expected != quote.provider() => {
                return Err(AnalysisError::InvalidInput(
                    "breadth quote batch contains mixed providers".into(),
                ))
            }
            None => provider = Some(quote.provider()),
            _ => {}
        }
        if quote.batch_id() != provenance_batch {
            return Err(AnalysisError::InvalidInput(format!(
                "breadth quote {} batch_id does not match batch provenance",
                quote.instrument().code()
            )));
        }
        if quote.observed_at() != provenance.fetched_at() {
            return Err(AnalysisError::InvalidInput(format!(
                "breadth quote {} observed_at does not match batch provenance",
                quote.instrument().code()
            )));
        }
        let quote_observed = EvidenceTimestamp::parse_instant(quote.observed_at())?;
        if quote_observed != observed {
            return Err(AnalysisError::InvalidInput(format!(
                "breadth quote {} observed instant does not match batch provenance",
                quote.instrument().code()
            )));
        }
        let source_at = quote.source_at().ok_or_else(|| {
            AnalysisError::InvalidInput(format!(
                "breadth quote {} is missing source_at",
                quote.instrument().code()
            ))
        })?;
        let timestamp = EvidenceTimestamp::parse_instant(source_at)?;
        if observed.duration_since(timestamp).is_none() {
            return Err(AnalysisError::InvalidInput(format!(
                "breadth quote {} source_at is later than observed_at",
                quote.instrument().code()
            )));
        }
        let (date, session) = china_source_identity(source_at)?;
        match &source_date {
            Some(expected) if expected != &date => {
                return Err(AnalysisError::InvalidInput(
                    "breadth quote batch spans multiple source dates".into(),
                ))
            }
            None => source_date = Some(date),
            _ => {}
        }
        match source_session {
            Some(expected) if expected != session => {
                return Err(AnalysisError::InvalidInput(
                    "breadth quote batch spans multiple market sessions".into(),
                ))
            }
            None => source_session = Some(session),
            _ => {}
        }
        times.push((timestamp, source_at.to_owned()));
    }
    let provider =
        provider.ok_or_else(|| AnalysisError::InvalidInput("breadth has no provider".into()))?;
    times.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let (earliest, oldest_source_at) = times
        .first()
        .ok_or_else(|| AnalysisError::InvalidInput("breadth has no source times".into()))?;
    let (latest, _) = times
        .last()
        .ok_or_else(|| AnalysisError::InvalidInput("breadth has no source times".into()))?;
    if provenance.source_at() != Some(oldest_source_at.as_str()) {
        return Err(AnalysisError::InvalidInput(
            "breadth batch provenance source_at must equal the oldest record source_at".into(),
        ));
    }
    let input_evidence =
        vec![
            SourceEvidence::new(provider, provenance.fetched_at(), provenance_batch)?
                .with_source_at(oldest_source_at)?,
        ];
    Ok(QuoteEvidence {
        input_evidence,
        earliest: *earliest,
        latest: *latest,
        conservative_source_at: oldest_source_at.clone(),
        source_date: source_date
            .ok_or_else(|| AnalysisError::InvalidInput("breadth source date absent".into()))?,
        source_session: source_session
            .ok_or_else(|| AnalysisError::InvalidInput("breadth source session absent".into()))?,
    })
}

fn latest_input_observed_at(input_evidence: &[SourceEvidence]) -> Result<String, AnalysisError> {
    input_evidence
        .iter()
        .map(|evidence| {
            EvidenceTimestamp::parse_instant(evidence.observed_at())
                .map(|timestamp| (timestamp, evidence.observed_at()))
                .map_err(AnalysisError::from)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(right.1)))
        .map(|(_, observed_at)| observed_at.to_owned())
        .ok_or_else(|| AnalysisError::InvalidInput("breadth has no input observation".into()))
}

fn china_source_identity(
    source_at: &str,
) -> Result<(magic_market_core::IsoDate, MarketSession), AnalysisError> {
    let suffix = source_at.get(19..).unwrap_or_default();
    let has_china_offset = suffix == "+08:00"
        || suffix
            .strip_prefix('.')
            .and_then(|fractional| fractional.strip_suffix("+08:00"))
            .is_some_and(|fraction| {
                !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
            });
    if !has_china_offset || !matches!(source_at.as_bytes().get(10), Some(b'T') | Some(b' ')) {
        return Err(AnalysisError::InvalidInput(format!(
            "breadth source_at {source_at:?} must be an explicit China +08:00 timestamp"
        )));
    }
    let date = magic_market_core::IsoDate::new(
        source_at
            .get(..10)
            .ok_or_else(|| AnalysisError::InvalidInput("breadth source date absent".into()))?,
    )?;
    let time = source_at
        .get(11..19)
        .ok_or_else(|| AnalysisError::InvalidInput("breadth source clock absent".into()))?;
    let session = if time < "09:15:00" {
        MarketSession::PreOpen
    } else if time < "09:30:00" {
        MarketSession::OpeningAuction
    } else if time <= "11:30:00" {
        MarketSession::Continuous
    } else if time < "13:00:00" {
        MarketSession::LunchBreak
    } else if time < "15:00:00" {
        MarketSession::Continuous
    } else if time == "15:00:00" {
        MarketSession::Close
    } else {
        MarketSession::PostClose
    };
    Ok((date, session))
}
