use magic_market_core::{
    AssetClass, BoardMembership, BoardMembershipProvider, DataBatch, Exchange, InstrumentId,
    Provenance, ProviderId,
};
use magic_market_router::{board_membership_source, FailureKind, RoutedSource, SourceError};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("fixture board-membership error")]
struct FixtureError;

struct CompleteEmptyProvider;

impl BoardMembershipProvider for CompleteEmptyProvider {
    type Error = FixtureError;

    fn board_memberships(
        &self,
        _instruments: &[InstrumentId],
    ) -> Result<DataBatch<BoardMembership>, Self::Error> {
        Ok(DataBatch::strict(
            Vec::new(),
            Provenance::new("tdx-block-files", "observed")
                .unwrap()
                .with_batch_id("tdx-board-memberships:v1|fixture")
                .unwrap(),
        ))
    }
}

fn classify_tdx(error: magic_tdx_rs::TdxError) -> SourceError {
    SourceError::try_next(FailureKind::Provider, error.to_string())
}

fn classify_fixture(error: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Provider, error.to_string())
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

#[test]
fn block_service_registers_on_the_existing_board_membership_adapter() {
    let provider = Arc::new(magic_tdx_rs::BlockService::with_default("127.0.0.1"));
    let _source = board_membership_source(ProviderId::Tdx, provider, classify_tdx);
}

#[test]
fn board_membership_adapter_preserves_complete_empty_batch_evidence() {
    let source = board_membership_source(
        ProviderId::Tdx,
        Arc::new(CompleteEmptyProvider),
        classify_fixture,
    );

    let batch = source.fetch(&[instrument()]).unwrap();

    assert!(batch.records().is_empty());
    assert_eq!(batch.provenance().source(), "tdx-block-files");
    assert_eq!(batch.provenance().fetched_at(), "observed");
    assert_eq!(
        batch.provenance().batch_id(),
        Some("tdx-board-memberships:v1|fixture")
    );
    assert!(batch.provenance().source_at().is_none());
}
