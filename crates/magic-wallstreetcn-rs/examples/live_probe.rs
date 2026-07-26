use magic_market_core::PositiveU32;
use magic_wallstreetcn_rs::WallstreetCnClient;
use std::error::Error;

#[derive(Debug, PartialEq, Eq)]
struct ProbeConfig {
    limit: u32,
    headline_match: Option<String>,
}

fn parse_limit(value: Option<&str>) -> Result<u32, String> {
    let value = value.unwrap_or("20");
    let parsed = value
        .parse::<u32>()
        .map_err(|error| format!("MAGIC_WALLSTREETCN_LIMIT must be an integer: {error}"))?;
    if (1..=50).contains(&parsed) {
        Ok(parsed)
    } else {
        Err("MAGIC_WALLSTREETCN_LIMIT must be between 1 and 50".into())
    }
}

fn parse_match(value: Option<String>) -> Result<Option<String>, String> {
    match value {
        Some(value) if value.is_empty() => {
            Err("MAGIC_WALLSTREETCN_MATCH must not be empty when present".into())
        }
        value => Ok(value),
    }
}

fn parse_config(
    limit: Option<&str>,
    headline_match: Option<String>,
) -> Result<ProbeConfig, String> {
    Ok(ProbeConfig {
        limit: parse_limit(limit)?,
        headline_match: parse_match(headline_match)?,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let limit = std::env::var("MAGIC_WALLSTREETCN_LIMIT").ok();
    let config = parse_config(
        limit.as_deref(),
        std::env::var("MAGIC_WALLSTREETCN_MATCH").ok(),
    )?;
    let capabilities = WallstreetCnClient::content_capabilities();
    println!("provider=wallstreetcn-rss-v1 capabilities={capabilities:?}");

    let client = WallstreetCnClient::new()?;
    let batch = client.probe_global_news(PositiveU32::new(config.limit)?)?;
    println!(
        "source={} source_at={:?} fetched_at={} batch_id={:?} complete={} records={}",
        batch.provenance().source(),
        batch.provenance().source_at(),
        batch.provenance().fetched_at(),
        batch.provenance().batch_id(),
        batch.quality().is_complete(),
        batch.records().len()
    );
    for item in batch.records() {
        let topics = item
            .topics
            .iter()
            .map(|topic| topic.as_str())
            .collect::<Vec<_>>();
        println!(
            "item_id={} title={:?} publisher={} canonical_url={} published_at={} instruments={} topics={topics:?} language={} summary_absent={} content_absent={} evidence_provider={:?} evidence_source_at={:?} evidence_observed_at={} evidence_batch_id={}",
            item.item_id,
            item.title.as_str(),
            item.publisher,
            item.canonical_url,
            item.published_at,
            item.instruments.len(),
            item.language,
            item.summary.is_none(),
            item.content.is_none(),
            item.evidence.provider(),
            item.evidence.source_at(),
            item.evidence.observed_at(),
            item.evidence.batch_id()
        );
    }
    if batch
        .records()
        .iter()
        .any(|item| item.summary.is_some() || item.content.is_some())
    {
        return Err("WallstreetCN metadata probe exposed summary or content".into());
    }
    if let Some(expected) = config.headline_match.as_deref() {
        let matched = batch
            .records()
            .iter()
            .any(|item| item.title.as_str().contains(expected));
        if !matched {
            return Err(format!(
                "current bounded feed does not contain case-sensitive title text {expected:?}"
            )
            .into());
        }
        println!("headline_match={expected:?} matched=true");
    }
    println!("live_probe_status=passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_bounded() {
        assert_eq!(
            parse_config(None, None).unwrap(),
            ProbeConfig {
                limit: 20,
                headline_match: None,
            }
        );
    }

    #[test]
    fn limit_and_match_inputs_are_strict() {
        assert_eq!(parse_limit(Some("1")).unwrap(), 1);
        assert_eq!(parse_limit(Some("50")).unwrap(), 50);
        assert!(parse_limit(Some("0")).is_err());
        assert!(parse_limit(Some("51")).is_err());
        assert!(parse_limit(Some("x")).is_err());
        assert!(parse_match(Some(String::new())).is_err());
        assert_eq!(
            parse_match(Some("半导体".into())).unwrap().as_deref(),
            Some("半导体")
        );
    }
}
