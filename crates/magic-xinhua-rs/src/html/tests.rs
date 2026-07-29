use super::*;

const HREF: &str = "//www.cnfin.com/yw-lb/detail/20260729/12345_1.html";

fn anchor(title: &str) -> String {
    format!(r#"<h3><a href="{HREF}" target="_blank">{title}</a></h3>"#)
}

#[test]
fn split_rows_rejects_missing_ambiguous_empty_and_oversized_lists() {
    assert!(split_rows("no rows").is_err());
    assert!(split_rows(&format!("{ROW_START}row")).is_err());
    assert!(split_rows(&format!("{ROW_START}row{LIST_END}{ROW_START}outside")).is_err());
    assert!(split_rows(&format!("{ROW_START}   {LIST_END}")).is_err());

    let fourteen = format!("{}{LIST_END}", ROW_START.repeat(MAX_SOURCE_ROWS + 1));
    assert!(split_rows(&fourteen).is_err());
    let two = format!("{ROW_START}first{ROW_START}second{LIST_END}");
    assert_eq!(split_rows(&two).unwrap(), ["first", "second"]);
}

#[test]
fn canonical_anchor_requires_exact_attributes_target_and_cardinality() {
    assert_eq!(
        canonical_anchor(&anchor("A &amp; B")).unwrap(),
        (HREF.into(), "A & B".into())
    );
    assert!(canonical_anchor("missing").is_err());
    assert!(canonical_anchor("<h3><a href=\"x\"").is_err());
    assert!(canonical_anchor("<h3><a href=\"x\" target=\"_blank\">title").is_err());
    assert!(canonical_anchor(&format!(
        r#"<h3><a href="{HREF}" target="_blank" class="extra">title</a></h3>"#
    ))
    .is_err());
    assert!(canonical_anchor(&format!(
        r#"<h3><a href="{HREF}" target="_self">title</a></h3>"#
    ))
    .is_err());
    assert!(canonical_anchor(&format!("{}{}", anchor("one"), anchor("two"))).is_err());
}

#[test]
fn attribute_parser_and_lookup_reject_every_ambiguous_shape() {
    assert_eq!(
        parse_attributes(r#"href="value" target="_blank""#).unwrap(),
        [
            ("href".into(), "value".into()),
            ("target".into(), "_blank".into())
        ]
    );
    for invalid in [
        r#"= "value""#,
        r#"bad@name="value""#,
        r#"href "value""#,
        "href='value'",
        r#"href="value"#,
    ] {
        assert!(parse_attributes(invalid).is_err(), "{invalid:?}");
    }
    let attributes = vec![("href".into(), "one".into()), ("href".into(), "two".into())];
    assert!(unique_attribute(&attributes, "target").is_err());
    assert!(unique_attribute(&attributes, "href").is_err());
}

#[test]
fn exact_tag_helpers_reject_missing_duplicate_and_unclosed_fields() {
    assert_eq!(
        exact_tag_text("<time> A &amp; B </time>", "<time>", "</time>", "time").unwrap(),
        "A & B"
    );
    assert_eq!(
        exact_tag_raw(
            "<source> A <b>B</b> </source>",
            "<source>",
            "</source>",
            "source"
        )
        .unwrap(),
        "A <b>B</b>"
    );
    for row in [
        "missing",
        "<time>one</time><time>two</time>",
        "<time>unclosed",
    ] {
        assert!(exact_tag_text(row, "<time>", "</time>", "time").is_err());
        assert!(exact_tag_raw(row, "<time>", "</time>", "time").is_err());
    }
}

#[test]
fn path_and_beijing_time_parsers_cover_closed_formats() {
    assert_eq!(
        parse_path(HREF).unwrap(),
        ("20260729".into(), "12345".into())
    );
    for invalid in [
        "https://www.cnfin.com/yw-lb/detail/20260729/12345_1.html",
        "//www.cnfin.com/yw-lb/detail/20260729/12345_1.html?x=1",
        "//www.cnfin.com/yw-lb/detail/20260729/12345_1.html#x",
        "//www.cnfin.com/yw-lb/detail/20260729\\12345_1.html",
        "//www.cnfin.com/yw-lb/detail/20260729/12 345_1.html",
        "//www.cnfin.com/yw-lb/detail/missing_1.html",
        "//www.cnfin.com/yw-lb/detail/2026072/12345_1.html",
        "//www.cnfin.com/yw-lb/detail/2026072x/12345_1.html",
        "//www.cnfin.com/yw-lb/detail/20260729/_1.html",
        "//www.cnfin.com/yw-lb/detail/20260729/abc_1.html",
    ] {
        assert!(parse_path(invalid).is_err(), "{invalid:?}");
    }
    let (normalized, epoch) = parse_beijing("2026-07-29 10:30:05").unwrap();
    assert_eq!(normalized, "2026-07-29T10:30:05+08:00");
    assert!(epoch > 0);
    assert!(parse_beijing("2026-07-29T10:30:05").is_err());
}

#[test]
fn metadata_decoder_handles_only_the_closed_entity_set() {
    assert_eq!(
        decode_text("  A&amp;B &lt;x&gt; &quot;q&quot; &apos;a&apos; &#65; &#x42;  ").unwrap(),
        "A&B <x> \"q\" 'a' A B"
    );
    for invalid in [
        "<b>nested</b>",
        "&unclosed",
        "&unknown;",
        "&#bad;",
        "&#xzz;",
        "&#x110000;",
        "   ",
        "line\nbreak",
    ] {
        assert!(decode_text(invalid).is_err(), "{invalid:?}");
    }
}
