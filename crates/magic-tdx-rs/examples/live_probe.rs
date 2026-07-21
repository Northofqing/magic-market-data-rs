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
        }
        Ok(false) => println!("connected=false"),
        Err(error) => println!("connected=error error={error}"),
    }
}
