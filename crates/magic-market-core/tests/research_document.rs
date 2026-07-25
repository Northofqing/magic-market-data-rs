use magic_market_core::{HttpsUrl, NonEmptyText, ProviderId, ResearchDocument, SourceEvidence};

fn document(body: &[u8]) -> Result<ResearchDocument, magic_market_core::CoreError> {
    ResearchDocument::new(
        NonEmptyText::new("AP202607231827290069").unwrap(),
        HttpsUrl::new("https://pdf.dfcfw.com/pdf/H3_AP202607231827290069_1.pdf").unwrap(),
        body.to_vec(),
        SourceEvidence::new(ProviderId::Eastmoney, "observed", "batch").unwrap(),
    )
}

#[test]
fn research_document_accepts_terminal_eof_with_pdf_trailing_whitespace() {
    let body = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\nstartxref\n9\n%%EOF\0\t\n\x0c\r ";
    assert!(document(body).is_ok());
}

#[test]
fn research_document_rejects_a_truncated_body_without_eof() {
    let body = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\nstartxref\n9\n";
    assert!(document(body).is_err());
}

#[test]
fn research_document_rejects_non_whitespace_after_eof() {
    let body = b"%PDF-1.7\nstartxref\n9\n%%EOF\ntruncated";
    assert!(document(body).is_err());
}

#[test]
fn research_document_rejects_terminal_eof_without_startxref() {
    let body = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\n%%EOF\n";
    assert!(document(body).is_err());
}
