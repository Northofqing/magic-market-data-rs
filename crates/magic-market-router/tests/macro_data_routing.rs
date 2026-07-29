use magic_market_core::{
    CurrencyCode, DataBatch, EconomicObservation, EconomicObservationStatus, EconomicPeriod,
    EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, FiniteNumber, IsoDate,
    OfficialFxFixing, OfficialFxFixingIdentity, OfficialFxFixingProvider, OfficialFxFixingRequest,
    PositiveU32, Provenance, ProviderId, RatioUnit, ReferenceRateIdentity, ReferenceRateKind,
    ReferenceRateObservation, ReferenceRateProvider, ReferenceRateRequest, ReferenceTenor,
    SourceEvidence,
};
use magic_market_router::{
    economic_series_source, official_fx_fixing_source, reference_rate_source, AcceptancePolicy,
    AttemptStatus, EconomicSeriesRouter, FailureAction, FailureKind, OfficialFxFixingRouter,
    ReferenceRateRouter, SourceError,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
#[error("fixture")]
struct FixtureError;

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Transport, "fixture transport")
}

fn classify_stop(_: FixtureError) -> SourceError {
    SourceError::stop(FailureKind::InvalidRequest, "fixture terminal")
}

fn batch<T>(records: Vec<T>, source: &str, batch_id: &str) -> DataBatch<T> {
    DataBatch::strict(
        records,
        Provenance::new(source, "observed")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    )
}

struct EconomicFixture {
    result: Result<DataBatch<EconomicObservation>, FixtureError>,
    calls: Arc<AtomicUsize>,
}

impl EconomicSeriesProvider for EconomicFixture {
    type Error = FixtureError;

    fn economic_series(
        &self,
        _request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, Self::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}

fn economic_key(provider: ProviderId, code: &str) -> EconomicSeriesKey {
    EconomicSeriesKey::new(provider, "scope", code).unwrap()
}

fn economic_request(provider: ProviderId, codes: &[&str], max_rows: u32) -> EconomicSeriesRequest {
    EconomicSeriesRequest::new(
        codes
            .iter()
            .map(|code| economic_key(provider, code))
            .collect(),
        EconomicPeriod::month(2026, 1).unwrap(),
        EconomicPeriod::month(2026, 2).unwrap(),
        PositiveU32::new(max_rows).unwrap(),
    )
    .unwrap()
}

fn economic_record(
    provider: ProviderId,
    code: &str,
    period: EconomicPeriod,
    region: Option<&str>,
    batch_id: &str,
) -> EconomicObservation {
    economic_record_with_region(provider, code, period, region, region, batch_id)
}

fn economic_record_with_region(
    provider: ProviderId,
    code: &str,
    period: EconomicPeriod,
    region_code: Option<&str>,
    region_name: Option<&str>,
    batch_id: &str,
) -> EconomicObservation {
    EconomicObservation::new(
        economic_key(provider, code),
        "fixture series",
        region_code.map(|value| magic_market_core::NonEmptyText::new(value).unwrap()),
        region_name.map(|value| magic_market_core::NonEmptyText::new(value).unwrap()),
        period,
        Some(FiniteNumber::new(1.0).unwrap()),
        "index",
        None,
        None,
        EconomicObservationStatus::Present,
        None,
        None,
        SourceEvidence::new(provider, "observed", batch_id).unwrap(),
    )
    .unwrap()
}

fn economic_error(
    request: &EconomicSeriesRequest,
    records: Vec<EconomicObservation>,
) -> (FailureKind, FailureAction) {
    let mut router = EconomicSeriesRouter::new(AcceptancePolicy::new());
    router
        .register(economic_series_source(
            request.provider(),
            Arc::new(EconomicFixture {
                result: Ok(batch(records, "economic", "batch")),
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            classify,
        ))
        .unwrap();
    match router.route(request).unwrap_err().attempts()[0].status() {
        AttemptStatus::Failed { kind, action, .. } => (*kind, *action),
        status => panic!("expected family validation failure, got {status:?}"),
    }
}

#[test]
fn economic_router_accepts_subset_in_canonical_order() {
    let request = economic_request(ProviderId::Fred, &["A", "B"], 10);
    let records = vec![
        economic_record(
            ProviderId::Fred,
            "A",
            EconomicPeriod::month(2026, 1).unwrap(),
            None,
            "batch",
        ),
        economic_record(
            ProviderId::Fred,
            "A",
            EconomicPeriod::month(2026, 2).unwrap(),
            None,
            "batch",
        ),
        economic_record(
            ProviderId::Fred,
            "B",
            EconomicPeriod::month(2026, 1).unwrap(),
            Some("US"),
            "batch",
        ),
    ];
    let mut router = EconomicSeriesRouter::new(AcceptancePolicy::new());
    router
        .register(economic_series_source(
            ProviderId::Fred,
            Arc::new(EconomicFixture {
                result: Ok(batch(records, "economic", "batch")),
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            classify,
        ))
        .unwrap();
    assert_eq!(router.route(&request).unwrap().batch().records().len(), 3);
}

#[test]
fn economic_router_classifies_identity_range_frequency_cardinality_duplicate_and_order() {
    let request = economic_request(ProviderId::Fred, &["A"], 1);
    let january = EconomicPeriod::month(2026, 1).unwrap();
    let february = EconomicPeriod::month(2026, 2).unwrap();

    assert_eq!(
        economic_error(
            &request,
            vec![economic_record(
                ProviderId::Fred,
                "B",
                january.clone(),
                None,
                "batch"
            )]
        )
        .0,
        FailureKind::Evidence
    );
    assert_eq!(
        economic_error(
            &request,
            vec![economic_record(
                ProviderId::Fred,
                "A",
                EconomicPeriod::month(2025, 12).unwrap(),
                None,
                "batch"
            )]
        )
        .0,
        FailureKind::Evidence
    );
    assert_eq!(
        economic_error(
            &request,
            vec![economic_record(
                ProviderId::Fred,
                "A",
                EconomicPeriod::year(2026).unwrap(),
                None,
                "batch"
            )]
        )
        .0,
        FailureKind::Evidence
    );
    assert_eq!(
        economic_error(
            &request,
            vec![
                economic_record(ProviderId::Fred, "A", january.clone(), None, "batch"),
                economic_record(ProviderId::Fred, "A", february.clone(), None, "batch"),
            ]
        )
        .0,
        FailureKind::Quality
    );

    let roomy = economic_request(ProviderId::Fred, &["A"], 3);
    let duplicate = economic_record(ProviderId::Fred, "A", january.clone(), None, "batch");
    assert_eq!(
        economic_error(&roomy, vec![duplicate.clone(), duplicate]).0,
        FailureKind::Quality
    );
    assert_eq!(
        economic_error(
            &roomy,
            vec![
                economic_record(ProviderId::Fred, "A", february, None, "batch"),
                economic_record(ProviderId::Fred, "A", january, None, "batch"),
            ]
        )
        .0,
        FailureKind::Quality
    );

    let multi_series = economic_request(ProviderId::Fred, &["A", "B"], 3);
    assert_eq!(
        economic_error(
            &multi_series,
            vec![
                economic_record(
                    ProviderId::Fred,
                    "B",
                    EconomicPeriod::month(2026, 1).unwrap(),
                    None,
                    "batch",
                ),
                economic_record(
                    ProviderId::Fred,
                    "A",
                    EconomicPeriod::month(2026, 1).unwrap(),
                    None,
                    "batch",
                ),
            ],
        )
        .0,
        FailureKind::Quality
    );
    assert_eq!(
        economic_error(
            &roomy,
            vec![
                economic_record_with_region(
                    ProviderId::Fred,
                    "A",
                    EconomicPeriod::month(2026, 1).unwrap(),
                    Some("US-Z"),
                    Some("Zulu"),
                    "batch",
                ),
                economic_record_with_region(
                    ProviderId::Fred,
                    "A",
                    EconomicPeriod::month(2026, 1).unwrap(),
                    Some("US-A"),
                    Some("Alpha"),
                    "batch",
                ),
            ],
        )
        .0,
        FailureKind::Quality
    );
    assert_eq!(
        economic_error(
            &roomy,
            vec![
                economic_record_with_region(
                    ProviderId::Fred,
                    "A",
                    EconomicPeriod::month(2026, 1).unwrap(),
                    Some("US"),
                    Some("Zulu"),
                    "batch",
                ),
                economic_record_with_region(
                    ProviderId::Fred,
                    "A",
                    EconomicPeriod::month(2026, 2).unwrap(),
                    Some("US"),
                    Some("Alpha"),
                    "batch",
                ),
            ],
        )
        .0,
        FailureKind::Quality
    );
}

#[test]
fn economic_router_leaves_record_and_batch_evidence_to_generic_rejection() {
    let request = economic_request(ProviderId::Fred, &["A"], 2);
    for (record_provider, record_batch) in [(ProviderId::Imf, "batch"), (ProviderId::Fred, "other")]
    {
        let record = economic_record(
            record_provider,
            "A",
            EconomicPeriod::month(2026, 1).unwrap(),
            None,
            record_batch,
        );
        let mut router = EconomicSeriesRouter::new(AcceptancePolicy::new());
        router
            .register(economic_series_source(
                ProviderId::Fred,
                Arc::new(EconomicFixture {
                    result: Ok(batch(vec![record], "economic", "batch")),
                    calls: Arc::new(AtomicUsize::new(0)),
                }),
                classify,
            ))
            .unwrap();
        assert!(matches!(
            router.route(&request).unwrap_err().attempts()[0].status(),
            AttemptStatus::Rejected {
                kind: FailureKind::Evidence,
                ..
            }
        ));
    }
}

#[test]
fn economic_router_never_calls_a_source_for_another_provider_namespace() {
    let request = economic_request(ProviderId::Fred, &["A"], 1);
    let calls = Arc::new(AtomicUsize::new(0));
    let mut router = EconomicSeriesRouter::new(AcceptancePolicy::new());
    router
        .register(economic_series_source(
            ProviderId::Imf,
            Arc::new(EconomicFixture {
                result: Err(FixtureError),
                calls: Arc::clone(&calls),
            }),
            classify,
        ))
        .unwrap();
    let error = router.route(&request).unwrap_err();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            action: FailureAction::TryNext,
            ..
        }
    ));
}

#[test]
fn economic_router_preserves_recoverable_and_terminal_error_actions() {
    let request = economic_request(ProviderId::Fred, &["A"], 1);
    let calls = Arc::new(AtomicUsize::new(0));
    let blocked_calls = Arc::new(AtomicUsize::new(0));
    let mut recoverable = EconomicSeriesRouter::new(AcceptancePolicy::new());
    recoverable
        .register(economic_series_source(
            ProviderId::Fred,
            Arc::new(EconomicFixture {
                result: Err(FixtureError),
                calls: Arc::clone(&calls),
            }),
            classify,
        ))
        .unwrap();
    recoverable
        .register(economic_series_source(
            ProviderId::Imf,
            Arc::new(EconomicFixture {
                result: Err(FixtureError),
                calls: Arc::clone(&blocked_calls),
            }),
            classify,
        ))
        .unwrap();
    let error = recoverable.route(&request).unwrap_err();
    assert_eq!(error.attempts().len(), 2);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(blocked_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Transport,
            action: FailureAction::TryNext,
            ..
        }
    ));

    let terminal_calls = Arc::new(AtomicUsize::new(0));
    let never_calls = Arc::new(AtomicUsize::new(0));
    let mut terminal = EconomicSeriesRouter::new(AcceptancePolicy::new());
    terminal
        .register(economic_series_source(
            ProviderId::Fred,
            Arc::new(EconomicFixture {
                result: Err(FixtureError),
                calls: Arc::clone(&terminal_calls),
            }),
            classify_stop,
        ))
        .unwrap();
    terminal
        .register(economic_series_source(
            ProviderId::Imf,
            Arc::new(EconomicFixture {
                result: Err(FixtureError),
                calls: Arc::clone(&never_calls),
            }),
            classify,
        ))
        .unwrap();
    let error = terminal.route(&request).unwrap_err();
    assert_eq!(error.attempts().len(), 1);
    assert_eq!(terminal_calls.load(Ordering::SeqCst), 1);
    assert_eq!(never_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        error.attempts()[0].status(),
        AttemptStatus::Failed {
            action: FailureAction::Stop,
            ..
        }
    ));
}

struct RateFixture(Vec<ReferenceRateObservation>);

impl ReferenceRateProvider for RateFixture {
    type Error = FixtureError;

    fn reference_rates(
        &self,
        _request: &ReferenceRateRequest,
    ) -> Result<DataBatch<ReferenceRateObservation>, Self::Error> {
        Ok(batch(self.0.clone(), "rates", "batch"))
    }
}

fn rate_identity(provider: ProviderId, tenor: ReferenceTenor) -> ReferenceRateIdentity {
    ReferenceRateIdentity::new(provider, ReferenceRateKind::Shibor(tenor)).unwrap()
}

fn rate_record(
    provider: ProviderId,
    tenor: ReferenceTenor,
    date: &str,
) -> ReferenceRateObservation {
    ReferenceRateObservation::new(
        rate_identity(provider, tenor),
        IsoDate::new(date).unwrap(),
        FiniteNumber::new(1.5).unwrap(),
        RatioUnit::Percent,
        None,
        None,
        SourceEvidence::new(provider, "observed", "batch").unwrap(),
    )
    .unwrap()
}

fn rate_error(
    request: &ReferenceRateRequest,
    records: Vec<ReferenceRateObservation>,
) -> (FailureKind, FailureAction) {
    let mut router = ReferenceRateRouter::new(AcceptancePolicy::new());
    router
        .register(reference_rate_source(
            request.provider(),
            Arc::new(RateFixture(records)),
            classify,
        ))
        .unwrap();
    match router.route(request).unwrap_err().attempts()[0].status() {
        AttemptStatus::Failed { kind, action, .. } => (*kind, *action),
        status => panic!("expected rate validation failure, got {status:?}"),
    }
}

#[test]
fn reference_rate_router_covers_identity_range_cardinality_duplicate_order_and_provider_evidence() {
    let overnight = rate_identity(ProviderId::Cfets, ReferenceTenor::Overnight);
    let one_week = rate_identity(ProviderId::Cfets, ReferenceTenor::OneWeek);
    let request = ReferenceRateRequest::new(
        vec![overnight, one_week],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(4).unwrap(),
    )
    .unwrap();
    let valid = vec![
        rate_record(ProviderId::Cfets, ReferenceTenor::Overnight, "2026-07-28"),
        rate_record(ProviderId::Cfets, ReferenceTenor::OneWeek, "2026-07-29"),
    ];
    let mut router = ReferenceRateRouter::new(AcceptancePolicy::new());
    router
        .register(reference_rate_source(
            ProviderId::Cfets,
            Arc::new(RateFixture(valid)),
            classify,
        ))
        .unwrap();
    assert!(router.route(&request).is_ok());

    assert_eq!(
        rate_error(
            &request,
            vec![rate_record(
                ProviderId::Cfets,
                ReferenceTenor::OneMonth,
                "2026-07-29"
            )]
        )
        .0,
        FailureKind::Evidence
    );
    assert_eq!(
        rate_error(
            &request,
            vec![rate_record(
                ProviderId::Cfets,
                ReferenceTenor::Overnight,
                "2026-07-27"
            )]
        )
        .0,
        FailureKind::Evidence
    );
    let same = rate_record(ProviderId::Cfets, ReferenceTenor::Overnight, "2026-07-28");
    assert_eq!(
        rate_error(&request, vec![same.clone(), same]).0,
        FailureKind::Quality
    );
    assert_eq!(
        rate_error(
            &request,
            vec![
                rate_record(ProviderId::Cfets, ReferenceTenor::OneWeek, "2026-07-29"),
                rate_record(ProviderId::Cfets, ReferenceTenor::Overnight, "2026-07-28"),
            ]
        )
        .0,
        FailureKind::Quality
    );
    assert_eq!(
        rate_error(
            &request,
            vec![
                rate_record(ProviderId::Cfets, ReferenceTenor::Overnight, "2026-07-29"),
                rate_record(ProviderId::Cfets, ReferenceTenor::Overnight, "2026-07-28"),
            ],
        )
        .0,
        FailureKind::Quality
    );
    let tight = ReferenceRateRequest::new(
        vec![rate_identity(ProviderId::Cfets, ReferenceTenor::Overnight)],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        rate_error(
            &tight,
            vec![
                rate_record(ProviderId::Cfets, ReferenceTenor::Overnight, "2026-07-28"),
                rate_record(ProviderId::Cfets, ReferenceTenor::Overnight, "2026-07-29"),
            ]
        )
        .0,
        FailureKind::Quality
    );

    let wrong_provider = rate_record(ProviderId::Fred, ReferenceTenor::Overnight, "2026-07-28");
    let mut generic = ReferenceRateRouter::new(AcceptancePolicy::new());
    generic
        .register(reference_rate_source(
            ProviderId::Cfets,
            Arc::new(RateFixture(vec![wrong_provider])),
            classify,
        ))
        .unwrap();
    assert!(matches!(
        generic.route(&request).unwrap_err().attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

struct FixingFixture(Vec<OfficialFxFixing>);

impl OfficialFxFixingProvider for FixingFixture {
    type Error = FixtureError;

    fn official_fx_fixings(
        &self,
        _request: &OfficialFxFixingRequest,
    ) -> Result<DataBatch<OfficialFxFixing>, Self::Error> {
        Ok(batch(self.0.clone(), "fixings", "batch"))
    }
}

fn fixing_identity(provider: ProviderId, base: &str) -> OfficialFxFixingIdentity {
    OfficialFxFixingIdentity::new(
        provider,
        CurrencyCode::new(base).unwrap(),
        CurrencyCode::new("CNY").unwrap(),
    )
    .unwrap()
}

fn fixing_record(provider: ProviderId, base: &str, date: &str) -> OfficialFxFixing {
    OfficialFxFixing::new(
        CurrencyCode::new(base).unwrap(),
        CurrencyCode::new("CNY").unwrap(),
        IsoDate::new(date).unwrap(),
        FiniteNumber::new(6.8).unwrap(),
        PositiveU32::new(1).unwrap(),
        None,
        None,
        SourceEvidence::new(provider, "observed", "batch").unwrap(),
    )
    .unwrap()
}

fn fixing_error(request: &OfficialFxFixingRequest, records: Vec<OfficialFxFixing>) -> FailureKind {
    let mut router = OfficialFxFixingRouter::new(AcceptancePolicy::new());
    router
        .register(official_fx_fixing_source(
            request.provider(),
            Arc::new(FixingFixture(records)),
            classify,
        ))
        .unwrap();
    match router.route(request).unwrap_err().attempts()[0].status() {
        AttemptStatus::Failed { kind, .. } => *kind,
        status => panic!("expected fixing validation failure, got {status:?}"),
    }
}

#[test]
fn official_fixing_router_covers_identity_range_cardinality_duplicate_order_and_provider_evidence()
{
    let request = OfficialFxFixingRequest::new(
        vec![
            fixing_identity(ProviderId::Cfets, "USD"),
            fixing_identity(ProviderId::Cfets, "EUR"),
        ],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(4).unwrap(),
    )
    .unwrap();
    let valid = vec![
        fixing_record(ProviderId::Cfets, "USD", "2026-07-28"),
        fixing_record(ProviderId::Cfets, "EUR", "2026-07-29"),
    ];
    let mut router = OfficialFxFixingRouter::new(AcceptancePolicy::new());
    router
        .register(official_fx_fixing_source(
            ProviderId::Cfets,
            Arc::new(FixingFixture(valid)),
            classify,
        ))
        .unwrap();
    assert!(router.route(&request).is_ok());

    assert_eq!(
        fixing_error(
            &request,
            vec![fixing_record(ProviderId::Cfets, "JPY", "2026-07-29")]
        ),
        FailureKind::Evidence
    );
    assert_eq!(
        fixing_error(
            &request,
            vec![fixing_record(ProviderId::Cfets, "USD", "2026-07-27")]
        ),
        FailureKind::Evidence
    );
    let same = fixing_record(ProviderId::Cfets, "USD", "2026-07-28");
    assert_eq!(
        fixing_error(&request, vec![same.clone(), same]),
        FailureKind::Quality
    );
    assert_eq!(
        fixing_error(
            &request,
            vec![
                fixing_record(ProviderId::Cfets, "USD", "2026-07-29"),
                fixing_record(ProviderId::Cfets, "USD", "2026-07-28"),
            ],
        ),
        FailureKind::Quality
    );
    assert_eq!(
        fixing_error(
            &request,
            vec![
                fixing_record(ProviderId::Cfets, "EUR", "2026-07-29"),
                fixing_record(ProviderId::Cfets, "USD", "2026-07-28"),
            ]
        ),
        FailureKind::Quality
    );
    let tight = OfficialFxFixingRequest::new(
        vec![fixing_identity(ProviderId::Cfets, "USD")],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        fixing_error(
            &tight,
            vec![
                fixing_record(ProviderId::Cfets, "USD", "2026-07-28"),
                fixing_record(ProviderId::Cfets, "USD", "2026-07-29"),
            ]
        ),
        FailureKind::Quality
    );

    let wrong_provider = fixing_record(ProviderId::Fred, "USD", "2026-07-28");
    let mut generic = OfficialFxFixingRouter::new(AcceptancePolicy::new());
    generic
        .register(official_fx_fixing_source(
            ProviderId::Cfets,
            Arc::new(FixingFixture(vec![wrong_provider])),
            classify,
        ))
        .unwrap();
    assert!(matches!(
        generic.route(&request).unwrap_err().attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}
