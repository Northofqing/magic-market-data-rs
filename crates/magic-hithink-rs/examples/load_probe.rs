use magic_hithink_rs::HithinkClient;
use magic_market_core::{
    AssetClass, BarInterval, BarsRequest, CorporateActionRequest, Exchange, InstrumentId, IsoDate,
    StatementKind,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HithinkClient::from_env()?;
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity)?;
    let index = InstrumentId::new(Exchange::Shanghai, "000300", AssetClass::Index)?;
    let fund = InstrumentId::new(Exchange::Shanghai, "510300", AssetClass::Fund)?;
    for call in 1..=3 {
        let valuations = client.probe_market_statistics(std::slice::from_ref(&instrument))?;
        let index_bars = client.probe_historical_bars(
            &BarsRequest::new(index.clone(), BarInterval::Day, 10)?
                .with_range("2026-08-18", "2026-08-21")?,
        )?;
        let fund_bars = client.probe_historical_bars(
            &BarsRequest::new(fund.clone(), BarInterval::Day, 10)?
                .with_range("2026-08-18", "2026-08-21")?,
        )?;
        let income = client
            .probe_financial_statements(std::slice::from_ref(&instrument), StatementKind::Income)?;
        let balance = client.probe_financial_statements(
            std::slice::from_ref(&instrument),
            StatementKind::Balance,
        )?;
        let cash_flow = client.probe_financial_statements(
            std::slice::from_ref(&instrument),
            StatementKind::CashFlow,
        )?;
        let actions = client.probe_corporate_actions(
            &CorporateActionRequest::new(instrument.clone())
                .with_range(IsoDate::new("2025-01-01")?, IsoDate::new("2026-08-21")?)?,
        )?;
        let metadata =
            client.probe_security_metadata(&[instrument.clone(), index.clone(), fund.clone()])?;
        println!(
            "serial_call={call} valuations={} index_bars={} fund_bars={} income={} balance={} cash_flow={} actions={} metadata={}",
            valuations.records().len(),
            index_bars.records().len(),
            fund_bars.records().len(),
            income.records().len(),
            balance.records().len(),
            cash_flow.records().len(),
            actions.batch().records().len(),
            metadata.records().len(),
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
