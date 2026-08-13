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
        })
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryResult {
    pub provider: String,
    pub batch_id: String,
    pub complete: bool,
    pub observed_at: String,
    pub source_at: Option<String>,
    pub records: Vec<CanonicalPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capability {
    pub operation: Operation,
    pub repository_admitted: bool,
    pub runtime_available: bool,
    pub provider: String,
    pub exact_scope: String,
    pub blocker: Option<String>,
}

type Handler = Arc<dyn Fn(QueryCommand) -> Result<QueryResult, ServiceError> + Send + Sync>;

#[derive(Clone)]
struct Registration {
    capability: Capability,
    handler: Option<Handler>,
}

#[derive(Clone)]
pub struct OperationRegistry {
    registrations: BTreeMap<Operation, Registration>,
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
                    Registration {
                        capability: Capability {
                            operation,
                            repository_admitted: false,
                            runtime_available: false,
                            provider: String::new(),
                            exact_scope: String::new(),
                            blocker: Some(blocker.clone()),
                        },
                        handler: None,
                    },
                )
            })
            .collect();
        Self { registrations }
    }

    pub fn register_unavailable(&mut self, capability: Capability) -> Result<(), ServiceError> {
        validate_capability(&capability, false)?;
        self.registrations.insert(
            capability.operation,
            Registration {
                capability,
                handler: None,
            },
        );
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
        self.registrations.insert(
            capability.operation,
            Registration {
                capability,
                handler: Some(Arc::new(handler)),
            },
        );
        Ok(())
    }

    #[must_use]
    pub fn capabilities(&self) -> Vec<Capability> {
        self.registrations
            .values()
            .map(|registration| registration.capability.clone())
            .collect()
    }

    pub fn execute(&self, command: QueryCommand) -> Result<QueryResult, ServiceError> {
        let registration = self
            .registrations
            .get(&command.operation)
            .ok_or_else(|| ServiceError::Internal("operation registry is incomplete".to_owned()))?;
        let capability = &registration.capability;
        if !capability.repository_admitted {
            return Err(ServiceError::Unsupported {
                operation: command.operation,
                reason: capability
                    .blocker
                    .clone()
                    .unwrap_or_else(|| "repository capability is unadmitted".to_owned()),
            });
        }
        if !capability.runtime_available {
            return Err(ServiceError::Unavailable {
                operation: command.operation,
                reason: capability
                    .blocker
                    .clone()
                    .unwrap_or_else(|| "runtime capability is unavailable".to_owned()),
            });
        }
        if let Some(preferred) = command.preferred_provider() {
            if preferred != capability.provider {
                return Err(ServiceError::Unsupported {
                    operation: command.operation,
                    reason: format!(
                        "provider {preferred} is not registered for exact scope {}",
                        capability.exact_scope
                    ),
                });
            }
        }
        let handler = registration.handler.as_ref().ok_or_else(|| {
            ServiceError::Internal("admitted runtime capability has no handler".to_owned())
        })?;
        handler(command)
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
    #[error("source precondition failed: {0}")]
    FailedPrecondition(String),
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
            }),
            Err(ServiceError::InvalidRequest(_))
        ));
    }
}
