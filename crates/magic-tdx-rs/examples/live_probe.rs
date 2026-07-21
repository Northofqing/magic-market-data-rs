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
        }
        Ok(false) => println!("connected=false"),
        Err(error) => println!("connected=error error={error}"),
    }
}
