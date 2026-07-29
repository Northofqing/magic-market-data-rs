use encoding_rs::GB18030;
use magic_market_core::{
    EconomicObservationStatus, EconomicPeriod, EconomicSeriesKey, EconomicSeriesRequest,
    PositiveU32, ProviderId,
};
use magic_pbc_rs::{descriptor_for_year, parse_money_supply_table};

fn request() -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        ["M0", "M1", "M2"]
            .into_iter()
            .map(|code| EconomicSeriesKey::new(ProviderId::Pbc, "money-supply", code).unwrap())
            .collect(),
        EconomicPeriod::month(2024, 1).unwrap(),
        EconomicPeriod::month(2024, 12).unwrap(),
        PositiveU32::new(100).unwrap(),
    )
    .unwrap()
}

fn parse(
    source: &[u8],
) -> Result<
    magic_market_core::DataBatch<magic_market_core::EconomicObservation>,
    magic_pbc_rs::PbcError,
> {
    parse_money_supply_table(
        source,
        descriptor_for_year(2024).unwrap(),
        &request(),
        "observed",
        "batch",
    )
}

fn remove_row(source: &str, class: &str) -> String {
    let marker = format!("<tr class={class}");
    let start = source.find(&marker).unwrap();
    let end = start + source[start..].find("</tr>").unwrap() + "</tr>".len();
    format!("{}{}", &source[..start], &source[end..])
}

#[test]
fn official_excel_shape_maps_live_values_and_blank_months() {
    let batch = parse(include_bytes!("fixtures/money-supply-2024.html")).unwrap();
    assert_eq!(batch.records().len(), 36);
    let m0_january = batch
        .records()
        .iter()
        .find(|row| {
            row.series().code() == "M0" && row.period() == &EconomicPeriod::month(2024, 1).unwrap()
        })
        .unwrap();
    assert_eq!(m0_january.value().unwrap().get(), 121_398.54);
    assert_eq!(m0_january.status(), &EconomicObservationStatus::Present);
    let m2_october = batch
        .records()
        .iter()
        .find(|row| {
            row.series().code() == "M2" && row.period() == &EconomicPeriod::month(2024, 10).unwrap()
        })
        .unwrap();
    assert_eq!(m2_october.value().unwrap().get(), 3_097_092.01);
    assert_eq!(
        batch
            .records()
            .iter()
            .filter(|row| row.status() == &EconomicObservationStatus::Missing)
            .count(),
        6
    );
}

#[test]
fn title_unit_and_month_header_drift_fail_closed() {
    let source = include_str!("fixtures/money-supply-2024.html");
    for mutated in [
        source.replacen(">货币供应量<", ">错误标题<", 1),
        source.replacen(">Money Supply<", ">Wrong Supply<", 1),
        source.replacen("单位：亿元人民币", "单位：元", 1),
        source.replacen("Unit:100 Million Yuan", "Unit:Yuan", 1),
        source.replacen(">项目 Item<", ">项目<", 1),
        source.replacen("2024.12 ", "2024.11 ", 1),
    ] {
        assert!(parse(mutated.as_bytes()).is_err());
    }
}

#[test]
fn bilingual_pairs_hierarchy_value_spans_and_tail_are_strict() {
    let source = include_str!("fixtures/money-supply-2024.html");
    for mutated in [
        source.replacen("Money &amp; Quasi-money", "Wrong English M2", 1),
        source.replacen("<td colspan=2><span", "<td colspan=1><span", 1),
        source.replacen("<td rowspan=2>2976250.20", "<td>2976250.20", 1),
        source.replacen("<td class=tail></td>", "<td class=tail>unexpected</td>", 1),
        source.replacen("<td class=tail></td>", "", 1),
    ] {
        assert!(parse(mutated.as_bytes()).is_err());
    }

    let swapped = source
        .replacen("Money &amp; Quasi-money", "TEMP ENGLISH", 1)
        .replacen(">Money</td>", ">Money &amp; Quasi-money</td>", 1)
        .replacen("TEMP ENGLISH", "Money", 1);
    assert!(parse(swapped.as_bytes()).is_err());
    assert!(parse(remove_row(source, "m2-en").as_bytes()).is_err());
}

#[test]
fn duplicate_targets_and_rows_at_the_note_boundary_fail_closed() {
    let source = include_str!("fixtures/money-supply-2024.html");
    let table_start = source.find("<table").unwrap();
    let table_end = source.find("</table>").unwrap() + "</table>".len();
    let table = &source[table_start..table_end];
    let duplicated = source.replace("</body>", &format!("{table}</body>"));
    assert!(parse(duplicated.as_bytes()).is_err());

    let injected = source.replace(
        "<tr class=note-zh",
        "<tr class=injected><td colspan=15>M2 999999</td><td></td></tr><tr class=note-zh",
    );
    assert!(parse(injected.as_bytes()).is_err());

    let spoofed_history = source.replacen("2022.12 ", "2022.11 ", 1);
    assert!(parse(spoofed_history.as_bytes()).is_err());
    let spoofed_note = source.replacen("注：自2022年12月起", "说明：自2022年12月起", 1);
    assert!(parse(spoofed_note.as_bytes()).is_err());
}

#[test]
fn surrounding_text_cannot_spoof_the_table_contract() {
    let source = include_str!("fixtures/money-supply-2024.html");
    let surrounding = source.replace(
        "摘要文字中的 999.9 不是数据。",
        "货币供应量 Money Supply 单位：亿元人民币 2024.12 M2 999999",
    );
    assert_eq!(parse(surrounding.as_bytes()).unwrap().records().len(), 36);
}

#[test]
fn request_year_is_bound_to_descriptor_year() {
    let request_2025 = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Pbc, "money-supply", "M2").unwrap()],
        EconomicPeriod::month(2025, 1).unwrap(),
        EconomicPeriod::month(2025, 12).unwrap(),
        PositiveU32::new(12).unwrap(),
    )
    .unwrap();
    assert!(parse_money_supply_table(
        include_bytes!("fixtures/money-supply-2024.html"),
        descriptor_for_year(2024).unwrap(),
        &request_2025,
        "observed",
        "batch",
    )
    .is_err());
}

#[test]
fn excel_markup_is_bounded_and_malformed_variants_fail_closed() {
    let source = include_str!("fixtures/money-supply-2024.html");
    for mutated in [
        source.replacen("rowspan=2", "data-rowspan=2", 1),
        source.replacen("<span ", "<script ", 1),
        source.replacen("</span>", "</script>", 1),
        source.replacen(
            "<![if supportMisalignedColumns]>",
            "<![if unsupportedCondition]>",
            1,
        ),
        source.replacen("</td></tr>", "</tr></td>", 1),
    ] {
        assert!(parse(mutated.as_bytes()).is_err());
    }
    let without_conditionals = source
        .replace("<![if supportMisalignedColumns]>", "")
        .replace("<![endif]>", "");
    assert!(parse(without_conditionals.as_bytes()).is_err());
}

#[test]
fn utf8_and_strict_gb18030_are_supported_but_malformed_bytes_fail() {
    let fixture = include_str!("fixtures/money-supply-2024.html");
    assert_eq!(parse(fixture.as_bytes()).unwrap().records().len(), 36);

    let (encoded, _, had_errors) = GB18030.encode(fixture);
    assert!(!had_errors);
    assert_eq!(parse(&encoded).unwrap().records().len(), 36);

    let error = parse(&[0x81]).unwrap_err();
    assert!(matches!(error, magic_pbc_rs::PbcError::Decode(_)));
}
