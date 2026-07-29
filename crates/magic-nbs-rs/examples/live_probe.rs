use magic_nbs_rs::NbsClient;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = NbsClient::new(Duration::from_secs(10))?.probe_public_landing_page()?;
    println!("NBS public landing page accepted: {bytes} bounded bytes");
    Ok(())
}
