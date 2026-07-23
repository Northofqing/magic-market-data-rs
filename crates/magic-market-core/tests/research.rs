use magic_market_core::{
    AssetClass, ConsensusSnapshot, EarningsEstimate, Exchange, FiniteNumber, HttpsUrl,
    InstrumentId, NonEmptyText, PositiveU32, ProviderId, ReportScope, ResearchReport,
    SemanticChannel, SemanticSearchDocument, SemanticSearchRequest, SourceEvidence, SourcedRecord,
};

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn evidence(provider: ProviderId) -> SourceEvidence {
    SourceEvidence::new(provider, "observed", "batch").unwrap()
}

#[test]
fn research_and_consensus_records_are_typed_and_sourced() {
    let estimate = EarningsEstimate {
        fiscal_year: PositiveU32::new(2026).unwrap(),
        eps: Some(FiniteNumber::new(0.42).unwrap()),
        revenue: None,
        profit: None,
    };
    let report = ResearchReport {
        report_id: NonEmptyText::new("H3_ABC").unwrap(),
        scope: ReportScope::Instrument(instrument()),
        title: NonEmptyText::new("公司深度报告").unwrap(),
        organization: NonEmptyText::new("示例证券").unwrap(),
        author: None,
        rating: Some(NonEmptyText::new("增持").unwrap()),
        published_at: NonEmptyText::new("2026-07-23 08:00:00").unwrap(),
        canonical_url: HttpsUrl::new("https://example.com/report/H3_ABC").unwrap(),
        pdf_url: Some(HttpsUrl::new("https://example.com/H3_ABC.pdf").unwrap()),
        estimates: vec![estimate.clone()],
        evidence: evidence(ProviderId::Eastmoney),
    };
    let consensus = ConsensusSnapshot {
        instrument: instrument(),
        estimates: vec![estimate],
        contributor_count: Some(PositiveU32::new(5).unwrap()),
        evidence: evidence(ProviderId::Tonghuashun),
    };

    assert_eq!(report.provider_id(), ProviderId::Eastmoney);
    assert_eq!(consensus.provider_id(), ProviderId::Tonghuashun);
    let json = serde_json::to_string(&report).unwrap();
    assert_eq!(
        serde_json::from_str::<ResearchReport>(&json).unwrap(),
        report
    );
}

#[test]
fn semantic_search_is_bounded_and_uses_https_documents() {
    let request = SemanticSearchRequest::new(
        "华电辽能 研报",
        SemanticChannel::Report,
        PositiveU32::new(20).unwrap(),
    )
    .unwrap();
    assert_eq!(request.query().as_str(), "华电辽能 研报");
    assert!(SemanticSearchRequest::new(
        "query",
        SemanticChannel::General,
        PositiveU32::new(101).unwrap()
    )
    .is_err());

    let document = SemanticSearchDocument {
        document_id: NonEmptyText::new("doc-1").unwrap(),
        channel: SemanticChannel::Report,
        title: NonEmptyText::new("报告").unwrap(),
        excerpt: None,
        canonical_url: HttpsUrl::new("https://example.com/doc-1").unwrap(),
        published_at: None,
        evidence: evidence(ProviderId::Iwencai),
    };
    assert_eq!(document.provider_id(), ProviderId::Iwencai);
    assert!(serde_json::from_str::<SemanticSearchDocument>(
        r#"{"document_id":"d","channel":"Report","title":"t","excerpt":null,"canonical_url":"http://example.com","published_at":null,"evidence":{"provider":"Iwencai","source_at":null,"observed_at":"o","batch_id":"b"}}"#
    )
    .is_err());
}
