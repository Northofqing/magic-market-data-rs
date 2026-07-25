use magic_market_core::{
    Announcement, AnnouncementDiscoveryRequest, AssetClass, Exchange, HttpsUrl, InstrumentId,
    InvestorQuestion, IsoDate, NewsItem, NonEmptyText, PositiveU32, ProviderId, SourceEvidence,
    SourcedRecord,
};

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn evidence(provider: ProviderId) -> SourceEvidence {
    SourceEvidence::new(provider, "observed", "batch").unwrap()
}

#[test]
fn news_and_announcement_urls_are_https_and_sourced() {
    let news = NewsItem {
        item_id: NonEmptyText::new("news-1").unwrap(),
        title: NonEmptyText::new("华电辽能新闻").unwrap(),
        summary: None,
        content: None,
        publisher: NonEmptyText::new("东方财富").unwrap(),
        canonical_url: HttpsUrl::new("https://example.com/news-1").unwrap(),
        published_at: NonEmptyText::new("2026-07-23 10:00:00").unwrap(),
        instruments: vec![instrument()],
        topics: vec![],
        language: NonEmptyText::new("zh-CN").unwrap(),
        evidence: evidence(ProviderId::Eastmoney),
    };
    let announcement = Announcement {
        announcement_id: NonEmptyText::new("ann-1").unwrap(),
        instrument: instrument(),
        instrument_name: Some(NonEmptyText::new("华电辽能").unwrap()),
        category: Some(NonEmptyText::new("公司公告").unwrap()),
        title: NonEmptyText::new("年度报告").unwrap(),
        published_at: NonEmptyText::new("2026-07-23").unwrap(),
        canonical_url: HttpsUrl::new("https://example.com/ann-1").unwrap(),
        pdf_url: None,
        evidence: evidence(ProviderId::Cninfo),
    };

    assert_eq!(news.provider_id(), ProviderId::Eastmoney);
    assert_eq!(announcement.provider_id(), ProviderId::Cninfo);
    assert!(serde_json::from_str::<NewsItem>(
        &serde_json::to_string(&news)
            .unwrap()
            .replace("https://example.com", "http://example.com")
    )
    .is_err());
}

#[test]
fn unanswered_investor_question_is_not_fabricated() {
    let question = InvestorQuestion::new(
        NonEmptyText::new("q-1").unwrap(),
        instrument(),
        NonEmptyText::new("公司").unwrap(),
        NonEmptyText::new("请问项目进展？").unwrap(),
        NonEmptyText::new("2026-07-23 09:00:00").unwrap(),
        None,
        None,
        evidence(ProviderId::Cninfo),
    )
    .unwrap();
    assert_eq!(question.question_id().as_str(), "q-1");
    assert_eq!(question.instrument(), &instrument());
    assert_eq!(question.company().as_str(), "公司");
    assert_eq!(question.question().as_str(), "请问项目进展？");
    assert_eq!(question.question_at().as_str(), "2026-07-23 09:00:00");
    assert!(question.answer().is_none());
    assert!(question.answer_at().is_none());
    assert!(question.source_question_id().is_none());
    assert!(question.answerer().is_none());
    assert_eq!(question.evidence().provider(), ProviderId::Cninfo);

    let answered = InvestorQuestion::new_with_metadata(
        NonEmptyText::new("q-answered").unwrap(),
        instrument(),
        NonEmptyText::new("公司").unwrap(),
        NonEmptyText::new("问题").unwrap(),
        NonEmptyText::new("2026-07-23").unwrap(),
        Some(NonEmptyText::new("回答").unwrap()),
        Some(NonEmptyText::new("2026-07-24").unwrap()),
        Some(NonEmptyText::new("source-q").unwrap()),
        Some(NonEmptyText::new("董秘").unwrap()),
        evidence(ProviderId::Cninfo),
    )
    .unwrap();
    assert_eq!(answered.answer().unwrap().as_str(), "回答");
    assert_eq!(answered.answer_at().unwrap().as_str(), "2026-07-24");
    assert_eq!(answered.source_question_id().unwrap().as_str(), "source-q");
    assert_eq!(answered.answerer().unwrap().as_str(), "董秘");
    assert_eq!(
        serde_json::from_str::<InvestorQuestion>(&serde_json::to_string(&answered).unwrap())
            .unwrap(),
        answered
    );

    assert!(InvestorQuestion::new(
        NonEmptyText::new("q-2").unwrap(),
        instrument(),
        NonEmptyText::new("公司").unwrap(),
        NonEmptyText::new("问题").unwrap(),
        NonEmptyText::new("2026-07-23").unwrap(),
        None,
        Some(NonEmptyText::new("2026-07-24").unwrap()),
        evidence(ProviderId::Cninfo),
    )
    .is_err());
    assert!(InvestorQuestion::new_with_metadata(
        NonEmptyText::new("q-3").unwrap(),
        instrument(),
        NonEmptyText::new("公司").unwrap(),
        NonEmptyText::new("问题").unwrap(),
        NonEmptyText::new("2026-07-23").unwrap(),
        None,
        None,
        Some(NonEmptyText::new("source-3").unwrap()),
        Some(NonEmptyText::new("董秘").unwrap()),
        evidence(ProviderId::Cninfo),
    )
    .is_err());
}

#[test]
fn announcement_discovery_request_revalidates_all_bounds_and_accessors() {
    let request = AnnouncementDiscoveryRequest::new(
        IsoDate::new("2026-01-01").unwrap(),
        IsoDate::new("2026-07-25").unwrap(),
        PositiveU32::new(10_000).unwrap(),
    )
    .unwrap()
    .with_exchange(Exchange::Shanghai);
    assert_eq!(request.start().as_str(), "2026-01-01");
    assert_eq!(request.end().as_str(), "2026-07-25");
    assert_eq!(request.exchange(), Some(Exchange::Shanghai));
    assert_eq!(request.limit().get(), 10_000);
    assert_eq!(
        serde_json::from_str::<AnnouncementDiscoveryRequest>(
            &serde_json::to_string(&request).unwrap()
        )
        .unwrap(),
        request
    );
    assert!(AnnouncementDiscoveryRequest::new(
        IsoDate::new("2026-07-25").unwrap(),
        IsoDate::new("2026-01-01").unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .is_err());
    assert!(AnnouncementDiscoveryRequest::new(
        IsoDate::new("2026-01-01").unwrap(),
        IsoDate::new("2026-07-25").unwrap(),
        PositiveU32::new(10_001).unwrap(),
    )
    .is_err());
}
