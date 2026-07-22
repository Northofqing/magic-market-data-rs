use magic_tdx_rs::TdxSmartClient;
fn main() {
    let client = TdxSmartClient::new();
    match client.connect_to_any(Some(3.0)) {
        Ok(true) => {
            println!("connected=true");
            match client.get_security_quotes(&[(1, "600519")]) {
                Ok(quotes) => println!("quotes={} first_price={}", quotes.len(), quotes.first().map_or(0.0, |q| q.price)),
                Err(error) => println!("quotes=error error={error}"),
            }
            match client.get_security_bars(4, 1, "600519", 0, 5, 0) {
                Ok(bars) => println!("bars={} first_datetime={}", bars.len(), bars.first().map_or("none", |b| b.datetime.as_str())),
                Err(error) => println!("bars=error error={error}"),
            }
            let inner = client.inner();
            match inner.get_security_count(1) { Ok(value) => println!("security_count_sh={value}"), Err(error) => println!("security_count=error error={error}") }
            match inner.get_security_list(1, 0) { Ok(items) => println!("security_list_sh={} first_code={}", items.len(), items.first().map_or("none", |v| v.code.as_str())), Err(error) => println!("security_list=error error={error}") }
            match inner.get_minute_time_data(1, "600519") { Ok(items) => println!("minute_data={} ", items.len()), Err(error) => println!("minute_data=error error={error}") }
            match inner.get_transaction_data(1, "600519", 0, 20) { Ok(items) => println!("transactions={} ", items.len()), Err(error) => println!("transactions=error error={error}") }
            match inner.get_finance_info(1, "600519") { Ok(_) => println!("finance_info=ok"), Err(error) => println!("finance_info=error error={error}") }
            match inner.get_xdxr_info(1, "600519") { Ok(items) => println!("xdxr={} ", items.len()), Err(error) => println!("xdxr=error error={error}") }
        }
        Ok(false) => println!("connected=false"),
        Err(error) => println!("connected=error error={error}"),
    }
}
