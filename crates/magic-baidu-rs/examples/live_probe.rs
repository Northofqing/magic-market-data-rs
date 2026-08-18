use magic_baidu_rs::{BaiduClient, TECHNICAL_BARS_ADMITTED};
use magic_market_core::{
    verify_admitted_batch, AssetClass, BarInterval, BarsRequest, Exchange, InstrumentId,
    ProbeAdmissionPolicy, ProbeStatus, ProviderId, TechnicalBarsProvider,
};
use std::error::Error;

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
    let client = BaiduClient::new()?;
    let capabilities = BaiduClient::capabilities();
    println!("provider=baidu-pae capabilities={capabilities:?}");
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)?;
    let request = BarsRequest::new(instrument, BarInterval::Day, 1)?;
    let batch = client.technical_bars(&request)?;
    verify_admitted_batch(
        &batch,
        &ProbeAdmissionPolicy::new(ProviderId::Baidu).require_source_at(),
        |record| record.evidence(),
        |record| {
            let bar = record.bar();
            format!(
                "{:?}:{}:{:?}:{}",
                bar.instrument().exchange(),
                bar.instrument().code(),
                bar.interval(),
                bar.bar_start()
            )
        },
    )?;
    println!(
        "source={} source_at={:?} fetched_at={} batch_id={:?} complete={} records={}",
        batch.provenance().source(),
        batch.provenance().source_at(),
        batch.provenance().fetched_at(),
        batch.provenance().batch_id(),
        batch.quality().is_complete(),
        batch.records().len()
    );
    for technical in batch.records() {
        let bar = technical.bar();
        println!(
            "exchange={:?} code={} asset_class={:?} interval={:?} bar_start={} bar_end={} open={} high={} low={} close={} volume_lots={} amount={:?} adjustment={:?} bar_source_at={:?} provider={:?} batch_id={} ma5={:?} ma10={:?} ma20={:?} evidence_provider={:?} evidence_source_at={:?} evidence_observed_at={} evidence_batch_id={}",
            bar.instrument().exchange(),
            bar.instrument().code(),
            bar.instrument().asset_class(),
            bar.interval(),
            bar.bar_start(),
            bar.bar_end(),
            bar.open().get(),
            bar.high().get(),
            bar.low().get(),
            bar.close().get(),
            bar.volume().get(),
            bar.amount().map(|value| value.get()),
            bar.adjustment(),
            bar.source_at(),
            bar.provider(),
            bar.batch_id(),
            technical.ma5().map(|value| value.get()),
            technical.ma10().map(|value| value.get()),
            technical.ma20().map(|value| value.get()),
            technical.evidence().provider(),
            technical.evidence().source_at(),
            technical.evidence().observed_at(),
            technical.evidence().batch_id()
        );
    }
    Ok(probe_status(TECHNICAL_BARS_ADMITTED))
}

fn probe_status(technical_bars_admitted: bool) -> ProbeStatus {
    if technical_bars_admitted {
        ProbeStatus::Admitted
    } else {
        ProbeStatus::DiagnosticCompleteUnadmitted
    }
}

#[cfg(test)]
#[path = "../tests/unit/live_probe_tests.rs"]
mod tests;
