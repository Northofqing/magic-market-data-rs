use magic_cninfo_rs::CninfoClient;
use magic_market_core::{
    AnnouncementDiscovery, AnnouncementDiscoveryRequest, Announcements, AssetClass, DataBatch,
    Exchange, InstrumentDateRangeRequest, InstrumentId, InvestorQuestions, IsoDate, PositiveU32,
};
use std::error::Error;
use std::fmt::Debug;

fn main() -> Result<(), Box<dyn Error>> {
    let client = CninfoClient::new()?;
    let announcement_instrument =
        equity(std::env::var("MAGIC_CNINFO_CODE").unwrap_or_else(|_| "600396".into()))?;
    let question_instrument =
        equity(std::env::var("MAGIC_CNINFO_QUESTION_CODE").unwrap_or_else(|_| "002594".into()))?;
    let limit = PositiveU32::new(3)?;

    println!("provider=cninfo");
    println!("capabilities={:#?}", CninfoClient::capabilities());

    let mapping = client.organization_mapping(&announcement_instrument)?;
    println!("organization_mapping={mapping:#?}");

    let announcements = InstrumentDateRangeRequest::new(announcement_instrument, limit)?;
    print_batch("announcements", &client.announcements(&announcements)?);

    let discovery_date =
        std::env::var("MAGIC_CNINFO_DISCOVERY_DATE").unwrap_or_else(|_| "2026-07-24".into());
    let discovery_date = IsoDate::new(discovery_date)?;
    let discovery = AnnouncementDiscoveryRequest::new(
        discovery_date.clone(),
        discovery_date,
        PositiveU32::new(10_000)?,
    )?;
    let discovered = client.discover_announcements(&discovery)?;
    if discovered
        .records()
        .iter()
        .any(|record| record.instrument_name.is_none())
    {
        return Err("full-market announcement record is missing its stock name".into());
    }
    print_batch("announcement_discovery", &discovered);

    let questions = InstrumentDateRangeRequest::new(question_instrument, limit)?;
    print_batch(
        "investor_questions",
        &client.investor_questions(&questions)?,
    );
    Ok(())
}

fn equity(code: String) -> Result<InstrumentId, Box<dyn Error>> {
    let exchange = match code.as_bytes().first().copied() {
        Some(b'6') => Exchange::Shanghai,
        Some(b'0') | Some(b'3') => Exchange::Shenzhen,
        Some(b'4') | Some(b'8') => Exchange::Beijing,
        Some(b'9') if code.starts_with("920") => Exchange::Beijing,
        _ => return Err(format!("unsupported or unverified A-share code family: {code}").into()),
    };
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
}

fn print_batch<T: Debug>(label: &str, batch: &DataBatch<T>) {
    println!("\n=== {label} ===");
    println!("records={}", batch.records().len());
    println!("provenance={:#?}", batch.provenance());
    println!("quality={:#?}", batch.quality());
    for (index, record) in batch.records().iter().enumerate() {
        println!("record[{index}]={record:#?}");
    }
}
