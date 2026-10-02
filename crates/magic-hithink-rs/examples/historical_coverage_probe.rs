//! Normal bounded Provider observation. No service listener or admission change.
use magic_hithink_rs::HithinkClient;
use magic_market_core::{AssetClass, BarInterval, BarsRequest, Exchange, InstrumentId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        return Err("usage: historical_coverage_probe SH_CODE START END LIMIT; key comes from HITHINK_FINANCE_API_KEY".into());
    }
    let request = BarsRequest::new(
        InstrumentId::new(Exchange::Shanghai, &args[0], AssetClass::Equity)?,
        BarInterval::Day,
        args[3].parse::<u16>()?,
    )?
    .with_range(&args[1], &args[2])?;
    let client = HithinkClient::from_env()?;
    let result = client.historical_bars_with_coverage(&request)?;
    let snapshot = client.load_probe_snapshot()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "request": request,
            "result": result,
            "load_probe": {
                "request_starts": snapshot.request_starts(),
                "active_requests": snapshot.active_requests(),
                "maximum_concurrency": snapshot.maximum_concurrency(),
                "minimum_start_gap_seconds": snapshot.minimum_start_gap().map(|gap| gap.as_secs_f64()),
            },
            "scope": "NormalProviderObservationNotGrpcAcceptance",
        }))?
    );
    Ok(())
}
