use magic_thepaper_rs::ThePaperClient;

#[test]
fn advertises_only_verified_global_news() {
    let capabilities = ThePaperClient::content_capabilities();
    assert!(capabilities.global_news);
    assert!(!capabilities.instrument_news);
    assert!(!capabilities.announcements);
    assert!(!capabilities.investor_questions);
}
