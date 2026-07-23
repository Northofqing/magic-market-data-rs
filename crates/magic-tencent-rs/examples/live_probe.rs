use magic_market_core::{AssetClass, Exchange, InstrumentId, Money, OrderBooks, RealtimeQuotes};
use magic_tencent_rs::TencentClient;
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

fn main() -> Result<(), Box<dyn Error>> {
    let codes =
        std::env::var("MAGIC_TENCENT_CODES").unwrap_or_else(|_| "600396.SH,000001.SZ".to_owned());
    let instruments = codes
        .split(',')
        .map(parse_instrument)
        .collect::<Result<Vec<_>, _>>()?;
    let timeout = std::env::var("MAGIC_TENCENT_TIMEOUT_SECS")
        .ok()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(10);
    let client = TencentClient::with_timeout(Duration::from_secs(timeout))?;

    println!(
        "provider=tencent-web capabilities={:?}",
        TencentClient::capabilities()
    );
    let quotes = client.realtime_quotes(&instruments)?;
    if quotes.records().len() != instruments.len() {
        return Err("quote response cardinality mismatch".into());
    }
    println!(
        "quotes count={} provenance={:?} quality={:?}",
        quotes.records().len(),
        quotes.provenance(),
        quotes.quality()
    );
    for quote in quotes.records() {
        println!(
            "quote code={} exchange={:?} name={:?} price={} previous_close={:?} open={:?} high={:?} low={:?} change_percent={:?} volume_lots={} amount_yuan={:?} status={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
            quote.instrument().code(),
            quote.instrument().exchange(),
            quote.name(),
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

    let books = client.order_books(&instruments)?;
    if books.records().len() != instruments.len() {
        return Err("order-book response cardinality mismatch".into());
    }
    println!(
        "order_books count={} provenance={:?} quality={:?}",
        books.records().len(),
        books.provenance(),
        books.quality()
    );
    for book in books.records() {
        println!(
            "order_book code={} exchange={:?} total_bid_lots={:?} total_ask_lots={:?} status={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
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
        for (index, (bid, ask)) in book.bids().iter().zip(book.asks()).enumerate() {
            println!(
                "  level={} bid_price={:?} bid_lots={:?} ask_price={:?} ask_lots={:?}",
                index + 1,
                bid.price().map(|value| value.get()),
                bid.quantity().map(|value| value.get()),
                ask.price().map(|value| value.get()),
                ask.quantity().map(|value| value.get())
            );
        }
    }
    println!("live_probe_status=passed");
    Ok(())
}
