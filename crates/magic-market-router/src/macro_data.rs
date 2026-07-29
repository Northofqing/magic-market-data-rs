use crate::{FailoverChain, FailureKind, SourceError, SourceFn};
use magic_market_core::{
    EconomicObservation, EconomicSeriesProvider, EconomicSeriesRequest, OfficialFxFixing,
    OfficialFxFixingProvider, OfficialFxFixingRequest, ProviderId, ReferenceRateObservation,
    ReferenceRateProvider, ReferenceRateRequest,
};
use std::collections::HashSet;
use std::sync::Arc;

pub type EconomicSeriesRouter = FailoverChain<EconomicSeriesRequest, EconomicObservation>;
pub type ReferenceRateRouter = FailoverChain<ReferenceRateRequest, ReferenceRateObservation>;
pub type OfficialFxFixingRouter = FailoverChain<OfficialFxFixingRequest, OfficialFxFixing>;

pub fn economic_series_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<EconomicSeriesRequest, EconomicObservation>
where
    Provider: EconomicSeriesProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &EconomicSeriesRequest| {
        if request.provider() != provider_id {
            return evidence(
                "economic-series source cannot substitute a different provider namespace",
            );
        }
        let batch = provider.economic_series(request).map_err(&classify)?;
        validate_economic_batch(request, batch.records())?;
        Ok(batch)
    })
}

pub fn reference_rate_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<ReferenceRateRequest, ReferenceRateObservation>
where
    Provider: ReferenceRateProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &ReferenceRateRequest| {
        if request.provider() != provider_id {
            return evidence(
                "reference-rate source cannot substitute a different provider identity",
            );
        }
        let batch = provider.reference_rates(request).map_err(&classify)?;
        validate_reference_rate_batch(request, batch.records())?;
        Ok(batch)
    })
}

pub fn official_fx_fixing_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<OfficialFxFixingRequest, OfficialFxFixing>
where
    Provider: OfficialFxFixingProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request: &OfficialFxFixingRequest| {
        if request.provider() != provider_id {
            return evidence(
                "official-fixing source cannot substitute a different provider identity",
            );
        }
        let batch = provider.official_fx_fixings(request).map_err(&classify)?;
        validate_official_fixing_batch(request, batch.records())?;
        Ok(batch)
    })
}

fn validate_economic_batch(
    request: &EconomicSeriesRequest,
    records: &[EconomicObservation],
) -> Result<(), SourceError> {
    if records.len() > request.max_rows().get() as usize {
        return quality("economic-series batch exceeds requested max_rows");
    }
    let mut identities = HashSet::with_capacity(records.len());
    let mut previous = None;
    for record in records {
        let position = request
            .series()
            .iter()
            .position(|requested| {
                requested.namespace() == record.series().namespace()
                    && requested.code() == record.series().code()
            })
            .ok_or_else(|| {
                evidence_error("economic-series batch contains an unrequested series identity")
            })?;
        if record.period().frequency() != request.start().frequency()
            || record.period() < request.start()
            || record.period() > request.end()
        {
            return evidence("economic-series observation is outside the requested range");
        }
        if !identities.insert((
            record.series().clone(),
            record.region_code().map(str::to_owned),
            record.period().clone(),
        )) {
            return quality("economic-series batch contains a duplicate observation identity");
        }
        let ordering_key = (
            position,
            record.region_code(),
            record.region_name(),
            record.period(),
        );
        if previous.is_some_and(|prior| prior > ordering_key) {
            return quality("economic-series batch is not in canonical order");
        }
        previous = Some(ordering_key);
    }
    Ok(())
}

fn validate_reference_rate_batch(
    request: &ReferenceRateRequest,
    records: &[ReferenceRateObservation],
) -> Result<(), SourceError> {
    if records.len() > request.max_rows().get() as usize {
        return quality("reference-rate batch exceeds requested max_rows");
    }
    let mut identities = HashSet::with_capacity(records.len());
    let mut previous = None;
    for record in records {
        let position = request
            .rates()
            .iter()
            .position(|requested| requested.kind() == record.identity().kind())
            .ok_or_else(|| {
                evidence_error("reference-rate batch contains an unrequested rate identity")
            })?;
        if record.fixing_date() < request.start() || record.fixing_date() > request.end() {
            return evidence("reference-rate observation is outside the requested date range");
        }
        if !identities.insert((record.identity().clone(), record.fixing_date().clone())) {
            return quality("reference-rate batch contains a duplicate observation identity");
        }
        let ordering_key = (position, record.fixing_date());
        if previous.is_some_and(|prior| prior > ordering_key) {
            return quality("reference-rate batch is not in canonical order");
        }
        previous = Some(ordering_key);
    }
    Ok(())
}

fn validate_official_fixing_batch(
    request: &OfficialFxFixingRequest,
    records: &[OfficialFxFixing],
) -> Result<(), SourceError> {
    if records.len() > request.max_rows().get() as usize {
        return quality("official-fixing batch exceeds requested max_rows");
    }
    let mut identities = HashSet::with_capacity(records.len());
    let mut previous = None;
    for record in records {
        let position = request
            .pairs()
            .iter()
            .position(|requested| {
                requested.base() == record.identity().base()
                    && requested.quote() == record.identity().quote()
            })
            .ok_or_else(|| {
                evidence_error("official-fixing batch contains an unrequested currency pair")
            })?;
        if record.fixing_date() < request.start() || record.fixing_date() > request.end() {
            return evidence("official-fixing observation is outside the requested date range");
        }
        if !identities.insert((record.identity().clone(), record.fixing_date().clone())) {
            return quality("official-fixing batch contains a duplicate observation identity");
        }
        let ordering_key = (position, record.fixing_date());
        if previous.is_some_and(|prior| prior > ordering_key) {
            return quality("official-fixing batch is not in canonical order");
        }
        previous = Some(ordering_key);
    }
    Ok(())
}

fn evidence<T>(message: &str) -> Result<T, SourceError> {
    Err(evidence_error(message))
}

fn evidence_error(message: &str) -> SourceError {
    SourceError::try_next(FailureKind::Evidence, message)
}

fn quality<T>(message: &str) -> Result<T, SourceError> {
    Err(SourceError::try_next(FailureKind::Quality, message))
}
