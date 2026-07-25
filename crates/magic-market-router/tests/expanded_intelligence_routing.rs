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
    ResearchDocumentRouter, SourceError,
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
                b"%PDF-1.7 fixture".to_vec(),
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
