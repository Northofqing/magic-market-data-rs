use crate::{FailoverChain, FailureKind, SourceError, SourceFn};
use magic_market_core::{
    EconomicCalendarProvider, EconomicCalendarRequest, EconomicEvent, ForeignExchangeProvider,
    FuturesDeliveryCalendar, FuturesDeliveryEvent, FuturesDeliveryRequest, FuturesProduct, FxQuote,
    FxRequest, GlobalIndexCode, GlobalIndexProvider, GlobalIndexQuote, GlobalIndexRequest,
    PolicyDocument, PolicyDocuments, PolicyRequest, ProviderId, ResearchDocument,
    ResearchDocumentRequest, ResearchDocuments,
};
use std::collections::HashSet;
use std::sync::Arc;

pub type GlobalIndexRouter = FailoverChain<GlobalIndexRequest, GlobalIndexQuote>;
pub type ForeignExchangeRouter = FailoverChain<FxRequest, FxQuote>;
pub type EconomicCalendarRouter = FailoverChain<EconomicCalendarRequest, EconomicEvent>;
pub type PolicyDocumentRouter = FailoverChain<PolicyRequest, PolicyDocument>;
pub type ResearchDocumentRouter = FailoverChain<ResearchDocumentRequest, ResearchDocument>;
pub type FuturesDeliveryRouter = FailoverChain<FuturesDeliveryRequest, FuturesDeliveryEvent>;

pub fn global_index_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<GlobalIndexRequest, GlobalIndexQuote>
where
    Provider: GlobalIndexProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.global_indices(request).map_err(&classify)?;
        validate_exact_indices(request.indices(), batch.records())?;
        Ok(batch)
    })
}

pub fn foreign_exchange_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<FxRequest, FxQuote>
where
    Provider: ForeignExchangeProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.foreign_exchange(request).map_err(&classify)?;
        if batch.records().len() != request.pairs().len() {
            return quality("FX batch cardinality does not match requested pairs");
        }
        let requested: HashSet<_> = request.pairs().iter().copied().collect();
        let mut actual = HashSet::with_capacity(batch.records().len());
        for record in batch.records() {
            if !requested.contains(&record.pair) {
                return evidence("FX batch contains an unrequested pair");
            }
            if !actual.insert(record.pair) {
                return quality("FX batch contains duplicate pairs");
            }
        }
        Ok(batch)
    })
}

pub fn economic_calendar_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<EconomicCalendarRequest, EconomicEvent>
where
    Provider: EconomicCalendarProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.economic_calendar(request).map_err(&classify)?;
        if batch.records().len() > request.limit().get() as usize {
            return quality("economic calendar batch exceeds requested limit");
        }
        let mut identities = HashSet::with_capacity(batch.records().len());
        let mut previous = None;
        for record in batch.records() {
            if request
                .country()
                .is_some_and(|country| country != &record.country)
            {
                return evidence("economic event country does not match requested country");
            }
            if !identities.insert(record.event_id.as_str()) {
                return quality("economic calendar contains duplicate event IDs");
            }
            if previous.is_some_and(|value: &str| value < record.released_at.as_str()) {
                return quality("economic calendar is not sorted newest first");
            }
            previous = Some(record.released_at.as_str());
        }
        Ok(batch)
    })
}

pub fn policy_document_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<PolicyRequest, PolicyDocument>
where
    Provider: PolicyDocuments + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.policy_documents(request).map_err(&classify)?;
        if batch.records().len() > request.page_size().get() as usize {
            return quality("policy batch exceeds requested page size");
        }
        let mut identities = HashSet::with_capacity(batch.records().len());
        let mut previous = None;
        for record in batch.records() {
            if request
                .start()
                .is_some_and(|start| &record.published_date < start)
                || request
                    .end()
                    .is_some_and(|end| &record.published_date > end)
            {
                return evidence("policy date is outside requested range");
            }
            if !identities.insert(record.document_id.as_str()) {
                return quality("policy batch contains duplicate document IDs");
            }
            if previous.is_some_and(|date| date < &record.published_date) {
                return quality("policy batch is not sorted newest first");
            }
            previous = Some(&record.published_date);
        }
        Ok(batch)
    })
}

pub fn research_document_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<ResearchDocumentRequest, ResearchDocument>
where
    Provider: ResearchDocuments + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.research_document(request).map_err(&classify)?;
        if batch.records().len() != 1 {
            return quality("research document batch must contain exactly one PDF");
        }
        let record = &batch.records()[0];
        if record.report_id != request.report_id || record.pdf_url != request.pdf_url {
            return evidence("research document identity does not match request");
        }
        Ok(batch)
    })
}

pub fn futures_delivery_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<FuturesDeliveryRequest, FuturesDeliveryEvent>
where
    Provider: FuturesDeliveryCalendar + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider
            .futures_delivery_calendar(request)
            .map_err(&classify)?;
        if batch.records().len() != 4 {
            return quality("CFFEX delivery batch must contain all four index futures");
        }
        let suffix = format!(
            "{:02}{:02}",
            request.year().get() % 100,
            request.month().get()
        );
        let expected: HashSet<_> = [
            FuturesProduct::If,
            FuturesProduct::Ih,
            FuturesProduct::Ic,
            FuturesProduct::Im,
        ]
        .into_iter()
        .collect();
        let mut actual = HashSet::with_capacity(4);
        for record in batch.records() {
            if !actual.insert(record.product) {
                return quality("CFFEX delivery batch contains duplicate products");
            }
            if !record.contract_code.as_str().ends_with(&suffix) {
                return evidence("CFFEX contract code does not match requested month");
            }
            let month = format!("{:04}-{:02}", request.year().get(), request.month().get());
            if record.delivery_date.as_str().get(..7) != Some(month.as_str())
                || record.last_trading_date != record.delivery_date
            {
                return evidence("CFFEX delivery date does not match requested month");
            }
        }
        if actual != expected {
            return quality("CFFEX delivery batch omits a required product");
        }
        Ok(batch)
    })
}

fn validate_exact_indices(
    requested: &[GlobalIndexCode],
    records: &[GlobalIndexQuote],
) -> Result<(), SourceError> {
    if records.len() != requested.len() {
        return quality("global-index batch cardinality does not match request");
    }
    let requested: HashSet<_> = requested.iter().copied().collect();
    let mut actual = HashSet::with_capacity(records.len());
    for record in records {
        if !requested.contains(&record.index) {
            return evidence("global-index batch contains an unrequested identity");
        }
        if !actual.insert(record.index) {
            return quality("global-index batch contains duplicate identities");
        }
    }
    Ok(())
}

fn evidence<T>(message: &'static str) -> Result<T, SourceError> {
    Err(SourceError::try_next(FailureKind::Evidence, message))
}

fn quality<T>(message: &'static str) -> Result<T, SourceError> {
    Err(SourceError::try_next(FailureKind::Quality, message))
}
