//! Observation evidence for one validated native historical response, not a
//! trading-calendar, revision or point-in-time coverage certificate.

use magic_market_core::{Bar, DataBatch};
use magic_market_transport::HttpResponse;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use super::{HistoricalData, Success};

#[derive(Debug, Serialize)]
pub struct HistoricalBarsOutcome {
    batch: DataBatch<Bar>,
    coverage: HistoricalCoverage,
}

impl HistoricalBarsOutcome {
    pub fn batch(&self) -> &DataBatch<Bar> {
        &self.batch
    }

    pub fn coverage(&self) -> &HistoricalCoverage {
        &self.coverage
    }

    pub fn into_batch(self) -> DataBatch<Bar> {
        self.batch
    }

    pub(super) fn validated(
        batch: DataBatch<Bar>,
        source_rows: usize,
        native_response: NativeHistoricalResponse,
        response_receipt: HistoricalResponseReceipt,
    ) -> Self {
        let returned_rows = batch.records().len();
        Self {
            batch,
            coverage: HistoricalCoverage {
                response_validated: true,
                validated_source_rows: source_rows,
                returned_rows,
                caller_limit_truncated: source_rows > returned_rows,
                source_exhaustion: UnprovenHistoricalEvidence::Unknown,
                authority_calendar_coverage: UnprovenHistoricalEvidence::Unknown,
                missing_date_reasons: UnprovenHistoricalEvidence::Unknown,
                source_revision: UnprovenHistoricalEvidence::NotProvided,
                historical_publication_time: UnprovenHistoricalEvidence::NotProvided,
                pit_guarantee: false,
                native_response,
                response_receipt,
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct HistoricalCoverage {
    response_validated: bool,
    validated_source_rows: usize,
    returned_rows: usize,
    caller_limit_truncated: bool,
    source_exhaustion: UnprovenHistoricalEvidence,
    authority_calendar_coverage: UnprovenHistoricalEvidence,
    missing_date_reasons: UnprovenHistoricalEvidence,
    source_revision: UnprovenHistoricalEvidence,
    historical_publication_time: UnprovenHistoricalEvidence,
    pit_guarantee: bool,
    native_response: NativeHistoricalResponse,
    response_receipt: HistoricalResponseReceipt,
}

impl HistoricalCoverage {
    pub fn validated_source_rows(&self) -> usize {
        self.validated_source_rows
    }

    pub fn returned_rows(&self) -> usize {
        self.returned_rows
    }

    pub fn caller_limit_truncated(&self) -> bool {
        self.caller_limit_truncated
    }
}

#[derive(Debug, Serialize)]
enum UnprovenHistoricalEvidence {
    Unknown,
    NotProvided,
}

/// The provider's actual optional field: omission must not become JSON null.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(tag = "state", content = "value")]
pub(super) enum NativeAdjustment {
    #[default]
    Absent,
    Null,
    Value(String),
}

impl NativeAdjustment {
    pub(super) fn as_deref(&self) -> Option<&str> {
        match self {
            Self::Value(value) => Some(value),
            Self::Absent | Self::Null => None,
        }
    }
}

impl<'de> Deserialize<'de> for NativeAdjustment {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<String>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

#[derive(Debug, Serialize)]
pub(super) struct NativeHistoricalResponse {
    request_id: String,
    thscode: String,
    interval: String,
    adjust: NativeAdjustment,
    timestamp_ms: i64,
}

impl NativeHistoricalResponse {
    pub(super) fn capture(response: &Success<HistoricalData>) -> Self {
        Self {
            request_id: response.request_id.clone(),
            thscode: response.data.thscode.clone(),
            interval: response.data.interval.clone(),
            adjust: response.data.adjust.clone(),
            timestamp_ms: response.data.timestamp,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct HistoricalResponseReceipt {
    body_sha256: String,
    body_byte_length: usize,
    final_url: String,
}

impl HistoricalResponseReceipt {
    pub(super) fn capture(response: &HttpResponse) -> Self {
        Self {
            body_sha256: format!("{:x}", Sha256::digest(response.body())),
            body_byte_length: response.body().len(),
            final_url: response.final_url().to_owned(),
        }
    }
}
