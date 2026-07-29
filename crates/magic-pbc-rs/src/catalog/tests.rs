use super::*;

const URL: &str =
    "https://www.pbc.gov.cn/eportal/fileDir/diaochatongjisi/resource/cms/2024/table.htm";

#[test]
fn descriptor_constructor_and_accessors_are_exact() {
    let descriptor = PbcTableDescriptor::new(
        2024,
        "money-supply",
        URL,
        "货币供应量",
        "Money Supply",
        "亿元人民币",
        "100 Million Yuan",
    )
    .unwrap();
    assert_eq!(descriptor.year(), 2024);
    assert_eq!(descriptor.namespace(), "money-supply");
    assert_eq!(descriptor.canonical_url(), URL);
    assert_eq!(descriptor.title_zh(), "货币供应量");
    assert_eq!(descriptor.title_en(), "Money Supply");
    assert_eq!(descriptor.unit_zh(), "亿元人民币");
    assert_eq!(descriptor.unit_en(), "100 Million Yuan");
    assert_eq!(descriptor_for_year(2024).unwrap().year(), 2024);
    assert!(descriptor_for_year(2025).is_err());
}

#[test]
fn descriptor_rejects_every_unverified_fact_and_url_shape() {
    let facts = [
        (
            1899,
            "money-supply",
            "货币供应量",
            "Money Supply",
            "亿元人民币",
            "100 Million Yuan",
        ),
        (
            2024,
            "other",
            "货币供应量",
            "Money Supply",
            "亿元人民币",
            "100 Million Yuan",
        ),
        (
            2024,
            "money-supply",
            "wrong",
            "Money Supply",
            "亿元人民币",
            "100 Million Yuan",
        ),
        (
            2024,
            "money-supply",
            "货币供应量",
            "wrong",
            "亿元人民币",
            "100 Million Yuan",
        ),
        (
            2024,
            "money-supply",
            "货币供应量",
            "Money Supply",
            "wrong",
            "100 Million Yuan",
        ),
        (
            2024,
            "money-supply",
            "货币供应量",
            "Money Supply",
            "亿元人民币",
            "wrong",
        ),
    ];
    for (year, namespace, zh, en, unit_zh, unit_en) in facts {
        assert!(PbcTableDescriptor::new(year, namespace, URL, zh, en, unit_zh, unit_en).is_err());
    }
    for url in [
        "https://example.com/2024/table.htm",
        "https://www.pbc.gov.cn/eportal/fileDir/diaochatongjisi/resource/cms/2025/table.htm",
        "https://www.pbc.gov.cn/eportal/fileDir/diaochatongjisi/resource/cms/2024/table.pdf",
    ] {
        assert!(PbcTableDescriptor::new(
            2024,
            "money-supply",
            url,
            "货币供应量",
            "Money Supply",
            "亿元人民币",
            "100 Million Yuan",
        )
        .is_err());
    }
}
