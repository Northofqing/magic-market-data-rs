use super::*;
use magic_market_core::{EconomicSeriesKey, PositiveU32};

fn request_for_months(start_month: u32, end_month: u32) -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Pbc, "money-supply", "M2").unwrap()],
        EconomicPeriod::month(2024, start_month).unwrap(),
        EconomicPeriod::month(2024, end_month).unwrap(),
        PositiveU32::new(12).unwrap(),
    )
    .unwrap()
}

fn audited_grid() -> Vec<Vec<ExpandedCell>> {
    let descriptor = crate::descriptor_for_year(2024).unwrap();
    let html = std::str::from_utf8(include_bytes!(
        "../../tests/fixtures/money-supply-2024.html"
    ))
    .unwrap();
    let table = table_slices(html)
        .unwrap()
        .into_iter()
        .find(|table| has_exact_title_pair(table, descriptor).unwrap())
        .unwrap();
    parse_grid(table).unwrap()
}

#[test]
fn methodology_revision_is_scoped_to_m1_from_2025() {
    let revision = EconomicRevision {
        kind: magic_market_core::EconomicRevisionKind::SourceDefined(
            NonEmptyText::new("source-defined").unwrap(),
        ),
        label: None,
    };
    assert!(applicable_revision(
        "M1",
        &EconomicPeriod::month(2025, 1).unwrap(),
        Some(&revision)
    )
    .is_some());
    assert!(applicable_revision(
        "M0",
        &EconomicPeriod::month(2025, 1).unwrap(),
        Some(&revision)
    )
    .is_none());
    assert!(applicable_revision(
        "M1",
        &EconomicPeriod::month(2024, 12).unwrap(),
        Some(&revision)
    )
    .is_none());
}

#[test]
fn charset_decoding_and_scalar_parsers_cover_every_public_shape() {
    assert_eq!(declared_charset(None).unwrap(), None);
    assert_eq!(
        declared_charset(Some("text/html; charset=\"UTF-8\"")).unwrap(),
        Some(HtmlCharset::Utf8)
    );
    assert_eq!(
        declared_charset(Some("text/html; ignored; charset=UTF-8")).unwrap(),
        Some(HtmlCharset::Utf8)
    );
    for label in ["gbk", "gb2312", "gb18030"] {
        assert_eq!(
            declared_charset(Some(&format!("text/html; x=1; charset={label}"))).unwrap(),
            Some(HtmlCharset::Gb18030)
        );
    }
    assert!(declared_charset(Some("text/html; charset=utf-8; charset=gbk")).is_err());
    assert!(declared_charset(Some("text/html; charset=shift_jis")).is_err());
    assert_eq!(decode_utf8(b"\xef\xbb\xbftext").unwrap(), "text");
    assert!(decode_utf8(&[0xff]).is_err());
    assert_eq!(
        decode_official_html(b"text", Some("text/html; charset=utf8")).unwrap(),
        "text"
    );
    assert_eq!(
        parse_header_month(" 2025.01 ").unwrap(),
        EconomicPeriod::month(2025, 1).unwrap()
    );
    for value in ["", "2025-01", "2025.13", "abcd.01"] {
        assert!(parse_header_month(value).is_err());
    }
    assert_eq!(parse_value("1,234.5").unwrap().unwrap().get(), 1234.5);
    assert!(parse_value("—").unwrap().is_none());
    assert!(parse_value("bad").is_err());
    assert!(parse_value("-1").is_err());
}

#[test]
fn cell_span_attribute_and_grid_helpers_fail_closed() {
    assert_eq!(parse_cell_spans(" rowspan='2' colspan=3").unwrap(), (2, 3));
    assert_eq!(parse_cell_spans(" class=x").unwrap(), (1, 1));
    for attrs in [
        "rowspan=2",
        " rowspan",
        " rowspan=0",
        " rowspan=17",
        " rowspan=2 rowspan=2",
        " colspan=2 colspan=2",
        " ?=x",
        " rowspan='2",
    ] {
        assert!(parse_cell_spans(attrs).is_err(), "{attrs}");
    }
    let mut cursor = 0;
    assert_eq!(
        parse_attribute_value("'two words'", &mut cursor).unwrap(),
        "two words"
    );
    let mut cursor = 0;
    assert!(parse_attribute_value("", &mut cursor).is_err());
    assert!(parse_span_token("rowspan", None).is_err());
    assert!(parse_span_token("rowspan", Some("x")).is_err());
    assert!(parse_span_token("rowspan", Some("0")).is_err());
    assert!(parse_grid("<tr><td rowspan=17>x</td></tr>").is_err());
    assert!(parse_grid(&"<tr></tr>".repeat(257)).is_err());
    assert!(parse_cells(&format!("<td>{}</td>", "x".repeat(MAX_CELL_CHARS + 1))).is_err());
}

#[test]
fn table_markup_parent_text_and_entity_guards_are_exhaustive() {
    assert_eq!(strip_tags("<b>A</b>&nbsp;B").unwrap(), " A  B");
    assert!(strip_tags("<b").is_err());
    assert_eq!(decode_entities("&lt;&gt;&quot;&amp;").unwrap(), "<>\"&");
    assert!(decode_entities("&unknown;").is_err());
    assert_eq!(find_tag_end("<td title='>'>x", 0).unwrap(), 13);
    assert!(find_tag_end("<td title='x'", 0).is_err());
    for (name, parent) in [
        ("table", None),
        ("caption", Some("table")),
        ("tr", Some("table")),
        ("td", Some("tr")),
        ("p", Some("td")),
        ("span", Some("p")),
        ("br", Some("span")),
    ] {
        assert!(validate_parent(name, parent).is_ok());
    }
    assert!(validate_parent("unknown", None).is_err());
    assert!(validate_parent("td", Some("table")).is_err());
    assert!(validate_direct_text(" ", Some("table")).is_ok());
    assert!(validate_direct_text("text", Some("td")).is_ok());
    assert!(validate_direct_text("text", Some("table")).is_err());
    for markup in [
        "<![if supportMisalignedColumns]>",
        "<![endif]>",
        "<table><br/></table>",
        "<table></br></table>",
        "<table></table attr>",
        "</table>",
        "<table><tr></table>",
        "<table><script></script></table>",
        "<table>stray",
        "<table>",
    ] {
        assert!(validate_allowed_tags(markup).is_err(), "{markup}");
    }
}

#[test]
fn expanded_cell_origin_helpers_distinguish_spans() {
    let cell = ExpandedCell {
        text: String::new(),
        origin_row: 2,
        origin_column: 3,
        rowspan: 1,
        colspan: 1,
    };
    assert!(canonical_origin(&cell, 2, 3, 1, 1));
    assert!(same_origin(&cell, &cell));
    assert!(independent_blank(&cell, 2, 3));
    assert_eq!(
        canonical_label("M2"),
        "货币和准货币（M2） / Money & Quasi-money"
    );
    assert_eq!(canonical_label("M1"), "货币（M1） / Money");
    assert_eq!(
        canonical_label("M0"),
        "流通中货币（M0） / Currency in Circulation"
    );
}

#[test]
fn response_size_table_selection_and_range_filtering_are_bounded() {
    let descriptor = crate::descriptor_for_year(2024).unwrap();
    assert!(matches!(
        parse_money_supply_response(
            &vec![b'x'; MAX_HTML_BYTES + 1],
            None,
            descriptor,
            &request_for_months(1, 12),
            "observed",
            "batch",
        ),
        Err(PbcError::Protocol(_))
    ));
    assert!(table_slices("no table").unwrap().is_empty());
    assert!(table_slices("<table><tr></tr>").is_err());
    assert_eq!(
        table_slices("<table></table><table></table>")
            .unwrap()
            .len(),
        2
    );
    assert!(
        !has_exact_title_pair("<table><tr><td>only one row</td></tr></table>", descriptor).unwrap()
    );

    let fixture = include_bytes!("../../tests/fixtures/money-supply-2024.html");
    let batch = parse_money_supply_response(
        fixture,
        Some("text/html; charset=utf-8"),
        descriptor,
        &request_for_months(6, 6),
        "observed",
        "batch",
    )
    .unwrap();
    assert_eq!(batch.records().len(), 1);
    assert_eq!(
        batch.records()[0].period(),
        &EconomicPeriod::month(2024, 6).unwrap()
    );
}

#[test]
fn grid_attribute_percent_and_conditional_helpers_cover_boundary_failures() {
    assert!(parse_official_grid(&[], crate::descriptor_for_year(2024).unwrap()).is_err());
    assert!(parse_grid("").is_err());
    assert!(parse_grid("<tr></tr>").is_err());
    assert!(parse_grid(&format!(
        "<tr>{}</tr>",
        "<td>x</td>".repeat(MAX_COLUMNS + 1)
    ))
    .is_err());
    let near_limit = format!(
        "{}<tr><td rowspan=2>x</td></tr>",
        "<tr><td>x</td></tr>".repeat(MAX_ROWS - 1)
    );
    assert!(parse_grid(&near_limit).is_err());
    assert!(parse_grid("<tr><td>x</td><td>y</td></tr><tr><td>z</td></tr>").is_err());
    assert!(parse_cells("<tr><th>x</th></tr>").is_ok());
    assert!(parse_cells("<td>unterminated").is_err());

    let mut cursor = 0;
    assert!(parse_attribute_value(" ", &mut cursor).is_err());
    for percent in ["1", "bad%", "inf%"] {
        assert!(parse_percent(percent).is_err(), "{percent}");
    }
    assert!(parse_percent("-1.25%").is_ok());
    assert!(validate_allowed_tags(
        "<table><![if supportMisalignedColumns]><tr></tr><tr></tr><![endif]></table>"
    )
    .is_err());

    let bad_blank = ExpandedCell {
        text: "not blank".into(),
        origin_row: 0,
        origin_column: 0,
        rowspan: 1,
        colspan: 1,
    };
    assert!(validate_independent_blank_row(&[bad_blank], 0, "test").is_err());
}

#[test]
fn audited_grid_rejects_noncanonical_span_provenance_at_each_layer() {
    let descriptor = crate::descriptor_for_year(2024).unwrap();

    let mut grid = audited_grid();
    grid[5][3].origin_row = 4;
    assert!(validate_month_header(&grid[5], descriptor).is_err());

    let mut grid = audited_grid();
    grid[6][0].text = "not blank".into();
    assert!(validate_header_spacer(&grid[6], 6).is_err());

    let mut grid = audited_grid();
    grid[7][3].origin_row = 6;
    let layout = SeriesLayout {
        code: "M2",
        zh_row: 7,
        en_row: 8,
        label_column: 0,
        label_span: 3,
        zh_label: "货币和准货币（M2）",
        en_label: "Money & Quasi-money",
    };
    assert!(validate_series_pair(&grid, layout).is_err());

    let mut grid = audited_grid();
    grid[16][0].text = "not blank".into();
    assert!(validate_note_and_history_suffix(&grid).is_err());

    let mut grid = audited_grid();
    grid[17][2].text = "wrong identity".into();
    assert!(validate_note_and_history_suffix(&grid).is_err());

    let mut grid = audited_grid();
    grid[17][3].origin_row = 16;
    assert!(validate_note_and_history_suffix(&grid).is_err());
}
