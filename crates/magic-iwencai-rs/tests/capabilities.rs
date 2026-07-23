use magic_iwencai_rs::IwencaiClient;

#[test]
fn semantic_search_stays_unadvertised_until_an_authorized_live_probe_succeeds() {
    let capabilities = IwencaiClient::research_capabilities();
    assert!(!capabilities.semantic_search);
    assert!(!capabilities.reports);
    assert!(!capabilities.consensus);
    assert!(!capabilities.pdf_download);
}
