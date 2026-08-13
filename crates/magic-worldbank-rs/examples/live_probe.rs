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
    let batch = client.economic_series(&request)?;
    for record in batch.records() {
        println!(
            "worldbank_record code={} period={} value={:?} unit={} source_at={:?}",
            record.series().code(),
            record.period().as_year().unwrap_or_default(),
            record.value().map(|value| value.get()),
            record.unit(),
            record.evidence().source_at(),
        );
    }
    println!(
        "worldbank_probe_records={} admission_scope=source:2/country:USA/NY.GDP.MKTP.CD/2024",
        batch.records().len()
    );
    println!("live_probe_status=passed");
    Ok(())
}
