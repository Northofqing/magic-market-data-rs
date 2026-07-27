use magic_market_core::{AssetClass, Exchange, InstrumentId, ProviderId};
use magic_market_router::{
    consensus_source, AcceptancePolicy, AttemptStatus, ConsensusRouter, FailureKind, SourceError,
};
use magic_ths_rs::{ThsClient, ThsError};
use std::error::Error;
use std::sync::Arc;

fn classify_ths(error: ThsError) -> SourceError {
    let message = error.to_string();
    match error {
        ThsError::InvalidRequest(_) => SourceError::stop(FailureKind::InvalidRequest, message),
        ThsError::Unsupported(_) => SourceError::try_next(FailureKind::Unsupported, message),
        ThsError::Authentication(_) | ThsError::HttpStatus(_) => {
            SourceError::try_next(FailureKind::Provider, message)
        }
        ThsError::RateLimited => SourceError::try_next(FailureKind::RateLimited, message),
        ThsError::Transport(_) => SourceError::try_next(FailureKind::Transport, message),
        ThsError::VerifiedEmpty(_) => SourceError::try_next(FailureKind::NoData, message),
        ThsError::Decode(_) | ThsError::Schema(_) | ThsError::Incomplete(_) | ThsError::Core(_) => {
            SourceError::try_next(FailureKind::Protocol, message)
        }
        ThsError::ProbeAdmission(_) => SourceError::try_next(FailureKind::Evidence, message),
    }
}

fn parse_instrument(value: &str) -> Result<InstrumentId, Box<dyn Error>> {
    let (code, suffix) = value
        .trim()
        .rsplit_once('.')
        .ok_or("consensus code must use CODE.SH/SZ/BJ")?;
    let exchange = match suffix.to_ascii_uppercase().as_str() {
        "SH" => Exchange::Shanghai,
        "SZ" => Exchange::Shenzhen,
        "BJ" => Exchange::Beijing,
        _ => return Err(format!("unsupported exchange suffix {suffix}").into()),
    };
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let values = std::env::var("MAGIC_THS_CONSENSUS_CODES").unwrap_or_else(|_| "600519.SH".into());
    let instruments = values
        .split(',')
        .map(parse_instrument)
        .collect::<Result<Vec<_>, _>>()?;
    let client = Arc::new(ThsClient::new()?);
    let mut router = ConsensusRouter::new(
        AcceptancePolicy::new()
            .with_require_complete(true)
            .with_require_source_at(true),
    );
    router.register(consensus_source(
        ProviderId::Tonghuashun,
        client,
        classify_ths,
    ))?;

    println!("operation=consensus_live");
    println!("providers={:?}", router.provider_ids());
    match router.route(&instruments) {
        Ok(outcome) => {
            println!("selected_provider={:?}", outcome.selected_provider());
            println!("records={}", outcome.batch().records().len());
            for attempt in outcome.attempts() {
                println!(
                    "attempt_provider={:?} status={:?}",
                    attempt.provider_id(),
                    attempt.status()
                );
            }
            for record in outcome.batch().records() {
                println!(
                    "stock={}.{:?} name={} estimates={} contributor_count={:?} source_at={:?} batch_id={}",
                    record.instrument.code(),
                    record.instrument.exchange(),
                    record.name,
                    record.estimates.len(),
                    record.contributor_count.map(|value| value.get()),
                    record.evidence.source_at(),
                    record.evidence.batch_id()
                );
            }
            println!("consensus_router_status=selected");
            Ok(())
        }
        Err(error) => {
            let verified_empty = error.attempts().iter().any(|attempt| {
                matches!(
                    attempt.status(),
                    AttemptStatus::Failed {
                        kind: FailureKind::NoData,
                        ..
                    }
                )
            });
            for attempt in error.attempts() {
                println!(
                    "attempt_provider={:?} status={:?}",
                    attempt.provider_id(),
                    attempt.status()
                );
            }
            println!("verified_empty_classified={verified_empty}");
            if verified_empty {
                println!("consensus_router_status=verified_empty");
                Ok(())
            } else {
                Err(Box::new(error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magic_market_core::{Provenance, SourceEvidence, VerifiedEmpty};

    #[test]
    fn classifier_stops_invalid_requests_and_preserves_verified_empty_as_no_data() {
        let invalid = classify_ths(ThsError::InvalidRequest("bad request".into()));
        assert_eq!(invalid.kind(), FailureKind::InvalidRequest);
        assert_eq!(invalid.action(), magic_market_router::FailureAction::Stop);

        let evidence =
            SourceEvidence::new(ProviderId::Tonghuashun, "observed", "empty-batch").unwrap();
        let provenance = Provenance::new("tonghuashun", "observed")
            .unwrap()
            .with_batch_id("empty-batch")
            .unwrap();
        let empty = VerifiedEmpty::new(
            "consensus",
            "600519.SH",
            "source reports empty",
            evidence,
            provenance,
        )
        .unwrap();
        let classified = classify_ths(ThsError::VerifiedEmpty(Box::new(empty)));
        assert_eq!(classified.kind(), FailureKind::NoData);
        assert_eq!(
            classified.action(),
            magic_market_router::FailureAction::TryNext
        );
    }

    #[test]
    fn classifier_exposes_rate_transport_protocol_and_evidence_failures() {
        for (error, expected) in [
            (ThsError::RateLimited, FailureKind::RateLimited),
            (ThsError::Transport("down".into()), FailureKind::Transport),
            (ThsError::Schema("drift".into()), FailureKind::Protocol),
        ] {
            assert_eq!(classify_ths(error).kind(), expected);
        }
    }
}
