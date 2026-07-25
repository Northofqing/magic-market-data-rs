use magic_market_core::{
    AssetClass, BoardCategory, BoardConstituentProvider, BoardConstituentRequest, BoardDefinition,
    BoardDirectoryProvider, BoardDirectoryRequest, BoardMembership, BoardMembershipProvider,
    DataBatch, DragonTigerDiscovery, DragonTigerDiscoveryRequest, DragonTigerEntry, Exchange,
    InstrumentId, IsoDate, NonEmptyText, PositiveU32, PriceLimitRule, Provenance, ProviderId,
    SecurityMetadata, SourceEvidence,
};
use magic_market_router::{
    board_constituent_source, board_directory_source, board_membership_source,
    dragon_tiger_discovery_source, join_board_membership_names, AcceptancePolicy, AttemptStatus,
    BoardConstituentRouter, BoardDirectoryRouter, BoardMembershipRouter,
    DragonTigerDiscoveryRouter, FailureKind, SourceError,
};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("fixture")]
struct FixtureError;

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Provider, "fixture")
}

fn instrument(exchange: Exchange, code: &str) -> InstrumentId {
    InstrumentId::new(exchange, code, AssetClass::Equity).unwrap()
}

fn evidence(provider: ProviderId, batch_id: &str, source_at: Option<&str>) -> SourceEvidence {
    let evidence = SourceEvidence::new(provider, "observed", batch_id).unwrap();
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

fn metadata(instrument: InstrumentId, name: Option<&str>, batch_id: &str) -> SecurityMetadata {
    SecurityMetadata::new(
        instrument,
        name.map(str::to_owned),
        None,
        None,
        None,
        PriceLimitRule::new(None, None).unwrap(),
        magic_market_core::DataStatus::Unavailable,
        Some("2026-07-24T15:00:00+08:00".into()),
        "observed",
        ProviderId::Sina,
        batch_id,
    )
    .unwrap()
}

fn metadata_batch(records: Vec<SecurityMetadata>, batch_id: &str) -> DataBatch<SecurityMetadata> {
    DataBatch::strict(
        records,
        Provenance::new("sina-security-metadata", "observed")
            .unwrap()
            .with_source_at("2026-07-24T15:00:00+08:00")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    )
}

struct DragonFixture {
    provider: ProviderId,
    exchange: Exchange,
    duplicate_id: bool,
}

impl DragonTigerDiscovery for DragonFixture {
    type Error = FixtureError;

    fn discover_dragon_tiger(
        &self,
        request: &DragonTigerDiscoveryRequest,
    ) -> Result<DataBatch<DragonTigerEntry>, Self::Error> {
        let batch_id = match self.provider {
            ProviderId::Eastmoney => "eastmoney-dragon",
            _ => "custom-dragon",
        };
        let source_at = format!("{}T16:00:00+08:00", request.trading_date().as_str());
        let records = (0..2)
            .map(|index| {
                let entry_id = if self.duplicate_id {
                    "trade-1".to_owned()
                } else {
                    format!("trade-{}", index + 1)
                };
                DragonTigerEntry::new(
                    NonEmptyText::new(entry_id).unwrap(),
                    instrument(self.exchange, &format!("92000{index}")),
                    request.trading_date().clone(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    evidence(self.provider, batch_id, Some(&source_at)),
                )
                .unwrap()
                .with_instrument_name(
                    NonEmptyText::new(format!("fixture stock {}", index + 1)).unwrap(),
                )
            })
            .collect();
        Ok(batch(records, batch_id, Some(&source_at)))
    }
}

#[test]
fn dragon_discovery_rejects_wrong_exchange_duplicates_and_limit() {
    let request = DragonTigerDiscoveryRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap()
    .with_exchange(Exchange::Beijing);

    let mut router =
        DragonTigerDiscoveryRouter::new(AcceptancePolicy::new().with_require_source_at(true));
    router
        .register(dragon_tiger_discovery_source(
            ProviderId::Eastmoney,
            Arc::new(DragonFixture {
                provider: ProviderId::Eastmoney,
                exchange: Exchange::Shanghai,
                duplicate_id: false,
            }),
            classify,
        ))
        .unwrap();
    router
        .register(dragon_tiger_discovery_source(
            ProviderId::Custom,
            Arc::new(DragonFixture {
                provider: ProviderId::Custom,
                exchange: Exchange::Beijing,
                duplicate_id: false,
            }),
            classify,
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Custom);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));

    let mut duplicate_router = DragonTigerDiscoveryRouter::new(AcceptancePolicy::new());
    duplicate_router
        .register(dragon_tiger_discovery_source(
            ProviderId::Eastmoney,
            Arc::new(DragonFixture {
                provider: ProviderId::Eastmoney,
                exchange: Exchange::Beijing,
                duplicate_id: true,
            }),
            classify,
        ))
        .unwrap();
    duplicate_router
        .register(dragon_tiger_discovery_source(
            ProviderId::Custom,
            Arc::new(DragonFixture {
                provider: ProviderId::Custom,
                exchange: Exchange::Beijing,
                duplicate_id: false,
            }),
            classify,
        ))
        .unwrap();
    let outcome = duplicate_router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Custom);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Quality,
            ..
        }
    ));

    let limit_one = DragonTigerDiscoveryRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap()
    .with_exchange(Exchange::Beijing);
    let error = duplicate_router.route(&limit_one).unwrap_err();
    assert!(error.attempts().iter().all(|attempt| matches!(
        attempt.status(),
        AttemptStatus::Failed {
            kind: FailureKind::Quality,
            ..
        }
    )));
}

struct BoardFixture {
    provider: ProviderId,
    category: BoardCategory,
    board_code: &'static str,
}

impl BoardDirectoryProvider for BoardFixture {
    type Error = FixtureError;

    fn boards(
        &self,
        _request: &BoardDirectoryRequest,
    ) -> Result<DataBatch<BoardDefinition>, Self::Error> {
        let batch_id = format!("{:?}-directory", self.provider);
        let record = BoardDefinition::new(
            NonEmptyText::new(self.board_code).unwrap(),
            NonEmptyText::new("人工智能").unwrap(),
            self.category,
            PositiveU32::new(2).unwrap(),
            evidence(self.provider, &batch_id, None),
        )
        .unwrap();
        Ok(batch(vec![record], &batch_id, None))
    }
}

impl BoardConstituentProvider for BoardFixture {
    type Error = FixtureError;

    fn board_constituents(
        &self,
        _request: &BoardConstituentRequest,
    ) -> Result<DataBatch<BoardMembership>, Self::Error> {
        let batch_id = format!("{:?}-constituents", self.provider);
        let record = BoardMembership {
            instrument: instrument(Exchange::Shanghai, "600000"),
            board_code: NonEmptyText::new(self.board_code).unwrap(),
            board_name: NonEmptyText::new("人工智能").unwrap(),
            category: self.category,
            evidence: evidence(self.provider, &batch_id, None),
        };
        Ok(batch(vec![record], &batch_id, None))
    }
}

#[test]
fn board_routes_enforce_directory_category_and_constituent_identity() {
    let directory =
        BoardDirectoryRequest::new(BoardCategory::Concept, PositiveU32::new(2).unwrap()).unwrap();
    let mut directory_router = BoardDirectoryRouter::new(AcceptancePolicy::new());
    directory_router
        .register(board_directory_source(
            ProviderId::Tdx,
            Arc::new(BoardFixture {
                provider: ProviderId::Tdx,
                category: BoardCategory::Industry,
                board_code: "tdx:industry:电力",
            }),
            classify,
        ))
        .unwrap();
    directory_router
        .register(board_directory_source(
            ProviderId::Custom,
            Arc::new(BoardFixture {
                provider: ProviderId::Custom,
                category: BoardCategory::Concept,
                board_code: "custom:concept:人工智能",
            }),
            classify,
        ))
        .unwrap();
    let outcome = directory_router.route(&directory).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Custom);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));

    let constituents = BoardConstituentRequest::new(
        NonEmptyText::new("tdx:concept:人工智能").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    let mut constituent_router = BoardConstituentRouter::new(AcceptancePolicy::new());
    constituent_router
        .register(board_constituent_source(
            ProviderId::Tdx,
            Arc::new(BoardFixture {
                provider: ProviderId::Tdx,
                category: BoardCategory::Concept,
                board_code: "tdx:concept:算力",
            }),
            classify,
        ))
        .unwrap();
    constituent_router
        .register(board_constituent_source(
            ProviderId::Custom,
            Arc::new(BoardFixture {
                provider: ProviderId::Custom,
                category: BoardCategory::Concept,
                board_code: "tdx:concept:人工智能",
            }),
            classify,
        ))
        .unwrap();
    let outcome = constituent_router.route(&constituents).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Custom);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

struct MembershipFixture {
    provider: ProviderId,
    returned_instrument: InstrumentId,
    duplicate: bool,
}

impl BoardMembershipProvider for MembershipFixture {
    type Error = FixtureError;

    fn board_memberships(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<BoardMembership>, Self::Error> {
        let batch_id = format!("{:?}-memberships", self.provider);
        let record = || BoardMembership {
            instrument: self.returned_instrument.clone(),
            board_code: NonEmptyText::new("tdx:concept:人工智能").unwrap(),
            board_name: NonEmptyText::new("人工智能").unwrap(),
            category: BoardCategory::Concept,
            evidence: evidence(self.provider, &batch_id, None),
        };
        let mut records = vec![record()];
        if self.duplicate {
            records.push(record());
        }
        Ok(batch(records, &batch_id, None))
    }
}

#[test]
fn reverse_memberships_reject_unrequested_instruments_and_duplicate_identities() {
    let requested = instrument(Exchange::Shanghai, "600000");
    let mut router = BoardMembershipRouter::new(AcceptancePolicy::new());
    router
        .register(board_membership_source(
            ProviderId::Tdx,
            Arc::new(MembershipFixture {
                provider: ProviderId::Tdx,
                returned_instrument: instrument(Exchange::Shanghai, "600001"),
                duplicate: false,
            }),
            classify,
        ))
        .unwrap();
    router
        .register(board_membership_source(
            ProviderId::Custom,
            Arc::new(MembershipFixture {
                provider: ProviderId::Custom,
                returned_instrument: requested.clone(),
                duplicate: false,
            }),
            classify,
        ))
        .unwrap();
    let outcome = router.route(std::slice::from_ref(&requested)).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Custom);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));

    let mut duplicate_router = BoardMembershipRouter::new(AcceptancePolicy::new());
    duplicate_router
        .register(board_membership_source(
            ProviderId::Tdx,
            Arc::new(MembershipFixture {
                provider: ProviderId::Tdx,
                returned_instrument: requested.clone(),
                duplicate: true,
            }),
            classify,
        ))
        .unwrap();
    duplicate_router
        .register(board_membership_source(
            ProviderId::Custom,
            Arc::new(MembershipFixture {
                provider: ProviderId::Custom,
                returned_instrument: requested.clone(),
                duplicate: false,
            }),
            classify,
        ))
        .unwrap();
    let outcome = duplicate_router
        .route(std::slice::from_ref(&requested))
        .unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Custom);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Quality,
            ..
        }
    ));
}

#[test]
fn board_name_join_requires_exact_metadata_and_keeps_both_evidence_records() {
    let requested = instrument(Exchange::Shenzhen, "002230");
    let membership_batch = batch(
        vec![BoardMembership {
            instrument: requested.clone(),
            board_code: NonEmptyText::new("tdx:concept:人工智能").unwrap(),
            board_name: NonEmptyText::new("人工智能").unwrap(),
            category: BoardCategory::Concept,
            evidence: evidence(ProviderId::Tdx, "tdx-board", None),
        }],
        "tdx-board",
        None,
    );
    let names = metadata_batch(
        vec![metadata(requested.clone(), Some("科大讯飞"), "sina-names")],
        "sina-names",
    );
    let joined = join_board_membership_names(&membership_batch, &names).unwrap();
    assert_eq!(joined[0].membership.instrument, requested);
    assert_eq!(joined[0].instrument_name.as_str(), "科大讯飞");
    assert_eq!(joined[0].membership.evidence.provider(), ProviderId::Tdx);
    assert_eq!(
        joined[0].instrument_name_evidence.provider(),
        ProviderId::Sina
    );

    let missing_name = metadata_batch(
        vec![metadata(
            joined[0].membership.instrument.clone(),
            None,
            "missing-name",
        )],
        "missing-name",
    );
    assert!(join_board_membership_names(&membership_batch, &missing_name).is_err());

    let wrong = metadata_batch(
        vec![metadata(
            instrument(Exchange::Shanghai, "600000"),
            Some("浦发银行"),
            "wrong-name",
        )],
        "wrong-name",
    );
    assert!(join_board_membership_names(&membership_batch, &wrong).is_err());
}
