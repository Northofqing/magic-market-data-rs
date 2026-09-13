use magic_fred_rs::FredClient;
use magic_market_core::{EconomicReleaseScheduleRequest, IsoDate, PositiveU32};
use time::{Duration, OffsetDateTime};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("FRED_API_KEY").unwrap_or_default();
    let today = OffsetDateTime::now_utc().date();
    let end = today
        .checked_add(Duration::days(30))
        .ok_or("FRED release schedule load probe date overflow")?;
    let request = EconomicReleaseScheduleRequest::new(
        IsoDate::new(today.to_string())?,
        IsoDate::new(end.to_string())?,
        PositiveU32::new(20)?,
    )?;
    let client = FredClient::new(api_key)?;
    for call in 1..=3 {
        let batch = client.probe_economic_release_schedule(&request)?;
        println!(
            "FRED economic_release_schedule load call={} complete={} records={} source_at={:?}",
            call,
            batch.quality().is_complete(),
            batch.records().len(),
            batch.provenance().source_at(),
        );
    }
    Ok(())
}
