use magic_tdx_rs::error_codes::ErrorCode;
use magic_tdx_rs::protocol::adjuster::{
    adjust_index_bars, adjust_security_bars, calc_fq_factors, FqType,
};
use magic_tdx_rs::protocol::finance_fields::{
    extract_with_labels, field_definitions, validate_fields_len,
};
use magic_tdx_rs::protocol::fq_service::FqService;
use magic_tdx_rs::protocol::parsers::{
    minute_time_from_index, parse_history_minute_time_data, parse_index_bars,
    parse_minute_time_data, parse_security_bars, parse_security_quotes,
    parse_transaction_data_with_coefficient, parse_xdxr_info,
};
use magic_tdx_rs::protocol::types::{
    get_security_coefficient, get_security_type, IndexBar, SecurityBar, XdXrInfo,
};
fn bar(year: u32, month: u32, day: u32, close: f64) -> SecurityBar {
    SecurityBar {
        open: close,
        close,
        high: close,
        low: close,
        vol: 100.0,
        amount: close * 100.0,
        year,
        month,
        day,
        hour: 0,
        minute: 0,
        datetime: format!("{year:04}-{month:02}-{day:02}"),
    }
}

fn xdxr(category: u32, year: u32, month: u32, day: u32, fenhong: Option<f64>) -> XdXrInfo {
    XdXrInfo {
        year,
        month,
        day,
        category,
        name: String::new(),
        fenhong,
        peigujia: None,
        songzhuangu: None,
        peigu: None,
        suogu: None,
        panqianliutong: None,
        panhouliutong: None,
        qianzongguben: None,
        houzongguben: None,
        fenshu: None,
        xingquanjia: None,
    }
}

#[test]
fn security_type_and_price_units_cover_every_documented_market_family() {
    let cases = [
        (1, "600396", 1, 0.01),
        (1, "688001", 1, 0.01),
        (1, "900901", 2, 0.001),
        (1, "519001", 5, 0.00001),
        (1, "510050", 3, 0.001),
        (1, "580001", 3, 0.001),
        (1, "110001", 4, 0.0001),
        (1, "130001", 4, 0.0001),
        (1, "000001", 0, 0.01),
        (0, "399001", 0, 0.01),
        (0, "000001", 1, 0.01),
        (0, "300001", 1, 0.01),
        (0, "200001", 2, 0.001),
        (0, "150001", 3, 0.001),
        (0, "160001", 3, 0.001),
        (0, "100001", 4, 0.0001),
        (0, "120001", 4, 0.0001),
        (9, "UNKNOWN", 1, 0.01),
    ];
    for (market, code, expected_type, expected_coefficient) in cases {
        assert_eq!(get_security_type(market, code), expected_type, "{code}");
        assert_eq!(
            get_security_coefficient(market, code),
            expected_coefficient,
            "{code}"
        );
    }
}

#[test]
fn finance_field_catalog_preserves_labels_indices_and_missing_values() {
    let definitions = field_definitions();
    assert_eq!(definitions.len(), 45);
    assert_eq!(definitions[0], (1, "eps", "基本每股收益"));
    assert!(validate_fields_len(320));
    assert!(!validate_fields_len(319));

    let mut fields = vec![0.0; 320];
    fields[0] = 1.25;
    fields[319] = 42.0;
    let labeled = extract_with_labels(&fields);
    assert!(labeled.contains(&("eps", "基本每股收益", 1.25)));
    assert!(labeled.contains(&("employees", "员工总数", 42.0)));

    let short = extract_with_labels(&[]);
    assert!(short.iter().all(|(_, _, value)| *value == 0.0));
}

#[test]
fn factor_service_calculates_and_applies_forward_adjustment_without_mutating_volume() {
    let mut bars = vec![bar(2025, 1, 1, 10.0), bar(2025, 1, 2, 9.0)];
    let event = xdxr(1, 2025, 1, 2, Some(10.0));

    let result = FqService::calc_factors(std::slice::from_ref(&event), &bars, &[]);
    assert_eq!(result.factors.len(), 1);
    assert_eq!(result.factors[0].date, 20250102);
    assert_eq!(result.factors[0].close_before, 10.0);
    assert!((result.factors[0].qfq_factor - 0.9).abs() < 1e-10);
    assert!((result.factors[0].hfq_factor - (1.0 / 0.9)).abs() < 1e-10);
    assert!((result.cumulative_qfq - 0.9).abs() < 1e-10);
    assert!((result.cumulative_hfq - (1.0 / 0.9)).abs() < 1e-10);

    FqService::apply_fq(&mut bars, &[], &[event], FqType::Qfq);
    assert_eq!(bars[0].close, 9.0);
    assert_eq!(bars[0].vol, 100.0);
    assert_eq!(bars[1].close, 9.0);
}

#[test]
fn factor_calculation_skips_irrelevant_events_and_handles_zero_factor_explicitly() {
    let bars = vec![bar(2025, 1, 1, 10.0)];
    let irrelevant = xdxr(5, 2025, 1, 2, Some(10.0));
    let no_context = xdxr(1, 2024, 1, 1, Some(10.0));
    let result = calc_fq_factors(&[irrelevant, no_context], &bars, &[]);
    assert!(result.factors.is_empty());
    assert_eq!(result.cumulative_qfq, 1.0);
    assert_eq!(result.cumulative_hfq, 1.0);

    let zero = xdxr(1, 2025, 1, 2, Some(100.0));
    let result = calc_fq_factors(&[zero], &bars, &[]);
    assert_eq!(result.factors[0].qfq_factor, 0.0);
    assert_eq!(result.factors[0].hfq_factor, 1.0);
    assert_eq!(result.cumulative_hfq, 1.0);

    let mut unchanged = bars;
    adjust_security_bars(
        &mut unchanged,
        &[],
        &[xdxr(1, 2024, 1, 1, Some(10.0))],
        FqType::Hfq,
    );
    assert_eq!(unchanged[0].close, 10.0);
}

#[test]
fn index_adjustment_is_an_explicit_noop() {
    let original = IndexBar {
        open: 10.0,
        close: 11.0,
        high: 12.0,
        low: 9.0,
        vol: 1.0,
        amount: 2.0,
        year: 2025,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        datetime: "2025-01-01".into(),
        up_count: 3,
        down_count: 4,
    };
    let mut bars = vec![original.clone()];
    adjust_index_bars(&mut bars, &[xdxr(1, 2025, 1, 2, Some(10.0))], FqType::Qfq);
    assert_eq!(bars[0].close, original.close);
    assert_eq!(bars[0].up_count, original.up_count);
}

fn minute_date(year: u16, month: u16, day: u16) -> u16 {
    ((year - 2004) << 11) + month * 100 + day
}

#[test]
fn bar_parsers_decode_daily_and_intraday_records_with_explicit_units() {
    let mut security = vec![1, 0];
    security.extend_from_slice(&minute_date(2026, 7, 23).to_le_bytes());
    security.extend_from_slice(&(9u16 * 60 + 31).to_le_bytes());
    security.extend_from_slice(&[10, 1, 2, 0]);
    security.extend_from_slice(&100u32.to_le_bytes());
    security.extend_from_slice(&1_000u32.to_le_bytes());
    let parsed = parse_security_bars(&security, 0).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].datetime, "2026-07-23 09:31");
    assert_eq!(parsed[0].open, 0.01);
    assert_eq!(parsed[0].close, 0.011);
    assert_eq!(parsed[0].high, 0.012);

    let mut index = vec![1, 0];
    index.extend_from_slice(&20260723u32.to_le_bytes());
    index.extend_from_slice(&[10, 1, 2, 0]);
    index.extend_from_slice(&100u32.to_le_bytes());
    index.extend_from_slice(&1_000u32.to_le_bytes());
    index.extend_from_slice(&3u16.to_le_bytes());
    index.extend_from_slice(&4u16.to_le_bytes());
    index.extend_from_slice(&[0; 4]);
    let parsed = parse_index_bars(&index, 4).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].datetime, "2026-07-23");
    assert_eq!(parsed[0].up_count, 3);
    assert_eq!(parsed[0].down_count, 4);
}

#[test]
fn security_bar_parser_rejects_a_declared_row_missing_from_the_payload() {
    let mut packet = vec![2, 0];
    packet.extend_from_slice(&20260723u32.to_le_bytes());
    packet.extend_from_slice(&[10, 1, 2, 0]);
    packet.extend_from_slice(&100u32.to_le_bytes());
    packet.extend_from_slice(&1_000u32.to_le_bytes());

    let error = parse_security_bars(&packet, 4).unwrap_err();
    assert_eq!(
        error.error_code(),
        Some(ErrorCode::RESPONSE_LENGTH_MISMATCH)
    );
    assert!(error.to_string().contains("row 1"));
}

#[test]
fn security_bar_parser_rejects_an_invalid_later_row_date() {
    let mut packet = vec![2, 0];
    for date in [20260723u32, 20261301u32] {
        packet.extend_from_slice(&date.to_le_bytes());
        packet.extend_from_slice(&[10, 1, 2, 0]);
        packet.extend_from_slice(&100u32.to_le_bytes());
        packet.extend_from_slice(&1_000u32.to_le_bytes());
    }

    let error = parse_security_bars(&packet, 4).unwrap_err();
    assert_eq!(error.error_code(), Some(ErrorCode::INVALID_DATE));
    assert!(error.to_string().contains("row 1"));
}

#[test]
fn index_bar_parser_rejects_a_declared_row_missing_from_the_payload() {
    let mut packet = vec![2, 0];
    packet.extend_from_slice(&20260723u32.to_le_bytes());
    packet.extend_from_slice(&[10, 1, 2, 0]);
    packet.extend_from_slice(&100u32.to_le_bytes());
    packet.extend_from_slice(&1_000u32.to_le_bytes());
    packet.extend_from_slice(&3u16.to_le_bytes());
    packet.extend_from_slice(&4u16.to_le_bytes());
    packet.extend_from_slice(&[0; 4]);

    let error = parse_index_bars(&packet, 4).unwrap_err();
    assert_eq!(
        error.error_code(),
        Some(ErrorCode::RESPONSE_LENGTH_MISMATCH)
    );
    assert!(error.to_string().contains("row 1"));
}

#[test]
fn index_bar_parser_rejects_an_invalid_later_row_date() {
    let mut packet = vec![2, 0];
    for date in [20260723u32, 20261301u32] {
        packet.extend_from_slice(&date.to_le_bytes());
        packet.extend_from_slice(&[10, 1, 2, 0]);
        packet.extend_from_slice(&100u32.to_le_bytes());
        packet.extend_from_slice(&1_000u32.to_le_bytes());
        packet.extend_from_slice(&3u16.to_le_bytes());
        packet.extend_from_slice(&4u16.to_le_bytes());
    }
    packet.extend_from_slice(&[0; 4]);

    let error = parse_index_bars(&packet, 4).unwrap_err();
    assert_eq!(error.error_code(), Some(ErrorCode::INVALID_DATE));
    assert!(error.to_string().contains("row 1"));
}

#[test]
fn bar_parsers_reject_cumulative_price_overflow() {
    let maximum = [0xbf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f];

    let mut security = vec![2, 0];
    for (open, close) in [(&maximum[..], &maximum[..]), (&[2][..], &[0][..])] {
        security.extend_from_slice(&20260723u32.to_le_bytes());
        security.extend_from_slice(open);
        security.extend_from_slice(close);
        security.extend_from_slice(&[0, 0]);
        security.extend_from_slice(&100u32.to_le_bytes());
        security.extend_from_slice(&1_000u32.to_le_bytes());
    }
    assert!(parse_security_bars(&security, 4)
        .unwrap_err()
        .to_string()
        .contains("open price overflow"));

    let mut index = vec![2, 0];
    for (open, close) in [(&maximum[..], &maximum[..]), (&[2][..], &[0][..])] {
        index.extend_from_slice(&20260723u32.to_le_bytes());
        index.extend_from_slice(open);
        index.extend_from_slice(close);
        index.extend_from_slice(&[0, 0]);
        index.extend_from_slice(&100u32.to_le_bytes());
        index.extend_from_slice(&1_000u32.to_le_bytes());
        index.extend_from_slice(&3u16.to_le_bytes());
        index.extend_from_slice(&4u16.to_le_bytes());
    }
    // The parser's conservative preflight reserves the largest minimum index
    // row width before decoding the final variable-width row.
    index.extend_from_slice(&[0; 4]);
    assert!(parse_index_bars(&index, 4)
        .unwrap_err()
        .to_string()
        .contains("open price overflow"));
}

#[test]
fn minute_and_transaction_parsers_preserve_time_order_price_scale_and_volume() {
    assert_eq!(minute_time_from_index(0), "09:31");
    assert_eq!(minute_time_from_index(119), "11:30");
    assert_eq!(minute_time_from_index(120), "13:01");
    assert_eq!(minute_time_from_index(239), "15:00");

    let mut realtime = vec![1, 0];
    realtime.extend_from_slice(&[0; 11]);
    realtime.extend_from_slice(&[10, 0, 2]);
    let points = parse_minute_time_data(&realtime, 1, "600396").unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].time, "09:31");
    assert_eq!(points[0].price, 0.1);
    assert_eq!(points[0].avg_price, 0.1);
    assert_eq!(points[0].vol, 2.0);

    let mut history = vec![0; 6];
    history.extend_from_slice(&[10, 0, 2]);
    let points = parse_history_minute_time_data(&history, 1, "600396").unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].time, "09:31");
    assert_eq!(points[0].price, 0.1);

    let transaction = [1, 0, 0x5a, 0x02, 10, 2, 3, 1, 0];
    let ticks = parse_transaction_data_with_coefficient(&transaction, 0.001).unwrap();
    assert_eq!(ticks.len(), 1);
    assert_eq!(ticks[0].time, "10:02");
    assert_eq!(ticks[0].price, 0.01);
    assert_eq!(ticks[0].vol, 2.0);
    assert_eq!(ticks[0].num, 3);
    assert_eq!(ticks[0].buyorsell, 1);
}

fn quote_packet() -> Vec<u8> {
    let mut body = vec![0, 0, 1, 0, 1];
    body.extend_from_slice(b"600396");
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(&[10, 0, 1, 2, 3, 0, 0]);
    body.extend_from_slice(&[10, 5]);
    body.extend_from_slice(&1_000u32.to_le_bytes());
    body.extend_from_slice(&[4, 6]);
    body.extend_from_slice(&[0, 0]);
    body.extend_from_slice(&[0; 20]);
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(&[0; 4]);
    body.extend_from_slice(&[0; 4]);
    body
}

fn assert_all_strict_prefixes_fail<T>(
    packet: &[u8],
    minimum_prefix: usize,
    parser: impl Fn(&[u8]) -> magic_tdx_rs::error::Result<T>,
) {
    for length in minimum_prefix..packet.len() {
        let error = parser(&packet[..length]).err();
        assert!(
            error.is_some(),
            "truncated packet prefix of {length} bytes was accepted"
        );
        assert_eq!(
            error.and_then(|value| value.error_code()),
            Some(ErrorCode::RESPONSE_LENGTH_MISMATCH),
            "prefix length {length}"
        );
    }
}

#[test]
fn quote_parser_decodes_complete_depth_and_rejects_a_truncated_tail() {
    let body = quote_packet();
    let quotes = parse_security_quotes(&body).unwrap();
    assert_eq!(quotes.len(), 1);
    assert_eq!(quotes[0].market, 1);
    assert_eq!(quotes[0].code, "600396");
    assert_eq!(quotes[0].price, 0.1);
    assert_eq!(quotes[0].last_close, 0.1);
    assert_eq!(quotes[0].open, 0.11);
    assert!(quotes[0].servertime.is_empty());
    assert_eq!(quotes[0].bid1, 0.1);
    assert_eq!(quotes[0].ask5, 0.1);

    let mut truncated = body;
    truncated.pop();
    assert!(parse_security_quotes(&truncated).is_err());
}

#[test]
fn variable_record_parsers_reject_every_partial_record_prefix() {
    let mut security = vec![1, 0];
    security.extend_from_slice(&20260723u32.to_le_bytes());
    security.extend_from_slice(&[10, 1, 2, 0]);
    security.extend_from_slice(&100u32.to_le_bytes());
    security.extend_from_slice(&1_000u32.to_le_bytes());
    assert_all_strict_prefixes_fail(&security, 0, |body| parse_security_bars(body, 4));

    let mut realtime = vec![1, 0];
    realtime.extend_from_slice(&[0; 11]);
    realtime.extend_from_slice(&[10, 0, 2]);
    assert_all_strict_prefixes_fail(&realtime, 13, |body| {
        parse_minute_time_data(body, 1, "600396")
    });

    let history = [vec![0; 6], vec![10, 0, 2]].concat();
    assert_all_strict_prefixes_fail(&history, 7, |body| {
        parse_history_minute_time_data(body, 1, "600396")
    });

    let quote = quote_packet();
    assert_all_strict_prefixes_fail(&quote, 0, parse_security_quotes);
}

fn xdxr_packet(categories: &[u8]) -> Vec<u8> {
    let mut body = vec![0; 9];
    body.extend_from_slice(&(categories.len() as u16).to_le_bytes());
    for (index, category) in categories.iter().copied().enumerate() {
        body.extend_from_slice(&[0; 8]);
        body.extend_from_slice(&(20260101u32 + index as u32).to_le_bytes());
        body.push(category);
        let mut values = [0u8; 16];
        match category {
            1 => {
                values[0..4].copy_from_slice(&1.0f32.to_le_bytes());
                values[4..8].copy_from_slice(&2.0f32.to_le_bytes());
                values[8..12].copy_from_slice(&3.0f32.to_le_bytes());
                values[12..16].copy_from_slice(&4.0f32.to_le_bytes());
            }
            11 | 12 => values[8..12].copy_from_slice(&0.5f32.to_le_bytes()),
            13 | 14 => {
                values[0..4].copy_from_slice(&5.0f32.to_le_bytes());
                values[8..12].copy_from_slice(&6.0f32.to_le_bytes());
            }
            3 => {
                values[0..4].copy_from_slice(&1u32.to_le_bytes());
                values[4..8].copy_from_slice(&2u32.to_le_bytes());
                values[8..12].copy_from_slice(&3u32.to_le_bytes());
                values[12..16].copy_from_slice(&4u32.to_le_bytes());
            }
            _ => {}
        }
        body.extend_from_slice(&values);
    }
    body
}

#[test]
fn corporate_action_parser_preserves_every_category_and_payload_shape() {
    let categories: Vec<u8> = (1..=14).chain(std::iter::once(99)).collect();
    let parsed = parse_xdxr_info(&xdxr_packet(&categories)).unwrap();
    assert_eq!(parsed.len(), categories.len());
    assert_eq!(parsed[0].name, "除权除息");
    assert_eq!(parsed[0].fenhong, Some(1.0));
    assert_eq!(parsed[0].peigujia, Some(2.0));
    assert_eq!(parsed[0].songzhuangu, Some(3.0));
    assert_eq!(parsed[0].peigu, Some(4.0));
    assert_eq!(parsed[1].panqianliutong, Some(0.0));
    assert!(parsed[2].panqianliutong.unwrap() > 0.0);
    assert_eq!(parsed[10].suogu, Some(0.5));
    assert_eq!(parsed[12].xingquanjia, Some(5.0));
    assert_eq!(parsed[12].fenshu, Some(6.0));
    assert_eq!(parsed.last().unwrap().name, "未知");
}
