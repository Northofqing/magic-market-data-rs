use super::*;

fn row(id: &str, source: &str, seconds: &str) -> Row {
    Row {
        id: id.into(),
        url: format!("/article/detail/{id}.html"),
        web_url: format!("/article/detail/{id}.html"),
        title: "synthetic title".into(),
        source: source.into(),
        time: seconds.parse::<i64>().unwrap_or_default() * 1_000,
        show_time: seconds.into(),
        page_time: id.into(),
    }
}

#[test]
fn cursor_and_terminal_shapes_deserialize_exactly() {
    let missing = CursorField::<i64>::default();
    assert_eq!(missing, CursorField::Missing);
    assert_eq!(
        serde_json::from_str::<CursorField<i64>>("null").unwrap(),
        CursorField::Null
    );
    assert_eq!(
        serde_json::from_str::<CursorField<i64>>("123").unwrap(),
        CursorField::Value(123)
    );
    assert!(matches!(
        serde_json::from_str::<QuickNewsData>("[]").unwrap(),
        QuickNewsData::Rows(rows) if rows.is_empty()
    ));
    assert!(matches!(
        serde_json::from_str::<QuickNewsData>(r#""""#).unwrap(),
        QuickNewsData::TerminalEmpty
    ));
    assert!(serde_json::from_str::<QuickNewsData>(r#""not-empty""#).is_err());
}

#[test]
fn row_parser_covers_provider_default_and_exact_identity_contract() {
    let parsed = parse_row(row("4754321", "", "1785291905")).unwrap();
    assert_eq!(parsed.id, "4754321");
    assert_eq!(parsed.publisher, "证券时报");
    assert_eq!(
        parsed.canonical_url,
        "https://www.stcn.com/article/detail/4754321.html"
    );
    assert_eq!(parsed.published_at, "2026-07-29T10:25:05+08:00");

    let mut invalid = row("abc", "人民财讯", "1785291905");
    assert!(parse_row(invalid).is_err());
    invalid = row("4754321", " 人民财讯", "1785291905");
    assert!(parse_row(invalid).is_err());
    invalid = row("4754321", "人民财讯", "1785291905");
    invalid.page_time = "other".into();
    assert!(parse_row(invalid).is_err());
    invalid = row("4754321", "人民财讯", "1785291905");
    invalid.time += 1_000;
    assert!(parse_row(invalid).is_err());
    invalid = row("4754321", "人民财讯", "1785291905");
    invalid.url = "/article/detail/other.html".into();
    assert!(parse_row(invalid).is_err());
}

#[test]
fn epoch_and_text_helpers_reject_noncanonical_values() {
    assert_eq!(parse_epoch_string("1785291905").unwrap(), 1_785_291_905);
    for invalid in [
        "",
        " 1",
        "1 ",
        "1\n",
        "abc",
        "-1",
        "0",
        "01",
        "999999999999999999999999999999999999",
    ] {
        assert!(parse_epoch_string(invalid).is_err(), "{invalid:?}");
    }
    assert_eq!(
        format_epoch(1_785_291_905).unwrap(),
        "2026-07-29T10:25:05+08:00"
    );
    assert!(format_epoch(i64::MAX).is_err());

    checked_text("safe", "field").unwrap();
    for invalid in ["", " ", " padded", "padded ", "unsafe\ntext"] {
        assert!(checked_text(invalid, "field").is_err(), "{invalid:?}");
    }
}
