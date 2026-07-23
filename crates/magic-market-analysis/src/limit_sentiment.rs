use crate::AnalysisError;
use magic_market_core::{LimitPoolEntry, LimitPoolKind, Ratio, RatioUnit};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct LimitSentiment {
    pub upper_count: u32,
    pub broken_count: u32,
    pub lower_count: u32,
    pub previous_upper_count: u32,
    pub seal_rate: Option<Ratio>,
}

/// Aggregates raw limit-pool records without inventing a zero-denominator rate.
pub fn limit_sentiment(entries: &[LimitPoolEntry]) -> Result<LimitSentiment, AnalysisError> {
    let mut result = LimitSentiment {
        upper_count: 0,
        broken_count: 0,
        lower_count: 0,
        previous_upper_count: 0,
        seal_rate: None,
    };
    let mut identities = HashSet::new();
    let expected_date = entries.first().map(|entry| &entry.trading_date);
    for entry in entries {
        if Some(&entry.trading_date) != expected_date {
            return Err(AnalysisError::InvalidInput(
                "limit-pool entries must share a trading date".into(),
            ));
        }
        if !identities.insert((entry.kind, entry.instrument.clone())) {
            return Err(AnalysisError::InvalidInput(
                "duplicate limit-pool identity".into(),
            ));
        }
        match entry.kind {
            LimitPoolKind::Upper => result.upper_count += 1,
            LimitPoolKind::Broken => result.broken_count += 1,
            LimitPoolKind::Lower => result.lower_count += 1,
            LimitPoolKind::PreviousUpper => result.previous_upper_count += 1,
        }
    }
    let seal_denominator = result.upper_count + result.broken_count;
    if seal_denominator > 0 {
        result.seal_rate = Some(Ratio::new(
            f64::from(result.upper_count) / f64::from(seal_denominator) * 100.0,
            RatioUnit::Percent,
        )?);
    }
    Ok(result)
}
