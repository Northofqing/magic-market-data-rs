use super::*;
use magic_market_core::{
    AssetClass, Exchange, FiniteNumber, InstrumentId, IsoDate, MarketRankingKind,
    MarketRankingUnit, PositiveU32, Provenance, SourceEvidence, VerifiedEmpty,
};
use magic_market_router::{AttemptStatus, FailureAction};
use std::sync::atomic::{AtomicUsize, Ordering};

const DATE: &str = "2026-07-29";
const OBSERVED_AT: &str = "2026-07-29T15:35:01+08:00";
const BATCH_ID: &str = "composition-provider-topn-fixture";
const SOURCE: &str = "fixture-provider-topn";
const FILTER: &str = "fixture-filter";

#[derive(Debug, Clone, thiserror::Error)]
#[error("fixture provider failure")]
struct FixtureError;

#[derive(Clone)]
struct FixtureProvider {
    batch: DataBatch<ProviderTopNRankingEntry>,
}

impl ProviderTopNRankings for FixtureProvider {
    type Error = FixtureError;

    fn provider_top_n_rankings(
        &self,
        _request: &ProviderTopNRankingRequest,
    ) -> Result<DataBatch<ProviderTopNRankingEntry>, Self::Error> {
        Ok(self.batch.clone())
    }
}

fn request_for(kind: MarketRankingKind) -> ProviderTopNRankingRequest {
    ProviderTopNRankingRequest::new(
        kind,
        IsoDate::new(DATE).unwrap(),
        PositiveU32::new(1).unwrap(),
        NonEmptyText::new(FILTER).unwrap(),
    )
    .unwrap()
}

fn request() -> ProviderTopNRankingRequest {
    request_for(MarketRankingKind::VolumeRatio)
}

fn batch(source: &str) -> DataBatch<ProviderTopNRankingEntry> {
    let record = ProviderTopNRankingEntry::new(
        MarketRankingKind::VolumeRatio,
        PositiveU32::new(1).unwrap(),
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
        NonEmptyText::new("华电辽能").unwrap(),
        FiniteNumber::new(2.5).unwrap(),
        MarketRankingUnit::Multiple,
        IsoDate::new(DATE).unwrap(),
        NonEmptyText::new(FILTER).unwrap(),
        PositiveU32::new(5_542).unwrap(),
        PositiveU32::new(1).unwrap(),
        SourceEvidence::new(ProviderId::Eastmoney, OBSERVED_AT, BATCH_ID).unwrap(),
    )
    .unwrap();
    DataBatch::strict(
        vec![record],
        Provenance::new(source, OBSERVED_AT)
            .unwrap()
            .with_batch_id(BATCH_ID)
            .unwrap(),
    )
}

fn source(
    batch: DataBatch<ProviderTopNRankingEntry>,
) -> Result<ComposedProviderTopNSource, RouterError> {
    build_source(
        ProviderId::Eastmoney,
        NonEmptyText::new(SOURCE).unwrap(),
        ProviderTopNRankingCapabilities {
            volume_ratio: true,
            main_net_inflow: false,
        },
        Arc::new(FixtureProvider { batch }),
        |_| SourceError::try_next(FailureKind::Transport, "fixture failure"),
    )
}

fn router(
    source: ComposedProviderTopNSource,
    clock: ChinaDateClock,
) -> EastmoneyProviderTopNRankingRouter {
    EastmoneyProviderTopNRankingRouter::with_source_and_clock(source, clock).unwrap()
}

#[test]
fn route_selects_only_a_revalidated_current_date_batch() {
    let router = router(source(batch(SOURCE)).unwrap(), Arc::new(|| Ok(DATE.into())));
    let outcome = router.route(&request()).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert_eq!(outcome.batch().records().len(), 1);
    assert_eq!(outcome.attempts()[0].status(), &AttemptStatus::Selected);
}

#[test]
fn debug_and_accessors_preserve_the_concrete_route_identity() {
    let source = source(batch(SOURCE)).unwrap();
    let source_debug = format!("{source:?}");
    assert!(source_debug.contains("ComposedProviderTopNSource"));
    assert!(source_debug.contains("Eastmoney"));
    assert!(source_debug.contains(SOURCE));
    assert_eq!(source.provider_id(), ProviderId::Eastmoney);
    assert_eq!(source.fetch(&request()).unwrap().records().len(), 1);

    let router = router(source, Arc::new(|| Ok(DATE.into())));
    let router_debug = format!("{router:?}");
    assert!(router_debug.contains("EastmoneyProviderTopNRankingRouter"));
    assert!(router_debug.contains("<injected>"));
    assert_eq!(router.provider_ids(), [ProviderId::Eastmoney]);
    assert_eq!(
        router.capabilities(),
        ProviderTopNRankingCapabilities {
            volume_ratio: true,
            main_net_inflow: false,
        }
    );
    assert_eq!(router.expected_source().as_str(), SOURCE);
    assert!(router.policy().require_complete());
    assert!(!router.policy().accept_complete_empty());
}

#[test]
fn production_source_owns_eastmoney_identity_and_capabilities() {
    let provider = Arc::new(EastmoneyClient::new().unwrap());
    let source = eastmoney_source(provider).unwrap();

    assert_eq!(source.provider_id(), ProviderId::Eastmoney);
    assert_eq!(source.expected_source.as_str(), "eastmoney-web");
    assert_eq!(
        source.capabilities,
        ProviderTopNRankingCapabilities {
            volume_ratio: true,
            main_net_inflow: true,
        }
    );
}

#[test]
fn route_rejects_a_provider_batch_with_wrong_source_identity() {
    let router = router(
        source(batch("forged-source")).unwrap(),
        Arc::new(|| Ok(DATE.into())),
    );
    let error = router.route(&request()).unwrap_err();
    assert!(matches!(
        error,
        EastmoneyProviderTopNRouterError::Routing(RouterError::Exhausted { .. })
    ));
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn route_admits_a_past_settled_date_after_a_midnight_rollover() {
    let date = Arc::new(std::sync::Mutex::new(DATE.to_owned()));
    let date_for_clock = Arc::clone(&date);
    let router = router(
        source(batch(SOURCE)).unwrap(),
        Arc::new(move || Ok(date_for_clock.lock().unwrap().clone())),
    );

    router.route(&request()).unwrap();
    *date.lock().unwrap() = "2026-07-30".into();

    let outcome = router.route(&request()).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
}

#[test]
fn route_rejects_a_future_request_before_provider_io() {
    let calls = Arc::new(AtomicUsize::new(0));

    #[derive(Clone)]
    struct CountingProvider {
        calls: Arc<AtomicUsize>,
    }

    impl ProviderTopNRankings for CountingProvider {
        type Error = FixtureError;

        fn provider_top_n_rankings(
            &self,
            _request: &ProviderTopNRankingRequest,
        ) -> Result<DataBatch<ProviderTopNRankingEntry>, Self::Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(batch(SOURCE))
        }
    }

    let source = build_source(
        ProviderId::Eastmoney,
        NonEmptyText::new(SOURCE).unwrap(),
        ProviderTopNRankingCapabilities {
            volume_ratio: true,
            main_net_inflow: false,
        },
        Arc::new(CountingProvider {
            calls: Arc::clone(&calls),
        }),
        |_| SourceError::try_next(FailureKind::Transport, "fixture failure"),
    )
    .unwrap();
    let router = router(source, Arc::new(|| Ok("2026-07-28".into())));

    let error = router.route(&request()).unwrap_err();
    assert!(matches!(
        error,
        EastmoneyProviderTopNRouterError::RejectedRequest(_)
    ));
    assert!(error.attempts().is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn route_rejects_an_unadmitted_metric_without_provider_io() {
    #[derive(Clone)]
    struct CountingProvider {
        calls: Arc<AtomicUsize>,
    }

    impl ProviderTopNRankings for CountingProvider {
        type Error = FixtureError;

        fn provider_top_n_rankings(
            &self,
            _request: &ProviderTopNRankingRequest,
        ) -> Result<DataBatch<ProviderTopNRankingEntry>, Self::Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(batch(SOURCE))
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let source = build_source(
        ProviderId::Eastmoney,
        NonEmptyText::new(SOURCE).unwrap(),
        ProviderTopNRankingCapabilities {
            volume_ratio: true,
            main_net_inflow: false,
        },
        Arc::new(CountingProvider {
            calls: Arc::clone(&calls),
        }),
        |_| SourceError::try_next(FailureKind::Transport, "fixture failure"),
    )
    .unwrap();
    let router = router(source, Arc::new(|| Ok(DATE.into())));

    let error = router
        .route(&request_for(MarketRankingKind::MainNetInflow))
        .unwrap_err();
    assert!(matches!(
        error,
        EastmoneyProviderTopNRouterError::Routing(RouterError::Exhausted { .. })
    ));
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Unsupported,
            ..
        }
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn provider_failure_is_classified_before_batch_validation() {
    #[derive(Clone)]
    struct FailingProvider;

    impl ProviderTopNRankings for FailingProvider {
        type Error = FixtureError;

        fn provider_top_n_rankings(
            &self,
            _request: &ProviderTopNRankingRequest,
        ) -> Result<DataBatch<ProviderTopNRankingEntry>, Self::Error> {
            Err(FixtureError)
        }
    }

    let source = build_source(
        ProviderId::Eastmoney,
        NonEmptyText::new(SOURCE).unwrap(),
        ProviderTopNRankingCapabilities {
            volume_ratio: true,
            main_net_inflow: false,
        },
        Arc::new(FailingProvider),
        |_| SourceError::try_next(FailureKind::Transport, "classified provider failure"),
    )
    .unwrap();
    let router = router(source, Arc::new(|| Ok(DATE.into())));

    let error = router.route(&request()).unwrap_err();
    assert!(matches!(
        error,
        EastmoneyProviderTopNRouterError::Routing(RouterError::Exhausted { .. })
    ));
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Transport,
            ..
        }
    ));
}

#[test]
fn clock_and_unadmitted_capability_fail_explicitly() {
    let router = router(
        source(batch(SOURCE)).unwrap(),
        Arc::new(|| Err("clock unavailable".into())),
    );
    assert!(matches!(
        router.route(&request()).unwrap_err(),
        EastmoneyProviderTopNRouterError::Clock(_)
    ));

    let error = build_source(
        ProviderId::Eastmoney,
        NonEmptyText::new(SOURCE).unwrap(),
        ProviderTopNRankingCapabilities::default(),
        Arc::new(FixtureProvider {
            batch: batch(SOURCE),
        }),
        |_| SourceError::try_next(FailureKind::Transport, "fixture failure"),
    )
    .unwrap_err();
    assert!(matches!(error, RouterError::InvalidConfiguration(_)));
}

#[test]
fn current_china_date_matches_the_observed_plus_eight_calendar_date() {
    let china_offset = UtcOffset::from_hms(8, 0, 0).unwrap();
    let before = OffsetDateTime::now_utc()
        .to_offset(china_offset)
        .date()
        .to_string();
    let actual = current_china_date().unwrap();
    let after = OffsetDateTime::now_utc()
        .to_offset(china_offset)
        .date()
        .to_string();

    assert!(actual == before || actual == after);
    IsoDate::new(actual).unwrap();
}

#[test]
fn eastmoney_error_classification_covers_every_public_variant() {
    let cases = [
        (
            EastmoneyError::InvalidRequest("bad request".into()),
            FailureKind::InvalidRequest,
            FailureAction::Stop,
        ),
        (
            EastmoneyError::Unsupported("unsupported".into()),
            FailureKind::Unsupported,
            FailureAction::TryNext,
        ),
        (
            EastmoneyError::Transport("transport".into()),
            FailureKind::Transport,
            FailureAction::TryNext,
        ),
        (
            EastmoneyError::ResponseTooLarge { limit: 42 },
            FailureKind::Protocol,
            FailureAction::TryNext,
        ),
        (
            EastmoneyError::Decode("decode".into()),
            FailureKind::Protocol,
            FailureAction::TryNext,
        ),
        (
            EastmoneyError::Protocol("protocol".into()),
            FailureKind::Protocol,
            FailureAction::TryNext,
        ),
        (
            EastmoneyError::Core(NonEmptyText::new("").unwrap_err()),
            FailureKind::Evidence,
            FailureAction::TryNext,
        ),
    ];
    for (error, expected_kind, expected_action) in cases {
        let classified = classify_eastmoney_error(error);
        assert_eq!(classified.kind(), expected_kind);
        assert_eq!(classified.action(), expected_action);
        assert!(!classified.message().is_empty());
    }

    let evidence = SourceEvidence::new(ProviderId::Eastmoney, OBSERVED_AT, BATCH_ID).unwrap();
    let provenance = Provenance::new(SOURCE, OBSERVED_AT)
        .unwrap()
        .with_batch_id(BATCH_ID)
        .unwrap();
    let verified_empty =
        VerifiedEmpty::new("provider_top_n", FILTER, "no rows", evidence, provenance).unwrap();
    let classified =
        classify_eastmoney_error(EastmoneyError::VerifiedEmpty(Box::new(verified_empty)));
    assert_eq!(classified.kind(), FailureKind::NoData);
    assert_eq!(classified.action(), FailureAction::TryNext);
}
