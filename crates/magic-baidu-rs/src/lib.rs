#![forbid(unsafe_code)]
//! Bounded read-only adapter for Baidu Stock Connect technical daily bars.

use magic_market_core::{
    Adjustment, AssetClass, Bar, BarInterval, BarsRequest, Capabilities, DataBatch, Exchange,
    HistoricalBars, InstrumentId, Money, Price, Provenance, ProviderId, Quantity, SourceEvidence,
    TechnicalBar, TechnicalBarsProvider,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const ENDPOINT: &str = "https://finance.pae.baidu.com/selfselect/getstockquotation";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ROWS: u16 = 2001;
const SHARES_PER_LOT: f64 = 100.0;
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Error)]
pub enum BaiduError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("Baidu response decoding failed: {0}")]
    Decode(String),
    #[error("Baidu protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    url: String,
    headers: Vec<(String, String)>,
}

impl HttpRequest {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }
}

pub trait BaiduTransport: Send + Sync {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, BaiduError>;
}

#[derive(Clone)]
struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    fn new(timeout: Duration) -> Result<Self, BaiduError> {
        if timeout.is_zero() {
            return Err(BaiduError::InvalidRequest(
                "timeout must be greater than zero".into(),
            ));
        }
        Ok(Self {
            agent: ureq::AgentBuilder::new()
                .timeout_connect(timeout)
                .timeout_read(timeout)
                .timeout_write(timeout)
                .redirects(0)
                .build(),
        })
    }
}

impl BaiduTransport for HttpsTransport {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, BaiduError> {
        ensure_official_url(request.url())?;
        let mut call = self.agent.get(request.url());
        for (name, value) in request.headers() {
            call = call.set(name, value);
        }
        let response = call
            .call()
            .map_err(|error| BaiduError::Transport(error.to_string()))?;
        if response.status() != 200 {
            return Err(BaiduError::Transport(format!(
                "unexpected HTTP status {}",
                response.status()
            )));
        }
        ensure_json_content_type(response.header("Content-Type"))?;
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|error| BaiduError::Transport(error.to_string()))?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(BaiduError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        Ok(body)
    }
}

#[derive(Clone)]
pub struct BaiduClient {
    transport: Arc<dyn BaiduTransport>,
    minimum_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
}

impl std::fmt::Debug for BaiduClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BaiduClient")
            .finish_non_exhaustive()
    }
}

impl BaiduClient {
    pub fn new() -> Result<Self, BaiduError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, BaiduError> {
        Ok(Self::from_parts(
            Arc::new(HttpsTransport::new(timeout)?),
            MINIMUM_REQUEST_INTERVAL,
        ))
    }

    pub fn with_transport(transport: impl BaiduTransport + 'static) -> Self {
        Self::from_parts(Arc::new(transport), Duration::ZERO)
    }

    fn from_parts(transport: Arc<dyn BaiduTransport>, minimum_interval: Duration) -> Self {
        Self {
            transport,
            minimum_interval,
            request_gate: Arc::new(Mutex::new(None)),
        }
    }

    pub const fn capabilities() -> Capabilities {
        Capabilities {
            bars: true,
            ..Capabilities::new()
        }
    }

    fn fetch(&self, request: &BarsRequest) -> Result<DataBatch<TechnicalBar>, BaiduError> {
        validate_request(request)?;
        let http_request = build_request(request.instrument())?;
        let body = self.execute(&http_request)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(BaiduError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        parse_response(&body, request, &now()?)
    }

    fn execute(&self, request: &HttpRequest) -> Result<Vec<u8>, BaiduError> {
        let mut last_started = self
            .request_gate
            .lock()
            .map_err(|_| BaiduError::Transport("request gate lock poisoned".into()))?;
        if let Some(previous) = *last_started {
            let elapsed = previous.elapsed();
            if elapsed < self.minimum_interval {
                thread::sleep(self.minimum_interval - elapsed);
            }
        }
        *last_started = Some(Instant::now());
        let result = self.transport.get(request);
        drop(last_started);
        result
    }
}

impl TechnicalBarsProvider for BaiduClient {
    type Error = BaiduError;

    fn technical_bars(
        &self,
        request: &BarsRequest,
    ) -> Result<DataBatch<TechnicalBar>, Self::Error> {
        self.fetch(request)
    }
}

impl HistoricalBars for BaiduClient {
    type Bar = Bar;
    type Error = BaiduError;

    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Self::Bar>, Self::Error> {
        let technical = self.fetch(request)?;
        let provenance = technical.provenance().clone();
        let records = technical
            .records()
            .iter()
            .map(|record| record.bar().clone())
            .collect();
        Ok(DataBatch::strict(records, provenance))
    }
}

fn validate_request(request: &BarsRequest) -> Result<(), BaiduError> {
    if request.instrument().asset_class() != AssetClass::Equity {
        return Err(BaiduError::Unsupported(
            "Baidu technical bars are verified only for A-share equities".into(),
        ));
    }
    validate_instrument_identity(request.instrument())?;
    if request.interval() != BarInterval::Day {
        return Err(BaiduError::Unsupported(
            "Baidu quotation_kline_ab is verified only for daily bars".into(),
        ));
    }
    if request.limit() > MAX_ROWS {
        return Err(BaiduError::InvalidRequest(format!(
            "Baidu accepts at most {MAX_ROWS} rows"
        )));
    }
    if request.start().is_some() || request.end().is_some() {
        return Err(BaiduError::Unsupported(
            "Baidu date-range selector has not been verified; use the bounded trailing limit"
                .into(),
        ));
    }
    Ok(())
}

fn build_request(instrument: &InstrumentId) -> Result<HttpRequest, BaiduError> {
    validate_instrument_identity(instrument)?;
    let code = instrument.code();
    Ok(HttpRequest {
        url: format!(
            "{ENDPOINT}?all=1&isIndex=false&isBk=false&isBlock=false&isFutures=false&isStock=true&newFormat=1&group=quotation_kline_ab&finClientType=pc&code={code}&start_time=&ktype=1"
        ),
        headers: vec![
            (
                "Accept".into(),
                "application/vnd.finance-web.v1+json".into(),
            ),
            ("Origin".into(), "https://gushitong.baidu.com".into()),
            ("Referer".into(), "https://gushitong.baidu.com/".into()),
            ("User-Agent".into(), "magic-baidu-rs/0.2".into()),
        ],
    })
}

fn validate_instrument_identity(instrument: &InstrumentId) -> Result<(), BaiduError> {
    let code = instrument.code();
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(BaiduError::InvalidRequest(
            "Baidu equity code must contain exactly six digits".into(),
        ));
    }
    let expected = match code.as_bytes()[0] {
        b'6' => Exchange::Shanghai,
        b'0' | b'3' => Exchange::Shenzhen,
        b'4' | b'8' => Exchange::Beijing,
        b'9' if code.starts_with("920") => Exchange::Beijing,
        _ => {
            return Err(BaiduError::InvalidRequest(format!(
                "Baidu equity code has an unsupported A-share prefix: {code}"
            )))
        }
    };
    if instrument.exchange() != expected {
        return Err(BaiduError::InvalidRequest(format!(
            "Baidu equity code {code} belongs to {expected:?}, not {:?}",
            instrument.exchange()
        )));
    }
    Ok(())
}

fn ensure_official_url(url: &str) -> Result<(), BaiduError> {
    let valid = url
        .strip_prefix("https://finance.pae.baidu.com/")
        .is_some_and(|path| !path.is_empty() && !path.starts_with('/'));
    if !valid || url.chars().any(|character| character.is_control()) {
        return Err(BaiduError::InvalidRequest(
            "Baidu transport only permits https://finance.pae.baidu.com/".into(),
        ));
    }
    Ok(())
}

fn ensure_json_content_type(content_type: Option<&str>) -> Result<(), BaiduError> {
    if content_type.is_some_and(|value| value.to_ascii_lowercase().contains("json")) {
        Ok(())
    } else {
        Err(BaiduError::Protocol(format!(
            "expected a JSON response, received content type {content_type:?}"
        )))
    }
}

fn parse_response(
    body: &[u8],
    request: &BarsRequest,
    observed_at: &str,
) -> Result<DataBatch<TechnicalBar>, BaiduError> {
    let root: Value = serde_json::from_slice(body)
        .map_err(|error| BaiduError::Decode(format!("technical-bar JSON: {error}")))?;
    let result_code = root
        .get("ResultCode")
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| value.as_i64().map(|code| code.to_string()))
        })
        .ok_or_else(|| BaiduError::Protocol("ResultCode is missing".into()))?;
    if result_code != "0" {
        let message = root
            .get("ResultMsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown Baidu error");
        return Err(BaiduError::Protocol(format!(
            "Baidu returned ResultCode {result_code}: {message}"
        )));
    }
    let market = root
        .pointer("/Result/newMarketData")
        .and_then(Value::as_object)
        .ok_or_else(|| BaiduError::Protocol("Result.newMarketData must be an object".into()))?;
    let keys = market
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| BaiduError::Protocol("newMarketData.keys must be an array".into()))?;
    let index = build_key_index(keys)?;
    let encoded_rows = market
        .get("marketData")
        .and_then(Value::as_str)
        .ok_or_else(|| BaiduError::Protocol("newMarketData.marketData must be a string".into()))?;
    let rows = encoded_rows
        .split(';')
        .filter(|row| !row.trim().is_empty())
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Err(BaiduError::Protocol(
            "Baidu returned an empty technical-bar batch".into(),
        ));
    }
    if rows.len() > usize::from(MAX_ROWS) {
        return Err(BaiduError::Protocol(format!(
            "Baidu returned {} rows; maximum is {MAX_ROWS}",
            rows.len()
        )));
    }
    let selected = &rows[rows.len().saturating_sub(usize::from(request.limit()))..];
    let batch_id = format!("baidu-pae:{observed_at}:technical-bars");
    let mut records = Vec::with_capacity(selected.len());
    let mut seen = HashSet::with_capacity(selected.len());
    let mut previous_date: Option<String> = None;
    for row in selected {
        let fields = row.split(',').collect::<Vec<_>>();
        if fields.len() != keys.len() {
            return Err(BaiduError::Protocol(format!(
                "technical-bar row has {} fields but keys has {}",
                fields.len(),
                keys.len()
            )));
        }
        let date = field(&fields, &index, "time")?.trim();
        validate_date(date)?;
        if !seen.insert(date.to_owned())
            || previous_date
                .as_deref()
                .is_some_and(|previous| previous >= date)
        {
            return Err(BaiduError::Protocol(
                "technical-bar rows are duplicated or unordered".into(),
            ));
        }
        previous_date = Some(date.to_owned());
        let open = positive(field(&fields, &index, "open")?, "open")?;
        let close = positive(field(&fields, &index, "close")?, "close")?;
        let high = positive(field(&fields, &index, "high")?, "high")?;
        let low = positive(field(&fields, &index, "low")?, "low")?;
        if low > open.min(close) || high < open.max(close) || low > high {
            return Err(BaiduError::Protocol(
                "technical-bar OHLC values have an inconsistent range".into(),
            ));
        }
        let volume_shares = nonnegative(field(&fields, &index, "volume")?, "volume")?;
        let amount = nonnegative(field(&fields, &index, "amount")?, "amount")?;
        let evidence = SourceEvidence::new(ProviderId::Baidu, observed_at, batch_id.clone())?
            .with_source_at(date)?;
        let bar = Bar::new(
            request.instrument().clone(),
            BarInterval::Day,
            date,
            date,
            Price::new(open)?,
            Price::new(high)?,
            Price::new(low)?,
            Price::new(close)?,
            Quantity::new(volume_shares / SHARES_PER_LOT)?,
            Some(Money::new(amount)?),
            Adjustment::Unadjusted,
            ProviderId::Baidu,
            batch_id.clone(),
        )?
        .with_source_at(date)?;
        records.push(TechnicalBar::new(
            bar,
            optional_positive(field(&fields, &index, "ma5avgprice")?, "ma5avgprice")?
                .map(Price::new)
                .transpose()?,
            optional_positive(field(&fields, &index, "ma10avgprice")?, "ma10avgprice")?
                .map(Price::new)
                .transpose()?,
            optional_positive(field(&fields, &index, "ma20avgprice")?, "ma20avgprice")?
                .map(Price::new)
                .transpose()?,
            evidence,
        )?);
    }
    let source_at = records
        .last()
        .and_then(|record| record.evidence().source_at())
        .ok_or_else(|| {
            BaiduError::Protocol("latest technical-bar source time is missing".into())
        })?;
    let provenance = Provenance::new("baidu-pae", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn build_key_index(keys: &[Value]) -> Result<HashMap<&str, usize>, BaiduError> {
    let mut index = HashMap::with_capacity(keys.len());
    for (position, key) in keys.iter().enumerate() {
        let key = key
            .as_str()
            .ok_or_else(|| BaiduError::Protocol("technical-bar key must be a string".into()))?;
        if index.insert(key, position).is_some() {
            return Err(BaiduError::Protocol(format!(
                "duplicate technical-bar key {key}"
            )));
        }
    }
    for required in [
        "time",
        "open",
        "close",
        "high",
        "low",
        "volume",
        "amount",
        "ma5avgprice",
        "ma10avgprice",
        "ma20avgprice",
    ] {
        if !index.contains_key(required) {
            return Err(BaiduError::Protocol(format!(
                "technical-bar key {required} is missing"
            )));
        }
    }
    Ok(index)
}

fn field<'a>(
    fields: &'a [&str],
    index: &HashMap<&str, usize>,
    name: &'static str,
) -> Result<&'a str, BaiduError> {
    index
        .get(name)
        .and_then(|position| fields.get(*position))
        .copied()
        .ok_or_else(|| BaiduError::Protocol(format!("technical-bar field {name} is missing")))
}

fn positive(value: &str, field: &'static str) -> Result<f64, BaiduError> {
    let value = finite(value, field)?;
    if value <= 0.0 {
        return Err(BaiduError::Protocol(format!(
            "technical-bar {field} must be positive"
        )));
    }
    Ok(value)
}

fn nonnegative(value: &str, field: &'static str) -> Result<f64, BaiduError> {
    let value = finite(value, field)?;
    if value < 0.0 {
        return Err(BaiduError::Protocol(format!(
            "technical-bar {field} must be non-negative"
        )));
    }
    Ok(value)
}

fn optional_positive(value: &str, field: &'static str) -> Result<Option<f64>, BaiduError> {
    if value.trim().is_empty() || value.trim() == "--" {
        return Ok(None);
    }
    positive(value, field).map(Some)
}

fn finite(value: &str, field: &'static str) -> Result<f64, BaiduError> {
    let parsed = value.trim().parse::<f64>().map_err(|_| {
        BaiduError::Protocol(format!("technical-bar {field} is not numeric: {value:?}"))
    })?;
    if !parsed.is_finite() {
        return Err(BaiduError::Protocol(format!(
            "technical-bar {field} must be finite"
        )));
    }
    Ok(parsed)
}

fn validate_date(value: &str) -> Result<(), BaiduError> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return Err(BaiduError::Protocol(format!(
            "technical-bar time is not YYYY-MM-DD: {value:?}"
        )));
    }
    Ok(())
}

fn now() -> Result<String, BaiduError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| BaiduError::Protocol(format!("system clock precedes UNIX epoch: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use magic_market_core::{Exchange, TechnicalBarsProvider};
    use std::sync::{mpsc, Mutex};

    const FIXTURE: &str = r#"{
      "ResultCode": "0",
      "Result": {"newMarketData": {
        "keys": ["timestamp","time","open","close","volume","high","low","amount","ma5avgprice","ma10avgprice","ma20avgprice"],
        "marketData": "1784649600,2026-07-22,14.92,14.92,10000,15.00,14.80,149200.00,--,--,--;1784736000,2026-07-23,15.30,16.41,341780059,16.41,14.85,5352355411.00,13.87,13.02,13.40"
      }}
    }"#;
    const EX_DIVIDEND_FIXTURE: &str = r#"{
      "ResultCode": "0",
      "Result": {"newMarketData": {
        "keys": ["time","open","close","volume","high","low","amount","preClose","range","ma5avgprice","ma10avgprice","ma20avgprice"],
        "marketData": "2026-06-25,1180.00,1184.08,100,1190.00,1170.00,118408.00,1180.00,4.08,1181.00,1179.00,1175.00;2026-06-26,1168.00,1168.63,100,1175.00,1160.00,116863.00,1184.08,-15.45,1178.00,1177.00,1174.00"
      }}
    }"#;

    #[derive(Debug)]
    struct FixtureTransport {
        response: Vec<u8>,
        request: Mutex<Option<HttpRequest>>,
    }

    impl BaiduTransport for FixtureTransport {
        fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, BaiduError> {
            *self
                .request
                .lock()
                .map_err(|_| BaiduError::Transport("fixture lock poisoned".into()))? =
                Some(request.clone());
            Ok(self.response.clone())
        }
    }

    #[derive(Debug)]
    struct BlockingTransport {
        response: Vec<u8>,
        starts: mpsc::Sender<Instant>,
        releases: Mutex<mpsc::Receiver<()>>,
    }

    impl BaiduTransport for BlockingTransport {
        fn get(&self, _request: &HttpRequest) -> Result<Vec<u8>, BaiduError> {
            self.starts
                .send(Instant::now())
                .map_err(|error| BaiduError::Transport(error.to_string()))?;
            self.releases
                .lock()
                .map_err(|_| BaiduError::Transport("release lock poisoned".into()))?
                .recv()
                .map_err(|error| BaiduError::Transport(error.to_string()))?;
            Ok(self.response.clone())
        }
    }

    fn request(limit: u16) -> BarsRequest {
        BarsRequest::new(
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)
                .expect("instrument"),
            BarInterval::Day,
            limit,
        )
        .expect("request")
    }

    #[test]
    fn maps_unadjusted_source_ma_and_trailing_limit() {
        let client = BaiduClient::with_transport(FixtureTransport {
            response: FIXTURE.as_bytes().to_vec(),
            request: Mutex::new(None),
        });
        let batch = client.technical_bars(&request(1)).expect("fixture parses");
        assert_eq!(batch.records().len(), 1);
        let technical = &batch.records()[0];
        assert_eq!(technical.bar().bar_start(), "2026-07-23");
        assert_eq!(technical.bar().adjustment(), Adjustment::Unadjusted);
        assert_eq!(technical.bar().volume().get(), 3_417_800.59);
        assert_eq!(
            technical.bar().amount().map(Money::get),
            Some(5_352_355_411.0)
        );
        assert_eq!(technical.ma5().map(Price::get), Some(13.87));
        assert_eq!(technical.ma10().map(Price::get), Some(13.02));
        assert_eq!(technical.ma20().map(Price::get), Some(13.40));
        assert_eq!(technical.evidence().provider(), ProviderId::Baidu);
        assert_eq!(technical.evidence().source_at(), Some("2026-07-23"));
        assert_eq!(batch.provenance().source(), "baidu-pae");
    }

    #[test]
    fn source_missing_ma_is_preserved_as_none() {
        let batch =
            parse_response(FIXTURE.as_bytes(), &request(2), "observed").expect("fixture parses");
        assert_eq!(batch.records()[0].ma5(), None);
        assert_eq!(batch.records()[0].ma10(), None);
        assert_eq!(batch.records()[0].ma20(), None);
    }

    #[test]
    fn real_ex_dividend_gap_remains_unadjusted() {
        let request = BarsRequest::new(
            InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity)
                .expect("instrument"),
            BarInterval::Day,
            2,
        )
        .expect("request");
        let batch = parse_response(EX_DIVIDEND_FIXTURE.as_bytes(), &request, "observed")
            .expect("real-shape ex-dividend fixture parses");
        assert_eq!(batch.records()[0].bar().close().get(), 1184.08);
        assert_eq!(batch.records()[1].bar().close().get(), 1168.63);
        assert_eq!(
            batch.records()[1].bar().adjustment(),
            Adjustment::Unadjusted
        );
    }

    #[test]
    fn numeric_result_code_and_protocol_errors_are_checked() {
        let numeric = FIXTURE.replace("\"ResultCode\": \"0\"", "\"ResultCode\": 0");
        assert!(parse_response(numeric.as_bytes(), &request(1), "observed").is_ok());
        let denied = FIXTURE.replace("\"ResultCode\": \"0\"", "\"ResultCode\": \"10003\"");
        assert!(matches!(
            parse_response(denied.as_bytes(), &request(1), "observed"),
            Err(BaiduError::Protocol(_))
        ));
    }

    #[test]
    fn bounds_interval_and_host_are_explicit() {
        let minute = BarsRequest::new(
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)
                .expect("instrument"),
            BarInterval::Minute1,
            1,
        )
        .expect("request");
        assert!(matches!(
            validate_request(&minute),
            Err(BaiduError::Unsupported(_))
        ));
        assert!(ensure_official_url("https://finance.pae.baidu.com/x").is_ok());
        assert!(ensure_official_url("https://finance.pae.baidu.com.evil.test/x").is_err());
        assert!(ensure_json_content_type(Some("application/json; charset=utf-8")).is_ok());
        assert!(ensure_json_content_type(Some("text/html")).is_err());
        assert!(ensure_json_content_type(None).is_err());
        let oversized = BaiduClient::with_transport(FixtureTransport {
            response: vec![b' '; MAX_RESPONSE_BYTES + 1],
            request: Mutex::new(None),
        })
        .technical_bars(&request(1))
        .expect_err("injected transports cannot bypass the response cap");
        assert!(matches!(oversized, BaiduError::Protocol(_)));
    }

    #[test]
    fn exchange_must_match_the_a_share_code_prefix() {
        let mismatched = BarsRequest::new(
            InstrumentId::new(Exchange::Shanghai, "002475", AssetClass::Equity)
                .expect("core permits provider-specific validation"),
            BarInterval::Day,
            1,
        )
        .expect("request");
        assert!(matches!(
            validate_request(&mismatched),
            Err(BaiduError::InvalidRequest(_))
        ));

        let verified_beijing = BarsRequest::new(
            InstrumentId::new(Exchange::Beijing, "920001", AssetClass::Equity)
                .expect("verified Beijing identity"),
            BarInterval::Day,
            1,
        )
        .expect("request");
        assert!(validate_request(&verified_beijing).is_ok());

        let unverified_nine_prefix = BarsRequest::new(
            InstrumentId::new(Exchange::Shanghai, "900901", AssetClass::Equity)
                .expect("core permits provider-specific validation"),
            BarInterval::Day,
            1,
        )
        .expect("request");
        assert!(matches!(
            validate_request(&unverified_nine_prefix),
            Err(BaiduError::InvalidRequest(message)) if message.contains("unsupported")
        ));
    }

    #[test]
    fn cloned_clients_share_a_gate_held_through_the_complete_transport_call() {
        let (starts_tx, starts_rx) = mpsc::channel();
        let (releases_tx, releases_rx) = mpsc::channel();
        let interval = Duration::from_millis(75);
        let client = BaiduClient::from_parts(
            Arc::new(BlockingTransport {
                response: FIXTURE.as_bytes().to_vec(),
                starts: starts_tx,
                releases: Mutex::new(releases_rx),
            }),
            interval,
        );
        let first = {
            let client = client.clone();
            std::thread::spawn(move || client.technical_bars(&request(1)))
        };
        let first_started = starts_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first request enters transport");
        let second = {
            let client = client.clone();
            std::thread::spawn(move || client.technical_bars(&request(1)))
        };
        assert!(
            starts_rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "the second clone must not enter while the first transport call is reading"
        );
        releases_tx.send(()).expect("release first request");
        let second_started = starts_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second request enters after the first completes");
        assert!(second_started.duration_since(first_started) >= interval);
        releases_tx.send(()).expect("release second request");
        first.join().expect("first thread").expect("first request");
        second
            .join()
            .expect("second thread")
            .expect("second request");
    }
}
