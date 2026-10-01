//! Serial, bounded collection through the admitted service operations.

use std::io::Write;

use magic_market_service::{
    BlockingQueryGateway, CanonicalPayload, Operation, QueryCommand, QueryResult, ServiceError,
};
use magic_official_news_rs::OfficialSource;
use serde::Serialize;
use serde_json::{json, Value};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::official_publications::{
    OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA, OFFICIAL_PUBLICATION_LISTING_SCHEMA,
    OFFICIAL_PUBLICATION_REQUEST_SCHEMA, OFFICIAL_PUBLICATION_SCHEMA,
};

#[derive(Debug, Error)]
pub enum OfficialCollectionError {
    #[error(transparent)]
    Service(#[from] ServiceError),
    #[error("collection journal write failed: {0}")]
    Journal(#[from] std::io::Error),
    #[error("collection journal serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct OfficialCollectionSummary {
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
}

/// Collect each admitted current listing and its selected originals, without overlap.
/// Every query is flushed separately; provider failures are journaled and isolated.
pub fn collect_official_publications_round(
    gateway: &impl BlockingQueryGateway,
    round: u64,
    limit: u32,
    maximum_payload_bytes: usize,
    journal: &mut impl Write,
) -> Result<OfficialCollectionSummary, OfficialCollectionError> {
    if round == 0 || !(1..=20).contains(&limit) || maximum_payload_bytes == 0 {
        return Err(ServiceError::InvalidRequest(
            "collection round/payload bound must be positive and limit must be 1..20".into(),
        )
        .into());
    }
    let mut summary = OfficialCollectionSummary::default();
    for source in OfficialSource::ALL
        .into_iter()
        .filter(|source| source.admitted())
    {
        let request_id = format!("official-{round}-{}-list", source.as_str());
        let command = collection_command(
            &request_id,
            Operation::OfficialPublications,
            source,
            OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA,
            json!({"limit": limit}),
            maximum_payload_bytes,
        )?;
        let Some(listing) = journal_query(
            gateway,
            command,
            source,
            round,
            limit,
            None,
            journal,
            &mut summary,
        )?
        else {
            continue;
        };
        // journal_query has checked this entire bounded envelope before success.
        for (index, item) in listing["items"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let url = item["canonical_url"].as_str().ok_or_else(|| {
                ServiceError::Internal("validated listing URL disappeared".into())
            })?;
            let request_id = format!("official-{round}-{}-article-{index}", source.as_str());
            let command = collection_command(
                &request_id,
                Operation::OfficialPublication,
                source,
                OFFICIAL_PUBLICATION_REQUEST_SCHEMA,
                json!({"url": url}),
                maximum_payload_bytes,
            )?;
            journal_query(
                gateway,
                command,
                source,
                round,
                limit,
                Some(url),
                journal,
                &mut summary,
            )?;
        }
    }
    Ok(summary)
}

fn collection_command(
    request_id: &str,
    operation: Operation,
    source: OfficialSource,
    schema: &str,
    value: Value,
    maximum_payload_bytes: usize,
) -> Result<QueryCommand, OfficialCollectionError> {
    Ok(QueryCommand::new(
        request_id,
        operation,
        Some(source.as_str().into()),
        CanonicalPayload::new(
            schema,
            1,
            serde_json::to_vec(&value)?,
            maximum_payload_bytes,
        )?,
    )?)
}

#[allow(clippy::too_many_arguments)]
fn journal_query(
    gateway: &impl BlockingQueryGateway,
    command: QueryCommand,
    source: OfficialSource,
    round: u64,
    limit: u32,
    requested_url: Option<&str>,
    journal: &mut impl Write,
    summary: &mut OfficialCollectionSummary,
) -> Result<Option<Value>, OfficialCollectionError> {
    let request_id = command.request_id().to_owned();
    let operation = command.operation();
    summary.attempted += 1;
    let result = gateway.execute(command).and_then(|result| {
        let value = validate_envelope(&result, operation, source, limit, requested_url)?;
        Ok((result, value))
    });
    let (event, value) = match result {
        Ok((result, value)) => {
            summary.succeeded += 1;
            let record = &result.records[0];
            (
                json!({
                    "format": "magic.market.official_collection", "schema_version": 1,
                    "round": round, "request_id": request_id, "operation": operation.as_str(),
                    "provider": source.as_str(), "requested_url": requested_url, "state": "success",
                    "batch_id": result.batch_id, "complete": result.complete,
                    "repository_admitted": result.repository_admitted,
                    "observed_at": result.observed_at, "source_at": result.source_at,
                    "record": {"schema": record.schema(), "schema_version": record.schema_version(), "data": value},
                }),
                Some(value),
            )
        }
        Err(error) => {
            summary.failed += 1;
            let observed_at = OffsetDateTime::now_utc().format(&Rfc3339).map_err(|_| {
                ServiceError::Internal("collection observation clock failed".into())
            })?;
            (
                json!({
                    "format": "magic.market.official_collection", "schema_version": 1,
                    "round": round, "request_id": request_id, "operation": operation.as_str(),
                    "provider": source.as_str(), "requested_url": requested_url,
                    "state": "failed", "observed_at": observed_at, "reason": error.to_string(),
                }),
                None,
            )
        }
    };
    serde_json::to_writer(&mut *journal, &event)?;
    journal.write_all(b"\n")?;
    journal.flush()?;
    Ok(value)
}

fn validate_envelope(
    result: &QueryResult,
    operation: Operation,
    source: OfficialSource,
    limit: u32,
    requested_url: Option<&str>,
) -> Result<Value, ServiceError> {
    let invalid =
        || ServiceError::FailedPrecondition("official collection envelope is inconsistent".into());
    if result.provider != source.as_str()
        || !result.repository_admitted
        || !result.complete
        || result.source_at.is_some()
        || result.records.len() != 1
        || result.batch_id.is_empty()
    {
        return Err(invalid());
    }
    let record = &result.records[0];
    let expected_schema = if operation == Operation::OfficialPublications {
        OFFICIAL_PUBLICATION_LISTING_SCHEMA
    } else {
        OFFICIAL_PUBLICATION_SCHEMA
    };
    if record.schema() != expected_schema || record.schema_version() != 1 {
        return Err(invalid());
    }
    let value: Value = serde_json::from_slice(record.data()).map_err(|_| invalid())?;
    if value["source"].as_str() != Some(source.as_str())
        || value["observed_at"].as_str() != Some(result.observed_at.as_str())
    {
        return Err(invalid());
    }
    if operation == Operation::OfficialPublications {
        let items = value["items"].as_array().ok_or_else(invalid)?;
        if items.is_empty()
            || items.len() > limit as usize
            || value["source_rows"]
                .as_u64()
                .is_none_or(|rows| rows < items.len() as u64)
            || items.iter().any(|item| {
                item["canonical_url"].as_str().is_none()
                    || item["source"].as_str() != Some(source.as_str())
            })
        {
            return Err(invalid());
        }
    } else if value["canonical_url"].as_str() != requested_url {
        return Err(invalid());
    }
    Ok(value)
}

#[cfg(test)]
mod tests;
