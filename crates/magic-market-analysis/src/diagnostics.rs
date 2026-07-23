use crate::AnalysisError;
use magic_market_core::{FiniteNumber, SourceEvidence};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct CrossSourceObservation {
    pub evidence: SourceEvidence,
    pub observed_epoch_millis: i64,
    pub value: Option<FiniteNumber>,
}

impl CrossSourceObservation {
    pub fn new(
        evidence: SourceEvidence,
        observed_epoch_millis: i64,
        value: Option<FiniteNumber>,
    ) -> Self {
        Self {
            evidence,
            observed_epoch_millis,
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrossSourceDiagnostics {
    pub observation_spread_millis: u64,
    pub value_spread: Option<FiniteNumber>,
    pub inputs: Vec<SourceEvidence>,
}

/// Compares one observation per provider and retains every input evidence item.
pub fn cross_source_diagnostics(
    observations: &[CrossSourceObservation],
) -> Result<CrossSourceDiagnostics, AnalysisError> {
    if observations.is_empty() {
        return Err(AnalysisError::InvalidInput(
            "at least one observation is required".into(),
        ));
    }
    let mut providers = HashSet::new();
    for observation in observations {
        if !providers.insert(observation.evidence.provider()) {
            return Err(AnalysisError::InvalidInput(format!(
                "duplicate provider {:?}",
                observation.evidence.provider()
            )));
        }
    }

    let minimum_time = observations
        .iter()
        .map(|observation| observation.observed_epoch_millis)
        .min()
        .unwrap_or_default();
    let maximum_time = observations
        .iter()
        .map(|observation| observation.observed_epoch_millis)
        .max()
        .unwrap_or_default();
    let observation_spread_millis = maximum_time.abs_diff(minimum_time);

    let mut values = observations
        .iter()
        .filter_map(|observation| observation.value.map(FiniteNumber::get));
    let first_value = values.next();
    let value_spread = if let Some(first) = first_value {
        let (minimum, maximum) = values.fold((first, first), |(minimum, maximum), value| {
            (minimum.min(value), maximum.max(value))
        });
        Some(FiniteNumber::new(maximum - minimum)?)
    } else {
        None
    };

    Ok(CrossSourceDiagnostics {
        observation_spread_millis,
        value_spread,
        inputs: observations
            .iter()
            .map(|observation| observation.evidence.clone())
            .collect(),
    })
}
