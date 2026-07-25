use magic_market_core::PositiveU32;
use magic_yonhap_rs::{YonhapChannel, YonhapClient};
use std::error::Error;

#[derive(Debug, PartialEq, Eq)]
struct ProbeConfig {
    channel: YonhapChannel,
    limit: u32,
    headline_match: Option<String>,
}

fn parse_channel(value: Option<&str>) -> Result<YonhapChannel, String> {
    match value.unwrap_or("rolling") {
        "rolling" => Ok(YonhapChannel::Rolling),
        "politics" => Ok(YonhapChannel::Politics),
        "economy" => Ok(YonhapChannel::Economy),
        "society" => Ok(YonhapChannel::Society),
        "culture-sports" => Ok(YonhapChannel::CultureSports),
        "north-korea" => Ok(YonhapChannel::NorthKorea),
        "china-korea" => Ok(YonhapChannel::ChinaKorea),
        value => Err(format!(
            "MAGIC_YONHAP_CHANNEL has unsupported value {value:?}"
        )),
    }
}

fn parse_limit(value: Option<&str>) -> Result<u32, String> {
    let value = value.unwrap_or("20");
    let parsed = value
        .parse::<u32>()
        .map_err(|error| format!("MAGIC_YONHAP_LIMIT must be an integer: {error}"))?;
    if (1..=50).contains(&parsed) {
        Ok(parsed)
    } else {
        Err("MAGIC_YONHAP_LIMIT must be between 1 and 50".into())
    }
}

fn parse_match(value: Option<String>) -> Result<Option<String>, String> {
    match value {
        Some(value) if value.is_empty() => {
            Err("MAGIC_YONHAP_MATCH must not be empty when present".into())
        }
        value => Ok(value),
    }
}

fn parse_config(
    channel: Option<&str>,
    limit: Option<&str>,
    headline_match: Option<String>,
) -> Result<ProbeConfig, String> {
    Ok(ProbeConfig {
        channel: parse_channel(channel)?,
        limit: parse_limit(limit)?,
        headline_match: parse_match(headline_match)?,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let channel = std::env::var("MAGIC_YONHAP_CHANNEL").ok();
    let limit = std::env::var("MAGIC_YONHAP_LIMIT").ok();
    let config = parse_config(
        channel.as_deref(),
        limit.as_deref(),
        std::env::var("MAGIC_YONHAP_MATCH").ok(),
    )?;
    let capabilities = YonhapClient::content_capabilities();
    println!(
        "provider=yonhap-cn-rss-v1 channel={} capabilities={capabilities:?}",
        config.channel.slug()
    );

    let client = YonhapClient::for_channel(config.channel)?;
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
        return Err("Yonhap metadata probe exposed summary or content".into());
    }
    if let Some(expected) = config.headline_match.as_deref() {
        let matched = batch
            .records()
            .iter()
            .any(|item| item.title.as_str().contains(expected));
        if !matched {
            return Err(format!(
                "current bounded {} feed does not contain case-sensitive title text {expected:?}",
                config.channel.slug()
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
    fn defaults_are_bounded_and_rolling() {
        assert_eq!(
            parse_config(None, None, None).unwrap(),
            ProbeConfig {
                channel: YonhapChannel::Rolling,
                limit: 20,
                headline_match: None,
            }
        );
    }

    #[test]
    fn every_documented_channel_spelling_is_supported() {
        let cases = [
            ("rolling", YonhapChannel::Rolling),
            ("politics", YonhapChannel::Politics),
            ("economy", YonhapChannel::Economy),
            ("society", YonhapChannel::Society),
            ("culture-sports", YonhapChannel::CultureSports),
            ("north-korea", YonhapChannel::NorthKorea),
            ("china-korea", YonhapChannel::ChinaKorea),
        ];
        for (value, expected) in cases {
            assert_eq!(parse_channel(Some(value)).unwrap(), expected);
        }
        assert!(parse_channel(Some("business")).is_err());
        assert!(parse_channel(Some("Economy")).is_err());
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
