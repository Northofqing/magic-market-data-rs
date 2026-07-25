use crate::{FailoverChain, FailureKind, SourceError, SourceFn};
use magic_market_core::{
    Announcement, AssetClass, Exchange, IsoDate, MarketAnnouncementRequest, MarketAnnouncements,
    ProviderId,
};
use std::collections::HashSet;
use std::sync::Arc;

pub type MarketAnnouncementRouter = FailoverChain<MarketAnnouncementRequest, Announcement>;

pub fn market_announcement_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<MarketAnnouncementRequest, Announcement>
where
    Provider: MarketAnnouncements + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.market_announcements(request).map_err(&classify)?;
        if batch.records().len() > request.limit().get() as usize {
            return Err(SourceError::try_next(
                FailureKind::Quality,
                "market announcement batch exceeds requested limit",
            ));
        }
        let batch_id = batch.provenance().batch_id().ok_or_else(|| {
            SourceError::try_next(
                FailureKind::Evidence,
                "market announcement batch has no batch ID",
            )
        })?;
        if batch.records().is_empty() {
            if !batch.quality().is_complete() {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "empty market announcement batch is not complete",
                ));
            }
            if batch.provenance().source_at().is_some() {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "verified-empty market announcement batch must not claim source_at",
                ));
            }
            return Ok(batch);
        }

        let first_source_at = batch.records()[0].published_at.as_str();
        if batch.provenance().source_at() != Some(first_source_at) {
            return Err(SourceError::try_next(
                FailureKind::Evidence,
                "market announcement batch source_at is not the newest record time",
            ));
        }
        let mut ids = HashSet::with_capacity(batch.records().len());
        let mut previous_source_at: Option<&str> = None;
        for record in batch.records() {
            if record.instrument.asset_class() != AssetClass::Equity
                || !matches!(
                    record.instrument.exchange(),
                    Exchange::Shanghai | Exchange::Shenzhen | Exchange::Beijing
                )
            {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "market announcement record is not an A-share equity",
                ));
            }
            let code = record.instrument.code();
            if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "market announcement security code is not six ASCII digits",
                ));
            }
            if record.evidence.provider() != provider_id {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "market announcement record provider does not match registered source",
                ));
            }
            if record.evidence.batch_id() != batch_id {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "market announcement record batch ID does not match batch provenance",
                ));
            }
            if record.evidence.source_at() != Some(record.published_at.as_str()) {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "market announcement record source_at does not equal published_at",
                ));
            }
            let published_date =
                market_announcement_date(record.published_at.as_str(), "published_at")?;
            if published_date < *request.start() || published_date > *request.end() {
                return Err(SourceError::try_next(
                    FailureKind::Evidence,
                    "market announcement publication date is outside requested range",
                ));
            }
            if previous_source_at.is_some_and(|previous| record.published_at.as_str() > previous) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "market announcement batch is not newest-first",
                ));
            }
            previous_source_at = Some(record.published_at.as_str());
            if !ids.insert(record.announcement_id.as_str()) {
                return Err(SourceError::try_next(
                    FailureKind::Quality,
                    "market announcement batch contains a duplicate announcement ID",
                ));
            }
        }
        Ok(batch)
    })
}

fn market_announcement_date(value: &str, field: &str) -> Result<IsoDate, SourceError> {
    let date = value.get(..10).ok_or_else(|| {
        SourceError::try_next(
            FailureKind::Evidence,
            format!("market announcement {field} must start with YYYY-MM-DD"),
        )
    })?;
    if !matches!(value.as_bytes().get(10), None | Some(b' ') | Some(b'T')) {
        return Err(SourceError::try_next(
            FailureKind::Evidence,
            format!("market announcement {field} has an invalid date/time separator"),
        ));
    }
    IsoDate::new(date).map_err(|error| {
        SourceError::try_next(
            FailureKind::Evidence,
            format!("market announcement {field} is invalid: {error}"),
        )
    })
}
