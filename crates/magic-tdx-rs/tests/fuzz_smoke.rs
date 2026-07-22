use std::panic::{catch_unwind, AssertUnwindSafe};
use magic_tdx_rs::protocol::parsers::{parse_security_bars, parse_security_quotes};

#[test]
fn truncated_inputs_never_panic() {
    for len in 0..=64 {
        let bytes: Vec<u8> = (0..len).map(|v| (v as u8).wrapping_mul(37)).collect();
        let bars = catch_unwind(AssertUnwindSafe(|| parse_security_bars(&bytes, 4)));
        let quotes = catch_unwind(AssertUnwindSafe(|| parse_security_quotes(&bytes)));
        assert!(bars.is_ok(), "bars panic at length {len}");
        assert!(quotes.is_ok(), "quotes panic at length {len}");
    }
}
