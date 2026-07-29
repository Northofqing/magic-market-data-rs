use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesRequest, PositiveU32, ProviderId,
};
use magic_worldbank_rs::{WorldBankClient, WorldBankError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() != Some("--diagnostic") {
        return Err("pass --diagnostic to execute the non-admission World Bank probe".into());
    }
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
    match client.probe_economic_series(&request) {
        Err(WorldBankError::Protocol(message)) if message.contains("unit") => {
            println!("World Bank structured-unit diagnostic confirmed; admission remains false");
            Ok(())
        }
        Ok(_) => Err("World Bank diagnostic unexpectedly produced an admitted batch".into()),
        Err(error) => Err(Box::new(error)),
    }
}
