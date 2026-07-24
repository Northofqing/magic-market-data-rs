use super::{iso_date, optional_f64, validate_date_or_datetime, validate_minute_timestamp};
use serde_json::json;

#[test]
fn source_null_markers_remain_absent() {
    assert_eq!(optional_f64(Some(&json!("--"))).unwrap(), None);
    assert_eq!(optional_f64(Some(&json!("12.5%"))).unwrap(), Some(12.5));
}

#[test]
fn timestamps_keep_the_source_calendar_date() {
    assert_eq!(
        iso_date("2026-07-23 15:00:00").unwrap().as_str(),
        "2026-07-23"
    );
    assert!(iso_date("20260723").is_err());
    assert!(iso_date("2026-02-30 15:00:00").is_err());
    assert!(iso_date("2026-07-23 25:00:00").is_err());
}

#[test]
fn source_dates_and_times_reject_malformed_or_impossible_values() {
    for valid in [
        "2026-07-23",
        "2026-07-23 15:00",
        "2026-07-23 15:00:59",
        "2026-07-23 15:00:59.000",
    ] {
        validate_date_or_datetime(valid, "fixture").unwrap();
    }
    for invalid in [
        "2026-02-30",
        "2026-07-23T15:00:00",
        "2026-07-23 24:00:00",
        "2026-07-23 15:60:00",
        "2026-07-23 15:00:60",
        "2026-07-23 15:00.123",
        "2026-07-23 15:00:00.",
        "2026-07-23 trailing",
    ] {
        assert!(
            validate_date_or_datetime(invalid, "fixture").is_err(),
            "{invalid}"
        );
    }
    validate_minute_timestamp("2026-07-23 15:00", "fixture").unwrap();
    for invalid in ["2026-07-23", "2026-07-23 15:00:00", "2026-02-30 15:00"] {
        assert!(
            validate_minute_timestamp(invalid, "fixture").is_err(),
            "{invalid}"
        );
    }
}
