use super::*;
use magic_market_core::{Exchange, HistoricalBars, TechnicalBarsProvider};
use std::io::{self, Read};
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

#[derive(Debug)]
struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("fixture read failed"))
    }
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
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).expect("instrument"),
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
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).expect("instrument"),
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
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).expect("instrument"),
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
    let snapshot = client.load_probe_snapshot().expect("probe snapshot");
    assert_eq!(snapshot.request_starts(), 2);
    assert_eq!(snapshot.maximum_concurrency(), 1);
    assert_eq!(snapshot.active_requests(), 0);
    assert!(snapshot.minimum_start_gap().expect("two request starts") >= interval);
}

#[test]
fn constructors_debug_capabilities_and_historical_projection_are_covered() {
    assert!(matches!(
        BaiduClient::with_timeout(Duration::ZERO),
        Err(BaiduError::InvalidRequest(_))
    ));
    let network_client = BaiduClient::with_timeout(Duration::from_millis(1))
        .expect("positive timeout builds an offline client");
    assert!(format!("{network_client:?}").contains("BaiduClient"));
    assert!(!BaiduClient::capabilities().bars);
    assert!(BaiduClient::new().is_ok());

    let invalid_request = HttpRequest {
        url: "https://evil.test/x".into(),
        headers: Vec::new(),
    };
    let transport =
        HttpsTransport::new(Duration::from_millis(1)).expect("positive timeout builds transport");
    assert!(matches!(
        transport.get(&invalid_request),
        Err(BaiduError::InvalidRequest(_))
    ));

    let client = BaiduClient::with_transport(FixtureTransport {
        response: FIXTURE.as_bytes().to_vec(),
        request: Mutex::new(None),
    });
    let batch = client
        .historical_bars(&request(1))
        .expect("technical bars project to ordinary bars");
    assert_eq!(batch.records().len(), 1);
    assert_eq!(batch.records()[0].close().get(), 16.41);
    assert_eq!(batch.provenance().source(), "baidu-pae");
}

#[test]
fn bounded_http_reader_preserves_status_content_type_size_and_io_failures() {
    assert_eq!(
        read_http_response(200, Some("application/json"), &b"{}"[..])
            .expect("valid bounded response"),
        b"{}"
    );
    assert!(matches!(
        read_http_response(503, Some("application/json"), &b"{}"[..]),
        Err(BaiduError::Transport(_))
    ));
    assert!(matches!(
        read_http_response(200, Some("text/html"), &b"{}"[..]),
        Err(BaiduError::Protocol(_))
    ));
    assert!(matches!(
        read_http_response(200, Some("application/json"), FailingReader),
        Err(BaiduError::Transport(_))
    ));
    assert!(matches!(
        read_http_response(
            200,
            Some("application/json"),
            vec![b'x'; MAX_RESPONSE_BYTES + 1].as_slice()
        ),
        Err(BaiduError::Protocol(_))
    ));
}

#[test]
fn request_validation_covers_asset_limit_range_identity_and_request_shape() {
    let non_equity = BarsRequest::new(
        InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).expect("index identity"),
        BarInterval::Day,
        1,
    )
    .expect("request");
    assert!(matches!(
        validate_request(&non_equity),
        Err(BaiduError::Unsupported(_))
    ));

    assert!(matches!(
        validate_request(&request(MAX_ROWS + 1)),
        Err(BaiduError::InvalidRequest(_))
    ));
    let ranged = request(1)
        .with_range("2026-07-01", "2026-07-23")
        .expect("valid core range");
    assert!(matches!(
        validate_request(&ranged),
        Err(BaiduError::Unsupported(_))
    ));

    for (exchange, code) in [
        (Exchange::Shanghai, "600396"),
        (Exchange::Shenzhen, "000001"),
        (Exchange::Shenzhen, "300001"),
        (Exchange::Beijing, "430001"),
        (Exchange::Beijing, "830001"),
        (Exchange::Beijing, "920001"),
    ] {
        let instrument =
            InstrumentId::new(exchange, code, AssetClass::Equity).expect("verified identity");
        assert!(validate_instrument_identity(&instrument).is_ok());
        let built = build_request(&instrument).expect("verified request");
        assert!(built.url().contains(&format!("code={code}")));
        assert_eq!(built.headers().len(), 4);
    }

    let short = InstrumentId::new(Exchange::Shanghai, "60039", AssetClass::Equity)
        .expect("core accepts provider-specific identity");
    assert!(matches!(
        validate_instrument_identity(&short),
        Err(BaiduError::InvalidRequest(_))
    ));
    assert!(ensure_official_url("https://finance.pae.baidu.com/").is_err());
    assert!(ensure_official_url("https://finance.pae.baidu.com//x").is_err());
    assert!(ensure_official_url("https://finance.pae.baidu.com/x\n").is_err());
}

#[test]
fn response_structure_and_row_integrity_fail_closed() {
    for body in [
        "{",
        r#"{"Result":{"newMarketData":{}}}"#,
        r#"{"ResultCode":"0","Result":null}"#,
        r#"{"ResultCode":"0","Result":{"newMarketData":{"keys":null}}}"#,
        r#"{"ResultCode":"0","Result":{"newMarketData":{"keys":[],"marketData":null}}}"#,
        r#"{"ResultCode":"0","Result":{"newMarketData":{"keys":["time","open","close","high","low","volume","amount","ma5avgprice","ma10avgprice","ma20avgprice"],"marketData":""}}}"#,
    ] {
        assert!(parse_response(body.as_bytes(), &request(1), "observed").is_err());
    }

    let missing_field = FIXTURE.replace(
        "1784736000,2026-07-23,15.30,16.41,341780059,16.41,14.85,5352355411.00,13.87,13.02,13.40",
        "1784736000,2026-07-23,15.30",
    );
    assert!(parse_response(missing_field.as_bytes(), &request(2), "observed").is_err());

    let duplicate = FIXTURE.replace("2026-07-23", "2026-07-22");
    assert!(parse_response(duplicate.as_bytes(), &request(2), "observed").is_err());
    let unordered = FIXTURE
        .replace("2026-07-22", "2026-07-24")
        .replace("2026-07-23", "2026-07-22");
    assert!(parse_response(unordered.as_bytes(), &request(2), "observed").is_err());
    let invalid_date = FIXTURE.replace("2026-07-23", "20260723xx");
    assert!(parse_response(invalid_date.as_bytes(), &request(1), "observed").is_err());
    let bad_ohlc = FIXTURE.replace(
        "15.30,16.41,341780059,16.41,14.85",
        "15.30,16.41,341780059,16.00,14.85",
    );
    assert!(parse_response(bad_ohlc.as_bytes(), &request(1), "observed").is_err());
    let negative_volume = FIXTURE.replace("341780059", "-1");
    assert!(parse_response(negative_volume.as_bytes(), &request(1), "observed").is_err());
    let negative_amount = FIXTURE.replace("5352355411.00", "-1");
    assert!(parse_response(negative_amount.as_bytes(), &request(1), "observed").is_err());
}

#[test]
fn key_and_numeric_helpers_reject_every_untrusted_shape() {
    let non_text = vec![Value::from(7)];
    assert!(build_key_index(&non_text).is_err());
    let duplicate = vec![
        Value::from("time"),
        Value::from("time"),
        Value::from("open"),
        Value::from("close"),
        Value::from("high"),
        Value::from("low"),
        Value::from("volume"),
        Value::from("amount"),
        Value::from("ma5avgprice"),
        Value::from("ma10avgprice"),
        Value::from("ma20avgprice"),
    ];
    assert!(build_key_index(&duplicate).is_err());
    let missing = vec![
        Value::from("time"),
        Value::from("open"),
        Value::from("close"),
    ];
    assert!(build_key_index(&missing).is_err());
    assert!(field(&["value"], &HashMap::new(), "open").is_err());

    assert!(positive("0", "open").is_err());
    assert!(positive("not-a-number", "open").is_err());
    assert!(positive("NaN", "open").is_err());
    assert!(nonnegative("-0.1", "volume").is_err());
    assert_eq!(
        optional_positive(" -- ", "ma5avgprice").expect("missing MA"),
        None
    );
    assert_eq!(
        optional_positive("1.5", "ma5avgprice").expect("positive MA"),
        Some(1.5)
    );
    assert!(optional_positive("-1", "ma5avgprice").is_err());
    assert!(validate_date("2026/07/23").is_err());
    assert!(validate_date("2026-7-023").is_err());
    assert!(now().expect("clock after epoch").contains('.'));
}

#[test]
fn server_row_cap_and_sequential_client_pacing_are_enforced() {
    let row = "2026-07-23,15.30,16.41,341780059,16.41,14.85,5352355411.00,13.87,13.02,13.40";
    let encoded = std::iter::repeat_n(row, usize::from(MAX_ROWS) + 1)
        .collect::<Vec<_>>()
        .join(";");
    let body = serde_json::to_vec(&serde_json::json!({
        "ResultCode": "0",
        "Result": {"newMarketData": {
            "keys": ["time","open","close","volume","high","low","amount","ma5avgprice","ma10avgprice","ma20avgprice"],
            "marketData": encoded
        }}
    }))
    .expect("JSON fixture");
    assert!(matches!(
        parse_response(&body, &request(1), "observed"),
        Err(BaiduError::Protocol(message)) if message.contains("maximum")
    ));

    let interval = Duration::from_millis(20);
    let client = BaiduClient::from_parts(
        Arc::new(FixtureTransport {
            response: FIXTURE.as_bytes().to_vec(),
            request: Mutex::new(None),
        }),
        interval,
    );
    client
        .technical_bars(&request(1))
        .expect("first fixture request");
    let second_started = Instant::now();
    client
        .technical_bars(&request(1))
        .expect("second fixture request");
    assert!(second_started.elapsed() >= interval);
}
