use magic_emquant_rs::EmQuantClient;
use magic_market_core::{
    AssetClass, Auctions, Bar, BarInterval, BarsRequest, DataBatch, Exchange, HistoricalBars,
    InstrumentId, Money, MoneyFlows, OrderBooks, RealtimeQuotes,
};
use std::error::Error;
use std::time::Duration;

fn parse_instrument(value: &str) -> Result<InstrumentId, Box<dyn Error>> {
    let (code, exchange) = value
        .trim()
        .rsplit_once('.')
        .ok_or("security code must use CODE.SH or CODE.SZ")?;
    let exchange = match exchange.to_ascii_uppercase().as_str() {
        "SH" => Exchange::Shanghai,
        "SZ" => Exchange::Shenzhen,
        _ => return Err("exchange suffix must be SH or SZ".into()),
    };
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
}

fn print_bars(label: &str, bars: &DataBatch<Bar>) {
    println!(
        "{label} count={} provenance={:?} quality={:?}",
        bars.records().len(),
        bars.provenance(),
        bars.quality()
    );
    for bar in bars.records() {
        println!(
            "bar code={} interval={:?} start={} end={} open={} high={} low={} close={} volume={} amount={:?} adjustment={:?} source_at={:?} provider={:?} batch_id={}",
            bar.instrument().code(),
            bar.interval(),
            bar.bar_start(),
            bar.bar_end(),
            bar.open().get(),
            bar.high().get(),
            bar.low().get(),
            bar.close().get(),
            bar.volume().get(),
            bar.amount().map(Money::get),
            bar.adjustment(),
            bar.source_at(),
            bar.provider(),
            bar.batch_id()
        );
    }
}

fn record_error(errors: &mut Vec<String>, label: &str, error: impl std::fmt::Display) {
    println!("{label}=error error={error}");
    errors.push(format!("{label}: {error}"));
}

fn require_count(errors: &mut Vec<String>, label: &str, actual: usize, expected: usize) {
    if actual != expected {
        errors.push(format!(
            "{label}: expected {expected} records, received {actual}"
        ));
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let codes =
        std::env::var("MAGIC_EMQUANT_CODES").unwrap_or_else(|_| "600396.SH,000001.SZ".to_owned());
    let instruments = codes
        .split(',')
        .map(parse_instrument)
        .collect::<Result<Vec<_>, _>>()?;
    let timeout = std::env::var("MAGIC_EMQUANT_TIMEOUT_SECS")
        .ok()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(30);
    let client = EmQuantClient::discover()?.with_timeout(Duration::from_secs(timeout))?;

    println!(
        "provider=eastmoney-emquant bridge={} capabilities={:?}",
        client.bridge_path().display(),
        client.capabilities()
    );
    let mut errors = Vec::new();
    match client.realtime_quotes(&instruments) {
        Ok(quotes) => {
            println!(
                "quotes count={} provenance={:?} quality={:?}",
                quotes.records().len(),
                quotes.provenance(),
                quotes.quality()
            );
            require_count(
                &mut errors,
                "quotes",
                quotes.records().len(),
                instruments.len(),
            );
            for quote in quotes.records() {
                println!(
                    "quote code={} name={:?} exchange={:?} price={} previous_close={:?} open={:?} high={:?} low={:?} change_percent={:?} volume={} amount={:?} status={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
                    quote.instrument().code(),
                    quote.name(),
                    quote.instrument().exchange(),
                    quote.price().get(),
                    quote.previous_close().map(|value| value.get()),
                    quote.open().map(|value| value.get()),
                    quote.high().map(|value| value.get()),
                    quote.low().map(|value| value.get()),
                    quote.change_percent().map(|value| value.get()),
                    quote.volume().get(),
                    quote.amount().map(Money::get),
                    quote.status(),
                    quote.source_at(),
                    quote.observed_at(),
                    quote.provider(),
                    quote.batch_id()
                );
            }
        }
        Err(error) => record_error(&mut errors, "quotes", error),
    }

    match client.order_books(&instruments) {
        Ok(books) => {
            println!(
                "order_books count={} provenance={:?} quality={:?}",
                books.records().len(),
                books.provenance(),
                books.quality()
            );
            require_count(
                &mut errors,
                "order_books",
                books.records().len(),
                instruments.len(),
            );
            for book in books.records() {
                println!(
                    "order_book code={} exchange={:?} total_bid_quantity={:?} total_ask_quantity={:?} status={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
                    book.instrument().code(),
                    book.instrument().exchange(),
                    book.total_bid_quantity().map(|value| value.get()),
                    book.total_ask_quantity().map(|value| value.get()),
                    book.status(),
                    book.source_at(),
                    book.observed_at(),
                    book.provider(),
                    book.batch_id()
                );
                for (index, (bid, ask)) in book.bids().iter().zip(book.asks().iter()).enumerate() {
                    println!(
                        "  level={} bid_price={:?} bid_volume={:?} ask_price={:?} ask_volume={:?}",
                        index + 1,
                        bid.price().map(|value| value.get()),
                        bid.quantity().map(|value| value.get()),
                        ask.price().map(|value| value.get()),
                        ask.quantity().map(|value| value.get())
                    );
                }
            }
        }
        Err(error) => record_error(&mut errors, "order_books", error),
    }

    match client.money_flows(&instruments) {
        Ok(flows) => {
            println!(
                "money_flows count={} provenance={:?} quality={:?}",
                flows.records().len(),
                flows.provenance(),
                flows.quality()
            );
            require_count(
                &mut errors,
                "money_flows",
                flows.records().len(),
                instruments.len(),
            );
            for flow in flows.records() {
                println!(
                    "money_flow code={} main_net={:?} super_large_net={:?} large_net={:?} medium_net={:?} small_net={:?} status={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
                    flow.instrument().code(),
                    flow.main_net().map(Money::get),
                    flow.super_large_net().map(Money::get),
                    flow.large_net().map(Money::get),
                    flow.medium_net().map(Money::get),
                    flow.small_net().map(Money::get),
                    flow.status(),
                    flow.source_at(),
                    flow.observed_at(),
                    flow.provider(),
                    flow.batch_id()
                );
            }
        }
        Err(error) => record_error(&mut errors, "money_flows", error),
    }
    match client.auction_snapshots(&instruments) {
        Ok(batch) => println!(
            "auctions count={} provenance={:?}",
            batch.records().len(),
            batch.provenance()
        ),
        Err(error) => println!("auctions unsupported_or_error={error}"),
    }

    let daily_request = BarsRequest::new(instruments[0].clone(), BarInterval::Day, 5)?;
    match client.historical_bars(&daily_request) {
        Ok(bars) => {
            print_bars("daily_bars", &bars);
            require_count(&mut errors, "daily_bars", bars.records().len(), 5);
        }
        Err(error) => record_error(&mut errors, "daily_bars", error),
    }
    let minute_request = BarsRequest::new(instruments[0].clone(), BarInterval::Minute5, 5)?;
    match client.historical_bars(&minute_request) {
        Ok(bars) => {
            print_bars("minute5_bars", &bars);
            require_count(&mut errors, "minute5_bars", bars.records().len(), 5);
        }
        Err(error) => record_error(&mut errors, "minute5_bars", error),
    }
    if errors.is_empty() {
        println!("live_probe_status=passed");
    } else {
        eprintln!("live_probe_status=failed failures={}", errors.join(" | "));
        std::process::exit(1);
    }
    Ok(())
}
