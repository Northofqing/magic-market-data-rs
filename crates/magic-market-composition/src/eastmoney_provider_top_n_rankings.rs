use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError};
use magic_market_core::{
    validate_provider_top_n_ranking_batch, DataBatch, IsoDate, NonEmptyText, ProviderId,
    ProviderTopNRankingCapabilities, ProviderTopNRankingEntry, ProviderTopNRankingRequest,
    ProviderTopNRankings,
};
use magic_market_router::{
    AcceptancePolicy, FailoverChain, FailureKind, RouteAttempt, RouteOutcome, RoutedSource,
    RouterError, SourceError, SourceFn,
};
use std::sync::Arc;
use time::{OffsetDateTime, UtcOffset};

type ChinaDateClock = Arc<dyn Fn() -> Result<String, String> + Send + Sync>;

struct ComposedProviderTopNSource {
    source: SourceFn<ProviderTopNRankingRequest, ProviderTopNRankingEntry>,
    capabilities: ProviderTopNRankingCapabilities,
    expected_source: NonEmptyText,
}

impl std::fmt::Debug for ComposedProviderTopNSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ComposedProviderTopNSource")
            .field("provider_id", &self.source.provider_id())
            .field("capabilities", &self.capabilities)
            .field("expected_source", &self.expected_source)
            .finish()
    }
}

impl RoutedSource<ProviderTopNRankingRequest, ProviderTopNRankingEntry>
    for ComposedProviderTopNSource
{
    fn provider_id(&self) -> ProviderId {
        self.source.provider_id()
    }

    fn fetch(
        &self,
        request: &ProviderTopNRankingRequest,
    ) -> Result<DataBatch<ProviderTopNRankingEntry>, SourceError> {
        self.source.fetch(request)
    }
}

fn eastmoney_source(
    provider: Arc<EastmoneyClient>,
) -> Result<ComposedProviderTopNSource, RouterError> {
    let provider_id = ProviderId::Eastmoney;
    let expected_source = EastmoneyClient::provider_top_n_source_identity().map_err(|error| {
        RouterError::InvalidConfiguration(format!(
            "Eastmoney Top-N source identity is invalid: {error}"
        ))
    })?;
    let capabilities = EastmoneyClient::provider_top_n_ranking_capabilities();
    build_source(
        provider_id,
        expected_source,
        capabilities,
        provider,
        classify_eastmoney_error,
    )
}

fn build_source<Provider, Classify>(
    provider_id: ProviderId,
    expected_source: NonEmptyText,
    capabilities: ProviderTopNRankingCapabilities,
    provider: Arc<Provider>,
    classify: Classify,
) -> Result<ComposedProviderTopNSource, RouterError>
where
    Provider: ProviderTopNRankings + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    if !capabilities.volume_ratio && !capabilities.main_net_inflow {
        return Err(RouterError::InvalidConfiguration(format!(
            "provider {provider_id:?} has no admitted provider Top-N ranking metric"
        )));
    }

    let validation_source = expected_source.clone();
    let source = SourceFn::new(provider_id, move |request: &ProviderTopNRankingRequest| {
        if !capabilities.supports(request.kind()) {
            return Err(SourceError::try_next(
                FailureKind::Unsupported,
                format!(
                    "provider {provider_id:?} does not admit provider Top-N metric {:?}",
                    request.kind()
                ),
            ));
        }
        let batch = provider
            .provider_top_n_rankings(request)
            .map_err(&classify)?;
        validate_provider_top_n_ranking_batch(
            &batch,
            request,
            capabilities,
            provider_id,
            &validation_source,
        )
        .map_err(|error| {
            SourceError::try_next(
                FailureKind::Evidence,
                format!("provider Top-N batch rejected: {error}"),
            )
        })?;
        Ok(batch)
    });

    Ok(ComposedProviderTopNSource {
        source,
        capabilities,
        expected_source,
    })
}

fn classify_eastmoney_error(error: EastmoneyError) -> SourceError {
    match error {
        EastmoneyError::InvalidRequest(message) => {
            SourceError::stop(FailureKind::InvalidRequest, message)
        }
        EastmoneyError::Unsupported(message) => {
            SourceError::try_next(FailureKind::Unsupported, message)
        }
        EastmoneyError::Transport(message) => {
            SourceError::try_next(FailureKind::Transport, message)
        }
        EastmoneyError::ResponseTooLarge { limit } => SourceError::try_next(
            FailureKind::Protocol,
            format!("Eastmoney response exceeds the {limit} byte limit"),
        ),
        EastmoneyError::VerifiedEmpty(evidence) => {
            SourceError::try_next(FailureKind::NoData, evidence.to_string())
        }
        EastmoneyError::Decode(message) | EastmoneyError::Protocol(message) => {
            SourceError::try_next(FailureKind::Protocol, message)
        }
        EastmoneyError::Core(error) => {
            SourceError::try_next(FailureKind::Evidence, error.to_string())
        }
    }
}

/// Errors specific to the concrete Eastmoney provider Top-N route.
///
/// The generic Router error remains unchanged and is wrapped rather than
/// extended, preserving downstream exhaustive matches.
#[derive(Debug, thiserror::Error)]
pub enum EastmoneyProviderTopNRouterError {
    #[error("Eastmoney provider Top-N routing request rejected: {0}")]
    RejectedRequest(String),
    #[error("Eastmoney provider Top-N China-date clock failed: {0}")]
    Clock(String),
    #[error(transparent)]
    Routing(#[from] RouterError),
}

impl EastmoneyProviderTopNRouterError {
    pub fn attempts(&self) -> &[RouteAttempt] {
        match self {
            Self::Routing(error) => error.attempts(),
            Self::RejectedRequest(_) | Self::Clock(_) => &[],
        }
    }
}

/// Non-forgeable concrete Eastmoney route for the narrow post-close Top-N
/// contract.
///
/// Construction creates the production [`EastmoneyClient`] internally and the
/// type exposes no client injection or generic registration method.
/// Provider-neutral Core validation still checks every selected batch before
/// routing succeeds.
pub struct EastmoneyProviderTopNRankingRouter {
    chain: FailoverChain<ProviderTopNRankingRequest, ProviderTopNRankingEntry>,
    china_date_clock: ChinaDateClock,
    capabilities: ProviderTopNRankingCapabilities,
    expected_source: NonEmptyText,
}

impl std::fmt::Debug for EastmoneyProviderTopNRankingRouter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EastmoneyProviderTopNRankingRouter")
            .field("chain", &self.chain)
            .field("china_date_clock", &"<injected>")
            .field("capabilities", &self.capabilities)
            .field("expected_source", &self.expected_source)
            .finish()
    }
}

impl EastmoneyProviderTopNRankingRouter {
    pub fn new() -> Result<Self, RouterError> {
        let provider = Arc::new(EastmoneyClient::new().map_err(|error| {
            RouterError::InvalidConfiguration(format!(
                "Eastmoney production transport initialization failed: {error}"
            ))
        })?);
        Self::with_source_and_clock(eastmoney_source(provider)?, Arc::new(current_china_date))
    }

    fn with_source_and_clock(
        source: ComposedProviderTopNSource,
        china_date_clock: ChinaDateClock,
    ) -> Result<Self, RouterError> {
        let capabilities = source.capabilities;
        let expected_source = source.expected_source.clone();
        let mut chain = FailoverChain::new(
            AcceptancePolicy::new()
                .with_require_complete(true)
                .with_accept_complete_empty(false),
        );
        chain.register(source)?;
        Ok(Self {
            chain,
            china_date_clock,
            capabilities,
            expected_source,
        })
    }

    pub fn policy(&self) -> AcceptancePolicy {
        self.chain.policy()
    }

    pub fn provider_ids(&self) -> Vec<ProviderId> {
        self.chain.provider_ids()
    }

    pub fn capabilities(&self) -> ProviderTopNRankingCapabilities {
        self.capabilities
    }

    pub fn expected_source(&self) -> &NonEmptyText {
        &self.expected_source
    }

    pub fn route(
        &self,
        request: &ProviderTopNRankingRequest,
    ) -> Result<RouteOutcome<ProviderTopNRankingEntry>, EastmoneyProviderTopNRouterError> {
        let current_china_date =
            (self.china_date_clock)().map_err(EastmoneyProviderTopNRouterError::Clock)?;
        let current_china_date = IsoDate::new(current_china_date).map_err(|error| {
            EastmoneyProviderTopNRouterError::Clock(format!(
                "current China date is invalid: {error}"
            ))
        })?;
        if request.trading_date() > &current_china_date {
            return Err(EastmoneyProviderTopNRouterError::RejectedRequest(format!(
                "provider Top-N request date {} is later than current China date {}",
                request.trading_date().as_str(),
                current_china_date.as_str()
            )));
        }
        Ok(self.chain.route(request)?)
    }
}

fn current_china_date() -> Result<String, String> {
    let china_offset =
        UtcOffset::from_hms(8, 0, 0).map_err(|error| format!("invalid fixed offset: {error}"))?;
    Ok(OffsetDateTime::now_utc()
        .to_offset(china_offset)
        .date()
        .to_string())
}

#[cfg(test)]
#[path = "../tests/internal/eastmoney_provider_top_n_rankings_tests.rs"]
mod tests;
