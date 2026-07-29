use magic_imf_rs::ImfClient;
use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ImfClient::new()?;
    let request = EconomicSeriesRequest::new(
        vec![
            EconomicSeriesKey::new(ProviderId::Imf, "WEO/USA", "NGDP_RPCH")?,
            EconomicSeriesKey::new(ProviderId::Imf, "WEO/CHN", "NGDP_RPCH")?,
        ],
        EconomicPeriod::year(2024)?,
        EconomicPeriod::year(2025)?,
        PositiveU32::new(4)?,
    )?;
    let batch = client.economic_series(&request)?;
    println!(
        "IMF diagnostic records={}; live admission remains false",
        batch.records().len()
    );
    Ok(())
}
