use magic_tdx_rs::TdxSmartClient;
fn main() {
    let client = TdxSmartClient::new();
    match client.connect_to_any(Some(3.0)) {
        Ok(true) => {
            println!("connected=true");
            match client.get_security_quotes(&[(1, "600519")]) {
                Ok(quotes) => println!(
                    "quotes={} first_price={}",
                    quotes.len(),
                    quotes.first().map_or(0.0, |q| q.price)
                ),
                Err(error) => println!("quotes=error error={error}"),
            }
            match client.get_security_bars(4, 1, "600519", 0, 5, 0) {
                Ok(bars) => println!(
                    "bars={} first_datetime={}",
                    bars.len(),
                    bars.first().map_or("none", |b| b.datetime.as_str())
                ),
                Err(error) => println!("bars=error error={error}"),
            }
            let inner = client.inner();
            for category in 0_u8..=11 {
                match inner.get_security_bars(category, 1, "600519", 0, 1, 0) {
                    Ok(items) => println!("stock_kline category={category} count={}", items.len()),
                    Err(error) => println!("stock_kline category={category} error={error}"),
                }
            }
            match inner.get_index_bars(4, 1, "000001", 0, 5, 0) {
                Ok(items) => println!(
                    "index_kline count={} first_datetime={}",
                    items.len(),
                    items.first().map_or("none", |v| v.datetime.as_str())
                ),
                Err(error) => println!("index_kline=error error={error}"),
            }
            match inner.get_security_count(1) {
                Ok(value) => println!("security_count_sh={value}"),
                Err(error) => println!("security_count=error error={error}"),
            }
            match inner.get_security_list(1, 0) {
                Ok(items) => println!(
                    "security_list_sh={} first_code={}",
                    items.len(),
                    items.first().map_or("none", |v| v.code.as_str())
                ),
                Err(error) => println!("security_list=error error={error}"),
            }
            match inner.get_minute_time_data(1, "600519") {
                Ok(items) => println!("minute_data={} ", items.len()),
                Err(error) => println!("minute_data=error error={error}"),
            }
            match inner.get_history_minute_time_data(1, "600519", 20260721) {
                Ok(items) => println!("minute_history date=20260721 count={}", items.len()),
                Err(error) => println!("minute_history=error error={error}"),
            }
            match inner.get_transaction_data(1, "600519", 0, 20) {
                Ok(items) => println!("transactions={} ", items.len()),
                Err(error) => println!("transactions=error error={error}"),
            }
            match inner.get_history_transaction_data(1, "600519", 0, 20, 20260721) {
                Ok(items) => println!("transactions_history date=20260721 count={}", items.len()),
                Err(error) => println!("transactions_history=error error={error}"),
            }
            match inner.get_finance_info(1, "600519") {
                Ok(_) => println!("finance_info=ok"),
                Err(error) => println!("finance_info=error error={error}"),
            }
            match inner.get_xdxr_info(1, "600519") {
                Ok(items) => println!("xdxr={} ", items.len()),
                Err(error) => println!("xdxr=error error={error}"),
            }
            let blocks = magic_tdx_rs::TdxBlockClient::with_default("180.153.18.170");
            match blocks.get_industry_blocks() {
                Ok(items) => println!("blocks_industry={}", items.len()),
                Err(error) => println!("blocks_industry=error error={error}"),
            }
            match blocks.get_concept_blocks() {
                Ok(items) => println!("blocks_concept={}", items.len()),
                Err(error) => println!("blocks_concept=error error={error}"),
            }
            match blocks.get_index_blocks() {
                Ok(items) => println!("blocks_index={}", items.len()),
                Err(error) => println!("blocks_index=error error={error}"),
            }
        }
        Ok(false) => println!("connected=false"),
        Err(error) => println!("connected=error error={error}"),
    }
}
