use magic_hithink_rs::HithinkClient;
use magic_market_core::{
    AssetClass, BarInterval, BarsRequest, Exchange, InstrumentId, IsoDate, LimitPoolKind,
    LimitPoolRequest, PositiveU32,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HithinkClient::from_env()?;
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity)?;
    let bars_request = BarsRequest::new(instrument.clone(), BarInterval::Day, 10)?
        .with_range("2026-08-18", "2026-08-21")?;
    let bars = client.probe_historical_bars(&bars_request)?;
    println!(
        "historical records={} source_at={:?} complete={}",
        bars.records().len(),
        bars.provenance().source_at(),
        bars.quality().is_complete()
    );

    let valuations = client.probe_market_statistics(std::slice::from_ref(&instrument))?;
    println!(
        "valuations records={} source_at={:?} complete={}",
        valuations.records().len(),
        valuations.provenance().source_at(),
        valuations.quality().is_complete()
    );

    for kind in [
        LimitPoolKind::Upper,
        LimitPoolKind::Lower,
        LimitPoolKind::Broken,
    ] {
        let request =
            LimitPoolRequest::new(kind, IsoDate::new("2026-08-21")?, PositiveU32::new(10)?)?;
        let batch = client.probe_limit_pool(&request)?;
        println!(
            "limit_pool kind={kind:?} records={} source_at={:?} complete={}",
            batch.records().len(),
            batch.provenance().source_at(),
            batch.quality().is_complete()
        );
    }

    let popularity = client.probe_popularity(PositiveU32::new(10)?)?;
    println!(
        "popularity records={} source_at={:?} complete={}",
        popularity.records().len(),
        popularity.provenance().source_at(),
        popularity.quality().is_complete()
    );
    Ok(())
}
