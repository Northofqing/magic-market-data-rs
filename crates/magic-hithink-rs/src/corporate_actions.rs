use super::{
    instrument_to_thscode, now, pair_ref, shanghai_midnight_date, shanghai_offset,
    validate_identity, HithinkClient, HithinkError, Success, ADJUSTMENT_FACTORS_PATH,
    CORPORATE_ACTIONS_ADMITTED,
};
use magic_market_core::{
    AssetClass, CorporateAction, CorporateActionCategory, CorporateActionRequest,
    CorporateActionResponse, CorporateActionStatus, CorporateActionTerms, CorporateActions,
    DataBatch, FiniteNumber, IsoDate, Provenance, ProviderId, SourceEvidence,
};
use serde::Deserialize;
use std::collections::HashSet;
use time::OffsetDateTime;

const MAX_ACTION_ROWS: usize = 2_000;

impl HithinkClient {
    /// Fetches Fuyao's cash-dividend and bonus-share adjustment event stream.
    pub fn probe_corporate_actions(
        &self,
        request: &CorporateActionRequest,
    ) -> Result<CorporateActionResponse, HithinkError> {
        if request.instrument().asset_class() != AssetClass::Equity {
            return Err(HithinkError::Unsupported(
                "Fuyao adjustment-factor events support A-share equities only".into(),
            ));
        }
        let current_date = current_shanghai_date()?;
        if request.end().is_some_and(|end| end > &current_date) {
            return Err(HithinkError::InvalidRequest(
                "corporate-action coverage must not extend beyond the current Shanghai date".into(),
            ));
        }
        let admission_as_of = request.end().cloned().unwrap_or(current_date);
        let mut query = vec![("thscode", instrument_to_thscode(request.instrument())?)];
        if let (Some(start), Some(end)) = (request.start(), request.end()) {
            query.push(("from", start.as_str().to_owned()));
            query.push(("to", end.as_str().to_owned()));
        }
        let response = self.get(ADJUSTMENT_FACTORS_PATH, query.iter().map(pair_ref))?;
        normalize_actions(request, response, admission_as_of)
    }
}

impl CorporateActions for HithinkClient {
    type Error = HithinkError;

    fn corporate_actions(
        &self,
        request: &CorporateActionRequest,
    ) -> Result<CorporateActionResponse, Self::Error> {
        if CORPORATE_ACTIONS_ADMITTED {
            self.probe_corporate_actions(request)
        } else {
            Err(HithinkError::Unsupported(
                "HITHINK corporate actions await production admission".into(),
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdjustmentFactorsData {
    thscode: String,
    ticker: String,
    item: Vec<AdjustmentFactorItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdjustmentFactorItem {
    ticker: String,
    ex_date_ms: i64,
    dividend_per_share: f64,
    per_share_bonus: f64,
}

fn normalize_actions(
    request: &CorporateActionRequest,
    response: Success<AdjustmentFactorsData>,
    admission_as_of: IsoDate,
) -> Result<CorporateActionResponse, HithinkError> {
    let expected = instrument_to_thscode(request.instrument())?;
    validate_identity(
        &expected,
        request.instrument().code(),
        &response.data.thscode,
        &response.data.ticker,
    )?;
    if response.data.item.len() > MAX_ACTION_ROWS {
        return Err(HithinkError::Protocol(format!(
            "adjustment-factor response exceeds {MAX_ACTION_ROWS} rows"
        )));
    }
    let observed_at = now()?;
    let batch_id = response.request_id;
    let evidence = SourceEvidence::new(
        ProviderId::Tonghuashun,
        observed_at.clone(),
        batch_id.clone(),
    )?;
    let mut dates = HashSet::with_capacity(response.data.item.len());
    let mut records = Vec::with_capacity(response.data.item.len());
    for item in response.data.item {
        if item.ticker != request.instrument().code() {
            return Err(HithinkError::Protocol(
                "adjustment-factor row identity contradicts the request".into(),
            ));
        }
        let ex_date = IsoDate::new(
            shanghai_midnight_date(item.ex_date_ms, "adjustment ex_date_ms")?.to_string(),
        )?;
        if request.start().is_some_and(|start| &ex_date < start)
            || request.end().is_some_and(|end| &ex_date > end)
            || ex_date > admission_as_of
            || !dates.insert(ex_date.clone())
        {
            return Err(HithinkError::Protocol(
                "adjustment-factor row is duplicate or outside request coverage".into(),
            ));
        }
        let cash = FiniteNumber::new(item.dividend_per_share)?;
        let bonus = FiniteNumber::new(item.per_share_bonus)?;
        if cash.get() < 0.0 || bonus.get() < 0.0 {
            return Err(HithinkError::Protocol(
                "adjustment-factor per-share terms must be non-negative".into(),
            ));
        }
        let terms = CorporateActionTerms::distribution(
            (cash.get() > 0.0).then_some(cash),
            (bonus.get() > 0.0).then_some(bonus),
            None,
            None,
        )?;
        records.push(
            CorporateAction::new(
                request.instrument().clone(),
                CorporateActionCategory::Distribution,
                ex_date.clone(),
                CorporateActionStatus::Implemented,
                terms,
                evidence.clone(),
            )?
            .with_dates(None, Some(ex_date), None),
        );
    }
    records.sort_by(|left, right| {
        left.effective_on()
            .cmp(right.effective_on())
            .then_with(|| left.category().cmp(&right.category()))
    });
    let provenance = Provenance::new("HithinkFinance", observed_at)?.with_batch_id(batch_id)?;
    let batch = DataBatch::strict(records, provenance);
    Ok(CorporateActionResponse::new(
        request.clone(),
        admission_as_of,
        evidence,
        batch,
    )?)
}

fn current_shanghai_date() -> Result<IsoDate, HithinkError> {
    Ok(IsoDate::new(
        OffsetDateTime::now_utc()
            .to_offset(shanghai_offset()?)
            .date()
            .to_string(),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{success, FixtureTransport};
    use crate::{parse_date, shanghai_millis};
    use magic_market_core::{Exchange, InstrumentId};
    use serde_json::json;
    use time::Time;

    fn instrument() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap()
    }

    #[test]
    fn adjustment_events_preserve_cash_bonus_and_exact_range() {
        let first = shanghai_millis(parse_date("2025-06-27").unwrap(), Time::MIDNIGHT).unwrap();
        let second = shanghai_millis(parse_date("2026-06-26").unwrap(), Time::MIDNIGHT).unwrap();
        let transport = FixtureTransport::new(vec![success(
            "actions-request",
            json!({
                "thscode": "600519.SH",
                "ticker": "600519",
                "item": [
                    {
                        "ticker": "600519",
                        "ex_date_ms": second,
                        "dividend_per_share": 30.0,
                        "per_share_bonus": 0.0
                    },
                    {
                        "ticker": "600519",
                        "ex_date_ms": first,
                        "dividend_per_share": 20.0,
                        "per_share_bonus": 0.1
                    }
                ]
            }),
        )]);
        let observed = transport.clone();
        let client = HithinkClient::with_transport("test_key", transport).unwrap();
        let request = CorporateActionRequest::new(instrument())
            .with_range(
                IsoDate::new("2025-01-01").unwrap(),
                IsoDate::new("2026-08-21").unwrap(),
            )
            .unwrap();
        let response = client.probe_corporate_actions(&request).unwrap();

        assert!(response.batch().quality().is_complete());
        assert_eq!(response.batch().records().len(), 2);
        assert_eq!(
            response.batch().records()[0].effective_on().as_str(),
            "2025-06-27"
        );
        assert_eq!(
            response.batch().records()[1].effective_on().as_str(),
            "2026-06-26"
        );
        assert_eq!(response.evidence().source_at(), None);
        match response.batch().records()[0].terms() {
            CorporateActionTerms::Distribution {
                cash_per_share,
                bonus_per_share,
                ..
            } => {
                assert_eq!(cash_per_share.unwrap().get(), 20.0);
                assert_eq!(bonus_per_share.unwrap().get(), 0.1);
            }
            _ => panic!("expected distribution terms"),
        }
        let urls = observed.requested_urls();
        assert!(urls[0].contains("from=2025-01-01"));
        assert!(urls[0].contains("to=2026-08-21"));
    }

    #[test]
    fn adjustment_events_reject_negative_or_empty_terms() {
        let date = shanghai_millis(parse_date("2026-06-26").unwrap(), Time::MIDNIGHT).unwrap();
        for (cash, bonus) in [(-1.0, 0.0), (0.0, 0.0)] {
            let client = HithinkClient::with_transport(
                "test_key",
                FixtureTransport::new(vec![success(
                    "invalid-actions",
                    json!({
                        "thscode": "600519.SH",
                        "ticker": "600519",
                        "item": [{
                            "ticker": "600519",
                            "ex_date_ms": date,
                            "dividend_per_share": cash,
                            "per_share_bonus": bonus
                        }]
                    }),
                )]),
            )
            .unwrap();
            assert!(client
                .probe_corporate_actions(&CorporateActionRequest::new(instrument()))
                .is_err());
        }
    }

    #[test]
    fn future_coverage_is_rejected_before_provider_io() {
        let client =
            HithinkClient::with_transport("test_key", FixtureTransport::default()).unwrap();
        let request = CorporateActionRequest::new(instrument())
            .with_range(
                IsoDate::new("2026-01-01").unwrap(),
                IsoDate::new("2099-12-31").unwrap(),
            )
            .unwrap();
        assert!(matches!(
            client.probe_corporate_actions(&request),
            Err(HithinkError::InvalidRequest(_))
        ));
    }
}
