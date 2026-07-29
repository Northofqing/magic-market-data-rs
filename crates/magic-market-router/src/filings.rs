use crate::{FailoverChain, FailureKind, SourceError, SourceFn};
use magic_market_core::{CompanyFiling, CompanyFilingRequest, CompanyFilingsProvider, ProviderId};
use std::collections::HashSet;
use std::sync::Arc;

pub type CompanyFilingRouter = FailoverChain<CompanyFilingRequest, CompanyFiling>;

pub fn company_filing_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<CompanyFilingRequest, CompanyFiling>
where
    Provider: CompanyFilingsProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.company_filings(request).map_err(&classify)?;
        validate_filing_batch(request, batch.records())?;
        Ok(batch)
    })
}

fn validate_filing_batch(
    request: &CompanyFilingRequest,
    records: &[CompanyFiling],
) -> Result<(), SourceError> {
    if records.len() > request.max_records().get() as usize {
        return quality("company-filing batch exceeds requested max_records");
    }
    let mut identities = HashSet::with_capacity(records.len());
    let mut previous: Option<(usize, &magic_market_core::IsoDate, Option<&str>)> = None;
    for record in records {
        let position = request
            .companies()
            .iter()
            .position(|company| company.cik() == record.company().cik())
            .ok_or_else(|| evidence_error("company-filing batch contains an unrequested CIK"))?;
        let requested = &request.companies()[position];
        if requested
            .ticker()
            .zip(record.company().ticker())
            .is_some_and(|(expected, actual)| expected != actual)
        {
            return evidence("company-filing ticker contradicts the requested company");
        }
        if !request.forms().is_empty()
            && !request
                .forms()
                .iter()
                .any(|form| form.as_str() == record.form())
        {
            return evidence("company-filing record has an unrequested form");
        }
        if request
            .start()
            .is_some_and(|start| record.filing_date() < start)
            || request.end().is_some_and(|end| record.filing_date() > end)
        {
            return evidence("company-filing record is outside the requested date range");
        }
        if !identities.insert((
            record.company().cik().to_owned(),
            record.accession().as_str().to_owned(),
        )) {
            return quality("company-filing batch contains a duplicate CIK/accession identity");
        }
        if let Some((prior_position, prior_date, prior_accepted)) = previous {
            let out_of_order = position < prior_position
                || (position == prior_position && record.filing_date() > prior_date)
                || (position == prior_position
                    && record.filing_date() == prior_date
                    && record.accepted_at() > prior_accepted);
            if out_of_order {
                return quality("company-filing batch is not in canonical order");
            }
        }
        previous = Some((position, record.filing_date(), record.accepted_at()));
    }
    Ok(())
}

fn evidence_error(message: &str) -> SourceError {
    SourceError::try_next(FailureKind::Evidence, message)
}

fn evidence<T>(message: &str) -> Result<T, SourceError> {
    Err(evidence_error(message))
}

fn quality<T>(message: &str) -> Result<T, SourceError> {
    Err(SourceError::try_next(FailureKind::Quality, message))
}
