use magic_tdx_rs::TdxSmartClient;
fn main() {
    let client = TdxSmartClient::new();
    match client.connect_to_any(Some(3.0)) {
        Ok(true) => println!("connected=true"),
        Ok(false) => println!("connected=false"),
        Err(error) => println!("connected=error error={error}"),
    }
}
