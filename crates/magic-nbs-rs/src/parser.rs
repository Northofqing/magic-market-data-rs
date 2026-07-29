use crate::{max_response_bytes, NbsError};
use magic_market_core::{
    DataBatch, EconomicObservation, EconomicObservationStatus, EconomicPeriod,
    EconomicSeriesRequest, FiniteNumber, Provenance, ProviderId, SourceEvidence,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

const MAX_METADATA_NODES: usize = 1_000;
const MAX_DATA_NODES: usize = 10_000;

#[derive(Deserialize)]
struct Envelope {
    returncode: i64,
    returndata: ReturnData,
}

#[derive(Deserialize)]
struct ReturnData {
    wdnodes: Vec<Dimension>,
    datanodes: Vec<DataNode>,
}

#[derive(Deserialize)]
struct Dimension {
    wdcode: String,
    nodes: Vec<MetadataNode>,
}

#[derive(Deserialize)]
struct MetadataNode {
    code: String,
    name: String,
    unit: Option<String>,
}

#[derive(Deserialize)]
struct DataNode {
    code: String,
    data: DataValue,
}

#[derive(Deserialize)]
struct DataValue {
    data: Option<f64>,
    hasdata: bool,
}

pub fn parse_national_monthly_payload(
    body: &[u8],
    request: &EconomicSeriesRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<DataBatch<EconomicObservation>, NbsError> {
    if body.len() > max_response_bytes() {
        return Err(NbsError::Protocol("response exceeds 4 MiB".into()));
    }
    validate_request(request)?;
    let envelope: Envelope =
        serde_json::from_slice(body).map_err(|error| NbsError::Decode(error.to_string()))?;
    if envelope.returncode != 200 {
        return Err(NbsError::Protocol(format!(
            "unexpected returncode {}",
            envelope.returncode
        )));
    }
    if envelope.returndata.wdnodes.len() != 2 {
        return Err(NbsError::Protocol(
            "response must contain exactly zb and sj dimensions".into(),
        ));
    }
    let metadata_count = envelope
        .returndata
        .wdnodes
        .iter()
        .try_fold(0usize, |total, node| total.checked_add(node.nodes.len()))
        .ok_or_else(|| NbsError::Protocol("metadata count overflow".into()))?;
    if metadata_count > MAX_METADATA_NODES || envelope.returndata.datanodes.len() > MAX_DATA_NODES {
        return Err(NbsError::Protocol("response node ceiling exceeded".into()));
    }

    let requested: HashSet<&str> = request.series().iter().map(|key| key.code()).collect();
    let mut series = HashMap::new();
    let mut periods = HashSet::new();
    let mut dimensions = HashSet::new();
    for dimension in envelope.returndata.wdnodes {
        if !dimensions.insert(dimension.wdcode.clone()) {
            return Err(NbsError::Protocol("duplicate dimension identity".into()));
        }
        match dimension.wdcode.as_str() {
            "zb" => {
                for node in dimension.nodes {
                    if !requested.contains(node.code.as_str()) {
                        return Err(NbsError::Protocol(format!(
                            "unrequested indicator {}",
                            node.code
                        )));
                    }
                    let unit = node
                        .unit
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| {
                            NbsError::Protocol(format!("indicator {} has no unit", node.code))
                        })?;
                    if series
                        .insert(node.code.clone(), (node.name, unit))
                        .is_some()
                    {
                        return Err(NbsError::Protocol("duplicate indicator identity".into()));
                    }
                }
            }
            "sj" => {
                for node in dimension.nodes {
                    let period = parse_month(&node.code)?;
                    if period < *request.start() || period > *request.end() {
                        return Err(NbsError::Protocol(format!(
                            "unrequested period {}",
                            node.code
                        )));
                    }
                    if !periods.insert(node.code) {
                        return Err(NbsError::Protocol("duplicate period identity".into()));
                    }
                }
            }
            other => {
                return Err(NbsError::Protocol(format!(
                    "unexpected response dimension {other}"
                )));
            }
        }
    }
    if series.len() != requested.len() || !dimensions.contains("zb") || !dimensions.contains("sj") {
        return Err(NbsError::Protocol(
            "response metadata does not exactly match the request".into(),
        ));
    }
    if periods.is_empty() || envelope.returndata.datanodes.is_empty() {
        return Err(NbsError::Protocol(
            "response must contain non-empty period metadata and data nodes".into(),
        ));
    }
    let expected_data_nodes = series
        .len()
        .checked_mul(periods.len())
        .ok_or_else(|| NbsError::Protocol("expected data-node count overflow".into()))?;
    if envelope.returndata.datanodes.len() != expected_data_nodes {
        return Err(NbsError::Protocol(
            "response does not cover every returned series/period identity".into(),
        ));
    }

    let evidence = SourceEvidence::new(ProviderId::Nbs, observed_at, batch_id)?;
    let mut seen = HashSet::new();
    let mut records = Vec::with_capacity(envelope.returndata.datanodes.len());
    for node in envelope.returndata.datanodes {
        let (code, month) = parse_data_identity(&node.code)?;
        if !requested.contains(code) || !periods.contains(month) || !seen.insert(node.code.clone())
        {
            return Err(NbsError::Protocol(
                "data node is duplicate or outside requested identities".into(),
            ));
        }
        let (name, unit) = series
            .get(code)
            .ok_or_else(|| NbsError::Protocol("data node indicator is unknown".into()))?;
        let period = parse_month(month)?;
        let (value, status) = if node.data.hasdata {
            let value = node
                .data
                .data
                .ok_or_else(|| NbsError::Protocol("hasdata row has no numeric value".into()))?;
            (
                Some(FiniteNumber::new(value)?),
                EconomicObservationStatus::Present,
            )
        } else {
            if node.data.data.is_some() {
                return Err(NbsError::Protocol(
                    "missing row unexpectedly contains a numeric value".into(),
                ));
            }
            (None, EconomicObservationStatus::Missing)
        };
        records.push(EconomicObservation::new(
            request
                .series()
                .iter()
                .find(|key| key.code() == code)
                .cloned()
                .ok_or_else(|| NbsError::Protocol("requested key disappeared".into()))?,
            name.clone(),
            None,
            None,
            period,
            value,
            unit.clone(),
            None,
            None,
            status,
            None,
            None,
            evidence.clone(),
        )?);
    }
    if seen.len() != expected_data_nodes {
        return Err(NbsError::Protocol(
            "response series/period coverage is incomplete".into(),
        ));
    }
    records.sort_by(|left, right| {
        left.series()
            .code()
            .cmp(right.series().code())
            .then_with(|| left.period().cmp(right.period()))
    });
    records.truncate(request.max_rows().get() as usize);
    let provenance =
        Provenance::new("NBS diagnostic payload", observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn validate_request(request: &EconomicSeriesRequest) -> Result<(), NbsError> {
    if request.provider() != ProviderId::Nbs {
        return Err(NbsError::InvalidRequest(
            "request provider must be NBS".into(),
        ));
    }
    if request
        .series()
        .iter()
        .any(|key| key.namespace() != "national-monthly")
    {
        return Err(NbsError::Unsupported(
            "only the national-monthly diagnostic namespace is recognized".into(),
        ));
    }
    if request.start().as_month().is_none() || request.end().as_month().is_none() {
        return Err(NbsError::InvalidRequest(
            "NBS diagnostic requests must be monthly".into(),
        ));
    }
    Ok(())
}

fn parse_month(value: &str) -> Result<EconomicPeriod, NbsError> {
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(NbsError::Protocol(format!("invalid NBS month {value:?}")));
    }
    let year = value[..4]
        .parse::<u32>()
        .map_err(|_| NbsError::Protocol("invalid month year".into()))?;
    let month = value[4..]
        .parse::<u32>()
        .map_err(|_| NbsError::Protocol("invalid month number".into()))?;
    EconomicPeriod::month(year, month).map_err(NbsError::from)
}

fn parse_data_identity(value: &str) -> Result<(&str, &str), NbsError> {
    let rest = value
        .strip_prefix("zb.")
        .ok_or_else(|| NbsError::Protocol("invalid data-node identity prefix".into()))?;
    let (code, month) = rest
        .split_once("_sj.")
        .ok_or_else(|| NbsError::Protocol("invalid data-node identity".into()))?;
    if code.is_empty() || month.is_empty() || month.contains('.') || code.contains('.') {
        return Err(NbsError::Protocol("invalid data-node identity".into()));
    }
    Ok((code, month))
}

#[cfg(test)]
mod tests;
