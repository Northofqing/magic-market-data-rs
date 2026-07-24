use magic_cninfo_rs::CninfoClient;
use magic_market_core::{
    verify_admitted_batch, Announcements, AssetClass, DataBatch, Exchange,
    InstrumentDateRangeRequest, InstrumentId, InvestorQuestions, PositiveU32, ProbeAdmissionPolicy,
    ProbeStatus, ProviderId,
};
use std::error::Error;
use std::fmt::Debug;

fn main() -> Result<(), Box<dyn Error>> {
    match run_probe() {
        Ok(status) => {
            println!("admitted={}", status.satisfies_capability());
            println!("live_probe_status={status}");
            Ok(())
        }
        Err(error) => {
            println!("live_probe_status={}", ProbeStatus::Failed);
            Err(error)
        }
    }
}

fn run_probe() -> Result<ProbeStatus, Box<dyn Error>> {
    let client = CninfoClient::new()?;
    let capabilities = CninfoClient::capabilities();
    let announcement_instrument =
        equity(std::env::var("MAGIC_CNINFO_CODE").unwrap_or_else(|_| "600396".into()))?;
    let question_instrument =
        equity(std::env::var("MAGIC_CNINFO_QUESTION_CODE").unwrap_or_else(|_| "002594".into()))?;
    let limit = PositiveU32::new(1)?;

    println!("provider=cninfo");
    println!("capabilities={capabilities:#?}");

    let mapping = client.organization_mapping(&announcement_instrument)?;
    println!("organization_mapping={mapping:#?}");

    let announcements = InstrumentDateRangeRequest::new(announcement_instrument, limit)?;
    let announcement_batch = client.announcements(&announcements)?;
    let announcement_status = verify_admitted_batch(
        &announcement_batch,
        &ProbeAdmissionPolicy::new(ProviderId::Cninfo).require_source_at(),
        |record| &record.evidence,
        |record| record.announcement_id.as_str().to_owned(),
    )?;
    print_batch("announcements", &announcement_batch);
    println!("announcements_probe_status={announcement_status}");

    let questions = InstrumentDateRangeRequest::new(question_instrument, limit)?;
    let question_batch = client.investor_questions(&questions)?;
    let question_status = verify_admitted_batch(
        &question_batch,
        &ProbeAdmissionPolicy::new(ProviderId::Cninfo).require_source_at(),
        |record| record.evidence(),
        |record| record.question_id().as_str().to_owned(),
    )?;
    print_batch("investor_questions", &question_batch);
    println!("investor_questions_probe_status={question_status}");

    Ok(combined_probe_status(
        capabilities.announcements,
        capabilities.investor_questions,
        announcement_status,
        question_status,
    ))
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

fn combined_probe_status(
    announcements_advertised: bool,
    questions_advertised: bool,
    announcement_status: ProbeStatus,
    question_status: ProbeStatus,
) -> ProbeStatus {
    if matches!(announcement_status, ProbeStatus::Failed)
        || matches!(question_status, ProbeStatus::Failed)
    {
        ProbeStatus::Failed
    } else if announcements_advertised
        && questions_advertised
        && announcement_status.satisfies_capability()
        && question_status.satisfies_capability()
    {
        ProbeStatus::Admitted
    } else {
        ProbeStatus::DiagnosticCompleteUnadmitted
    }
}

#[cfg(test)]
#[path = "../tests/unit/live_probe_tests.rs"]
mod tests;
