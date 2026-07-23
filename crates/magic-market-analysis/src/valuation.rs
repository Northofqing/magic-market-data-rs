use crate::AnalysisError;
use magic_market_core::{
    FiniteNumber, NonEmptyText, Price, ProviderId, Ratio, RatioUnit, SourceEvidence,
};

/// Price divided by positive forward EPS.
pub fn forward_pe(price: Price, eps: FiniteNumber) -> Result<FiniteNumber, AnalysisError> {
    if eps.get() <= 0.0 {
        return Err(AnalysisError::InvalidInput(
            "forward EPS must be positive".into(),
        ));
    }
    Ok(FiniteNumber::new(price.get() / eps.get())?)
}

/// PE divided by a positive percentage growth rate.
pub fn peg(pe: FiniteNumber, growth: Ratio) -> Result<FiniteNumber, AnalysisError> {
    let growth_percent = match growth.unit() {
        RatioUnit::Percent => growth.get(),
        RatioUnit::Decimal => growth.get() * 100.0,
    };
    if growth_percent <= 0.0 {
        return Err(AnalysisError::InvalidInput(
            "growth must be positive".into(),
        ));
    }
    Ok(FiniteNumber::new(pe.get() / growth_percent)?)
}

/// Years for earnings growth to reduce a fixed-price PE to a configured target.
pub fn pe_digestion_years(
    current_pe: FiniteNumber,
    target_pe: FiniteNumber,
    annual_growth: Ratio,
) -> Result<FiniteNumber, AnalysisError> {
    if current_pe.get() <= 0.0 || target_pe.get() <= 0.0 {
        return Err(AnalysisError::InvalidInput(
            "current and target PE must be positive".into(),
        ));
    }
    if current_pe.get() <= target_pe.get() {
        return Ok(FiniteNumber::new(0.0)?);
    }
    let growth_decimal = match annual_growth.unit() {
        RatioUnit::Decimal => annual_growth.get(),
        RatioUnit::Percent => annual_growth.get() / 100.0,
    };
    if growth_decimal <= 0.0 {
        return Err(AnalysisError::InvalidInput(
            "annual growth must be positive".into(),
        ));
    }
    let years = (current_pe.get() / target_pe.get()).ln() / (1.0 + growth_decimal).ln();
    Ok(FiniteNumber::new(years)?)
}

/// One derived scalar with all source inputs retained.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributedValue {
    pub name: NonEmptyText,
    pub value: FiniteNumber,
    pub provider: ProviderId,
    pub inputs: Vec<SourceEvidence>,
}

impl AttributedValue {
    pub fn new(
        name: NonEmptyText,
        value: FiniteNumber,
        inputs: Vec<SourceEvidence>,
    ) -> Result<Self, AnalysisError> {
        if inputs.is_empty() {
            return Err(AnalysisError::InvalidInput(
                "attributed value requires source evidence".into(),
            ));
        }
        Ok(Self {
            name,
            value,
            provider: ProviderId::LocalAnalysis,
            inputs,
        })
    }
}
