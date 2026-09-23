//! Walks TDX servers and dumps the raw security-bars response body, so the
//! E2103 signature ("security bar row 0 is truncated") can be attributed to a
//! server or a decode path rather than guessed at.
//!
//! Usage:
//!   cargo run --example e2103_server_walk [code] [category] [count] [market] [ip-filter]
//!
//! `ip-filter` is a comma-separated list of IP prefixes; when present only
//! matching servers are walked, which keeps a re-check of a known-good subset
//! quick when most of the registry is unreachable.

use magic_tdx_rs::net::utils::build_security_bars_packet;
use magic_tdx_rs::protocol::constants::{ALL_KNOWN_SERVERS, KLINE_DAILY, PRIMARY_SERVERS};
use magic_tdx_rs::protocol::parsers::parse_security_bars;
use magic_tdx_rs::TdxHqClient;

fn main() {
    let arg = |n: usize| std::env::args().nth(n);
    let code = arg(1).unwrap_or_else(|| "600519".to_string());
    let category: u8 = arg(2).and_then(|v| v.parse().ok()).unwrap_or(KLINE_DAILY);
    let count: u16 = arg(3).and_then(|v| v.parse().ok()).unwrap_or(5);
    let market: u8 = arg(4).and_then(|v| v.parse().ok()).unwrap_or(1);
    let filter: Vec<String> = arg(5)
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    println!("code={code} market={market} category={category} count={count} start=0 fq=0");

    let mut servers: Vec<(String, &str, u16)> = Vec::new();
    for (name, ip, port) in PRIMARY_SERVERS {
        servers.push((format!("PRIMARY/{name}"), ip, *port));
    }
    for (name, ip, port) in ALL_KNOWN_SERVERS {
        if PRIMARY_SERVERS.iter().any(|(_, known, _)| known == ip) {
            continue;
        }
        servers.push((format!("KNOWN/{name}"), ip, *port));
    }
    if !filter.is_empty() {
        servers.retain(|(_, ip, _)| filter.iter().any(|prefix| ip.starts_with(prefix.as_str())));
    }

    let mut decode_err = Vec::new();
    let mut ok = Vec::new();
    let mut unreachable = 0usize;

    for (label, ip, port) in &servers {
        let client = TdxHqClient::new();
        match client.connect(ip, *port, Some(3.0)) {
            Ok(true) => {}
            Ok(false) => {
                unreachable += 1;
                println!("{label} {ip}:{port} connect=false");
                continue;
            }
            Err(error) => {
                unreachable += 1;
                println!("{label} {ip}:{port} connect_error={error}");
                continue;
            }
        }

        let packet = build_security_bars_packet(category, market, &code, 0, count, 0);
        match client.send_raw_and_recv(&packet) {
            Ok(body) => {
                let head = body
                    .iter()
                    .take(16)
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let declared = if body.len() >= 2 {
                    u16::from_le_bytes([body[0], body[1]])
                } else {
                    0
                };
                let outcome = match parse_security_bars(&body, category) {
                    Ok(bars) => {
                        let first = bars.first().map(|b| b.datetime.clone()).unwrap_or_default();
                        let last = bars.last().map(|b| b.datetime.clone()).unwrap_or_default();
                        format!("OK n={} first={first} last={last}", bars.len())
                    }
                    Err(error) => format!("ERR {error}"),
                };
                println!(
                    "{label} {ip}:{port} body_len={} declared_count={declared} head=[{head}] -> {outcome}",
                    body.len()
                );
                if outcome.starts_with("ERR") {
                    decode_err.push(format!("{label} {ip}:{port}"));
                } else {
                    ok.push(format!("{label} {ip}:{port}"));
                }
            }
            Err(error) => {
                unreachable += 1;
                println!("{label} {ip}:{port} send_error={error}");
            }
        }
        client.disconnect();
    }

    println!();
    println!("=== summary ===");
    println!("walked                = {}", servers.len());
    println!("reachable+parsed_ok   = {}", ok.len());
    println!("reachable+decode_err  = {}", decode_err.len());
    println!("unreachable           = {unreachable}");
}
