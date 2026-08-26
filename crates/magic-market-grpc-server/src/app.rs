use std::sync::Arc;
use std::time::Duration;

use magic_market_grpc_contracts::v1;
use magic_market_grpc_contracts::{CANONICAL_JSON_CONTENT_TYPE, PROTOCOL_VERSION};
use magic_market_service::{
    BlockingQueryGateway, CanonicalPayload, Operation, ProviderFailureKind, QueryCommand,
    ServiceError,
};
use prost::Message;
use tokio::sync::Semaphore;
use tonic::metadata::{MetadataMap, MetadataValue};
use tonic::{Code, Request, Response, Status};

use crate::logging::{self, Level};
use crate::observability::{QueryOutcome, RuntimeObservability};

const ERROR_DETAIL_METADATA_KEY: &str = "magic-error-detail-bin";

#[derive(Clone)]
pub(crate) struct GrpcApplication<G> {
    gateway: Arc<G>,
    maximum_payload_bytes: usize,
    unary: Arc<Semaphore>,
    blocking: Arc<Semaphore>,
    blocking_deadline: Duration,
    observability: Arc<RuntimeObservability>,
    unary_concurrency_limit: usize,
    blocking_concurrency_limit: usize,
}

impl<G> GrpcApplication<G>
where
    G: BlockingQueryGateway,
{
    pub(crate) fn new(
        gateway: Arc<G>,
        maximum_payload_bytes: usize,
        unary_concurrency: usize,
        blocking_concurrency: usize,
        blocking_deadline: Duration,
    ) -> Result<Self, &'static str> {
        if maximum_payload_bytes == 0
            || unary_concurrency == 0
            || blocking_concurrency == 0
            || blocking_deadline.is_zero()
        {
            return Err("gRPC application limits must be positive");
        }
        Ok(Self {
            gateway,
            maximum_payload_bytes,
            unary: Arc::new(Semaphore::new(unary_concurrency)),
            blocking: Arc::new(Semaphore::new(blocking_concurrency)),
            blocking_deadline,
            observability: Arc::new(RuntimeObservability::new()),
            unary_concurrency_limit: unary_concurrency,
            blocking_concurrency_limit: blocking_concurrency,
        })
    }

    async fn query(
        &self,
        operation: Operation,
        request: Request<v1::QueryRequest>,
    ) -> Result<Response<v1::QueryResponse>, Status> {
        let observation = self.observability.observe_query();
        let result = self.query_inner(operation, request).await;
        let outcome = match result.as_ref().err().map(Status::code) {
            None => QueryOutcome::Succeeded,
            Some(Code::ResourceExhausted) => QueryOutcome::Rejected,
            Some(Code::DeadlineExceeded) => QueryOutcome::TimedOut,
            Some(_) => QueryOutcome::Failed,
        };
        observation.finish(outcome);
        result
    }

    async fn query_inner(
        &self,
        operation: Operation,
        request: Request<v1::QueryRequest>,
    ) -> Result<Response<v1::QueryResponse>, Status> {
        let request = request.into_inner();
        magic_market_grpc_contracts::validate_query_request(&request, self.maximum_payload_bytes)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        let context = request
            .context
            .ok_or_else(|| Status::invalid_argument("request context is required"))?;
        let payload = request
            .payload
            .ok_or_else(|| Status::invalid_argument("request payload is required"))?;
        let payload = CanonicalPayload::new(
            payload.schema,
            payload.schema_version,
            payload.data,
            self.maximum_payload_bytes,
        )
        .map_err(|error| status_from_error(&context.request_id, operation, error))?;
        let command = QueryCommand::new(
            context.request_id.clone(),
            operation,
            Some(request.preferred_provider),
            payload,
        )
        .map(|command| command.with_unadmitted_access(request.allow_unadmitted))
        .map_err(|error| status_from_error(&context.request_id, operation, error))?;

        let _unary_permit = self
            .unary
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::resource_exhausted("unary concurrency is exhausted"))?;
        let blocking_permit = self
            .blocking
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::resource_exhausted("blocking concurrency is exhausted"))?;
        let gateway = self.gateway.clone();
        let request_id = context.request_id;
        let task = tokio::task::spawn_blocking(move || {
            // A timed-out blocking call cannot be force-cancelled. Keep the
            // blocking permit with the worker so abandoned work remains
            // bounded, while the request-scoped unary permit is released as
            // soon as the client-facing deadline expires.
            let _blocking_permit = blocking_permit;
            gateway.execute(command)
        });
        let result = tokio::time::timeout(self.blocking_deadline, task)
            .await
            .map_err(|_| Status::deadline_exceeded("blocking provider deadline exceeded"))?
            .map_err(|_| Status::internal("blocking provider worker failed"))?
            .map_err(|error| status_from_error(&request_id, operation, error))?;

        Ok(Response::new(v1::QueryResponse {
            request_id,
            operation: grpc_operation(operation) as i32,
            admission: if result.repository_admitted {
                v1::AdmissionState::Admitted as i32
            } else {
                v1::AdmissionState::Unadmitted as i32
            },
            selected_provider: result.provider,
            batch_id: result.batch_id,
            complete: result.complete,
            observed_at: result.observed_at,
            source_at: result.source_at.unwrap_or_default(),
            records: result
                .records
                .into_iter()
                .map(|record| v1::CanonicalPayload {
                    schema: record.schema().to_owned(),
                    schema_version: record.schema_version(),
                    content_type: CANONICAL_JSON_CONTENT_TYPE.to_owned(),
                    data: record.data().to_vec(),
                })
                .collect(),
            diagnostic_blocker: result.diagnostic_blocker.unwrap_or_default(),
        }))
    }
}

fn validate_context(context: Option<&v1::RequestContext>) -> Result<&v1::RequestContext, Status> {
    let context = context.ok_or_else(|| Status::invalid_argument("request context is required"))?;
    if context.protocol_version != PROTOCOL_VERSION {
        return Err(Status::invalid_argument("unsupported protocol version"));
    }
    if context.request_id.trim().is_empty() {
        return Err(Status::invalid_argument("request_id is required"));
    }
    Ok(context)
}

#[tonic::async_trait]
impl<G> v1::system_service_server::SystemService for GrpcApplication<G>
where
    G: BlockingQueryGateway,
{
    async fn get_capabilities(
        &self,
        request: Request<v1::CapabilitiesRequest>,
    ) -> Result<Response<v1::CapabilitiesResponse>, Status> {
        let request = request.into_inner();
        let context = validate_context(request.context.as_ref())?;
        let capabilities = self
            .gateway
            .capabilities()
            .into_iter()
            .map(|capability| v1::Capability {
                operation: grpc_operation(capability.operation) as i32,
                repository_admission: if capability.repository_admitted {
                    v1::AdmissionState::Admitted as i32
                } else {
                    v1::AdmissionState::Unadmitted as i32
                },
                runtime_available: capability.runtime_available,
                provider: capability.provider,
                exact_scope: capability.exact_scope,
                blocker: capability.blocker.unwrap_or_default(),
                diagnostic_available: capability.diagnostic_available,
            })
            .collect();
        Ok(Response::new(v1::CapabilitiesResponse {
            request_id: context.request_id.clone(),
            capabilities,
        }))
    }

    async fn get_health(
        &self,
        request: Request<v1::HealthRequest>,
    ) -> Result<Response<v1::HealthResponse>, Status> {
        let request = request.into_inner();
        let context = validate_context(request.context.as_ref())?;
        let ready = self
            .gateway
            .capabilities()
            .iter()
            .any(|capability| capability.repository_admitted && capability.runtime_available);
        let observability = self.observability.snapshot();
        Ok(Response::new(v1::HealthResponse {
            request_id: context.request_id.clone(),
            live: true,
            ready,
            state: if ready {
                "ready".to_owned()
            } else {
                "serving_fail_closed".to_owned()
            },
            observability: Some(v1::RuntimeObservability {
                process_started_at_unix_ms: observability.process_started_at_unix_ms,
                uptime_millis: observability.uptime_millis,
                query_started: observability.query_started,
                query_succeeded: observability.query_succeeded,
                query_failed: observability.query_failed,
                query_cancelled: observability.query_cancelled,
                query_in_flight: observability.query_in_flight,
                query_rejected: observability.query_rejected,
                query_timed_out: observability.query_timed_out,
                query_duration_micros_total: observability.query_duration_micros_total,
                query_duration_micros_max: observability.query_duration_micros_max,
                unary_concurrency_limit: usize_to_u64(self.unary_concurrency_limit),
                unary_concurrency_available: usize_to_u64(self.unary.available_permits()),
                blocking_concurrency_limit: usize_to_u64(self.blocking_concurrency_limit),
                blocking_concurrency_available: usize_to_u64(self.blocking.available_permits()),
            }),
        }))
    }
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

macro_rules! implement_query_service {
    ($($method:ident => $operation:ident),+ $(,)?) => {
        #[tonic::async_trait]
        impl<G> v1::market_data_service_server::MarketDataService for GrpcApplication<G>
        where
            G: BlockingQueryGateway,
        {
            $(
                async fn $method(
                    &self,
                    request: Request<v1::QueryRequest>,
                ) -> Result<Response<v1::QueryResponse>, Status> {
                    self.query(Operation::$operation, request).await
                }
            )+
        }
    };
}

implement_query_service! {
        historical_bars => HistoricalBars,
        minute_data => MinuteData,
        realtime_quotes => RealtimeQuotes,
        money_flows => MoneyFlows,
        order_books => OrderBooks,
        auctions => Auctions,
        trades => Trades,
        security_metadata => SecurityMetadata,
        global_indices => GlobalIndices,
        foreign_exchange => ForeignExchange,
        economic_calendar => EconomicCalendar,
        futures_delivery => FuturesDelivery,
        reference_rates => ReferenceRates,
        official_fx_fixings => OfficialFxFixings,
        economic_series => EconomicSeries,
        company_filings => CompanyFilings,
        global_news => GlobalNews,
        announcements => Announcements,
        market_announcements => MarketAnnouncements,
        investor_questions => InvestorQuestions,
        policy_documents => PolicyDocuments,
        security_profiles => SecurityProfiles,
        financial_statements => FinancialStatements,
        market_statistics => MarketStatistics,
        technical_bars => TechnicalBars,
        corporate_actions => CorporateActions,
        board_directory => BoardDirectory,
        board_constituents => BoardConstituents,
        board_memberships => BoardMemberships,
        research_reports => ResearchReports,
        research_documents => ResearchDocuments,
        consensus => Consensus,
        target_prices => TargetPrices,
        semantic_search => SemanticSearch,
        fund_flow_series => FundFlowSeries,
        board_flows => BoardFlows,
        margin_data => MarginData,
        block_trades => BlockTrades,
        holder_counts => HolderCounts,
        lockup_events => LockupEvents,
        dividend_plans => DividendPlans,
        post_close_flows => PostCloseFlows,
        northbound_daily => NorthboundDaily,
        limit_pools => LimitPools,
        strong_stock_reasons => StrongStockReasons,
        dragon_tiger => DragonTiger,
        market_dragon_tiger => MarketDragonTiger,
        dragon_tiger_discovery => DragonTigerDiscovery,
        market_rankings => MarketRankings,
        market_breadth => MarketBreadth,
        popularity => Popularity,
        concept_hits => ConceptHits,
        option_data => OptionData,
        provider_top_n_rankings => ProviderTopNRankings,
        instrument_news => InstrumentNews,
        index_quotes => IndexQuotes,
        intraday_shape => IntradayShape,
        t0_evidence => T0Evidence,
        outcome_daily_bars => OutcomeDailyBars,
        upper_limit_pool_review => UpperLimitPoolReview,
}

pub(crate) fn grpc_operation(operation: Operation) -> v1::Operation {
    match operation {
        Operation::HistoricalBars => v1::Operation::HistoricalBars,
        Operation::MinuteData => v1::Operation::MinuteData,
        Operation::RealtimeQuotes => v1::Operation::RealtimeQuotes,
        Operation::MoneyFlows => v1::Operation::MoneyFlows,
        Operation::OrderBooks => v1::Operation::OrderBooks,
        Operation::Auctions => v1::Operation::Auctions,
        Operation::Trades => v1::Operation::Trades,
        Operation::SecurityMetadata => v1::Operation::SecurityMetadata,
        Operation::GlobalIndices => v1::Operation::GlobalIndices,
        Operation::ForeignExchange => v1::Operation::ForeignExchange,
        Operation::EconomicCalendar => v1::Operation::EconomicCalendar,
        Operation::FuturesDelivery => v1::Operation::FuturesDelivery,
        Operation::ReferenceRates => v1::Operation::ReferenceRates,
        Operation::OfficialFxFixings => v1::Operation::OfficialFxFixings,
        Operation::EconomicSeries => v1::Operation::EconomicSeries,
        Operation::CompanyFilings => v1::Operation::CompanyFilings,
        Operation::GlobalNews => v1::Operation::GlobalNews,
        Operation::Announcements => v1::Operation::Announcements,
        Operation::MarketAnnouncements => v1::Operation::MarketAnnouncements,
        Operation::InvestorQuestions => v1::Operation::InvestorQuestions,
        Operation::PolicyDocuments => v1::Operation::PolicyDocuments,
        Operation::SecurityProfiles => v1::Operation::SecurityProfiles,
        Operation::FinancialStatements => v1::Operation::FinancialStatements,
        Operation::MarketStatistics => v1::Operation::MarketStatistics,
        Operation::TechnicalBars => v1::Operation::TechnicalBars,
        Operation::CorporateActions => v1::Operation::CorporateActions,
        Operation::BoardDirectory => v1::Operation::BoardDirectory,
        Operation::BoardConstituents => v1::Operation::BoardConstituents,
        Operation::BoardMemberships => v1::Operation::BoardMemberships,
        Operation::ResearchReports => v1::Operation::ResearchReports,
        Operation::ResearchDocuments => v1::Operation::ResearchDocuments,
        Operation::Consensus => v1::Operation::Consensus,
        Operation::TargetPrices => v1::Operation::TargetPrices,
        Operation::SemanticSearch => v1::Operation::SemanticSearch,
        Operation::FundFlowSeries => v1::Operation::FundFlowSeries,
        Operation::BoardFlows => v1::Operation::BoardFlows,
        Operation::MarginData => v1::Operation::MarginData,
        Operation::BlockTrades => v1::Operation::BlockTrades,
        Operation::HolderCounts => v1::Operation::HolderCounts,
        Operation::LockupEvents => v1::Operation::LockupEvents,
        Operation::DividendPlans => v1::Operation::DividendPlans,
        Operation::PostCloseFlows => v1::Operation::PostCloseFlows,
        Operation::NorthboundDaily => v1::Operation::NorthboundDaily,
        Operation::LimitPools => v1::Operation::LimitPools,
        Operation::StrongStockReasons => v1::Operation::StrongStockReasons,
        Operation::DragonTiger => v1::Operation::DragonTiger,
        Operation::MarketDragonTiger => v1::Operation::MarketDragonTiger,
        Operation::DragonTigerDiscovery => v1::Operation::DragonTigerDiscovery,
        Operation::MarketRankings => v1::Operation::MarketRankings,
        Operation::MarketBreadth => v1::Operation::MarketBreadth,
        Operation::Popularity => v1::Operation::Popularity,
        Operation::ConceptHits => v1::Operation::ConceptHits,
        Operation::OptionData => v1::Operation::OptionData,
        Operation::ProviderTopNRankings => v1::Operation::ProviderTopNRankings,
        Operation::InstrumentNews => v1::Operation::InstrumentNews,
        Operation::IndexQuotes => v1::Operation::IndexQuotes,
        Operation::IntradayShape => v1::Operation::IntradayShape,
        Operation::T0Evidence => v1::Operation::T0Evidence,
        Operation::OutcomeDailyBars => v1::Operation::OutcomeDailyBars,
        Operation::UpperLimitPoolReview => v1::Operation::UpperLimitPoolReview,
    }
}

fn status_from_error(request_id: &str, operation: Operation, error: ServiceError) -> Status {
    let (
        code,
        reason_code,
        retryable,
        message,
        provider,
        evidence_code,
        evidence_field,
        record_index,
    ) = match error {
        ServiceError::InvalidRequest(message) => (
            Code::InvalidArgument,
            "invalid_request",
            false,
            message,
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
        ServiceError::Unsupported { reason, .. } => (
            Code::Unimplemented,
            "capability_unadmitted",
            false,
            reason,
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
        ServiceError::Unauthenticated => (
            Code::Unauthenticated,
            "unauthenticated",
            false,
            "authentication failed".to_owned(),
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
        ServiceError::PermissionDenied(message) => (
            Code::PermissionDenied,
            "permission_denied",
            false,
            message,
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
        ServiceError::ResourceExhausted(message) => (
            Code::ResourceExhausted,
            "resource_exhausted",
            true,
            message,
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
        ServiceError::DeadlineExceeded(message) => (
            Code::DeadlineExceeded,
            "deadline_exceeded",
            true,
            message,
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
        ServiceError::Unavailable { reason, .. } => (
            Code::Unavailable,
            "provider_unavailable",
            true,
            reason,
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
        ServiceError::ProviderFailure {
            operation: rejected_operation,
            provider,
            kind,
            provider_reason,
        } => {
            let (code, reason_code, retryable, message) = match kind {
                ProviderFailureKind::AuthenticationRejected => (
                    Code::PermissionDenied,
                    "provider_authentication_rejected",
                    false,
                    "provider authentication rejected",
                ),
                ProviderFailureKind::RateLimited => (
                    Code::ResourceExhausted,
                    "provider_rate_limited",
                    true,
                    "provider rate limited the query",
                ),
                ProviderFailureKind::QueryRejected => (
                    Code::FailedPrecondition,
                    "external_query_rejected",
                    false,
                    "provider rejected external query",
                ),
                ProviderFailureKind::ResponseInvalid => (
                    Code::FailedPrecondition,
                    "provider_response_invalid",
                    false,
                    "provider response violated its contract",
                ),
                ProviderFailureKind::Unavailable => (
                    Code::Unavailable,
                    "provider_unavailable",
                    true,
                    "provider is unavailable",
                ),
            };
            logging::event(
                Level::Error,
                "grpc_server",
                "provider_failure",
                format_args!(
                    "stage={} request_id={:?} operation={} provider={:?} provider_reason={:?}",
                    reason_code,
                    safe_log_value(request_id, 128),
                    rejected_operation.as_str(),
                    safe_log_value(&provider, 64),
                    safe_log_value(&provider_reason, 512),
                ),
            );
            (
                code,
                reason_code,
                retryable,
                message.to_owned(),
                provider,
                String::new(),
                String::new(),
                None,
            )
        }
        ServiceError::FailedPrecondition(message) => (
            Code::FailedPrecondition,
            "source_precondition_failed",
            false,
            message,
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
        ServiceError::InvalidEvidence {
            provider,
            evidence_code,
            evidence_field,
            record_index,
            message,
        } => (
            Code::FailedPrecondition,
            "invalid_evidence",
            false,
            message,
            provider,
            evidence_code,
            evidence_field,
            record_index,
        ),
        ServiceError::Internal(_) => (
            Code::Internal,
            "internal",
            false,
            "internal service error".to_owned(),
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
    };
    let detail = v1::ErrorDetail {
        request_id: request_id.to_owned(),
        operation: grpc_operation(operation) as i32,
        provider,
        reason_code: reason_code.to_owned(),
        retryable,
        admission: v1::AdmissionState::Unadmitted as i32,
        evidence_code,
        evidence_field,
        record_index: record_index.unwrap_or_default(),
        has_record_index: record_index.is_some(),
    }
    .encode_to_vec();
    let mut metadata = MetadataMap::new();
    metadata.insert_bin(
        ERROR_DETAIL_METADATA_KEY,
        MetadataValue::from_bytes(&detail),
    );
    Status::with_metadata(code, message, metadata)
}

fn safe_log_value(value: &str, maximum_chars: usize) -> String {
    let value = value
        .chars()
        .filter(|character| !character.is_control())
        .take(maximum_chars)
        .collect::<String>();
    if value.is_empty() {
        "<empty>".to_owned()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use magic_market_grpc_contracts::v1::market_data_service_server::MarketDataService;
    use magic_market_grpc_contracts::v1::system_service_server::SystemService;
    use magic_market_service::{Capability, OperationRegistry, QueryResult};

    use super::*;

    struct SlowGateway {
        delay: Duration,
    }

    impl BlockingQueryGateway for SlowGateway {
        fn capabilities(&self) -> Vec<Capability> {
            Vec::new()
        }

        fn execute(&self, _command: QueryCommand) -> Result<QueryResult, ServiceError> {
            std::thread::sleep(self.delay);
            Err(ServiceError::Unavailable {
                operation: Operation::RealtimeQuotes,
                reason: "slow test provider".to_owned(),
            })
        }
    }

    fn request() -> Request<v1::QueryRequest> {
        Request::new(v1::QueryRequest {
            context: Some(v1::RequestContext {
                protocol_version: PROTOCOL_VERSION,
                request_id: "request-1".to_owned(),
            }),
            preferred_provider: String::new(),
            payload: Some(v1::CanonicalPayload {
                schema: "test.request".to_owned(),
                schema_version: 1,
                content_type: CANONICAL_JSON_CONTENT_TYPE.to_owned(),
                data: b"{}".to_vec(),
            }),
            allow_unadmitted: false,
        })
    }

    #[test]
    fn invalid_evidence_status_has_safe_non_retryable_structured_details() {
        let status = status_from_error(
            "request-evidence",
            Operation::Consensus,
            ServiceError::InvalidEvidence {
                provider: "Tonghuashun".to_owned(),
                evidence_code: "consensus_numeric_order".to_owned(),
                evidence_field: "consensus.mean".to_owned(),
                record_index: Some(2),
                message: "consensus evidence is inconsistent".to_owned(),
            },
        );
        assert_eq!(status.code(), Code::FailedPrecondition);
        let detail = v1::ErrorDetail::decode(
            status
                .metadata()
                .get_bin(ERROR_DETAIL_METADATA_KEY)
                .unwrap()
                .to_bytes()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(detail.reason_code, "invalid_evidence");
        assert!(!detail.retryable);
        assert_eq!(detail.admission, v1::AdmissionState::Unadmitted as i32);
        assert_eq!(detail.provider, "Tonghuashun");
        assert_eq!(detail.evidence_code, "consensus_numeric_order");
        assert_eq!(detail.evidence_field, "consensus.mean");
        assert_eq!(detail.record_index, 2);
        assert!(detail.has_record_index);
    }

    #[test]
    fn external_query_rejection_is_safe_structured_and_non_retryable() {
        let status = status_from_error(
            "request-cls-rejected",
            Operation::GlobalNews,
            ServiceError::ProviderFailure {
                operation: Operation::GlobalNews,
                provider: "Cailianpress".to_owned(),
                kind: ProviderFailureKind::QueryRejected,
                provider_reason: "errno=1001 message=bad sign".to_owned(),
            },
        );
        assert_eq!(status.code(), Code::FailedPrecondition);
        assert_eq!(status.message(), "provider rejected external query");
        assert!(!status.message().contains("bad sign"));
        let detail = v1::ErrorDetail::decode(
            status
                .metadata()
                .get_bin(ERROR_DETAIL_METADATA_KEY)
                .unwrap()
                .to_bytes()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(detail.reason_code, "external_query_rejected");
        assert!(!detail.retryable);
        assert_eq!(detail.admission, v1::AdmissionState::Unadmitted as i32);
        assert_eq!(detail.provider, "Cailianpress");
        assert!(detail.evidence_code.is_empty());
        assert!(detail.evidence_field.is_empty());
        assert!(!detail.has_record_index);
        assert_eq!(safe_log_value("bad\r\nsign", 32), "badsign");
        assert_eq!(safe_log_value("123456", 4), "1234");
    }

    #[test]
    fn provider_failure_kinds_have_closed_safe_status_contracts() {
        let cases = [
            (
                ProviderFailureKind::AuthenticationRejected,
                Code::PermissionDenied,
                "provider_authentication_rejected",
                false,
            ),
            (
                ProviderFailureKind::RateLimited,
                Code::ResourceExhausted,
                "provider_rate_limited",
                true,
            ),
            (
                ProviderFailureKind::QueryRejected,
                Code::FailedPrecondition,
                "external_query_rejected",
                false,
            ),
            (
                ProviderFailureKind::ResponseInvalid,
                Code::FailedPrecondition,
                "provider_response_invalid",
                false,
            ),
            (
                ProviderFailureKind::Unavailable,
                Code::Unavailable,
                "provider_unavailable",
                true,
            ),
        ];
        for (kind, expected_code, expected_reason, expected_retryable) in cases {
            let status = status_from_error(
                "request-provider-failure",
                Operation::GlobalNews,
                ServiceError::ProviderFailure {
                    operation: Operation::GlobalNews,
                    provider: "Cailianpress".to_owned(),
                    kind,
                    provider_reason: "internal-only reason".to_owned(),
                },
            );
            assert_eq!(status.code(), expected_code);
            assert!(!status.message().contains("internal-only"));
            let detail = v1::ErrorDetail::decode(
                status
                    .metadata()
                    .get_bin(ERROR_DETAIL_METADATA_KEY)
                    .unwrap()
                    .to_bytes()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(detail.reason_code, expected_reason);
            assert_eq!(detail.retryable, expected_retryable);
            assert_eq!(detail.provider, "Cailianpress");
        }
    }

    #[tokio::test]
    async fn unadmitted_rpc_fails_before_io_with_structured_details() {
        let application = GrpcApplication::new(
            Arc::new(OperationRegistry::all_unadmitted("evidence missing")),
            1024,
            1,
            1,
            Duration::from_secs(1),
        )
        .unwrap();
        let status = application.realtime_quotes(request()).await.unwrap_err();
        assert_eq!(status.code(), Code::Unimplemented);
        let detail = v1::ErrorDetail::decode(
            status
                .metadata()
                .get_bin(ERROR_DETAIL_METADATA_KEY)
                .unwrap()
                .to_bytes()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(detail.request_id, "request-1");
        assert_eq!(detail.reason_code, "capability_unadmitted");

        let health = application
            .get_health(Request::new(v1::HealthRequest {
                context: Some(v1::RequestContext {
                    protocol_version: PROTOCOL_VERSION,
                    request_id: "health-observability".to_owned(),
                }),
            }))
            .await
            .unwrap()
            .into_inner();
        let observability = health.observability.unwrap();
        assert_eq!(observability.query_started, 1);
        assert_eq!(observability.query_failed, 1);
        assert_eq!(observability.query_in_flight, 0);
        assert_eq!(observability.unary_concurrency_limit, 1);
        assert_eq!(observability.unary_concurrency_available, 1);
        assert_eq!(observability.blocking_concurrency_limit, 1);
        assert_eq!(observability.blocking_concurrency_available, 1);
    }

    #[tokio::test]
    async fn explicit_diagnostic_rpc_returns_records_without_promoting_admission() {
        let mut registry = OperationRegistry::all_unadmitted("evidence missing");
        registry
            .register_diagnostic_handler(
                Capability {
                    operation: Operation::TechnicalBars,
                    repository_admitted: false,
                    runtime_available: false,
                    provider: "Baidu".to_owned(),
                    exact_scope: "diagnostic daily bars".to_owned(),
                    blocker: Some("continuity is unproved".to_owned()),
                    diagnostic_available: false,
                },
                |_| {
                    Ok(QueryResult {
                        provider: "Baidu".to_owned(),
                        batch_id: "diagnostic-batch".to_owned(),
                        complete: true,
                        observed_at: "2026-08-15T00:00:00Z".to_owned(),
                        source_at: Some("2026-08-14".to_owned()),
                        records: vec![CanonicalPayload::new(
                            "magic.market.technical_bar",
                            1,
                            br#"{"ma5":null}"#.to_vec(),
                            1024,
                        )
                        .unwrap()],
                        repository_admitted: true,
                        diagnostic_blocker: None,
                    })
                },
            )
            .unwrap();
        let application =
            GrpcApplication::new(Arc::new(registry), 1024, 1, 1, Duration::from_secs(1)).unwrap();
        let mut request = request();
        request.get_mut().preferred_provider = "Baidu".to_owned();
        request.get_mut().allow_unadmitted = true;
        let response = application
            .technical_bars(request)
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.admission, v1::AdmissionState::Unadmitted as i32);
        assert!(!response.complete);
        assert_eq!(response.selected_provider, "Baidu");
        assert_eq!(response.records.len(), 1);
        assert_eq!(response.diagnostic_blocker, "continuity is unproved");
    }

    #[tokio::test]
    async fn deadline_releases_unary_capacity_but_keeps_abandoned_work_bounded() {
        let application = GrpcApplication::new(
            Arc::new(SlowGateway {
                delay: Duration::from_millis(150),
            }),
            1024,
            1,
            1,
            Duration::from_millis(20),
        )
        .unwrap();

        let status = application.realtime_quotes(request()).await.unwrap_err();
        assert_eq!(status.code(), Code::DeadlineExceeded);
        assert_eq!(application.unary.available_permits(), 1);
        assert_eq!(application.blocking.available_permits(), 0);
        let observability = application.observability.snapshot();
        assert_eq!(observability.query_timed_out, 1);
        assert_eq!(observability.query_failed, 1);
        assert_eq!(observability.query_in_flight, 0);

        tokio::time::timeout(Duration::from_secs(2), async {
            while application.blocking.available_permits() == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("blocking provider worker did not release its permit");
        assert_eq!(application.blocking.available_permits(), 1);
    }

    #[test]
    fn every_service_operation_has_an_exact_grpc_mapping() {
        let mapped = magic_market_service::ALL_OPERATIONS
            .iter()
            .copied()
            .map(grpc_operation)
            .collect::<Vec<_>>();
        assert_eq!(
            mapped.len(),
            magic_market_grpc_contracts::READ_OPERATIONS.len()
        );
        assert_eq!(mapped, magic_market_grpc_contracts::READ_OPERATIONS);
    }
}
