use magic_market_core::{AssetClass, BarInterval, BarsRequest, Exchange, InstrumentId, ProviderId};
use magic_market_router::{
    bars_source, AcceptancePolicy, AttemptStatus, BarsRouter, FailureAction, FailureKind,
    RouterError, SourceError,
};
use magic_tdx_rs::{TdxError, TdxHqClient};
use std::sync::Arc;

#[test]
fn tdx_public_bar_route_preserves_explicit_range_rejection() {
    let request = BarsRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
        BarInterval::Day,
        5,
    )
    .unwrap()
    .with_range("2026-07-01", "2026-07-23")
    .unwrap();

    // This real client starts disconnected. Disabling its own retry prevents
    // an accidental transport path from reaching a live endpoint.
    let client = TdxHqClient::new();
    client.set_auto_retry(false);
    let mut router = BarsRouter::new(AcceptancePolicy::new());
    router
        .register(bars_source(
            ProviderId::Tdx,
            Arc::new(client),
            |error| match error {
                TdxError::Unsupported(reason) => {
                    SourceError::stop(FailureKind::Unsupported, reason)
                }
                other => SourceError::stop(FailureKind::Provider, other.to_string()),
            },
        ))
        .unwrap();

    let error = router.route(&request).unwrap_err();

    assert!(matches!(&error, RouterError::Stopped { .. }));
    assert_eq!(error.attempts().len(), 1);
    assert_eq!(error.attempts()[0].provider_id(), ProviderId::Tdx);
    assert_eq!(
        error.attempts()[0].status(),
        &AttemptStatus::Failed {
            kind: FailureKind::Unsupported,
            action: FailureAction::Stop,
            message: "TDX historical bars do not support normalized date ranges; omit start/end"
                .into(),
        }
    );
    assert_eq!(
        (
            request.instrument().exchange(),
            request.instrument().code(),
            request.instrument().asset_class(),
            request.interval(),
            request.start(),
            request.end(),
            request.limit(),
        ),
        (
            Exchange::Shanghai,
            "600396",
            AssetClass::Equity,
            BarInterval::Day,
            Some("2026-07-01"),
            Some("2026-07-23"),
            5,
        )
    );
}

mod quote_identity_public_contract {
    use magic_market_core::{AssetClass, Exchange, InstrumentId, ProviderId, Quote};
    use magic_market_router::{
        quote_source, AcceptancePolicy, AttemptStatus, FailoverChain, FailureAction, FailureKind,
        RouterError, SourceError,
    };
    use magic_tdx_rs::{TdxDirectClient, TdxError};
    use serde_json::json;
    use std::io::{self, Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    // Independent protocol literals, not SDK request builders or business mocks.
    const SETUP_1: &[u8] = &[
        0x0c, 0x02, 0x18, 0x93, 0x00, 0x01, 0x03, 0x00, 0x03, 0x00, 0x0d, 0x00, 0x01,
    ];
    const SETUP_2: &[u8] = &[
        0x0c, 0x02, 0x18, 0x94, 0x00, 0x01, 0x03, 0x00, 0x03, 0x00, 0x0d, 0x00, 0x02,
    ];
    const SETUP_3: &[u8] = &[
        0x0c, 0x03, 0x18, 0x99, 0x00, 0x01, 0x20, 0x00, 0x20, 0x00, 0xdb, 0x0f, 0xd5, 0xd0, 0xc9,
        0xcc, 0xd6, 0xa4, 0xa8, 0xaf, 0x00, 0x00, 0x00, 0x8f, 0xc2, 0x25, 0x40, 0x13, 0x00, 0x00,
        0xd5, 0x00, 0xc9, 0xcc, 0xbd, 0xf0, 0xd7, 0xea, 0x00, 0x00, 0x00, 0x02,
    ];
    const QUOTE_REQUEST: &[u8] = &[
        0x0c, 0x01, 0x20, 0x63, 0x00, 0x02, 0x13, 0x00, 0x13, 0x00, 0x3e, 0x05, 0x05, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, b'6', b'0', b'0', b'3', b'9', b'6',
    ];
    const HANDSHAKE_RESPONSE: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 7];
    const QUOTE_HEADER: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 60, 0, 60, 0];
    // Complete 60-byte row envelope. Price10/close-delta0/open1/high2/low-1,
    // volume10; amount0x41000000 decodes to8.0. Reserved/depth/tail fields intact.
    const QUOTE_BODY: &[u8] = &[
        0, 0, 1, 0, 1, b'6', b'0', b'0', b'3', b'9', b'6', 0, 0, 10, 0, 1, 2, 0x41, 0, 0, 10, 5, 0,
        0, 0, 0x41, 4, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0,
    ];
    type WireTrace = Vec<Vec<Vec<u8>>>;

    fn remaining(deadline: Instant, stop: &AtomicBool) -> io::Result<Duration> {
        if stop.load(Ordering::SeqCst) {
            return Err(io::Error::other("fixture cancelled"));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "fixture deadline"));
        }
        Ok(remaining.min(Duration::from_secs(2)))
    }

    fn read_bounded(
        stream: &mut TcpStream,
        expected: &[u8],
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<Vec<u8>> {
        let mut bytes = vec![0; expected.len()];
        let mut offset = 0;
        while offset < bytes.len() {
            stream.set_read_timeout(Some(remaining(deadline, stop)?))?;
            match stream.read(&mut bytes[offset..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "short request",
                    ))
                }
                Ok(count) => offset += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        if bytes != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected external request bytes: {bytes:?}"),
            ));
        }
        Ok(bytes)
    }

    fn write_bounded(
        stream: &mut TcpStream,
        bytes: &[u8],
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        let mut offset = 0;
        while offset < bytes.len() {
            stream.set_write_timeout(Some(remaining(deadline, stop)?))?;
            match stream.write(&bytes[offset..]) {
                Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "short response")),
                Ok(count) => offset += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn serve(listener: TcpListener, stop: Arc<AtomicBool>) -> io::Result<WireTrace> {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut trace = Vec::new();
        for wrong_code in [false, true] {
            let accept_deadline = (Instant::now() + Duration::from_secs(3)).min(deadline);
            let mut stream = loop {
                remaining(accept_deadline, &stop)?;
                match listener.accept() {
                    Ok((stream, peer)) if peer.ip().is_loopback() => break stream,
                    Ok(_) => return Err(io::Error::other("non-loopback peer")),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            };
            // Windows accept inherits the listener's nonblocking mode. Use the
            // bounded blocking read/write timeouts below for each accepted stream.
            stream.set_nonblocking(false)?;
            let mut requests = Vec::new();
            for setup in [SETUP_1, SETUP_2, SETUP_3] {
                requests.push(read_bounded(&mut stream, setup, deadline, &stop)?);
                write_bounded(&mut stream, HANDSHAKE_RESPONSE, deadline, &stop)?;
            }
            requests.push(read_bounded(&mut stream, QUOTE_REQUEST, deadline, &stop)?);
            let mut body = QUOTE_BODY.to_vec();
            if wrong_code {
                body[5..11].copy_from_slice(b"600397");
            }
            write_bounded(&mut stream, QUOTE_HEADER, deadline, &stop)?;
            write_bounded(&mut stream, &body, deadline, &stop)?;
            trace.push(requests);
            // Drop this accepted stream before accepting the second case.
        }
        Ok(trace)
    }

    struct Fixture {
        address: SocketAddr,
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<io::Result<WireTrace>>>,
    }

    impl Fixture {
        fn start() -> io::Result<Self> {
            let listener = TcpListener::bind(("127.0.0.1", 0))?;
            listener.set_nonblocking(true)?;
            let address = listener.local_addr()?;
            let stop = Arc::new(AtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
            let worker = thread::Builder::new()
                .name("tdx-quote-wire-fixture".into())
                .spawn(move || serve(listener, worker_stop))?;
            Ok(Self {
                address,
                stop,
                worker: Some(worker),
            })
        }

        fn finish(mut self) -> io::Result<WireTrace> {
            self.worker
                .take()
                .ok_or_else(|| io::Error::other("missing fixture worker"))?
                .join()
                .map_err(|_| io::Error::other("fixture worker panicked"))?
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(worker) = self.worker.take() {
                // Even assertion unwind cancels bounded I/O and naturally joins.
                // Do not panic from Drop; the original failing test stays failed.
                let _ = worker.join();
            }
        }
    }

    // Same caller-owned classification as the repository live_probe example.
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

    #[test]
    fn tdx_public_quote_route_rejects_same_count_wrong_code_without_publishing_a_batch() {
        let fixture = Fixture::start().unwrap();
        let client = Arc::new(TdxDirectClient::new(
            "127.0.0.1",
            fixture.address.port(),
            2.0,
        ));
        let mut router = FailoverChain::<[InstrumentId], Quote>::new(AcceptancePolicy::default());
        router
            .register(quote_source(ProviderId::Tdx, client, classify_tdx))
            .unwrap();
        let request =
            [InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()];
        let request_before = serde_json::to_value(&request).unwrap();
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let positive = router.route(&request);
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let negative = router.route(&request);
        // Close and join BEFORE business assertions; all setup/unwind paths have Drop.
        let trace = fixture.finish().unwrap();
        assert_eq!(
            trace,
            vec![
                vec![
                    SETUP_1.to_vec(),
                    SETUP_2.to_vec(),
                    SETUP_3.to_vec(),
                    QUOTE_REQUEST.to_vec()
                ],
                vec![
                    SETUP_1.to_vec(),
                    SETUP_2.to_vec(),
                    SETUP_3.to_vec(),
                    QUOTE_REQUEST.to_vec()
                ],
            ]
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), request_before);
        assert_eq!(router.policy(), AcceptancePolicy::default());
        assert_eq!(router.provider_ids(), vec![ProviderId::Tdx]);
        let outcome = positive.unwrap();
        assert_eq!(outcome.selected_provider(), ProviderId::Tdx);
        assert_eq!(outcome.attempts().len(), 1);
        assert_eq!(outcome.attempts()[0].provider_id(), ProviderId::Tdx);
        assert_eq!(outcome.attempts()[0].status(), &AttemptStatus::Selected);
        let batch = outcome.batch();
        assert_eq!(batch.records().len(), 1);
        let observed = batch.provenance().fetched_at();
        let observed_seconds = observed.parse::<u64>().unwrap();
        assert!((before..=after).contains(&observed_seconds));
        assert_eq!(observed, observed_seconds.to_string());
        let expected_batch_id = format!("tdx-direct:{observed}:quote");
        let expected_quote = json!({
            "instrument": {"exchange": "Shanghai", "code": "600396", "asset_class": "Equity"},
            "name": null, "price": 0.10, "previous_close": 0.10, "open": 0.11,
            "high": 0.12, "low": 0.09, "change_percent": {"value": 0.0, "unit": "Percent"},
            "volume": 10.0, "amount": 8.0, "status": "Unavailable", "source_at": null,
            "observed_at": observed, "provider": "Tdx", "batch_id": expected_batch_id,
        });
        assert_eq!(
            serde_json::to_value(batch).unwrap(),
            json!({
                "records": [expected_quote.clone()],
                "provenance": {"source": "tdx-direct", "source_at": null,
                    "fetched_at": observed, "batch_id": expected_batch_id},
                "quality": {"complete": false, "issues": [
                    "600396: one or more normalized quote fields unavailable",
                    "600396: security name unavailable from the TDX quote packet",
                    "600396: TDX quote source timestamp format is unverified"
                ]},
            })
        );
        let typed_expected: Quote = serde_json::from_value(expected_quote).unwrap();
        assert_eq!(batch.records()[0], typed_expected);
        let error = negative.unwrap_err();
        assert!(matches!(&error, RouterError::Exhausted { .. }));
        assert_eq!(
            error.to_string(),
            "all registered market-data sources were exhausted"
        );
        assert_eq!(error.attempts().len(), 1);
        assert_eq!(error.attempts()[0].provider_id(), ProviderId::Tdx);
        assert_eq!(
            error.attempts()[0].status(),
            &AttemptStatus::Failed {
                kind: FailureKind::Protocol,
                action: FailureAction::TryNext,
                message: "Invalid data: TDX omitted a requested quote".into(),
            }
        );
    }
}

mod intraday_future_shape_public_contract {
    use magic_market_core::{
        AssetClass, Bar, BarInterval, BarsRequest, Exchange, InstrumentId, ProviderId,
        SourcedRecord,
    };
    use magic_market_router::{
        bars_source, AcceptancePolicy, AttemptStatus, BarsRouter, FailureAction, FailureKind,
        RouterError, SourceError,
    };
    use magic_tdx_rs::{TdxDirectClient, TdxError};
    use serde_json::json;
    use std::io::{self, Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const SETUP_1: &[u8] = &[0x0c, 0x02, 0x18, 0x93, 0, 1, 3, 0, 3, 0, 0x0d, 0, 1];
    const SETUP_2: &[u8] = &[0x0c, 0x02, 0x18, 0x94, 0, 1, 3, 0, 3, 0, 0x0d, 0, 2];
    const SETUP_3: &[u8] = &[
        0x0c, 0x03, 0x18, 0x99, 0, 1, 0x20, 0, 0x20, 0, 0xdb, 0x0f, 0xd5, 0xd0, 0xc9, 0xcc, 0xd6,
        0xa4, 0xa8, 0xaf, 0, 0, 0, 0x8f, 0xc2, 0x25, 0x40, 0x13, 0, 0, 0xd5, 0, 0xc9, 0xcc, 0xbd,
        0xf0, 0xd7, 0xea, 0, 0, 0, 2,
    ];
    // Literal category0/market1/code600396/start0/count2/adjust0 request.
    const BARS_REQUEST: &[u8] = &[
        0x0c, 0x01, 0x08, 0x64, 0x01, 0x01, 0x1c, 0, 0x1c, 0, 0x2d, 0x05, 1, 0, b'6', b'0', b'0',
        b'3', b'9', b'6', 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    const HANDSHAKE_RESPONSE: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 7];
    const BARS_HEADER: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 34, 0, 34, 0];
    // Complete count2 envelopes. Each row is date2/minutes2/pricedeltas4/vol4/amount4.
    // 0x8066=2020-01-02; 0xd065=2030-01-01. 0x023f/0x0244=09:35/09:40.
    // Price deltas10,0,2,-1; packed volume/amount0x41000000 are8 source shares/CNY.
    const VALID_BODY: &[u8] = &[
        2, 0, 0x66, 0x80, 0x3f, 2, 10, 0, 2, 0x41, 0, 0, 0, 0x41, 0, 0, 0, 0x41, 0x66, 0x80, 0x44,
        2, 10, 0, 2, 0x41, 0, 0, 0, 0x41, 0, 0, 0, 0x41,
    ];
    const TWO_FUTURE_BODY: &[u8] = &[
        2, 0, 0x65, 0xd0, 0x3f, 2, 10, 0, 2, 0x41, 0, 0, 0, 0x41, 0, 0, 0, 0x41, 0x65, 0xd0, 0x44,
        2, 10, 0, 2, 0x41, 0, 0, 0, 0x41, 0, 0, 0, 0x41,
    ];
    const MISPLACED_FUTURE_BODY: &[u8] = &[
        2, 0, 0x65, 0xd0, 0x3f, 2, 10, 0, 2, 0x41, 0, 0, 0, 0x41, 0, 0, 0, 0x41, 0x66, 0x80, 0x44,
        2, 10, 0, 2, 0x41, 0, 0, 0, 0x41, 0, 0, 0, 0x41,
    ];
    const CLOCK_MIN: u64 = 1_767_225_600; // 2026-01-01T00:00:00Z: parser year cap covers2030.
    const CLOCK_MAX: u64 = 1_893_461_700; // 2030-01-01T01:35:00Z: first future source instant.
    type WireTrace = Vec<Vec<Vec<u8>>>;

    struct WorkerReport {
        trace: WireTrace,
        result: io::Result<()>,
    }

    fn remaining(deadline: Instant, stop: &AtomicBool) -> io::Result<Duration> {
        if stop.load(Ordering::SeqCst) {
            return Err(io::Error::other("fixture cancelled"));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "fixture deadline"));
        }
        Ok(remaining.min(Duration::from_secs(2)))
    }

    // Record full OR partial observed bytes before any equality guard/error return.
    fn read_recorded(
        stream: &mut TcpStream,
        expected: &[u8],
        requests: &mut Vec<Vec<u8>>,
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        let mut bytes = Vec::with_capacity(expected.len());
        let result = (|| {
            while bytes.len() < expected.len() {
                stream.set_read_timeout(Some(remaining(deadline, stop)?))?;
                let mut buffer = [0u8; 64];
                let limit = (expected.len() - bytes.len()).min(buffer.len());
                match stream.read(&mut buffer[..limit]) {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "short request",
                        ))
                    }
                    Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        })();
        requests.push(bytes.clone());
        result?;
        if bytes != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected external request bytes",
            ));
        }
        Ok(())
    }

    fn write_bounded(
        stream: &mut TcpStream,
        bytes: &[u8],
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        let mut offset = 0;
        while offset < bytes.len() {
            stream.set_write_timeout(Some(remaining(deadline, stop)?))?;
            match stream.write(&bytes[offset..]) {
                Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "short response")),
                Ok(count) => offset += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn serve(listener: TcpListener, stop: Arc<AtomicBool>) -> WorkerReport {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut trace = Vec::new();
        let result = (|| {
            for body in [VALID_BODY, TWO_FUTURE_BODY, MISPLACED_FUTURE_BODY] {
                let accept_deadline = (Instant::now() + Duration::from_secs(3)).min(deadline);
                let (mut stream, peer) = loop {
                    remaining(accept_deadline, &stop)?;
                    match listener.accept() {
                        Ok(accepted) => break accepted,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2))
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => return Err(error),
                    }
                };
                trace.push(Vec::new()); // Connection attempt before peer/setup guards.
                if !peer.ip().is_loopback() {
                    return Err(io::Error::other("non-loopback peer"));
                }
                stream.set_nonblocking(false)?; // Windows accept inherits nonblocking.
                let requests = trace
                    .last_mut()
                    .ok_or_else(|| io::Error::other("missing trace"))?;
                for setup in [SETUP_1, SETUP_2, SETUP_3] {
                    read_recorded(&mut stream, setup, requests, deadline, &stop)?;
                    write_bounded(&mut stream, HANDSHAKE_RESPONSE, deadline, &stop)?;
                }
                read_recorded(&mut stream, BARS_REQUEST, requests, deadline, &stop)?;
                write_bounded(&mut stream, BARS_HEADER, deadline, &stop)?;
                write_bounded(&mut stream, body, deadline, &stop)?;
                // Stream drops here on success; all ?/panic paths also drop it.
            }
            Ok(())
        })();
        WorkerReport { trace, result }
    }

    struct Fixture {
        address: SocketAddr,
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<WorkerReport>>,
    }

    impl Fixture {
        fn start() -> io::Result<Self> {
            let listener = TcpListener::bind(("127.0.0.1", 0))?;
            listener.set_nonblocking(true)?;
            let address = listener.local_addr()?;
            let stop = Arc::new(AtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
            let worker = thread::Builder::new()
                .name("tdx-bars-integrity-wire-fixture".into())
                .spawn(move || serve(listener, worker_stop))?;
            Ok(Self {
                address,
                stop,
                worker: Some(worker),
            })
        }

        fn finish(mut self) -> io::Result<WorkerReport> {
            self.worker
                .take()
                .ok_or_else(|| io::Error::other("missing fixture worker"))?
                .join()
                .map_err(|_| io::Error::other("fixture worker panicked"))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join(); // Bounded I/O/accept loop naturally closes; no detached handle.
            }
        }
    }

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

    fn unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[test]
    fn tdx_public_intraday_route_rejects_multiple_or_misplaced_future_rows_atomically() {
        let clock_start = unix_seconds();
        assert!(
            (CLOCK_MIN..CLOCK_MAX).contains(&clock_start),
            "fixed2030 fixture clock/year-cap validity expired; do not skip"
        );
        let request = BarsRequest::new(
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
            BarInterval::Minute5,
            2,
        )
        .unwrap();
        let request_before = serde_json::to_value(&request).unwrap();
        let fixture = Fixture::start().unwrap();
        let client = Arc::new(TdxDirectClient::new(
            "127.0.0.1",
            fixture.address.port(),
            2.0,
        ));
        let mut router = BarsRouter::new(AcceptancePolicy::default());
        router
            .register(bars_source(ProviderId::Tdx, client, classify_tdx))
            .unwrap();
        let before = unix_seconds();
        let positive = router.route(&request);
        let after = unix_seconds();
        let two_future = router.route(&request);
        let misplaced_future = router.route(&request);
        let clock_end = unix_seconds();
        // Join/close before ANY outcome assertion, retaining observed failure traces.
        let report = fixture.finish().unwrap();
        assert_eq!(
            report.trace,
            vec![
                vec![
                    SETUP_1.to_vec(),
                    SETUP_2.to_vec(),
                    SETUP_3.to_vec(),
                    BARS_REQUEST.to_vec()
                ],
                vec![
                    SETUP_1.to_vec(),
                    SETUP_2.to_vec(),
                    SETUP_3.to_vec(),
                    BARS_REQUEST.to_vec()
                ],
                vec![
                    SETUP_1.to_vec(),
                    SETUP_2.to_vec(),
                    SETUP_3.to_vec(),
                    BARS_REQUEST.to_vec()
                ],
            ]
        );
        report.result.unwrap();
        assert!(
            (clock_start..CLOCK_MAX).contains(&clock_end),
            "clock crossed fixed2030 future boundary; do not skip"
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), request_before);
        assert_eq!(router.policy(), AcceptancePolicy::default());
        assert_eq!(router.provider_ids(), vec![ProviderId::Tdx]);
        let outcome = positive.unwrap();
        assert_eq!(outcome.selected_provider(), ProviderId::Tdx);
        assert_eq!(outcome.attempts().len(), 1);
        assert_eq!(outcome.attempts()[0].provider_id(), ProviderId::Tdx);
        assert_eq!(outcome.attempts()[0].status(), &AttemptStatus::Selected);
        let batch = outcome.batch();
        let observed = batch.provenance().fetched_at();
        let observed_seconds = observed.parse::<u64>().unwrap();
        assert!((before..=after).contains(&observed_seconds));
        assert_eq!(observed, observed_seconds.to_string());
        let batch_id = batch.provenance().batch_id().unwrap();
        let prefix = format!("tdx-direct:{observed}:");
        let sequence = batch_id.strip_prefix(&prefix).unwrap();
        let sequence_number = sequence.parse::<u64>().unwrap();
        assert!(sequence_number > 0);
        assert_eq!(sequence, sequence_number.to_string());
        // Runtime-generated sequence is grammar-checked, not guessed or a data oracle.
        let expected_records = json!([
            {"instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
             "interval":"Minute5","bar_start":"2020-01-02 09:35:00","bar_end":"2020-01-02 09:35:00",
             "open":0.010,"high":0.012,"low":0.009,"close":0.010,"volume":0.08,"amount":8.0,
             "adjustment":"Unadjusted","source_at":"2020-01-02 09:35","observed_at":observed,
             "provider":"Tdx","batch_id":batch_id},
            {"instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
             "interval":"Minute5","bar_start":"2020-01-02 09:40:00","bar_end":"2020-01-02 09:40:00",
             "open":0.020,"high":0.022,"low":0.019,"close":0.020,"volume":0.08,"amount":8.0,
             "adjustment":"Unadjusted","source_at":"2020-01-02 09:40","observed_at":observed,
             "provider":"Tdx","batch_id":batch_id}
        ]);
        assert_eq!(
            serde_json::to_value(batch).unwrap(),
            json!({
                "records":expected_records,
                "provenance":{"source":"tdx-direct","source_at":"2020-01-02 09:40","fetched_at":observed,"batch_id":batch_id},
                "quality":{"complete":true,"issues":[]}
            })
        );
        for (record, expected) in batch
            .records()
            .iter()
            .zip(expected_records.as_array().unwrap())
        {
            let typed: Bar = serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(record, &typed);
            assert_eq!(record.provider_id(), ProviderId::Tdx);
            assert_eq!(record.evidence_batch_id(), batch_id);
            assert_eq!(record.evidence_observed_at(), Some(observed));
            assert_eq!(record.evidence_source_at(), record.source_at());
        }
        for (result, message) in [
            (two_future, "Invalid data: TDX intraday bars contain more than one future source row"),
            (misplaced_future, "Invalid data: TDX future intraday row 2030-01-01 09:35 is not the newest source row"),
        ] {
            let error = result.unwrap_err();
            assert!(matches!(&error, RouterError::Exhausted { .. }));
            assert_eq!(error.to_string(), "all registered market-data sources were exhausted");
            assert_eq!(error.attempts().len(), 1);
            assert_eq!(error.attempts()[0].provider_id(), ProviderId::Tdx);
            assert_eq!(error.attempts()[0].status(), &AttemptStatus::Failed {
                kind: FailureKind::Protocol, action: FailureAction::TryNext, message: message.into(),
            });
        }
    }
}

mod corporate_action_window_public_contract {
    use magic_market_core::{
        AssetClass, CorporateAction, CorporateActionRequest, Exchange, InstrumentId, IsoDate,
        ProviderId, SourcedRecord,
    };
    use magic_market_router::{
        corporate_action_source, AcceptancePolicy, AttemptStatus, CorporateActionRouter,
        FailureAction, FailureKind, RouterError, SourceError,
    };
    use magic_tdx_rs::{TdxDirectClient, TdxError};
    use serde_json::json;
    use std::io::{self, Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const SETUP_1: &[u8] = &[0x0c, 0x02, 0x18, 0x93, 0, 1, 3, 0, 3, 0, 0x0d, 0, 1];
    const SETUP_2: &[u8] = &[0x0c, 0x02, 0x18, 0x94, 0, 1, 3, 0, 3, 0, 0x0d, 0, 2];
    const SETUP_3: &[u8] = &[
        0x0c, 0x03, 0x18, 0x99, 0, 1, 0x20, 0, 0x20, 0, 0xdb, 0x0f, 0xd5, 0xd0, 0xc9, 0xcc, 0xd6,
        0xa4, 0xa8, 0xaf, 0, 0, 0, 0x8f, 0xc2, 0x25, 0x40, 0x13, 0, 0, 0xd5, 0, 0xc9, 0xcc, 0xbd,
        0xf0, 0xd7, 0xea, 0, 0, 0, 2,
    ];
    // Literal market1/code600519 request, without an SDK request builder.
    const XDXR_REQUEST: &[u8] = &[
        0x0c, 0x1f, 0x18, 0x76, 0, 1, 0x0b, 0, 0x0b, 0, 0x0f, 0, 1, 0, 1, b'6', b'0', b'0', b'5',
        b'1', b'9',
    ];
    const HANDSHAKE_RESPONSE: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 7];
    const XDXR_HEADER: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 69, 0, 69, 0];
    // Complete11+2*29 body; descending dates20240601/20230601, category1.
    // Source terms are10,2,3,4: cash1/share, rights price2, bonus0.3/share, rights0.4/share.
    const VALID_BODY: &[u8] = &[
        0, 0, 1, b'6', b'0', b'0', b'5', b'1', b'9', 2, 0, 1, b'6', b'0', b'0', b'5', b'1', b'9',
        0, 217, 216, 52, 1, 1, 0, 0, 32, 65, 0, 0, 0, 64, 0, 0, 64, 64, 0, 0, 128, 64, 1, b'6',
        b'0', b'0', b'5', b'1', b'9', 0, 201, 177, 52, 1, 1, 0, 0, 32, 65, 0, 0, 0, 64, 0, 0, 64,
        64, 0, 0, 128, 64,
    ];
    // Independent complete literal: ONLY tail rights-price f32 at57..61 changes2→-1.
    const NEGATIVE_TAIL_BODY: &[u8] = &[
        0, 0, 1, b'6', b'0', b'0', b'5', b'1', b'9', 2, 0, 1, b'6', b'0', b'0', b'5', b'1', b'9',
        0, 217, 216, 52, 1, 1, 0, 0, 32, 65, 0, 0, 0, 64, 0, 0, 64, 64, 0, 0, 128, 64, 1, b'6',
        b'0', b'0', b'5', b'1', b'9', 0, 201, 177, 52, 1, 1, 0, 0, 32, 65, 0, 0, 128, 191, 0, 0,
        64, 64, 0, 0, 128, 64,
    ];
    const CLOCK_MIN: u64 = 1_735_689_600; //2025-01-01T00:00:00Z; both source dates are past.
    const CLOCK_MAX: u64 = 4_102_444_800; //2100-01-01T00:00:00Z; bounded independent calendar oracle.
    type WireTrace = Vec<Vec<Vec<u8>>>;

    struct WorkerReport {
        trace: WireTrace,
        result: io::Result<()>,
    }

    fn remaining(deadline: Instant, stop: &AtomicBool) -> io::Result<Duration> {
        if stop.load(Ordering::SeqCst) {
            return Err(io::Error::other("fixture cancelled"));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "fixture deadline"));
        }
        Ok(remaining.min(Duration::from_secs(2)))
    }

    fn read_recorded(
        stream: &mut TcpStream,
        expected: &[u8],
        requests: &mut Vec<Vec<u8>>,
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        let mut bytes = Vec::with_capacity(expected.len());
        let result = (|| {
            while bytes.len() < expected.len() {
                stream.set_read_timeout(Some(remaining(deadline, stop)?))?;
                let mut buffer = [0u8; 64];
                let limit = (expected.len() - bytes.len()).min(buffer.len());
                match stream.read(&mut buffer[..limit]) {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "short request",
                        ))
                    }
                    Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        })();
        requests.push(bytes.clone()); //Full/partial attempt recorded BEFORE error/equality guards.
        result?;
        if bytes != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected external request bytes",
            ));
        }
        Ok(())
    }

    fn write_bounded(
        stream: &mut TcpStream,
        bytes: &[u8],
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        let mut offset = 0;
        while offset < bytes.len() {
            stream.set_write_timeout(Some(remaining(deadline, stop)?))?;
            match stream.write(&bytes[offset..]) {
                Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "short response")),
                Ok(count) => offset += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn serve(listener: TcpListener, stop: Arc<AtomicBool>) -> WorkerReport {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut trace = Vec::new();
        let result = (|| {
            for body in [VALID_BODY, NEGATIVE_TAIL_BODY] {
                let accept_deadline = (Instant::now() + Duration::from_secs(3)).min(deadline);
                let (mut stream, peer) = loop {
                    remaining(accept_deadline, &stop)?;
                    match listener.accept() {
                        Ok(accepted) => break accepted,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => return Err(error),
                    }
                };
                trace.push(Vec::new()); //Connection recorded before peer/configuration guards.
                if !peer.ip().is_loopback() {
                    return Err(io::Error::other("non-loopback peer"));
                }
                stream.set_nonblocking(false)?; //Windows accepted socket may inherit nonblocking.
                let requests = trace
                    .last_mut()
                    .ok_or_else(|| io::Error::other("missing trace"))?;
                for setup in [SETUP_1, SETUP_2, SETUP_3] {
                    read_recorded(&mut stream, setup, requests, deadline, &stop)?;
                    write_bounded(&mut stream, HANDSHAKE_RESPONSE, deadline, &stop)?;
                }
                read_recorded(&mut stream, XDXR_REQUEST, requests, deadline, &stop)?;
                write_bounded(&mut stream, XDXR_HEADER, deadline, &stop)?;
                write_bounded(&mut stream, body, deadline, &stop)?;
                //Direct owns a per-call socket. Prove EOF, rather than ignoring extra requests.
                loop {
                    stream.set_read_timeout(Some(remaining(deadline, &stop)?))?;
                    let mut extra = [0u8; 1];
                    match stream.read(&mut extra) {
                        Ok(0) => break,
                        Ok(count) => {
                            requests.push(extra[..count].to_vec()); //Observed before rejection.
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "extra request bytes",
                            ));
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => return Err(error),
                    }
                }
                //Owned stream drops on success, every error, and unwind; no heartbeat/client cache.
            }
            Ok(())
        })();
        WorkerReport { trace, result }
    }

    struct Fixture {
        address: SocketAddr,
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<WorkerReport>>,
    }

    impl Fixture {
        fn start() -> io::Result<Self> {
            let listener = TcpListener::bind(("127.0.0.1", 0))?;
            listener.set_nonblocking(true)?;
            let address = listener.local_addr()?;
            let stop = Arc::new(AtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
            let worker = thread::Builder::new()
                .name("tdx-corporate-action-wire-fixture".into())
                .spawn(move || serve(listener, worker_stop))?;
            Ok(Self {
                address,
                stop,
                worker: Some(worker),
            })
        }

        fn finish(mut self) -> io::Result<WorkerReport> {
            self.worker
                .take()
                .ok_or_else(|| io::Error::other("missing fixture worker"))?
                .join()
                .map_err(|_| io::Error::other("fixture worker panicked"))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join(); //Cancel bounded accept/IO and join; no detached worker/panic in Drop.
            }
        }
    }

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

    fn unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    //Independent civil-from-days arithmetic, not SDK today_yyyymmdd or the returned date.
    fn china_date(seconds: u64) -> String {
        let days = i64::try_from(seconds.checked_add(8 * 3600).unwrap() / 86_400).unwrap();
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let day_of_era = z - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let mut year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_prime = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
        let month = month_prime + if month_prime < 10 { 3 } else { -9 };
        if month <= 2 {
            year += 1;
        }
        format!("{year:04}-{month:02}-{day:02}")
    }

    #[test]
    fn tdx_public_corporate_action_route_validates_tail_terms_before_range_projection() {
        for (seconds, date) in [
            (0, "1970-01-01"),
            (951_753_600, "2000-02-29"),
            (1_709_164_800, "2024-02-29"),
            (1_760_716_800, "2025-10-18"),
        ] {
            assert_eq!(china_date(seconds), date);
        }
        let clock_start = unix_seconds();
        assert!(
            (CLOCK_MIN..CLOCK_MAX).contains(&clock_start),
            "fixture clock validity expired; do not skip"
        );
        let admission_literal = china_date(clock_start);
        let admission_as_of = IsoDate::new(&admission_literal).unwrap();
        let request = CorporateActionRequest::new(
            InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        )
        .with_range(
            IsoDate::new("2024-01-01").unwrap(),
            IsoDate::new("2024-12-31").unwrap(),
        )
        .unwrap();
        let fixture = Fixture::start().unwrap();
        let client = Arc::new(TdxDirectClient::new(
            "127.0.0.1",
            fixture.address.port(),
            2.0,
        ));
        let mut router =
            CorporateActionRouter::new(AcceptancePolicy::default(), admission_as_of.clone());
        router
            .register(corporate_action_source(
                ProviderId::Tdx,
                client,
                classify_tdx,
            ))
            .unwrap();
        let before = unix_seconds();
        let positive = router.route(&request);
        let after = unix_seconds();
        let negative = router.route(&request);
        let clock_end = unix_seconds();
        //Natural join/close before any business assertion; Drop covers every earlier unwind.
        let report = fixture.finish().unwrap();
        assert_eq!(
            report.trace,
            vec![
                vec![
                    SETUP_1.to_vec(),
                    SETUP_2.to_vec(),
                    SETUP_3.to_vec(),
                    XDXR_REQUEST.to_vec()
                ],
                vec![
                    SETUP_1.to_vec(),
                    SETUP_2.to_vec(),
                    SETUP_3.to_vec(),
                    XDXR_REQUEST.to_vec()
                ],
            ]
        );
        report.result.unwrap();
        assert!((clock_start..CLOCK_MAX).contains(&clock_end));
        assert_eq!(china_date(before), admission_literal);
        assert_eq!(china_date(after), admission_literal);
        assert_eq!(
            china_date(clock_end),
            admission_literal,
            "China midnight crossed; fail, not skip"
        );
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({
                "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
                "start":"2024-01-01", "end":"2024-12-31"
            })
        );
        assert_eq!(router.policy(), AcceptancePolicy::default());
        assert_eq!(router.admission_as_of(), &admission_as_of);
        assert_eq!(router.provider_ids(), vec![ProviderId::Tdx]);
        let outcome = positive.unwrap();
        assert_eq!(outcome.admission_as_of(), &admission_as_of);
        assert_eq!(outcome.selected_provider(), ProviderId::Tdx);
        assert_eq!(outcome.attempts().len(), 1);
        assert_eq!(outcome.attempts()[0].provider_id(), ProviderId::Tdx);
        assert_eq!(outcome.attempts()[0].status(), &AttemptStatus::Selected);
        assert_eq!(outcome.implemented_actions().count(), 1);
        let batch = outcome.batch();
        let observed = batch.provenance().fetched_at();
        let observed_seconds = observed.parse::<u64>().unwrap();
        assert_eq!(observed, observed_seconds.to_string());
        assert!((before..=after).contains(&observed_seconds));
        let batch_id = format!(
            "tdx:{observed}:corporate-actions:Shanghai:600519:Equity:2024-01-01:2024-12-31"
        );
        let expected_record = json!({
            "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
            "category":"Distribution", "effective_on":"2024-06-01",
            "record_on":null, "ex_on":null, "payable_on":null, "status":"Implemented",
            "terms":{"Distribution":{"cash_per_share":1.0,"bonus_per_share":0.3,
                "rights_per_share":0.4,"rights_price":2.0}},
            "evidence":{"provider":"Tdx","source_at":null,"observed_at":observed,"batch_id":batch_id}
        });
        assert_eq!(
            serde_json::to_value(batch).unwrap(),
            json!({
                "records":[expected_record.clone()],
                "provenance":{"source":"tdx","source_at":null,"fetched_at":observed,"batch_id":batch_id},
                "quality":{"complete":true,"issues":[]}
            })
        );
        let typed_expected: CorporateAction = serde_json::from_value(expected_record).unwrap();
        assert_eq!(batch.records(), &[typed_expected]);
        assert_eq!(batch.records()[0].provider_id(), ProviderId::Tdx);
        assert_eq!(batch.records()[0].evidence_batch_id(), batch_id);
        assert_eq!(batch.records()[0].evidence_observed_at(), Some(observed));
        assert_eq!(batch.records()[0].evidence_source_at(), None);
        let error = negative.unwrap_err(); //No accepted batch/valid-prefix publication on tail failure.
        assert!(matches!(&error, RouterError::Exhausted { .. }));
        assert_eq!(
            error.to_string(),
            "all registered market-data sources were exhausted"
        );
        assert_eq!(error.attempts().len(), 1);
        assert_eq!(error.attempts()[0].provider_id(), ProviderId::Tdx);
        assert_eq!(
            error.attempts()[0].status(),
            &AttemptStatus::Failed {
                kind: FailureKind::Protocol,
                action: FailureAction::TryNext,
                message: "Invalid data: TDX distribution rights price must be non-negative".into(),
            }
        );
    }
}

mod daily_forming_tail_public_contract {
    use magic_market_core::{
        AssetClass, Bar, BarInterval, BarsRequest, Exchange, InstrumentId, ProviderId,
    };
    use magic_market_router::{
        bars_source, AcceptancePolicy, AttemptStatus, BarsRouter, FailureAction, FailureKind,
        RouterError, SourceError,
    };
    use magic_tdx_rs::{TdxDirectClient, TdxError};
    use serde_json::{json, Value};
    use std::io::{self, Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const SETUP_1: &[u8] = &[0x0c, 0x02, 0x18, 0x93, 0, 1, 3, 0, 3, 0, 0x0d, 0, 1];
    const SETUP_2: &[u8] = &[0x0c, 0x02, 0x18, 0x94, 0, 1, 3, 0, 3, 0, 0x0d, 0, 2];
    const SETUP_3: &[u8] = &[
        0x0c, 0x03, 0x18, 0x99, 0, 1, 0x20, 0, 0x20, 0, 0xdb, 0x0f, 0xd5, 0xd0, 0xc9, 0xcc, 0xd6,
        0xa4, 0xa8, 0xaf, 0, 0, 0, 0x8f, 0xc2, 0x25, 0x40, 0x13, 0, 0, 0xd5, 0, 0xc9, 0xcc, 0xbd,
        0xf0, 0xd7, 0xea, 0, 0, 0, 2,
    ];
    const HANDSHAKE_RESPONSE: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 7];
    // Independent full literals: Shanghai 600396, daily category4, unadjusted fq0.
    const FIRST_REQUEST: &[u8] = &[
        0x0c, 0x01, 0x08, 0x64, 0x01, 0x01, 0x1c, 0, 0x1c, 0, 0x2d, 0x05, 1, 0, b'6', b'0', b'0',
        b'3', b'9', b'6', 4, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    const BACKFILL_REQUEST: &[u8] = &[
        0x0c, 0x01, 0x08, 0x64, 0x01, 0x01, 0x1c, 0, 0x1c, 0, 0x2d, 0x05, 1, 0, b'6', b'0', b'0',
        b'3', b'9', b'6', 4, 0, 0, 0, 2, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    // Four variable-length signed price deltas followed by raw packed volume/amount.
    // 0x41000000 is8; the public normalized volume is8shares /100 =0.08lots.
    const ROW_10: [u8; 12] = [10, 0, 2, 0x41, 0, 0, 0, 0x41, 0, 0, 0, 0x41];
    const ROW_5: [u8; 12] = [5, 0, 2, 0x41, 0, 0, 0, 0x41, 0, 0, 0, 0x41];
    type WireTrace = Vec<Vec<Vec<u8>>>;

    #[derive(Clone, Copy, Debug)]
    enum Case {
        Normal,
        ZeroTailAmount,
        InconsistentTailOhlc,
        DuplicateCurrentDay,
        MisplacedCurrentDay,
        EmptyBackfill,
        ExcessBackfill,
    }

    struct Sample {
        day: u64,
        current: String,
        previous: String,
        older: String,
        primary: Vec<u8>,
        backfill: Vec<u8>,
    }

    fn unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    // Test-only independent Gregorian calendar conversion; no production clock/normalizer call.
    fn calendar(mut days: u64) -> (u32, String) {
        let mut year = 1970u32;
        loop {
            let leap =
                year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
            let length = if leap { 366 } else { 365 };
            if days < length {
                break;
            }
            days -= length;
            year += 1;
        }
        let leap =
            year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
        let months = [
            31,
            if leap { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        let mut month = 1u32;
        for length in months {
            if days < length {
                break;
            }
            days -= length;
            month += 1;
        }
        let date = days as u32 + 1;
        (
            year * 10_000 + month * 100 + date,
            format!("{year:04}-{month:02}-{date:02}"),
        )
    }

    fn raw_body(rows: &[(u32, [u8; 12])]) -> Vec<u8> {
        let mut body = (rows.len() as u16).to_le_bytes().to_vec();
        for (date, row) in rows {
            body.extend_from_slice(&date.to_le_bytes());
            body.extend_from_slice(row);
        }
        body
    }

    fn sample(case: Case, seconds: u64) -> Sample {
        let day = seconds.checked_add(28_800).unwrap() / 86_400;
        let (current_raw, current) = calendar(day);
        let (previous_raw, previous) = calendar(day.checked_sub(1).unwrap());
        let (older_raw, older) = calendar(day.checked_sub(2).unwrap());
        let (oldest_raw, _) = calendar(day.checked_sub(3).unwrap());
        let mut tail = ROW_10;
        if matches!(case, Case::ZeroTailAmount) {
            tail[8..12].fill(0);
        }
        if matches!(case, Case::InconsistentTailOhlc) {
            tail[2] = 0x41; // high19 < open/close20; still finite/nonnegative SDK raw prices.
        }
        let primary = match case {
            Case::DuplicateCurrentDay => raw_body(&[(current_raw, ROW_10), (current_raw, ROW_10)]),
            Case::MisplacedCurrentDay => raw_body(&[(current_raw, ROW_10), (previous_raw, ROW_10)]),
            _ => raw_body(&[(previous_raw, ROW_10), (current_raw, tail)]),
        };
        let backfill = match case {
            Case::EmptyBackfill => raw_body(&[]),
            Case::ExcessBackfill => raw_body(&[(oldest_raw, ROW_5), (older_raw, ROW_5)]),
            _ => raw_body(&[(older_raw, ROW_5)]),
        };
        Sample {
            day,
            current,
            previous,
            older,
            primary,
            backfill,
        }
    }

    fn remaining(deadline: Instant, stop: &AtomicBool) -> io::Result<Duration> {
        if stop.load(Ordering::SeqCst) {
            return Err(io::Error::other("fixture cancelled"));
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "fixture deadline"));
        }
        Ok(left.min(Duration::from_secs(2)))
    }

    // Lossless full/partial attempt entry exists before any request-equality guard.
    fn read_recorded(
        stream: &mut TcpStream,
        expected: &[u8],
        frames: &mut Vec<Vec<u8>>,
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        frames.push(Vec::new());
        let bytes = frames
            .last_mut()
            .ok_or_else(|| io::Error::other("missing frame"))?;
        while bytes.len() < expected.len() {
            stream.set_read_timeout(Some(remaining(deadline, stop)?))?;
            let mut buffer = [0u8; 64];
            let count = (expected.len() - bytes.len()).min(buffer.len());
            match stream.read(&mut buffer[..count]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "short request",
                    ))
                }
                Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        if bytes != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected request bytes",
            ));
        }
        Ok(())
    }

    fn write_bounded(
        stream: &mut TcpStream,
        bytes: &[u8],
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        let mut offset = 0;
        while offset < bytes.len() {
            stream.set_write_timeout(Some(remaining(deadline, stop)?))?;
            match stream.write(&bytes[offset..]) {
                Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "short response")),
                Ok(count) => offset += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    struct WorkerReport {
        trace: WireTrace,
        result: io::Result<()>,
    }

    fn serve(
        listener: TcpListener,
        stop: Arc<AtomicBool>,
        primary: Vec<u8>,
        backfill: Vec<u8>,
    ) -> WorkerReport {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut trace = Vec::new();
        let result = (|| {
            for (index, body) in [primary, backfill].into_iter().enumerate() {
                let accept_deadline = (Instant::now() + Duration::from_secs(3)).min(deadline);
                let accepted = loop {
                    // Cancellation is successful only while awaiting an OPTIONAL next connection.
                    // Cancellation in active framing/I/O remains an error, not a fabricated fixture PASS.
                    if stop.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    remaining(accept_deadline, &stop)?;
                    match listener.accept() {
                        Ok(accepted) => break accepted,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => return Err(error),
                    }
                };
                let (mut stream, peer) = accepted;
                trace.push(Vec::new()); // Accepted external attempt before peer/framing guards.
                if !peer.ip().is_loopback() {
                    return Err(io::Error::other("non-loopback peer"));
                }
                stream.set_nonblocking(false)?;
                let frames = trace
                    .last_mut()
                    .ok_or_else(|| io::Error::other("missing trace"))?;
                for setup in [SETUP_1, SETUP_2, SETUP_3] {
                    read_recorded(&mut stream, setup, frames, deadline, &stop)?;
                    write_bounded(&mut stream, HANDSHAKE_RESPONSE, deadline, &stop)?;
                }
                let request = if index == 0 {
                    FIRST_REQUEST
                } else {
                    BACKFILL_REQUEST
                };
                read_recorded(&mut stream, request, frames, deadline, &stop)?;
                let size = u16::try_from(body.len()).map_err(io::Error::other)?;
                let mut header = [0u8; 16];
                header[12..14].copy_from_slice(&size.to_le_bytes());
                header[14..16].copy_from_slice(&size.to_le_bytes());
                write_bounded(&mut stream, &header, deadline, &stop)?;
                write_bounded(&mut stream, &body, deadline, &stop)?;
                // Each real Direct query owns and drops its connection, no SDK worker/pool is used.
            }
            Ok(())
        })();
        WorkerReport { trace, result }
    }

    struct Fixture {
        address: SocketAddr,
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<WorkerReport>>,
    }

    impl Fixture {
        fn start(sample: &Sample) -> io::Result<Self> {
            let listener = TcpListener::bind(("127.0.0.1", 0))?;
            listener.set_nonblocking(true)?;
            let address = listener.local_addr()?;
            let stop = Arc::new(AtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
            let primary = sample.primary.clone();
            let backfill = sample.backfill.clone();
            let worker = thread::Builder::new()
                .name("tdx-day-tail-wire-fixture".into())
                .spawn(move || serve(listener, worker_stop, primary, backfill))?;
            Ok(Self {
                address,
                stop,
                worker: Some(worker),
            })
        }

        fn finish(mut self) -> io::Result<WorkerReport> {
            // A completed public route cannot need another socket: cancel the OPTIONAL accept.
            self.stop.store(true, Ordering::SeqCst);
            self.worker
                .take()
                .ok_or_else(|| io::Error::other("missing fixture worker"))?
                .join()
                .map_err(|_| io::Error::other("fixture worker panicked"))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join(); // Deadline-bound accept/I/O; no detached thread or foreign PID.
            }
        }
    }

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

    fn literal_bar(date: &str, prices: [f64; 4], observed: &str, batch_id: &str) -> Value {
        json!({
            "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
            "interval":"Day","bar_start":date,"bar_end":date,
            "open":prices[0],"high":prices[1],"low":prices[2],"close":prices[3],
            "volume":0.08,"amount":8.0,"adjustment":"Unadjusted",
            "source_at":date,"observed_at":observed,"provider":"Tdx","batch_id":batch_id
        })
    }

    fn assert_complete(
        batch: &magic_market_core::DataBatch<Bar>,
        rows: Vec<(&str, [f64; 4])>,
        before: u64,
        after: u64,
    ) {
        let observed = batch.provenance().fetched_at();
        let seconds = observed.parse::<u64>().unwrap();
        assert_eq!(observed, seconds.to_string());
        assert!((before..=after).contains(&seconds));
        let batch_id = batch.provenance().batch_id().unwrap();
        let suffix = batch_id
            .strip_prefix(&format!("tdx-direct:{observed}:"))
            .unwrap();
        let sequence = suffix.parse::<u64>().unwrap();
        assert!(sequence > 0);
        assert_eq!(suffix, sequence.to_string());
        let records: Vec<Value> = rows
            .iter()
            .map(|(date, prices)| literal_bar(date, *prices, observed, batch_id))
            .collect();
        let source_at = rows.last().unwrap().0;
        assert_eq!(
            serde_json::to_value(batch).unwrap(),
            json!({
                "records":records,
                "provenance":{"source":"tdx-direct","source_at":source_at,
                    "fetched_at":observed,"batch_id":batch_id},
                "quality":{"complete":true,"issues":[]}
            })
        );
        for (record, expected) in batch.records().iter().zip(records) {
            let typed: Bar = serde_json::from_value(expected).unwrap();
            assert_eq!(record, &typed);
        }
    }

    fn assert_rejection(error: &RouterError, permitted: &[String]) {
        assert!(matches!(error, RouterError::Exhausted { .. }));
        assert_eq!(
            error.to_string(),
            "all registered market-data sources were exhausted"
        );
        assert_eq!(error.attempts().len(), 1);
        assert_eq!(error.attempts()[0].provider_id(), ProviderId::Tdx);
        let AttemptStatus::Failed {
            kind,
            action,
            message,
        } = error.attempts()[0].status()
        else {
            panic!("not a failed public route");
        };
        assert_eq!(kind, &FailureKind::Protocol);
        assert_eq!(action, &FailureAction::TryNext);
        assert!(
            permitted.contains(message),
            "unexpected public error: {message}"
        );
    }

    #[test]
    fn tdx_public_daily_route_validates_forming_and_settled_tail_before_any_publication() {
        let request = BarsRequest::new(
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
            BarInterval::Day,
            2,
        )
        .unwrap();
        let request_before = serde_json::to_value(&request).unwrap();
        for case in [
            Case::Normal,
            Case::ZeroTailAmount,
            Case::InconsistentTailOhlc,
            Case::DuplicateCurrentDay,
            Case::MisplacedCurrentDay,
            Case::EmptyBackfill,
            Case::ExcessBackfill,
        ] {
            let seed = unix_seconds();
            let sample = sample(case, seed);
            let fixture = Fixture::start(&sample).unwrap();
            let client = Arc::new(TdxDirectClient::new(
                "127.0.0.1",
                fixture.address.port(),
                2.0,
            ));
            let mut router = BarsRouter::new(AcceptancePolicy::default());
            let registration = router
                .register(bars_source(ProviderId::Tdx, client, classify_tdx))
                .map(|_| ());
            let before = unix_seconds();
            let duration_start = Instant::now();
            let outcome = if registration.is_ok() {
                Some(router.route(&request))
            } else {
                None
            };
            let duration = duration_start.elapsed();
            let after = unix_seconds();
            // Finish/cancel ALL fixture threads/sockets before outcome/registration assertions.
            let report = fixture.finish().unwrap();
            registration.unwrap();
            let WorkerReport {
                trace,
                result: fixture_result,
            } = report;
            fixture_result.unwrap();
            assert!(
                duration <= Duration::from_secs(20),
                "bounded route exceeded20s: {case:?}"
            );
            assert!(
                seed <= before && before <= after,
                "system clock moved backwards"
            );
            assert!(after - before <= 20, "wall-clock bracket exceeded20s");
            let outcome = outcome.unwrap();
            assert_eq!(serde_json::to_value(&request).unwrap(), request_before);
            assert_eq!(router.policy(), AcceptancePolicy::default());
            assert_eq!(router.provider_ids(), vec![ProviderId::Tdx]);
            assert!((1..=2).contains(&trace.len()));
            for (index, frames) in trace.iter().enumerate() {
                assert_eq!(
                    frames,
                    &vec![
                        SETUP_1.to_vec(),
                        SETUP_2.to_vec(),
                        SETUP_3.to_vec(),
                        if index == 0 {
                            FIRST_REQUEST.to_vec()
                        } else {
                            BACKFILL_REQUEST.to_vec()
                        },
                    ]
                );
            }

            // Independent business clock bracket, not chosen by SDK output or wire trace.
            // Only [Shanghai sample-date midnight,15:00) is a forming daily row.
            // Crossing15:00 admits BOTH states only within the observed bracket.
            // Crossing midnight keeps the old sample date settled; it is not the new current day.
            let midnight = sample
                .day
                .checked_mul(86_400)
                .unwrap()
                .checked_sub(28_800)
                .unwrap();
            let close = midnight.checked_add(54_000).unwrap();
            let forming_possible = before < close && after >= midnight;
            let settled_possible = before < midnight || after >= close;
            match case {
                Case::Normal | Case::EmptyBackfill | Case::ExcessBackfill => {
                    if trace.len() == 1 {
                        assert!(
                            settled_possible,
                            "forming row was published without required backfill"
                        );
                        let selected = outcome.unwrap();
                        assert_eq!(selected.selected_provider(), ProviderId::Tdx);
                        assert_eq!(selected.attempts().len(), 1);
                        assert_eq!(selected.attempts()[0].provider_id(), ProviderId::Tdx);
                        assert_eq!(selected.attempts()[0].status(), &AttemptStatus::Selected);
                        assert_complete(
                            selected.batch(),
                            vec![
                                (&sample.previous, [0.010, 0.012, 0.009, 0.010]),
                                (&sample.current, [0.020, 0.022, 0.019, 0.020]),
                            ],
                            before,
                            after,
                        );
                    } else {
                        assert!(
                            forming_possible,
                            "settled row incorrectly caused a backfill"
                        );
                        if matches!(case, Case::Normal) {
                            let selected = outcome.unwrap();
                            assert_eq!(selected.selected_provider(), ProviderId::Tdx);
                            assert_eq!(selected.attempts().len(), 1);
                            assert_eq!(selected.attempts()[0].provider_id(), ProviderId::Tdx);
                            assert_eq!(selected.attempts()[0].status(), &AttemptStatus::Selected);
                            assert_complete(
                                selected.batch(),
                                vec![
                                    (&sample.older, [0.005, 0.007, 0.004, 0.005]),
                                    (&sample.previous, [0.010, 0.012, 0.009, 0.010]),
                                ],
                                before,
                                after,
                            );
                        } else {
                            let actual = if matches!(case, Case::EmptyBackfill) {
                                0
                            } else {
                                2
                            };
                            let error = outcome.unwrap_err();
                            assert_rejection(&error, &[format!(
                                "TDX historical bar cardinality mismatch at offset 2: returned {actual} rows for exact page limit 1 within total request 2"
                            )]);
                        }
                    }
                }
                Case::ZeroTailAmount | Case::InconsistentTailOhlc => {
                    assert_eq!(
                        trace.len(),
                        1,
                        "invalid source tail must fail before backfill"
                    );
                    let message = if matches!(case, Case::ZeroTailAmount) {
                        format!(
                            "Invalid data: TDX bar has positive volume with zero amount at {}",
                            sample.current
                        )
                    } else {
                        "Core contract error: invalid request: inconsistent OHLC range".into()
                    };
                    assert_rejection(&outcome.unwrap_err(), &[message]);
                }
                Case::DuplicateCurrentDay | Case::MisplacedCurrentDay => {
                    assert_eq!(
                        trace.len(),
                        1,
                        "invalid daily shape must never request replacement"
                    );
                    let mut permitted = Vec::new();
                    if forming_possible {
                        permitted.push(if matches!(case, Case::DuplicateCurrentDay) {
                            "Invalid data: TDX daily bars contain more than one current-day source row".into()
                        } else {
                            format!("Invalid data: TDX current-day daily row {} is not the newest source row", sample.current)
                        });
                    }
                    if settled_possible {
                        let date = if matches!(case, Case::DuplicateCurrentDay) {
                            &sample.current
                        } else {
                            &sample.previous
                        };
                        permitted.push(format!(
                            "Invalid data: TDX bar times are duplicate or non-increasing at {date}"
                        ));
                    }
                    assert!(!permitted.is_empty());
                    assert_rejection(&outcome.unwrap_err(), &permitted);
                }
            }
        }
    }
}
