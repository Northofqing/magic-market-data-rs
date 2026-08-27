#![forbid(unsafe_code)]
//! Transport-neutral external query facade.

use std::collections::BTreeMap;
use std::sync::Arc;

use thiserror::Error;

macro_rules! define_operations {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum Operation {
            $($variant),+
        }

        pub const ALL_OPERATIONS: &[Operation] = &[
            $(Operation::$variant),+
        ];

        impl Operation {
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }
        }
    };
}

define_operations! {
    HistoricalBars => "historical_bars",
    MinuteData => "minute_data",
    RealtimeQuotes => "realtime_quotes",
    MoneyFlows => "money_flows",
    OrderBooks => "order_books",
    Auctions => "auctions",
    Trades => "trades",
    SecurityMetadata => "security_metadata",
    GlobalIndices => "global_indices",
    ForeignExchange => "foreign_exchange",
    EconomicCalendar => "economic_calendar",
    FuturesDelivery => "futures_delivery",
    ReferenceRates => "reference_rates",
    OfficialFxFixings => "official_fx_fixings",
    EconomicSeries => "economic_series",
    CompanyFilings => "company_filings",
    GlobalNews => "global_news",
    Announcements => "announcements",
    MarketAnnouncements => "market_announcements",
    InvestorQuestions => "investor_questions",
    PolicyDocuments => "policy_documents",
    SecurityProfiles => "security_profiles",
    FinancialStatements => "financial_statements",
    MarketStatistics => "market_statistics",
    TechnicalBars => "technical_bars",
    CorporateActions => "corporate_actions",
    BoardDirectory => "board_directory",
    BoardConstituents => "board_constituents",
    BoardMemberships => "board_memberships",
    ResearchReports => "research_reports",
    ResearchDocuments => "research_documents",
    Consensus => "consensus",
    TargetPrices => "target_prices",
    SemanticSearch => "semantic_search",
    FundFlowSeries => "fund_flow_series",
    BoardFlows => "board_flows",
    MarginData => "margin_data",
    BlockTrades => "block_trades",
    HolderCounts => "holder_counts",
    LockupEvents => "lockup_events",
    DividendPlans => "dividend_plans",
    PostCloseFlows => "post_close_flows",
    NorthboundDaily => "northbound_daily",
    LimitPools => "limit_pools",
    StrongStockReasons => "strong_stock_reasons",
    DragonTiger => "dragon_tiger",
    MarketDragonTiger => "market_dragon_tiger",
    DragonTigerDiscovery => "dragon_tiger_discovery",
    MarketRankings => "market_rankings",
    MarketBreadth => "market_breadth",
    Popularity => "popularity",
    ConceptHits => "concept_hits",
    OptionData => "option_data",
    ProviderTopNRankings => "provider_top_n_rankings",
    InstrumentNews => "instrument_news",
    IndexQuotes => "index_quotes",
    IntradayShape => "intraday_shape",
    T0Evidence => "t0_evidence",
    OutcomeDailyBars => "outcome_daily_bars",
    UpperLimitPoolReview => "upper_limit_pool_review",
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPayload {
    schema: String,
    schema_version: u32,
    data: Vec<u8>,
}

impl CanonicalPayload {
    pub fn new(
        schema: impl Into<String>,
        schema_version: u32,
        data: Vec<u8>,
        maximum_bytes: usize,
    ) -> Result<Self, ServiceError> {
        let schema = schema.into();
        if schema.trim().is_empty() {
            return Err(ServiceError::InvalidRequest(
                "payload schema is required".to_owned(),
            ));
        }
        if schema_version == 0 {
            return Err(ServiceError::InvalidRequest(
                "payload schema version must be positive".to_owned(),
            ));
        }
        if data.is_empty() {
            return Err(ServiceError::InvalidRequest(
                "payload data is required".to_owned(),
            ));
        }
        if maximum_bytes == 0 || data.len() > maximum_bytes {
            return Err(ServiceError::ResourceExhausted(format!(
                "payload size {} exceeds maximum {maximum_bytes}",
                data.len()
            )));
        }
        serde_json::from_slice::<serde_json::Value>(&data).map_err(|error| {
            ServiceError::InvalidRequest(format!("payload is not valid JSON: {error}"))
        })?;
        Ok(Self {
            schema,
            schema_version,
            data,
        })
    }

    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryCommand {
    request_id: String,
    operation: Operation,
    preferred_provider: Option<String>,
    payload: CanonicalPayload,
    allow_unadmitted: bool,
}

impl QueryCommand {
    pub fn new(
        request_id: impl Into<String>,
        operation: Operation,
        preferred_provider: Option<String>,
        payload: CanonicalPayload,
    ) -> Result<Self, ServiceError> {
        let request_id = request_id.into();
        if request_id.trim().is_empty() {
            return Err(ServiceError::InvalidRequest(
                "request_id is required".to_owned(),
            ));
        }
        let preferred_provider = preferred_provider
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        Ok(Self {
            request_id,
            operation,
            preferred_provider,
            payload,
            allow_unadmitted: false,
        })
    }

    #[must_use]
    pub const fn with_unadmitted_access(mut self, allow_unadmitted: bool) -> Self {
        self.allow_unadmitted = allow_unadmitted;
        self
    }

    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    #[must_use]
    pub fn preferred_provider(&self) -> Option<&str> {
        self.preferred_provider.as_deref()
    }

    #[must_use]
    pub const fn payload(&self) -> &CanonicalPayload {
        &self.payload
    }

    #[must_use]
    pub const fn allows_unadmitted(&self) -> bool {
        self.allow_unadmitted
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryResult {
    pub provider: String,
    pub batch_id: String,
    pub complete: bool,
    pub observed_at: String,
    pub source_at: Option<String>,
    pub records: Vec<CanonicalPayload>,
    pub repository_admitted: bool,
    pub diagnostic_blocker: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capability {
    pub operation: Operation,
    pub repository_admitted: bool,
    pub runtime_available: bool,
    pub provider: String,
    pub exact_scope: String,
    pub blocker: Option<String>,
    pub diagnostic_available: bool,
}

type Handler = Arc<dyn Fn(QueryCommand) -> Result<QueryResult, ServiceError> + Send + Sync>;

#[derive(Clone)]
struct Registration {
    capability: Capability,
    handler: Option<Handler>,
}

#[derive(Clone)]
pub struct OperationRegistry {
    registrations: BTreeMap<Operation, Vec<Registration>>,
}

impl OperationRegistry {
    #[must_use]
    pub fn all_unadmitted(blocker: impl Into<String>) -> Self {
        let blocker = blocker.into();
        let registrations = ALL_OPERATIONS
            .iter()
            .copied()
            .map(|operation| {
                (
                    operation,
                    vec![Registration {
                        capability: Capability {
                            operation,
                            repository_admitted: false,
                            runtime_available: false,
                            provider: String::new(),
                            exact_scope: String::new(),
                            blocker: Some(blocker.clone()),
                            diagnostic_available: false,
                        },
                        handler: None,
                    }],
                )
            })
            .collect();
        Self { registrations }
    }

    pub fn register_unavailable(&mut self, capability: Capability) -> Result<(), ServiceError> {
        validate_capability(&capability, false)?;
        self.insert_registration(Registration {
            capability,
            handler: None,
        });
        Ok(())
    }

    pub fn register_diagnostic_handler<F>(
        &mut self,
        mut capability: Capability,
        handler: F,
    ) -> Result<(), ServiceError>
    where
        F: Fn(QueryCommand) -> Result<QueryResult, ServiceError> + Send + Sync + 'static,
    {
        capability.diagnostic_available = true;
        validate_diagnostic_capability(&capability)?;
        self.insert_registration(Registration {
            capability,
            handler: Some(Arc::new(handler)),
        });
        Ok(())
    }

    pub fn register_handler<F>(
        &mut self,
        capability: Capability,
        handler: F,
    ) -> Result<(), ServiceError>
    where
        F: Fn(QueryCommand) -> Result<QueryResult, ServiceError> + Send + Sync + 'static,
    {
        validate_capability(&capability, true)?;
        self.insert_registration(Registration {
            capability,
            handler: Some(Arc::new(handler)),
        });
        Ok(())
    }

    fn insert_registration(&mut self, registration: Registration) {
        let operation = registration.capability.operation;
        let provider = registration.capability.provider.clone();
        let registrations = self.registrations.entry(operation).or_default();
        registrations.retain(|existing| {
            !existing.capability.provider.is_empty() && existing.capability.provider != provider
        });
        registrations.push(registration);
    }

    #[must_use]
    pub fn capabilities(&self) -> Vec<Capability> {
        self.registrations
            .values()
            .flatten()
            .map(|registration| registration.capability.clone())
            .collect()
    }

    pub fn execute(&self, command: QueryCommand) -> Result<QueryResult, ServiceError> {
        let registrations = self
            .registrations
            .get(&command.operation)
            .ok_or_else(|| ServiceError::Internal("operation registry is incomplete".to_owned()))?;
        let registration = if let Some(preferred) = command.preferred_provider() {
            registrations
                .iter()
                .find(|registration| registration.capability.provider == preferred)
                .ok_or_else(|| ServiceError::Unsupported {
                    operation: command.operation,
                    reason: format!("provider {preferred} is not registered for this operation"),
                })?
        } else {
            registrations
                .iter()
                .find(|registration| {
                    registration.capability.repository_admitted
                        && registration.capability.runtime_available
                        && registration.handler.is_some()
                })
                .or_else(|| registrations.first())
                .ok_or_else(|| {
                    ServiceError::Internal("operation has no registrations".to_owned())
                })?
        };
        let capability = &registration.capability;
        if !capability.repository_admitted && !command.allows_unadmitted() {
            return Err(ServiceError::Unsupported {
                operation: command.operation,
                reason: capability
                    .blocker
                    .clone()
                    .unwrap_or_else(|| "repository capability is unadmitted".to_owned()),
            });
        }
        if !capability.repository_admitted && !capability.diagnostic_available {
            return Err(ServiceError::Unsupported {
                operation: command.operation,
                reason: capability.blocker.clone().unwrap_or_else(|| {
                    "repository capability has no diagnostic handler".to_owned()
                }),
            });
        }
        if capability.repository_admitted && !capability.runtime_available {
            return Err(ServiceError::Unavailable {
                operation: command.operation,
                reason: capability
                    .blocker
                    .clone()
                    .unwrap_or_else(|| "runtime capability is unavailable".to_owned()),
            });
        }
        let handler = registration.handler.as_ref().ok_or_else(|| {
            ServiceError::Internal("admitted runtime capability has no handler".to_owned())
        })?;
        let mut result = handler(command)?;
        result.repository_admitted = capability.repository_admitted;
        result.diagnostic_blocker = if capability.repository_admitted {
            None
        } else {
            capability.blocker.clone()
        };
        if !capability.repository_admitted {
            result.complete = false;
        }
        Ok(result)
    }
}

fn validate_capability(
    capability: &Capability,
    requires_handler: bool,
) -> Result<(), ServiceError> {
    if capability.provider.trim().is_empty() && (capability.repository_admitted || requires_handler)
    {
        return Err(ServiceError::InvalidRequest(
            "admitted capability provider is required".to_owned(),
        ));
    }
    if capability.exact_scope.trim().is_empty()
        && (capability.repository_admitted || requires_handler)
    {
        return Err(ServiceError::InvalidRequest(
            "admitted capability exact scope is required".to_owned(),
        ));
    }
    if capability.runtime_available && !capability.repository_admitted {
        return Err(ServiceError::InvalidRequest(
            "runtime availability cannot promote repository admission".to_owned(),
        ));
    }
    if requires_handler && !(capability.repository_admitted && capability.runtime_available) {
        return Err(ServiceError::InvalidRequest(
            "handler requires admitted and runtime-available capability".to_owned(),
        ));
    }
    Ok(())
}

fn validate_diagnostic_capability(capability: &Capability) -> Result<(), ServiceError> {
    validate_capability(capability, false)?;
    if capability.repository_admitted || capability.runtime_available {
        return Err(ServiceError::InvalidRequest(
            "diagnostic handler must remain repository-unadmitted and runtime-unavailable"
                .to_owned(),
        ));
    }
    if capability.provider.trim().is_empty()
        || capability.exact_scope.trim().is_empty()
        || capability.blocker.as_deref().is_none_or(str::is_empty)
    {
        return Err(ServiceError::InvalidRequest(
            "diagnostic handler requires provider, exact scope and blocker".to_owned(),
        ));
    }
    Ok(())
}

pub trait BlockingQueryGateway: Send + Sync + 'static {
    fn capabilities(&self) -> Vec<Capability>;
    fn execute(&self, command: QueryCommand) -> Result<QueryResult, ServiceError>;
}

impl BlockingQueryGateway for OperationRegistry {
    fn capabilities(&self) -> Vec<Capability> {
        Self::capabilities(self)
    }

    fn execute(&self, command: QueryCommand) -> Result<QueryResult, ServiceError> {
        Self::execute(self, command)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderFailureKind {
    AuthenticationRejected,
    RateLimited,
    QueryRejected,
    ResponseInvalid,
    Unavailable,
}

/// One bounded, client-safe step in an ordered provider route failure.
///
/// Free-form upstream messages deliberately remain outside this type.  The
/// external boundary may persist these values without leaking credentials or
/// provider response text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAttempt {
    provider: String,
    outcome: String,
    reason_code: String,
    retryable: bool,
    terminal: bool,
}

impl ProviderAttempt {
    pub fn new(
        provider: impl Into<String>,
        outcome: impl Into<String>,
        reason_code: impl Into<String>,
        retryable: bool,
        terminal: bool,
    ) -> Result<Self, ServiceError> {
        let provider = provider.into();
        let outcome = outcome.into();
        let reason_code = reason_code.into();
        if provider.trim() != provider
            || provider.is_empty()
            || provider.len() > 64
            || provider.chars().any(char::is_control)
        {
            return Err(ServiceError::Internal(
                "provider attempt identity is invalid".to_owned(),
            ));
        }
        if !matches!(outcome.as_str(), "failed" | "rejected" | "selected") {
            return Err(ServiceError::Internal(
                "provider attempt outcome is invalid".to_owned(),
            ));
        }
        if reason_code.is_empty()
            || reason_code.len() > 64
            || !reason_code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        {
            return Err(ServiceError::Internal(
                "provider attempt reason code is invalid".to_owned(),
            ));
        }
        Ok(Self {
            provider,
            outcome,
            reason_code,
            retryable,
            terminal,
        })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn outcome(&self) -> &str {
        &self.outcome
    }

    pub fn reason_code(&self) -> &str {
        &self.reason_code
    }

    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    pub const fn terminal(&self) -> bool {
        self.terminal
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ServiceError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("operation {operation:?} is unsupported: {reason}")]
    Unsupported {
        operation: Operation,
        reason: String,
    },
    #[error("authentication failed")]
    Unauthenticated,
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("resource exhausted: {0}")]
    ResourceExhausted(String),
    #[error("deadline exceeded: {0}")]
    DeadlineExceeded(String),
    #[error("operation {operation:?} is unavailable: {reason}")]
    Unavailable {
        operation: Operation,
        reason: String,
    },
    #[error("{provider} provider failure for operation {operation:?}: {kind:?}")]
    ProviderFailure {
        operation: Operation,
        provider: String,
        kind: ProviderFailureKind,
        provider_reason: String,
    },
    #[error("provider route failed for operation {operation:?}")]
    ProviderRouteFailure {
        operation: Operation,
        exhausted: bool,
        attempts: Vec<ProviderAttempt>,
    },
    #[error("source precondition failed: {0}")]
    FailedPrecondition(String),
    #[error("invalid {provider} evidence ({evidence_code}, {evidence_field}): {message}")]
    InvalidEvidence {
        provider: String,
        evidence_code: String,
        evidence_field: String,
        record_index: Option<u32>,
        message: String,
    },
    #[error("internal service error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn payload() -> CanonicalPayload {
        CanonicalPayload::new("test.request", 1, br#"{"value":1}"#.to_vec(), 1024).unwrap()
    }

    fn command(operation: Operation, provider: Option<&str>) -> QueryCommand {
        QueryCommand::new(
            "request-1",
            operation,
            provider.map(str::to_owned),
            payload(),
        )
        .unwrap()
    }

    #[test]
    fn registry_is_exhaustive_and_unadmitted_fails_before_handler() {
        let registry = OperationRegistry::all_unadmitted("evidence missing");
        assert_eq!(registry.capabilities().len(), ALL_OPERATIONS.len());
        assert!(matches!(
            registry.execute(command(Operation::RealtimeQuotes, None)),
            Err(ServiceError::Unsupported { .. })
        ));
    }

    #[test]
    fn admitted_handler_runs_and_provider_mismatch_does_not() {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let mut registry = OperationRegistry::all_unadmitted("missing");
        registry
            .register_handler(
                Capability {
                    operation: Operation::RealtimeQuotes,
                    repository_admitted: true,
                    runtime_available: true,
                    provider: "Tencent".to_owned(),
                    exact_scope: "A-share equity quote".to_owned(),
                    blocker: None,
                    diagnostic_available: false,
                },
                move |_| {
                    seen.fetch_add(1, Ordering::SeqCst);
                    Ok(QueryResult {
                        provider: "Tencent".to_owned(),
                        batch_id: "batch-1".to_owned(),
                        complete: true,
                        observed_at: "2026-08-13T00:00:00Z".to_owned(),
                        source_at: Some("2026-08-13T00:00:00Z".to_owned()),
                        records: vec![payload()],
                        repository_admitted: true,
                        diagnostic_blocker: None,
                    })
                },
            )
            .unwrap();

        assert!(matches!(
            registry.execute(command(Operation::RealtimeQuotes, Some("Tdx"))),
            Err(ServiceError::Unsupported { .. })
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let result = registry
            .execute(command(Operation::RealtimeQuotes, Some("Tencent")))
            .unwrap();
        assert_eq!(result.provider, "Tencent");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn invalid_json_and_runtime_promotion_are_rejected() {
        assert!(matches!(
            CanonicalPayload::new("x", 1, b"not-json".to_vec(), 100),
            Err(ServiceError::InvalidRequest(_))
        ));
        let mut registry = OperationRegistry::all_unadmitted("missing");
        assert!(matches!(
            registry.register_unavailable(Capability {
                operation: Operation::Trades,
                repository_admitted: false,
                runtime_available: true,
                provider: "Tdx".to_owned(),
                exact_scope: "trades".to_owned(),
                blocker: Some("not admitted".to_owned()),
                diagnostic_available: false,
            }),
            Err(ServiceError::InvalidRequest(_))
        ));
    }

    #[test]
    fn multiple_providers_are_selectable_and_default_is_first_admitted() {
        let mut registry = OperationRegistry::all_unadmitted("missing");
        for provider in ["Tencent", "Sina"] {
            let returned_provider = provider.to_owned();
            registry
                .register_handler(
                    Capability {
                        operation: Operation::RealtimeQuotes,
                        repository_admitted: true,
                        runtime_available: true,
                        provider: provider.to_owned(),
                        exact_scope: "A-share quote".to_owned(),
                        blocker: None,
                        diagnostic_available: false,
                    },
                    move |_| {
                        Ok(QueryResult {
                            provider: returned_provider.clone(),
                            batch_id: "batch-1".to_owned(),
                            complete: true,
                            observed_at: "2026-08-14T00:00:00Z".to_owned(),
                            source_at: None,
                            records: vec![payload()],
                            repository_admitted: true,
                            diagnostic_blocker: None,
                        })
                    },
                )
                .unwrap();
        }

        assert_eq!(
            registry
                .execute(command(Operation::RealtimeQuotes, None))
                .unwrap()
                .provider,
            "Tencent"
        );
        assert_eq!(
            registry
                .execute(command(Operation::RealtimeQuotes, Some("Sina")))
                .unwrap()
                .provider,
            "Sina"
        );
        assert_eq!(
            registry
                .capabilities()
                .iter()
                .filter(|capability| capability.operation == Operation::RealtimeQuotes)
                .count(),
            2
        );
    }

    #[test]
    fn default_skips_diagnostic_and_explicit_diagnostic_never_falls_through() {
        let diagnostic_calls = Arc::new(AtomicUsize::new(0));
        let admitted_calls = Arc::new(AtomicUsize::new(0));
        let seen_diagnostic = diagnostic_calls.clone();
        let seen_admitted = admitted_calls.clone();
        let mut registry = OperationRegistry::all_unadmitted("missing");
        registry
            .register_diagnostic_handler(
                Capability {
                    operation: Operation::RealtimeQuotes,
                    repository_admitted: false,
                    runtime_available: false,
                    provider: "EmQuant".to_owned(),
                    exact_scope: "entitlement-dependent quote diagnostic".to_owned(),
                    blocker: Some("quote entitlement is unproved".to_owned()),
                    diagnostic_available: false,
                },
                move |_| {
                    seen_diagnostic.fetch_add(1, Ordering::SeqCst);
                    Ok(QueryResult {
                        provider: "EmQuant".to_owned(),
                        batch_id: "emquant-diagnostic".to_owned(),
                        complete: true,
                        observed_at: "2026-08-22T00:00:00Z".to_owned(),
                        source_at: None,
                        records: vec![payload()],
                        repository_admitted: true,
                        diagnostic_blocker: None,
                    })
                },
            )
            .unwrap();
        registry
            .register_handler(
                Capability {
                    operation: Operation::RealtimeQuotes,
                    repository_admitted: true,
                    runtime_available: true,
                    provider: "Tencent".to_owned(),
                    exact_scope: "A-share quote".to_owned(),
                    blocker: None,
                    diagnostic_available: false,
                },
                move |_| {
                    seen_admitted.fetch_add(1, Ordering::SeqCst);
                    Ok(QueryResult {
                        provider: "Tencent".to_owned(),
                        batch_id: "tencent-admitted".to_owned(),
                        complete: true,
                        observed_at: "2026-08-22T00:00:00Z".to_owned(),
                        source_at: Some("2026-08-22T00:00:00Z".to_owned()),
                        records: vec![payload()],
                        repository_admitted: true,
                        diagnostic_blocker: None,
                    })
                },
            )
            .unwrap();

        let default_result = registry
            .execute(command(Operation::RealtimeQuotes, None))
            .unwrap();
        assert_eq!(default_result.provider, "Tencent");
        assert_eq!(diagnostic_calls.load(Ordering::SeqCst), 0);
        assert_eq!(admitted_calls.load(Ordering::SeqCst), 1);

        let diagnostic = command(Operation::RealtimeQuotes, Some("EmQuant"));
        assert!(matches!(
            registry.execute(diagnostic.clone()),
            Err(ServiceError::Unsupported { .. })
        ));
        assert_eq!(diagnostic_calls.load(Ordering::SeqCst), 0);
        assert_eq!(admitted_calls.load(Ordering::SeqCst), 1);

        let diagnostic_result = registry
            .execute(diagnostic.with_unadmitted_access(true))
            .unwrap();
        assert_eq!(diagnostic_result.provider, "EmQuant");
        assert!(!diagnostic_result.repository_admitted);
        assert!(!diagnostic_result.complete);
        assert_eq!(diagnostic_calls.load(Ordering::SeqCst), 1);
        assert_eq!(admitted_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn diagnostic_handler_requires_explicit_opt_in_and_never_promotes_admission() {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let mut registry = OperationRegistry::all_unadmitted("missing");
        registry
            .register_diagnostic_handler(
                Capability {
                    operation: Operation::TechnicalBars,
                    repository_admitted: false,
                    runtime_available: false,
                    provider: "Baidu".to_owned(),
                    exact_scope: "diagnostic daily technical bars".to_owned(),
                    blocker: Some("continuity evidence missing".to_owned()),
                    diagnostic_available: false,
                },
                move |_| {
                    seen.fetch_add(1, Ordering::SeqCst);
                    Ok(QueryResult {
                        provider: "Baidu".to_owned(),
                        batch_id: "batch-diagnostic".to_owned(),
                        complete: true,
                        observed_at: "2026-08-15T00:00:00Z".to_owned(),
                        source_at: Some("2026-08-14".to_owned()),
                        records: vec![payload()],
                        repository_admitted: true,
                        diagnostic_blocker: None,
                    })
                },
            )
            .unwrap();

        let command = command(Operation::TechnicalBars, Some("Baidu"));
        assert!(matches!(
            registry.execute(command.clone()),
            Err(ServiceError::Unsupported { .. })
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let result = registry
            .execute(command.with_unadmitted_access(true))
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(!result.repository_admitted);
        assert!(!result.complete);
        assert_eq!(
            result.diagnostic_blocker.as_deref(),
            Some("continuity evidence missing")
        );
        assert!(registry
            .capabilities()
            .iter()
            .any(
                |capability| capability.operation == Operation::TechnicalBars
                    && capability.diagnostic_available
            ));
    }
}
