use magic_hithink_rs::HithinkClient;
use magic_market_core::{AssetClass, Exchange, InstrumentId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HithinkClient::from_env()?;
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity)?;
    for call in 1..=3 {
        let batch = client.probe_market_statistics(std::slice::from_ref(&instrument))?;
        println!(
            "serial_call={call} records={} complete={}",
            batch.records().len(),
            batch.quality().is_complete()
        );
    }
    let snapshot = client.load_probe_snapshot()?;
    println!(
        "request_starts={} active_requests={} maximum_concurrency={} minimum_start_gap={:?}",
        snapshot.request_starts(),
        snapshot.active_requests(),
        snapshot.maximum_concurrency(),
        snapshot.minimum_start_gap()
    );
    Ok(())
}
