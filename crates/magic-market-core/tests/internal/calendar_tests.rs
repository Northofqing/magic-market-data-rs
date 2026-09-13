use super::*;

#[test]
fn calendar_requests_revalidate_bounds() {
    assert!(EconomicCalendarRequest::new(PositiveU32::new(20).unwrap()).is_ok());
    assert!(EconomicCalendarRequest::new(PositiveU32::new(21).unwrap()).is_err());
    assert!(FuturesDeliveryRequest::new(
        PositiveU32::new(2026).unwrap(),
        PositiveU32::new(13).unwrap()
    )
    .is_err());
    assert!(serde_json::from_str::<FuturesDeliveryRequest>(r#"{"year":2026,"month":13}"#).is_err());
}

#[test]
fn economic_calendar_request_preserves_optional_country() {
    let request = EconomicCalendarRequest::new(PositiveU32::new(7).unwrap())
        .unwrap()
        .with_country("China")
        .unwrap();
    assert_eq!(request.limit().get(), 7);
    assert_eq!(request.country().unwrap().as_str(), "China");

    let restored: EconomicCalendarRequest =
        serde_json::from_str(r#"{"limit":7,"country":"China"}"#).unwrap();
    assert_eq!(restored, request);

    let without_country: EconomicCalendarRequest =
        serde_json::from_str(r#"{"limit":1,"country":null}"#).unwrap();
    assert!(without_country.country().is_none());
    assert!(
        serde_json::from_str::<EconomicCalendarRequest>(r#"{"limit":1,"country":"   "}"#).is_err()
    );
}

#[test]
fn economic_release_observation_request_is_a_bounded_window_not_a_calendar_range() {
    let request = EconomicReleaseObservationsRequest::new(PositiveU32::new(7).unwrap())
        .unwrap()
        .with_country("中国")
        .unwrap();
    assert_eq!(request.limit().get(), 7);
    assert_eq!(request.country().unwrap().as_str(), "中国");

    let restored: EconomicReleaseObservationsRequest =
        serde_json::from_str(r#"{"limit":7,"country":"中国"}"#).unwrap();
    assert_eq!(restored, request);
    assert!(serde_json::from_str::<EconomicReleaseObservationsRequest>(
        r#"{"limit":21,"country":null}"#
    )
    .is_err());
    assert!(serde_json::from_str::<EconomicReleaseObservationsRequest>(
        r#"{"limit":1,"country":"   "}"#
    )
    .is_err());
    assert!(serde_json::from_str::<EconomicReleaseObservationsRequest>(
        r#"{"limit":1,"start":"2026-09-12"}"#
    )
    .is_err());
}

#[test]
fn economic_release_schedule_request_preserves_an_inclusive_bounded_date_range() {
    let request = EconomicReleaseScheduleRequest::new(
        IsoDate::new("2026-09-01").unwrap(),
        IsoDate::new("2026-09-30").unwrap(),
        PositiveU32::new(20).unwrap(),
    )
    .unwrap();
    assert_eq!(request.start().as_str(), "2026-09-01");
    assert_eq!(request.end().as_str(), "2026-09-30");
    assert_eq!(request.limit().get(), 20);

    let restored: EconomicReleaseScheduleRequest =
        serde_json::from_str(r#"{"start":"2026-09-01","end":"2026-09-30","limit":20}"#).unwrap();
    assert_eq!(restored, request);
    assert!(EconomicReleaseScheduleRequest::new(
        IsoDate::new("2026-09-30").unwrap(),
        IsoDate::new("2026-09-01").unwrap(),
        PositiveU32::new(20).unwrap(),
    )
    .is_err());
    assert!(EconomicReleaseScheduleRequest::new(
        IsoDate::new("2026-01-01").unwrap(),
        IsoDate::new("2027-01-02").unwrap(),
        PositiveU32::new(20).unwrap(),
    )
    .is_err());
    assert!(serde_json::from_str::<EconomicReleaseScheduleRequest>(
        r#"{"start":"2026-09-01","end":"2026-09-30","limit":101}"#
    )
    .is_err());
    assert!(serde_json::from_str::<EconomicReleaseScheduleRequest>(
        r#"{"start":"2026-09-01","end":"2026-09-30","limit":20,"country":"US"}"#
    )
    .is_err());
}

#[test]
fn economic_release_schedule_entry_preserves_date_only_provider_evidence() {
    let evidence = SourceEvidence::new(
        crate::ProviderId::Fred,
        "2026-09-12T15:30:00Z",
        "FRED:economic-release-schedule:1",
    )
    .unwrap();
    let entry = EconomicReleaseScheduleEntry::new(
        PositiveU32::new(10).unwrap(),
        "Consumer Price Index",
        IsoDate::new("2026-09-15").unwrap(),
        Some("2026-08-01 09:30:00-05".to_owned()),
        evidence,
    )
    .unwrap();
    assert_eq!(entry.release_id().get(), 10);
    assert_eq!(entry.release_name().as_str(), "Consumer Price Index");
    assert_eq!(entry.release_date().as_str(), "2026-09-15");
    assert_eq!(
        entry.release_last_updated().unwrap().as_str(),
        "2026-08-01 09:30:00-05"
    );
    assert_eq!(entry.provider_id(), crate::ProviderId::Fred);
    assert_eq!(entry.evidence_observed_at(), Some("2026-09-12T15:30:00Z"));
    assert_eq!(entry.evidence_source_at(), None);

    let restored: EconomicReleaseScheduleEntry =
        serde_json::from_str(&serde_json::to_string(&entry).unwrap()).unwrap();
    assert_eq!(restored, entry);

    let source_at = SourceEvidence::new(
        crate::ProviderId::Fred,
        "2026-09-12T15:30:00Z",
        "FRED:economic-release-schedule:1",
    )
    .unwrap()
    .with_source_at("2026-09-15T00:00:00Z")
    .unwrap();
    assert!(EconomicReleaseScheduleEntry::new(
        PositiveU32::new(10).unwrap(),
        "Consumer Price Index",
        IsoDate::new("2026-09-15").unwrap(),
        None,
        source_at,
    )
    .is_err());
}

#[test]
fn economic_release_schedule_provider_is_a_public_seam() {
    fn assert_provider<T: EconomicReleaseScheduleProvider>() {}

    struct NoProvider;
    impl EconomicReleaseScheduleProvider for NoProvider {
        type Error = crate::CoreError;

        fn economic_release_schedule(
            &self,
            _request: &EconomicReleaseScheduleRequest,
        ) -> Result<DataBatch<EconomicReleaseScheduleEntry>, Self::Error> {
            unreachable!()
        }
    }

    assert_provider::<NoProvider>();
}

#[test]
fn futures_delivery_request_revalidates_year_and_exposes_month() {
    let request = FuturesDeliveryRequest::new(
        PositiveU32::new(2026).unwrap(),
        PositiveU32::new(7).unwrap(),
    )
    .unwrap();
    assert_eq!(request.year().get(), 2026);
    assert_eq!(request.month().get(), 7);
    assert_eq!(
        serde_json::from_str::<FuturesDeliveryRequest>(r#"{"year":2026,"month":7}"#).unwrap(),
        request
    );
    assert!(FuturesDeliveryRequest::new(
        PositiveU32::new(1999).unwrap(),
        PositiveU32::new(1).unwrap()
    )
    .is_err());
    assert!(FuturesDeliveryRequest::new(
        PositiveU32::new(10_000).unwrap(),
        PositiveU32::new(1).unwrap()
    )
    .is_err());
}

#[test]
fn calendar_records_expose_source_identity() {
    let evidence =
        SourceEvidence::new(crate::ProviderId::Cffex, "observed", "calendar-batch").unwrap();
    let event = EconomicEvent {
        event_id: NonEmptyText::new("event-1").unwrap(),
        indicator_id: PositiveU32::new(1).unwrap(),
        country: NonEmptyText::new("中国").unwrap(),
        name: NonEmptyText::new("工业企业利润").unwrap(),
        period: None,
        scheduled_at: NonEmptyText::new("2026-07-25T09:30:00+08:00").unwrap(),
        released_at: NonEmptyText::new("2026-07-25T09:30:01+08:00").unwrap(),
        previous: None,
        consensus: None,
        actual: None,
        revised: None,
        unit: None,
        importance: PositiveU32::new(1).unwrap(),
        impact: None,
        evidence: evidence.clone(),
    };
    assert_eq!(event.provider_id(), crate::ProviderId::Cffex);
    assert_eq!(event.evidence_batch_id(), "calendar-batch");

    let delivery = FuturesDeliveryEvent {
        product: FuturesProduct::If,
        contract_code: NonEmptyText::new("IF2607").unwrap(),
        last_trading_date: None,
        delivery_date: IsoDate::new("2026-07-17").unwrap(),
        method: FuturesDeliveryMethod::NotProvided,
        notice_url: HttpsUrl::new("https://www.cffex.com.cn/notice.html").unwrap(),
        evidence,
    };
    assert_eq!(delivery.provider_id(), crate::ProviderId::Cffex);
    assert_eq!(delivery.evidence_batch_id(), "calendar-batch");
    let restored: FuturesDeliveryEvent =
        serde_json::from_str(&serde_json::to_string(&delivery).unwrap()).unwrap();
    assert_eq!(restored, delivery);
    assert_eq!(restored.method, FuturesDeliveryMethod::NotProvided);
    assert!(restored.last_trading_date.is_none());
}
