use super::*;

fn row(id: u64, title: &str, source: &str, date: &str, url: &str) -> SourceRow {
    SourceRow {
        NewsID: id,
        NewsTitle: title.into(),
        CreateDate: date.into(),
        NewsSource: source.into(),
        url: url.into(),
    }
}

#[test]
fn firstlist_extraction_rejects_malformed_scripts_and_payload_boundaries() {
    assert!(extract_firstlist("<scripture>var firstlist = [];</scripture>").is_err());
    assert!(extract_firstlist("<script").is_err());
    assert!(extract_firstlist("<script>var firstlist = []").is_err());
    assert!(extract_firstlist("<script>const other = [];</script>").is_err());
    assert!(extract_firstlist("<script>var firstlist = {};</script>").is_err());
    assert!(extract_firstlist("<script>var firstlist = [1;</script>").is_err());

    let oversized = format!(
        "<script>var firstlist = [{}];</script>",
        " ".repeat(MAX_EMBEDDED_JSON_BYTES)
    );
    assert!(extract_firstlist(&oversized).is_err());
    assert_eq!(
        extract_firstlist(
            r#"<script>var firstlist = [{"text":"] escaped \" text"}]; trailing();</script>"#
        )
        .unwrap(),
        r#"[{"text":"] escaped \" text"}]"#
    );
}

#[test]
fn executable_script_accepts_only_closed_attribute_grammar() {
    for attributes in [
        "",
        " defer",
        r#" type="text/javascript""#,
        " type='application/javascript'",
        r#" type="module""#,
    ] {
        assert!(executable_script(attributes).unwrap(), "{attributes:?}");
    }
    assert!(!executable_script(r#" type="application/json""#).unwrap());
    for attributes in [
        r#" = "value""#,
        r#" bad@name="value""#,
        " type=module",
        r#" type="module"#,
        r#" type="module" type="text/javascript""#,
    ] {
        assert!(executable_script(attributes).is_err(), "{attributes:?}");
    }
}

#[test]
fn javascript_scanner_ignores_decoys_and_finds_statement_assignments() {
    let script = r#"
        // var firstlist = [];
        /* var firstlist = []; */
        const string = "var firstlist = []";
        const template = `var firstlist = []`;
        const regex = /var firstlist\s*=\s*\[\]/gi;
        ; 123.45e6;
        { var /* trivia */ firstlist /* trivia */ = [{"ok":true}]; }
    "#;
    let offsets = executable_assignment_offsets(script);
    assert_eq!(offsets.len(), 1);
    assert!(script[offsets[0]..].trim_start().starts_with('['));
    assert!(executable_assignment_offsets("return /regex/; @ value;").is_empty());

    assert_eq!(skip_js_trivia(b" \n// line\n/* block */name", 0), 21);
    assert_eq!(skip_js_string(br#""a\\\"b"tail"#, 0), 8);
    assert_eq!(skip_js_string(b"`unterminated", 0), 13);
    assert_eq!(skip_js_regex(br#"/a[\/]b/gi tail"#, 0), 10);
    assert_eq!(skip_js_regex(b"/unterminated\nrest", 0), 18);
    assert!(is_identifier_start(b'a'));
    assert!(is_identifier_start(b'_'));
    assert!(is_identifier_start(b'$'));
    assert!(!is_identifier_start(b'1'));
    assert_eq!(identifier_end(b"name_1$ tail", 0), 7);
    assert_eq!(identifier_end(b"1name", 0), 0);
}

#[test]
fn row_and_display_text_helpers_cover_identity_and_time_errors() {
    let parsed = parse_row(row(
        1,
        " title ",
        " publisher ",
        "2026-07-29T10:25:00",
        "/news/1.html",
    ))
    .unwrap();
    assert_eq!(parsed.id, "1");
    assert_eq!(parsed.title, "title");
    assert_eq!(parsed.publisher, "publisher");
    assert_eq!(parsed.canonical_url, "https://www.yicai.com/news/1.html");
    assert_eq!(parsed.published_at, "2026-07-29T10:25:00+08:00");
    assert!(parsed.epoch > 0);

    assert!(parse_row(row(
        0,
        "title",
        "publisher",
        "2026-07-29T10:25:00",
        "/news/0.html"
    ))
    .is_err());
    assert!(parse_row(row(
        1,
        "title",
        "publisher",
        "2026-07-29T10:25:00",
        "/news/2.html"
    ))
    .is_err());
    assert!(parse_row(row(1, "title", "publisher", "not-a-time", "/news/1.html")).is_err());
    assert!(normalized_display_text(" \t ".into(), "field").is_err());
    assert!(normalized_display_text("unsafe\ntext".into(), "field").is_err());
    assert_eq!(
        normalized_display_text("  display　".into(), "field").unwrap(),
        "display"
    );
}
