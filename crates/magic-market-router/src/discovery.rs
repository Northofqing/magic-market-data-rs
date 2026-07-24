use crate::{FailoverChain, FailureKind, SourceError, SourceFn};
use magic_market_core::{
    BoardConstituentProvider, BoardConstituentRequest, BoardDefinition, BoardDirectoryProvider,
    BoardDirectoryRequest, BoardMembership, DragonTigerDiscovery, DragonTigerDiscoveryRequest,
    DragonTigerEntry, ProviderId,
};
use std::collections::HashSet;
use std::sync::Arc;

pub type DragonTigerDiscoveryRouter = FailoverChain<DragonTigerDiscoveryRequest, DragonTigerEntry>;
pub type BoardDirectoryRouter = FailoverChain<BoardDirectoryRequest, BoardDefinition>;
pub type BoardConstituentRouter = FailoverChain<BoardConstituentRequest, BoardMembership>;

pub fn dragon_tiger_discovery_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<DragonTigerDiscoveryRequest, DragonTigerEntry>
where
    Provider: DragonTigerDiscovery + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.discover_dragon_tiger(request).map_err(&classify)?;
        if batch.records().len() > request.limit().get() as usize {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "dragon-tiger discovery batch exceeds requested limit",
            ));
        }
        validate_batch_date(
            batch.provenance().source_at(),
            request.trading_date().as_str(),
            "dragon-tiger discovery batch",
        )?;

        let mut identities = HashSet::with_capacity(batch.records().len());
        for record in batch.records() {
            if record.trading_date() != request.trading_date() {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "dragon-tiger discovery record date does not match requested date",
                ));
            }
            if request
                .exchange()
                .is_some_and(|exchange| record.instrument().exchange() != exchange)
            {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "dragon-tiger discovery record exchange does not match requested exchange",
                ));
            }
            let identity = (
                record.trading_date().as_str().to_owned(),
                record.entry_id().as_str().to_owned(),
            );
            if !identities.insert(identity) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "dragon-tiger discovery batch contains duplicate entry identities",
                ));
            }
        }
        Ok(batch)
    })
}

pub fn board_directory_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<BoardDirectoryRequest, BoardDefinition>
where
    Provider: BoardDirectoryProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.boards(request).map_err(&classify)?;
        if batch.records().len() > request.limit().get() as usize {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "board directory batch exceeds requested limit",
            ));
        }

        let mut board_codes = HashSet::with_capacity(batch.records().len());
        for record in batch.records() {
            if record.category() != request.category() {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "board directory record category does not match requested category",
                ));
            }
            if !board_codes.insert(record.board_code().as_str()) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "board directory batch contains duplicate board identities",
                ));
            }
        }
        Ok(batch)
    })
}

pub fn board_constituent_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<BoardConstituentRequest, BoardMembership>
where
    Provider: BoardConstituentProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.board_constituents(request).map_err(&classify)?;
        if batch.records().len() > request.limit().get() as usize {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "board constituent batch exceeds requested limit",
            ));
        }

        let mut identities = HashSet::with_capacity(batch.records().len());
        for record in batch.records() {
            if record.board_code != *request.board_code() {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "board constituent record does not match requested board",
                ));
            }
            if !identities.insert((record.instrument.clone(), record.board_code.as_str())) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "board constituent batch contains duplicate member identities",
                ));
            }
        }
        Ok(batch)
    })
}

fn validate_batch_date(
    source_at: Option<&str>,
    expected_date: &str,
    family: &str,
) -> Result<(), SourceError> {
    let Some(source_at) = source_at else {
        return Ok(());
    };
    let remainder = source_at.strip_prefix(expected_date).ok_or_else(|| {
        SourceError::try_next(
            FailureKind::Evidence,
            format!("{family} source timestamp does not match requested date"),
        )
    })?;
    if !matches!(remainder.as_bytes().first(), Some(b'T' | b' ')) {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            format!("{family} source timestamp does not match requested date"),
        ));
    }
    Ok(())
}
