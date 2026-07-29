use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesRequest, PositiveU32, ProviderId,
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
    let batch = PbcClient::new(Duration::from_secs(10))?.probe_money_supply(&request)?;
    for row in batch.records() {
        println!(
            "{} {:?} {:?} {} batch={}",
            row.series().code(),
            row.period(),
            row.value().map(|value| value.get()),
            row.unit(),
            row.evidence().batch_id()
        );
    }
    Ok(())
}
