use magic_market_core::{
    verify_admitted_newest_first_batch, DataBatch, NonEmptyText, ProbeAdmissionError,
    ProbeAdmissionPolicy, ProbeStatus, Provenance, ProviderId, SourceEvidence,
};
use magic_stcn_rs::parse_quick_news;
use std::time::Duration;

const FIXTURE: &[u8] = include_bytes!("fixtures/quick-news.json");

#[test]
fn normalizes_only_quick_news_metadata() {
    let batch = parse_quick_news(FIXTURE, 1).expect("synthetic fixture should parse");
    assert_eq!(batch.records().len(), 1);
    let item = &batch.records()[0];
    assert_eq!(item.item_id.as_str(), "4754321");
    assert_eq!(item.publisher.as_str(), "人民财讯");
    assert_eq!(
        item.canonical_url.as_str(),
        "https://www.stcn.com/article/detail/4754321.html"
    );
    assert!(item.summary.is_none());
    assert!(item.content.is_none());
    let serialized = serde_json::to_string(batch.records()).expect("serialize");
    assert!(!serialized.contains("这段合成正文必须被忽略"));
    assert!(!serialized.contains("这段分享摘要也必须被忽略"));
}

#[test]
fn validates_rows_beyond_requested_limit() {
    let fixture = String::from_utf8(FIXTURE.to_vec()).expect("UTF-8");
    let invalid = fixture.replacen("\"source\": \"人民财讯\"", "\"source\": \" 人民财讯\"", 1);
    assert!(parse_quick_news(invalid.as_bytes(), 1).is_err());
}

#[test]
fn preserves_audited_syndicated_sources_and_labels_blank_provider_rows() {
    let fixture = String::from_utf8(FIXTURE.to_vec()).expect("UTF-8");
    let syndicated = fixture.replacen("\"source\": \"人民财讯\"", "\"source\": \"新华社\"", 1);
    assert_eq!(
        parse_quick_news(syndicated.as_bytes(), 1)
            .expect("syndicated metadata")
            .records()[0]
            .publisher
            .as_str(),
        "新华社"
    );
    let blank = fixture.replacen("\"source\": \"人民财讯\"", "\"source\": \"\"", 1);
    assert_eq!(
        parse_quick_news(blank.as_bytes(), 1)
            .expect("provider-authored metadata")
            .records()[0]
            .publisher
            .as_str(),
        "证券时报"
    );
}

#[test]
fn rejects_cursor_timestamp_and_url_mismatches() {
    let fixture = String::from_utf8(FIXTURE.to_vec()).expect("UTF-8");
    let cursor = fixture.replace("\"last_time\": 1785291845", "\"last_time\": 1");
    assert!(parse_quick_news(cursor.as_bytes(), 2).is_err());
    let timestamp = fixture.replace("\"show_time\": \"1785291905\"", "\"show_time\": \"1\"");
    assert!(parse_quick_news(timestamp.as_bytes(), 2).is_err());
    let url = fixture.replace("4754321.html", "4754999.html");
    assert!(parse_quick_news(url.as_bytes(), 2).is_err());
}

#[test]
fn rejects_terminal_empty_without_verified_empty_source_evidence() {
    let terminal = br#"{"state":1,"data":"","page_time":null,"last_time":null}"#;
    assert!(parse_quick_news(terminal, 1).is_err());
    let invalid = br#"{"state":1,"data":"","page_time":2,"last_time":null}"#;
    assert!(parse_quick_news(invalid, 1).is_err());
    let missing_page_time = br#"{"state":1,"data":"","last_time":null}"#;
    assert!(parse_quick_news(missing_page_time, 1).is_err());
    let missing_last_time = br#"{"state":1,"data":"","page_time":null}"#;
    assert!(parse_quick_news(missing_last_time, 1).is_err());
}

#[test]
fn uses_the_newest_raw_time_for_batch_and_each_records_own_evidence() {
    let batch = parse_quick_news(FIXTURE, 2).expect("synthetic fixture should parse");
    assert_eq!(batch.provenance().source_at(), Some("1785291905"));
    assert_eq!(batch.records()[0].evidence.source_at(), Some("1785291905"));
    assert_eq!(batch.records()[1].evidence.source_at(), Some("1785291845"));
    assert_eq!(
        verify_admitted_newest_first_batch(
            &batch,
            &ProbeAdmissionPolicy::new(ProviderId::SecuritiesTimes).require_source_at(),
            |item| &item.evidence,
            |item| item.published_at.as_str(),
            |item| item.item_id.as_str().to_owned(),
        )
        .expect("strict metadata batch should satisfy shared admission"),
        ProbeStatus::Admitted
    );
}

#[test]
fn rejects_the_thirty_first_row_during_deserialization() {
    let rows = (0..31)
        .map(|index| {
            let id = 4_800_000 - index;
            let show_time = 1_785_291_905 - i64::from(index);
            format!(
                r#"{{"id":"{id}","url":"/article/detail/{id}.html","web_url":"/article/detail/{id}.html","title":"synthetic {id}","source":"人民财讯","time":{},"show_time":"{show_time}","pageTime":"{id}","content":"discard me","share":{{"description":"discard me"}}}}"#,
                show_time * 1000
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let body = format!(
        r#"{{"state":1,"data":[{rows}],"page_time":2,"last_time":{}}}"#,
        1_785_291_905_i64 - 30
    );
    assert!(parse_quick_news(body.as_bytes(), 30).is_err());
}

#[test]
fn rejects_wrong_envelope_and_non_terminal_cursor_shapes() {
    assert!(parse_quick_news(br#"{"state":0,"data":[],"page_time":2,"last_time":1}"#, 1).is_err());
    assert!(parse_quick_news(
        br#"{"state":1,"data":"not-terminal","page_time":null,"last_time":null}"#,
        1
    )
    .is_err());
    assert!(parse_quick_news(br#"{"state":1,"data":{},"page_time":2,"last_time":1}"#, 1).is_err());
    assert!(parse_quick_news(br#"{"state":1,"data":[],"page_time":2,"last_time":1}"#, 1).is_err());

    let fixture = String::from_utf8(FIXTURE.to_vec()).expect("UTF-8");
    let missing_page_time = fixture.replace(",\n  \"page_time\": 2", "");
    assert!(parse_quick_news(missing_page_time.as_bytes(), 1).is_err());
    let null_page_time = fixture.replace("\"page_time\": 2", "\"page_time\": null");
    assert!(parse_quick_news(null_page_time.as_bytes(), 1).is_err());
    let missing_last_time = fixture.replace(",\n  \"last_time\": 1785291845", "");
    assert!(parse_quick_news(missing_last_time.as_bytes(), 1).is_err());
}

#[test]
fn rejects_invalid_row_identity_order_and_metadata() {
    let fixture = String::from_utf8(FIXTURE.to_vec()).expect("UTF-8");

    let duplicate = fixture.replace("4754320", "4754321");
    assert!(parse_quick_news(duplicate.as_bytes(), 2).is_err());

    let non_numeric_id = fixture.replace("4754321", "not-digits");
    assert!(parse_quick_news(non_numeric_id.as_bytes(), 2).is_err());

    let empty_title = fixture.replacen(
        "\"title\": \"合成的人民财讯快讯标题 1\"",
        "\"title\": \"\"",
        1,
    );
    assert!(parse_quick_news(empty_title.as_bytes(), 2).is_err());

    let page_mismatch =
        fixture.replacen("\"pageTime\": \"4754321\"", "\"pageTime\": \"4754999\"", 1);
    assert!(parse_quick_news(page_mismatch.as_bytes(), 2).is_err());

    let non_monotonic = fixture
        .replacen("1785291845000", "1785291965000", 1)
        .replacen("1785291845", "1785291965", 1);
    assert!(parse_quick_news(non_monotonic.as_bytes(), 2).is_err());

    let absolute_web_url = fixture.replacen(
        "\"web_url\": \"/article/detail/4754321.html\"",
        "\"web_url\": \"https://www.stcn.com/article/detail/4754321.html\"",
        1,
    );
    assert!(parse_quick_news(absolute_web_url.as_bytes(), 2).is_err());
}

#[test]
fn accepts_only_the_audited_string_row_fields_and_second_cursor() {
    assert!(parse_quick_news(FIXTURE, 2).is_ok());
    let fixture = String::from_utf8(FIXTURE.to_vec()).expect("UTF-8");

    let numeric_show_time = fixture.replacen(
        "\"show_time\": \"1785291905\"",
        "\"show_time\": 1785291905",
        1,
    );
    assert!(parse_quick_news(numeric_show_time.as_bytes(), 2).is_err());

    let numeric_page_time =
        fixture.replacen("\"pageTime\": \"4754321\"", "\"pageTime\": 4754321", 1);
    assert!(parse_quick_news(numeric_page_time.as_bytes(), 2).is_err());

    let millisecond_last_time =
        fixture.replace("\"last_time\": 1785291845", "\"last_time\": 1785291845000");
    assert!(parse_quick_news(millisecond_last_time.as_bytes(), 2).is_err());

    let wrong_initial_page = fixture.replace("\"page_time\": 2", "\"page_time\": 3");
    assert!(parse_quick_news(wrong_initial_page.as_bytes(), 2).is_err());
}

#[test]
fn enforces_json_and_return_resource_ceilings() {
    assert!(parse_quick_news(FIXTURE, 0).is_err());
    assert!(parse_quick_news(FIXTURE, 31).is_err());
    assert!(parse_quick_news(&vec![b' '; 2 * 1024 * 1024 + 1], 1).is_err());
}

#[test]
fn strict_probe_policy_rejects_stale_securities_times_news() {
    let mut item = parse_quick_news(FIXTURE, 1).expect("fixture").records()[0].clone();
    let observed_at = "2026-07-29T12:00:00+08:00";
    let source_at = "2026-07-20T12:00:00+08:00";
    let batch_id = "securities-times-stale";
    item.published_at = NonEmptyText::new(source_at).expect("published time");
    item.evidence = SourceEvidence::new(ProviderId::SecuritiesTimes, observed_at, batch_id)
        .expect("evidence")
        .with_source_at(source_at)
        .expect("source time");
    let batch = DataBatch::strict(
        vec![item],
        Provenance::new("securities-times", observed_at)
            .expect("provenance")
            .with_source_at(source_at)
            .expect("source time")
            .with_batch_id(batch_id)
            .expect("batch ID"),
    );
    assert!(matches!(
        verify_admitted_newest_first_batch(
            &batch,
            &ProbeAdmissionPolicy::new(ProviderId::SecuritiesTimes)
                .with_max_source_age(Duration::from_secs(72 * 60 * 60))
                .expect("policy"),
            |item| &item.evidence,
            |item| item.published_at.as_str(),
            |item| item.item_id.as_str().to_owned(),
        ),
        Err(ProbeAdmissionError::StaleSourceTime { .. })
    ));
}
