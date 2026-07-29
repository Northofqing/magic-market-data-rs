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
            "money-supply",
            "M2",
        )?],
        EconomicPeriod::month(2024, 1)?,
        EconomicPeriod::month(2024, 12)?,
        PositiveU32::new(12)?,
    )?;
    let client = PbcClient::new(Duration::from_secs(10))?;
    for _ in 0..3 {
        let _ = client.probe_money_supply(&request)?;
    }
    let status = verify_serial_load(&client.load_probe_snapshot()?, Duration::from_secs(1))?;
    println!("PBC three-request load probe: {status}");
    Ok(())
}
