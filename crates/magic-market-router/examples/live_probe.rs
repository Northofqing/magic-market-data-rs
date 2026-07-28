use magic_market_core::{
    AssetClass, CoreError, EvidenceTimestamp, Exchange, InstrumentId, ProviderId,
};
use magic_market_router::{
    quote_source, AcceptancePolicy, FailureKind, QuoteRouter, RouteAttempt, SourceError, SourceFn,
};
use magic_tdx_rs::{TdxError, TdxSmartClient};
use magic_tencent_rs::{TencentClient, TencentError};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

fn classify_tdx(error: TdxError) -> SourceError {
    let message = error.to_string();
    match error {
        TdxError::ConnectionTimeout => SourceError::try_next(FailureKind::Timeout, message),
        TdxError::Io(_)
        | TdxError::Connection(_)
        | TdxError::SetupFailed(_)
        | TdxError::Disconnected
        | TdxError::RetryExhausted(_) => SourceError::try_next(FailureKind::Transport, message),
        TdxError::Unsupported(_) => SourceError::try_next(FailureKind::Unsupported, message),
        TdxError::Parse(_)
        | TdxError::InvalidData(_)
        | TdxError::HistoricalBarCardinality { .. }
        | TdxError::ResponseParse(_)
        | TdxError::Core(_) => SourceError::try_next(FailureKind::Protocol, message),
        TdxError::FileNotFound(_) | TdxError::Coded(_) => {
            SourceError::try_next(FailureKind::Provider, message)
        }
    }
}

fn classify_tencent(error: TencentError) -> SourceError {
    let message = error.to_string();
    match error {
        TencentError::InvalidRequest(_) => SourceError::stop(FailureKind::InvalidRequest, message),
        TencentError::Unsupported(_) => SourceError::try_next(FailureKind::Unsupported, message),
        TencentError::Transport(_) => SourceError::try_next(FailureKind::Transport, message),
        TencentError::Decode(_) | TencentError::Protocol(_) | TencentError::Core(_) => {
            SourceError::try_next(FailureKind::Protocol, message)
        }
    }
}

fn parse_instrument(value: &str) -> Result<InstrumentId, CoreError> {
    let (code, suffix) = value
        .trim()
        .rsplit_once('.')
        .ok_or_else(|| CoreError::InvalidRequest("code must use CODE.SH/SZ/BJ form".into()))?;
    let exchange = match suffix.to_ascii_uppercase().as_str() {
        "SH" => Exchange::Shanghai,
        "SZ" => Exchange::Shenzhen,
        "BJ" => Exchange::Beijing,
        other => {
            return Err(CoreError::InvalidRequest(format!(
                "unsupported exchange suffix {other}"
            )));
        }
    };
    InstrumentId::new(exchange, code, AssetClass::Equity)
}

fn print_attempt(attempt: &RouteAttempt) {
    println!(
        "attempt provider={:?} status={:?}",
        attempt.provider_id(),
        attempt.status()
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    let session =
        std::env::var("MAGIC_ROUTER_SESSION").unwrap_or_else(|_| "unspecified".to_owned());
    if session != "continuous" {
        println!("freshness_policy=not_run session={session}");
        println!(
            "router_live_probe_status=skipped_non_continuous_session \
             hint=set MAGIC_ROUTER_SESSION=continuous only during continuous trading"
        );
        return Ok(());
    }
    println!("freshness_policy=continuous_trading max_source_age_ms=5000");

    let requested = std::env::var("MAGIC_ROUTER_CODE").unwrap_or_else(|_| "600396.SH".to_owned());
    let instrument = parse_instrument(&requested)?;
    let tdx = Arc::new(TdxSmartClient::new());
    let tdx_source = match tdx.connect_to_any(Some(5.0)) {
        Ok(true) => quote_source(ProviderId::Tdx, Arc::clone(&tdx), classify_tdx),
        Ok(false) => SourceFn::new(ProviderId::Tdx, |_| {
            Err(SourceError::try_next(
                FailureKind::Transport,
                "TDX connect_to_any returned false",
            ))
        }),
        Err(error) => {
            let message = error.to_string();
            SourceFn::new(ProviderId::Tdx, move |_| {
                Err(SourceError::try_next(
                    FailureKind::Transport,
                    message.clone(),
                ))
            })
        }
    };
    let tencent = Arc::new(TencentClient::new()?);

    let policy = AcceptancePolicy::new()
        .with_require_complete(true)
        .with_max_source_age(Duration::from_secs(5))?;
    let mut router = QuoteRouter::new(policy);
    router.register(tdx_source)?;
    router.register(quote_source(ProviderId::Tencent, tencent, classify_tencent))?;

    println!(
        "router providers={:?} require_complete={} require_source_at={} max_source_age={:?}",
        router.provider_ids(),
        router.policy().require_complete(),
        router.policy().require_source_at(),
        router.policy().max_source_age(),
    );
    let outcome = match router.route(std::slice::from_ref(&instrument)) {
        Ok(outcome) => outcome,
        Err(error) => {
            for attempt in error.attempts() {
                print_attempt(attempt);
            }
            return Err(Box::new(error));
        }
    };
    for attempt in outcome.attempts() {
        print_attempt(attempt);
    }
    println!(
        "selected_provider={:?} records={} provenance={:?} quality={:?}",
        outcome.selected_provider(),
        outcome.batch().records().len(),
        outcome.batch().provenance(),
        outcome.batch().quality()
    );
    for quote in outcome.batch().records() {
        let source_at = quote
            .source_at()
            .ok_or("strict quote route selected a record without source_at")?;
        let source_age = EvidenceTimestamp::parse_instant(quote.observed_at())?
            .duration_since(EvidenceTimestamp::parse_instant(source_at)?)
            .ok_or("strict quote route selected a future source_at")?;
        println!(
            "quote code={} price={} source_at={} observed_at={} source_age_ms={} provider={:?} batch_id={}",
            quote.instrument().code(),
            quote.price().get(),
            source_at,
            quote.observed_at(),
            source_age.as_millis(),
            quote.provider(),
            quote.batch_id()
        );
    }
    if outcome.batch().records().is_empty() {
        return Err("router selected an empty Quote batch".into());
    }
    println!("router_live_probe_status=passed");
    Ok(())
}
