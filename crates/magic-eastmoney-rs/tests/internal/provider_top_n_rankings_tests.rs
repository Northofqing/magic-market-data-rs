use super::*;
use crate::test_support::ScriptedTransport;

fn request(kind: MarketRankingKind, limit: u32) -> ProviderTopNRankingRequest {
    ProviderTopNRankingRequest::new(
        kind,
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(limit).unwrap(),
        NonEmptyText::new(A_SHARE_FILTER).unwrap(),
    )
    .unwrap()
}

fn fixture() -> Vec<u8> {
    r#"{
      "rc":0,
      "data":{
        "total":5542,
        "diff":[
          {"f10":9.8,"f12":"600396","f13":1,"f14":"华电辽能","f62":380000000.0,"f297":20260729},
          {"f10":8.1,"f12":"002396","f13":0,"f14":"星网锐捷","f62":300000000.0,"f297":"20260729"}
        ]
      }
    }"#
    .as_bytes()
    .to_vec()
}

#[test]
fn provider_top_n_batch_id_distinguishes_metrics_at_the_same_observation_time() {
    let observed_at = "2026-07-29T16:00:00+08:00";
    let volume_ratio = parse_provider_top_n(
        &fixture(),
        &request(MarketRankingKind::VolumeRatio, 2),
        observed_at,
    )
    .unwrap();
    let main_net_inflow = parse_provider_top_n(
        &fixture(),
        &request(MarketRankingKind::MainNetInflow, 2),
        observed_at,
    )
    .unwrap();

    assert_ne!(
        volume_ratio.provenance().batch_id(),
        main_net_inflow.provenance().batch_id()
    );
}

#[test]
fn provider_top_n_batch_id_distinguishes_normalized_response_content() {
    let request = request(MarketRankingKind::VolumeRatio, 2);
    let observed_at = "2026-07-29T16:00:00+08:00";
    let original = parse_provider_top_n(&fixture(), &request, observed_at).unwrap();
    let changed_fixture = String::from_utf8(fixture())
        .unwrap()
        .replace("华电辽能", "华电辽能股份");
    let changed = parse_provider_top_n(changed_fixture.as_bytes(), &request, observed_at).unwrap();

    assert_ne!(
        original.provenance().batch_id(),
        changed.provenance().batch_id()
    );
}

#[test]
fn provider_top_n_batch_id_uses_sha256_over_canonical_json() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    let request = request(MarketRankingKind::VolumeRatio, 2);
    let observed_at = "2026-07-29T16:00:00+08:00";
    let original = parse_provider_top_n(&fixture(), &request, observed_at).unwrap();
    let reordered = r#"{
      "data":{
        "diff":[
          {"f297":20260729,"f62":380000000.0,"f14":"华电辽能","f13":1,"f12":"600396","f10":9.8},
          {"f297":"20260729","f62":300000000.0,"f14":"星网锐捷","f13":0,"f12":"002396","f10":8.1}
        ],
        "total":5542
      },
      "rc":0
    }"#;
    let canonical_equivalent =
        parse_provider_top_n(reordered.as_bytes(), &request, observed_at).unwrap();

    assert_eq!(
        original.provenance().batch_id(),
        canonical_equivalent.provenance().batch_id()
    );

    let batch_id = original.provenance().batch_id().unwrap();
    let suffix = batch_id
        .strip_prefix("eastmoney-web:provider-top-n-ranking:v1:volume-ratio:2026-07-29:2:")
        .unwrap();
    let (filter_hash, suffix) = suffix.split_once(':').unwrap();
    let (content_hash, observed_at) = suffix.split_once(':').unwrap();
    assert_eq!(filter_hash.len(), 64);
    assert!(filter_hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(content_hash.len(), 64);
    assert!(content_hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(observed_at, "2026-07-29T16:00:00+08:00");
}

#[test]
fn canonical_response_identity_covers_boolean_provider_metadata() {
    let request = request(MarketRankingKind::VolumeRatio, 2);
    let observed_at = "2026-07-29T16:00:00+08:00";
    let mut with_boolean: Value = serde_json::from_slice(&fixture()).unwrap();
    with_boolean["provider_cache_hit"] = Value::Bool(true);
    let mut without_boolean = with_boolean.clone();
    without_boolean
        .as_object_mut()
        .unwrap()
        .remove("provider_cache_hit");

    let boolean_batch = parse_provider_top_n(
        &serde_json::to_vec(&with_boolean).unwrap(),
        &request,
        observed_at,
    )
    .unwrap();
    let baseline_batch = parse_provider_top_n(
        &serde_json::to_vec(&without_boolean).unwrap(),
        &request,
        observed_at,
    )
    .unwrap();

    assert_ne!(
        boolean_batch.provenance().batch_id(),
        baseline_batch.provenance().batch_id()
    );
}

#[test]
fn maps_one_provider_ordered_page_without_full_market_or_intraday_claims() {
    let request = request(MarketRankingKind::VolumeRatio, 2);
    let batch = parse_provider_top_n(&fixture(), &request, "2026-07-29T15:35:00+08:00").unwrap();
    assert_eq!(batch.records().len(), 2);
    let first = &batch.records()[0];
    assert_eq!(first.source_order_ordinal().get(), 1);
    assert_eq!(first.instrument().code(), "600396");
    assert_eq!(first.latest_trading_date().as_str(), "2026-07-29");
    assert_eq!(first.provider_declared_total().get(), 5542);
    assert_eq!(first.inspected_row_count().get(), 2);
    assert_eq!(first.evidence().provider(), ProviderId::Eastmoney);
    assert_eq!(first.evidence().source_at(), None);
    assert_eq!(batch.provenance().source_at(), None);
}

#[test]
fn main_net_is_signed_yuan_and_volume_ratio_is_non_negative() {
    let main_net = parse_provider_top_n(
        &fixture(),
        &request(MarketRankingKind::MainNetInflow, 2),
        "2026-07-29T16:00:00+08:00",
    )
    .unwrap();
    assert_eq!(main_net.records()[0].unit(), &MarketRankingUnit::Yuan);

    let negative = String::from_utf8(fixture())
        .unwrap()
        .replace("\"f10\":9.8", "\"f10\":-1");
    assert!(parse_provider_top_n(
        negative.as_bytes(),
        &request(MarketRankingKind::VolumeRatio, 2),
        "2026-07-29T16:00:00+08:00"
    )
    .is_err());
}

#[test]
fn rejects_partial_missing_unsorted_duplicate_and_wrong_date_pages_atomically() {
    let request = request(MarketRankingKind::VolumeRatio, 2);
    let valid: Value = serde_json::from_slice(&fixture()).unwrap();
    let mut cases = vec![
        serde_json::json!({"rc":1}),
        serde_json::json!({"rc":0}),
        serde_json::json!({"rc":0,"data":{"total":0,"diff":[]}}),
        serde_json::json!({"rc":0,"data":{"total":5542,"diff":[]}}),
    ];
    for field in ["f10", "f12", "f13", "f14", "f297"] {
        let mut value = valid.clone();
        value["data"]["diff"][0][field] = Value::Null;
        cases.push(value);
    }
    let mut wrong_date = valid.clone();
    wrong_date["data"]["diff"][0]["f297"] = serde_json::json!(20260728);
    cases.push(wrong_date);
    let mut duplicate = valid.clone();
    duplicate["data"]["diff"][1]["f12"] = duplicate["data"]["diff"][0]["f12"].clone();
    duplicate["data"]["diff"][1]["f13"] = duplicate["data"]["diff"][0]["f13"].clone();
    cases.push(duplicate);
    let mut unsorted = valid;
    unsorted["data"]["diff"][1]["f10"] = serde_json::json!(10.0);
    cases.push(unsorted);

    for value in cases {
        assert!(parse_provider_top_n(
            &serde_json::to_vec(&value).unwrap(),
            &request,
            "2026-07-29T16:00:00+08:00"
        )
        .is_err());
    }
    assert!(parse_provider_top_n(b"{invalid", &request, "2026-07-29T16:00:00+08:00").is_err());
}

#[test]
fn capture_gate_rejects_pre_window_wrong_offset_and_cross_midnight_completion() {
    let request = request(MarketRankingKind::VolumeRatio, 2);
    assert!(validate_capture_observation(&request, "2026-07-29T15:35:00+08:00").is_ok());
    for observed_at in [
        "2026-07-29T15:34:59+08:00",
        "2026-07-29T15:35:00Z",
        "2026-07-30T15:35:00+08:00",
    ] {
        assert!(validate_capture_observation(&request, observed_at).is_err());
    }

    let transport = ScriptedTransport::from_results([Ok(fixture())]);
    let client = EastmoneyClient::with_transport(transport);
    let mut times = ["2026-07-29T15:35:00+08:00", "2026-07-30T00:00:00+08:00"].into_iter();
    assert!(client
        .diagnose_provider_top_n_rankings_with_clock(&request, || {
            Ok(times.next().unwrap().into())
        })
        .is_err());
}

#[test]
fn diagnostic_retries_transport_only_and_uses_one_complete_delay_response() {
    let transport = ScriptedTransport::from_results([
        Err(EastmoneyError::Transport("primary-1".into())),
        Err(EastmoneyError::Transport("primary-2".into())),
        Err(EastmoneyError::Transport("primary-3".into())),
        Ok(fixture()),
    ]);
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);
    let batch = client
        .diagnose_provider_top_n_rankings_with_clock(
            &request(MarketRankingKind::VolumeRatio, 2),
            || Ok("2026-07-29T16:00:00+08:00".into()),
        )
        .unwrap();
    assert_eq!(batch.records().len(), 2);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests[..3]
        .iter()
        .all(|request| request.contains(PRIMARY_ENDPOINT)));
    assert!(requests[3].contains(DELAY_ENDPOINT));
}

#[test]
fn formal_capabilities_are_admitted_without_changing_full_market_capabilities() {
    let request = EastmoneyClient::provider_top_n_a_share_request(
        MarketRankingKind::VolumeRatio,
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    assert_eq!(request.filter_identity().as_str(), A_SHARE_FILTER);
    assert_eq!(
        EastmoneyClient::provider_top_n_source_identity()
            .unwrap()
            .as_str(),
        SOURCE_NAME
    );
    assert!(EastmoneyClient::provider_top_n_ranking_capabilities().volume_ratio);
    assert!(EastmoneyClient::provider_top_n_ranking_capabilities().main_net_inflow);
    assert!(!EastmoneyClient::market_ranking_capabilities().volume_ratio);
    assert!(!EastmoneyClient::market_ranking_capabilities().main_net_inflow);
    assert!(!EastmoneyClient::signal_capabilities().market_rankings);
    let url = provider_top_n_url(PRIMARY_ENDPOINT, &request, "f10").unwrap();
    assert!(url.contains("pn=1"));
    assert!(url.contains("pz=2"));
    assert!(url.contains("f297"));
    assert!(!url.contains("f124"));
}

#[test]
fn provider_top_n_protocol_preflight_rejects_unregistered_contexts() {
    let wrong_filter = ProviderTopNRankingRequest::new(
        MarketRankingKind::VolumeRatio,
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(2).unwrap(),
        NonEmptyText::new("not-the-admitted-a-share-filter").unwrap(),
    )
    .unwrap();
    assert!(matches!(
        validate_request(&wrong_filter),
        Err(EastmoneyError::InvalidRequest(_))
    ));
    assert!(matches!(
        ranking_field(&MarketRankingKind::Popularity),
        Err(EastmoneyError::Unsupported(_))
    ));
    assert!(matches!(
        ranking_unit(&MarketRankingKind::Popularity),
        Err(EastmoneyError::Unsupported(_))
    ));
    assert!(matches!(
        ranking_identity(&MarketRankingKind::Popularity),
        Err(EastmoneyError::Unsupported(_))
    ));

    let request = request(MarketRankingKind::VolumeRatio, 2);
    assert!(matches!(
        provider_top_n_url("https://example.invalid", &request, "f10"),
        Err(EastmoneyError::InvalidRequest(_))
    ));
    assert!(matches!(
        provider_top_n_url(PRIMARY_ENDPOINT, &request, "f62"),
        Err(EastmoneyError::InvalidRequest(_))
    ));
}

#[test]
fn public_diagnostic_rejects_unadmitted_filter_before_clock_or_transport() {
    let wrong_filter = ProviderTopNRankingRequest::new(
        MarketRankingKind::VolumeRatio,
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(2).unwrap(),
        NonEmptyText::new("not-the-admitted-a-share-filter").unwrap(),
    )
    .unwrap();
    let transport = ScriptedTransport::from_results([]);
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);

    assert!(matches!(
        client.diagnose_provider_top_n_rankings(&wrong_filter),
        Err(EastmoneyError::InvalidRequest(_))
    ));
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn provider_top_n_parser_rejects_resource_and_date_shape_boundaries() {
    let request = request(MarketRankingKind::VolumeRatio, 2);
    let valid: Value = serde_json::from_slice(&fixture()).unwrap();

    let mut non_array_rows = valid.clone();
    non_array_rows["data"]["diff"] = serde_json::json!({});
    let mut invalid_compact_date = valid.clone();
    invalid_compact_date["data"]["diff"][0]["f297"] = serde_json::json!("2026-07-29");
    let mut invalid_calendar_date = valid;
    invalid_calendar_date["data"]["diff"][0]["f297"] = serde_json::json!("20260230");

    for value in [non_array_rows, invalid_compact_date, invalid_calendar_date] {
        assert!(parse_provider_top_n(
            &serde_json::to_vec(&value).unwrap(),
            &request,
            "2026-07-29T16:00:00+08:00",
        )
        .is_err());
    }

    for observed_at in [
        "2026-07-29T1:35:00+08:00",
        "2026-07-29T153500+08:00",
        "2026-07-29T15:35:000+08:00",
    ] {
        assert!(validate_capture_observation(&request, observed_at).is_err());
    }
}

#[test]
fn provider_declared_total_is_evidence_not_an_unregistered_rejection_threshold() {
    let request = request(MarketRankingKind::VolumeRatio, 2);
    let mut value: Value = serde_json::from_slice(&fixture()).unwrap();
    value["data"]["total"] = serde_json::json!(20_001);

    let batch = parse_provider_top_n(
        &serde_json::to_vec(&value).unwrap(),
        &request,
        "2026-07-29T16:00:00+08:00",
    )
    .unwrap();

    assert_eq!(batch.records().len(), 2);
    assert!(batch
        .records()
        .iter()
        .all(|record| record.provider_declared_total().get() == 20_001));
}

#[test]
fn diagnostic_preserves_clock_non_transport_and_exhausted_transport_failures() {
    let request = request(MarketRankingKind::VolumeRatio, 2);

    let transport = ScriptedTransport::from_results([]);
    let client = EastmoneyClient::with_transport(transport);
    assert!(matches!(
        client.diagnose_provider_top_n_rankings_with_clock(&request, || {
            Err(EastmoneyError::Transport("clock failed".into()))
        }),
        Err(EastmoneyError::Transport(message)) if message == "clock failed"
    ));

    let transport = ScriptedTransport::from_results([Err(EastmoneyError::Decode(
        "non-transport failure".into(),
    ))]);
    let client = EastmoneyClient::with_transport(transport);
    assert!(matches!(
        client.diagnose_provider_top_n_rankings_with_clock(&request, || {
            Ok("2026-07-29T16:00:00+08:00".into())
        }),
        Err(EastmoneyError::Decode(message)) if message == "non-transport failure"
    ));

    let transport = ScriptedTransport::from_results([Ok(fixture())]);
    let client = EastmoneyClient::with_transport(transport);
    let mut calls = 0;
    assert!(matches!(
        client.diagnose_provider_top_n_rankings_with_clock(&request, || {
            calls += 1;
            if calls == 1 {
                Ok("2026-07-29T16:00:00+08:00".into())
            } else {
                Err(EastmoneyError::Transport("completion clock failed".into()))
            }
        }),
        Err(EastmoneyError::Transport(message)) if message == "completion clock failed"
    ));

    let transport = ScriptedTransport::from_results((0..6).map(|index| {
        Err(EastmoneyError::Transport(format!(
            "transport failure {index}"
        )))
    }));
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);
    assert!(matches!(
        client.diagnose_provider_top_n_rankings_with_clock(&request, || {
            Ok("2026-07-29T16:00:00+08:00".into())
        }),
        Err(EastmoneyError::Transport(message))
            if message.contains("all Eastmoney provider Top-N HTTPS endpoints failed")
    ));
    assert_eq!(requests.lock().unwrap().len(), 6);
}
