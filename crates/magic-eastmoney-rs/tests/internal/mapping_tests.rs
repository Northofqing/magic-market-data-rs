use super::{
    decimal, finite, iso_date, money, non_empty, optional_f64, optional_string, optional_u32,
    percent, price, quantity, required_string, validate_date_or_datetime,
    validate_minute_timestamp,
};
use serde_json::{json, Value};

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

#[test]
fn string_number_and_domain_value_mappers_cover_missing_and_invalid_shapes() {
    let object = json!({
        "text": " value ",
        "number": 12,
        "blank": " ",
        "dash": "-",
        "bool": true
    });
    assert_eq!(required_string(&object, "text").unwrap(), "value");
    assert_eq!(required_string(&object, "number").unwrap(), "12");
    assert!(required_string(&object, "missing").is_err());
    assert_eq!(optional_string(None).unwrap(), None);
    assert_eq!(optional_string(Some(&Value::Null)).unwrap(), None);
    assert_eq!(optional_string(object.get("blank")).unwrap(), None);
    assert_eq!(optional_string(object.get("dash")).unwrap(), None);
    assert!(optional_string(object.get("bool")).is_err());

    assert_eq!(optional_f64(None).unwrap(), None);
    assert_eq!(optional_f64(Some(&Value::Null)).unwrap(), None);
    assert_eq!(optional_f64(Some(&json!(12.5))).unwrap(), Some(12.5));
    assert_eq!(optional_f64(Some(&json!(" 12.5% "))).unwrap(), Some(12.5));
    assert_eq!(optional_f64(Some(&json!(" "))).unwrap(), None);
    assert!(optional_f64(Some(&json!("not-a-number"))).is_err());
    assert!(optional_f64(Some(&json!(true))).is_err());
    assert!(optional_f64(Some(&json!("NaN"))).is_err());

    assert_eq!(optional_u32(Some(&json!(7))).unwrap(), Some(7));
    assert_eq!(optional_u32(None).unwrap(), None);
    for value in [json!(-1), json!(1.5), json!(4_294_967_296_u64)] {
        assert!(optional_u32(Some(&value)).is_err());
    }

    assert!(non_empty(None).unwrap().is_none());
    assert_eq!(non_empty(Some("x".into())).unwrap().unwrap().as_str(), "x");
    assert!(non_empty(Some(String::new())).is_err());
    assert!(finite(None).unwrap().is_none());
    assert_eq!(finite(Some(1.0)).unwrap().unwrap().get(), 1.0);
    assert!(finite(Some(f64::INFINITY)).is_err());
    assert_eq!(money(Some(1.0)).unwrap().unwrap().get(), 1.0);
    assert!(money(Some(f64::NAN)).is_err());
    assert_eq!(quantity(Some(2.0)).unwrap().unwrap().get(), 2.0);
    assert!(quantity(Some(-1.0)).is_err());
    assert_eq!(price(Some(3.0)).unwrap().unwrap().get(), 3.0);
    assert!(price(Some(0.0)).is_err());
    assert_eq!(percent(Some(4.0)).unwrap().unwrap().get(), 4.0);
    assert_eq!(decimal(Some(0.5)).unwrap().unwrap().get(), 0.5);
    assert!(percent(Some(f64::INFINITY)).is_err());
    assert!(decimal(Some(f64::NAN)).is_err());
}

#[test]
fn timestamp_validators_cover_short_unicode_fraction_and_clock_shapes() {
    for invalid in [
        "",
        "短",
        "2026-07-23x15:00",
        "2026-07-23 1:00",
        "2026-07-23 15-00",
        "2026-07-23 15:00:00.x",
        "2026-07-23 15:00.1",
    ] {
        assert!(
            validate_date_or_datetime(invalid, "fixture").is_err(),
            "{invalid}"
        );
    }
    for invalid in [
        "",
        "短",
        "2026-07-23T15:00",
        "2026-07-23 1:00",
        "2026-07-23 15-00",
        "2026-07-23 24:00",
    ] {
        assert!(
            validate_minute_timestamp(invalid, "fixture").is_err(),
            "{invalid}"
        );
    }
    assert!(iso_date("短").is_err());
}
