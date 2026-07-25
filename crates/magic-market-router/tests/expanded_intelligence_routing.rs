use magic_market_core::{
    Announcement, AnnouncementDiscovery, AnnouncementDiscoveryRequest, AssetClass, DataBatch,
    EconomicCalendarProvider, EconomicCalendarRequest, EconomicEvent, Exchange,
    ForeignExchangeProvider, FuturesDeliveryCalendar, FuturesDeliveryEvent, FuturesDeliveryMethod,
    FuturesDeliveryRequest, FuturesProduct, FxPair, FxQuote, FxRequest, GlobalIndexCode,
    GlobalIndexProvider, GlobalIndexQuote, GlobalIndexRequest, HttpsUrl, InstrumentId, IsoDate,
    NonEmptyText, PolicyDocument, PolicyDocuments, PolicyRequest, PositiveU32, Price, Provenance,
    ProviderId, Ratio, RatioUnit, ResearchDocument, ResearchDocumentRequest, ResearchDocuments,
    SourceEvidence,
};
use magic_market_router::{
    announcement_discovery_source, economic_calendar_source, foreign_exchange_source,
    futures_delivery_source, global_index_source, policy_document_source, research_document_source,
    AcceptancePolicy, AnnouncementDiscoveryRouter, EconomicCalendarRouter, FailureKind,
    ForeignExchangeRouter, FuturesDeliveryRouter, GlobalIndexRouter, PolicyDocumentRouter,
    ResearchDocumentRouter, RoutedSource, SourceError,
};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("fixture")]
struct FixtureError;

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Provider, "fixture")
}

fn text(value: &str) -> NonEmptyText {
    NonEmptyText::new(value).unwrap()
}

fn evidence(batch_id: &str, source_at: Option<&str>) -> SourceEvidence {
    let evidence = SourceEvidence::new(ProviderId::Custom, "observed", batch_id).unwrap();
    match source_at {
        Some(value) => evidence.with_source_at(value).unwrap(),
        None => evidence,
    }
}

fn batch<T>(records: Vec<T>, batch_id: &str, source_at: Option<&str>) -> DataBatch<T> {
    let provenance = Provenance::new("fixture", "observed")
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap();
    let provenance = match source_at {
        Some(value) => provenance.with_source_at(value).unwrap(),
        None => provenance,
    };
    DataBatch::strict(records, provenance)
}

struct Fixture;

impl AnnouncementDiscovery for Fixture {
    type Error = FixtureError;

    fn discover_announcements(
        &self,
        _request: &AnnouncementDiscoveryRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error> {
        Ok(batch(
            vec![Announcement {
                announcement_id: text("announcement-1"),
                instrument: InstrumentId::new(Exchange::Shanghai, "600000", AssetClass::Equity)
                    .unwrap(),
                instrument_name: Some(text("浦发银行")),
                category: Some(text("年度报告")),
                title: text("fixture announcement"),
                published_at: text("2026-07-24T12:00:00+08:00"),
                canonical_url: HttpsUrl::new("https://example.com/announcement").unwrap(),
                pdf_url: Some(HttpsUrl::new("https://example.com/announcement.pdf").unwrap()),
                evidence: evidence("announcement", Some("2026-07-24T12:00:00+08:00")),
            }],
            "announcement",
            Some("2026-07-24T12:00:00+08:00"),
        ))
    }
}

impl GlobalIndexProvider for Fixture {
    type Error = FixtureError;

    fn global_indices(
        &self,
        _request: &GlobalIndexRequest,
    ) -> Result<DataBatch<GlobalIndexQuote>, Self::Error> {
        Ok(batch(
            vec![GlobalIndexQuote {
                index: GlobalIndexCode::Sp500,
                name: text("S&P 500"),
                value: Price::new(6_400.0).unwrap(),
                change: magic_market_core::FiniteNumber::new(5.0).unwrap(),
                change_percent: Ratio::new(0.08, RatioUnit::Percent).unwrap(),
                evidence: evidence("global", None),
            }],
            "global",
            None,
        ))
    }
}

impl ForeignExchangeProvider for Fixture {
    type Error = FixtureError;

    fn foreign_exchange(&self, _request: &FxRequest) -> Result<DataBatch<FxQuote>, Self::Error> {
        Ok(batch(
            vec![FxQuote {
                pair: FxPair::UsdCny,
                name: text("美元人民币"),
                rate: Price::new(7.16).unwrap(),
                change: None,
                change_percent: None,
                evidence: evidence("fx", Some("2026-07-24T15:30:00+08:00")),
            }],
            "fx",
            Some("2026-07-24T15:30:00+08:00"),
        ))
    }
}

impl EconomicCalendarProvider for Fixture {
    type Error = FixtureError;

    fn economic_calendar(
        &self,
        _request: &EconomicCalendarRequest,
    ) -> Result<DataBatch<EconomicEvent>, Self::Error> {
        Ok(batch(
            vec![EconomicEvent {
                event_id: text("event-1"),
                indicator_id: PositiveU32::new(950).unwrap(),
                country: text("中国"),
                name: text("工业企业利润"),
                period: Some(text("6月")),
                scheduled_at: text("2026-07-25T09:30:00+08:00"),
                released_at: text("2026-07-25T09:30:01+08:00"),
                previous: Some(text("-9.1")),
                consensus: None,
                actual: Some(text("0")),
                revised: None,
                unit: Some(text("%")),
                importance: PositiveU32::new(3).unwrap(),
                impact: Some(text("利多")),
                evidence: evidence("economic", Some("2026-07-25T09:30:01+08:00")),
            }],
            "economic",
            Some("2026-07-25T09:30:01+08:00"),
        ))
    }
}

impl PolicyDocuments for Fixture {
    type Error = FixtureError;

    fn policy_documents(
        &self,
        _request: &PolicyRequest,
    ) -> Result<DataBatch<PolicyDocument>, Self::Error> {
        Ok(batch(
            vec![PolicyDocument {
                document_id: text("policy-1"),
                title: text("十五五规划"),
                summary: Some(text("fixture")),
                organization: text("国务院"),
                document_number: Some(text("国发〔2026〕1号")),
                category: Some(text("gongwen")),
                published_date: IsoDate::new("2026-07-24").unwrap(),
                canonical_url: HttpsUrl::new("https://www.gov.cn/zhengce/policy.html").unwrap(),
                evidence: evidence("policy", Some("2026-07-24")),
            }],
            "policy",
            Some("2026-07-24"),
        ))
    }
}

impl ResearchDocuments for Fixture {
    type Error = FixtureError;

    fn research_document(
        &self,
        request: &ResearchDocumentRequest,
    ) -> Result<DataBatch<ResearchDocument>, Self::Error> {
        Ok(batch(
            vec![ResearchDocument::new(
                request.report_id.clone(),
                request.pdf_url.clone(),
                b"%PDF-1.7\nfixture\nstartxref\n9\n%%EOF\n".to_vec(),
                evidence("research-document", Some("2026-07-24")),
            )
            .unwrap()],
            "research-document",
            Some("2026-07-24"),
        ))
    }
}

impl FuturesDeliveryCalendar for Fixture {
    type Error = FixtureError;

    fn futures_delivery_calendar(
        &self,
        _request: &FuturesDeliveryRequest,
    ) -> Result<DataBatch<FuturesDeliveryEvent>, Self::Error> {
        let records = [
            (FuturesProduct::If, "IF2602"),
            (FuturesProduct::Ih, "IH2602"),
            (FuturesProduct::Ic, "IC2602"),
            (FuturesProduct::Im, "IM2602"),
        ]
        .into_iter()
        .map(|(product, contract)| FuturesDeliveryEvent {
            product,
            contract_code: text(contract),
            last_trading_date: IsoDate::new("2026-02-24").unwrap(),
            delivery_date: IsoDate::new("2026-02-24").unwrap(),
            method: FuturesDeliveryMethod::Cash,
            notice_url: HttpsUrl::new("https://www.cffex.com.cn/jystz/notice.html").unwrap(),
            evidence: evidence("delivery", Some("2026-02-24")),
        })
        .collect();
        Ok(batch(records, "delivery", Some("2026-02-24")))
    }
}

#[test]
fn expanded_intelligence_adapters_admit_exact_evidenced_batches() {
    let provider = Arc::new(Fixture);

    let announcement_request = AnnouncementDiscoveryRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    let mut announcements = AnnouncementDiscoveryRouter::new(AcceptancePolicy::new());
    announcements
        .register(announcement_discovery_source(
            ProviderId::Custom,
            Arc::clone(&provider),
            classify,
        ))
        .unwrap();
    assert_eq!(
        announcements
            .route(&announcement_request)
            .unwrap()
            .batch()
            .records()[0]
            .instrument_name
            .as_ref()
            .unwrap()
            .as_str(),
        "浦发银行"
    );

    let global_request = GlobalIndexRequest::new(vec![GlobalIndexCode::Sp500]).unwrap();
    let mut global = GlobalIndexRouter::new(AcceptancePolicy::new());
    global
        .register(global_index_source(
            ProviderId::Custom,
            Arc::clone(&provider),
            classify,
        ))
        .unwrap();
    assert_eq!(
        global
            .route(&global_request)
            .unwrap()
            .batch()
            .records()
            .len(),
        1
    );

    let fx_request = FxRequest::new(vec![FxPair::UsdCny]).unwrap();
    let mut fx = ForeignExchangeRouter::new(AcceptancePolicy::new());
    fx.register(foreign_exchange_source(
        ProviderId::Custom,
        Arc::clone(&provider),
        classify,
    ))
    .unwrap();
    assert_eq!(fx.route(&fx_request).unwrap().batch().records().len(), 1);

    let economic_request = EconomicCalendarRequest::new(PositiveU32::new(10).unwrap())
        .unwrap()
        .with_country("中国")
        .unwrap();
    let mut economic = EconomicCalendarRouter::new(AcceptancePolicy::new());
    economic
        .register(economic_calendar_source(
            ProviderId::Custom,
            Arc::clone(&provider),
            classify,
        ))
        .unwrap();
    assert_eq!(
        economic.route(&economic_request).unwrap().batch().records()[0]
            .actual
            .as_ref()
            .unwrap()
            .as_str(),
        "0"
    );

    let policy_request =
        PolicyRequest::new(PositiveU32::new(1).unwrap(), PositiveU32::new(10).unwrap()).unwrap();
    let mut policy = PolicyDocumentRouter::new(AcceptancePolicy::new());
    policy
        .register(policy_document_source(
            ProviderId::Custom,
            Arc::clone(&provider),
            classify,
        ))
        .unwrap();
    assert_eq!(
        policy
            .route(&policy_request)
            .unwrap()
            .batch()
            .records()
            .len(),
        1
    );

    let document_request = ResearchDocumentRequest {
        report_id: text("report-1"),
        pdf_url: HttpsUrl::new("https://pdf.dfcfw.com/report.pdf").unwrap(),
    };
    let mut document = ResearchDocumentRouter::new(AcceptancePolicy::new());
    document
        .register(research_document_source(
            ProviderId::Custom,
            Arc::clone(&provider),
            classify,
        ))
        .unwrap();
    assert!(
        document.route(&document_request).unwrap().batch().records()[0]
            .body
            .starts_with(b"%PDF-")
    );

    let delivery_request = FuturesDeliveryRequest::new(
        PositiveU32::new(2026).unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    let mut delivery = FuturesDeliveryRouter::new(AcceptancePolicy::new());
    delivery
        .register(futures_delivery_source(
            ProviderId::Custom,
            provider,
            classify,
        ))
        .unwrap();
    assert_eq!(
        delivery
            .route(&delivery_request)
            .unwrap()
            .batch()
            .records()
            .len(),
        4
    );
}

struct StaticAnnouncements(Vec<Announcement>);

impl AnnouncementDiscovery for StaticAnnouncements {
    type Error = FixtureError;

    fn discover_announcements(
        &self,
        _request: &AnnouncementDiscoveryRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error> {
        Ok(batch(self.0.clone(), "announcements-invalid", None))
    }
}

fn announcement(id: &str, exchange: Exchange, date: &str, name: Option<&str>) -> Announcement {
    Announcement {
        announcement_id: text(id),
        instrument: InstrumentId::new(exchange, "600000", AssetClass::Equity).unwrap(),
        instrument_name: name.map(text),
        category: None,
        title: text("fixture announcement"),
        published_at: text(date),
        canonical_url: HttpsUrl::new("https://example.com/announcement").unwrap(),
        pdf_url: None,
        evidence: evidence("announcements-invalid", None),
    }
}

#[test]
fn announcement_discovery_adapter_rejects_every_identity_and_range_violation() {
    let request = AnnouncementDiscoveryRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap()
    .with_exchange(Exchange::Shanghai);
    let valid = announcement(
        "a",
        Exchange::Shanghai,
        "2026-07-24T12:00:00+08:00",
        Some("浦发银行"),
    );
    let cases = [
        vec![valid.clone(), valid.clone()],
        vec![announcement(
            "a",
            Exchange::Shanghai,
            "2026-07-24T12:00:00+08:00",
            None,
        )],
        vec![announcement(
            "a",
            Exchange::Shanghai,
            "bad",
            Some("浦发银行"),
        )],
        vec![announcement(
            "a",
            Exchange::Shanghai,
            "2026-07-23T12:00:00+08:00",
            Some("浦发银行"),
        )],
        vec![announcement(
            "a",
            Exchange::Shenzhen,
            "2026-07-24T12:00:00+08:00",
            Some("浦发银行"),
        )],
        vec![valid.clone(), valid],
    ];
    for records in cases {
        let source = announcement_discovery_source(
            ProviderId::Custom,
            Arc::new(StaticAnnouncements(records)),
            classify,
        );
        assert!(source.fetch(&request).is_err());
    }

    let duplicate_request = AnnouncementDiscoveryRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap()
    .with_exchange(Exchange::Shanghai);
    let duplicate = announcement(
        "duplicate",
        Exchange::Shanghai,
        "2026-07-24T12:00:00+08:00",
        Some("浦发银行"),
    );
    let source = announcement_discovery_source(
        ProviderId::Custom,
        Arc::new(StaticAnnouncements(vec![duplicate.clone(), duplicate])),
        classify,
    );
    assert!(source.fetch(&duplicate_request).is_err());
}

struct StaticGlobal(Vec<GlobalIndexQuote>);

impl GlobalIndexProvider for StaticGlobal {
    type Error = FixtureError;

    fn global_indices(
        &self,
        _request: &GlobalIndexRequest,
    ) -> Result<DataBatch<GlobalIndexQuote>, Self::Error> {
        Ok(batch(self.0.clone(), "global-invalid", None))
    }
}

fn global_quote(index: GlobalIndexCode) -> GlobalIndexQuote {
    GlobalIndexQuote {
        index,
        name: text("global"),
        value: Price::new(1.0).unwrap(),
        change: magic_market_core::FiniteNumber::new(0.0).unwrap(),
        change_percent: Ratio::new(0.0, RatioUnit::Percent).unwrap(),
        evidence: evidence("global-invalid", None),
    }
}

struct StaticFx(Vec<FxQuote>);

impl ForeignExchangeProvider for StaticFx {
    type Error = FixtureError;

    fn foreign_exchange(&self, _request: &FxRequest) -> Result<DataBatch<FxQuote>, Self::Error> {
        Ok(batch(self.0.clone(), "fx-invalid", None))
    }
}

fn fx_quote(pair: FxPair) -> FxQuote {
    FxQuote {
        pair,
        name: text("fx"),
        rate: Price::new(1.0).unwrap(),
        change: None,
        change_percent: None,
        evidence: evidence("fx-invalid", None),
    }
}

#[test]
fn global_and_fx_adapters_reject_cardinality_unrequested_and_duplicate_identities() {
    let global_request =
        GlobalIndexRequest::new(vec![GlobalIndexCode::Sp500, GlobalIndexCode::DowJones]).unwrap();
    for records in [
        vec![global_quote(GlobalIndexCode::Sp500)],
        vec![
            global_quote(GlobalIndexCode::Sp500),
            global_quote(GlobalIndexCode::NasdaqComposite),
        ],
        vec![
            global_quote(GlobalIndexCode::Sp500),
            global_quote(GlobalIndexCode::Sp500),
        ],
    ] {
        let source = global_index_source(
            ProviderId::Custom,
            Arc::new(StaticGlobal(records)),
            classify,
        );
        assert!(source.fetch(&global_request).is_err());
    }

    let fx_request = FxRequest::new(vec![FxPair::UsdCny, FxPair::EurUsd]).unwrap();
    for records in [
        vec![fx_quote(FxPair::UsdCny)],
        vec![fx_quote(FxPair::UsdCny), fx_quote(FxPair::UsdJpy)],
        vec![fx_quote(FxPair::UsdCny), fx_quote(FxPair::UsdCny)],
    ] {
        let source =
            foreign_exchange_source(ProviderId::Custom, Arc::new(StaticFx(records)), classify);
        assert!(source.fetch(&fx_request).is_err());
    }
}

struct StaticEconomic(Vec<EconomicEvent>);

impl EconomicCalendarProvider for StaticEconomic {
    type Error = FixtureError;

    fn economic_calendar(
        &self,
        _request: &EconomicCalendarRequest,
    ) -> Result<DataBatch<EconomicEvent>, Self::Error> {
        Ok(batch(self.0.clone(), "economic-invalid", None))
    }
}

fn economic_event(id: &str, country: &str, released_at: &str) -> EconomicEvent {
    EconomicEvent {
        event_id: text(id),
        indicator_id: PositiveU32::new(1).unwrap(),
        country: text(country),
        name: text("indicator"),
        period: None,
        scheduled_at: text(released_at),
        released_at: text(released_at),
        previous: None,
        consensus: None,
        actual: None,
        revised: None,
        unit: None,
        importance: PositiveU32::new(1).unwrap(),
        impact: None,
        evidence: evidence("economic-invalid", None),
    }
}

#[test]
fn economic_adapter_rejects_limit_country_duplicates_and_wrong_order() {
    let request = EconomicCalendarRequest::new(PositiveU32::new(1).unwrap())
        .unwrap()
        .with_country("中国")
        .unwrap();
    let cases = [
        vec![
            economic_event("a", "中国", "2026-07-24T12:00:00+08:00"),
            economic_event("b", "中国", "2026-07-23T12:00:00+08:00"),
        ],
        vec![economic_event("a", "美国", "2026-07-24T12:00:00+08:00")],
        vec![
            economic_event("a", "中国", "2026-07-24T12:00:00+08:00"),
            economic_event("a", "中国", "2026-07-23T12:00:00+08:00"),
        ],
        vec![
            economic_event("a", "中国", "2026-07-23T12:00:00+08:00"),
            economic_event("b", "中国", "2026-07-24T12:00:00+08:00"),
        ],
    ];
    for records in cases {
        let source = economic_calendar_source(
            ProviderId::Custom,
            Arc::new(StaticEconomic(records)),
            classify,
        );
        assert!(source.fetch(&request).is_err());
    }

    let validation_request = EconomicCalendarRequest::new(PositiveU32::new(2).unwrap())
        .unwrap()
        .with_country("中国")
        .unwrap();
    for records in [
        vec![
            economic_event("a", "中国", "2026-07-24T12:00:00+08:00"),
            economic_event("a", "中国", "2026-07-23T12:00:00+08:00"),
        ],
        vec![
            economic_event("a", "中国", "2026-07-23T12:00:00+08:00"),
            economic_event("b", "中国", "2026-07-24T12:00:00+08:00"),
        ],
    ] {
        let source = economic_calendar_source(
            ProviderId::Custom,
            Arc::new(StaticEconomic(records)),
            classify,
        );
        assert!(source.fetch(&validation_request).is_err());
    }
}

struct StaticPolicy(Vec<PolicyDocument>);

impl PolicyDocuments for StaticPolicy {
    type Error = FixtureError;

    fn policy_documents(
        &self,
        _request: &PolicyRequest,
    ) -> Result<DataBatch<PolicyDocument>, Self::Error> {
        Ok(batch(self.0.clone(), "policy-invalid", None))
    }
}

fn policy_document(id: &str, date: &str) -> PolicyDocument {
    PolicyDocument {
        document_id: text(id),
        title: text("policy"),
        summary: None,
        organization: text("国务院"),
        document_number: None,
        category: None,
        published_date: IsoDate::new(date).unwrap(),
        canonical_url: HttpsUrl::new("https://www.gov.cn/policy.html").unwrap(),
        evidence: evidence("policy-invalid", None),
    }
}

#[test]
fn policy_adapter_rejects_page_range_duplicates_and_wrong_order() {
    let request = PolicyRequest::new(PositiveU32::new(1).unwrap(), PositiveU32::new(1).unwrap())
        .unwrap()
        .with_range(
            IsoDate::new("2026-07-23").unwrap(),
            IsoDate::new("2026-07-24").unwrap(),
        )
        .unwrap();
    let cases = [
        vec![
            policy_document("a", "2026-07-24"),
            policy_document("b", "2026-07-23"),
        ],
        vec![policy_document("a", "2026-07-22")],
        vec![
            policy_document("a", "2026-07-24"),
            policy_document("a", "2026-07-23"),
        ],
        vec![
            policy_document("a", "2026-07-23"),
            policy_document("b", "2026-07-24"),
        ],
    ];
    for records in cases {
        let source = policy_document_source(
            ProviderId::Custom,
            Arc::new(StaticPolicy(records)),
            classify,
        );
        assert!(source.fetch(&request).is_err());
    }

    let validation_request =
        PolicyRequest::new(PositiveU32::new(1).unwrap(), PositiveU32::new(2).unwrap())
            .unwrap()
            .with_range(
                IsoDate::new("2026-07-23").unwrap(),
                IsoDate::new("2026-07-24").unwrap(),
            )
            .unwrap();
    for records in [
        vec![
            policy_document("a", "2026-07-24"),
            policy_document("a", "2026-07-23"),
        ],
        vec![
            policy_document("a", "2026-07-23"),
            policy_document("b", "2026-07-24"),
        ],
    ] {
        let source = policy_document_source(
            ProviderId::Custom,
            Arc::new(StaticPolicy(records)),
            classify,
        );
        assert!(source.fetch(&validation_request).is_err());
    }
}

struct StaticResearch(Vec<ResearchDocument>);

impl ResearchDocuments for StaticResearch {
    type Error = FixtureError;

    fn research_document(
        &self,
        _request: &ResearchDocumentRequest,
    ) -> Result<DataBatch<ResearchDocument>, Self::Error> {
        Ok(batch(self.0.clone(), "research-invalid", None))
    }
}

fn research_document(id: &str, url: &str) -> ResearchDocument {
    ResearchDocument::new(
        text(id),
        HttpsUrl::new(url).unwrap(),
        b"%PDF-1.7\nfixture\nstartxref\n9\n%%EOF\n".to_vec(),
        evidence("research-invalid", None),
    )
    .unwrap()
}

#[test]
fn research_document_adapter_rejects_cardinality_and_identity_mismatch() {
    let request = ResearchDocumentRequest {
        report_id: text("expected"),
        pdf_url: HttpsUrl::new("https://example.com/expected.pdf").unwrap(),
    };
    for records in [
        Vec::new(),
        vec![
            research_document("expected", "https://example.com/expected.pdf"),
            research_document("expected", "https://example.com/expected.pdf"),
        ],
        vec![research_document(
            "wrong",
            "https://example.com/expected.pdf",
        )],
        vec![research_document(
            "expected",
            "https://example.com/wrong.pdf",
        )],
    ] {
        let source = research_document_source(
            ProviderId::Custom,
            Arc::new(StaticResearch(records)),
            classify,
        );
        assert!(source.fetch(&request).is_err());
    }
}

struct StaticDelivery(Vec<FuturesDeliveryEvent>);

impl FuturesDeliveryCalendar for StaticDelivery {
    type Error = FixtureError;

    fn futures_delivery_calendar(
        &self,
        _request: &FuturesDeliveryRequest,
    ) -> Result<DataBatch<FuturesDeliveryEvent>, Self::Error> {
        Ok(batch(self.0.clone(), "delivery-invalid", None))
    }
}

fn delivery_event(
    product: FuturesProduct,
    contract: &str,
    last: &str,
    delivery: &str,
) -> FuturesDeliveryEvent {
    FuturesDeliveryEvent {
        product,
        contract_code: text(contract),
        last_trading_date: IsoDate::new(last).unwrap(),
        delivery_date: IsoDate::new(delivery).unwrap(),
        method: FuturesDeliveryMethod::Cash,
        notice_url: HttpsUrl::new("https://www.cffex.com.cn/notice.html").unwrap(),
        evidence: evidence("delivery-invalid", None),
    }
}

fn valid_deliveries() -> Vec<FuturesDeliveryEvent> {
    [
        (FuturesProduct::If, "IF2602"),
        (FuturesProduct::Ih, "IH2602"),
        (FuturesProduct::Ic, "IC2602"),
        (FuturesProduct::Im, "IM2602"),
    ]
    .into_iter()
    .map(|(product, contract)| delivery_event(product, contract, "2026-02-24", "2026-02-24"))
    .collect()
}

#[test]
fn futures_delivery_adapter_rejects_incomplete_duplicate_and_wrong_month_records() {
    let request = FuturesDeliveryRequest::new(
        PositiveU32::new(2026).unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    let mut duplicate = valid_deliveries();
    duplicate[3] = delivery_event(FuturesProduct::Ic, "IC2602", "2026-02-24", "2026-02-24");
    let mut wrong_contract = valid_deliveries();
    wrong_contract[0].contract_code = text("IF2603");
    let mut wrong_month = valid_deliveries();
    wrong_month[0].delivery_date = IsoDate::new("2026-03-24").unwrap();
    let mut mismatched_dates = valid_deliveries();
    mismatched_dates[0].last_trading_date = IsoDate::new("2026-02-23").unwrap();
    for records in [
        vec![valid_deliveries()[0].clone()],
        duplicate,
        wrong_contract,
        wrong_month,
        mismatched_dates,
    ] {
        let source = futures_delivery_source(
            ProviderId::Custom,
            Arc::new(StaticDelivery(records)),
            classify,
        );
        assert!(source.fetch(&request).is_err());
    }
}
