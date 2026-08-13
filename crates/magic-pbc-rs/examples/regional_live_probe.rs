use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesRequest, PositiveU32, ProviderId,
};
use magic_pbc_rs::PbcClient;
use std::time::Duration;

fn request() -> Result<EconomicSeriesRequest, magic_market_core::CoreError> {
    EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::Pbc,
            "regional-social-financing-flow",
            "AFRE_FLOW",
        )?],
        EconomicPeriod::quarter(2025, 1)?,
        EconomicPeriod::quarter(2025, 1)?,
        PositiveU32::new(31)?,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let batch =
        PbcClient::new(Duration::from_secs(10))?.probe_regional_social_financing(&request()?)?;
    println!(
        "PBC regional social-financing live probe: records={} first_region={} first_value={} unit={} scale={}",
        batch.records().len(),
        batch.records()[0].region_name().unwrap_or("missing"),
        batch.records()[0].value().map(|value| value.get()).unwrap_or(f64::NAN),
        batch.records()[0].unit(),
        batch.records()[0].scale().unwrap_or("missing"),
    );
    Ok(())
}
