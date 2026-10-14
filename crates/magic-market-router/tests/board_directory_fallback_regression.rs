use magic_market_core::{
    BoardCategory, BoardDefinition, BoardDirectoryProvider, BoardDirectoryRequest, DataBatch,
    NonEmptyText, PositiveU32, Provenance, ProviderId, SourceEvidence,
};
use magic_market_router::{
    board_directory_source, AcceptancePolicy, AttemptStatus, BoardDirectoryRouter, FailureAction,
    FailureKind, SourceError,
};
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
#[error("offline directory rate limit")]
enum OfflineDirectoryError {
    RateLimited,
}

#[derive(Clone)]
struct ExternalDirectoryFixture(Result<DataBatch<BoardDefinition>, OfflineDirectoryError>);

impl BoardDirectoryProvider for ExternalDirectoryFixture {
    type Error = OfflineDirectoryError;

    fn boards(
        &self,
        _request: &BoardDirectoryRequest,
    ) -> Result<DataBatch<BoardDefinition>, Self::Error> {
        self.0.clone()
    }
}

fn classify_external_error(error: OfflineDirectoryError) -> SourceError {
    match error {
        OfflineDirectoryError::RateLimited => SourceError::try_next(
            FailureKind::RateLimited,
            "offline directory capacity exhausted",
        ),
    }
}

#[test]
fn board_directory_route_retains_rate_limit_attempt_and_complete_fallback_evidence() {
    let request =
        BoardDirectoryRequest::new(BoardCategory::Concept, PositiveU32::new(1).unwrap()).unwrap();
    let fallback_record = BoardDefinition::new(
        NonEmptyText::new("custom:block_gn.dat:artificial-intelligence").unwrap(),
        NonEmptyText::new("Artificial Intelligence").unwrap(),
        BoardCategory::Concept,
        PositiveU32::new(37).unwrap(),
        SourceEvidence::new(
            ProviderId::Custom,
            "2026-10-09T09:30:01+08:00",
            "directory-fallback:20261009:1",
        )
        .unwrap()
        .with_source_at("2026-10-09T09:30:00+08:00")
        .unwrap(),
    )
    .unwrap();
    let fallback_batch = DataBatch::strict(
        vec![fallback_record],
        Provenance::new("offline-directory-secondary", "2026-10-09T09:30:01+08:00")
            .unwrap()
            .with_source_at("2026-10-09T09:30:00+08:00")
            .unwrap()
            .with_batch_id("directory-fallback:20261009:1")
            .unwrap(),
    );
    let mut router = BoardDirectoryRouter::new(
        AcceptancePolicy::new()
            .with_require_complete(true)
            .with_require_source_at(true),
    );
    router
        .register(board_directory_source(
            ProviderId::Tdx,
            Arc::new(ExternalDirectoryFixture(Err(
                OfflineDirectoryError::RateLimited,
            ))),
            classify_external_error,
        ))
        .unwrap()
        .register(board_directory_source(
            ProviderId::Custom,
            Arc::new(ExternalDirectoryFixture(Ok(fallback_batch))),
            classify_external_error,
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Custom);
    assert_eq!(outcome.attempts().len(), 2);
    assert_eq!(outcome.attempts()[0].provider_id(), ProviderId::Tdx);
    assert_eq!(
        outcome.attempts()[0].status(),
        &AttemptStatus::Failed {
            kind: FailureKind::RateLimited,
            action: FailureAction::TryNext,
            message: "offline directory capacity exhausted".into(),
        }
    );
    assert_eq!(outcome.attempts()[1].provider_id(), ProviderId::Custom);
    assert_eq!(outcome.attempts()[1].status(), &AttemptStatus::Selected);

    assert!(outcome.batch().quality().is_complete());
    assert_eq!(outcome.batch().records().len(), 1);
    let record = &outcome.batch().records()[0];
    assert_eq!(
        record.board_code().as_str(),
        "custom:block_gn.dat:artificial-intelligence"
    );
    assert_eq!(record.board_name().as_str(), "Artificial Intelligence");
    assert_eq!(record.category(), BoardCategory::Concept);
    assert_eq!(record.member_count().get(), 37);
    assert_eq!(record.evidence().provider(), ProviderId::Custom);
    assert_eq!(
        record.evidence().batch_id(),
        "directory-fallback:20261009:1"
    );
    assert_eq!(record.evidence().observed_at(), "2026-10-09T09:30:01+08:00");
    assert_eq!(
        record.evidence().source_at(),
        Some("2026-10-09T09:30:00+08:00")
    );
    let provenance = outcome.batch().provenance();
    assert_eq!(provenance.source(), "offline-directory-secondary");
    assert_eq!(provenance.batch_id(), Some("directory-fallback:20261009:1"));
    assert_eq!(provenance.fetched_at(), "2026-10-09T09:30:01+08:00");
    assert_eq!(provenance.source_at(), Some("2026-10-09T09:30:00+08:00"));
}
