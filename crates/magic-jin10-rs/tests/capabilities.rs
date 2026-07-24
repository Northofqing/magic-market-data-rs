use magic_jin10_rs::Jin10Client;

#[test]
fn advertises_only_verified_global_news() {
    let capabilities = Jin10Client::content_capabilities();
    assert!(capabilities.global_news);
    assert!(!capabilities.instrument_news);
    assert!(!capabilities.announcements);
    assert!(!capabilities.investor_questions);
}
