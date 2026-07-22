use magic_emquant_rs::EmQuantClient;
use magic_market_core::{
    AssetClass, Bar, BarInterval, BarsRequest, DataBatch, Exchange, HistoricalBars, InstrumentId,
    Money, OrderBooks, RealtimeQuotes,
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
            bar.instrument.code(),
            bar.interval,
            bar.bar_start,
            bar.bar_end,
            bar.open.get(),
            bar.high.get(),
            bar.low.get(),
            bar.close.get(),
            bar.volume.get(),
            bar.amount.map(Money::get),
            bar.adjustment,
            bar.source_at,
            bar.provider,
            bar.batch_id
        );
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let bridge = std::env::var_os("MAGIC_EMQUANT_BRIDGE")
        .ok_or("set MAGIC_EMQUANT_BRIDGE to the compiled snapshot bridge")?;
    let codes =
        std::env::var("MAGIC_EMQUANT_CODES").unwrap_or_else(|_| "600519.SH,000001.SZ".to_owned());
    let instruments = codes
        .split(',')
        .map(parse_instrument)
        .collect::<Result<Vec<_>, _>>()?;
    let timeout = std::env::var("MAGIC_EMQUANT_TIMEOUT_SECS")
        .ok()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(30);
    let client = EmQuantClient::new(bridge)?.with_timeout(Duration::from_secs(timeout))?;

    println!(
        "provider=eastmoney-emquant capabilities={:?}",
        client.capabilities()
    );
    let quotes = client.realtime_quotes(&instruments)?;
    println!(
        "quotes count={} provenance={:?} quality={:?}",
        quotes.records().len(),
        quotes.provenance(),
        quotes.quality()
    );
    for quote in quotes.records() {
        println!(
            "quote code={} exchange={:?} price={} volume={} amount={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
            quote.instrument.code(),
            quote.instrument.exchange(),
            quote.price.get(),
            quote.volume.get(),
            quote.amount.map(Money::get),
            quote.source_at,
            quote.observed_at,
            quote.provider,
            quote.batch_id
        );
    }

    let books = client.order_books(&instruments)?;
    println!(
        "order_books count={} provenance={:?} quality={:?}",
        books.records().len(),
        books.provenance(),
        books.quality()
    );
    for book in books.records() {
        println!(
            "order_book code={} exchange={:?} status={:?}",
            book.instrument.code(),
            book.instrument.exchange(),
            book.status
        );
        for (index, (bid, ask)) in book.bids.iter().zip(book.asks.iter()).enumerate() {
            println!(
                "  level={} bid_price={:?} bid_volume={:?} ask_price={:?} ask_volume={:?}",
                index + 1,
                bid.price.map(|value| value.get()),
                bid.quantity.map(|value| value.get()),
                ask.price.map(|value| value.get()),
                ask.quantity.map(|value| value.get())
            );
        }
    }

    let bars = client.historical_bars(&BarsRequest::new(
        instruments[0].clone(),
        BarInterval::Day,
        5,
    )?)?;
    print_bars("daily_bars", &bars);
    let minute_bars = client.historical_bars(&BarsRequest::new(
        instruments[0].clone(),
        BarInterval::Minute5,
        5,
    )?)?;
    print_bars("minute5_bars", &minute_bars);
    Ok(())
}
