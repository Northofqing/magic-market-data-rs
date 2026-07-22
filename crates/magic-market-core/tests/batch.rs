use magic_market_core::{DataBatch, Provenance};
#[test]
fn strict_batch_preserves_metadata() {
    let p = Provenance::new("fixture", "now").with_source_at("source");
    let b = DataBatch::strict(vec![1, 2], p.clone());
    assert_eq!(b.records(), &[1, 2]);
    assert_eq!(b.provenance(), &p);
    assert!(b.quality().complete);
}

#[test]
fn best_effort_preserves_quality_issues() {
    let batch = DataBatch::best_effort(
        vec![1],
        Provenance::new("fixture", "now"),
        vec!["missing page".into()],
    );
    assert!(!batch.quality().complete);
    assert_eq!(batch.quality().issues, vec!["missing page"]);
}
