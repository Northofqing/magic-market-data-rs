use crate::{FailureAction, FailureKind, RoutedSource, SourceError};
use magic_market_core::{DataBatch, EvidenceTimestamp, ProviderId, SourcedRecord};
use std::time::Duration;
use thiserror::Error;

/// Minimum batch evidence required before one source can be selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AcceptancePolicy {
    require_complete: bool,
    require_source_at: bool,
    accept_complete_empty: bool,
    max_source_age: Option<Duration>,
}

impl AcceptancePolicy {
    pub const fn new() -> Self {
        Self {
            require_complete: false,
            require_source_at: false,
            accept_complete_empty: false,
            max_source_age: None,
        }
    }

    pub const fn with_require_complete(mut self, required: bool) -> Self {
        self.require_complete = required;
        self
    }

    pub const fn with_require_source_at(mut self, required: bool) -> Self {
        self.require_source_at = required || self.max_source_age.is_some();
        self
    }

    pub const fn require_complete(self) -> bool {
        self.require_complete
    }

    pub const fn require_source_at(self) -> bool {
        self.require_source_at
    }

    /// Requires record and batch provider times no older than `maximum`.
    ///
    /// A freshness bound inherently requires `source_at`; it never falls back to
    /// local observation time when source time is absent.
    pub fn with_max_source_age(mut self, maximum: Duration) -> Result<Self, RouterError> {
        if maximum.is_zero() {
            return Err(RouterError::InvalidConfiguration(
                "maximum source age must be positive".into(),
            ));
        }
        self.require_source_at = true;
        self.max_source_age = Some(maximum);
        Ok(self)
    }

    pub const fn max_source_age(self) -> Option<Duration> {
        self.max_source_age
    }

    /// Allows a source contract to select a complete, evidence-bearing empty batch.
    ///
    /// This is default-off because most routes use an empty batch as a failover
    /// signal. Providers opting in must prove their empty semantics separately.
    pub const fn with_accept_complete_empty(mut self, accepted: bool) -> Self {
        self.accept_complete_empty = accepted;
        self
    }

    pub const fn accept_complete_empty(self) -> bool {
        self.accept_complete_empty
    }
}

/// Disposition of one provider in an ordered route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptStatus {
    Failed {
        kind: FailureKind,
        action: FailureAction,
        message: String,
    },
    Rejected {
        kind: FailureKind,
        message: String,
    },
    Selected,
}

/// Auditable result of one provider attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteAttempt {
    provider_id: ProviderId,
    status: AttemptStatus,
}

impl RouteAttempt {
    pub fn provider_id(&self) -> ProviderId {
        self.provider_id
    }

    pub fn status(&self) -> &AttemptStatus {
        &self.status
    }

    fn failed(provider_id: ProviderId, error: &SourceError) -> Self {
        Self {
            provider_id,
            status: AttemptStatus::Failed {
                kind: error.kind(),
                action: error.action(),
                message: error.message().to_owned(),
            },
        }
    }

    fn rejected(provider_id: ProviderId, kind: FailureKind, message: String) -> Self {
        Self {
            provider_id,
            status: AttemptStatus::Rejected { kind, message },
        }
    }

    fn selected(provider_id: ProviderId) -> Self {
        Self {
            provider_id,
            status: AttemptStatus::Selected,
        }
    }
}

/// Terminal routing failures with the complete ordered attempt trace.
#[derive(Debug, Error)]
pub enum RouterError {
    #[error("invalid router configuration: {0}")]
    InvalidConfiguration(String),
    #[error("routing stopped by a terminal source failure")]
    Stopped { attempts: Vec<RouteAttempt> },
    #[error("all registered market-data sources were exhausted")]
    Exhausted { attempts: Vec<RouteAttempt> },
}

impl RouterError {
    pub fn attempts(&self) -> &[RouteAttempt] {
        match self {
            Self::InvalidConfiguration(_) => &[],
            Self::Stopped { attempts } | Self::Exhausted { attempts } => attempts,
        }
    }
}

/// Accepted provider batch together with every preceding attempt.
#[derive(Debug)]
pub struct RouteOutcome<Record> {
    selected_provider: ProviderId,
    batch: DataBatch<Record>,
    attempts: Vec<RouteAttempt>,
}

impl<Record> RouteOutcome<Record> {
    pub fn selected_provider(&self) -> ProviderId {
        self.selected_provider
    }

    pub fn batch(&self) -> &DataBatch<Record> {
        &self.batch
    }

    pub fn attempts(&self) -> &[RouteAttempt] {
        &self.attempts
    }

    pub fn into_batch(self) -> DataBatch<Record> {
        self.batch
    }

    pub fn into_parts(self) -> (DataBatch<Record>, Vec<RouteAttempt>) {
        (self.batch, self.attempts)
    }
}

/// Ordered first-acceptable-batch routing for one request/record family.
pub struct FailoverChain<Request: ?Sized, Record> {
    policy: AcceptancePolicy,
    sources: Vec<Box<dyn RoutedSource<Request, Record>>>,
}

impl<Request: ?Sized, Record> std::fmt::Debug for FailoverChain<Request, Record> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FailoverChain")
            .field("policy", &self.policy)
            .field("source_count", &self.sources.len())
            .finish()
    }
}

impl<Request: ?Sized, Record> FailoverChain<Request, Record> {
    pub fn new(policy: AcceptancePolicy) -> Self {
        Self {
            policy,
            sources: Vec::new(),
        }
    }

    pub fn policy(&self) -> AcceptancePolicy {
        self.policy
    }

    pub fn register<Source>(&mut self, source: Source) -> Result<&mut Self, RouterError>
    where
        Source: RoutedSource<Request, Record> + 'static,
    {
        let provider_id = source.provider_id();
        if self
            .sources
            .iter()
            .any(|registered| registered.provider_id() == provider_id)
        {
            return Err(RouterError::InvalidConfiguration(format!(
                "provider {provider_id:?} is already registered"
            )));
        }
        self.sources.push(Box::new(source));
        Ok(self)
    }

    pub fn provider_ids(&self) -> Vec<ProviderId> {
        self.sources
            .iter()
            .map(|source| source.provider_id())
            .collect()
    }
}

impl<Request: ?Sized, Record: SourcedRecord> FailoverChain<Request, Record> {
    pub fn route(&self, request: &Request) -> Result<RouteOutcome<Record>, RouterError> {
        if self.sources.is_empty() {
            return Err(RouterError::InvalidConfiguration(
                "at least one source must be registered".into(),
            ));
        }

        let mut attempts = Vec::with_capacity(self.sources.len());
        for source in &self.sources {
            let provider_id = source.provider_id();
            let batch = match source.fetch(request) {
                Ok(batch) => batch,
                Err(error) => {
                    attempts.push(RouteAttempt::failed(provider_id, &error));
                    if error.action() == FailureAction::Stop {
                        return Err(RouterError::Stopped { attempts });
                    }
                    continue;
                }
            };

            if let Some((kind, message)) = rejected_batch(self.policy, provider_id, &batch) {
                attempts.push(RouteAttempt::rejected(provider_id, kind, message));
                continue;
            }

            attempts.push(RouteAttempt::selected(provider_id));
            return Ok(RouteOutcome {
                selected_provider: provider_id,
                batch,
                attempts,
            });
        }

        Err(RouterError::Exhausted { attempts })
    }
}

fn rejected_batch<Record: SourcedRecord>(
    policy: AcceptancePolicy,
    provider_id: ProviderId,
    batch: &DataBatch<Record>,
) -> Option<(FailureKind, String)> {
    if batch.records().is_empty() && !policy.accept_complete_empty() {
        return Some((
            FailureKind::NoData,
            "provider returned an empty successful batch".into(),
        ));
    }
    if policy.require_complete() && !batch.quality().is_complete() {
        return Some((
            FailureKind::Quality,
            format!(
                "batch quality is incomplete: {}",
                batch.quality().issues().join("; ")
            ),
        ));
    }
    if policy.require_source_at() && batch.provenance().source_at().is_none() {
        return Some((
            FailureKind::Quality,
            "batch source timestamp is unavailable".into(),
        ));
    }
    let Some(batch_id) = batch.provenance().batch_id() else {
        return Some((
            FailureKind::Evidence,
            "batch provenance has no batch ID".into(),
        ));
    };
    if let Some(record) = batch
        .records()
        .iter()
        .find(|record| record.provider_id() != provider_id)
    {
        return Some((
            FailureKind::Evidence,
            format!(
                "record provider {:?} does not match registered provider {provider_id:?}",
                record.provider_id()
            ),
        ));
    }
    if let Some(record) = batch
        .records()
        .iter()
        .find(|record| record.evidence_batch_id() != batch_id)
    {
        return Some((
            FailureKind::Evidence,
            format!(
                "record batch ID {:?} does not match provenance batch ID {batch_id:?}",
                record.evidence_batch_id()
            ),
        ));
    }
    if let Some(maximum) = policy.max_source_age() {
        if let Some(rejection) = freshness_rejection(batch, maximum) {
            return Some(rejection);
        }
    }
    None
}

fn freshness_rejection<Record: SourcedRecord>(
    batch: &DataBatch<Record>,
    maximum: Duration,
) -> Option<(FailureKind, String)> {
    let observed_at = batch.provenance().fetched_at();
    let observed_time = match EvidenceTimestamp::parse_instant(observed_at) {
        Ok(value) => value,
        Err(_) => {
            return Some((
                FailureKind::Evidence,
                "batch observation timestamp is malformed".into(),
            ));
        }
    };

    let mut oldest_record: Option<EvidenceTimestamp> = None;
    for record in batch.records() {
        if record.evidence_observed_at() != Some(observed_at) {
            return Some((
                FailureKind::Evidence,
                "record observed timestamp does not match batch observation timestamp".into(),
            ));
        }
        let Some(source_at) = record.evidence_source_at() else {
            return Some((
                FailureKind::Evidence,
                "record source timestamp is unavailable".into(),
            ));
        };
        let source_time = match EvidenceTimestamp::parse_instant(source_at) {
            Ok(value) => value,
            Err(_) => {
                return Some((
                    FailureKind::Evidence,
                    "record source timestamp is malformed".into(),
                ));
            }
        };
        if observed_time.duration_since(source_time).is_none() {
            return Some((
                FailureKind::Evidence,
                "record source timestamp is later than its observed timestamp".into(),
            ));
        }
        oldest_record = Some(oldest_record.map_or(source_time, |oldest| oldest.min(source_time)));
    }

    let Some(batch_source_at) = batch.provenance().source_at() else {
        return Some((
            FailureKind::Evidence,
            "batch source timestamp is unavailable".into(),
        ));
    };
    let batch_source_time = match EvidenceTimestamp::parse_instant(batch_source_at) {
        Ok(value) => value,
        Err(_) => {
            return Some((
                FailureKind::Evidence,
                "batch source timestamp is malformed".into(),
            ));
        }
    };
    if let Some(oldest) = oldest_record {
        if batch_source_time != oldest {
            return Some((
                FailureKind::Evidence,
                "batch source timestamp does not equal the oldest record source timestamp".into(),
            ));
        }
    }
    let Some(age) = observed_time.duration_since(batch_source_time) else {
        return Some((
            FailureKind::Evidence,
            "batch source timestamp is later than its observation timestamp".into(),
        ));
    };
    if age > maximum {
        return Some((
            FailureKind::Quality,
            format!(
                "batch source timestamp is stale by {}; maximum allowed age is {}",
                display_duration(age),
                display_duration(maximum)
            ),
        ));
    }
    None
}

fn display_duration(duration: Duration) -> String {
    let nanos = duration.subsec_nanos();
    if nanos == 0 {
        format!("{}s", duration.as_secs())
    } else {
        let fraction = format!("{nanos:09}");
        format!("{}.{}s", duration.as_secs(), fraction.trim_end_matches('0'))
    }
}
