//! Read-only native coverage capture, not a gRPC/deployment acceptance probe.
use magic_cninfo_rs::CninfoClient;
use magic_market_core::{IsoDate, MarketAnnouncementRequest, PositiveU32};
use sha2::{Digest, Sha256};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let [date, limit] = arguments.as_slice() else {
        return Err(
            "usage: market_announcements_coverage_probe <YYYY-MM-DD> <limit:1..300>".into(),
        );
    };
    let date = IsoDate::new(date)?;
    let request =
        MarketAnnouncementRequest::new(date.clone(), date, PositiveU32::new(limit.parse()?)?)?;
    let request_bytes = serde_json::to_vec(&request)?;
    let result = CninfoClient::new()?.market_announcements_with_coverage(&request)?;
    println!(
        "{}",
        serde_json::json!({
            "receipt_version": 1,
            "scope": "CninfoNativeDateRangeQuery",
            "grpc_acceptance": false,
            "deployment_identity_verified": false,
            "pit_guarantee": false,
            "exchange_event_universe_complete": false,
            "request": request,
            "request_serialized_sha256": format!("{:x}", Sha256::digest(&request_bytes)),
            "provider_source_sha256": format!("{:x}", Sha256::digest(include_bytes!("../src/market_announcements.rs"))),
            "result": result
        })
    );
    Ok(())
}
