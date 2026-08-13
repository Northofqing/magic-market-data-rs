#![forbid(unsafe_code)]
//! Generated, versioned external gRPC contracts.

pub const PROTOCOL_VERSION: u32 = 1;
pub const CANONICAL_JSON_CONTENT_TYPE: &str = "application/json; charset=utf-8";

pub mod v1 {
    tonic::include_proto!("magic.market.v1");
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("magic.market.v1");
}

/// Every read-only Core family exposed by `MarketDataService`.
pub const READ_OPERATIONS: &[v1::Operation] = &[
    v1::Operation::HistoricalBars,
    v1::Operation::MinuteData,
    v1::Operation::RealtimeQuotes,
    v1::Operation::MoneyFlows,
    v1::Operation::OrderBooks,
    v1::Operation::Auctions,
    v1::Operation::Trades,
    v1::Operation::SecurityMetadata,
    v1::Operation::GlobalIndices,
    v1::Operation::ForeignExchange,
    v1::Operation::EconomicCalendar,
    v1::Operation::FuturesDelivery,
    v1::Operation::ReferenceRates,
    v1::Operation::OfficialFxFixings,
    v1::Operation::EconomicSeries,
    v1::Operation::CompanyFilings,
    v1::Operation::GlobalNews,
    v1::Operation::Announcements,
    v1::Operation::MarketAnnouncements,
    v1::Operation::InvestorQuestions,
    v1::Operation::PolicyDocuments,
    v1::Operation::SecurityProfiles,
    v1::Operation::FinancialStatements,
    v1::Operation::MarketStatistics,
    v1::Operation::TechnicalBars,
    v1::Operation::CorporateActions,
    v1::Operation::BoardDirectory,
    v1::Operation::BoardConstituents,
    v1::Operation::BoardMemberships,
    v1::Operation::ResearchReports,
    v1::Operation::ResearchDocuments,
    v1::Operation::Consensus,
    v1::Operation::TargetPrices,
    v1::Operation::SemanticSearch,
    v1::Operation::FundFlowSeries,
    v1::Operation::BoardFlows,
    v1::Operation::MarginData,
    v1::Operation::BlockTrades,
    v1::Operation::HolderCounts,
    v1::Operation::LockupEvents,
    v1::Operation::DividendPlans,
    v1::Operation::PostCloseFlows,
    v1::Operation::NorthboundDaily,
    v1::Operation::LimitPools,
    v1::Operation::StrongStockReasons,
    v1::Operation::DragonTiger,
    v1::Operation::MarketDragonTiger,
    v1::Operation::DragonTigerDiscovery,
    v1::Operation::MarketRankings,
    v1::Operation::MarketBreadth,
    v1::Operation::Popularity,
    v1::Operation::ConceptHits,
    v1::Operation::OptionData,
    v1::Operation::ProviderTopNRankings,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    MissingContext,
    WrongProtocolVersion { actual: u32 },
    MissingRequestId,
    MissingPayload,
    MissingSchema,
    InvalidSchemaVersion,
    WrongContentType,
    EmptyPayload,
    PayloadTooLarge { actual: usize, maximum: usize },
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingContext => formatter.write_str("request context is required"),
            Self::WrongProtocolVersion { actual } => {
                write!(formatter, "unsupported protocol version {actual}")
            }
            Self::MissingRequestId => formatter.write_str("request_id is required"),
            Self::MissingPayload => formatter.write_str("canonical payload is required"),
            Self::MissingSchema => formatter.write_str("payload schema is required"),
            Self::InvalidSchemaVersion => {
                formatter.write_str("payload schema version must be positive")
            }
            Self::WrongContentType => {
                formatter.write_str("payload content type must be canonical JSON")
            }
            Self::EmptyPayload => formatter.write_str("payload bytes are required"),
            Self::PayloadTooLarge { actual, maximum } => {
                write!(formatter, "payload size {actual} exceeds maximum {maximum}")
            }
        }
    }
}

impl std::error::Error for ContractError {}

pub fn validate_query_request(
    request: &v1::QueryRequest,
    maximum_payload_bytes: usize,
) -> Result<(), ContractError> {
    let context = request
        .context
        .as_ref()
        .ok_or(ContractError::MissingContext)?;
    if context.protocol_version != PROTOCOL_VERSION {
        return Err(ContractError::WrongProtocolVersion {
            actual: context.protocol_version,
        });
    }
    if context.request_id.trim().is_empty() {
        return Err(ContractError::MissingRequestId);
    }
    let payload = request
        .payload
        .as_ref()
        .ok_or(ContractError::MissingPayload)?;
    validate_payload(payload, maximum_payload_bytes)
}

pub fn validate_payload(
    payload: &v1::CanonicalPayload,
    maximum_payload_bytes: usize,
) -> Result<(), ContractError> {
    if payload.schema.trim().is_empty() {
        return Err(ContractError::MissingSchema);
    }
    if payload.schema_version == 0 {
        return Err(ContractError::InvalidSchemaVersion);
    }
    if payload.content_type != CANONICAL_JSON_CONTENT_TYPE {
        return Err(ContractError::WrongContentType);
    }
    if payload.data.is_empty() {
        return Err(ContractError::EmptyPayload);
    }
    if maximum_payload_bytes == 0 || payload.data.len() > maximum_payload_bytes {
        return Err(ContractError::PayloadTooLarge {
            actual: payload.data.len(),
            maximum: maximum_payload_bytes,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::*;

    fn request() -> v1::QueryRequest {
        v1::QueryRequest {
            context: Some(v1::RequestContext {
                protocol_version: PROTOCOL_VERSION,
                request_id: "request-1".to_owned(),
            }),
            preferred_provider: String::new(),
            payload: Some(v1::CanonicalPayload {
                schema: "magic.market.core.bars_request".to_owned(),
                schema_version: 1,
                content_type: CANONICAL_JSON_CONTENT_TYPE.to_owned(),
                data: br#"{"symbol":"600000.SH"}"#.to_vec(),
            }),
        }
    }

    #[test]
    fn read_operation_registry_is_complete_and_unique() {
        assert_eq!(READ_OPERATIONS.len(), 54);
        let mut values = READ_OPERATIONS
            .iter()
            .map(|value| *value as i32)
            .collect::<Vec<_>>();
        values.sort_unstable();
        values.dedup();
        assert_eq!(values.len(), READ_OPERATIONS.len());
        assert!(!READ_OPERATIONS.contains(&v1::Operation::Unspecified));
    }

    #[test]
    fn query_contract_round_trips_and_checks_bounds() {
        let encoded = request().encode_to_vec();
        let decoded = v1::QueryRequest::decode(encoded.as_slice()).unwrap();
        validate_query_request(&decoded, 1024).unwrap();
        assert!(matches!(
            validate_query_request(&decoded, 1),
            Err(ContractError::PayloadTooLarge { .. })
        ));
    }

    #[test]
    fn descriptor_set_is_present() {
        assert!(!v1::FILE_DESCRIPTOR_SET.is_empty());
    }
}
