use magic_fred_rs::FredClient;
use magic_market_core::{EconomicReleaseScheduleRequest, IsoDate, PositiveU32};
use time::{Duration, OffsetDateTime};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("FRED_API_KEY").unwrap_or_default();
    let today = OffsetDateTime::now_utc().date();
    let end = today
        .checked_add(Duration::days(30))
        .ok_or("FRED release schedule probe date overflow")?;
    let request = EconomicReleaseScheduleRequest::new(
        IsoDate::new(today.to_string())?,
        IsoDate::new(end.to_string())?,
        PositiveU32::new(20)?,
    )?;
    let batch = FredClient::new(api_key)?.probe_economic_release_schedule(&request)?;
    println!(
        "FRED economic_release_schedule status=diagnostic complete={} records={} start={} end={} source_at={:?}",
        batch.quality().is_complete(),
        batch.records().len(),
        request.start(),
        request.end(),
        batch.provenance().source_at(),
    );
    for record in batch.records().iter().take(2) {
        println!(
            "release_id={} release_date={} name={}",
            record.release_id().get(),
            record.release_date(),
            record.release_name()
        );
    }
    Ok(())
}
