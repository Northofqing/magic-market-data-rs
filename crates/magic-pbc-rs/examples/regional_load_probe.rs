use magic_market_core::{
    verify_serial_load, EconomicPeriod, EconomicSeriesKey, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_pbc_rs::PbcClient;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::Pbc,
            "regional-social-financing-flow",
            "AFRE_FLOW",
        )?],
        EconomicPeriod::quarter(2025, 1)?,
        EconomicPeriod::quarter(2025, 1)?,
        PositiveU32::new(31)?,
    )?;
    let client = PbcClient::new(Duration::from_secs(10))?;
    for _ in 0..3 {
        let batch = client.probe_regional_social_financing(&request)?;
        if batch.records().len() != 31 {
            return Err("regional load probe returned incomplete region coverage".into());
        }
    }
    let status = verify_serial_load(&client.load_probe_snapshot()?, Duration::from_secs(1))?;
    println!("PBC regional three-request load probe: {status}");
    Ok(())
}
