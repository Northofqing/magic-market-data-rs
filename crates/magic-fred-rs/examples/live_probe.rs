use magic_fred_rs::FredClient;
use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesRequest, PositiveU32, ProviderId,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("FRED_API_KEY").unwrap_or_default();
    let client = FredClient::new(key)?;
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Fred, "fred", "GDP")?],
        EconomicPeriod::quarter(2025, 1)?,
        EconomicPeriod::quarter(2025, 4)?,
        PositiveU32::new(4)?,
    )?;
    let batch = client.probe_economic_series(&request)?;
    println!(
        "FRED diagnostic records={}; live admission remains false",
        batch.records().len()
    );
    Ok(())
}
