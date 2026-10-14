use super::super::register_tdx_market_parity;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use magic_tdx_rs::TdxSmartClient;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SETUP_1: &[u8] = &[0x0c, 0x02, 0x18, 0x93, 0, 1, 3, 0, 3, 0, 0x0d, 0, 1];
const SETUP_2: &[u8] = &[0x0c, 0x02, 0x18, 0x94, 0, 1, 3, 0, 3, 0, 0x0d, 0, 2];
const SETUP_3: &[u8] = &[
    0x0c, 0x03, 0x18, 0x99, 0, 1, 0x20, 0, 0x20, 0, 0xdb, 0x0f, 0xd5, 0xd0, 0xc9, 0xcc, 0xd6, 0xa4,
    0xa8, 0xaf, 0, 0, 0, 0x8f, 0xc2, 0x25, 0x40, 0x13, 0, 0, 0xd5, 0, 0xc9, 0xcc, 0xbd, 0xf0, 0xd7,
    0xea, 0, 0, 0, 2,
];
const SETUP_RESPONSE: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 7];
const QUOTE_REQUEST: &[u8] = &[
    0x0c, 0x01, 0x20, 0x63, 0, 0x02, 0x13, 0, 0x13, 0, 0x3e, 0x05, 0x05, 0, 0, 0, 0, 0, 0, 0, 1, 0,
    1, 0x36, 0x30, 0x30, 0x30, 0x30, 0x30,
];
// External synthetic protocol bytes, not an SDK encoder/typed record.
const QUOTE_BODY: [u8; 62] = [
    0x01, 0x06, 0x01, 0, 0x01, 0x36, 0x30, 0x30, 0x30, 0x30, 0x30, 0, 0, 0xa8, 0x0f, 0, 0, 0, 0, 0,
    0, 0xa4, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const QUOTE_HEADER: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 62, 0, 62, 0];
const REQUEST_JSON: &[u8] =
    br#"{"instruments":[{"exchange":"Shanghai","code":"600000","asset_class":"Equity"}]}"#;

struct Gate {
    listener: Option<TcpListener>,
    shutdown: Option<TcpStream>,
}

struct Boundary {
    gate: Mutex<Gate>,
    stop: AtomicBool,
    deadline: Instant,
}

#[derive(Debug, Default)]
struct Trace {
    peers: Vec<SocketAddr>,
    requests: Vec<Vec<u8>>,
    trailing: Vec<u8>,
    cancelled_before_complete: bool,
    response_complete: bool,
}

struct Report {
    trace: Trace,
    result: io::Result<()>,
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|value| !value.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "owned quote fixture deadline"))
}

fn lock_gate(boundary: &Boundary) -> io::Result<MutexGuard<'_, Gate>> {
    loop {
        remaining(boundary.deadline)?;
        match boundary.gate.try_lock() {
            Ok(gate) => return Ok(gate),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(io::Error::other("quote fixture gate poisoned"))
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                thread::sleep(remaining(boundary.deadline)?.min(Duration::from_millis(2)));
            }
        }
    }
}

// Nonblocking accept, clone and publication are serialized with cancellation.
// No accepted stream can escape between an unlocked accept and registration.
fn accept_one(boundary: &Boundary, trace: &mut Trace) -> io::Result<Option<TcpStream>> {
    let mut gate = lock_gate(boundary)?;
    if boundary.stop.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let accepted = match gate.listener.as_ref() {
        Some(listener) => listener.accept(),
        None => return Ok(None),
    };
    match accepted {
        Ok((stream, peer)) => {
            trace.peers.push(peer); // lossless external attempt BEFORE guards
            if trace.peers.len() != 1 || peer.ip() != std::net::IpAddr::V4(Ipv4Addr::LOCALHOST) {
                stream.shutdown(Shutdown::Both)?;
                return Err(io::Error::other("unexpected quote fixture connection"));
            }
            let shutdown = match stream.try_clone() {
                Ok(shutdown) => shutdown,
                Err(error) => {
                    stream.shutdown(Shutdown::Both)?;
                    return Err(error);
                }
            };
            gate.shutdown = Some(shutdown);
            Ok(Some(stream))
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_to(
    stream: &mut TcpStream,
    bytes: &mut Vec<u8>,
    length: usize,
    boundary: &Boundary,
    trace: &mut Trace,
) -> io::Result<bool> {
    while bytes.len() < length {
        if boundary.stop.load(Ordering::SeqCst) {
            return Ok(false);
        }
        let _ = accept_one(boundary, trace)?; // any extra accept fails above
        stream.set_read_timeout(Some(
            remaining(boundary.deadline)?.min(Duration::from_millis(50)),
        ))?;
        let mut buffer = [0u8; 42];
        let count = (length - bytes.len()).min(buffer.len());
        match stream.read(&mut buffer[..count]) {
            Ok(0) if boundary.stop.load(Ordering::SeqCst) => return Ok(false),
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "quote fixture peer closed before cancellation",
                ))
            }
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                remaining(boundary.deadline)?;
            }
            Err(_) if boundary.stop.load(Ordering::SeqCst) => return Ok(false),
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

fn write_response(stream: &mut TcpStream, bytes: &[u8], boundary: &Boundary) -> io::Result<()> {
    let mut written = 0;
    while written < bytes.len() {
        if boundary.stop.load(Ordering::SeqCst) {
            return Err(io::Error::other("quote fixture cancelled during response"));
        }
        stream.set_write_timeout(Some(
            remaining(boundary.deadline)?.min(Duration::from_millis(50)),
        ))?;
        match stream.write(&bytes[written..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "quote fixture short response",
                ))
            }
            Ok(count) => written += count,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                remaining(boundary.deadline)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn serve(boundary: &Boundary, trace: &mut Trace) -> io::Result<()> {
    let mut stream = loop {
        if boundary.stop.load(Ordering::SeqCst) {
            trace.cancelled_before_complete = true;
            return Ok(());
        }
        if let Some(stream) = accept_one(boundary, trace)? {
            break stream;
        }
        thread::sleep(remaining(boundary.deadline)?.min(Duration::from_millis(2)));
    };
    for (index, expected) in [SETUP_1, SETUP_2, SETUP_3, QUOTE_REQUEST]
        .into_iter()
        .enumerate()
    {
        let mut observed = Vec::with_capacity(42);
        let read_result = (|| {
            if !read_to(&mut stream, &mut observed, 10, boundary, trace)? {
                return Ok(false);
            }
            let payload = usize::from(u16::from_le_bytes([observed[6], observed[7]]));
            if payload > 32 {
                return Err(io::Error::other(
                    "quote fixture request exceeds 42-byte cap",
                ));
            }
            read_to(&mut stream, &mut observed, 10 + payload, boundary, trace)
        })();
        trace.requests.push(observed); // retained even on short/error/invalid frame
        if !read_result? {
            trace.cancelled_before_complete = true;
            return Ok(());
        }
        if trace
            .requests
            .last()
            .is_none_or(|bytes| bytes.as_slice() != expected)
        {
            return Err(io::Error::other(
                "quote fixture unexpected complete request",
            ));
        }
        if index < 3 {
            write_response(&mut stream, SETUP_RESPONSE, boundary)?;
        } else {
            write_response(&mut stream, QUOTE_HEADER, boundary)?;
            write_response(&mut stream, &QUOTE_BODY, boundary)?;
            trace.response_complete = true;
        }
    }
    loop {
        if boundary.stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let _ = accept_one(boundary, trace)?;
        stream.set_read_timeout(Some(
            remaining(boundary.deadline)?.min(Duration::from_millis(50)),
        ))?;
        let mut buffer = [0u8; 32];
        match stream.read(&mut buffer) {
            Ok(0) if boundary.stop.load(Ordering::SeqCst) => return Ok(()),
            Ok(0) => {
                return Err(io::Error::other(
                    "quote stream closed before owner cancellation",
                ))
            }
            Ok(count) => {
                trace.trailing.extend_from_slice(&buffer[..count]);
                return Err(io::Error::other(
                    "quote fixture unexpected trailing request",
                ));
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                remaining(boundary.deadline)?;
            }
            Err(_) if boundary.stop.load(Ordering::SeqCst) => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

struct Fixture {
    client: Arc<TdxSmartClient>,
    boundary: Arc<Boundary>,
    worker: Option<JoinHandle<Report>>,
    finished: bool,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let endpoint: SocketAddrV4 = match listener.local_addr()? {
            SocketAddr::V4(endpoint) => endpoint,
            SocketAddr::V6(_) => return Err(io::Error::other("quote fixture requires IPv4").into()),
        };
        let client = Arc::new(TdxSmartClient::with_loopback_endpoints(&[endpoint])?);
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(15))
            .ok_or_else(|| io::Error::other("quote fixture deadline overflow"))?;
        let boundary = Arc::new(Boundary {
            gate: Mutex::new(Gate {
                listener: Some(listener),
                shutdown: None,
            }),
            stop: AtomicBool::new(false),
            deadline,
        });
        // Owner exists before any worker is created, including spawn failure.
        let mut fixture = Self {
            client,
            boundary,
            worker: None,
            finished: false,
        };
        let boundary = Arc::clone(&fixture.boundary);
        fixture.worker = Some(
            thread::Builder::new()
                .name("owned-smart-quote-fixture".into())
                .spawn(move || {
                    let mut trace = Trace::default();
                    let result = serve(&boundary, &mut trace);
                    Report { trace, result }
                })?,
        );
        Ok(fixture)
    }

    fn finish(&mut self) -> (Vec<String>, Option<Report>) {
        let mut errors = Vec::new();
        self.boundary.stop.store(true, Ordering::SeqCst);
        let cancelled = (|| {
            let (listener, shutdown) = {
                let mut gate = lock_gate(&self.boundary)?;
                (gate.listener.take(), gate.shutdown.take())
            };
            drop(listener);
            if let Some(shutdown) = shutdown {
                shutdown.shutdown(Shutdown::Both)?;
            }
            Ok::<(), io::Error>(())
        })();
        if let Err(error) = cancelled {
            errors.push(format!("fixture cancel: {error}"));
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.client.close_loopback_checked()
        })) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(format!("SDK checked close: {error}")),
            Err(_) => errors.push("SDK checked close panicked".into()),
        }
        let report = self.worker.take().and_then(|worker| match worker.join() {
            Ok(report) => {
                if let Err(error) = &report.result {
                    errors.push(format!("fixture worker: {error}"));
                }
                Some(report)
            }
            Err(_) => {
                errors.push("fixture worker panicked".into());
                None
            }
        });
        self.finished = true;
        (errors, report)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !self.finished {
            let (errors, _) = self.finish();
            for error in errors {
                eprintln!("quote fixture fallback cleanup: {error}");
            }
        }
    }
}

fn unix_seconds() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| io::Error::other(error.to_string()))
}

#[test]
fn registered_smart_quotes_preserve_unavailable_source_evidence_over_owned_tcp(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = Fixture::new()?;
    let mut registry = OperationRegistry::all_unadmitted("offline setup only");
    register_tdx_market_parity(&mut registry, Arc::clone(&fixture.client), 5.0, 32768)?;
    let payload = CanonicalPayload::new(
        "magic.market.realtime_quotes.request",
        1,
        REQUEST_JSON.to_vec(),
        32768,
    )?;
    let command = QueryCommand::new(
        "smart-owned-quote",
        Operation::RealtimeQuotes,
        Some("Tdx".into()),
        payload,
    )?;
    let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let before = unix_seconds()?;
        let result = registry.execute(command);
        let after = unix_seconds()?;
        Ok::<_, io::Error>((before, result, after))
    }));
    let (cleanup_errors, report) = fixture.finish(); // cancel -> checked close -> ALL joins
    let observed = match observed {
        Ok(observed) => observed,
        Err(panic) => {
            for error in cleanup_errors {
                eprintln!("quote cleanup after primary panic: {error}");
            }
            std::panic::resume_unwind(panic);
        }
    };
    if !cleanup_errors.is_empty() {
        return Err(io::Error::other(format!(
            "cleanup failed: {cleanup_errors:?}; observed={observed:?}"
        ))
        .into());
    }
    let (before, result, after) = observed?;
    // Current implementation is expected to fail here with exact Unsupported.
    // That is NOT accepted as success or an actual RED until ROOT runs it.
    let report = report.ok_or_else(|| io::Error::other("quote fixture report missing"))?;
    if let Err(error) = &result {
        assert_eq!(
            error,
            &ServiceError::Unsupported {
                operation: Operation::RealtimeQuotes,
                reason: "isolated TDX runtime is not implemented".into(),
            }
        );
        assert_eq!(report.trace.peers.len(), 1);
        assert_eq!(
            report
                .trace
                .requests
                .iter()
                .filter(|bytes| !bytes.is_empty())
                .cloned()
                .collect::<Vec<_>>(),
            vec![SETUP_1.to_vec(), SETUP_2.to_vec(), SETUP_3.to_vec()]
        );
        assert!(report.trace.trailing.is_empty());
        assert!(report.trace.cancelled_before_complete);
        assert!(!report.trace.response_complete);
        panic!("expected successful real Smart quote query; current scoped runtime RED: {error:?}");
    }
    let result = result?;
    assert_eq!(report.trace.peers.len(), 1);
    assert_eq!(
        report.trace.peers[0].ip(),
        std::net::IpAddr::V4(Ipv4Addr::LOCALHOST)
    );
    assert_eq!(
        report.trace.requests,
        vec![
            SETUP_1.to_vec(),
            SETUP_2.to_vec(),
            SETUP_3.to_vec(),
            QUOTE_REQUEST.to_vec()
        ]
    );
    assert!(report.trace.trailing.is_empty());
    assert!(!report.trace.cancelled_before_complete);
    assert!(report.trace.response_complete);
    let observed_seconds: u64 = result.observed_at.parse()?;
    assert!(observed_seconds > 0 && before <= observed_seconds && observed_seconds <= after);
    assert_eq!(result.observed_at, observed_seconds.to_string());
    let batch_id = format!("tdx-smart:{}:quote", observed_seconds);
    assert_eq!(result.provider, "Tdx");
    assert_eq!(result.batch_id, batch_id);
    assert_eq!(result.source_at, None);
    assert!(!result.complete);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.records.len(), 1);
    let record = &result.records[0];
    assert_eq!(record.schema(), "magic.market.quote");
    assert_eq!(record.schema_version(), 1);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(record.data())?,
        serde_json::json!({
            "instrument": {"exchange":"Shanghai","code":"600000","asset_class":"Equity"},
            "name":null,"price":10.0,"previous_close":10.0,"open":10.0,"high":10.0,"low":10.0,
            "change_percent":{"value":0.0,"unit":"Percent"},"volume":100.0,"amount":0.0,
            "status":"Unavailable","source_at":null,"observed_at":observed_seconds.to_string(),
            "provider":"Tdx","batch_id":batch_id
        })
    );
    Ok(())
}

fn serve_cancelled_quote(
    boundary: &Boundary,
    trace: &mut Trace,
    query_seen: std::sync::mpsc::Sender<()>,
    client_close_done: &AtomicBool,
) -> io::Result<()> {
    let mut stream = loop {
        if boundary.stop.load(Ordering::SeqCst) {
            trace.cancelled_before_complete = true;
            return Ok(());
        }
        if let Some(stream) = accept_one(boundary, trace)? {
            break stream;
        }
        thread::sleep(remaining(boundary.deadline)?.min(Duration::from_millis(2)));
    };
    for (index, expected) in [SETUP_1, SETUP_2, SETUP_3, QUOTE_REQUEST]
        .into_iter()
        .enumerate()
    {
        let mut observed = Vec::with_capacity(42);
        let read_result = (|| {
            if !read_to(&mut stream, &mut observed, 10, boundary, trace)? {
                return Ok(false);
            }
            let payload = usize::from(u16::from_le_bytes([observed[6], observed[7]]));
            if payload > 32 {
                return Err(io::Error::other(
                    "cancel fixture request exceeds 42-byte cap",
                ));
            }
            read_to(&mut stream, &mut observed, 10 + payload, boundary, trace)
        })();
        trace.requests.push(observed); // complete/partial external bytes BEFORE guards
        if !read_result? {
            trace.cancelled_before_complete = true;
            return Ok(());
        }
        if trace
            .requests
            .last()
            .is_none_or(|bytes| bytes.as_slice() != expected)
        {
            return Err(io::Error::other(
                "cancel fixture unexpected complete request",
            ));
        }
        if index < 3 {
            write_response(&mut stream, SETUP_RESPONSE, boundary)?;
        }
    }
    // This notification proves only the complete external quote request arrived.
    // No quote header or payload is sent, and no SDK/private state is inspected.
    query_seen
        .send(())
        .map_err(|_| io::Error::other("cancel fixture observer dropped"))?;
    loop {
        if boundary.stop.load(Ordering::SeqCst) {
            trace.cancelled_before_complete = true;
            wait_client_close(boundary, client_close_done)?;
            return Ok(());
        }
        let _ = accept_one(boundary, trace)?;
        stream.set_read_timeout(Some(
            remaining(boundary.deadline)?.min(Duration::from_millis(50)),
        ))?;
        let mut buffer = [0u8; 32];
        match stream.read(&mut buffer) {
            Ok(0) if boundary.stop.load(Ordering::SeqCst) => {
                trace.cancelled_before_complete = true;
                wait_client_close(boundary, client_close_done)?;
                return Ok(());
            }
            Ok(0) => {
                return Err(io::Error::other(
                    "cancel quote peer closed before owner cancellation",
                ))
            }
            Ok(count) => {
                trace.trailing.extend_from_slice(&buffer[..count]);
                return Err(io::Error::other(
                    "cancel quote fixture received an extra request",
                ));
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                remaining(boundary.deadline)?;
            }
            Err(_) if boundary.stop.load(Ordering::SeqCst) => {
                trace.cancelled_before_complete = true;
                wait_client_close(boundary, client_close_done)?;
                return Ok(());
            }
            Err(error) => return Err(error),
        }
    }
}

fn wait_client_close(boundary: &Boundary, done: &AtomicBool) -> io::Result<()> {
    // Cancellation ends source I/O, but retains the passive peer handle until
    // checked client close returns. Do not inject a peer reset into that close.
    while !done.load(Ordering::SeqCst) {
        thread::sleep(remaining(boundary.deadline)?.min(Duration::from_millis(2)));
    }
    Ok(())
}

type CancelQuoteOutcome = Result<magic_market_service::QueryResult, ServiceError>;

struct CancelQuoteFixture {
    client: Arc<TdxSmartClient>,
    endpoint: SocketAddrV4,
    boundary: Arc<Boundary>,
    client_close_done: Arc<AtomicBool>,
    query_seen: std::sync::mpsc::Receiver<()>,
    server: Option<JoinHandle<Report>>,
    query: Option<JoinHandle<CancelQuoteOutcome>>,
    finished: bool,
}

impl CancelQuoteFixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let endpoint = match listener.local_addr()? {
            SocketAddr::V4(endpoint) => endpoint,
            SocketAddr::V6(_) => {
                return Err(io::Error::other("cancel fixture requires IPv4").into())
            }
        };
        let client = Arc::new(TdxSmartClient::with_loopback_endpoints(&[endpoint])?);
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(15))
            .ok_or_else(|| io::Error::other("cancel fixture deadline overflow"))?;
        let boundary = Arc::new(Boundary {
            gate: Mutex::new(Gate {
                listener: Some(listener),
                shutdown: None,
            }),
            stop: AtomicBool::new(false),
            deadline,
        });
        let (query_seen, observer) = std::sync::mpsc::channel();
        let client_close_done = Arc::new(AtomicBool::new(false));
        // The owner exists before either spawn; spawn errors still cancel/close.
        let mut fixture = Self {
            client,
            endpoint,
            boundary,
            client_close_done,
            query_seen: observer,
            server: None,
            query: None,
            finished: false,
        };
        let boundary = Arc::clone(&fixture.boundary);
        let client_close_done = Arc::clone(&fixture.client_close_done);
        fixture.server = Some(
            thread::Builder::new()
                .name("owned-smart-cancel-source".into())
                .spawn(move || {
                    let mut trace = Trace::default();
                    let result = serve_cancelled_quote(
                        &boundary,
                        &mut trace,
                        query_seen,
                        &client_close_done,
                    );
                    Report { trace, result }
                })?,
        );
        Ok(fixture)
    }

    fn start_public_query(&mut self) -> io::Result<()> {
        if self.query.is_some() {
            return Err(io::Error::other("cancel fixture query already started"));
        }
        let client = Arc::clone(&self.client);
        self.query = Some(
            thread::Builder::new()
                .name("owned-smart-cancel-query".into())
                .spawn(move || {
                    let mut registry = OperationRegistry::all_unadmitted("offline setup only");
                    register_tdx_market_parity(&mut registry, client, 5.0, 32768)?;
                    let payload = CanonicalPayload::new(
                        "magic.market.realtime_quotes.request",
                        1,
                        REQUEST_JSON.to_vec(),
                        32768,
                    )?;
                    let command = QueryCommand::new(
                        "smart-owned-cancel",
                        Operation::RealtimeQuotes,
                        Some("Tdx".into()),
                        payload,
                    )?;
                    registry.execute(command)
                })?,
        );
        Ok(())
    }

    fn finish(
        &mut self,
    ) -> (
        Vec<String>,
        Option<Report>,
        Option<std::thread::Result<CancelQuoteOutcome>>,
    ) {
        let mut errors = Vec::new();
        // Required order: external cancel -> SDK checked close -> ALL joins.
        self.boundary.stop.store(true, Ordering::SeqCst);
        let cancelled = (|| {
            let listener = {
                let mut gate = lock_gate(&self.boundary)?;
                gate.listener.take()
            };
            drop(listener);
            Ok::<(), io::Error>(())
        })();
        if let Err(error) = cancelled {
            errors.push(format!("cancel boundary: {error}"));
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.client.close_loopback_checked()
        })) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(format!("cancel SDK checked close: {error}")),
            Err(_) => errors.push("cancel SDK checked close panicked".into()),
        }
        self.client_close_done.store(true, Ordering::SeqCst);
        match lock_gate(&self.boundary) {
            Ok(mut gate) => drop(gate.shutdown.take()),
            Err(error) => errors.push(format!("release cancelled peer handle: {error}")),
        }
        // No detach/timed-join fiction: both owned handles are actually joined.
        // Fixture loops/socket timeouts and SDK operation/close deadlines are bounded;
        // if those contracts fail, the outer owned Job must reject the attempt.
        let query = self.query.take().map(JoinHandle::join);
        if query.as_ref().is_some_and(|result| result.is_err()) {
            errors.push("cancel query worker panicked".into());
        }
        let report = self.server.take().and_then(|server| match server.join() {
            Ok(report) => {
                if let Err(error) = &report.result {
                    errors.push(format!("cancel source worker: {error}"));
                }
                Some(report)
            }
            Err(_) => {
                errors.push("cancel source worker panicked".into());
                None
            }
        });
        self.finished = true;
        (errors, report, query)
    }
}

impl Drop for CancelQuoteFixture {
    fn drop(&mut self) {
        if !self.finished {
            let (errors, _, _) = self.finish();
            for error in errors {
                eprintln!("cancel fixture fallback cleanup: {error}");
            }
        }
    }
}

fn assert_closed_quote_alias<T>(result: Result<T, magic_tdx_rs::TdxError>) {
    match result {
        Err(magic_tdx_rs::TdxError::Disconnected) => {}
        Err(error) => panic!("closed quote alias returned wrong error: {error:?}"),
        Ok(_) => panic!("closed quote alias reopened or returned query data"),
    }
}

#[test]
fn registered_smart_quote_cancel_closes_scope_and_refuses_retained_aliases(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = CancelQuoteFixture::new()?;
    let first = Arc::clone(&fixture.client);
    let second = Arc::clone(&fixture.client);
    // Both inner aliases are retained while Open, before the public query starts.
    let first_inner = first.inner();
    let second_inner = second.inner();
    fixture.start_public_query()?;
    let fourth_request_seen = fixture.query_seen.recv_timeout(Duration::from_secs(4));
    let still_pending = fixture
        .query
        .as_ref()
        .is_some_and(|query| !query.is_finished());
    let (cleanup_errors, report, query) = fixture.finish();
    if !cleanup_errors.is_empty() {
        return Err(io::Error::other(format!(
            "checked cancellation did not close cleanly: {cleanup_errors:?}"
        ))
        .into());
    }
    // Assert only AFTER every fixture/query thread has been joined.
    fourth_request_seen.map_err(|error| {
        io::Error::other(format!("complete quote request was not observed: {error}"))
    })?;
    assert!(
        still_pending,
        "public query finished before cancellation of the held response"
    );
    let report = report.ok_or_else(|| io::Error::other("cancel source report missing"))?;
    assert_eq!(report.trace.peers.len(), 1);
    assert_eq!(
        report.trace.peers[0].ip(),
        std::net::IpAddr::V4(Ipv4Addr::LOCALHOST)
    );
    assert_eq!(
        report.trace.requests,
        vec![
            SETUP_1.to_vec(),
            SETUP_2.to_vec(),
            SETUP_3.to_vec(),
            QUOTE_REQUEST.to_vec(),
        ]
    );
    assert!(report.trace.trailing.is_empty());
    assert!(report.trace.cancelled_before_complete);
    assert!(
        !report.trace.response_complete,
        "no quote header/body was authorized by the source"
    );
    let query = query
        .ok_or_else(|| io::Error::other("cancel query handle missing"))?
        .map_err(|_| io::Error::other("cancel query worker panicked"))?;
    match query {
        Err(ServiceError::Unavailable { operation, reason }) => {
            assert_eq!(operation, Operation::RealtimeQuotes);
            assert!(!reason.trim().is_empty());
        }
        Err(error) => panic!("cancelled public query returned wrong typed error: {error:?}"),
        Ok(_) => panic!("cancelled public query published data without a source payload"),
    }
    // No fail-closed error is accepted as successful SDK close; idempotent close
    // and retained-alias Disconnected are required after the successful first close.
    first.close_loopback_checked()?;
    second.close_loopback_checked()?;
    for client in [&first, &second] {
        assert_closed_quote_alias(client.connect_to_any(Some(5.0)));
        assert_closed_quote_alias(client.get_security_quotes(&[(1, "600000")]));
    }
    for inner in [first_inner, second_inner] {
        assert_closed_quote_alias(inner.connect("127.0.0.1", fixture.endpoint.port(), Some(5.0)));
        assert_closed_quote_alias(inner.connect_to_any(Some(5.0)));
        assert_closed_quote_alias(inner.get_security_quotes(&[(1, "600000")]));
        assert_closed_quote_alias(inner.send_raw_and_recv(QUOTE_REQUEST));
    }
    first.close_loopback_checked()?;
    Ok(())
}

// Public book atomicity/evidence characterization, external wire fixture only.
// Five-level ask delta #5 is -1001; base1000 => -1, never a valid Core Price.
const ORDER_BOOK_NEGATIVE_ASK5_BODY: [u8; 63] = [
    0x01, 0x06, 0x01, 0, 0x01, 0x36, 0x30, 0x30, 0x30, 0x30, 0x30, 0, 0, 0xa8, 0x0f, 0, 0, 0, 0, 0,
    0, 0xa4, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0xe9, 0x0f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn serve_order_book_response(
    boundary: &Boundary,
    trace: &mut Trace,
    body: &[u8],
    expected_parse_failure: bool,
    client_close_done: &AtomicBool,
) -> io::Result<()> {
    let mut stream = loop {
        if boundary.stop.load(Ordering::SeqCst) {
            trace.cancelled_before_complete = true;
            wait_client_close(boundary, client_close_done)?;
            return Ok(());
        }
        if let Some(stream) = accept_one(boundary, trace)? {
            break stream;
        }
        thread::sleep(remaining(boundary.deadline)?.min(Duration::from_millis(2)));
    };
    for (index, expected) in [SETUP_1, SETUP_2, SETUP_3, QUOTE_REQUEST]
        .into_iter()
        .enumerate()
    {
        let mut observed = Vec::with_capacity(42);
        let read_result = (|| {
            if !read_to(&mut stream, &mut observed, 10, boundary, trace)? {
                return Ok(false);
            }
            let payload = usize::from(u16::from_le_bytes([observed[6], observed[7]]));
            if payload > 32 {
                return Err(io::Error::other("book request exceeds 42-byte cap"));
            }
            read_to(&mut stream, &mut observed, 10 + payload, boundary, trace)
        })();
        // Preserve full/partial attempted raw requests BEFORE propagating guards.
        trace.requests.push(observed);
        if !read_result? {
            trace.cancelled_before_complete = true;
            wait_client_close(boundary, client_close_done)?;
            return Ok(());
        }
        if trace
            .requests
            .last()
            .is_none_or(|bytes| bytes.as_slice() != expected)
        {
            return Err(io::Error::other("book fixture unexpected complete request"));
        }
        if index < 3 {
            write_response(&mut stream, SETUP_RESPONSE, boundary)?;
        } else {
            let header: &[u8] = if expected_parse_failure {
                &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 63, 0, 63, 0]
            } else {
                QUOTE_HEADER
            };
            write_response(&mut stream, header, boundary)?;
            write_response(&mut stream, body, boundary)?;
            trace.response_complete = true;
        }
    }
    let mut peer_eof = false;
    loop {
        if boundary.stop.load(Ordering::SeqCst) {
            // Keep the passive peer handle alive until checked SDK close returns.
            wait_client_close(boundary, client_close_done)?;
            return Ok(());
        }
        let _ = accept_one(boundary, trace)?; // retries/additional peers still fail
        if peer_eof {
            thread::sleep(remaining(boundary.deadline)?.min(Duration::from_millis(2)));
            continue;
        }
        stream.set_read_timeout(Some(
            remaining(boundary.deadline)?.min(Duration::from_millis(50)),
        ))?;
        let mut buffer = [0u8; 32];
        match stream.read(&mut buffer) {
            Ok(0) if boundary.stop.load(Ordering::SeqCst) => {
                wait_client_close(boundary, client_close_done)?;
                return Ok(());
            }
            Ok(0) if expected_parse_failure => {
                // Parser refusal may close its lease. EOF is NOT a business PASS.
                // Continue observing additional connections until owner cancellation.
                peer_eof = true;
            }
            Ok(0) => {
                return Err(io::Error::other(
                    "normal book peer closed before cancellation",
                ))
            }
            Ok(count) => {
                trace.trailing.extend_from_slice(&buffer[..count]);
                return Err(io::Error::other(
                    "book fixture received extra request bytes",
                ));
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                remaining(boundary.deadline)?;
            }
            Err(_) if boundary.stop.load(Ordering::SeqCst) => {
                wait_client_close(boundary, client_close_done)?;
                return Ok(());
            }
            Err(error) => return Err(error),
        }
    }
}

impl CancelQuoteFixture {
    fn new_order_book_response(
        body: &'static [u8],
        expected_parse_failure: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let endpoint = match listener.local_addr()? {
            SocketAddr::V4(endpoint) => endpoint,
            SocketAddr::V6(_) => return Err(io::Error::other("book fixture requires IPv4").into()),
        };
        let client = Arc::new(TdxSmartClient::with_loopback_endpoints(&[endpoint])?);
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(15))
            .ok_or_else(|| io::Error::other("book fixture deadline overflow"))?;
        let boundary = Arc::new(Boundary {
            gate: Mutex::new(Gate {
                listener: Some(listener),
                shutdown: None,
            }),
            stop: AtomicBool::new(false),
            deadline,
        });
        let (_unused_sender, observer) = std::sync::mpsc::channel();
        let client_close_done = Arc::new(AtomicBool::new(false));
        // Existing RAII owner and checked cancel->close->ALL-joins lifecycle.
        // Owner exists before spawn, including a failed spawn.
        let mut fixture = Self {
            client,
            endpoint,
            boundary,
            client_close_done,
            query_seen: observer,
            server: None,
            query: None,
            finished: false,
        };
        let boundary = Arc::clone(&fixture.boundary);
        let client_close_done = Arc::clone(&fixture.client_close_done);
        fixture.server = Some(
            thread::Builder::new()
                .name("owned-smart-book-source".into())
                .spawn(move || {
                    let mut trace = Trace::default();
                    let result = serve_order_book_response(
                        &boundary,
                        &mut trace,
                        body,
                        expected_parse_failure,
                        &client_close_done,
                    );
                    Report { trace, result }
                })?,
        );
        Ok(fixture)
    }

    fn start_public_order_book_query(&mut self) -> io::Result<()> {
        if self.query.is_some() {
            return Err(io::Error::other("book query already started"));
        }
        let client = Arc::clone(&self.client);
        self.query = Some(
            thread::Builder::new()
                .name("owned-smart-book-query".into())
                .spawn(move || {
                    let mut registry = OperationRegistry::all_unadmitted("offline setup only");
                    register_tdx_market_parity(&mut registry, client, 5.0, 32768)?;
                    let payload = CanonicalPayload::new(
                        "magic.market.order_books.request",
                        1,
                        REQUEST_JSON.to_vec(),
                        32768,
                    )?;
                    let command = QueryCommand::new(
                        "smart-owned-order-book",
                        Operation::OrderBooks,
                        Some("Tdx".into()),
                        payload,
                    )?;
                    registry.execute(command)
                })?,
        );
        Ok(())
    }
}

fn observe_public_order_book_response(
    body: &'static [u8],
    expected_parse_failure: bool,
) -> Result<(u64, CancelQuoteOutcome, u64, Trace), Box<dyn std::error::Error>> {
    let mut fixture = CancelQuoteFixture::new_order_book_response(body, expected_parse_failure)?;
    let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let before = unix_seconds()?;
        fixture.start_public_order_book_query()?;
        while fixture
            .query
            .as_ref()
            .is_some_and(|query| !query.is_finished())
        {
            thread::sleep(remaining(fixture.boundary.deadline)?.min(Duration::from_millis(2)));
        }
        let after = unix_seconds()?;
        Ok::<_, io::Error>((before, after))
    }));
    // No assertion, early return, or panic escapes before actual joins.
    let (cleanup_errors, report, query) = fixture.finish();
    let observed = match observed {
        Ok(observed) => observed,
        Err(panic) => {
            for error in cleanup_errors {
                eprintln!("book cleanup after primary panic: {error}");
            }
            std::panic::resume_unwind(panic);
        }
    };
    if !cleanup_errors.is_empty() {
        return Err(io::Error::other(format!(
            "book cleanup failed: {cleanup_errors:?}; observed={observed:?}; query={query:?}"
        ))
        .into());
    }
    let (before, after) = observed?;
    let report = report.ok_or_else(|| io::Error::other("book source report missing"))?;
    let query = query
        .ok_or_else(|| io::Error::other("book query handle missing"))?
        .map_err(|_| io::Error::other("book query worker panicked"))?;
    Ok((before, query, after, report.trace))
}

fn assert_public_order_book_attempts(trace: &Trace) {
    assert_eq!(trace.peers.len(), 1);
    assert_eq!(
        trace.peers[0].ip(),
        std::net::IpAddr::V4(Ipv4Addr::LOCALHOST)
    );
    assert_eq!(
        trace.requests,
        vec![
            SETUP_1.to_vec(),
            SETUP_2.to_vec(),
            SETUP_3.to_vec(),
            QUOTE_REQUEST.to_vec()
        ]
    );
    assert!(trace.trailing.is_empty());
    assert!(!trace.cancelled_before_complete);
    assert!(trace.response_complete);
}

#[test]
fn registered_smart_order_books_preserve_zero_depth_and_reject_negative_last_ask(
) -> Result<(), Box<dyn std::error::Error>> {
    let (before, normal, after, normal_trace) =
        observe_public_order_book_response(&QUOTE_BODY, false)?;
    assert_public_order_book_attempts(&normal_trace);
    let normal = normal?;
    let observed_seconds: u64 = normal.observed_at.parse()?;
    assert!(observed_seconds > 0 && before <= observed_seconds && observed_seconds <= after);
    assert_eq!(normal.observed_at, observed_seconds.to_string());
    let batch_id = format!("tdx-smart:{observed_seconds}:order-book");
    assert_eq!(normal.provider, "Tdx");
    assert_eq!(normal.batch_id, batch_id);
    assert_eq!(normal.source_at, None);
    assert!(!normal.complete); // Source-time format remains unverified.
    assert!(normal.repository_admitted);
    assert_eq!(normal.diagnostic_blocker, None);
    assert_eq!(normal.records.len(), 1);
    let book = &normal.records[0];
    assert_eq!(book.schema(), "magic.market.order_book");
    assert_eq!(book.schema_version(), 1);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(book.data())?,
        serde_json::json!({
            "instrument": {"exchange":"Shanghai","code":"600000","asset_class":"Equity"},
            "bids": [
                {"price":10.0,"quantity":0.0},
                {"price":10.0,"quantity":0.0},
                {"price":10.0,"quantity":0.0},
                {"price":10.0,"quantity":0.0},
                {"price":10.0,"quantity":0.0}
            ],
            "asks": [
                {"price":10.0,"quantity":0.0},
                {"price":10.0,"quantity":0.0},
                {"price":10.0,"quantity":0.0},
                {"price":10.0,"quantity":0.0},
                {"price":10.0,"quantity":0.0}
            ],
            "total_bid_quantity":0.0,
            "total_ask_quantity":0.0,
            "status":"Unavailable",
            "source_at":null,
            "observed_at":observed_seconds.to_string(),
            "provider":"Tdx",
            "batch_id":batch_id
        })
    );
    let (_, rejected, _, rejected_trace) =
        observe_public_order_book_response(&ORDER_BOOK_NEGATIVE_ASK5_BODY, true)?;
    assert_public_order_book_attempts(&rejected_trace);
    match rejected {
        Err(ServiceError::FailedPrecondition(reason)) => {
            assert_eq!(
                reason,
                "[E3005] type mismatch: quote row 0 ask level 5 price is negative"
            );
        }
        Err(error) => panic!("negative final ask returned wrong public error: {error:?}"),
        Ok(_) => panic!("negative final ask published a successful QueryResult or book prefix"),
    }
    Ok(())
}
