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

use magic_market_core::{
    AssetClass, CapitalCapabilities, ContentCapabilities, DataBatch, Exchange, InstrumentId,
    LimitPoolCapabilities, MarketDiscoveryCapabilities, Provenance, ProviderId,
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
            announcement_discovery: false,
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
        self.finish_allow_empty(records)
    }

    pub(crate) fn finish_allow_empty<T>(
        &self,
        records: Vec<T>,
    ) -> Result<DataBatch<T>, EastmoneyError> {
        let provenance = Provenance::new(SOURCE_NAME, self.observed_at.clone())?
            .with_batch_id(self.batch_id.clone())?;
        let provenance = match &self.source_at {
            Some(source_at) => provenance.with_source_at(source_at.clone())?,
            None => provenance,
        };
        Ok(DataBatch::strict(records, provenance))
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
        Some(b'4' | b'8') => Ok(Exchange::Beijing),
        Some(b'9') if code.starts_with("920") => Ok(Exchange::Beijing),
        Some(b'9') => Err(format!(
            "Eastmoney stock code {code} uses an unverified 9-prefix exchange mapping"
        )),
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
mod tests {
    use super::{
        instrument_from_market, query_url, secid, validate_instrument, BatchContext,
        EastmoneyClient, EastmoneyError, EastmoneyTransport,
    };
    use magic_market_core::{
        AssetClass, BlockTrades, BoardCategory, BoardFlows, DividendPlans, DragonTigerData,
        DragonTigerDiscovery, DragonTigerDiscoveryRequest, Exchange, FlowInterval, FlowScope,
        FundFlowRequest, FundFlowSeries, HolderCounts, HttpsUrl, InstrumentDateRangeRequest,
        InstrumentId, InstrumentSignalRequest, IsoDate, LimitPoolKind, LimitPoolRequest,
        LimitPools, LockupEvents, MarginData, NewsProvider, NonEmptyText, PopularityData,
        PositiveU32, ProviderId, ReportScope, ResearchDocumentRequest, ResearchDocuments,
        ResearchReports, ResearchRequest,
    };

    struct RejectingTransport;

    impl EastmoneyTransport for RejectingTransport {
        fn get(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            Err(EastmoneyError::Transport("offline fixture".into()))
        }

        fn post_json(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _body: &[u8],
            _max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            Err(EastmoneyError::Transport("offline fixture".into()))
        }
    }

    #[test]
    fn query_values_are_utf8_percent_encoded() {
        assert_eq!(
            query_url(
                "https://push2.eastmoney.com/x",
                &[("filter", "电力 A".into())]
            ),
            "https://push2.eastmoney.com/x?filter=%E7%94%B5%E5%8A%9B%20A"
        );
    }

    #[test]
    fn secid_preserves_verified_exchange_routing() {
        let instrument =
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
        assert_eq!(secid(&instrument).unwrap(), "1.600396");
    }

    #[test]
    fn code_prefix_must_match_declared_and_source_exchange() {
        let mismatches = [
            (Exchange::Shanghai, "002475"),
            (Exchange::Shenzhen, "600396"),
            (Exchange::Beijing, "300001"),
        ];
        for (exchange, code) in mismatches {
            let instrument = InstrumentId::new(exchange, code, AssetClass::Equity).unwrap();
            assert!(matches!(
                validate_instrument(&instrument),
                Err(super::EastmoneyError::InvalidRequest(message))
                    if message.contains("exchange")
            ));
        }
        assert!(matches!(
            instrument_from_market("002475", 1),
            Err(super::EastmoneyError::Protocol(message))
                if message.contains("market")
        ));
    }

    #[test]
    fn only_verified_920_nine_prefix_maps_to_beijing() {
        let verified = InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap();
        assert!(validate_instrument(&verified).is_ok());

        let unverified =
            InstrumentId::new(Exchange::Beijing, "900901", AssetClass::Equity).unwrap();
        assert!(matches!(
            validate_instrument(&unverified),
            Err(super::EastmoneyError::Unsupported(message))
                if message.contains("unverified 9-prefix")
        ));
        assert!(matches!(
            instrument_from_market("900901", 0),
            Err(super::EastmoneyError::Protocol(message))
                if message.contains("unverified 9-prefix")
        ));
    }

    #[test]
    fn unverified_fund_flow_is_not_admitted_as_a_capability() {
        assert!(!EastmoneyClient::capital_capabilities().fund_flow_series);
    }

    #[test]
    fn keyword_only_instrument_news_is_not_admitted_as_a_capability() {
        assert!(!EastmoneyClient::content_capabilities().instrument_news);
        assert!(EastmoneyClient::content_capabilities().global_news);
    }

    #[test]
    fn batch_and_record_evidence_share_identity() {
        let context = BatchContext::new("fixture", Some("2026-07-23")).unwrap();
        let evidence = context.evidence().unwrap();
        let batch = context.finish(vec![1_u8]).unwrap();
        assert_eq!(evidence.provider(), ProviderId::Eastmoney);
        assert_eq!(Some(evidence.batch_id()), batch.provenance().batch_id());
        assert_eq!(evidence.source_at(), Some("2026-07-23"));
    }

    #[test]
    fn empty_batches_are_explicit_protocol_failures() {
        let context = BatchContext::new("fixture", None).unwrap();
        assert!(context.finish::<u8>(Vec::new()).is_err());
    }

    #[test]
    fn every_public_provider_entry_builds_a_bounded_request_before_transport() {
        assert!(EastmoneyClient::with_timeout(std::time::Duration::ZERO).is_err());
        assert!(format!("{:?}", EastmoneyClient::new().unwrap()).contains("EastmoneyClient"));

        let client = EastmoneyClient::with_transport(RejectingTransport);
        let instrument =
            InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
        let date = IsoDate::new("2026-07-25").unwrap();
        let one = PositiveU32::new(1).unwrap();

        for category in [
            BoardCategory::Industry,
            BoardCategory::Concept,
            BoardCategory::Region,
        ] {
            for interval in [FlowInterval::Day1, FlowInterval::Day5, FlowInterval::Day10] {
                assert!(client.board_flows(category, interval, one).is_err());
            }
        }

        for interval in [FlowInterval::Minute1, FlowInterval::Day1] {
            let request =
                FundFlowRequest::new(FlowScope::Instrument(instrument.clone()), interval, one)
                    .unwrap();
            assert!(client.fund_flow_series(&request).is_err());
        }

        let capital =
            InstrumentDateRangeRequest::new(instrument.clone(), PositiveU32::new(10).unwrap())
                .unwrap();
        assert!(client.margin_data(&capital).is_err());
        assert!(client.block_trades(&capital).is_err());
        assert!(client.holder_counts(&capital).is_err());
        assert!(client.lockup_events(&capital).is_err());
        assert!(client.dividend_plans(&capital).is_err());

        for kind in [
            LimitPoolKind::Upper,
            LimitPoolKind::Broken,
            LimitPoolKind::Lower,
            LimitPoolKind::PreviousUpper,
        ] {
            let request = LimitPoolRequest::new(kind, date.clone(), one).unwrap();
            assert!(client.limit_pool(&request).is_err());
        }

        let signal =
            InstrumentSignalRequest::new(instrument.clone(), PositiveU32::new(10).unwrap())
                .unwrap()
                .with_trading_date(date.clone());
        assert!(client.dragon_tiger_entries(&signal).is_err());
        assert!(client.dragon_tiger_seats(&signal).is_err());
        let discovery =
            DragonTigerDiscoveryRequest::new(date, PositiveU32::new(10).unwrap()).unwrap();
        assert!(client.discover_dragon_tiger(&discovery).is_err());

        assert!(client.popularity(one).is_err());
        assert!(client.global_news(one).is_err());
        let report = ResearchRequest::new(
            ReportScope::Instrument(instrument),
            one,
            PositiveU32::new(20).unwrap(),
        )
        .unwrap();
        assert!(client.research_reports(&report).is_err());
        let industry = ResearchRequest::new(
            ReportScope::Industry(NonEmptyText::new("bank").unwrap()),
            one,
            PositiveU32::new(20).unwrap(),
        )
        .unwrap();
        assert!(client.research_reports(&industry).is_err());

        let document = ResearchDocumentRequest {
            report_id: NonEmptyText::new("ABC").unwrap(),
            pdf_url: HttpsUrl::new("https://pdf.dfcfw.com/pdf/H3_ABC_1.pdf").unwrap(),
        };
        assert!(client.research_document(&document).is_err());
    }
}
