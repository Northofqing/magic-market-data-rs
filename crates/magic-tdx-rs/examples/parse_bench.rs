use std::time::Instant;
use magic_tdx_rs::protocol::parsers::parse_security_count;

fn main() {
    let input = [0x01_u8, 0x00];
    let iterations = 1_000_000_u64;
    let start = Instant::now();
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        if let Ok(value) = parse_security_count(&input) { checksum += u64::from(value); }
    }
    let elapsed = start.elapsed();
    let ns = elapsed.as_secs_f64() * 1e9 / iterations as f64;
    println!("operation=parse_security_count iterations={iterations} elapsed_ms={} ns_per_op={ns:.2} checksum={checksum}", elapsed.as_secs_f64() * 1e3);
}
