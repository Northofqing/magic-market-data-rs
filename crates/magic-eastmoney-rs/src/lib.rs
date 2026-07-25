#![forbid(unsafe_code)]
//! Read-only adapter for explicitly bounded Eastmoney public-web endpoints.
//!
//! These endpoints do not publish a project-visible stability SLA. The adapter
//! never reads cookies or account data, never impersonates Choice/EMQuant, and
//! advertises only families backed by strict parsers and probes.

mod board_flow;
mod capital;
mod datacenter_api;
mod discovery;
mod dragon_tiger;
mod error;
mod fund_flow;
mod limit_pool;
mod mapping;
mod news;
mod popularity;
mod post_close;
mod reports;
mod transport;

#[cfg(test)]
#[path = "../tests/internal/support.rs"]
mod test_support;

use magic_market_core::{
    AssetClass, CapitalCapabilities, ContentCapabilities, DataBatch, Exchange, InstrumentId,
    LimitPoolCapabilities, LoadProbeSnapshot, MarketDiscoveryCapabilities, Provenance, ProviderId,
    ResearchCapabilities, SignalCapabilities, SourceEvidence,
};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use error::EastmoneyError;
pub use transport::EastmoneyTransport;
use transport::{
    HttpsTransport, DEFAULT_MAX_RESPONSE_BYTES, MAX_HTML_RESPONSE_BYTES, MAX_PDF_RESPONSE_BYTES,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(12);
const SOURCE_NAME: &str = "eastmoney-web";

/// Public-web Eastmoney client. Clones share a pooled, rate-limited transport.
#[derive(Clone)]
pub struct EastmoneyClient {
    pub(crate) transport: Arc<dyn EastmoneyTransport>,
}

impl std::fmt::Debug for EastmoneyClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EastmoneyClient")
            .finish_non_exhaustive()
    }
}

impl EastmoneyClient {
    /// Creates the production client with bounded timeouts and one request/second pacing.
    pub fn new() -> Result<Self, EastmoneyError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    /// Creates the production client with a caller-selected positive timeout.
    pub fn with_timeout(timeout: Duration) -> Result<Self, EastmoneyError> {
        Ok(Self {
            transport: Arc::new(HttpsTransport::new(timeout)?),
        })
    }

    /// Creates a client backed by deterministic injected fixtures.
    pub fn with_transport(transport: impl EastmoneyTransport + 'static) -> Self {
        Self {
            transport: Arc::new(transport),
        }
    }

    pub fn load_probe_snapshot(&self) -> Result<LoadProbeSnapshot, EastmoneyError> {
        self.transport.load_probe_snapshot().ok_or_else(|| {
            EastmoneyError::Unsupported(
                "request-start telemetry is unavailable for the configured transport".into(),
            )
        })
    }

    /// Capabilities proved for research-report endpoints.
    pub const fn research_capabilities() -> ResearchCapabilities {
        ResearchCapabilities {
            reports: true,
            consensus: false,
            semantic_search: false,
            pdf_download: true,
            document_body: true,
        }
    }

    /// Capabilities proved for capital-flow and datacenter endpoints.
    pub const fn capital_capabilities() -> CapitalCapabilities {
        CapitalCapabilities {
            fund_flow_series: false,
            board_flow: true,
            margin: true,
            block_trades: true,
            holder_count: true,
            lockups: true,
            dividends: true,
            post_close_flow: true,
            northbound_daily_statistics: false,
        }
    }

    /// Capabilities proved for bounded public signal endpoints.
    pub const fn signal_capabilities() -> SignalCapabilities {
        SignalCapabilities {
            board_memberships: false,
            strong_stock_reasons: false,
            dragon_tiger: true,
            market_rankings: false,
            popularity: true,
            concept_hits: false,
        }
    }

    /// Capabilities proved for public limit-pool endpoints.
    pub const fn limit_pool_capabilities() -> LimitPoolCapabilities {
        LimitPoolCapabilities {
            upper: true,
            broken: true,
            lower: true,
            previous_upper: true,
            reasons: false,
        }
    }

    /// Capabilities proved for Eastmoney content search.
    pub const fn content_capabilities() -> ContentCapabilities {
        ContentCapabilities {
            instrument_news: false,
            global_news: true,
            announcements: false,
            market_announcements: false,
            investor_questions: false,
        }
    }

    /// Capabilities proved for complete market-intelligence discovery.
    pub const fn market_discovery_capabilities() -> MarketDiscoveryCapabilities {
        MarketDiscoveryCapabilities {
            dragon_tiger_discovery: true,
            board_directory: false,
            board_memberships: false,
            board_constituents: false,
        }
    }

    pub(crate) fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.transport.get(url, headers, DEFAULT_MAX_RESPONSE_BYTES)
    }

    pub(crate) fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.transport
            .post_json(url, headers, body, DEFAULT_MAX_RESPONSE_BYTES)
    }

    pub(crate) fn get_pdf(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.transport.get_pdf(url, headers, MAX_PDF_RESPONSE_BYTES)
    }

    pub(crate) fn get_html(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.transport
            .get_html(url, headers, MAX_HTML_RESPONSE_BYTES)
    }
}

pub(crate) struct BatchContext {
    observed_at: String,
    batch_id: String,
    source_at: Option<String>,
}

impl BatchContext {
    pub(crate) fn new(family: &str, source_at: Option<&str>) -> Result<Self, EastmoneyError> {
        if family.is_empty()
            || !family
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(EastmoneyError::InvalidRequest(
                "batch family must be non-empty lowercase ASCII".into(),
            ));
        }
        let observed_at = observed_at()?;
        let batch_id = format!("{SOURCE_NAME}:{family}:{observed_at}");
        Ok(Self {
            observed_at,
            batch_id,
            source_at: source_at.map(str::to_owned),
        })
    }

    pub(crate) fn evidence(&self) -> Result<SourceEvidence, EastmoneyError> {
        self.evidence_at(self.source_at.as_deref())
    }

    pub(crate) fn evidence_at(
        &self,
        source_at: Option<&str>,
    ) -> Result<SourceEvidence, EastmoneyError> {
        let evidence = SourceEvidence::new(
            ProviderId::Eastmoney,
            self.observed_at.clone(),
            self.batch_id.clone(),
        )?;
        match source_at {
            Some(source_at) => Ok(evidence.with_source_at(source_at)?),
            None => Ok(evidence),
        }
    }

    pub(crate) fn finish<T>(&self, records: Vec<T>) -> Result<DataBatch<T>, EastmoneyError> {
        if records.is_empty() {
            return Err(EastmoneyError::Protocol(
                "Eastmoney response contains no usable records".into(),
            ));
        }
        self.finish_with_issues(records, Vec::new())
    }

    pub(crate) fn finish_allow_empty<T>(
        &self,
        records: Vec<T>,
    ) -> Result<DataBatch<T>, EastmoneyError> {
        self.finish_with_issues(records, Vec::new())
    }

    /// Finishes a source-counted family that can prove an empty response and
    /// can explicitly report a caller-truncated page.
    pub(crate) fn finish_with_issues<T>(
        &self,
        records: Vec<T>,
        issues: Vec<String>,
    ) -> Result<DataBatch<T>, EastmoneyError> {
        let provenance = Provenance::new(SOURCE_NAME, self.observed_at.clone())?
            .with_batch_id(self.batch_id.clone())?;
        let provenance = match &self.source_at {
            Some(source_at) => provenance.with_source_at(source_at.clone())?,
            None => provenance,
        };
        if issues.is_empty() {
            Ok(DataBatch::strict(records, provenance))
        } else {
            Ok(DataBatch::best_effort(records, provenance, issues)?)
        }
    }
}

pub(crate) fn query_url(base: &str, params: &[(&str, String)]) -> String {
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{base}?{query}")
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(*byte));
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

pub(crate) fn validate_instrument(instrument: &InstrumentId) -> Result<(), EastmoneyError> {
    if instrument.asset_class() != AssetClass::Equity {
        return Err(EastmoneyError::Unsupported(format!(
            "asset class {:?} is not verified by this Eastmoney endpoint family",
            instrument.asset_class()
        )));
    }
    if instrument.code().len() != 6 || !instrument.code().bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(EastmoneyError::InvalidRequest(format!(
            "{} must be a six-digit A-share code",
            instrument.code()
        )));
    }
    let expected_exchange =
        exchange_for_code(instrument.code()).map_err(EastmoneyError::Unsupported)?;
    if instrument.exchange() != expected_exchange {
        return Err(EastmoneyError::InvalidRequest(format!(
            "Eastmoney code {} implies {expected_exchange:?} exchange, not {:?}",
            instrument.code(),
            instrument.exchange()
        )));
    }
    Ok(())
}

pub(crate) fn secid(instrument: &InstrumentId) -> Result<String, EastmoneyError> {
    validate_instrument(instrument)?;
    let market = match instrument.exchange() {
        Exchange::Shanghai => "1",
        Exchange::Shenzhen => "0",
        Exchange::Beijing => {
            return Err(EastmoneyError::Unsupported(
                "Beijing secid routing is not verified for this endpoint family".into(),
            ))
        }
    };
    Ok(format!("{market}.{}", instrument.code()))
}

pub(crate) fn instrument_from_market(
    code: &str,
    market: i64,
) -> Result<InstrumentId, EastmoneyError> {
    let exchange = source_exchange_for_code(code)?;
    let expected_market = match exchange {
        Exchange::Shanghai => 1,
        Exchange::Shenzhen | Exchange::Beijing => 0,
    };
    if market != expected_market {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney source code {code} implies market {expected_market}, not {market}"
        )));
    }
    source_instrument(code, exchange)
}

pub(crate) fn source_instrument(
    code: &str,
    exchange: Exchange,
) -> Result<InstrumentId, EastmoneyError> {
    let expected_exchange = source_exchange_for_code(code)?;
    if exchange != expected_exchange {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney source code {code} implies {expected_exchange:?} exchange, not {exchange:?}"
        )));
    }
    Ok(InstrumentId::new(exchange, code, AssetClass::Equity)?)
}

pub(crate) fn validate_source_instrument(
    expected: &InstrumentId,
    source_code: &str,
    source_exchange: Option<Exchange>,
) -> Result<(), EastmoneyError> {
    let actual = source_instrument(
        source_code,
        source_exchange.unwrap_or(source_exchange_for_code(source_code)?),
    )?;
    if actual.code() != expected.code() || actual.exchange() != expected.exchange() {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney source instrument {:?}.{} does not match requested {:?}.{}",
            actual.exchange(),
            actual.code(),
            expected.exchange(),
            expected.code()
        )));
    }
    Ok(())
}

pub(crate) fn validate_source_secucode(
    expected: &InstrumentId,
    secucode: &str,
) -> Result<(), EastmoneyError> {
    let (code, suffix) = secucode.split_once('.').ok_or_else(|| {
        EastmoneyError::Protocol(format!(
            "Eastmoney source SECUCODE {secucode:?} has no exchange suffix"
        ))
    })?;
    let exchange = match suffix.to_ascii_uppercase().as_str() {
        "SH" => Exchange::Shanghai,
        "SZ" => Exchange::Shenzhen,
        "BJ" => Exchange::Beijing,
        _ => {
            return Err(EastmoneyError::Protocol(format!(
                "unsupported Eastmoney SECUCODE suffix {suffix:?}"
            )))
        }
    };
    validate_source_instrument(expected, code, Some(exchange))
}

fn exchange_for_code(code: &str) -> Result<Exchange, String> {
    match code.as_bytes().first().copied() {
        Some(b'6') => Ok(Exchange::Shanghai),
        Some(b'0' | b'3') => Ok(Exchange::Shenzhen),
        Some(b'4' | b'8' | b'9') => Ok(Exchange::Beijing),
        Some(prefix) => Err(format!(
            "Eastmoney stock-code prefix {:?} has no verified exchange mapping",
            char::from(prefix)
        )),
        None => Err("Eastmoney stock code is empty".into()),
    }
}

fn source_exchange_for_code(code: &str) -> Result<Exchange, EastmoneyError> {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney source returned invalid stock code {code:?}"
        )));
    }
    exchange_for_code(code).map_err(EastmoneyError::Protocol)
}

fn observed_at() -> Result<String, EastmoneyError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            EastmoneyError::Transport(format!("system clock predates Unix epoch: {error}"))
        })?
        .as_millis();
    Ok(format!("unix-ms:{millis}"))
}

#[cfg(test)]
#[path = "../tests/internal/lib_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/internal/discovery_and_news_regression_tests.rs"]
mod discovery_and_news_regression_tests;
