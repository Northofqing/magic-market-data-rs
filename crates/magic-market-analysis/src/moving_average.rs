use crate::AnalysisError;
use magic_market_core::{Bar, PositiveU32, Price};

/// Computes an ascending simple moving average with explicit warm-up `None`s.
pub fn simple_moving_average(
    bars: &[Bar],
    window: PositiveU32,
) -> Result<Vec<Option<Price>>, AnalysisError> {
    let window = usize::try_from(window.get())
        .map_err(|_| AnalysisError::InvalidInput("window does not fit usize".into()))?;
    if bars.is_empty() {
        return Err(AnalysisError::InvalidInput("bars must not be empty".into()));
    }
    if window > bars.len() {
        return Err(AnalysisError::InvalidInput(
            "window must not exceed bar count".into(),
        ));
    }
    for pair in bars.windows(2) {
        if pair[0].bar_start() >= pair[1].bar_start() {
            return Err(AnalysisError::InvalidInput(
                "bars must be strictly chronological".into(),
            ));
        }
        if pair[0].instrument() != pair[1].instrument()
            || pair[0].interval() != pair[1].interval()
            || pair[0].adjustment() != pair[1].adjustment()
        {
            return Err(AnalysisError::InvalidInput(
                "bars must share instrument, interval and adjustment".into(),
            ));
        }
    }

    let mut result = Vec::with_capacity(bars.len());
    let mut sum = 0.0;
    for (index, bar) in bars.iter().enumerate() {
        sum += bar.close().get();
        if index >= window {
            sum -= bars[index - window].close().get();
        }
        if index + 1 < window {
            result.push(None);
        } else {
            result.push(Some(Price::new(sum / window as f64)?));
        }
    }
    Ok(result)
}
