use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_worldbank_rs::WorldBankClient;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = WorldBankClient::new()?;
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::WorldBank,
            "source:2/country:USA",
            "NY.GDP.MKTP.CD",
        )?],
        EconomicPeriod::year(2024)?,
        EconomicPeriod::year(2024)?,
        PositiveU32::new(1)?,
    )?;
    let mut records = 0usize;
    for _ in 0..3 {
        records = records
            .checked_add(client.economic_series(&request)?.records().len())
            .ok_or("record counter overflow")?;
    }
    println!(
        "worldbank_load_probe_calls=3 records={} admission_scope=source:2/country:USA/NY.GDP.MKTP.CD/2024",
        records
    );
    println!("load_probe_status=passed");
    Ok(())
}
