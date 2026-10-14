#![cfg(feature = "test-support")]

use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};

use magic_tdx_rs::{TdxError, TdxSmartClient};

fn assert_initial_public_state(client: &TdxSmartClient) {
    assert!(!client.inner().is_connected());
    assert_eq!(client.inner().connected_server(), None);
    let pool = client.inner().pool_stats();
    assert_eq!((pool.idle, pool.active, pool.total), (0, 0, 0));
    assert_eq!(
        client.cache_stats(),
        "last_success: None, blacklist: 0, stats: 0"
    );
}

fn assert_invalid_scope(endpoints: &[SocketAddrV4], reason: &str) {
    match TdxSmartClient::with_loopback_endpoints(endpoints) {
        Err(TdxError::InvalidData(actual)) => assert_eq!(actual, reason),
        Err(other) => panic!("expected InvalidData({reason:?}), got {other:?}"),
        Ok(_) => panic!("invalid endpoint scope was accepted: {reason}"),
    }
}

#[test]
fn loopback_constructor_validates_fixed_scope_and_starts_idle(
) -> Result<(), Box<dyn std::error::Error>> {
    // Setup owns the listeners throughout every constructor call. No worker,
    // outgoing connection, protocol request, sleep, environment or cache access.
    let listeners = [
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?,
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?,
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?,
    ];
    for listener in &listeners {
        listener.set_nonblocking(true)?;
    }
    let endpoints = listeners
        .iter()
        .map(|listener| match listener.local_addr()? {
            SocketAddr::V4(endpoint) => Ok(endpoint),
            SocketAddr::V6(_) => Err(std::io::Error::other("expected IPv4 listener")),
        })
        .collect::<std::io::Result<Vec<_>>>()?;

    // Both accepted cardinality boundaries have independent idle-state oracles.
    for scope in [&endpoints[..1], &endpoints[..2]] {
        let client = TdxSmartClient::with_loopback_endpoints(scope)?;
        assert_initial_public_state(&client);
        // Deliberately do not use discovery, connect, probe, query, clear_cache,
        // or checked close in this constructor-only slice.
        drop(client);
    }

    assert_invalid_scope(&[], "isolated TDX endpoints must contain 1 or 2 entries");
    assert_invalid_scope(
        &endpoints,
        "isolated TDX endpoints must contain 1 or 2 entries",
    );
    assert_invalid_scope(
        &[endpoints[0], endpoints[0]],
        "isolated TDX endpoints must be distinct",
    );
    assert_invalid_scope(
        &[SocketAddrV4::new(
            Ipv4Addr::new(127, 0, 0, 2),
            endpoints[0].port(),
        )],
        "isolated TDX endpoints must use 127.0.0.1",
    );
    assert_invalid_scope(
        &[SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)],
        "isolated TDX endpoint ports must be nonzero",
    );

    // This bounded external observation detects a queued constructor connection;
    // it is NOT a proof of absence of disk I/O, delayed work or other destinations.
    for listener in &listeners {
        match listener.accept() {
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
            Ok((stream, _)) => {
                drop(stream);
                panic!("constructor queued a connection to a retained listener");
            }
        }
    }
    Ok(())
}

struct IdleCloseLoopbackFixture {
    listeners: Vec<TcpListener>,
    client: TdxSmartClient,
    checked_closed: bool,
}

impl Drop for IdleCloseLoopbackFixture {
    fn drop(&mut self) {
        // Cancel the external boundary before checked SDK close, including panic.
        // This fixture never starts an accept worker or retains an accepted stream.
        self.listeners.clear();
        if !self.checked_closed {
            if let Err(error) = self.client.close_loopback_checked() {
                eprintln!("idle loopback fixture fallback close failed: {error}");
            }
        }
    }
}

fn assert_idle_runtime_refusal<T>(result: Result<T, TdxError>, closed: bool) {
    match (closed, result) {
        (false, Err(TdxError::Unsupported(reason))) => {
            assert_eq!(reason, "isolated TDX runtime is not implemented");
        }
        (true, Err(TdxError::Disconnected)) => {}
        (_, Err(other)) => panic!("unexpected idle runtime refusal: {other:?}"),
        (_, Ok(_)) => panic!("idle isolated runtime returned success/data"),
    }
}

fn assert_idle_alias_refuses(
    client: &TdxSmartClient,
    retained_inner: &magic_tdx_rs::TdxHqClient,
    endpoint: SocketAddrV4,
    closed: bool,
) {
    if closed {
        assert_idle_runtime_refusal(client.connect_to_any(Some(0.02)), true);
    } else {
        // First-stage Smart handshake is enabled; invalid timeout must reject before any I/O.
        match client.connect_to_any(Some(0.0)) {
            Err(TdxError::Connection(reason)) => {
                assert_eq!(reason, "isolated timeout must satisfy 0 < seconds <= 5")
            }
            Err(other) => panic!("unexpected isolated timeout refusal: {other:?}"),
            Ok(_) => panic!("zero isolated timeout was accepted"),
        }
    }
    assert_idle_runtime_refusal(client.get_security_quotes(&[(1, "600000")]), closed);
    assert_idle_runtime_refusal(
        retained_inner.connect("127.0.0.1", endpoint.port(), Some(0.02)),
        closed,
    );
    assert_idle_runtime_refusal(retained_inner.connect_to_any(Some(0.02)), closed);
    assert_idle_runtime_refusal(retained_inner.get_security_quotes(&[(1, "600000")]), closed);
    // Legal empty input must not turn an isolated refusal into success via a shortcut.
    assert_idle_runtime_refusal(
        retained_inner.fetch_context_for_factors(4, 1, "600000", &[], &[]),
        closed,
    );
    // These unchanged Vec interfaces are transitional refusal, NOT source data.
    assert!(client.probe_and_cache(0.02).is_empty());
    assert!(retained_inner.probe_servers(0.02).is_empty());
    assert_initial_public_state(client);
}

fn exercise_idle_public_mutators(
    client: &TdxSmartClient,
    retained_inner: &magic_tdx_rs::TdxHqClient,
    endpoint: SocketAddrV4,
) {
    let servers = [("retained fixture", "127.0.0.1", endpoint.port())];
    retained_inner.set_servers(&servers);
    retained_inner.add_server("retained fixture", "127.0.0.1", endpoint.port());
    retained_inner.reorder_servers(&servers);
    retained_inner.block_server("127.0.0.1", endpoint.port());
    retained_inner.unblock_server("127.0.0.1", endpoint.port());
    retained_inner.clear_blocked_servers();
    retained_inner.set_auto_retry(true);
    retained_inner.set_cache_ttl(1);
    retained_inner.set_connect_timeout(0.02);
    retained_inner.set_rate_limit(1);
    retained_inner.set_rate_limit_daily(1);
    retained_inner.disconnect();
    client.clear_cache();
    assert!(retained_inner.blocked_servers().is_empty());
    assert_initial_public_state(client);
}

#[test]
fn idle_loopback_close_irreversibly_refuses_retained_inner_aliases(
) -> Result<(), Box<dyn std::error::Error>> {
    let listeners = vec![
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?,
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?,
    ];
    for listener in &listeners {
        listener.set_nonblocking(true)?;
    }
    let endpoints = listeners
        .iter()
        .map(|listener| match listener.local_addr()? {
            SocketAddr::V4(endpoint) => Ok(endpoint),
            SocketAddr::V6(_) => Err(std::io::Error::other("expected IPv4 listener")),
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    let client = TdxSmartClient::with_loopback_endpoints(&endpoints)?;
    let mut fixture = IdleCloseLoopbackFixture {
        listeners,
        client,
        checked_closed: false,
    };
    let retained_inner = fixture.client.inner();

    assert_initial_public_state(&fixture.client);
    exercise_idle_public_mutators(&fixture.client, retained_inner, endpoints[1]);
    assert_idle_alias_refuses(&fixture.client, retained_inner, endpoints[0], false);
    for listener in &fixture.listeners {
        match listener.accept() {
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
            Ok((stream, _)) => {
                drop(stream);
                panic!("idle refusal queued a connection to a retained listener");
            }
        }
    }

    // Required ownership order: cancel listeners -> SDK close -> ALL worker joins.
    // Workers only call the public idle close; no accept/TCP/cache/heartbeat worker.
    fixture.listeners.clear();
    let close_results = std::thread::scope(|scope| {
        // Two independently scheduled callers; overlap is NOT guaranteed.
        // No rendezvous that could strand a worker if thread creation panics.
        let first = scope.spawn(|| fixture.client.close_loopback_checked());
        let second = scope.spawn(|| fixture.client.close_loopback_checked());
        let first = first.join();
        let second = second.join();
        (first, second)
    });
    // Both handles have been joined before checking either result/panic.
    close_results
        .0
        .map_err(|_| std::io::Error::other("first idle-close worker panicked"))??;
    close_results
        .1
        .map_err(|_| std::io::Error::other("second idle-close worker panicked"))??;
    fixture.client.close_loopback_checked()?;

    // The alias was obtained while Open, not reconstructed after close.
    exercise_idle_public_mutators(&fixture.client, retained_inner, endpoints[1]);
    assert_idle_alias_refuses(&fixture.client, retained_inner, endpoints[0], true);
    let later_alias = fixture.client.inner();
    assert_idle_runtime_refusal(later_alias.get_security_count(1), true);
    fixture.client.close_loopback_checked()?;
    fixture.checked_closed = true;
    assert_initial_public_state(&fixture.client);
    Ok(())
}

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

// Independent external protocol literals from the repository's raw TCP fixture.
// No SDK encoder, parser, private scope/pool or mocked provider is called.
const SMART_HANDSHAKE_1: &[u8] = &[0x0c, 0x02, 0x18, 0x93, 0, 1, 3, 0, 3, 0, 0x0d, 0, 1];
const SMART_HANDSHAKE_2: &[u8] = &[0x0c, 0x02, 0x18, 0x94, 0, 1, 3, 0, 3, 0, 0x0d, 0, 2];
const SMART_HANDSHAKE_3: &[u8] = &[
    0x0c, 0x03, 0x18, 0x99, 0, 1, 0x20, 0, 0x20, 0, 0xdb, 0x0f, 0xd5, 0xd0, 0xc9, 0xcc, 0xd6, 0xa4,
    0xa8, 0xaf, 0, 0, 0, 0x8f, 0xc2, 0x25, 0x40, 0x13, 0, 0, 0xd5, 0, 0xc9, 0xcc, 0xbd, 0xf0, 0xd7,
    0xea, 0, 0, 0, 2,
];
const SMART_HANDSHAKE_RESPONSE: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 7];

struct HandshakeGate {
    listener: Option<TcpListener>,
    accept_in_flight: bool,
    shutdown: Option<TcpStream>,
}

struct HandshakeBoundary {
    gate: Mutex<HandshakeGate>,
    stop: Arc<AtomicBool>,
    deadline: Instant,
}

#[derive(Debug)]
struct HandshakeTrace {
    peer: SocketAddr,
    requests: Vec<Vec<u8>>,
    trailing: Vec<u8>,
}

struct HandshakeReport {
    connections: Vec<HandshakeTrace>,
    result: io::Result<()>,
}

fn handshake_remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|value| !value.is_zero())
        .ok_or_else(|| io::Error::new(ErrorKind::TimedOut, "handshake fixture deadline"))
}

fn handshake_lock<T>(mutex: &Mutex<T>, deadline: Instant) -> io::Result<MutexGuard<'_, T>> {
    loop {
        handshake_remaining(deadline)?;
        match mutex.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(io::Error::other("handshake fixture lock poisoned"))
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                thread::sleep(handshake_remaining(deadline)?.min(Duration::from_millis(2)));
            }
        }
    }
}

fn handshake_io_timeout(boundary: &HandshakeBoundary) -> io::Result<Duration> {
    if boundary.stop.load(Ordering::SeqCst) {
        return Err(io::Error::other(
            "handshake fixture cancelled before complete wire",
        ));
    }
    Ok(handshake_remaining(boundary.deadline)?.min(Duration::from_secs(2)))
}

// Lease the listener across nonblocking accept AND accepted-stream registration.
// The cancellation acknowledgement cannot overlook a late accepted stream.
fn handshake_poll_accept(
    boundary: &HandshakeBoundary,
    connections: &mut Vec<HandshakeTrace>,
    allow_primary: bool,
) -> io::Result<Option<TcpStream>> {
    if boundary.stop.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let listener = {
        let mut gate = handshake_lock(&boundary.gate, boundary.deadline)?;
        if boundary.stop.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let listener = gate
            .listener
            .take()
            .ok_or_else(|| io::Error::other("fixture listener missing"))?;
        gate.accept_in_flight = true;
        listener
    };
    let accepted = listener.accept();
    // Record the actual peer before every settings/identity/ownership guard.
    if let Ok((_, peer)) = &accepted {
        connections.push(HandshakeTrace {
            peer: *peer,
            requests: Vec::new(),
            trailing: Vec::new(),
        });
    }
    let cloned = match &accepted {
        Ok((stream, _)) => Some(stream.try_clone()),
        Err(_) => None,
    };
    let mut listener = Some(listener);
    let mut registered = false;
    let mut clone = None;
    let mut clone_error = None;
    if let Some(result) = cloned {
        match result {
            Ok(stream) => clone = Some(stream),
            Err(error) => clone_error = Some(error),
        }
    }
    let cancelled = {
        let mut gate = handshake_lock(&boundary.gate, boundary.deadline)?;
        let cancelled = boundary.stop.load(Ordering::SeqCst);
        if !cancelled {
            gate.listener = listener.take();
        }
        if !cancelled
            && allow_primary
            && connections.len() == 1
            && gate.shutdown.is_none()
            && clone_error.is_none()
        {
            gate.shutdown = clone.take();
            registered = accepted.is_ok();
        }
        // Keep rejected/late accepted resources leased through checked shutdown.
        if accepted.is_err() || registered {
            gate.accept_in_flight = false;
        }
        cancelled
    };
    drop(listener);
    match accepted {
        Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error),
        Ok((stream, peer)) if registered => {
            if peer.ip() != std::net::IpAddr::V4(Ipv4Addr::LOCALHOST) {
                drop(stream);
                return Err(io::Error::other("unexpected fixture peer"));
            }
            // Windows accepted sockets can inherit the listener's nonblocking mode.
            stream.set_nonblocking(false)?;
            Ok(Some(stream))
        }
        Ok((stream, _)) => {
            // Unknown/extra/late accept is closed before reporting rejection.
            let cleanup = match clone {
                Some(shutdown) => {
                    let cleanup = shutdown.shutdown(Shutdown::Both);
                    drop(shutdown);
                    cleanup
                }
                None => stream.shutdown(Shutdown::Both),
            };
            drop(stream);
            let settled = handshake_lock(&boundary.gate, boundary.deadline)
                .map(|mut gate| gate.accept_in_flight = false);
            if let Err(error) = cleanup {
                return Err(io::Error::other(format!(
                    "accepted stream shutdown: {error}; settlement: {settled:?}"
                )));
            }
            settled?;
            if let Some(error) = clone_error {
                return Err(error);
            }
            if cancelled {
                return Ok(None);
            }
            Err(io::Error::other("unexpected handshake fixture connection"))
        }
    }
}

fn handshake_read_to(
    stream: &mut TcpStream,
    observed: &mut Vec<u8>,
    length: usize,
    boundary: &HandshakeBoundary,
) -> io::Result<()> {
    while observed.len() < length {
        stream.set_read_timeout(Some(handshake_io_timeout(boundary)?))?;
        let mut buffer = [0u8; 42];
        let cap = (length - observed.len()).min(buffer.len());
        match stream.read(&mut buffer[..cap]) {
            Ok(0) => {
                return Err(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "short handshake request",
                ))
            }
            Ok(count) => observed.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn handshake_record_request(
    stream: &mut TcpStream,
    expected: &[u8],
    trace: &mut HandshakeTrace,
    boundary: &HandshakeBoundary,
) -> io::Result<()> {
    trace.requests.push(Vec::with_capacity(42));
    let observed = trace
        .requests
        .last_mut()
        .ok_or_else(|| io::Error::other("request trace missing"))?;
    handshake_read_to(stream, observed, 10, boundary)?;
    let payload = usize::from(u16::from_le_bytes([observed[6], observed[7]]));
    if payload > 32 {
        return Err(io::Error::other("handshake request exceeds 42-byte cap"));
    }
    handshake_read_to(stream, observed, 10 + payload, boundary)?;
    if observed.as_slice() != expected {
        return Err(io::Error::other("unexpected complete handshake request"));
    }
    Ok(())
}

fn handshake_write_response(
    stream: &mut TcpStream,
    boundary: &HandshakeBoundary,
) -> io::Result<()> {
    let mut offset = 0;
    while offset < SMART_HANDSHAKE_RESPONSE.len() {
        stream.set_write_timeout(Some(handshake_io_timeout(boundary)?))?;
        match stream.write(&SMART_HANDSHAKE_RESPONSE[offset..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    ErrorKind::WriteZero,
                    "short handshake response",
                ))
            }
            Ok(count) => offset += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn handshake_serve(
    boundary: &HandshakeBoundary,
    primary: bool,
    connections: &mut Vec<HandshakeTrace>,
) -> io::Result<()> {
    let accept_deadline = Instant::now()
        .checked_add(Duration::from_secs(3))
        .ok_or_else(|| io::Error::other("accept deadline overflow"))?
        .min(boundary.deadline);
    let mut stream = loop {
        if boundary.stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        if primary {
            handshake_remaining(accept_deadline)?;
        } else {
            handshake_remaining(boundary.deadline)?;
        }
        if let Some(stream) = handshake_poll_accept(boundary, connections, primary)? {
            break stream;
        }
        thread::sleep(handshake_remaining(boundary.deadline)?.min(Duration::from_millis(2)));
    };
    for request in [SMART_HANDSHAKE_1, SMART_HANDSHAKE_2, SMART_HANDSHAKE_3] {
        handshake_record_request(&mut stream, request, &mut connections[0], boundary)?;
        handshake_write_response(&mut stream, boundary)?;
    }
    stream.set_nonblocking(true)?;
    // Keep the successfully handshaken socket alive until owner cancellation.
    // Record surplus requests/extra accepts; cancellation EOF is NOT SDK EOF proof.
    loop {
        if boundary.stop.load(Ordering::SeqCst) {
            drop(stream);
            return Ok(());
        }
        handshake_remaining(boundary.deadline)?;
        if handshake_poll_accept(boundary, connections, false)?.is_some() {
            return Err(io::Error::other(
                "extra connection was unexpectedly admitted",
            ));
        }
        let mut buffer = [0u8; 64];
        match stream.read(&mut buffer) {
            Ok(0) => {
                if boundary.stop.load(Ordering::SeqCst) {
                    drop(stream);
                    return Ok(());
                }
                return Err(io::Error::other("peer closed before fixture cancellation"));
            }
            Ok(count) => {
                connections[0].trailing.extend_from_slice(&buffer[..count]);
                return Err(io::Error::other(
                    "unexpected bytes after three handshake requests",
                ));
            }
            Err(error)
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
            Err(error) => {
                if boundary.stop.load(Ordering::SeqCst) {
                    drop(stream);
                    return Ok(());
                }
                return Err(error);
            }
        }
        thread::sleep(handshake_remaining(boundary.deadline)?.min(Duration::from_millis(2)));
    }
}

struct HandshakeFixture {
    client: Arc<TdxSmartClient>,
    endpoints: [SocketAddrV4; 2],
    boundaries: [Arc<HandshakeBoundary>; 2],
    workers: Vec<JoinHandle<HandshakeReport>>,
    finished: bool,
}

struct HandshakeClosure {
    errors: Vec<String>,
    reports: Vec<HandshakeReport>,
}

impl HandshakeFixture {
    fn new() -> io::Result<Self> {
        let listeners = [
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?,
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?,
        ];
        for listener in &listeners {
            listener.set_nonblocking(true)?;
        }
        let mut endpoints = [SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0); 2];
        for (index, listener) in listeners.iter().enumerate() {
            endpoints[index] = match listener.local_addr()? {
                SocketAddr::V4(endpoint) => endpoint,
                SocketAddr::V6(_) => {
                    return Err(io::Error::other("expected owned IPv4 fixture endpoint"))
                }
            };
        }
        let stop = Arc::new(AtomicBool::new(false));
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(15))
            .ok_or_else(|| io::Error::other("fixture deadline overflow"))?;
        let client = Arc::new(
            TdxSmartClient::with_loopback_endpoints(&endpoints)
                .map_err(|error| io::Error::other(error.to_string()))?,
        );
        let boundaries = listeners.map(|listener| {
            Arc::new(HandshakeBoundary {
                gate: Mutex::new(HandshakeGate {
                    listener: Some(listener),
                    accept_in_flight: false,
                    shutdown: None,
                }),
                stop: Arc::clone(&stop),
                deadline,
            })
        });
        // Install the owning fixture before any thread creation; reserve both slots.
        let mut fixture = Self {
            client,
            endpoints,
            boundaries,
            workers: Vec::new(),
            finished: false,
        };
        fixture
            .workers
            .try_reserve_exact(2)
            .map_err(|error| io::Error::other(error.to_string()))?;
        for index in 0..2 {
            let boundary = Arc::clone(&fixture.boundaries[index]);
            let worker = thread::Builder::new()
                .name(format!("owned-smart-handshake-{index}"))
                .spawn(move || {
                    let mut connections = Vec::new();
                    let result = handshake_serve(&boundary, index == 0, &mut connections);
                    HandshakeReport {
                        connections,
                        result,
                    }
                })?;
            fixture.workers.push(worker);
        }
        Ok(fixture)
    }

    fn cancel_no_join(&self) -> io::Result<()> {
        for boundary in &self.boundaries {
            boundary.stop.store(true, Ordering::SeqCst);
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(3))
            .ok_or_else(|| io::Error::other("cancel deadline overflow"))?
            .min(self.boundaries[0].deadline);
        let mut errors = Vec::new();
        for boundary in &self.boundaries {
            let resources = handshake_lock(&boundary.gate, deadline)
                .map(|mut gate| (gate.listener.take(), gate.shutdown.take()));
            match resources {
                Ok((listener, shutdown)) => {
                    drop(listener);
                    if let Some(shutdown) = shutdown {
                        if let Err(error) = shutdown.shutdown(Shutdown::Both) {
                            errors.push(error.to_string());
                        }
                        drop(shutdown);
                    }
                }
                Err(error) => errors.push(error.to_string()),
            }
        }
        // Acknowledge every outstanding accept/registration lease without joining.
        for boundary in &self.boundaries {
            let acknowledgement = (|| loop {
                let gate = handshake_lock(&boundary.gate, deadline)?;
                if !gate.accept_in_flight && gate.listener.is_none() && gate.shutdown.is_none() {
                    return Ok::<(), io::Error>(());
                }
                drop(gate);
                thread::sleep(handshake_remaining(deadline)?.min(Duration::from_millis(2)));
            })();
            if let Err(error) = acknowledgement {
                errors.push(error.to_string());
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(errors.join("; ")))
        }
    }

    fn finish(&mut self) -> HandshakeClosure {
        let mut errors = Vec::new();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.cancel_no_join())) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(format!("fixture cancellation: {error}")),
            Err(_) => errors.push("fixture cancellation panicked".to_owned()),
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.client.close_loopback_checked()
        })) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(format!("checked SDK close: {error}")),
            Err(_) => errors.push("checked SDK close panicked".to_owned()),
        }
        let mut reports = Vec::with_capacity(2);
        // Consume ALL handles even when another worker panics or reports failure.
        for (index, worker) in self.workers.drain(..).enumerate() {
            match worker.join() {
                Ok(report) => {
                    if let Err(error) = &report.result {
                        errors.push(format!("fixture worker {index}: {error}"));
                    }
                    reports.push(report);
                }
                Err(_) => errors.push(format!("fixture worker {index} panicked")),
            }
        }
        self.finished = true;
        HandshakeClosure { errors, reports }
    }
}

impl Drop for HandshakeFixture {
    fn drop(&mut self) {
        if !self.finished {
            let closure = self.finish();
            for error in closure.errors {
                eprintln!("handshake fixture fallback: {error}");
            }
        }
    }
}

#[derive(Debug, PartialEq)]
struct HandshakePublicState {
    connected: bool,
    server: Option<(String, u16)>,
    pool: (usize, usize, usize, usize),
    cache: String,
}

fn handshake_public_state(
    client: &TdxSmartClient,
    retained: &magic_tdx_rs::TdxHqClient,
) -> HandshakePublicState {
    let pool = retained.pool_stats();
    HandshakePublicState {
        connected: retained.is_connected(),
        server: retained.connected_server(),
        pool: (pool.idle, pool.active, pool.total, pool.max_size),
        cache: client.cache_stats(),
    }
}

#[test]
fn real_loopback_handshake_is_owned_until_checked_close() -> Result<(), Box<dyn std::error::Error>>
{
    let mut fixture = HandshakeFixture::new()?;
    let retained_client = Arc::clone(&fixture.client);
    let retained_inner = retained_client.inner();
    // Record public observations; defer every assertion until common cleanup.
    let observations = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let before = handshake_public_state(&retained_client, retained_inner);
        let connected = retained_client.connect_to_any(Some(5.0));
        let after = handshake_public_state(&retained_client, retained_inner);
        (before, connected, after)
    }));
    let closure = fixture.finish(); // cancel -> checked SDK close -> ALL joins
    let (before, connected, after) = match observations {
        Ok(observations) => observations,
        Err(panic) => {
            for error in closure.errors {
                eprintln!("handshake cleanup after primary panic: {error}");
            }
            std::panic::resume_unwind(panic);
        }
    };
    if !closure.errors.is_empty() {
        return Err(io::Error::other(format!(
            "primary connect: {connected:?}; cleanup: {}",
            closure.errors.join("; ")
        ))
        .into());
    }
    let idle = HandshakePublicState {
        connected: false,
        server: None,
        pool: (0, 0, 0, 5),
        cache: "last_success: None, blacklist: 0, stats: 0".to_owned(),
    };
    assert_eq!(before, idle);
    assert!(
        matches!(&connected, Ok(true)),
        "expected public Ok(true), got {connected:?}"
    );
    assert_eq!(
        after,
        HandshakePublicState {
            connected: true,
            server: Some(("127.0.0.1".to_owned(), fixture.endpoints[0].port())),
            pool: (1, 0, 1, 5),
            cache: "last_success: None, blacklist: 0, stats: 0".to_owned(),
        }
    );
    assert_eq!(
        handshake_public_state(&retained_client, retained_inner),
        idle
    );
    assert_eq!(closure.reports.len(), 2);
    assert_eq!(closure.reports[0].connections.len(), 1);
    let trace = &closure.reports[0].connections[0];
    assert_eq!(trace.peer.ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(
        trace.requests,
        vec![
            SMART_HANDSHAKE_1.to_vec(),
            SMART_HANDSHAKE_2.to_vec(),
            SMART_HANDSHAKE_3.to_vec()
        ]
    );
    assert!(trace.trailing.is_empty());
    assert!(closure.reports[1].connections.is_empty());
    retained_client.close_loopback_checked()?;
    assert_idle_runtime_refusal(retained_client.connect_to_any(Some(5.0)), true);
    assert_idle_runtime_refusal(
        retained_inner.connect("127.0.0.1", fixture.endpoints[0].port(), Some(0.02)),
        true,
    );
    assert_idle_runtime_refusal(retained_inner.get_security_quotes(&[(1, "600000")]), true);
    assert_idle_runtime_refusal(retained_client.inner().get_security_count(1), true);
    retained_client.close_loopback_checked()?;
    Ok(())
}

// Withhold the response at the external TCP boundary, not inside the SDK.
// The complete first request is traced by the existing bounded raw fixture.
fn smart_silent_handshake_serve(
    boundary: &HandshakeBoundary,
    connections: &mut Vec<HandshakeTrace>,
) -> io::Result<()> {
    let accept_deadline = Instant::now()
        .checked_add(Duration::from_secs(3))
        .ok_or_else(|| io::Error::other("silent accept deadline overflow"))?
        .min(boundary.deadline);
    let mut stream = loop {
        if boundary.stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        handshake_remaining(accept_deadline)?;
        if let Some(stream) = handshake_poll_accept(boundary, connections, true)? {
            break stream;
        }
        thread::sleep(handshake_remaining(boundary.deadline)?.min(Duration::from_millis(2)));
    };
    handshake_record_request(
        &mut stream,
        SMART_HANDSHAKE_1,
        &mut connections[0],
        boundary,
    )?;
    stream.set_nonblocking(true)?;
    let mut peer_closed = false;
    // No response bytes or synthetic clock/delay are injected. Keep observing
    // extra accepts after EOF until the owner cancels the external boundary.
    loop {
        if boundary.stop.load(Ordering::SeqCst) {
            drop(stream);
            return Ok(());
        }
        handshake_remaining(boundary.deadline)?;
        if handshake_poll_accept(boundary, connections, false)?.is_some() {
            return Err(io::Error::other(
                "silent fixture admitted an extra connection",
            ));
        }
        if !peer_closed {
            let mut buffer = [0u8; 64];
            match stream.read(&mut buffer) {
                Ok(0) => peer_closed = true,
                Ok(count) => {
                    connections[0].trailing.extend_from_slice(&buffer[..count]);
                    return Err(io::Error::other("request followed an unanswered handshake"));
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
                Err(error) => {
                    if boundary.stop.load(Ordering::SeqCst) {
                        drop(stream);
                        return Ok(());
                    }
                    return Err(error);
                }
            }
        }
        thread::sleep(handshake_remaining(boundary.deadline)?.min(Duration::from_millis(2)));
    }
}

fn smart_silent_handshake_fixture() -> io::Result<HandshakeFixture> {
    let listeners = [
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?,
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?,
    ];
    for listener in &listeners {
        listener.set_nonblocking(true)?;
    }
    let mut endpoints = [SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0); 2];
    for (index, listener) in listeners.iter().enumerate() {
        endpoints[index] = match listener.local_addr()? {
            SocketAddr::V4(endpoint) => endpoint,
            SocketAddr::V6(_) => {
                return Err(io::Error::other("expected owned IPv4 fixture endpoint"))
            }
        };
    }
    let stop = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(15))
        .ok_or_else(|| io::Error::other("silent fixture deadline overflow"))?;
    let client = Arc::new(
        TdxSmartClient::with_loopback_endpoints(&endpoints)
            .map_err(|error| io::Error::other(error.to_string()))?,
    );
    let boundaries = listeners.map(|listener| {
        Arc::new(HandshakeBoundary {
            gate: Mutex::new(HandshakeGate {
                listener: Some(listener),
                accept_in_flight: false,
                shutdown: None,
            }),
            stop: Arc::clone(&stop),
            deadline,
        })
    });
    // Own the client and cancellation gates before fallible reserve or spawn.
    let mut fixture = HandshakeFixture {
        client,
        endpoints,
        boundaries,
        workers: Vec::new(),
        finished: false,
    };
    fixture
        .workers
        .try_reserve_exact(2)
        .map_err(|error| io::Error::other(error.to_string()))?;
    for index in 0..2 {
        let boundary = Arc::clone(&fixture.boundaries[index]);
        let worker = thread::Builder::new()
            .name(format!("owned-smart-timeout-source-{index}"))
            .spawn(move || {
                let mut connections = Vec::new();
                let result = if index == 0 {
                    smart_silent_handshake_serve(&boundary, &mut connections)
                } else {
                    handshake_serve(&boundary, false, &mut connections)
                };
                HandshakeReport {
                    connections,
                    result,
                }
            })?;
        fixture.workers.push(worker);
    }
    Ok(fixture)
}

struct SmartTimeoutApplication {
    connected: Result<bool, TdxError>,
    elapsed: Duration,
    completion_sent: bool,
}

struct SmartTimeoutOwner {
    wire: HandshakeFixture,
    application: Option<JoinHandle<SmartTimeoutApplication>>,
    finished: bool,
}

impl SmartTimeoutOwner {
    fn start_application(&mut self) -> io::Result<std::sync::mpsc::Receiver<()>> {
        if self.application.is_some() {
            return Err(io::Error::other("timeout application already owned"));
        }
        let client = Arc::clone(&self.wire.client);
        let (completed, receiver) = std::sync::mpsc::channel();
        let worker = thread::Builder::new()
            .name("owned-smart-timeout-application".to_owned())
            .spawn(move || {
                let started = Instant::now();
                let connected = client.connect_to_any(Some(0.25));
                let elapsed = started.elapsed();
                let completion_sent = completed.send(()).is_ok();
                SmartTimeoutApplication {
                    connected,
                    elapsed,
                    completion_sent,
                }
            })?;
        self.application = Some(worker);
        Ok(receiver)
    }

    fn finish(&mut self) -> (HandshakeClosure, Option<SmartTimeoutApplication>) {
        let mut closure = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.wire.finish() // cancel -> checked SDK close -> BOTH source joins
        })) {
            Ok(closure) => closure,
            Err(_) => HandshakeClosure {
                errors: vec!["timeout wire cleanup panicked".to_owned()],
                reports: Vec::new(),
            },
        };
        // Even a wire-cleanup failure/panic must consume the application handle.
        // No join occurs before fixture cancellation and checked SDK close.
        let application = self
            .application
            .take()
            .and_then(|worker| match worker.join() {
                Ok(observation) => Some(observation),
                Err(_) => {
                    closure
                        .errors
                        .push("timeout application panicked".to_owned());
                    None
                }
            });
        self.finished = true;
        (closure, application)
    }
}

impl Drop for SmartTimeoutOwner {
    fn drop(&mut self) {
        if !self.finished {
            let (closure, _) = self.finish();
            for error in closure.errors {
                eprintln!("timeout fixture fallback: {error}");
            }
        }
    }
}

#[test]
fn unanswered_handshake_exhausts_one_deadline_and_checked_close_refuses_reconnect(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut owner = SmartTimeoutOwner {
        wire: smart_silent_handshake_fixture()?,
        application: None,
        finished: false,
    };
    let client = Arc::clone(&owner.wire.client);
    let retained_inner = client.inner();
    let observations = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let before = handshake_public_state(&client, retained_inner);
        let receiver = owner.start_application()?;
        // A bounded completion event, not a sleep used as evidence of progress.
        let completed = receiver.recv_timeout(Duration::from_secs(2));
        let after = if completed.is_ok() {
            Some(handshake_public_state(&client, retained_inner))
        } else {
            None
        };
        Ok::<_, io::Error>((before, completed, after))
    }));
    let (closure, application) = owner.finish(); // cancel -> SDK close -> ALL THREE joins
    let (before, completed, after) = match observations {
        Ok(Ok(observations)) => observations,
        Ok(Err(error)) => {
            return Err(io::Error::other(format!(
                "timeout setup: {error}; cleanup: {}",
                closure.errors.join("; ")
            ))
            .into());
        }
        Err(panic) => {
            for error in closure.errors {
                eprintln!("timeout cleanup after primary panic: {error}");
            }
            std::panic::resume_unwind(panic);
        }
    };
    if !closure.errors.is_empty() {
        return Err(io::Error::other(format!(
            "completion: {completed:?}; cleanup: {}",
            closure.errors.join("; ")
        ))
        .into());
    }
    // Every assertion below follows common cleanup; a late return cannot pass.
    assert!(
        completed.is_ok(),
        "public connect did not complete within two seconds: {completed:?}"
    );
    let application = application.ok_or_else(|| io::Error::other("timeout application missing"))?;
    assert!(application.completion_sent);
    assert!(
        matches!(&application.connected, Err(TdxError::ConnectionTimeout)),
        "expected exact ConnectionTimeout, got {:?}",
        application.connected
    );
    assert_eq!(
        application
            .connected
            .as_ref()
            .err()
            .map(ToString::to_string),
        Some("Connection timeout".to_owned())
    );
    assert!(
        application.elapsed >= Duration::from_millis(250),
        "deadline returned early: {:?}",
        application.elapsed
    );
    assert!(
        application.elapsed <= Duration::from_secs(2),
        "operation deadline was not bounded: {:?}",
        application.elapsed
    );
    let idle = HandshakePublicState {
        connected: false,
        server: None,
        pool: (0, 0, 0, 5),
        cache: "last_success: None, blacklist: 0, stats: 0".to_owned(),
    };
    assert_eq!(before, idle);
    assert_eq!(after, Some(idle));
    assert_eq!(
        handshake_public_state(&client, retained_inner),
        HandshakePublicState {
            connected: false,
            server: None,
            pool: (0, 0, 0, 5),
            cache: "last_success: None, blacklist: 0, stats: 0".to_owned(),
        }
    );
    assert_eq!(closure.reports.len(), 2);
    assert_eq!(closure.reports[0].connections.len(), 1);
    let trace = &closure.reports[0].connections[0];
    assert_eq!(trace.peer.ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(trace.requests, vec![SMART_HANDSHAKE_1.to_vec()]);
    assert!(trace.trailing.is_empty());
    assert!(
        closure.reports[1].connections.is_empty(),
        "whole deadline was renewed for endpoint B"
    );
    client.close_loopback_checked()?;
    assert_idle_runtime_refusal(client.connect_to_any(Some(0.25)), true);
    assert_idle_runtime_refusal(
        retained_inner.connect("127.0.0.1", owner.wire.endpoints[0].port(), Some(0.25)),
        true,
    );
    assert_idle_runtime_refusal(client.inner().connect_to_any(Some(0.25)), true);
    client.close_loopback_checked()?;
    Ok(())
}
