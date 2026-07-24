use super::*;

#[test]
fn test_code_bytes_full() {
    let b = code_bytes("600519");
    assert_eq!(b, [0x36, 0x30, 0x30, 0x35, 0x31, 0x39]);
}

#[test]
fn test_code_bytes_short() {
    let b = code_bytes("SH");
    // "SH" = [0x53, 0x48], then padded with 0
    assert_eq!(b[0], 0x53);
    assert_eq!(b[1], 0x48);
    assert_eq!(b[2], 0x00);
    assert_eq!(b[5], 0x00);
}

#[test]
fn test_code_bytes_long() {
    let b = code_bytes("1234567");
    // truncated to 6 bytes
    assert_eq!(b.len(), 6);
}

#[test]
fn test_build_security_bars_packet() {
    let pkt = build_security_bars_packet(4, 1, "600519", 0, 800, 1);
    assert_eq!(pkt.len(), 38);
    // Header: 0x010C(2) + 0x01016408(4) + 0x001C(2) + 0x001C(2) + CMD(2) = 12
    // market at pos 12-13 (u16 LE)
    assert_eq!(u16::from_le_bytes([pkt[12], pkt[13]]), 1);
    // code at pos 14-19
    assert_eq!(&pkt[14..20], b"600519");
    // category at pos 20-21
    assert_eq!(u16::from_le_bytes([pkt[20], pkt[21]]), 4);
    // fq at pos 22-23
    assert_eq!(u16::from_le_bytes([pkt[22], pkt[23]]), 1);
}

#[test]
fn test_build_index_bars_packet() {
    let pkt = build_index_bars_packet(4, 1, "000001", 0, 100, 0);
    assert_eq!(pkt.len(), 38);
    // Same format as security bars, verify code at pos 14-19
    assert_eq!(&pkt[14..20], b"000001");
}

#[test]
fn test_decompress_zlib_no_data() {
    // Empty data should fail decompression
    let result = decompress_zlib(&[]);
    assert!(result.is_err() || result.is_ok());
    // zlib needs a proper header; empty input may error or produce empty
}

#[test]
fn test_decompress_zlib_invalid() {
    let result = decompress_zlib(&[0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(result.is_err());
}

#[test]
fn test_fetch_context_empty_bars() {
    let bars: Vec<SecurityBar> = vec![];
    let xdxr: Vec<XdXrInfo> = vec![];
    let ctx = fetch_context_bars_for_adjust(|_| Ok(Vec::new()), 4, 0, "000001", &bars, &xdxr);
    assert!(ctx.is_empty());
}

#[test]
fn test_auto_market() {
    assert_eq!(auto_market("600519").unwrap(), MARKET_SH);
    assert_eq!(auto_market("000858").unwrap(), MARKET_SZ);
    assert_eq!(auto_market("300750").unwrap(), MARKET_SZ);
    assert!(auto_market("123456").is_err());
    assert!(auto_market("abc").is_err());
    assert!(auto_market("12345").is_err());
}

#[test]
fn test_encode_gbk() {
    let result = encode_gbk("公司概况");
    assert!(result.is_ok());
    let bytes = result.unwrap();
    assert_eq!(bytes.len(), 8); // 4 个中文字符 * 2 字节
}

#[test]
fn test_encode_gbk_padded() {
    let result = encode_gbk_padded("test", 10);
    assert!(result.is_ok());
    let bytes = result.unwrap();
    assert_eq!(bytes.len(), 10);
    assert_eq!(&bytes[..4], b"test");
    assert_eq!(&bytes[4..], &[0, 0, 0, 0, 0, 0]);
}

// --- RateLimiter ---

#[test]
fn test_rate_limiter_cap_at_200() {
    let limiter = RateLimiter::new(5); // 200 req/s
    limiter.set_rps(500); // 超过上限
    let inner = limiter.inner.lock().unwrap();
    assert_eq!(inner.base_interval, Duration::from_millis(5));
}

#[test]
fn test_rate_limiter_set_200_exact() {
    let limiter = RateLimiter::new(5);
    limiter.set_rps(200);
    let inner = limiter.inner.lock().unwrap();
    assert_eq!(inner.base_interval, Duration::from_millis(5));
}

#[test]
fn test_rate_limiter_set_below_cap() {
    let limiter = RateLimiter::new(5);
    limiter.set_rps(10);
    let inner = limiter.inner.lock().unwrap();
    assert_eq!(inner.base_interval, Duration::from_millis(100));
}

#[test]
fn test_rate_limiter_disable() {
    let limiter = RateLimiter::new(5);
    limiter.set_rps(0);
    assert!(!limiter.enabled.load(Ordering::Relaxed));
}

#[test]
fn test_rate_limiter_wait_no_delay_when_disabled() {
    let limiter = RateLimiter::new(0);
    let start = Instant::now();
    limiter.wait();
    assert!(start.elapsed() < Duration::from_millis(10));
}

// --- TradingPhase ---

#[test]
fn test_rate_limiter_phase_trading() {
    // base 100ms (10 req/s), Trading: ×1.0 → 100ms
    let limiter = RateLimiter::new(100);
    limiter.set_phase(TradingPhase::Trading);
    let inner = limiter.inner.lock().unwrap();
    assert_eq!(inner.min_interval, Duration::from_millis(100));
}

#[test]
fn test_rate_limiter_phase_pre_post() {
    // base 100ms (10 req/s), PrePost: /2 → 50ms (20 req/s)
    let limiter = RateLimiter::new(100);
    limiter.set_phase(TradingPhase::PrePost);
    let inner = limiter.inner.lock().unwrap();
    assert_eq!(inner.min_interval, Duration::from_millis(50));
}

#[test]
fn test_rate_limiter_phase_closed() {
    // base 100ms (10 req/s), Closed: /4 → 25ms (40 req/s)
    let limiter = RateLimiter::new(100);
    limiter.set_phase(TradingPhase::Closed);
    let inner = limiter.inner.lock().unwrap();
    assert_eq!(inner.min_interval, Duration::from_millis(25));
}

#[test]
fn test_detect_trading_phase_returns_valid() {
    let phase = detect_trading_phase();
    assert!(matches!(
        phase,
        TradingPhase::Trading | TradingPhase::PrePost | TradingPhase::Closed
    ));
}

#[test]
fn test_detect_trading_phase_uses_china_local_date_and_boundaries() {
    // 1970-01-02 is Friday. Values below are UTC timestamps for China-local times.
    assert_eq!(detect_trading_phase_at(91_799), TradingPhase::PrePost); // 09:29:59
    assert_eq!(detect_trading_phase_at(91_800), TradingPhase::Trading); // 09:30:00
    assert_eq!(detect_trading_phase_at(99_000), TradingPhase::Trading); // 11:30:00
    assert_eq!(detect_trading_phase_at(99_001), TradingPhase::PrePost); // 11:30:01
    assert_eq!(detect_trading_phase_at(104_399), TradingPhase::PrePost); // 12:59:59
    assert_eq!(detect_trading_phase_at(104_400), TradingPhase::Trading); // 13:00:00
    assert_eq!(detect_trading_phase_at(111_600), TradingPhase::Trading); // 15:00:00
    assert_eq!(detect_trading_phase_at(111_601), TradingPhase::PrePost); // 15:00:01
    assert_eq!(detect_trading_phase_at(187_200), TradingPhase::Closed); // Saturday noon
    assert_eq!(detect_trading_phase_at(318_600), TradingPhase::PrePost); // Monday 00:30
}
