use magic_market_core::{
    verify_admitted_newest_first_batch, DataBatch, NonEmptyText, ProbeAdmissionError,
    ProbeAdmissionPolicy, ProbeStatus, Provenance, ProviderId, SourceEvidence,
};
use magic_yicai_rs::parse_listing;
use std::time::Duration;

const FIXTURE: &str = include_str!("fixtures/news-info.html");

#[test]
fn normalizes_only_embedded_listing_metadata() {
    let batch = parse_listing(FIXTURE, 2).expect("synthetic fixture should parse");
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].item_id.as_str(), "102765432");
    assert_eq!(batch.records()[0].publisher.as_str(), "第一财经");
    assert_eq!(batch.records()[1].publisher.as_str(), "新华社");
    assert_eq!(
        batch.records()[0].canonical_url.as_str(),
        "https://www.yicai.com/news/102765432.html"
    );
    assert!(batch.records()[0].summary.is_none());
    assert!(batch.records()[0].content.is_none());
    let serialized = serde_json::to_string(batch.records()).expect("serialize");
    assert!(!serialized.contains("这段合成正文必须被忽略"));
    assert!(!serialized.contains("not-retained"));
}

#[test]
fn trims_outer_display_whitespace_before_storage() {
    let padded = FIXTURE
        .replacen(
            "\"NewsTitle\":\"合成的第一财经标题 1\"",
            "\"NewsTitle\":\"  合成的第一财经标题 1　\"",
            1,
        )
        .replacen(
            "\"NewsSource\":\"第一财经\"",
            "\"NewsSource\":\"　第一财经  \"",
            1,
        );

    let batch = parse_listing(&padded, 1).expect("outer display whitespace should be normalized");
    assert_eq!(batch.records()[0].title.as_str(), "合成的第一财经标题 1");
    assert_eq!(batch.records()[0].publisher.as_str(), "第一财经");
}

#[test]
fn rejects_whitespace_only_or_control_bearing_display_text() {
    let whitespace_title = FIXTURE.replacen(
        "\"NewsTitle\":\"合成的第一财经标题 1\"",
        "\"NewsTitle\":\" 　 \"",
        1,
    );
    assert!(parse_listing(&whitespace_title, 1).is_err());

    let whitespace_source =
        FIXTURE.replacen("\"NewsSource\":\"第一财经\"", "\"NewsSource\":\" 　 \"", 1);
    assert!(parse_listing(&whitespace_source, 1).is_err());

    let outer_control = FIXTURE.replacen(
        "\"NewsTitle\":\"合成的第一财经标题 1\"",
        "\"NewsTitle\":\"\\t合成的第一财经标题 1\"",
        1,
    );
    assert!(parse_listing(&outer_control, 1).is_err());

    let embedded_control = FIXTURE.replacen(
        "\"NewsSource\":\"第一财经\"",
        "\"NewsSource\":\"第一\\u0000财经\"",
        1,
    );
    assert!(parse_listing(&embedded_control, 1).is_err());
}

#[test]
fn validates_rows_beyond_requested_limit() {
    let invalid = FIXTURE.replace("2026-07-29T10:23:00", "not-a-time");
    assert!(parse_listing(&invalid, 1).is_err());
}

#[test]
fn accepts_only_the_audited_t_separated_create_date() {
    assert!(parse_listing(FIXTURE, 1).is_ok());
    let unsupported_space_form = FIXTURE.replace("2026-07-29T10:25:00", "2026-07-29 10:25:00");
    assert!(parse_listing(&unsupported_space_form, 1).is_err());
}

#[test]
fn rejects_ambiguous_assignment_and_id_mismatch() {
    let duplicate_assignment = format!("{FIXTURE}<script>var firstlist = [];</script>");
    assert!(parse_listing(&duplicate_assignment, 1).is_err());
    let mismatch = FIXTURE.replace("/news/102765432.html", "/news/102765999.html");
    assert!(parse_listing(&mismatch, 1).is_err());
}

#[test]
fn uses_the_newest_raw_time_for_batch_and_each_records_own_evidence() {
    let batch = parse_listing(FIXTURE, 3).expect("synthetic fixture should parse");
    assert_eq!(batch.provenance().source_at(), Some("2026-07-29T10:25:00"));
    assert_eq!(
        batch.records()[0].evidence.source_at(),
        Some("2026-07-29T10:25:00")
    );
    assert_eq!(
        batch.records()[2].evidence.source_at(),
        Some("2026-07-29T10:23:00")
    );
    assert_eq!(
        verify_admitted_newest_first_batch(
            &batch,
            &ProbeAdmissionPolicy::new(ProviderId::Yicai).require_source_at(),
            |item| &item.evidence,
            |item| item.published_at.as_str(),
            |item| item.item_id.as_str().to_owned(),
        )
        .expect("strict metadata batch should satisfy shared admission"),
        ProbeStatus::Admitted
    );
}

#[test]
fn recognizes_only_executable_script_assignments() {
    let comment_only = "<script>// var firstlist = [];</script>";
    assert!(parse_listing(comment_only, 1).is_err());

    let string_only = r#"<script>const decoy = "var firstlist = []";</script>"#;
    assert!(parse_listing(string_only, 1).is_err());

    let with_decoys = format!(
        r#"<script>// var firstlist = [];
const decoy = "var firstlist = []";
</script>{FIXTURE}"#
    );
    assert!(parse_listing(&with_decoys, 1).is_ok());

    let outside_script = FIXTURE.replace("<script>", "").replace("</script>", "");
    assert!(parse_listing(&outside_script, 1).is_err());

    let trivia_assignment = FIXTURE.replace("var firstlist =", "var/* safe */firstlist/* safe */=");
    assert!(parse_listing(&trivia_assignment, 1).is_ok());

    let regex_only = r#"<script>const decoy = /; var firstlist = \[\];/;</script>"#;
    assert!(parse_listing(regex_only, 1).is_err());
    let with_regex_decoy = format!("{regex_only}{FIXTURE}");
    assert!(parse_listing(&with_regex_decoy, 1).is_ok());

    let json_script_only = r#"<script type="application/json">var firstlist = [];</script>"#;
    assert!(parse_listing(json_script_only, 1).is_err());
    let with_json_script_decoy = format!("{json_script_only}{FIXTURE}");
    assert!(parse_listing(&with_json_script_decoy, 1).is_ok());
}

#[test]
fn rejects_invalid_rows_order_and_source_bounds() {
    let duplicate = FIXTURE.replace("102765431", "102765432");
    assert!(parse_listing(&duplicate, 3).is_err());

    let query = FIXTURE.replacen("/news/102765432.html", "/news/102765432.html?copy=1", 1);
    assert!(parse_listing(&query, 1).is_err());

    let empty_source = FIXTURE.replacen("\"NewsSource\":\"第一财经\"", "\"NewsSource\":\"\"", 1);
    assert!(parse_listing(&empty_source, 1).is_err());

    let non_monotonic = FIXTURE.replacen("2026-07-29T10:24:00", "2026-07-29T10:26:00", 1);
    assert!(parse_listing(&non_monotonic, 3).is_err());

    assert!(parse_listing(FIXTURE, 0).is_err());
    assert!(parse_listing(FIXTURE, 51).is_err());
    assert!(parse_listing(&"x".repeat(2 * 1024 * 1024 + 1), 1).is_err());
}

#[test]
fn caps_the_json_substring_and_source_objects_before_normalization() {
    let oversized_ignored_field = format!(
        r#"<script>var firstlist = [{{"NewsID":1,"NewsTitle":"synthetic","CreateDate":"2026-07-29T10:25:00","NewsSource":"第一财经","url":"/news/1.html","NewsNotes":"{}"}}];</script>"#,
        "x".repeat(512 * 1024)
    );
    assert!(parse_listing(&oversized_ignored_field, 1).is_err());

    let row = r#"{"NewsID":1,"NewsTitle":"synthetic","CreateDate":"2026-07-29T10:25:00","NewsSource":"第一财经","url":"/news/1.html"}"#;
    let too_many = format!(
        "<script>var firstlist = [{}];</script>",
        std::iter::repeat_n(row, 301).collect::<Vec<_>>().join(",")
    );
    assert!(parse_listing(&too_many, 1).is_err());
}

#[test]
fn strict_probe_policy_rejects_stale_yicai_news() {
    let mut item = parse_listing(FIXTURE, 1).expect("fixture").records()[0].clone();
    let observed_at = "2026-07-29T12:00:00+08:00";
    let source_at = "2026-07-20T12:00:00+08:00";
    let batch_id = "yicai-stale";
    item.published_at = NonEmptyText::new(source_at).expect("published time");
    item.evidence = SourceEvidence::new(ProviderId::Yicai, observed_at, batch_id)
        .expect("evidence")
        .with_source_at(source_at)
        .expect("source time");
    let batch = DataBatch::strict(
        vec![item],
        Provenance::new("yicai", observed_at)
            .expect("provenance")
            .with_source_at(source_at)
            .expect("source time")
            .with_batch_id(batch_id)
            .expect("batch ID"),
    );
    assert!(matches!(
        verify_admitted_newest_first_batch(
            &batch,
            &ProbeAdmissionPolicy::new(ProviderId::Yicai)
                .with_max_source_age(Duration::from_secs(72 * 60 * 60))
                .expect("policy"),
            |item| &item.evidence,
            |item| item.published_at.as_str(),
            |item| item.item_id.as_str().to_owned(),
        ),
        Err(ProbeAdmissionError::StaleSourceTime { .. })
    ));
}
