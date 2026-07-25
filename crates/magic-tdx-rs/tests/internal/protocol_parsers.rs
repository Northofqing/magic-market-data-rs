use super::*;

// --- parse_security_count ---

#[test]
fn test_security_count_empty() {
    assert!(parse_security_count(&[]).is_err());
}

#[test]
fn test_security_count_one_byte() {
    assert!(parse_security_count(&[0x01]).is_err());
}

#[test]
fn test_security_count_zero() {
    let result = parse_security_count(&[0x00, 0x00]).unwrap();
    assert_eq!(result, 0);
}

#[test]
fn test_security_count_normal() {
    let result = parse_security_count(&[0xE8, 0x03]).unwrap(); // 1000
    assert_eq!(result, 1000);
}

// --- parse_security_list ---

#[test]
fn test_security_list_empty_body() {
    assert!(parse_security_list(&[]).is_err());
}

#[test]
fn test_security_list_one_byte() {
    assert!(parse_security_list(&[0x01]).is_err());
}

#[test]
fn test_security_list_zero_count() {
    let result = parse_security_list(&[0x00, 0x00]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_security_list_truncated_record() {
    // count=1 but only 10 bytes (need 29)
    let mut data = vec![0x01, 0x00];
    data.extend_from_slice(&[0u8; 10]);
    let result = parse_security_list(&data).unwrap();
    assert!(result.is_empty()); // breaks early
}

#[test]
fn test_security_list_one_record() {
    // count=1, record_size=29
    let mut data = vec![0x01, 0x00];
    let mut record = vec![0u8; 29];
    // code: "600519\0"
    record[..6].copy_from_slice(b"600519");
    // name: GBK encoded "贵州茅台\0\0\0\0"
    let (gbk_bytes, _, _) = GBK.encode("贵州茅台");
    record[8..8 + gbk_bytes.len()].copy_from_slice(&gbk_bytes);
    data.extend_from_slice(&record);
    let result = parse_security_list(&data).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code, "600519");
    assert_eq!(result[0].name, "贵州茅台");
}

// --- parse_security_bars ---

#[test]
fn test_security_bars_empty() {
    assert!(parse_security_bars(&[], 4).is_err());
}

#[test]
fn test_security_bars_zero_count() {
    let result = parse_security_bars(&[0x00, 0x00], 4).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_security_bars_truncated() {
    // count=1 but only 5 bytes (need at least 16)
    let mut data = vec![0x01, 0x00];
    data.extend_from_slice(&[0u8; 5]);
    let error = parse_security_bars(&data, 4).unwrap_err();
    assert_eq!(
        error.error_code(),
        Some(ErrorCode::RESPONSE_LENGTH_MISMATCH)
    );
    assert!(error.to_string().contains("row 0"));
}

#[test]
fn test_security_bars_daily_format() {
    // category=4 (daily): u32 date + 4*price(var) + vol(4) + amount(4)
    // Build minimal: date=20260429, then 4 zero prices (0x00), vol=0, amount=0
    let mut data = vec![0x01, 0x00]; // count=1
    data.extend_from_slice(&20260429u32.to_le_bytes()); // date
    data.extend_from_slice(&[0x00; 16]); // 4 prices(var=1B each) + vol(4) + amount(4)
    let result = parse_security_bars(&data, 4).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].year, 2026);
    assert_eq!(result[0].month, 4);
    assert_eq!(result[0].day, 29);
}

// --- parse_index_bars ---

#[test]
fn test_index_bars_empty() {
    assert!(parse_index_bars(&[], 4).is_err());
}

#[test]
fn test_index_bars_zero_count() {
    let result = parse_index_bars(&[0x00, 0x00], 4).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_index_bars_truncated() {
    let mut data = vec![0x01, 0x00];
    data.extend_from_slice(&[0u8; 10]);
    let error = parse_index_bars(&data, 4).unwrap_err();
    assert_eq!(
        error.error_code(),
        Some(ErrorCode::RESPONSE_LENGTH_MISMATCH)
    );
    assert!(error.to_string().contains("row 0"));
}

// --- parse_minute_time_data ---

#[test]
fn test_minute_time_empty() {
    assert!(parse_minute_time_data(&[], 1, "600519").is_err());
}

#[test]
fn test_minute_time_too_short() {
    assert!(parse_minute_time_data(&[0x00], 1, "600519").is_err());
}

#[test]
fn test_minute_time_zero_count() {
    // 头部: 2(count) + 2(padding) + 1(indicator) + 6(stock_code) + 2(unknown) = 13 bytes
    // 需要至少 14 字节 (13 头部 + 1 数据)
    let body = [
        0x00, 0x00, 0x00, 0x00, 0x01, 0x36, 0x30, 0x30, 0x35, 0x31, 0x39, 0x00, 0x00, 0x00,
    ];
    let result = parse_minute_time_data(&body, 1, "600519").unwrap();
    assert!(result.is_empty());
}

// --- parse_history_minute_time_data ---

#[test]
fn test_history_minute_time_empty() {
    let result = parse_history_minute_time_data(&[], 1, "600519").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_history_minute_time_short_header() {
    // Less than 6 bytes header
    let result = parse_history_minute_time_data(&[0u8; 5], 1, "600519").unwrap();
    assert!(result.is_empty());
}

// --- parse_transaction_data ---

#[test]
fn test_transaction_empty() {
    assert!(parse_transaction_data(&[]).is_err());
}

#[test]
fn test_transaction_one_byte() {
    assert!(parse_transaction_data(&[0x01]).is_err());
}

#[test]
fn test_transaction_zero_count() {
    let result = parse_transaction_data(&[0x00, 0x00]).unwrap();
    assert!(result.is_empty());
}

// --- parse_history_transaction_data ---

#[test]
fn test_history_transaction_short() {
    assert!(parse_history_transaction_data(&[0u8; 5]).is_err());
}

#[test]
fn test_history_transaction_empty_body() {
    let result = parse_history_transaction_data(&[0u8; 6]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_history_transaction_keeps_minimum_size_last_record() {
    let body = [1, 0, 0, 0, 0, 0, 0x5a, 0x02, 10, 1, 0, 0];
    let result = parse_history_transaction_data(&body).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].time, "10:02");
    assert_eq!(result[0].price, 0.1);
    assert_eq!(result[0].vol, 1.0);
    assert_eq!(result[0].buyorsell, 0);
}

#[test]
fn test_history_transaction_rejects_truncated_record() {
    let body = [1, 0, 0, 0, 0, 0, 0x5a, 0x02, 10, 1, 0];
    assert!(parse_history_transaction_data(&body).is_err());
}

// --- parse_security_quotes ---

#[test]
fn test_quotes_empty() {
    assert!(parse_security_quotes(&[]).is_err());
}

#[test]
fn test_quotes_too_short() {
    assert!(parse_security_quotes(&[0x00, 0x00, 0x00]).is_err());
}

#[test]
fn test_quotes_zero_count() {
    // b1 cb (2 bytes) + count=0 (2 bytes)
    let result = parse_security_quotes(&[0x00, 0x00, 0x00, 0x00]).unwrap();
    assert!(result.is_empty());
}

// --- parse_finance_info ---

#[test]
fn test_finance_empty() {
    assert!(parse_finance_info(&[], 1, "600519").is_err());
}

#[test]
fn test_finance_short() {
    assert!(parse_finance_info(&[0u8; 50], 1, "600519").is_err());
}

#[test]
fn test_finance_valid() {
    // 9 bytes header + 136 bytes struct = 145 bytes minimum
    let mut data = vec![0u8; 145];
    // Set liutongguben (first f32 at pos 9)
    let liutongguben: f32 = 10.0;
    data[9..13].copy_from_slice(&liutongguben.to_le_bytes());
    let result = parse_finance_info(&data, 1, "600519").unwrap();
    assert_eq!(result.code, "600519");
    assert_eq!(result.market, 1);
    // raw value, no unit conversion
    assert!((result.liutongguben - 10.0).abs() < 0.1);
}

// --- parse_xdxr_info ---

#[test]
fn test_xdxr_empty() {
    assert!(parse_xdxr_info(&[]).is_err());
}

#[test]
fn test_xdxr_short() {
    assert!(parse_xdxr_info(&[0u8; 10]).is_err());
}

#[test]
fn test_xdxr_zero_count() {
    /* 9 bytes header + 2 bytes count=0 */
    let data = vec![0u8; 11];
    let result = parse_xdxr_info(&data).unwrap();
    assert!(result.is_empty());
}

// --- parse_block_info_meta ---

#[test]
fn test_block_meta_empty() {
    assert!(parse_block_info_meta(&[]).is_err());
}

#[test]
fn test_block_meta_short() {
    assert!(parse_block_info_meta(&[0u8; 30]).is_err());
}

#[test]
fn test_block_meta_valid() {
    let mut data = vec![0u8; 38];
    data[0..4].copy_from_slice(&1000u32.to_le_bytes()); // size
    data[5..37].copy_from_slice(&[0xAB; 32]); // hash
    let result = parse_block_info_meta(&data).unwrap();
    assert_eq!(result.size, 1000);
    assert_eq!(result.hash_value.len(), 64); // 32 bytes * 2 hex chars
}

// --- parse_block_info ---

#[test]
fn test_block_info_empty() {
    let result = parse_block_info(&[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_block_info_short_header() {
    let result = parse_block_info(&[0u8; 3]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_block_info_valid() {
    let mut data = vec![0u8; 4]; // 4 byte header
    data.extend_from_slice(&[0x42; 10]); // 10 bytes payload
    let result = parse_block_info(&data).unwrap();
    assert_eq!(result.len(), 10);
    assert!(result.iter().all(|&b| b == 0x42));
}
