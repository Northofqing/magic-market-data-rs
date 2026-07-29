use magic_cfets_rs::parse_central_parity_pages;
use magic_market_core::{
    CurrencyCode, IsoDate, OfficialFxFixingIdentity, OfficialFxFixingRequest, PositiveU32,
    ProviderId,
};

fn request() -> OfficialFxFixingRequest {
    OfficialFxFixingRequest::new(
        [("USD", "CNY"), ("JPY", "CNY"), ("CNY", "KRW")]
            .into_iter()
            .map(|(base, quote)| {
                OfficialFxFixingIdentity::new(
                    ProviderId::Cfets,
                    CurrencyCode::new(base).unwrap(),
                    CurrencyCode::new(quote).unwrap(),
                )
                .unwrap()
            })
            .collect(),
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(100).unwrap(),
    )
    .unwrap()
}

#[test]
fn filtered_values_align_with_currency_and_searchlist_not_full_head() {
    let pages = [
        include_bytes!("fixtures/ccpr-page-1.json").as_slice(),
        include_bytes!("fixtures/ccpr-page-2.json").as_slice(),
    ];
    let batch = parse_central_parity_pages(&pages, &request(), "observed", "batch").unwrap();
    assert_eq!(batch.records().len(), 6);
    assert_eq!(
        batch
            .records()
            .iter()
            .map(|row| (row.base().as_str(), row.quote().as_str()))
            .collect::<Vec<_>>(),
        [
            ("USD", "CNY"),
            ("USD", "CNY"),
            ("JPY", "CNY"),
            ("JPY", "CNY"),
            ("CNY", "KRW"),
            ("CNY", "KRW"),
        ]
    );
    let jpy = batch
        .records()
        .iter()
        .find(|row| row.base().as_str() == "JPY")
        .unwrap();
    assert_eq!(jpy.quotation_base().get(), 100);
    let krw = batch
        .records()
        .iter()
        .find(|row| row.quote().as_str() == "KRW")
        .unwrap();
    assert_eq!(krw.value().get(), 193.5);
}

#[test]
fn selected_order_and_pagination_mutations_fail_closed() {
    let first = include_str!("fixtures/ccpr-page-1.json")
        .replace("\"USD/CNY\", \"100JPY/CNY\"", "\"100JPY/CNY\", \"USD/CNY\"");
    let pages = [
        first.as_bytes(),
        include_bytes!("fixtures/ccpr-page-2.json").as_slice(),
    ];
    assert!(parse_central_parity_pages(&pages, &request(), "observed", "batch").is_err());

    let reordered_head = include_str!("fixtures/ccpr-page-1.json")
        .replace("\"USD/CNY\", \"EUR/CNY\"", "\"EUR/CNY\", \"USD/CNY\"");
    let pages = [
        reordered_head.as_bytes(),
        include_bytes!("fixtures/ccpr-page-2.json").as_slice(),
    ];
    assert!(parse_central_parity_pages(&pages, &request(), "observed", "batch").is_err());

    let empty = include_str!("fixtures/ccpr-page-1.json")
        .replace("\"total\": 2, \"pageTotal\": 2", "\"total\": 0, \"pageTotal\": 1")
        .replace(
            "\"records\": [{\"date\":\"2026-07-29\",\"values\":[\"6.7928\",\"4.5660\",\"193.72\"]}]",
            "\"records\": []",
        );
    let pages = [empty.as_bytes()];
    assert!(parse_central_parity_pages(&pages, &request(), "observed", "batch").is_err());
}
