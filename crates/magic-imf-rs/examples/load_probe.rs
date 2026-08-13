use magic_imf_rs::ImfClient;
use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesRequest, PositiveU32, ProviderId,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ImfClient::new()?;
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(
            ProviderId::Imf,
            "WEO/USA",
            "NGDP_RPCH",
        )?],
        EconomicPeriod::year(2024)?,
        EconomicPeriod::year(2025)?,
        PositiveU32::new(2)?,
    )?;
    let mut records = 0usize;
    let mut last_error = None;
    for call in 1..=3 {
        match client.probe_economic_series(&request) {
            Ok(batch) => {
                records = records
                    .checked_add(batch.records().len())
                    .ok_or("record counter overflow")?;
                println!("imf_load_call={call} status=diagnostic_complete_unadmitted");
            }
            Err(error) => {
                println!("imf_load_call={call} status=failed error={error:?}");
                last_error = Some(error);
            }
        }
    }
    if let Some(error) = last_error {
        return Err(Box::new(error));
    }
    println!(
        "imf_load_probe_status=diagnostic_complete_unadmitted calls=3 records={records} admission=false"
    );
    Ok(())
}
