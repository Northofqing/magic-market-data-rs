use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_nbs_rs::NbsClient;
use std::time::Duration;

fn request(namespace: &str) -> Result<EconomicSeriesRequest, magic_market_core::CoreError> {
    EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::Nbs,
            namespace,
            "headline",
        )?],
        EconomicPeriod::month(2026, 7)?,
        EconomicPeriod::month(2026, 7)?,
        PositiveU32::new(1)?,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = NbsClient::new(Duration::from_secs(20))?;
    for (label, namespace) in [
        ("national", "national-cpi-yoy"),
        ("Beijing", "beijing-cpi-yoy"),
    ] {
        let batch = client.economic_series(&request(namespace)?)?;
        let row = &batch.records()[0];
        println!(
            "NBS {label} CPI live: period={:?} value={} unit={} admission=admitted",
            row.period().as_month(),
            row.value().ok_or("NBS CPI value missing")?.get(),
            row.unit()
        );
    }
    Ok(())
}
