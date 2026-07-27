use magic_market_core::{
    verify_admitted_batch, verify_verified_empty, AssetClass, ConsensusData, DataBatch, Exchange,
    InstrumentId, InstrumentSignalRequest, IsoDate, LimitPoolKind, LimitPoolRequest, LimitPools,
    PopularityData, PositiveU32, ProbeAdmissionPolicy, ProviderId, SourceEvidence,
    StrongStockReasons,
};
use magic_ths_rs::{ThsClient, ThsError};
use std::error::Error;
use std::fmt::Debug;

fn main() -> Result<(), Box<dyn Error>> {
    let client = ThsClient::new()?;
    let consensus_instrument =
        equity(std::env::var("MAGIC_THS_CONSENSUS_CODE").unwrap_or_else(|_| "600519".into()))?;
    let strong_instrument =
        equity(std::env::var("MAGIC_THS_STRONG_CODE").unwrap_or_else(|_| "000815".into()))?;
    let trading_date = IsoDate::new(
        std::env::var("MAGIC_THS_TRADING_DATE").unwrap_or_else(|_| "2026-07-22".into()),
    )?;
    let small = PositiveU32::new(3)?;
    let whole_limit_pool = PositiveU32::new(200)?;

    println!("provider=tonghuashun");
    println!("capabilities={:#?}", ThsClient::capabilities());

    let policy = ProbeAdmissionPolicy::new(ProviderId::Tonghuashun);
    let source_policy = policy.require_source_at();
    match client.consensus(&[consensus_instrument]) {
        Ok(batch) => print_admitted_batch(
            "consensus",
            &batch,
            &source_policy,
            |record| &record.evidence,
            |record| instrument_identity(&record.instrument),
        )?,
        Err(ThsError::VerifiedEmpty(empty)) => {
            let status = verify_verified_empty(&empty, &source_policy)?;
            println!("\n=== consensus ===");
            println!("family=consensus status={status}");
            println!("request_identity={}", empty.request_identity());
            println!("reason={}", empty.reason());
            println!("provenance={:#?}", empty.provenance());
            println!("evidence={:#?}", empty.evidence());
        }
        Err(error) => return Err(Box::new(error)),
    }
    let strong_request = InstrumentSignalRequest::new(strong_instrument, small)?
        .with_trading_date(trading_date.clone());
    let strong = client.strong_stock_reasons(&strong_request)?;
    print_admitted_batch(
        "strong_stock_reasons",
        &strong,
        &source_policy,
        |record| &record.evidence,
        |record| {
            format!(
                "{}:{}",
                instrument_identity(&record.instrument),
                record.trading_date.as_str()
            )
        },
    )?;
    let pool_request = LimitPoolRequest::new(LimitPoolKind::Upper, trading_date, whole_limit_pool)?;
    let pool = client.limit_pool(&pool_request)?;
    print_admitted_batch(
        "upper_limit_pool",
        &pool,
        &source_policy,
        |record| &record.evidence,
        |record| {
            format!(
                "{}:{}:{:?}",
                instrument_identity(&record.instrument),
                record.trading_date.as_str(),
                record.kind
            )
        },
    )?;
    let popularity = client.popularity(small)?;
    print_admitted_batch(
        "popularity",
        &popularity,
        &policy,
        |record| &record.evidence,
        |record| instrument_identity(&record.instrument),
    )?;
    println!("live_probe_status=admitted");
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

fn instrument_identity(instrument: &InstrumentId) -> String {
    let suffix = match instrument.exchange() {
        Exchange::Shanghai => "SH",
        Exchange::Shenzhen => "SZ",
        Exchange::Beijing => "BJ",
    };
    format!("{}.{suffix}", instrument.code())
}

fn print_admitted_batch<T: Debug>(
    label: &str,
    batch: &DataBatch<T>,
    policy: &ProbeAdmissionPolicy,
    evidence_of: impl Fn(&T) -> &SourceEvidence,
    identity_of: impl Fn(&T) -> String,
) -> Result<(), Box<dyn Error>> {
    let status = verify_admitted_batch(batch, policy, evidence_of, identity_of)?;
    println!("\n=== {label} ===");
    println!("family={label} status={status}");
    println!("records={}", batch.records().len());
    println!("provenance={:#?}", batch.provenance());
    println!("quality={:#?}", batch.quality());
    for (index, record) in batch.records().iter().enumerate() {
        println!("record[{index}]={record:#?}");
    }
    Ok(())
}
