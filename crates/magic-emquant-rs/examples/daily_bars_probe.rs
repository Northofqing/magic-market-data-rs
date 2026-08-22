use magic_emquant_rs::{EmQuantClient, EMQUANT_DAILY_BARS_ADMITTED, MAX_EMQUANT_DAILY_BARS};
use magic_market_core::{
    AssetClass, BarInterval, BarsRequest, Exchange, HistoricalBars, InstrumentId, ProviderId,
};
use std::error::Error;

const DEFAULT_CODES: &str = "600396.SH,000001.SZ";
const DEFAULT_START: &str = "2026-08-18";
const DEFAULT_END: &str = "2026-08-20";
const DEFAULT_LIMIT: u16 = 5;

fn parse_instrument(value: &str) -> Result<InstrumentId, Box<dyn Error>> {
    let (code, suffix) = value
        .trim()
        .rsplit_once('.')
        .ok_or("security code must use CODE.SH or CODE.SZ")?;
    let exchange = match suffix {
        "SH" => Exchange::Shanghai,
        "SZ" => Exchange::Shenzhen,
        _ => return Err("exchange suffix must be SH or SZ".into()),
    };
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    if !EMQUANT_DAILY_BARS_ADMITTED {
        return Err("EMQuant daily bars are not repository-admitted".into());
    }
    let codes =
        std::env::var("MAGIC_EMQUANT_DAILY_CODES").unwrap_or_else(|_| DEFAULT_CODES.to_owned());
    let start =
        std::env::var("MAGIC_EMQUANT_DAILY_START").unwrap_or_else(|_| DEFAULT_START.to_owned());
    let end = std::env::var("MAGIC_EMQUANT_DAILY_END").unwrap_or_else(|_| DEFAULT_END.to_owned());
    let limit = std::env::var("MAGIC_EMQUANT_DAILY_LIMIT")
        .ok()
        .map(|value| value.parse::<u16>())
        .transpose()?
        .unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_EMQUANT_DAILY_BARS {
        return Err(
            format!("MAGIC_EMQUANT_DAILY_LIMIT must be 1..={MAX_EMQUANT_DAILY_BARS}").into(),
        );
    }

    let instruments = codes
        .split(',')
        .map(parse_instrument)
        .collect::<Result<Vec<_>, _>>()?;
    if instruments.is_empty() {
        return Err("at least one EMQuant daily-bar instrument is required".into());
    }
    let client = EmQuantClient::discover()?;
    for instrument in instruments {
        let request = BarsRequest::new(instrument.clone(), BarInterval::Day, limit)?
            .with_range(&start, &end)?;
        let batch = client.historical_bars(&request)?;
        if batch.records().is_empty() || !batch.quality().is_complete() {
            return Err(format!("{} returned no complete daily bars", instrument.code()).into());
        }
        if batch.records().iter().any(|bar| {
            bar.provider() != ProviderId::Eastmoney
                || bar.interval() != BarInterval::Day
                || bar.source_at().is_none()
                || bar.observed_at().is_none()
                || bar.batch_id() != batch.provenance().batch_id().unwrap_or_default()
        }) {
            return Err(format!("{} returned conflicting evidence", instrument.code()).into());
        }
        println!(
            "code={} records={} first_source_at={} latest_source_at={} observed_at={} batch_id={}",
            instrument.code(),
            batch.records().len(),
            batch
                .records()
                .first()
                .and_then(|bar| bar.source_at())
                .unwrap_or(""),
            batch
                .records()
                .last()
                .and_then(|bar| bar.source_at())
                .unwrap_or(""),
            batch.provenance().fetched_at(),
            batch.provenance().batch_id().unwrap_or_default()
        );
    }
    println!("emquant_daily_bars_probe=passed");
    Ok(())
}
