use std::panic::{catch_unwind, AssertUnwindSafe};
use magic_tdx_rs::protocol::parsers::{parse_finance_info, parse_history_minute_time_data, parse_security_bars, parse_security_quotes, parse_transaction_data, parse_xdxr_info};

#[test]
fn truncated_inputs_never_panic() {
    for len in 0..=64 {
        let bytes: Vec<u8> = (0..len).map(|v| (v as u8).wrapping_mul(37)).collect();
        let bars = catch_unwind(AssertUnwindSafe(|| parse_security_bars(&bytes, 4)));
        let quotes = catch_unwind(AssertUnwindSafe(|| parse_security_quotes(&bytes)));
        let minute = catch_unwind(AssertUnwindSafe(|| parse_history_minute_time_data(&bytes, 1, "600519")));
        let trades = catch_unwind(AssertUnwindSafe(|| parse_transaction_data(&bytes)));
        let finance = catch_unwind(AssertUnwindSafe(|| parse_finance_info(&bytes, 1, "600519")));
        let xdxr = catch_unwind(AssertUnwindSafe(|| parse_xdxr_info(&bytes)));
        assert!(bars.is_ok(), "bars panic at length {len}");
        assert!(quotes.is_ok(), "quotes panic at length {len}");
        assert!(minute.is_ok(), "minute panic at length {len}");
        assert!(trades.is_ok(), "trades panic at length {len}");
        assert!(finance.is_ok(), "finance panic at length {len}");
        assert!(xdxr.is_ok(), "xdxr panic at length {len}");
    }
}
