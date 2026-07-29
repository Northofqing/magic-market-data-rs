use super::*;
use magic_market_core::{EconomicSeriesKey, PositiveU32};

fn request(
    provider: ProviderId,
    namespace: &str,
    start: EconomicPeriod,
    end: EconomicPeriod,
) -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(provider, namespace, "A010101").unwrap()],
        start,
        end,
        PositiveU32::new(10).unwrap(),
    )
    .unwrap()
}

fn monthly_request() -> EconomicSeriesRequest {
    request(
        ProviderId::Nbs,
        "national-monthly",
        EconomicPeriod::month(2025, 6).unwrap(),
        EconomicPeriod::month(2025, 6).unwrap(),
    )
}

fn fixture() -> String {
    include_str!("../../tests/fixtures/national-monthly.json").to_owned()
}

fn rejected(body: String) {
    assert!(parse_national_monthly_payload(
        body.as_bytes(),
        &monthly_request(),
        "observed",
        "batch"
    )
    .is_err());
}

fn rejected_value(value: &serde_json::Value) {
    rejected(serde_json::to_string(value).unwrap());
}

#[test]
fn primitive_and_request_validation_are_exact() {
    assert_eq!(
        parse_month("202506").unwrap(),
        EconomicPeriod::month(2025, 6).unwrap()
    );
    for value in ["", "2025-06", "202513", "abc006"] {
        assert!(parse_month(value).is_err());
    }
    assert_eq!(
        parse_data_identity("zb.A010101_sj.202506").unwrap(),
        ("A010101", "202506")
    );
    for value in [
        "A010101_sj.202506",
        "zb.A010101.202506",
        "zb._sj.202506",
        "zb.A.1_sj.202506",
        "zb.A010101_sj.2025.06",
    ] {
        assert!(parse_data_identity(value).is_err());
    }
    assert!(validate_request(&request(
        ProviderId::Fred,
        "national-monthly",
        EconomicPeriod::month(2025, 6).unwrap(),
        EconomicPeriod::month(2025, 6).unwrap(),
    ))
    .is_err());
    assert!(validate_request(&request(
        ProviderId::Nbs,
        "regional",
        EconomicPeriod::month(2025, 6).unwrap(),
        EconomicPeriod::month(2025, 6).unwrap(),
    ))
    .is_err());
    assert!(validate_request(&request(
        ProviderId::Nbs,
        "national-monthly",
        EconomicPeriod::year(2025).unwrap(),
        EconomicPeriod::year(2025).unwrap(),
    ))
    .is_err());
}

#[test]
fn envelope_metadata_and_data_failures_are_atomic() {
    assert!(parse_national_monthly_payload(
        &vec![b'x'; max_response_bytes() + 1],
        &monthly_request(),
        "observed",
        "batch"
    )
    .is_err());
    rejected("{".into());
    rejected(fixture().replace("\"returncode\": 200", "\"returncode\": 500"));
    rejected(fixture().replace(
        "{\"wdcode\": \"sj\", \"nodes\": [{\"code\": \"202506\", \"name\": \"2025年6月\"}]}",
        "",
    ));
    rejected(fixture().replace("\"wdcode\": \"sj\"", "\"wdcode\": \"zb\""));
    rejected(fixture().replace("\"wdcode\": \"sj\"", "\"wdcode\": \"other\""));
    rejected(fixture().replace("\"code\": \"A010101\"", "\"code\": \"OTHER\""));
    rejected(fixture().replace("\"unit\": \"点\"", "\"unit\": \" \""));
    rejected(fixture().replace("\"code\": \"202506\"", "\"code\": \"202505\""));
    rejected(fixture().replace(
        "\"nodes\": [{\"code\": \"202506\", \"name\": \"2025年6月\"}]",
        "\"nodes\": []",
    ));
    rejected(fixture().replace(
        "\"datanodes\": [\n      {\"code\": \"zb.A010101_sj.202506\", \"data\": {\"data\": 0.0, \"hasdata\": true}}\n    ]",
        "\"datanodes\": []"
    ));
    rejected(fixture().replace("zb.A010101_sj.202506", "bad"));
    rejected(fixture().replace("\"hasdata\": true", "\"hasdata\": false"));
    rejected(fixture().replace("\"data\": 0.0", "\"data\": null"));
}

#[test]
fn explicit_missing_value_is_preserved() {
    let missing = fixture()
        .replace("\"data\": 0.0", "\"data\": null")
        .replace("\"hasdata\": true", "\"hasdata\": false");
    let batch =
        parse_national_monthly_payload(missing.as_bytes(), &monthly_request(), "observed", "batch")
            .unwrap();
    assert_eq!(
        batch.records()[0].status(),
        &EconomicObservationStatus::Missing
    );
    assert!(batch.records()[0].value().is_none());
}

#[test]
fn metadata_limits_duplicates_and_exact_coverage_fail_closed() {
    let mut value: serde_json::Value = serde_json::from_str(&fixture()).unwrap();
    value["returndata"]["wdnodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"wdcode":"extra","nodes":[]}));
    rejected_value(&value);

    let mut value: serde_json::Value = serde_json::from_str(&fixture()).unwrap();
    value["returndata"]["wdnodes"][0]["nodes"] = serde_json::Value::Array(vec![
        serde_json::json!({
            "code":"A010101",
            "name":"指标甲",
            "unit":"点"
        });
        MAX_METADATA_NODES
            + 1
    ]);
    rejected_value(&value);

    let mut duplicate_indicator: serde_json::Value = serde_json::from_str(&fixture()).unwrap();
    let indicator = duplicate_indicator["returndata"]["wdnodes"][0]["nodes"][0].clone();
    duplicate_indicator["returndata"]["wdnodes"][0]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(indicator);
    rejected_value(&duplicate_indicator);

    let mut duplicate_period: serde_json::Value = serde_json::from_str(&fixture()).unwrap();
    let period = duplicate_period["returndata"]["wdnodes"][1]["nodes"][0].clone();
    duplicate_period["returndata"]["wdnodes"][1]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(period);
    rejected_value(&duplicate_period);

    let two_series = EconomicSeriesRequest::new(
        ["A010101", "A010102"]
            .into_iter()
            .map(|code| EconomicSeriesKey::new(ProviderId::Nbs, "national-monthly", code).unwrap())
            .collect(),
        EconomicPeriod::month(2025, 6).unwrap(),
        EconomicPeriod::month(2025, 6).unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    assert!(
        parse_national_monthly_payload(fixture().as_bytes(), &two_series, "observed", "batch")
            .is_err()
    );

    let mut missing_data: serde_json::Value = serde_json::from_str(&fixture()).unwrap();
    missing_data["returndata"]["wdnodes"][1]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"code":"202505","name":"2025年5月"}));
    rejected_value(&missing_data);

    let two_month_request = request(
        ProviderId::Nbs,
        "national-monthly",
        EconomicPeriod::month(2025, 5).unwrap(),
        EconomicPeriod::month(2025, 6).unwrap(),
    );
    assert!(parse_national_monthly_payload(
        &serde_json::to_vec(&missing_data).unwrap(),
        &two_month_request,
        "observed",
        "batch"
    )
    .is_err());

    let mut unknown_identity: serde_json::Value = serde_json::from_str(&fixture()).unwrap();
    unknown_identity["returndata"]["datanodes"][0]["code"] =
        serde_json::Value::String("zb.OTHER_sj.202506".into());
    rejected_value(&unknown_identity);
}

#[test]
fn multiple_series_and_periods_are_sorted_canonically() {
    let request = EconomicSeriesRequest::new(
        ["A010102", "A010101"]
            .into_iter()
            .map(|code| EconomicSeriesKey::new(ProviderId::Nbs, "national-monthly", code).unwrap())
            .collect(),
        EconomicPeriod::month(2025, 5).unwrap(),
        EconomicPeriod::month(2025, 6).unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    let value = serde_json::json!({
        "returncode": 200,
        "returndata": {
            "wdnodes": [
                {"wdcode":"zb","nodes":[
                    {"code":"A010101","name":"甲","unit":"点"},
                    {"code":"A010102","name":"乙","unit":"点"}
                ]},
                {"wdcode":"sj","nodes":[
                    {"code":"202506","name":"2025年6月"},
                    {"code":"202505","name":"2025年5月"}
                ]}
            ],
            "datanodes": [
                {"code":"zb.A010102_sj.202506","data":{"data":4.0,"hasdata":true}},
                {"code":"zb.A010101_sj.202506","data":{"data":2.0,"hasdata":true}},
                {"code":"zb.A010102_sj.202505","data":{"data":3.0,"hasdata":true}},
                {"code":"zb.A010101_sj.202505","data":{"data":1.0,"hasdata":true}}
            ]
        }
    });
    let body = serde_json::to_vec(&value).unwrap();
    let batch = parse_national_monthly_payload(&body, &request, "observed", "batch").unwrap();
    let identities = batch
        .records()
        .iter()
        .map(|row| (row.series().code(), row.period().clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        vec![
            ("A010101", EconomicPeriod::month(2025, 5).unwrap()),
            ("A010101", EconomicPeriod::month(2025, 6).unwrap()),
            ("A010102", EconomicPeriod::month(2025, 5).unwrap()),
            ("A010102", EconomicPeriod::month(2025, 6).unwrap()),
        ]
    );
}
