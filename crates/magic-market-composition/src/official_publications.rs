//! Source-native official publications at the transport-neutral service boundary.

use std::time::Duration;

use magic_market_service::{
    CanonicalPayload, Capability, Operation, OperationRegistry, ProviderFailureKind, QueryCommand,
    QueryResult, ServiceError,
};
use magic_market_transport::TransportError;
use magic_official_news_rs::{OfficialNewsClient, OfficialNewsError, OfficialSource};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ProductionRegistryError;

pub const OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA: &str = "magic.market.official_publications.request";
pub const OFFICIAL_PUBLICATION_REQUEST_SCHEMA: &str = "magic.market.official_publication.request";
pub const OFFICIAL_PUBLICATION_LISTING_SCHEMA: &str = "magic.market.official_publication_listing";
pub const OFFICIAL_PUBLICATION_SCHEMA: &str = "magic.market.official_publication";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListingRequest {
    limit: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArticleRequest {
    url: String,
}

/// A focused registry for the collector, using the same handlers as the full service.
pub fn official_publication_registry(
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<OperationRegistry, ProductionRegistryError> {
    let mut registry =
        OperationRegistry::all_unadmitted("not part of the official-publication registry");
    register_official_publications(&mut registry, provider_timeout, maximum_payload_bytes)?;
    Ok(registry)
}

pub(crate) fn register_official_publications(
    registry: &mut OperationRegistry,
    provider_timeout: Duration,
    maximum_payload_bytes: usize,
) -> Result<(), ProductionRegistryError> {
    if provider_timeout.is_zero() || maximum_payload_bytes == 0 {
        return Err(ProductionRegistryError::InvalidLimit(
            "official publication timeout and payload bound must be positive",
        ));
    }
    for source in OfficialSource::ALL {
        if source.admitted() {
            register_client(
                registry,
                source,
                OfficialNewsClient::with_timeout(source, provider_timeout)?,
                maximum_payload_bytes,
            )?;
        } else {
            for operation in [
                Operation::OfficialPublications,
                Operation::OfficialPublication,
            ] {
                registry.register_unavailable(capability(operation, source, false))?;
            }
        }
    }
    for operation in [
        Operation::OfficialPublications,
        Operation::OfficialPublication,
    ] {
        registry.set_default_provider(operation, OfficialSource::Nbs.as_str())?;
    }
    Ok(())
}

fn capability(operation: Operation, source: OfficialSource, admitted: bool) -> Capability {
    let column = match source {
        OfficialSource::Nbs => "NBS latest releases",
        OfficialSource::Pbc => "PBC communication releases",
        OfficialSource::Ndrc => "NDRC news releases",
        OfficialSource::Mof => "MOF comprehensive department policy releases",
        OfficialSource::Miit => "MIIT ministry leadership activities",
        OfficialSource::Mofcom => "MOFCOM daily news releases",
        OfficialSource::Nea => "NEA news-center top two five-row bureau-work windows only",
        OfficialSource::Csrc => {
            "CSRC current aggregated news page including same-host leadership activities"
        }
        OfficialSource::Gacc => "GACC unverified listing/original templates",
    };
    Capability {
        operation,
        repository_admitted: admitted,
        runtime_available: admitted,
        provider: source.as_str().into(),
        exact_scope: format!(
            "{column}: {}; same-host verified original HTML paths; native labels/precision/evidence; no history or publication instant",
            source.listing_url(),
        ),
        blocker: (!admitted).then(|| {
            "official source listing/original contract and normal TLS remain unadmitted".into()
        }),
        diagnostic_available: false,
    }
}

fn register_client(
    registry: &mut OperationRegistry,
    source: OfficialSource,
    client: OfficialNewsClient,
    maximum_payload_bytes: usize,
) -> Result<(), ServiceError> {
    let listing_client = client.clone();
    registry.register_handler(
        capability(Operation::OfficialPublications, source, true),
        move |command| {
            let request: ListingRequest = decode(&command, OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA)?;
            let listing = listing_client
                .latest(request.limit)
                .map_err(|error| source_error(command.operation(), source, error))?;
            envelope(
                command.operation(),
                source,
                OFFICIAL_PUBLICATION_LISTING_SCHEMA,
                &listing.observed_at,
                &listing,
                maximum_payload_bytes,
            )
        },
    )?;
    registry.register_handler(
        capability(Operation::OfficialPublication, source, true),
        move |command| {
            let request: ArticleRequest = decode(&command, OFFICIAL_PUBLICATION_REQUEST_SCHEMA)?;
            let publication = client
                .article(&request.url)
                .map_err(|error| source_error(command.operation(), source, error))?;
            envelope(
                command.operation(),
                source,
                OFFICIAL_PUBLICATION_SCHEMA,
                &publication.observed_at,
                &publication,
                maximum_payload_bytes,
            )
        },
    )
}

fn decode<T: serde::de::DeserializeOwned>(
    command: &QueryCommand,
    schema: &str,
) -> Result<T, ServiceError> {
    if command.payload().schema() != schema || command.payload().schema_version() != 1 {
        return Err(ServiceError::InvalidRequest(format!(
            "{} requires {schema} version 1",
            command.operation().as_str(),
        )));
    }
    serde_json::from_slice(command.payload().data())
        .map_err(|error| ServiceError::InvalidRequest(format!("invalid official request: {error}")))
}

fn envelope<T: Serialize>(
    operation: Operation,
    source: OfficialSource,
    schema: &str,
    observed_at: &str,
    value: &T,
    maximum_payload_bytes: usize,
) -> Result<QueryResult, ServiceError> {
    let data = serde_json::to_vec(value)
        .map_err(|_| ServiceError::Internal("official publication serialization failed".into()))?;
    let mut digest = Sha256::new();
    digest.update(operation.as_str());
    digest.update([0]);
    digest.update(source.as_str());
    digest.update([0]);
    digest.update(&data);
    let batch_id = format!("official-{:x}", digest.finalize());
    Ok(QueryResult {
        provider: source.as_str().into(),
        batch_id,
        complete: true,
        observed_at: observed_at.into(),
        source_at: None,
        records: vec![CanonicalPayload::new(
            schema,
            1,
            data,
            maximum_payload_bytes,
        )?],
        repository_admitted: true,
        diagnostic_blocker: None,
    })
}

fn source_error(
    operation: Operation,
    source: OfficialSource,
    error: OfficialNewsError,
) -> ServiceError {
    let (kind, reason) = match error {
        OfficialNewsError::InvalidRequest(reason) => return ServiceError::InvalidRequest(reason),
        OfficialNewsError::Unsupported(reason) => {
            return ServiceError::Unsupported { operation, reason }
        }
        OfficialNewsError::Unadmitted(_) => {
            return ServiceError::Unsupported {
                operation,
                reason: "official publication source is unadmitted".into(),
            }
        }
        OfficialNewsError::Transport(TransportError::HttpStatus { status: 429 }) => {
            (ProviderFailureKind::RateLimited, "http_429")
        }
        OfficialNewsError::Transport(
            TransportError::Authentication(_) | TransportError::HttpStatus { status: 401 | 403 },
        ) => (
            ProviderFailureKind::AuthenticationRejected,
            "authentication_rejected",
        ),
        OfficialNewsError::Transport(
            TransportError::Network(_) | TransportError::HttpStatus { status: 500..=599 },
        ) => (ProviderFailureKind::Unavailable, "transport_unavailable"),
        OfficialNewsError::Transport(TransportError::Internal(_)) => {
            return ServiceError::Internal("official transport internal failure".into())
        }
        OfficialNewsError::Transport(_)
        | OfficialNewsError::Core(_)
        | OfficialNewsError::Protocol(_) => (
            ProviderFailureKind::ResponseInvalid,
            "official_response_invalid",
        ),
    };
    ServiceError::ProviderFailure {
        operation,
        provider: source.as_str().into(),
        kind,
        provider_reason: reason.into(),
    }
}

#[cfg(test)]
mod tests;
