use magic_iwencai_rs::{IwencaiClient, IwencaiError};
use magic_market_core::{
    PositiveU32, ProbeStatus, SemanticChannel, SemanticSearch, SemanticSearchRequest,
};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let capabilities = IwencaiClient::research_capabilities();
    let configured_key = configured_key_present();
    println!(
        "provider=iwencai-openapi capabilities={capabilities:?} configured_key={}",
        configured_key
    );
    if !configured_key {
        println!("live_probe_status={}", ProbeStatus::SkippedMissingSecret);
        return Err(Box::new(IwencaiError::Authentication(
            "set MAGIC_IWENCAI_API_KEY to an authorized SkillHub API key".into(),
        )));
    }
    let client = IwencaiClient::from_env()?;
    let query = std::env::var("MAGIC_IWENCAI_QUERY")
        .unwrap_or_else(|_| "人形机器人 行星滚柱丝杠 2026".to_owned());
    let request =
        SemanticSearchRequest::new(query, SemanticChannel::Report, PositiveU32::new(10)?)?;
    let batch = match client.semantic_search(&request) {
        Ok(batch) => batch,
        Err(error) => {
            println!("live_probe_status={}", ProbeStatus::Failed);
            return Err(Box::new(error));
        }
    };
    println!(
        "source={} source_at={:?} fetched_at={} batch_id={:?} complete={} records={}",
        batch.provenance().source(),
        batch.provenance().source_at(),
        batch.provenance().fetched_at(),
        batch.provenance().batch_id(),
        batch.quality().is_complete(),
        batch.records().len()
    );
    for document in batch.records() {
        println!(
            "document_id={} channel={:?} title={:?} excerpt={:?} canonical_url={} published_at={:?} evidence_provider={:?} evidence_source_at={:?} evidence_observed_at={} evidence_batch_id={}",
            document.document_id,
            document.channel,
            document.title.as_str(),
            document.excerpt.as_ref().map(|value| value.as_str()),
            document.canonical_url,
            document.published_at.as_ref().map(|value| value.as_str()),
            document.evidence.provider(),
            document.evidence.source_at(),
            document.evidence.observed_at(),
            document.evidence.batch_id()
        );
    }
    println!("admitted={}", capabilities.semantic_search);
    println!(
        "live_probe_status={}",
        successful_probe_status(capabilities.semantic_search)
    );
    Ok(())
}

fn configured_key_present() -> bool {
    std::env::var_os("MAGIC_IWENCAI_API_KEY").is_some()
        || std::env::var_os("IWENCAI_API_KEY").is_some()
}

fn successful_probe_status(capability_admitted: bool) -> ProbeStatus {
    if capability_admitted {
        ProbeStatus::Admitted
    } else {
        ProbeStatus::DiagnosticCompleteUnadmitted
    }
}

#[cfg(test)]
#[path = "../tests/unit/live_probe_tests.rs"]
mod tests;
