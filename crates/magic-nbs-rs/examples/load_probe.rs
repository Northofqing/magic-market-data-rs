use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_nbs_rs::NbsClient;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = NbsClient::new(Duration::from_secs(20))?;
    for namespace in ["national-cpi-yoy", "beijing-cpi-yoy"] {
        let request = EconomicSeriesRequest::new(
            vec![EconomicSeriesKey::new(
                ProviderId::Nbs,
                namespace,
                "headline",
            )?],
            EconomicPeriod::month(2026, 7)?,
            EconomicPeriod::month(2026, 7)?,
            PositiveU32::new(1)?,
        )?;
        for _ in 0..3 {
            client.economic_series(&request)?;
        }
    }
    println!("NBS national and Beijing CPI serial load probe completed three calls each");
    Ok(())
}
