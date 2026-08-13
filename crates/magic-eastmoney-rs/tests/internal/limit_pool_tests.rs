use super::{format_time, parse_limit_pool};
use crate::test_support::ScriptedTransport;
use crate::{EastmoneyClient, EastmoneyError};
use magic_market_core::{
    IsoDate, LimitPoolKind, LimitPoolRequest, LimitPools, PositiveU32, RatioUnit,
};

fn request(kind: LimitPoolKind) -> LimitPoolRequest {
    LimitPoolRequest::new(
        kind,
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap()
}

#[test]
fn maps_verified_limit_pool_units_and_metadata() {
    let fixture = r#"{"rc":0,"data":{"tc":1,"qdate":20260723,"pool":[{
      "c":"600396","m":1,"n":"华电辽能","p":1308000,"zdp":9.97,
      "hs":4.2,"fund":12345678,"fbt":93100,"lbt":145501,
      "zbc":2,"lbc":3,"hybk":"电力行业"
    }]}}"#
        .as_bytes();
    let batch = parse_limit_pool(fixture, &request(LimitPoolKind::Upper)).unwrap();
    let row = &batch.records()[0];
    assert_eq!(row.kind, LimitPoolKind::Upper);
    assert_eq!(row.instrument.code(), "600396");
    assert_eq!(row.trading_date.as_str(), "2026-07-23");
    assert_eq!(row.price.get(), 1308.0);
    assert_eq!(row.change.get(), 9.97);
    assert_eq!(row.change.unit(), RatioUnit::Percent);
    assert!(row.volume.is_none());
    assert_eq!(row.turnover.unwrap().get(), 4.2);
    assert_eq!(row.sealed_amount.unwrap().get(), 12345678.0);
    assert_eq!(row.first_seal_at.as_ref().unwrap().as_str(), "09:31:00");
    assert_eq!(row.last_seal_at.as_ref().unwrap().as_str(), "14:55:01");
    assert_eq!(row.break_count, Some(2));
    assert_eq!(row.streak.unwrap().get(), 3);
    assert_eq!(row.industry.as_ref().unwrap().as_str(), "电力行业");
    assert!(row.board_name.is_none());
    assert!(row.seal_state.is_none());
    assert!(row.reseal_count.is_none());
    assert!(row.reason.is_none());
    assert_eq!(row.evidence.source_at(), Some("2026-07-23"));
}

#[test]
fn source_qdate_is_required_calendar_valid_and_matches_the_request() {
    for fixture in [
        r#"{"rc":0,"data":{"tc":1,"pool":[{
          "c":"600396","m":1,"p":1308000,"zdp":9.97
        }]}}"#,
        r#"{"rc":0,"data":{"tc":1,"qdate":20260722,"pool":[{
          "c":"600396","m":1,"p":1308000,"zdp":9.97
        }]}}"#,
        r#"{"rc":0,"data":{"tc":1,"qdate":20260230,"pool":[{
          "c":"600396","m":1,"p":1308000,"zdp":9.97
        }]}}"#,
        r#"{"rc":0,"data":{"tc":1,"qdate":"2026-07-23","pool":[{
          "c":"600396","m":1,"p":1308000,"zdp":9.97
        }]}}"#,
    ] {
        assert!(
            parse_limit_pool(fixture.as_bytes(), &request(LimitPoolKind::PreviousUpper)).is_err(),
            "{fixture}"
        );
    }
}

#[test]
fn source_seal_times_require_a_real_hh_mm_ss_clock() {
    for valid in ["93100", "093100", "09:31:00"] {
        assert_eq!(
            format_time(Some(valid.into())).unwrap(),
            Some("09:31:00".into())
        );
    }
    for invalid in ["246000", "99:99:99", "09:60:00", "09:31", "1234567"] {
        assert!(format_time(Some(invalid.into())).is_err(), "{invalid}");
    }
}

#[test]
fn null_pool_and_nonzero_rc_fail() {
    assert!(parse_limit_pool(br#"{"rc":0,"data":null}"#, &request(LimitPoolKind::Lower)).is_err());
    assert!(
        parse_limit_pool(br#"{"rc":-1,"data":null}"#, &request(LimitPoolKind::Broken)).is_err()
    );
}

#[test]
fn br019_source_total_controls_complete_truncated_and_verified_empty_states() {
    let complete = parse_limit_pool(
        br#"{"rc":0,"data":{"tc":1,"qdate":20260723,"pool":[{
          "c":"600396","m":1,"p":1308000,"zdp":9.97
        }]}}"#,
        &request(LimitPoolKind::Upper),
    )
    .expect("complete source total");
    assert!(complete.quality().is_complete());

    let truncated = parse_limit_pool(
        br#"{"rc":0,"data":{"tc":2,"qdate":20260723,"pool":[{
          "c":"600396","m":1,"p":1308000,"zdp":9.97
        }]}}"#,
        &request(LimitPoolKind::Upper),
    )
    .expect("bounded source page remains inspectable");
    assert!(!truncated.quality().is_complete());
    assert_eq!(truncated.quality().issues().len(), 1);
    assert!(truncated.quality().issues()[0].contains("source_total=2"));

    let empty = parse_limit_pool(
        br#"{"rc":0,"data":{"tc":0,"qdate":20260723,"pool":[]}}"#,
        &request(LimitPoolKind::Upper),
    )
    .expect_err("source-proven empty must use the typed empty contract");
    let EastmoneyError::VerifiedEmpty(empty) = empty else {
        panic!("expected typed verified empty")
    };
    assert_eq!(empty.family(), "limit_pool");
    assert_eq!(empty.request_identity(), "Upper:2026-07-23:limit=10");
    assert_eq!(empty.evidence().source_at(), Some("2026-07-23"));
    assert_eq!(empty.provenance().source_at(), Some("2026-07-23"));
}

#[test]
fn br019_missing_contradictory_totals_and_duplicate_identities_fail_atomically() {
    for fixture in [
        r#"{"rc":0,"data":{"qdate":20260723,"pool":[]}}"#,
        r#"{"rc":0,"data":{"tc":-1,"qdate":20260723,"pool":[]}}"#,
        r#"{"rc":0,"data":{"tc":0,"qdate":20260723,"pool":[{
          "c":"600396","m":1,"p":1308000,"zdp":9.97
        }]}}"#,
        r#"{"rc":0,"data":{"tc":2,"qdate":20260723,"pool":[{
          "c":"600396","m":1,"p":1308000,"zdp":9.97
        },{
          "c":"600396","m":1,"p":1308000,"zdp":9.97
        }]}}"#,
    ] {
        assert!(
            parse_limit_pool(fixture.as_bytes(), &request(LimitPoolKind::Upper)).is_err(),
            "{fixture}"
        );
    }
}

#[test]
fn public_limit_pool_contract_routes_every_verified_pool_kind() {
    const FIXTURE: &str = r#"{"rc":0,"data":{"tc":1,"qdate":20260723,"pool":[{
      "c":"600396","m":1,"p":1308000,"zdp":9.97
    }]}}"#;
    for (kind, path, sort) in [
        (LimitPoolKind::Upper, "getTopicZTPool", "sort=fbt%3Aasc"),
        (LimitPoolKind::Broken, "getTopicZBPool", "sort=fbt%3Aasc"),
        (LimitPoolKind::Lower, "getTopicDTPool", "sort=fund%3Aasc"),
        (
            LimitPoolKind::PreviousUpper,
            "getYesterdayZTPool",
            "sort=zs%3Adesc",
        ),
    ] {
        let transport = ScriptedTransport::from_bodies([FIXTURE.as_bytes()]);
        let requests = transport.requests();
        let client = EastmoneyClient::with_transport(transport);
        let batch = client.limit_pool(&request(kind)).unwrap();
        assert_eq!(batch.records()[0].kind, kind);
        let source_request = requests.lock().unwrap()[0].clone();
        assert!(source_request.contains(path), "{source_request}");
        assert!(source_request.contains(sort), "{source_request}");
        assert!(source_request.contains("date=20260723"), "{source_request}");
    }
}

#[test]
fn limit_pool_decode_shape_market_and_price_failures_are_explicit() {
    assert!(matches!(
        parse_limit_pool(b"{", &request(LimitPoolKind::Upper)),
        Err(EastmoneyError::Decode(_))
    ));
    for fixture in [
        r#"{"rc":0,"data":{"tc":1,"qdate":20260723,"pool":{}}}"#,
        r#"{"rc":0,"data":{"tc":1,"qdate":20260723,"pool":[{
          "c":"600396","p":1308000,"zdp":9.97
        }]}}"#,
        r#"{"rc":0,"data":{"tc":1,"qdate":20260723,"pool":[{
          "c":"600396","m":1.5,"p":1308000,"zdp":9.97
        }]}}"#,
        r#"{"rc":0,"data":{"tc":1,"qdate":20260723,"pool":[{
          "c":"600396","m":1,"zdp":9.97
        }]}}"#,
        r#"{"rc":0,"data":{"tc":1,"qdate":20260723,"pool":[{
          "c":"600396","m":1,"p":1308000
        }]}}"#,
    ] {
        assert!(
            parse_limit_pool(fixture.as_bytes(), &request(LimitPoolKind::Upper)).is_err(),
            "{fixture}"
        );
    }
    assert_eq!(format_time(None).unwrap(), None);
    assert!(format_time(Some("not-a-clock".into())).is_err());
}
