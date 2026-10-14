use magic_market_core::{
    Adjustment, AssetClass, Bar, BarInterval, BarsRequest, Board, DataBatch, DataStatus, Exchange,
    HistoricalBars, InstrumentId, Money, Price, PriceLimitRule, Provenance, ProviderId, Quantity,
    SecurityMetadata, SecurityMetadataProvider,
};
use magic_market_router::{
    bars_source, security_metadata_source, AcceptancePolicy, AttemptStatus, FailureAction,
    FailureKind, RoutedSource, RouterError, SecurityMetadataRouter, SourceError,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const OBSERVED: &str = "2026-10-09T15:00:05+08:00";
const SOURCE: &str = "2026-10-09T15:00:00+08:00";

#[derive(Debug, Clone)]
struct OfflineError(&'static str);

impl std::fmt::Display for OfflineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for OfflineError {}

// Only the external provider is substituted; requests and records use real Core APIs.
struct OfflineMetadata {
    result: Result<DataBatch<SecurityMetadata>, OfflineError>,
    requests: Mutex<Vec<Vec<InstrumentId>>>,
}

impl OfflineMetadata {
    fn new(result: Result<DataBatch<SecurityMetadata>, OfflineError>) -> Self {
        Self {
            result,
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl SecurityMetadataProvider for OfflineMetadata {
    type Error = OfflineError;

    fn security_metadata(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        self.requests.lock().unwrap().push(instruments.to_vec());
        self.result.clone()
    }
}

fn instrument(asset_class: AssetClass) -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600519", asset_class).unwrap()
}

fn provenance(batch_id: &str) -> Provenance {
    Provenance::new("offline-security-master", OBSERVED)
        .unwrap()
        .with_source_at(SOURCE)
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap()
}

fn metadata(
    asset_class: AssetClass,
    provider: ProviderId,
    batch_id: &str,
    observed_at: &str,
) -> SecurityMetadata {
    SecurityMetadata::new(
        instrument(asset_class),
        Some("贵州茅台".into()),
        Some(Board::Main),
        Some(false),
        Some("2001-08-27".into()),
        PriceLimitRule::new(None, None).unwrap(),
        DataStatus::Unavailable,
        Some(SOURCE.into()),
        observed_at,
        provider,
        batch_id,
    )
    .unwrap()
}

fn unexpected_error(error: OfflineError) -> SourceError {
    panic!("successful offline provider unexpectedly failed: {error}")
}

#[test]
fn metadata_adapter_keeps_provider_order_typed_identities_and_partial_quality() {
    let requested = [instrument(AssetClass::Equity), instrument(AssetClass::Fund)];
    let records = vec![
        metadata(
            AssetClass::Fund,
            ProviderId::Tdx,
            "master:partial",
            OBSERVED,
        ),
        metadata(
            AssetClass::Equity,
            ProviderId::Tdx,
            "master:partial",
            OBSERVED,
        ),
    ];
    let batch = DataBatch::best_effort(
        records,
        provenance("master:partial"),
        vec!["price-limit rule unavailable".into()],
    )
    .unwrap();
    let provider = Arc::new(OfflineMetadata::new(Ok(batch)));
    let source = security_metadata_source(ProviderId::Tdx, Arc::clone(&provider), unexpected_error);

    let actual = source.fetch(&requested).unwrap();
    assert_eq!(source.provider_id(), ProviderId::Tdx);
    assert_eq!(*provider.requests.lock().unwrap(), vec![requested.to_vec()]);
    assert_eq!(actual.records().len(), 2);
    assert_eq!(
        actual.records()[0].instrument().asset_class(),
        AssetClass::Fund
    );
    assert_eq!(
        actual.records()[1].instrument().asset_class(),
        AssetClass::Equity
    );
    for record in actual.records() {
        assert_eq!(record.instrument().exchange(), Exchange::Shanghai);
        assert_eq!(record.instrument().code(), "600519");
        assert_eq!(record.name(), Some("贵州茅台"));
        assert_eq!(record.board(), Some(Board::Main));
        assert_eq!(record.is_st(), Some(false));
        assert_eq!(record.listed_on(), Some("2001-08-27"));
        assert_eq!(record.status(), DataStatus::Unavailable);
        assert_eq!(record.source_at(), Some(SOURCE));
        assert_eq!(record.observed_at(), OBSERVED);
        assert_eq!(record.provider(), ProviderId::Tdx);
        assert_eq!(record.batch_id(), "master:partial");
    }
    assert_eq!(actual.provenance().source(), "offline-security-master");
    assert_eq!(actual.provenance().source_at(), Some(SOURCE));
    assert_eq!(actual.provenance().fetched_at(), OBSERVED);
    assert_eq!(actual.provenance().batch_id(), Some("master:partial"));
    assert!(!actual.quality().is_complete());
    assert_eq!(actual.quality().issues(), &["price-limit rule unavailable"]);
}

#[test]
fn metadata_adapter_rejects_both_missing_and_extra_records_as_retryable_quality() {
    let requested = [instrument(AssetClass::Equity)];
    let record = metadata(
        AssetClass::Equity,
        ProviderId::Tdx,
        "master:count",
        OBSERVED,
    );
    let extra = metadata(AssetClass::Fund, ProviderId::Tdx, "master:count", OBSERVED);
    for records in [Vec::new(), vec![record, extra]] {
        let provider = Arc::new(OfflineMetadata::new(Ok(DataBatch::strict(
            records,
            provenance("master:count"),
        ))));
        let source =
            security_metadata_source(ProviderId::Tdx, Arc::clone(&provider), unexpected_error);
        let error = source.fetch(&requested).unwrap_err();
        assert_eq!(error.kind(), FailureKind::Quality);
        assert_eq!(error.action(), FailureAction::TryNext);
        assert_eq!(
            error.message(),
            "security metadata response cardinality does not match request"
        );
        assert_eq!(*provider.requests.lock().unwrap(), vec![requested.to_vec()]);
    }
}

#[test]
fn metadata_route_distinguishes_type_failure_batch_rejection_and_time_rejection_before_selection() {
    let requested = [instrument(AssetClass::Equity)];
    let wrong_type = Arc::new(OfflineMetadata::new(Ok(DataBatch::strict(
        vec![metadata(
            AssetClass::Fund,
            ProviderId::Tdx,
            "master:type",
            OBSERVED,
        )],
        provenance("master:type"),
    ))));
    let wrong_batch = Arc::new(OfflineMetadata::new(Ok(DataBatch::strict(
        vec![metadata(
            AssetClass::Equity,
            ProviderId::Tencent,
            "record-only",
            OBSERVED,
        )],
        provenance("master:batch"),
    ))));
    let wrong_time = Arc::new(OfflineMetadata::new(Ok(DataBatch::strict(
        vec![metadata(
            AssetClass::Equity,
            ProviderId::Eastmoney,
            "master:time",
            "2026-10-09T15:00:04+08:00",
        )],
        provenance("master:time"),
    ))));
    let valid = Arc::new(OfflineMetadata::new(Ok(DataBatch::strict(
        vec![metadata(
            AssetClass::Equity,
            ProviderId::Custom,
            "master:valid",
            OBSERVED,
        )],
        provenance("master:valid"),
    ))));
    let policy = AcceptancePolicy::new()
        .with_require_complete(true)
        .with_max_source_age(Duration::from_secs(5))
        .unwrap();
    let mut router = SecurityMetadataRouter::new(policy);
    for (provider_id, provider) in [
        (ProviderId::Tdx, &wrong_type),
        (ProviderId::Tencent, &wrong_batch),
        (ProviderId::Eastmoney, &wrong_time),
        (ProviderId::Custom, &valid),
    ] {
        router
            .register(security_metadata_source(
                provider_id,
                Arc::clone(provider),
                unexpected_error,
            ))
            .unwrap();
    }

    let outcome = router.route(&requested).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Custom);
    assert_eq!(outcome.attempts().len(), 4);
    let expected_statuses = [
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            action: FailureAction::TryNext,
            message: "security metadata record instrument was not requested".into(),
        },
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            message: "record batch ID \"record-only\" does not match provenance batch ID \"master:batch\"".into(),
        },
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            message: "record observed timestamp does not match batch observation timestamp".into(),
        },
        AttemptStatus::Selected,
    ];
    for ((attempt, expected), provider_id) in
        outcome.attempts().iter().zip(&expected_statuses).zip([
            ProviderId::Tdx,
            ProviderId::Tencent,
            ProviderId::Eastmoney,
            ProviderId::Custom,
        ])
    {
        assert_eq!(attempt.provider_id(), provider_id);
        assert_eq!(attempt.status(), expected);
    }
    for provider in [&wrong_type, &wrong_batch, &wrong_time, &valid] {
        assert_eq!(*provider.requests.lock().unwrap(), vec![requested.to_vec()]);
    }
    assert_eq!(outcome.batch().records().len(), 1);
    let selected = &outcome.batch().records()[0];
    assert_eq!(selected.instrument().asset_class(), AssetClass::Equity);
    assert_eq!(selected.name(), Some("贵州茅台"));
    assert_eq!(selected.provider(), ProviderId::Custom);
    assert_eq!(selected.batch_id(), "master:valid");
    assert_eq!(selected.observed_at(), OBSERVED);
    assert_eq!(selected.source_at(), Some(SOURCE));
    assert_eq!(
        outcome.batch().provenance().source(),
        "offline-security-master"
    );
    assert_eq!(
        outcome.batch().provenance().batch_id(),
        Some("master:valid")
    );
    assert_eq!(outcome.batch().provenance().source_at(), Some(SOURCE));
    assert_eq!(outcome.batch().provenance().fetched_at(), OBSERVED);
    assert!(outcome.batch().quality().is_complete());
}

#[test]
fn metadata_provider_error_is_classified_once_and_stop_prevents_fallback() {
    let requested = [instrument(AssetClass::Equity)];
    let failed = Arc::new(OfflineMetadata::new(Err(OfflineError(
        "offline entitlement revoked",
    ))));
    let fallback = Arc::new(OfflineMetadata::new(Ok(DataBatch::strict(
        vec![metadata(
            AssetClass::Equity,
            ProviderId::Custom,
            "unused",
            OBSERVED,
        )],
        provenance("unused"),
    ))));
    let classifications = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&classifications);
    let mut router = SecurityMetadataRouter::new(AcceptancePolicy::new());
    router
        .register(security_metadata_source(
            ProviderId::Tdx,
            Arc::clone(&failed),
            move |error| {
                captured.lock().unwrap().push(error.0);
                SourceError::stop(FailureKind::Unsupported, error.to_string())
            },
        ))
        .unwrap();
    router
        .register(security_metadata_source(
            ProviderId::Custom,
            Arc::clone(&fallback),
            unexpected_error,
        ))
        .unwrap();

    let error = router.route(&requested).unwrap_err();
    assert!(matches!(&error, RouterError::Stopped { .. }));
    assert_eq!(error.attempts().len(), 1);
    assert_eq!(error.attempts()[0].provider_id(), ProviderId::Tdx);
    assert_eq!(
        error.attempts()[0].status(),
        &AttemptStatus::Failed {
            kind: FailureKind::Unsupported,
            action: FailureAction::Stop,
            message: "offline entitlement revoked".into(),
        }
    );
    assert_eq!(
        *classifications.lock().unwrap(),
        vec!["offline entitlement revoked"]
    );
    assert_eq!(*failed.requests.lock().unwrap(), vec![requested.to_vec()]);
    assert!(fallback.requests.lock().unwrap().is_empty());
}

struct OfflineBars {
    batch: DataBatch<Bar>,
    requests: Mutex<Vec<BarsRequest>>,
}

impl HistoricalBars for OfflineBars {
    type Bar = Bar;
    type Error = OfflineError;

    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Bar>, Self::Error> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(self.batch.clone())
    }
}

#[test]
fn bars_adapter_forwards_inclusive_range_and_preserves_nonempty_partial_result() {
    let request = BarsRequest::new(instrument(AssetClass::Equity), BarInterval::Day, 7)
        .unwrap()
        .with_range("2026-10-01", "2026-10-09")
        .unwrap();
    let record = Bar::new(
        instrument(AssetClass::Equity),
        BarInterval::Day,
        "2026-10-09",
        "2026-10-09",
        Price::new(10.0).unwrap(),
        Price::new(12.0).unwrap(),
        Price::new(9.0).unwrap(),
        Price::new(11.0).unwrap(),
        Quantity::new(200.0).unwrap(),
        Some(Money::new(2200.0).unwrap()),
        Adjustment::Unadjusted,
        ProviderId::Tdx,
        "bars:partial",
    )
    .unwrap()
    .with_source_at(SOURCE)
    .unwrap()
    .with_observed_at(OBSERVED)
    .unwrap();
    let provider = Arc::new(OfflineBars {
        batch: DataBatch::best_effort(
            vec![record],
            Provenance::new("offline-bars", OBSERVED)
                .unwrap()
                .with_source_at(SOURCE)
                .unwrap()
                .with_batch_id("bars:partial")
                .unwrap(),
            vec!["older page unavailable".into()],
        )
        .unwrap(),
        requests: Mutex::new(Vec::new()),
    });
    let source = bars_source(ProviderId::Tdx, Arc::clone(&provider), unexpected_error);
    let actual = source.fetch(&request).unwrap();
    assert_eq!(source.provider_id(), ProviderId::Tdx);
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].instrument().code(), "600519");
    assert_eq!(requests[0].instrument().exchange(), Exchange::Shanghai);
    assert_eq!(requests[0].instrument().asset_class(), AssetClass::Equity);
    assert_eq!(requests[0].interval(), BarInterval::Day);
    assert_eq!(requests[0].limit(), 7);
    assert_eq!(requests[0].start(), Some("2026-10-01"));
    assert_eq!(requests[0].end(), Some("2026-10-09"));
    assert_eq!(actual.records().len(), 1);
    let bar = &actual.records()[0];
    assert_eq!(bar.instrument().code(), "600519");
    assert_eq!(bar.instrument().exchange(), Exchange::Shanghai);
    assert_eq!(bar.instrument().asset_class(), AssetClass::Equity);
    assert_eq!(bar.interval(), BarInterval::Day);
    assert_eq!(bar.open().get(), 10.0);
    assert_eq!(bar.high().get(), 12.0);
    assert_eq!(bar.low().get(), 9.0);
    assert_eq!(bar.close().get(), 11.0);
    assert_eq!(bar.volume().get(), 200.0);
    assert_eq!(bar.amount().unwrap().get(), 2200.0);
    assert_eq!(bar.bar_start(), "2026-10-09");
    assert_eq!(bar.bar_end(), "2026-10-09");
    assert_eq!(bar.adjustment(), Adjustment::Unadjusted);
    assert_eq!(bar.source_at(), Some(SOURCE));
    assert_eq!(bar.observed_at(), Some(OBSERVED));
    assert_eq!(bar.provider(), ProviderId::Tdx);
    assert_eq!(bar.batch_id(), "bars:partial");
    assert_eq!(actual.provenance().source(), "offline-bars");
    assert_eq!(actual.provenance().source_at(), Some(SOURCE));
    assert_eq!(actual.provenance().fetched_at(), OBSERVED);
    assert_eq!(actual.provenance().batch_id(), Some("bars:partial"));
    assert!(!actual.quality().is_complete());
    assert_eq!(actual.quality().issues(), &["older page unavailable"]);
}

mod announcement_atomic_range_public_contract {
    use std::convert::Infallible;
    use std::sync::Arc;

    use magic_market_core::{
        Announcement, Announcements, AssetClass, DataBatch, Exchange, HttpsUrl,
        InstrumentDateRangeRequest, InstrumentId, IsoDate, NonEmptyText, PositiveU32, Provenance,
        ProviderId, SourceEvidence,
    };
    use magic_market_router::{
        announcement_source, AcceptancePolicy, AnnouncementRouter, AttemptStatus, FailureAction,
        FailureKind,
    };

    struct ExternalAnnouncementFixture {
        batch: DataBatch<Announcement>,
    }

    impl Announcements for ExternalAnnouncementFixture {
        type Error = Infallible;

        fn announcements(
            &self,
            _request: &InstrumentDateRangeRequest,
        ) -> Result<DataBatch<Announcement>, Self::Error> {
            Ok(self.batch.clone())
        }
    }

    fn requested_issuer() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap()
    }

    fn announcement_batch(
        provider: ProviderId,
        batch_id: &str,
        rows: &[(&str, &str)],
    ) -> DataBatch<Announcement> {
        let records = rows
            .iter()
            .map(|&(id, date)| Announcement {
                announcement_id: NonEmptyText::new(id).unwrap(),
                instrument: requested_issuer(),
                instrument_name: None,
                category: None,
                title: NonEmptyText::new("source disclosure label").unwrap(),
                published_at: NonEmptyText::new(date).unwrap(),
                canonical_url: HttpsUrl::new("https://example.com/disclosure").unwrap(),
                pdf_url: None,
                evidence: SourceEvidence::new(provider, "2026-07-24T10:00:00+08:00", batch_id)
                    .unwrap()
                    .with_source_at(date)
                    .unwrap(),
            })
            .collect();
        DataBatch::strict(
            records,
            Provenance::new("external-announcement-fixture", "2026-07-24T10:00:00+08:00")
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        )
    }

    #[test]
    fn announcement_route_rejects_a_late_row_atomically_and_accepts_the_date_only_boundary() {
        let request =
            InstrumentDateRangeRequest::new(requested_issuer(), PositiveU32::new(2).unwrap())
                .unwrap()
                .with_range(
                    IsoDate::new("2026-07-23").unwrap(),
                    IsoDate::new("2026-07-23").unwrap(),
                )
                .unwrap();
        let mut router = AnnouncementRouter::new(AcceptancePolicy::new());
        router
            .register(announcement_source(
                ProviderId::Sse,
                Arc::new(ExternalAnnouncementFixture {
                    batch: announcement_batch(
                        ProviderId::Sse,
                        "sse-invalid-window",
                        &[
                            ("valid-prefix", "2026-07-23"),
                            ("one-day-after-through", "2026-07-24"),
                        ],
                    ),
                }),
                |never: Infallible| match never {},
            ))
            .unwrap();
        router
            .register(announcement_source(
                ProviderId::Cninfo,
                Arc::new(ExternalAnnouncementFixture {
                    batch: announcement_batch(
                        ProviderId::Cninfo,
                        "cninfo-exact-day",
                        &[("accepted-boundary", "2026-07-23")],
                    ),
                }),
                |never: Infallible| match never {},
            ))
            .unwrap();

        let outcome = router.route(&request).unwrap();
        assert_eq!(outcome.selected_provider(), ProviderId::Cninfo);
        assert_eq!(outcome.attempts().len(), 2);
        assert_eq!(outcome.attempts()[0].provider_id(), ProviderId::Sse);
        assert!(matches!(
            outcome.attempts()[0].status(),
            AttemptStatus::Failed {
                kind: FailureKind::Evidence,
                action: FailureAction::TryNext,
                ..
            }
        ));
        assert_eq!(outcome.attempts()[1].provider_id(), ProviderId::Cninfo);
        assert_eq!(outcome.attempts()[1].status(), &AttemptStatus::Selected);

        let batch = outcome.batch();
        assert!(batch.quality().is_complete());
        assert_eq!(batch.records().len(), 1);
        assert_eq!(batch.provenance().batch_id(), Some("cninfo-exact-day"));
        assert_eq!(batch.provenance().source_at(), None);
        let record = &batch.records()[0];
        assert_eq!(record.announcement_id.as_str(), "accepted-boundary");
        assert_eq!(record.instrument.exchange(), Exchange::Shanghai);
        assert_eq!(record.instrument.code(), "600519");
        assert_eq!(record.instrument.asset_class(), AssetClass::Equity);
        assert_eq!(record.published_at.as_str(), "2026-07-23");
        assert_eq!(record.evidence.provider(), ProviderId::Cninfo);
        assert_eq!(record.evidence.batch_id(), "cninfo-exact-day");
        assert_eq!(record.evidence.source_at(), Some("2026-07-23"));
        assert_eq!(record.evidence.observed_at(), "2026-07-24T10:00:00+08:00");
    }
}
