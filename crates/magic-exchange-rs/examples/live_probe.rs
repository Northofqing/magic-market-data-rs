use magic_exchange_rs::{CffexClient, HkexClient, SseClient, SzseClient};
use magic_market_core::{
    Announcements, AssetClass, DataBatch, DragonTigerData, Exchange, FuturesDeliveryRequest,
    InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest, IsoDate, NorthboundChannel,
    NorthboundDailyRequest, NorthboundDailyStatistics, OrderBooks, PositiveU32, RealtimeQuotes,
};
use std::error::Error;
use std::fmt::Debug;

fn main() -> Result<(), Box<dyn Error>> {
    let limit = env_u32("MAGIC_EXCHANGE_LIVE_LIMIT", 3)?;
    if limit == 0 || limit > 20 {
        return Err("MAGIC_EXCHANGE_LIVE_LIMIT must be in 1..=20".into());
    }
    let delivery_year = env_u32("MAGIC_CFFEX_DELIVERY_YEAR", 2026)?;
    let delivery_month = env_u32("MAGIC_CFFEX_DELIVERY_MONTH", 2)?;
    if std::env::var("MAGIC_EXCHANGE_LIVE_OPERATION").as_deref() == Ok("cffex-delivery") {
        let cffex = CffexClient::new()?;
        let request = FuturesDeliveryRequest::new(
            PositiveU32::new(delivery_year)?,
            PositiveU32::new(delivery_month)?,
        )?;
        println!("provider=cffex-official");
        println!(
            "calendar_capabilities={:#?}",
            CffexClient::calendar_capabilities()
        );
        print_batch(
            "cffex_futures_delivery",
            &cffex.probe_futures_delivery_calendar(&request)?,
            4,
            4,
        )?;
        println!("\nlive_probe_status=passed");
        return Ok(());
    }
    let sse_code = std::env::var("MAGIC_EXCHANGE_SSE_CODE").unwrap_or_else(|_| "600396".into());
    let szse_code = std::env::var("MAGIC_EXCHANGE_SZSE_CODE").unwrap_or_else(|_| "000858".into());
    let sse_dragon_date =
        std::env::var("MAGIC_EXCHANGE_SSE_DRAGON_DATE").unwrap_or_else(|_| "2026-07-22".into());
    let szse_dragon_code =
        std::env::var("MAGIC_EXCHANGE_SZSE_DRAGON_CODE").unwrap_or_else(|_| "000603".into());
    let szse_dragon_date =
        std::env::var("MAGIC_EXCHANGE_SZSE_DRAGON_DATE").unwrap_or_else(|_| "2026-07-23".into());
    let hkex_date =
        std::env::var("MAGIC_EXCHANGE_HKEX_DATE").unwrap_or_else(|_| "2026-07-22".into());
    let sse_request = request(Exchange::Shanghai, sse_code, limit)?;
    let szse_request = request(Exchange::Shenzhen, szse_code, limit)?;
    let sse = SseClient::new()?;
    let szse = SzseClient::new()?;
    let hkex = HkexClient::new()?;

    println!("provider=sse-official");
    println!("capabilities={:#?}", SseClient::capabilities());
    let sse_batch = sse.announcements(&sse_request)?;
    print_batch("sse_announcements", &sse_batch, 1, limit as usize)?;
    let sse_dragon_request = signal_request(
        Exchange::Shanghai,
        sse_request.instrument().code(),
        &sse_dragon_date,
        10,
    )?;
    print_batch(
        "sse_dragon_tiger_entries",
        &sse.dragon_tiger_entries(&sse_dragon_request)?,
        1,
        10,
    )?;
    print_batch(
        "sse_dragon_tiger_seats",
        &sse.dragon_tiger_seats(&sse_dragon_request)?,
        10,
        10,
    )?;

    println!("\nprovider=szse-official");
    println!("capabilities={:#?}", SzseClient::capabilities());
    let szse_batch = szse.announcements(&szse_request)?;
    print_batch("szse_announcements", &szse_batch, 1, limit as usize)?;
    let szse_instrument = [szse_request.instrument().clone()];
    let quote_batch = szse.realtime_quotes(&szse_instrument)?;
    print_batch("szse_official_quotes", &quote_batch, 1, 1)?;
    let book_batch = szse.order_books(&szse_instrument)?;
    print_batch("szse_official_order_books", &book_batch, 1, 1)?;
    let szse_dragon_request =
        signal_request(Exchange::Shenzhen, &szse_dragon_code, &szse_dragon_date, 10)?;
    print_batch(
        "szse_dragon_tiger_entries",
        &szse.dragon_tiger_entries(&szse_dragon_request)?,
        1,
        10,
    )?;
    print_batch(
        "szse_dragon_tiger_seats",
        &szse.dragon_tiger_seats(&szse_dragon_request)?,
        10,
        10,
    )?;

    println!("\nprovider=hkex-official");
    println!("capabilities={:#?}", HkexClient::capabilities());
    let hkex_date = IsoDate::new(hkex_date)?;
    for channel in [NorthboundChannel::Shanghai, NorthboundChannel::Shenzhen] {
        let request = NorthboundDailyRequest::new(hkex_date.clone(), channel);
        let batch = hkex.northbound_daily_statistics(&request)?;
        print_batch(
            match channel {
                NorthboundChannel::Shanghai => "hkex_sse_northbound_daily",
                NorthboundChannel::Shenzhen => "hkex_szse_northbound_daily",
            },
            &batch,
            1,
            1,
        )?;
    }

    println!("\nlive_probe_status=passed");
    Ok(())
}

fn signal_request(
    exchange: Exchange,
    code: &str,
    trading_date: &str,
    limit: u32,
) -> Result<InstrumentSignalRequest, Box<dyn Error>> {
    Ok(InstrumentSignalRequest::new(
        InstrumentId::new(exchange, code, AssetClass::Equity)?,
        PositiveU32::new(limit)?,
    )?
    .with_trading_date(IsoDate::new(trading_date)?))
}

fn request(
    exchange: Exchange,
    code: String,
    limit: u32,
) -> Result<InstrumentDateRangeRequest, Box<dyn Error>> {
    Ok(InstrumentDateRangeRequest::new(
        InstrumentId::new(exchange, code, AssetClass::Equity)?,
        PositiveU32::new(limit)?,
    )?)
}

fn print_batch<T: Debug>(
    label: &str,
    batch: &DataBatch<T>,
    minimum: usize,
    maximum: usize,
) -> Result<(), Box<dyn Error>> {
    println!("\n=== {label} ===");
    println!("records={}", batch.records().len());
    println!("provenance={:#?}", batch.provenance());
    println!("quality={:#?}", batch.quality());
    for (index, record) in batch.records().iter().enumerate() {
        println!("record[{index}]={record:#?}");
    }
    if !batch.quality().is_complete() {
        return Err(format!("{label} returned incomplete quality").into());
    }
    if !(minimum..=maximum).contains(&batch.records().len()) {
        return Err(format!(
            "{label} returned {} records; expected {minimum}..={maximum}",
            batch.records().len()
        )
        .into());
    }
    Ok(())
}

fn env_u32(name: &str, default: u32) -> Result<u32, Box<dyn Error>> {
    Ok(std::env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .parse::<u32>()?)
}
