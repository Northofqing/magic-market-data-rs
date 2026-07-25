use magic_gov_rs::GovClient;
use magic_market_core::{PolicyDocuments, PolicyRequest, PositiveU32};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let query = std::env::var("MAGIC_GOV_POLICY_QUERY").unwrap_or_else(|_| "金融".into());
    let limit = std::env::var("MAGIC_GOV_POLICY_LIMIT")
        .unwrap_or_else(|_| "5".into())
        .parse::<u32>()?;
    let request =
        PolicyRequest::new(PositiveU32::new(1)?, PositiveU32::new(limit)?)?.with_query(query)?;
    let client = GovClient::new()?;

    println!(
        "provider=state-council policy_capabilities={:?}",
        GovClient::policy_capabilities()
    );
    let batch = client.policy_documents(&request)?;
    if batch.records().is_empty() || batch.records().len() > limit as usize {
        return Err("official policy response cardinality is invalid".into());
    }
    println!(
        "records={} provenance={:?} quality={:?}",
        batch.records().len(),
        batch.provenance(),
        batch.quality()
    );
    for record in batch.records() {
        println!(
            "document_id={} title={} organization={} published_date={} category={:?} document_number={:?} canonical_url={} source_at={:?} observed_at={} provider={:?} batch_id={}",
            record.document_id,
            record.title,
            record.organization,
            record.published_date,
            record.category.as_ref().map(|value| value.as_str()),
            record.document_number.as_ref().map(|value| value.as_str()),
            record.canonical_url,
            record.evidence.source_at(),
            record.evidence.observed_at(),
            record.evidence.provider(),
            record.evidence.batch_id()
        );
    }
    println!("live_probe_status=passed");
    Ok(())
}
