use magic_market_core::{
    CompanyFilingRequest, CompanyFilingsProvider, NonEmptyText, PositiveU32, SecCompanyIdentity,
};
use magic_sec_rs::SecEdgarClient;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let user_agent = std::env::var("SEC_USER_AGENT")
        .map_err(|_| "SEC_USER_AGENT must be application/version contact@example.com")?;
    let client = SecEdgarClient::new(user_agent)?;
    let request = CompanyFilingRequest::new(
        vec![SecCompanyIdentity::new("320193", Some("AAPL"))?],
        ["10-K", "10-Q", "8-K"]
            .into_iter()
            .map(NonEmptyText::new)
            .collect::<Result<Vec<_>, _>>()?,
        None,
        None,
        PositiveU32::new(5)?,
    )?;
    let batch = client.company_filings(&request)?;
    for record in batch.records() {
        println!(
            "{} {} {} {} {} {} {} {} {} {} {} {}",
            record.company().cik(),
            record.company().ticker().unwrap_or("-"),
            record.company_name(),
            record.form(),
            record.filing_date().as_str(),
            record
                .report_period()
                .map_or("-", magic_market_core::IsoDate::as_str),
            record.accession().as_str(),
            record.filing_index_url().as_str(),
            record.primary_document_url().as_str(),
            record.accepted_at().unwrap_or("-"),
            record.evidence().observed_at(),
            record.evidence().batch_id(),
        );
    }
    Ok(())
}
