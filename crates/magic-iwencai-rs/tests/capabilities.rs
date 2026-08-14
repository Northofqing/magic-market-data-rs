use magic_iwencai_rs::IwencaiClient;

#[test]
fn semantic_search_is_admitted_after_authorized_live_and_serial_load_probes() {
    let capabilities = IwencaiClient::research_capabilities();
    assert!(capabilities.semantic_search);
    assert!(!capabilities.reports);
    assert!(!capabilities.consensus);
    assert!(!capabilities.pdf_download);
}
