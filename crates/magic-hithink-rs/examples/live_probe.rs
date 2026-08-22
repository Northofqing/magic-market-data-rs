use magic_hithink_rs::HithinkClient;
use magic_market_core::{
    AssetClass, BarInterval, BarsRequest, CorporateActionRequest, Exchange, InstrumentId, IsoDate,
    LimitPoolKind, LimitPoolRequest, PositiveU32, StatementKind,
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

    for (asset_class, code, label) in [
        (AssetClass::Index, "000300", "index"),
        (AssetClass::Fund, "510300", "etf"),
    ] {
        let request = BarsRequest::new(
            InstrumentId::new(Exchange::Shanghai, code, asset_class)?,
            BarInterval::Day,
            10,
        )?
        .with_range("2026-08-18", "2026-08-21")?;
        let batch = client.probe_historical_bars(&request)?;
        println!(
            "historical asset={label} records={} source_at={:?} complete={}",
            batch.records().len(),
            batch.provenance().source_at(),
            batch.quality().is_complete()
        );
    }

    for kind in [
        StatementKind::Income,
        StatementKind::Balance,
        StatementKind::CashFlow,
    ] {
        let batch = client.probe_financial_statements(std::slice::from_ref(&instrument), kind)?;
        println!(
            "financial kind={kind:?} records={} source_at={:?} complete={}",
            batch.records().len(),
            batch.provenance().source_at(),
            batch.quality().is_complete()
        );
    }

    let actions = client.probe_corporate_actions(
        &CorporateActionRequest::new(instrument.clone())
            .with_range(IsoDate::new("2025-01-01")?, IsoDate::new("2026-08-21")?)?,
    )?;
    println!(
        "corporate_actions records={} source_at={:?} complete={}",
        actions.batch().records().len(),
        actions.batch().provenance().source_at(),
        actions.batch().quality().is_complete()
    );

    let metadata = client.probe_security_metadata(&[
        instrument.clone(),
        InstrumentId::new(Exchange::Shanghai, "000300", AssetClass::Index)?,
        InstrumentId::new(Exchange::Shanghai, "510300", AssetClass::Fund)?,
    ])?;
    println!(
        "metadata records={} source_at={:?} complete={}",
        metadata.records().len(),
        metadata.provenance().source_at(),
        metadata.quality().is_complete()
    );

    let auctions = client.probe_auction_snapshots(std::slice::from_ref(&instrument))?;
    println!(
        "auction_diagnostic records={} source_at={:?} complete={} status={:?}",
        auctions.records().len(),
        auctions.provenance().source_at(),
        auctions.quality().is_complete(),
        auctions.records().first().map(|record| record.status())
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
