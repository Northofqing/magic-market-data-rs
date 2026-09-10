use magic_hithink_rs::{CurrentAuctionStage, HithinkClient};
use magic_market_core::{AssetClass, Exchange, InstrumentId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let stage = match args.next().as_deref().unwrap_or("final") {
        "live" => CurrentAuctionStage::Live,
        "final" => CurrentAuctionStage::Final,
        _ => return Err("stage must be live or final".into()),
    };
    let calls = args.next().as_deref().unwrap_or("1").parse::<usize>()?;
    if !(1..=3).contains(&calls) || args.next().is_some() {
        return Err("usage: auction_observation_probe [live|final] [1..3]".into());
    }

    let client = probe_client()?;
    let instruments = [
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity)?,
        InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity)?,
    ];

    for call in 1..=calls {
        let batch = client.current_auction_observations(&instruments, stage)?;
        if batch.records().len() != instruments.len()
            || !batch.quality().is_complete()
            || batch.provenance().source_at().is_some()
            || batch
                .records()
                .iter()
                .any(|record| record.evidence().source_at().is_some())
        {
            return Err("current auction observation evidence is inconsistent".into());
        }
        println!(
            "call={call} stage={stage:?} records={} observed_at={} source_at_absent=true",
            batch.records().len(),
            batch.provenance().fetched_at(),
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

fn probe_client() -> Result<HithinkClient, Box<dyn std::error::Error>> {
    if let Ok(client) = HithinkClient::from_env() {
        return Ok(client);
    }
    let contents = std::fs::read_to_string(".env.local")?;
    let value = contents
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("HITHINK_FINANCE_API_KEY="))
        .next()
        .ok_or("HITHINK_FINANCE_API_KEY is absent from process and .env.local")?
        .trim()
        .trim_matches(&['\"', '\''][..]);
    Ok(HithinkClient::new(value.to_owned())?)
}
