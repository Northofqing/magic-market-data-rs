use crate::parser::{
    merge_records, parse_older, parse_parent, sort_records, validate_older_filename,
    OlderFileDescriptor, MAX_DECODED_FILINGS,
};
use crate::{SecEdgarClient, SecEdgarError, DEFAULT_TIMEOUT};
use magic_market_core::{CompanyFiling, CompanyFilingRequest, DataBatch, Provenance};
use magic_market_transport::{
    EndpointPolicy, HttpMethod, HttpRequest, MediaType, ReqwestTransport, TransportError,
};
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub(crate) const MAX_SUBMISSIONS_BYTES: usize = 8 * 1024 * 1024;
const SOURCE_NAME: &str = "sec-edgar-submissions";
const MAX_COMPANIES_PER_CALL: usize = 10;

pub(crate) fn submissions_policy(timeout: Duration) -> Result<EndpointPolicy, TransportError> {
    EndpointPolicy::new(
        "data.sec.gov",
        vec!["/submissions".into()],
        vec![],
        vec![MediaType::Json],
        MAX_SUBMISSIONS_BYTES,
        timeout,
    )
}

pub(crate) fn production_transport(timeout: Duration) -> Result<ReqwestTransport, TransportError> {
    ReqwestTransport::new(submissions_policy(timeout)?)
}

pub(crate) fn fetch_company_filings(
    client: &SecEdgarClient,
    request: &CompanyFilingRequest,
) -> Result<DataBatch<CompanyFiling>, SecEdgarError> {
    if request.companies().len() > MAX_COMPANIES_PER_CALL {
        return Err(SecEdgarError::InvalidRequest(
            "SEC provider accepts at most 10 companies per call".into(),
        ));
    }
    let observed_at = now()?;
    let batch_id = format!("sec-edgar:{observed_at}");
    let mut all_records = Vec::new();
    let maximum_records = request.max_records().get() as usize;

    for requested_company in request.companies() {
        let remaining_budget = maximum_records.saturating_sub(all_records.len());
        let main_url = main_submissions_url(requested_company.cik());
        let body = fetch_json(client, &main_url)?;
        let mut parsed = parse_parent(&body, requested_company, request, &observed_at, &batch_id)?;
        let mut decoded_rows = parsed.recent_decoded_count;
        let older_files = std::mem::take(&mut parsed.older_files);
        for descriptor in older_files {
            if !should_fetch_older(&descriptor, request, parsed.records.len(), remaining_budget) {
                continue;
            }
            validate_older_filename(&descriptor.name, requested_company.cik())?;
            let older_url = older_submissions_url(&descriptor.name)?;
            let body = fetch_json(client, &older_url)?;
            let older = parse_older(
                &body,
                &parsed.company,
                &parsed.company_name,
                request,
                &observed_at,
                &batch_id,
            )?;
            if older.decoded_count != descriptor.filing_count
                || older.min_date != descriptor.filing_from
                || older.max_date != descriptor.filing_to
            {
                return Err(SecEdgarError::Protocol(
                    "older submissions rows contradict the parent file catalog".into(),
                ));
            }
            decoded_rows = decoded_rows
                .checked_add(older.decoded_count)
                .ok_or_else(|| SecEdgarError::Protocol("decoded filing count overflow".into()))?;
            if decoded_rows > MAX_DECODED_FILINGS {
                return Err(SecEdgarError::Protocol(
                    "decoded filings exceed the per-company limit".into(),
                ));
            }
            merge_records(
                &mut parsed.records,
                &mut parsed.record_index,
                &mut parsed.signatures,
                older.records,
                older.signatures,
            )?;
        }
        sort_records(&mut parsed.records);
        all_records.extend(parsed.records);
    }

    all_records.truncate(request.max_records().get() as usize);
    let source_at = all_records
        .iter()
        .filter_map(CompanyFiling::accepted_at)
        .max()
        .map(str::to_owned);
    let mut provenance = Provenance::new(SOURCE_NAME, &observed_at)?.with_batch_id(&batch_id)?;
    if let Some(source_at) = source_at {
        provenance = provenance.with_source_at(source_at)?;
    }
    Ok(DataBatch::strict(all_records, provenance))
}

fn should_fetch_older(
    descriptor: &OlderFileDescriptor,
    request: &CompanyFilingRequest,
    current_matches: usize,
    remaining_budget: usize,
) -> bool {
    match (request.start(), request.end()) {
        (Some(start), Some(end)) => {
            descriptor.filing_from <= *end && descriptor.filing_to >= *start
        }
        _ => current_matches < remaining_budget,
    }
}

fn fetch_json(client: &SecEdgarClient, url: &str) -> Result<Vec<u8>, SecEdgarError> {
    validate_exact_submissions_url(url)?;
    let request = HttpRequest::new(
        HttpMethod::Get,
        url,
        vec![
            ("User-Agent".into(), client.user_agent.as_str().into()),
            ("Accept".into(), "application/json".into()),
            ("Accept-Encoding".into(), "identity".into()),
        ],
        vec![],
    )?;
    client.gate.wait_for_turn()?;
    let response = client
        .transport
        .execute(&request)
        .map_err(map_transport_error)?;
    let response = submissions_policy(DEFAULT_TIMEOUT)?
        .validate_response_for(&request, response)
        .map_err(map_transport_error)?;
    Ok(response.body().to_vec())
}

fn map_transport_error(error: TransportError) -> SecEdgarError {
    match error {
        TransportError::HttpStatus { status: 403 } => {
            SecEdgarError::Authentication("SEC rejected the supplied descriptive User-Agent".into())
        }
        other => SecEdgarError::Transport(other),
    }
}

fn main_submissions_url(cik: &str) -> String {
    format!("https://data.sec.gov/submissions/CIK{cik}.json")
}

fn older_submissions_url(filename: &str) -> Result<String, SecEdgarError> {
    if filename.contains('/')
        || filename.contains('\\')
        || filename.contains('?')
        || filename.contains('#')
        || filename.chars().any(char::is_control)
    {
        return Err(SecEdgarError::Protocol(
            "older submissions filename is not one exact path segment".into(),
        ));
    }
    Ok(format!("https://data.sec.gov/submissions/{filename}"))
}

fn validate_exact_submissions_url(url: &str) -> Result<(), SecEdgarError> {
    let Some(path) = url.strip_prefix("https://data.sec.gov/submissions/") else {
        return Err(SecEdgarError::InvalidRequest(
            "SEC transport permits only the exact submissions host and path".into(),
        ));
    };
    if path.is_empty()
        || path.contains('/')
        || path.contains('\\')
        || path.contains('?')
        || path.contains('#')
        || path.contains('@')
        || path.chars().any(char::is_control)
    {
        return Err(SecEdgarError::InvalidRequest(
            "SEC submissions path is not an exact allowed filename".into(),
        ));
    }
    let is_main = path.len() == "CIK0000000000.json".len()
        && path.starts_with("CIK")
        && path.ends_with(".json")
        && path[3..13].bytes().all(|byte| byte.is_ascii_digit());
    let is_older = path.len() == "CIK0000000000-submissions-000.json".len()
        && path.starts_with("CIK")
        && path.ends_with(".json")
        && path[3..13].bytes().all(|byte| byte.is_ascii_digit())
        && &path[13..26] == "-submissions-"
        && path[26..29].bytes().all(|byte| byte.is_ascii_digit());
    if !is_main && !is_older {
        return Err(SecEdgarError::InvalidRequest(
            "SEC submissions filename is outside the exact allowlist".into(),
        ));
    }
    Ok(())
}

fn now() -> Result<String, SecEdgarError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| SecEdgarError::Protocol(error.to_string()))
}

#[cfg(test)]
mod tests;
