use magic_market_core::{
    AssetClass, BarInterval, BarsRequest, Exchange, FinancialStatements, HistoricalBars,
    InstrumentId, MinuteData, MinuteDataRequest, Money, NonEmptyText, OptionData, OrderBooks,
    RealtimeQuotes, SecurityMetadataProvider, StatementKind,
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

    for kind in [
        StatementKind::Balance,
        StatementKind::Income,
        StatementKind::CashFlow,
    ] {
        let statements = client.financial_statements(std::slice::from_ref(primary), kind)?;
        if statements.records().is_empty() || statements.records().len() > 8 {
            return Err(format!("{kind:?} statement response count is invalid").into());
        }
        println!(
            "financial_statements kind={kind:?} count={} provenance={:?} quality={:?}",
            statements.records().len(),
            statements.provenance(),
            statements.quality()
        );
        for statement in statements.records() {
            println!(
                "  statement code={} kind={:?} report_period={} announced_on={:?} currency={:?} lines={} source_at={:?} observed_at={} provider={:?} batch_id={}",
                statement.instrument.code(),
                statement.kind,
                statement.report_period.as_str(),
                statement.announced_on.as_ref().map(|date| date.as_str()),
                statement.currency.as_ref().map(|value| value.as_str()),
                statement.lines.len(),
                statement.evidence.source_at(),
                statement.evidence.observed_at(),
                statement.evidence.provider(),
                statement.evidence.batch_id()
            );
            for line in &statement.lines {
                println!(
                    "    financial_line key={} source_label={} value={:?} unit={:?}",
                    line.key.as_str(),
                    line.source_label.as_str(),
                    line.value.map(|value| value.get()),
                    line.unit.as_ref().map(|value| value.as_str())
                );
            }
        }
    }

    let option_underlying_code =
        std::env::var("MAGIC_SINA_OPTION_UNDERLYING").unwrap_or_else(|_| "510050".to_owned());
    let option_underlying =
        InstrumentId::new(Exchange::Shanghai, option_underlying_code, AssetClass::Fund)?;
    let contracts = client.option_contracts(&option_underlying, None)?;
    if contracts.records().is_empty() {
        return Err("option discovery returned no contracts".into());
    }
    println!(
        "option_contracts underlying={} count={} capabilities={:?} provenance={:?} quality={:?}",
        option_underlying.code(),
        contracts.records().len(),
        SinaClient::option_capabilities(),
        contracts.provenance(),
        contracts.quality()
    );
    for contract in contracts.records() {
        println!(
            "  option_contract code={} underlying={} month={} expiry={:?} kind={:?} strike={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
            contract.contract_code.as_str(),
            contract.underlying.code(),
            contract.expiry_month.as_str(),
            contract.expiry.as_ref().map(|date| date.as_str()),
            contract.kind,
            contract.strike.map(|value| value.get()),
            contract.evidence.source_at(),
            contract.evidence.observed_at(),
            contract.evidence.provider(),
            contract.evidence.batch_id()
        );
    }
    let option_sample_count = std::env::var("MAGIC_SINA_OPTION_SAMPLE_CONTRACTS")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(2);
    if option_sample_count == 0 || option_sample_count > 10 {
        return Err("MAGIC_SINA_OPTION_SAMPLE_CONTRACTS must be between 1 and 10".into());
    }
    let option_codes = contracts
        .records()
        .iter()
        .take(option_sample_count)
        .map(|contract| NonEmptyText::new(contract.contract_code.as_str()))
        .collect::<Result<Vec<_>, _>>()?;
    let option_quotes = client.option_quotes(&option_codes)?;
    println!(
        "option_quotes count={} provenance={:?} quality={:?}",
        option_quotes.records().len(),
        option_quotes.provenance(),
        option_quotes.quality()
    );
    for quote in option_quotes.records() {
        println!(
            "  option_quote code={} name={:?} bid={:?} bid_quantity={:?} ask={:?} ask_quantity={:?} last={:?} previous_close={:?} open={:?} high={:?} low={:?} upper_limit={:?} lower_limit={:?} strike={:?} volume={:?} open_interest={:?} amount={:?} change_percent={:?} amplitude_percent={:?} quote_at={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
            quote.contract_code.as_str(),
            quote.name.as_ref().map(|value| value.as_str()),
            quote.bid.map(|value| value.get()),
            quote.bid_quantity.map(|value| value.get()),
            quote.ask.map(|value| value.get()),
            quote.ask_quantity.map(|value| value.get()),
            quote.last.map(|value| value.get()),
            quote.previous_close.map(|value| value.get()),
            quote.open.map(|value| value.get()),
            quote.high.map(|value| value.get()),
            quote.low.map(|value| value.get()),
            quote.upper_limit.map(|value| value.get()),
            quote.lower_limit.map(|value| value.get()),
            quote.strike.map(|value| value.get()),
            quote.volume.map(|value| value.get()),
            quote.open_interest.map(|value| value.get()),
            quote.amount.map(|value| value.get()),
            quote.change.map(|value| value.get()),
            quote.amplitude.map(|value| value.get()),
            quote.quote_at.as_ref().map(|value| value.as_str()),
            quote.evidence.source_at(),
            quote.evidence.observed_at(),
            quote.evidence.provider(),
            quote.evidence.batch_id()
        );
    }
    let option_greeks = client.option_greeks(&option_codes)?;
    println!(
        "option_greeks count={} provenance={:?} quality={:?}",
        option_greeks.records().len(),
        option_greeks.provenance(),
        option_greeks.quality()
    );
    for greeks in option_greeks.records() {
        println!(
            "  option_greek code={} name={:?} volume={:?} delta={:?} gamma={:?} theta={:?} vega={:?} rho={:?} implied_volatility={:?} high={:?} low={:?} trade_code={:?} strike={:?} last={:?} theoretical_price={:?} source_at={:?} observed_at={} provider={:?} batch_id={}",
            greeks.contract_code.as_str(),
            greeks.name.as_ref().map(|value| value.as_str()),
            greeks.volume.map(|value| value.get()),
            greeks.delta.map(|value| value.get()),
            greeks.gamma.map(|value| value.get()),
            greeks.theta.map(|value| value.get()),
            greeks.vega.map(|value| value.get()),
            greeks.rho.map(|value| value.get()),
            greeks.implied_volatility.map(|value| value.get()),
            greeks.high.map(|value| value.get()),
            greeks.low.map(|value| value.get()),
            greeks.trade_code.as_ref().map(|value| value.as_str()),
            greeks.strike.map(|value| value.get()),
            greeks.last.map(|value| value.get()),
            greeks.theoretical_price.map(|value| value.get()),
            greeks.evidence.source_at(),
            greeks.evidence.observed_at(),
            greeks.evidence.provider(),
            greeks.evidence.batch_id()
        );
    }

    println!(
        "unsupported trades={} money_flow={} auction={} corporate_actions={} blocks={}",
        !SinaClient::capabilities().trades,
        !SinaClient::capabilities().money_flow,
        !SinaClient::capabilities().auction,
        !SinaClient::capabilities().corporate_actions,
        !SinaClient::capabilities().blocks
    );
    println!("live_probe_status=passed");
    Ok(())
}
