use magic_market_core::{
    AssetClass, BarInterval, BarsRequest, Exchange, HistoricalBars, InstrumentId, MinuteData,
    MinuteDataRequest, Money, OrderBooks, RealtimeQuotes, SecurityMetadataProvider,
};
use magic_sina_rs::SinaClient;
use std::error::Error;
use std::time::Duration;

fn parse_instrument(value: &str) -> Result<InstrumentId, Box<dyn Error>> {
    let (code, exchange) = value
        .trim()
        .rsplit_once('.')
        .ok_or("security code must use CODE.SH, CODE.SZ or CODE.BJ")?;
    let exchange = match exchange.to_ascii_uppercase().as_str() {
        "SH" => Exchange::Shanghai,
        "SZ" => Exchange::Shenzhen,
        "BJ" => Exchange::Beijing,
        _ => return Err("exchange suffix must be SH, SZ or BJ".into()),
    };
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let codes = std::env::var("MAGIC_SINA_CODES")
        .unwrap_or_else(|_| "600396.SH,000001.SZ,920118.BJ".to_owned());
    let instruments = codes
        .split(',')
        .map(parse_instrument)
        .collect::<Result<Vec<_>, _>>()?;
    let timeout = std::env::var("MAGIC_SINA_TIMEOUT_SECS")
        .ok()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(10);
    let client = SinaClient::with_timeout(Duration::from_secs(timeout))?;

    println!(
        "provider=sina-web capabilities={:?}",
        SinaClient::capabilities()
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

    let metadata = client.security_metadata(&instruments)?;
    if metadata.records().len() != instruments.len() {
        return Err("security metadata response cardinality mismatch".into());
    }
    println!(
        "security_metadata count={} provenance={:?} quality={:?}",
        metadata.records().len(),
        metadata.provenance(),
        metadata.quality()
    );
    for record in metadata.records() {
        println!(
            "security code={} exchange={:?} name={:?} board={:?} is_st={:?} listed_on={:?} price_limit_percent={:?} price_limit_version={:?} status={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
            record.instrument().code(),
            record.instrument().exchange(),
            record.name(),
            record.board(),
            record.is_st(),
            record.listed_on(),
            record.price_limit().percent().map(|value| value.get()),
            record.price_limit().version(),
            record.status(),
            record.source_at(),
            record.observed_at(),
            record.provider(),
            record.batch_id()
        );
    }

    let primary = instruments
        .iter()
        .find(|instrument| instrument.exchange() == Exchange::Shanghai)
        .ok_or("live probe requires one Shanghai instrument")?;
    for interval in [
        BarInterval::Minute1,
        BarInterval::Minute5,
        BarInterval::Minute15,
        BarInterval::Minute30,
        BarInterval::Hour1,
        BarInterval::Day,
    ] {
        let request = BarsRequest::new(primary.clone(), interval, 3)?;
        let bars = client.historical_bars(&request)?;
        if bars.records().is_empty() || bars.records().len() > 3 {
            return Err(format!("{interval:?} bar response count is invalid").into());
        }
        println!(
            "bars interval={interval:?} count={} provenance={:?} quality={:?}",
            bars.records().len(),
            bars.provenance(),
            bars.quality()
        );
        for bar in bars.records() {
            println!(
                "  bar start={} end={} open={} high={} low={} close={} volume_lots={} amount_yuan={:?} adjustment={:?} source_at={:?} provider={:?} batch_id={}",
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

    for interval in [BarInterval::Week, BarInterval::Month, BarInterval::Year] {
        let request = BarsRequest::new(primary.clone(), interval, 3)?;
        match client.historical_bars(&request) {
            Err(error) => println!("bars interval={interval:?} unsupported={error}"),
            Ok(_) => {
                return Err(format!("Sina {interval:?} bars unexpectedly reported support").into());
            }
        }
    }

    if let Some(beijing) = instruments
        .iter()
        .find(|instrument| instrument.exchange() == Exchange::Beijing)
    {
        for interval in [BarInterval::Minute5, BarInterval::Day] {
            let request = BarsRequest::new(beijing.clone(), interval, 3)?;
            let bars = client.historical_bars(&request)?;
            if bars.records().is_empty() {
                return Err(format!("Beijing {interval:?} bars are empty").into());
            }
            println!(
                "beijing_bars interval={interval:?} count={} provenance={:?} quality={:?}",
                bars.records().len(),
                bars.provenance(),
                bars.quality()
            );
            for bar in bars.records() {
                println!(
                    "  beijing_bar start={} open={} high={} low={} close={} volume_lots={} amount_yuan={:?} source_at={:?}",
                    bar.bar_start(),
                    bar.open().get(),
                    bar.high().get(),
                    bar.low().get(),
                    bar.close().get(),
                    bar.volume().get(),
                    bar.amount().map(Money::get),
                    bar.source_at()
                );
            }
        }
    }

    for instrument in &instruments {
        let minute = client.minute_data(&MinuteDataRequest::new(instrument.clone()))?;
        if minute.records().is_empty() {
            return Err(
                format!("current minute response is empty for {}", instrument.code()).into(),
            );
        }
        println!(
            "minute_current code={} count={} provenance={:?} quality={:?}",
            instrument.code(),
            minute.records().len(),
            minute.provenance(),
            minute.quality()
        );
        for point in minute.records() {
            println!(
                "  minute at={} price={} cumulative_lots={} cumulative_amount_yuan={:?} status={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
                point.minute_at(),
                point.price().get(),
                point.cumulative_quantity().get(),
                point.cumulative_amount().map(Money::get),
                point.status(),
                point.source_at(),
                point.observed_at(),
                point.provider(),
                point.batch_id()
            );
        }
    }

    println!(
        "unsupported trades={} money_flow={} auction={} fundamentals={} corporate_actions={} blocks={}",
        !SinaClient::capabilities().trades,
        !SinaClient::capabilities().money_flow,
        !SinaClient::capabilities().auction,
        !SinaClient::capabilities().fundamentals,
        !SinaClient::capabilities().corporate_actions,
        !SinaClient::capabilities().blocks
    );
    println!("live_probe_status=passed");
    Ok(())
}
