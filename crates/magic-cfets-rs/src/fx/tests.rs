use super::*;

fn identity(provider: ProviderId, base: &str, quote: &str) -> OfficialFxFixingIdentity {
    OfficialFxFixingIdentity::new(
        provider,
        CurrencyCode::new(base).unwrap(),
        CurrencyCode::new(quote).unwrap(),
    )
    .unwrap()
}

fn request(provider: ProviderId) -> OfficialFxFixingRequest {
    OfficialFxFixingRequest::new(
        vec![identity(provider, "USD", "CNY")],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap()
}

fn single_page() -> String {
    include_str!("../../tests/fixtures/ccpr-page-1.json")
        .replace(
            "\"total\": 2, \"pageTotal\": 2",
            "\"total\": 1, \"pageTotal\": 1",
        )
        .replace(
            "\"currency\": \"USD/CNY,100JPY/CNY,CNY/KRW\"",
            "\"currency\": \"USD/CNY\"",
        )
        .replace(
            "\"searchlist\": [\"USD/CNY\", \"100JPY/CNY\", \"CNY/KRW\"]",
            "\"searchlist\": [\"USD/CNY\"]",
        )
        .replace(
            "\"values\":[\"6.7928\",\"4.5660\",\"193.72\"]",
            "\"values\":[\"6.7928\"]",
        )
}

fn rejected(body: String) {
    assert!(
        parse_central_parity_pages(&[body.as_bytes()], &request(ProviderId::Cfets), "o", "b")
            .is_err()
    );
}

#[test]
fn heading_and_page_count_contracts_cover_the_closed_catalog() {
    for (base, quote, heading, _) in HEADINGS {
        assert_eq!(
            source_heading(&identity(ProviderId::Cfets, base, quote)).unwrap(),
            heading
        );
    }
    assert!(source_heading(&identity(ProviderId::Fred, "USD", "CNY")).is_err());
    assert!(source_heading(&identity(ProviderId::Cfets, "EUR", "USD")).is_err());
    assert!(page_total(single_page().as_bytes()).is_ok());
    assert!(page_total(&vec![b'x'; MAX_RESPONSE_BYTES + 1]).is_err());
    assert!(page_total(b"{").is_err());
    assert!(page_total(
        single_page()
            .replace("\"pageTotal\": 1", "\"pageTotal\": 0")
            .as_bytes()
    )
    .is_err());
}

#[test]
fn pagination_metadata_rows_and_values_fail_closed() {
    let empty: [&[u8]; 0] = [];
    assert!(parse_central_parity_pages(&empty, &request(ProviderId::Cfets), "o", "b").is_err());
    let too_many = vec![single_page().into_bytes(); MAX_PAGES + 1];
    assert!(parse_central_parity_pages(&too_many, &request(ProviderId::Cfets), "o", "b").is_err());
    assert!(parse_central_parity_pages(
        &[single_page().as_bytes()],
        &request(ProviderId::Fred),
        "o",
        "b"
    )
    .is_err());
    rejected("{".into());
    rejected(single_page().replace("\"flagMessage\": \"\"", "\"flagMessage\": \"error\""));
    rejected(single_page().replace("\"pageNum\": 1", "\"pageNum\": 2"));
    rejected(single_page().replace("\"pageSize\": 1", "\"pageSize\": 0"));
    rejected(single_page().replace("\"currency\": \"USD/CNY\"", "\"currency\": \"EUR/CNY\""));
    rejected(single_page().replace("\"USD/CNY\", \"EUR/CNY\"", "\"EUR/CNY\", \"USD/CNY\""));
    rejected(single_page().replace("\"total\": 1", "\"total\": 0"));
    rejected(single_page().replace("\"values\":[\"6.7928\"]", "\"values\":[]"));
    rejected(single_page().replace("\"total\": 1", "\"total\": 2"));
    rejected(single_page().replace("\"date\":\"2026-07-29\"", "\"date\":\"2026-07-27\""));
    rejected(single_page().replace("\"6.7928\"", "\"bad\""));
    rejected(single_page().replace("\"6.7928\"", "\"0\""));
    rejected(
        single_page()
            .replace("\"total\": 1", "\"total\": 0")
            .replace(
                "\"records\": [{\"date\":\"2026-07-29\",\"values\":[\"6.7928\"]}]",
                "\"records\": []",
            ),
    );
}

#[test]
fn duplicate_date_identity_is_rejected_and_position_is_stable() {
    let duplicate = single_page()
        .replace("\"total\": 1", "\"total\": 2")
        .replace(
            "\"records\": [{\"date\":\"2026-07-29\",\"values\":[\"6.7928\"]}]",
            "\"records\": [{\"date\":\"2026-07-29\",\"values\":[\"6.7928\"]},{\"date\":\"2026-07-29\",\"values\":[\"6.7928\"]}]",
        );
    rejected(duplicate);
    let fixing = OfficialFxFixing::new(
        CurrencyCode::new("USD").unwrap(),
        CurrencyCode::new("CNY").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        FiniteNumber::new(6.7).unwrap(),
        PositiveU32::new(1).unwrap(),
        None,
        None,
        SourceEvidence::new(ProviderId::Cfets, "o", "b").unwrap(),
    )
    .unwrap();
    assert_eq!(
        request_pair_position(&request(ProviderId::Cfets), &fixing),
        Some(0)
    );
}

#[test]
fn per_page_size_cross_page_metadata_and_row_ceiling_are_enforced() {
    assert!(parse_central_parity_pages(
        &[vec![b'x'; MAX_RESPONSE_BYTES + 1]],
        &request(ProviderId::Cfets),
        "o",
        "b"
    )
    .is_err());

    let first = single_page().replace(
        "\"total\": 1, \"pageTotal\": 1",
        "\"total\": 2, \"pageTotal\": 2",
    );
    let second = first
        .replace("\"pageNum\": 1", "\"pageNum\": 2")
        .replace("\"total\": 2", "\"total\": 3");
    assert!(parse_central_parity_pages(
        &[first.as_bytes(), second.as_bytes()],
        &request(ProviderId::Cfets),
        "o",
        "b"
    )
    .is_err());

    let mut oversized: serde_json::Value = serde_json::from_str(&single_page()).unwrap();
    oversized["data"]["total"] = serde_json::json!(MAX_ROWS + 1);
    oversized["records"] = serde_json::Value::Array(vec![
        serde_json::json!({
            "date":"2026-07-29",
            "values":["6.7928"]
        });
        MAX_ROWS + 1
    ]);
    assert!(parse_central_parity_pages(
        &[serde_json::to_vec(&oversized).unwrap()],
        &request(ProviderId::Cfets),
        "o",
        "b"
    )
    .is_err());
}
