//! Two bounded Day1 reads through the normal public Rust Provider.
//! No RPC listener, credential override, transport injection or automatic retry.
use magic_eastmoney_rs::EastmoneyClient;
use magic_market_core::{
    AssetClass, Exchange, FlowInterval, FlowScope, FundFlowRequest, FundFlowSeries, InstrumentId,
    PositiveU32,
};
use std::error::Error;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let digest = ring::digest::digest(&ring::digest::SHA256, include_bytes!("../src/fund_flow.rs"));
    let source_sha256 = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!("provider_source_sha256={source_sha256}");
    println!("scope=normalized_public_provider_receipt_not_raw_http_or_rpc_acceptance");
    let client = EastmoneyClient::new()?;
    let scope = FlowScope::Instrument(InstrumentId::new(
        Exchange::Shenzhen,
        "300005",
        AssetClass::Equity,
    )?);
    let started = Instant::now();
    let mut failed = false;
    for limit in [1, 2] {
        let request =
            FundFlowRequest::new(scope.clone(), FlowInterval::Day1, PositiveU32::new(limit)?)?;
        println!("request={request:#?}");
        println!("attempt_start_elapsed_ms={}", started.elapsed().as_millis());
        println!(
            "attempt_observed_unix_ms={}",
            time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000
        );
        match client.fund_flow_series(&request) {
            Ok(batch) => println!("normalized_batch={batch:#?}"),
            Err(error) => {
                failed = true;
                println!("provider_error={error:#?}");
            }
        }
        println!("attempt_end_elapsed_ms={}", started.elapsed().as_millis());
        println!("load_snapshot={:#?}", client.load_probe_snapshot());
    }
    if failed {
        return Err("at least one bounded Day1 Provider read failed".into());
    }
    Ok(())
}
