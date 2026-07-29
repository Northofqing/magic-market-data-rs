use magic_market_core::{
    verify_admitted_batch, DataBatch, ProbeAdmissionError, ProbeAdmissionPolicy, ProbeStatus,
    Provenance, ProviderId, SourceEvidence,
};
use magic_xinhua_rs::parse_listing;
use std::time::Duration;

const FIXTURE: &str = include_str!("fixtures/news.html");

#[test]
fn normalizes_only_listing_metadata_after_full_validation() {
    let batch = parse_listing(FIXTURE, 1).expect("synthetic fixture should parse");
    assert_eq!(batch.records().len(), 1);
    let item = &batch.records()[0];
    assert_eq!(item.item_id.as_str(), "4277771");
    assert_eq!(item.title.as_str(), "合成的公开财经标题 1");
    assert_eq!(item.publisher.as_str(), "新华财经");
    assert_eq!(item.published_at.as_str(), "2026-07-29T10:31:05+08:00");
    assert_eq!(
        item.canonical_url.as_str(),
        "https://www.cnfin.com/yw-lb/detail/20260729/4277771_1.html"
    );
    assert!(item.summary.is_none());
    assert!(item.content.is_none());
    assert!(item.instruments.is_empty());
    assert_eq!(item.language.as_str(), "zh-CN");
    let serialized = serde_json::to_string(batch.records()).expect("serialize");
    assert!(!serialized.contains("这段合成摘要必须被忽略"));
}

#[test]
fn validates_rows_beyond_requested_limit() {
    let invalid = FIXTURE.replace("2026-07-29 10:29:05", "2026-07-29 99:29:05");
    assert!(parse_listing(&invalid, 1).is_err());
}

#[test]
fn rejects_duplicate_and_unsafe_canonical_links() {
    let duplicate = FIXTURE.replace("4277770_1.html", "4277771_1.html");
    assert!(parse_listing(&duplicate, 3).is_err());
    let query = FIXTURE.replace("4277771_1.html", "4277771_1.html?copy=1");
    assert!(parse_listing(&query, 3).is_err());
}

#[test]
fn uses_the_oldest_returned_time_for_admitted_batch_evidence() {
    let batch = parse_listing(FIXTURE, 3).expect("synthetic fixture should parse");
    assert_eq!(
        batch.provenance().source_at(),
        Some("2026-07-29T10:29:05+08:00")
    );
    assert!(batch
        .records()
        .iter()
        .all(|item| { item.evidence.source_at() == batch.provenance().source_at() }));
    assert_eq!(
        verify_admitted_batch(
            &batch,
            &ProbeAdmissionPolicy::new(ProviderId::XinhuaFinance).require_source_at(),
            |item| &item.evidence,
            |item| item.item_id.as_str().to_owned(),
        )
        .expect("strict metadata batch should satisfy shared admission"),
        ProbeStatus::Admitted
    );
}

#[test]
fn rejects_attribute_name_aliases() {
    let data_href = FIXTURE.replacen("<h3><a href=", "<h3><a data-href=", 1);
    assert!(parse_listing(&data_href, 1).is_err());

    let data_title = FIXTURE.replacen(
        "target=\"_blank\">合成的公开财经标题 1",
        "target=\"_blank\" data-title=\"alias\">合成的公开财经标题 1",
        1,
    );
    assert!(parse_listing(&data_title, 1).is_err());
}

#[test]
fn rejects_malformed_or_ambiguous_source_metadata() {
    let alternate_host = FIXTURE.replace(
        "//www.cnfin.com/yw-lb/detail/20260729/4277771_1.html",
        "//example.invalid/yw-lb/detail/20260729/4277771_1.html",
    );
    assert!(parse_listing(&alternate_host, 1).is_err());

    let fragment = FIXTURE.replace("4277771_1.html", "4277771_1.html#copy");
    assert!(parse_listing(&fragment, 1).is_err());

    let wrong_date = FIXTURE.replace("detail/20260729/4277771", "detail/20260728/4277771");
    assert!(parse_listing(&wrong_date, 1).is_err());

    let empty_title = FIXTURE.replacen(">合成的公开财经标题 1</a></h3>", "></a></h3>", 1);
    assert!(parse_listing(&empty_title, 1).is_err());

    let missing_time = FIXTURE.replacen("ui-publish", "other-time", 1);
    assert!(parse_listing(&missing_time, 1).is_err());

    let wrong_source = FIXTURE.replacen("资讯<span> | </span>要闻", "资讯<span> | </span>宏观", 1);
    assert!(parse_listing(&wrong_source, 1).is_err());

    let wrong_source_class = FIXTURE.replacen("ui-sourceinfo", "ui-source", 1);
    assert!(parse_listing(&wrong_source_class, 1).is_err());

    let non_monotonic = FIXTURE.replacen("2026-07-29 10:30:05", "2026-07-29 10:32:05", 1);
    assert!(parse_listing(&non_monotonic, 3).is_err());
}

#[test]
fn enforces_source_and_return_resource_ceilings() {
    assert!(parse_listing(FIXTURE, 0).is_err());
    assert!(parse_listing(FIXTURE, 14).is_err());
    let extra_rows = "<div class=\"ui-zxlist-item\">synthetic</div>".repeat(11);
    let too_many = FIXTURE.replace(
        "<div id=\"listPage\"",
        &format!("{extra_rows}<div id=\"listPage\""),
    );
    assert!(parse_listing(&too_many, 1).is_err());
    assert!(parse_listing(&"x".repeat(1024 * 1024 + 1), 1).is_err());
}

#[test]
fn strict_probe_policy_rejects_stale_xinhua_news() {
    let mut item = parse_listing(FIXTURE, 1).expect("fixture").records()[0].clone();
    let observed_at = "2026-07-29T12:00:00+08:00";
    let source_at = "2026-07-20T12:00:00+08:00";
    let batch_id = "xinhua-stale";
    item.evidence = SourceEvidence::new(ProviderId::XinhuaFinance, observed_at, batch_id)
        .expect("evidence")
        .with_source_at(source_at)
        .expect("source time");
    let batch = DataBatch::strict(
        vec![item],
        Provenance::new("xinhua-finance", observed_at)
            .expect("provenance")
            .with_source_at(source_at)
            .expect("source time")
            .with_batch_id(batch_id)
            .expect("batch ID"),
    );
    assert!(matches!(
        verify_admitted_batch(
            &batch,
            &ProbeAdmissionPolicy::new(ProviderId::XinhuaFinance)
                .with_max_source_age(Duration::from_secs(72 * 60 * 60))
                .expect("policy"),
            |item| &item.evidence,
            |item| item.item_id.as_str().to_owned(),
        ),
        Err(ProbeAdmissionError::StaleSourceTime { .. })
    ));
}
