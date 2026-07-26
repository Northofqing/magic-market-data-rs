use magic_cninfo_rs::CninfoClient;
use magic_market_core::{
    Announcement, DataBatch, IsoDate, MarketAnnouncementRequest, MarketAnnouncements, PositiveU32,
    ProbeStatus, ProviderId,
};
use std::collections::HashSet;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    match run_probe() {
        Ok(status) => {
            println!("market_announcements_probe_status={status}");
            println!("admitted={}", status.satisfies_capability());
            Ok(())
        }
        Err(error) => {
            println!("market_announcements_probe_status={}", ProbeStatus::Failed);
            Err(error)
        }
    }
}

fn run_probe() -> Result<ProbeStatus, Box<dyn Error>> {
    let date =
        std::env::var("MAGIC_CNINFO_MARKET_DATE").unwrap_or_else(|_| "2026-07-24".to_owned());
    let date = IsoDate::new(date)?;
    let limit = std::env::var("MAGIC_CNINFO_MARKET_LIMIT")
        .unwrap_or_else(|_| "3".to_owned())
        .parse::<u32>()?;
    let request =
        MarketAnnouncementRequest::new(date.clone(), date.clone(), PositiveU32::new(limit)?)?;
    let client = CninfoClient::new()?;

    println!("provider=cninfo");
    println!("operation=native_whole_market_announcements");
    println!("request_date={}", date.as_str());
    println!("request_limit={}", request.limit().get());

    let batch = client.market_announcements(&request)?;
    print_batch(&batch);
    validate_probe_batch(&batch, &request)
}

fn validate_probe_batch(
    batch: &DataBatch<Announcement>,
    request: &MarketAnnouncementRequest,
) -> Result<ProbeStatus, Box<dyn Error>> {
    if !batch.quality().is_complete() {
        return Err(format!(
            "market announcement batch is incomplete: {:?}",
            batch.quality().issues()
        )
        .into());
    }
    if batch.provenance().source() != "cninfo-market" {
        return Err("market announcement batch has unexpected source".into());
    }
    let batch_id = batch
        .provenance()
        .batch_id()
        .ok_or("market announcement batch has no batch ID")?;
    if batch.records().is_empty() {
        if batch.provenance().source_at().is_some() || !batch_id.contains("total=0") {
            return Err("empty batch lacks exact CNInfo zero-total evidence".into());
        }
        return Ok(ProbeStatus::VerifiedEmpty);
    }
    if batch.records().len() > request.limit().get() as usize {
        return Err("market announcement batch exceeds requested limit".into());
    }
    if batch.provenance().source_at() != Some(batch.records()[0].published_at.as_str()) {
        return Err("batch source_at is not the newest publication time".into());
    }

    let mut ids = HashSet::with_capacity(batch.records().len());
    let mut previous_source_at: Option<&str> = None;
    for record in batch.records() {
        let source_at = record
            .evidence
            .source_at()
            .ok_or("record is missing provider publication time")?;
        if record.evidence.provider() != ProviderId::Cninfo
            || record.evidence.observed_at() != batch.provenance().fetched_at()
            || record.evidence.batch_id() != batch_id
            || source_at != record.published_at.as_str()
        {
            return Err("record evidence does not match normalized announcement facts".into());
        }
        let source_date = source_at
            .get(..10)
            .ok_or("record source timestamp has no date")?;
        if source_date < request.start().as_str() || source_date > request.end().as_str() {
            return Err("record source date is outside requested range".into());
        }
        if previous_source_at.is_some_and(|previous| source_at > previous) {
            return Err("market announcement records are not newest-first".into());
        }
        previous_source_at = Some(source_at);
        if !ids.insert(record.announcement_id.as_str()) {
            return Err("market announcement IDs are not unique".into());
        }
    }
    Ok(ProbeStatus::Admitted)
}

fn print_batch(batch: &DataBatch<Announcement>) {
    println!("records={}", batch.records().len());
    println!("batch_source={}", batch.provenance().source());
    println!(
        "batch_source_at={}",
        batch.provenance().source_at().unwrap_or("<absent>")
    );
    println!(
        "batch_id={}",
        batch.provenance().batch_id().unwrap_or("<absent>")
    );
    println!("quality_complete={}", batch.quality().is_complete());
    for (index, record) in batch.records().iter().enumerate() {
        println!(
            "record[{index}]={:?}:{} id={} published_at={} source_at={} title={}",
            record.instrument.exchange(),
            record.instrument.code(),
            record.announcement_id.as_str(),
            record.published_at.as_str(),
            record.evidence.source_at().unwrap_or("<absent>"),
            record.title.as_str()
        );
    }
}

#[cfg(test)]
#[path = "../tests/unit/market_announcements_probe_tests.rs"]
mod tests;
