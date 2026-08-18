use super::{
    parse_atomic_market_ranking_page, parse_diagnostic_market_ranking_page,
    parse_market_ranking_pages, parse_page, ranking_unit, ranking_url, ranking_url_for,
    session_for_source_at,
};
use crate::test_support::ScriptedTransport;
use magic_market_core::{
    MarketRankingKind, MarketRankingUnit, MarketRankings, MarketSession, PositiveU32, ProviderId,
};

fn page(total: u32, rows: &str) -> Vec<u8> {
    format!(r#"{{"rc":0,"data":{{"total":{total},"diff":[{rows}]}}}}"#).into_bytes()
}

fn row(
    code: &str,
    market: u32,
    name: &str,
    volume_ratio: &str,
    main_net: &str,
    epoch: u32,
) -> String {
    format!(
        r#"{{"f10":{volume_ratio},"f12":"{code}","f13":{market},"f14":"{name}","f62":{main_net},"f124":{epoch}}}"#
    )
}

#[test]
fn partial_diagnostic_keeps_absent_source_fields_null_and_reports_partial_coverage() {
    let rows = [
        row("600001", 1, "A", "3", "30", 1_784_872_800),
        r#"{"f13":0,"f62":20,"f124":0}"#.to_owned(),
    ]
    .join(",");
    let batch = parse_diagnostic_market_ranking_page(
        &page(2, &rows),
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();

    assert!(!batch.quality().is_complete());
    assert_eq!(batch.records().len(), 2);
    let first = serde_json::to_value(&batch.records()[0]).unwrap();
    assert_eq!(first["reported_universe_size"], 2);
    assert_eq!(first["fetched_count"], 2);
    assert_eq!(first["value"], 3.0);
    let missing = serde_json::to_value(&batch.records()[1]).unwrap();
    assert!(missing["instrument"].is_null());
    assert!(missing["label"].is_null());
    assert!(missing["value"].is_null());
    assert!(missing["source_at"].is_null());
    assert!(missing["evidence"]["source_at"].is_null());
}

#[test]
fn partial_diagnostic_fetches_one_bounded_page_and_rejects_unbounded_limits_before_io() {
    let rows = (0..100)
        .map(|index| {
            row(
                &format!("600{index:03}"),
                1,
                "A",
                &(200 - index).to_string(),
                &(2_000 - index * 10).to_string(),
                1_784_872_800,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let transport = ScriptedTransport::from_results([Ok(page(5_000, &rows))]);
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);
    let batch = client
        .diagnose_partial_market_rankings(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(2).unwrap(),
        )
        .unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(requests.lock().unwrap().len(), 1);

    let transport = ScriptedTransport::from_bodies([]);
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);
    assert!(matches!(
        client.diagnose_partial_market_rankings(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(101).unwrap(),
        ),
        Err(crate::EastmoneyError::InvalidRequest(_))
    ));
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn bounded_snapshot_is_one_complete_response_and_requires_every_proved_field() {
    let rows = (0..100)
        .map(|index| {
            row(
                &format!("600{index:03}"),
                1,
                &format!("A{index}"),
                &(200 - index).to_string(),
                &(2_000 - index * 10).to_string(),
                1_784_872_800,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let transport = ScriptedTransport::from_results([Ok(page(5_000, &rows))]);
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);
    let batch = client
        .bounded_market_rankings_snapshot(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(20).unwrap(),
        )
        .unwrap();
    assert!(batch.quality().is_complete());
    assert_eq!(batch.records().len(), 20);
    assert_eq!(requests.lock().unwrap().len(), 1);
    let first = serde_json::to_value(&batch.records()[0]).unwrap();
    assert_eq!(first["reported_universe_size"], 5_000);
    assert_eq!(first["fetched_count"], 100);
    assert_eq!(first["source_rank"], 1);
    assert!(!first["source_at"].is_null());
    assert!(!first["value"].is_null());

    let batch_id = batch.provenance().batch_id().unwrap();
    assert!(batch.provenance().source_at().is_none());
    for (index, record) in batch.records().iter().enumerate() {
        assert_eq!(record.source_rank.get(), u32::try_from(index + 1).unwrap());
        assert_eq!(record.evidence.batch_id(), batch_id);
        assert_eq!(
            record.evidence.observed_at(),
            batch.provenance().fetched_at()
        );
        assert_eq!(record.evidence.source_at(), record.source_at.as_deref());
    }

    for bad_rows in [
        [
            r#"{"f10":3,"f13":1,"f14":"A","f62":30,"f124":1784872800}"#.to_owned(),
            row("600002", 1, "B", "2", "20", 1_784_872_800),
        ]
        .join(","),
        [
            r#"{"f10":3,"f12":"600001","f14":"A","f62":30,"f124":1784872800}"#.to_owned(),
            row("600002", 1, "B", "2", "20", 1_784_872_800),
        ]
        .join(","),
        [
            row("600001", 1, "", "3", "30", 1_784_872_800),
            row("600002", 1, "B", "2", "20", 1_784_872_800),
        ]
        .join(","),
        [
            r#"{"f10":3,"f12":"600001","f13":1,"f62":30,"f124":1784872800}"#.to_owned(),
            row("600002", 1, "B", "2", "20", 1_784_872_800),
        ]
        .join(","),
        [
            row("600001", 1, "A", "3", "30", 1_784_872_800),
            r#"{"f12":"600002","f13":1,"f14":"B","f62":20,"f124":1784872800}"#.to_owned(),
        ]
        .join(","),
        [
            r#"{"f10":3,"f12":"600001","f13":1,"f14":"A","f62":30}"#.to_owned(),
            row("600002", 1, "B", "2", "20", 1_784_872_800),
        ]
        .join(","),
        [
            row("600001", 1, "A", "3", "30", 1_784_872_800),
            row("600001", 1, "duplicate", "2", "20", 1_784_872_800),
        ]
        .join(","),
        [
            row("600001", 1, "A", "2", "20", 1_784_872_800),
            row("600002", 1, "B", "3", "30", 1_784_872_800),
        ]
        .join(","),
    ] {
        assert!(parse_atomic_market_ranking_page(
            &page(2, &bad_rows),
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(2).unwrap(),
        )
        .is_err());
    }
}

#[test]
fn bounded_snapshot_rejects_limit_above_one_page_before_io() {
    let transport = ScriptedTransport::from_bodies([]);
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);

    assert!(matches!(
        client.bounded_market_rankings_snapshot(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(101).unwrap(),
        ),
        Err(crate::EastmoneyError::InvalidRequest(_))
    ));
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn complete_pages_map_typed_rankings_with_code_name_coverage_skew_and_evidence() {
    let pages = vec![
        page(
            3,
            &[
                row("600001", 1, "上证一号", "12.5", "300", 1_784_872_800),
                row("000001", 0, "深证一号", "8.0", "200", 1_784_872_800),
            ]
            .join(","),
        ),
        page(
            3,
            &row("920118", 0, "北证样本", "3.0", "100", 1_784_872_800),
        ),
    ];
    let batch = parse_market_ranking_pages(
        &pages,
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(2).unwrap(),
        2,
    )
    .unwrap();
    assert_eq!(batch.records().len(), 2);
    let first = &batch.records()[0];
    assert_eq!(first.instrument().unwrap().code(), "600001");
    assert_eq!(first.label().as_str(), "上证一号");
    assert_eq!(first.unit(), &MarketRankingUnit::Multiple);
    assert_eq!(first.universe_size().get(), 3);
    assert_eq!(first.covered_count().get(), 3);
    assert_eq!(first.max_source_skew_millis(), 0);
    assert_eq!(first.evidence().provider(), ProviderId::Eastmoney);
    assert_eq!(first.source_date().as_str(), "2026-07-24");
}

#[test]
fn total_page_contradictions_and_incomplete_coverage_fail_atomically() {
    let first = page(
        3,
        &[
            row("600001", 1, "A", "3", "30", 1_784_872_800),
            row("600002", 1, "B", "2", "20", 1_784_872_800),
        ]
        .join(","),
    );
    assert!(parse_market_ranking_pages(
        std::slice::from_ref(&first),
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(2).unwrap(),
        2,
    )
    .is_err());
    let second = page(4, &row("600003", 1, "C", "1", "10", 1_784_872_800));
    assert!(parse_market_ranking_pages(
        &[first, second],
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(2).unwrap(),
        2,
    )
    .is_err());
}

#[test]
fn duplicates_missing_identity_wrong_market_and_nonfinite_metrics_fail() {
    for rows in [
        [
            row("600001", 1, "A", "3", "30", 1_784_872_800),
            row("600001", 1, "A", "2", "20", 1_784_872_800),
        ]
        .join(","),
        [
            row("600001", 0, "A", "3", "30", 1_784_872_800),
            row("600002", 1, "B", "2", "20", 1_784_872_800),
        ]
        .join(","),
        [
            row("600001", 1, "", "3", "30", 1_784_872_800),
            row("600002", 1, "B", "2", "20", 1_784_872_800),
        ]
        .join(","),
        [
            row("600001", 1, "A", "\"NaN\"", "30", 1_784_872_800),
            row("600002", 1, "B", "2", "20", 1_784_872_800),
        ]
        .join(","),
    ] {
        assert!(parse_market_ranking_pages(
            &[page(2, &rows)],
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(2).unwrap(),
            2,
        )
        .is_err());
    }
}

#[test]
fn wrong_order_mixed_timestamps_and_missing_time_fail() {
    let wrong_order = [
        row("600001", 1, "A", "2", "30", 1_784_872_800),
        row("600002", 1, "B", "3", "20", 1_784_872_800),
    ]
    .join(",");
    assert!(parse_market_ranking_pages(
        &[page(2, &wrong_order)],
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(2).unwrap(),
        2,
    )
    .is_err());

    let bounded_but_non_common_time = [
        row("600001", 1, "A", "3", "30", 1_784_872_800),
        row("000001", 0, "B", "2", "20", 1_784_872_801),
        row("920118", 0, "C", "1", "10", 1_784_872_800),
    ]
    .join(",");
    assert!(parse_market_ranking_pages(
        &[page(3, &bounded_but_non_common_time)],
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(3).unwrap(),
        3,
    )
    .is_err());

    let mixed_session = [
        row("600001", 1, "A", "3", "30", 1_784_872_800),
        row("600002", 1, "B", "2", "20", 1_784_878_200),
    ]
    .join(",");
    assert!(parse_market_ranking_pages(
        &[page(2, &mixed_session)],
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(2).unwrap(),
        2,
    )
    .is_err());

    let missing_time = format!(
        r#"{{"f10":3,"f12":"600001","f13":1,"f14":"A","f62":30}},
{}"#,
        row("600002", 1, "B", "2", "20", 1_784_872_800)
    );
    assert!(parse_market_ranking_pages(
        &[page(2, &missing_time)],
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(2).unwrap(),
        2,
    )
    .is_err());
}

#[test]
fn main_net_inflow_uses_yuan_and_all_other_kinds_stay_explicitly_unsupported() {
    let pages = vec![page(
        3,
        &[
            row("600001", 1, "A", "3", "-50", 1_784_872_800),
            row("000001", 0, "B", "2", "-60", 1_784_872_800),
            row("920118", 0, "C", "1", "-70", 1_784_872_800),
        ]
        .join(","),
    )];
    let batch = parse_market_ranking_pages(
        &pages,
        &MarketRankingKind::MainNetInflow,
        PositiveU32::new(1).unwrap(),
        3,
    )
    .unwrap();
    assert_eq!(batch.records()[0].unit(), &MarketRankingUnit::Yuan);
    assert_eq!(batch.records()[0].value().get(), -50.0);
    assert!(parse_market_ranking_pages(
        &pages,
        &MarketRankingKind::Popularity,
        PositiveU32::new(1).unwrap(),
        1,
    )
    .is_err());
}

#[test]
fn ranking_unit_rejects_metrics_without_source_proof() {
    assert!(matches!(
        ranking_unit(&MarketRankingKind::Popularity),
        Err(crate::EastmoneyError::Unsupported(message))
            if message.contains("not source-proven")
    ));
}

#[test]
fn one_ranking_batch_cannot_mix_source_trading_dates() {
    let mixed_dates = [
        row("600001", 1, "A", "3", "30", 1_784_872_800),
        row("000001", 0, "B", "2", "20", 1_784_959_200),
    ]
    .join(",");
    let error = parse_market_ranking_pages(
        &[page(2, &mixed_dates)],
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(2).unwrap(),
        2,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        crate::EastmoneyError::Protocol(message)
            if message.contains("source dates differ across the universe")
    ));
}

#[test]
fn ranking_page_retries_only_transport_errors_with_the_existing_bounded_get() {
    let body = page(
        3,
        &[
            row("600001", 1, "沪市", "3", "30", 1_784_872_800),
            row("000001", 0, "深市", "2", "20", 1_784_872_800),
            row("920118", 0, "北市", "1", "10", 1_784_872_800),
        ]
        .join(","),
    );
    let transport = ScriptedTransport::from_results([
        Err(crate::EastmoneyError::Transport("transient TLS EOF".into())),
        Ok(body),
    ]);
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);
    assert!(client
        .market_rankings(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap()
        )
        .is_ok());
    assert_eq!(requests.lock().unwrap().len(), 2);

    let transport = ScriptedTransport::from_results([
        Err(crate::EastmoneyError::Transport("one".into())),
        Err(crate::EastmoneyError::Transport("two".into())),
        Err(crate::EastmoneyError::Transport("three".into())),
        Ok(page(
            3,
            &[
                row("600001", 1, "沪市", "3", "30", 1_784_872_800),
                row("000001", 0, "深市", "2", "20", 1_784_872_800),
                row("920118", 0, "北市", "1", "10", 1_784_872_800),
            ]
            .join(","),
        )),
    ]);
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);
    assert!(client
        .market_rankings(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap()
        )
        .is_ok());
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests[..3]
        .iter()
        .all(|request| request.contains("https://push2.eastmoney.com/")));
    assert!(requests[3].contains("https://push2delay.eastmoney.com/"));
    drop(requests);

    let transport = ScriptedTransport::from_results((1..=6).map(|attempt| {
        Err(crate::EastmoneyError::Transport(format!(
            "transport failure {attempt}"
        )))
    }));
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);
    assert!(matches!(
        client.market_rankings(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap()
        ),
        Err(crate::EastmoneyError::Transport(message))
            if message.contains("all Eastmoney full-market HTTPS endpoints failed")
                && message.contains("transport failure 6")
    ));
    assert_eq!(requests.lock().unwrap().len(), 6);

    let transport =
        ScriptedTransport::from_bodies([br#"{"rc":1,"data":{"total":3,"diff":[]}}"#.as_slice()]);
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);
    assert!(matches!(
        client.market_rankings(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap()
        ),
        Err(crate::EastmoneyError::Protocol(_))
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[test]
fn endpoint_failover_discards_partial_pages_and_restarts_the_whole_snapshot() {
    let mut first_page_rows = vec![
        row("600001", 1, "沪市", "101", "1010", 1_784_872_800),
        row("000001", 0, "深市", "100", "1000", 1_784_872_800),
        row("920118", 0, "北市", "99", "990", 1_784_872_800),
    ];
    for suffix in 2..=98 {
        first_page_rows.push(row(
            &format!("600{suffix:03}"),
            1,
            "沪市",
            &(100 - suffix).to_string(),
            &(1_000 - suffix * 10).to_string(),
            1_784_872_800,
        ));
    }
    assert_eq!(first_page_rows.len(), 100);
    let first_page = page(101, &first_page_rows.join(","));
    let last_page = page(101, &row("600099", 1, "沪市", "1", "10", 1_784_872_800));
    let transport = ScriptedTransport::from_results([
        Ok(first_page.clone()),
        Err(crate::EastmoneyError::Transport(
            "primary page 2 EOF 1".into(),
        )),
        Err(crate::EastmoneyError::Transport(
            "primary page 2 EOF 2".into(),
        )),
        Err(crate::EastmoneyError::Transport(
            "primary page 2 EOF 3".into(),
        )),
        Ok(first_page),
        Ok(last_page),
    ]);
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);
    let batch = client
        .market_rankings(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap(),
        )
        .unwrap();
    assert_eq!(batch.records()[0].universe_size().get(), 101);

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 6);
    assert!(requests[..4]
        .iter()
        .all(|request| request.contains("https://push2.eastmoney.com/")));
    assert!(requests[4..]
        .iter()
        .all(|request| request.contains("https://push2delay.eastmoney.com/")));
    assert!(requests[0].contains("pn=1"));
    assert!(requests[1..4]
        .iter()
        .all(|request| request.contains("pn=2")));
    assert!(requests[4].contains("pn=1"));
    assert!(requests[5].contains("pn=2"));
}

#[test]
fn market_session_boundaries_do_not_misclassify_the_close() {
    for (clock, expected) in [
        ("09:14:59", MarketSession::PreOpen),
        ("09:15:00", MarketSession::OpeningAuction),
        ("09:30:00", MarketSession::Continuous),
        ("11:30:00", MarketSession::Continuous),
        ("11:30:01", MarketSession::LunchBreak),
        ("13:00:00", MarketSession::Continuous),
        ("14:59:59", MarketSession::Continuous),
        ("15:00:00", MarketSession::Close),
        ("15:00:01", MarketSession::PostClose),
    ] {
        assert_eq!(
            session_for_source_at(&format!("2026-07-27T{clock}+08:00")).unwrap(),
            expected,
            "clock={clock}"
        );
    }
}

#[test]
fn source_page_cap_is_100_and_a_500_row_assumption_is_rejected() {
    let url = ranking_url(&MarketRankingKind::VolumeRatio, "f10", 1, 100).unwrap();
    assert!(url.contains("pz=100"));

    let repeated = (0..100)
        .map(|_| row("600001", 1, "A", "3", "30", 1_784_872_800))
        .collect::<Vec<_>>()
        .join(",");
    assert!(parse_market_ranking_pages(
        &[page(101, &repeated)],
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(1).unwrap(),
        500,
    )
    .is_err());
}

#[test]
fn complete_rows_still_fail_without_all_three_a_share_exchanges() {
    let rows = [
        row("600001", 1, "沪市", "3", "30", 1_784_872_800),
        row("000001", 0, "深市", "2", "20", 1_784_872_800),
    ]
    .join(",");
    assert!(parse_market_ranking_pages(
        &[page(2, &rows)],
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(2).unwrap(),
        2,
    )
    .is_err());
}

#[test]
fn zero_source_timestamp_fails_atomically_instead_of_skipping_halted_rows() {
    let rows = [
        row("600001", 1, "沪市", "3", "30", 1_784_872_800),
        row("000001", 0, "深市", "2", "20", 1_784_872_800),
        row("920118", 0, "北市", "1", "10", 0),
    ]
    .join(",");
    assert!(parse_market_ranking_pages(
        &[page(3, &rows)],
        &MarketRankingKind::VolumeRatio,
        PositiveU32::new(3).unwrap(),
        3,
    )
    .is_err());
}

#[test]
fn ranking_url_and_page_envelope_fail_closed_for_every_unregistered_shape() {
    let url = ranking_url(&MarketRankingKind::MainNetInflow, "f62", 2, 100).unwrap();
    for marker in [
        "pn=2",
        "pz=100",
        "fid=f62",
        "ut=8dec03ba335b81bf4ebdf7b29ec27d15",
        "fields=f1%2Cf10%2Cf12%2Cf13%2Cf14%2Cf62%2Cf124",
        "m%3A0%2Bt%3A81%2Bs%3A262144%2Bf%3A%212",
    ] {
        assert!(url.contains(marker), "missing {marker} in {url}");
    }

    for result in [
        ranking_url_for(
            "https://example.com/api/qt/clist/get",
            &MarketRankingKind::VolumeRatio,
            "f10",
            1,
            100,
        ),
        ranking_url(&MarketRankingKind::VolumeRatio, "f62", 1, 100),
        ranking_url(&MarketRankingKind::VolumeRatio, "f10", 0, 100),
        ranking_url(&MarketRankingKind::VolumeRatio, "f10", 1, 0),
    ] {
        assert!(matches!(
            result,
            Err(crate::EastmoneyError::InvalidRequest(_))
        ));
    }
    assert!(matches!(
        ranking_url(&MarketRankingKind::Popularity, "f10", 1, 100),
        Err(crate::EastmoneyError::Unsupported(_))
    ));

    let valid = parse_page(&page(
        1,
        &row("600001", 1, "沪市", "1", "10", 1_784_872_800),
    ))
    .unwrap();
    assert_eq!(valid.total, 1);
    assert_eq!(valid.rows.len(), 1);
    for invalid in [
        br#"{"#.as_slice(),
        br#"{"data":{"total":1,"diff":[]}}"#.as_slice(),
        br#"{"rc":1,"data":{"total":1,"diff":[]}}"#.as_slice(),
        br#"{"rc":0}"#.as_slice(),
        br#"{"rc":0,"data":[]}"#.as_slice(),
        br#"{"rc":0,"data":{"diff":[]}}"#.as_slice(),
        br#"{"rc":0,"data":{"total":"bad","diff":[]}}"#.as_slice(),
        br#"{"rc":0,"data":{"total":1}}"#.as_slice(),
        br#"{"rc":0,"data":{"total":1,"diff":{}}}"#.as_slice(),
    ] {
        assert!(parse_page(invalid).is_err(), "{invalid:?}");
    }
}

#[test]
fn public_ranking_state_machine_rejects_limits_and_remote_pagination_contradictions() {
    let transport = ScriptedTransport::from_bodies([]);
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);
    assert!(matches!(
        client.market_rankings(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(201).unwrap()
        ),
        Err(crate::EastmoneyError::InvalidRequest(message))
            if message.contains("at most 200")
    ));
    assert!(requests.lock().unwrap().is_empty());

    for body in [
        page(0, ""),
        page(20_001, ""),
        page(
            101,
            &row("600001", 1, "short page", "1", "10", 1_784_872_800),
        ),
    ] {
        let transport = ScriptedTransport::from_results([Ok(body)]);
        let requests = transport.requests();
        let client = crate::EastmoneyClient::with_transport(transport);
        assert!(matches!(
            client.market_rankings(
                &MarketRankingKind::VolumeRatio,
                PositiveU32::new(1).unwrap()
            ),
            Err(crate::EastmoneyError::Protocol(_))
        ));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }

    let first_rows = (0..100)
        .map(|index| {
            row(
                &format!("600{index:03}"),
                1,
                "沪市",
                &(200 - index).to_string(),
                &(2_000 - index * 10).to_string(),
                1_784_872_800,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let transport = ScriptedTransport::from_results([
        Ok(page(101, &first_rows)),
        Ok(page(
            102,
            &row("920118", 0, "北市", "1", "10", 1_784_872_800),
        )),
    ]);
    let requests = transport.requests();
    let client = crate::EastmoneyClient::with_transport(transport);
    assert!(matches!(
        client.market_rankings(
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap()
        ),
        Err(crate::EastmoneyError::Protocol(message))
            if message.contains("total changed across pages")
    ));
    assert_eq!(requests.lock().unwrap().len(), 2);
}

#[test]
fn ranking_parser_rejects_unbounded_pages_negative_ratios_and_missing_metric_fields() {
    assert!(matches!(
        parse_market_ranking_pages(
            &[],
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap(),
            1,
        ),
        Err(crate::EastmoneyError::InvalidRequest(_))
    ));
    assert!(matches!(
        parse_market_ranking_pages(
            &[page(1, &row("600001", 1, "沪市", "1", "10", 1_784_872_800))],
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap(),
            0,
        ),
        Err(crate::EastmoneyError::InvalidRequest(_))
    ));

    let three_rows = [
        row("600001", 1, "沪市", "3", "30", 1_784_872_800),
        row("000001", 0, "深市", "2", "20", 1_784_872_800),
        row("920118", 0, "北市", "1", "10", 1_784_872_800),
    ]
    .join(",");
    assert!(matches!(
        parse_market_ranking_pages(
            &[page(3, &three_rows)],
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap(),
            2,
        ),
        Err(crate::EastmoneyError::Protocol(message))
            if message.contains("exceeds declared page size")
    ));

    let negative_ratio = [
        row("600001", 1, "沪市", "-1", "30", 1_784_872_800),
        row("000001", 0, "深市", "-2", "20", 1_784_872_800),
        row("920118", 0, "北市", "-3", "10", 1_784_872_800),
    ]
    .join(",");
    assert!(matches!(
        parse_market_ranking_pages(
            &[page(3, &negative_ratio)],
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap(),
            3,
        ),
        Err(crate::EastmoneyError::Protocol(message))
            if message.contains("must be non-negative")
    ));

    for missing in [
        r#"{"f12":"600001","f13":1,"f14":"沪市","f62":30,"f124":1784872800}"#,
        r#"{"f10":3,"f12":"600001","f13":1,"f14":"沪市","f124":1784872800}"#,
        r#"{"f10":3,"f12":"600001","f14":"沪市","f62":30,"f124":1784872800}"#,
    ] {
        assert!(parse_market_ranking_pages(
            &[page(1, missing)],
            &MarketRankingKind::VolumeRatio,
            PositiveU32::new(1).unwrap(),
            1,
        )
        .is_err());
    }
    assert!(session_for_source_at("2026-07-27").is_err());
}
