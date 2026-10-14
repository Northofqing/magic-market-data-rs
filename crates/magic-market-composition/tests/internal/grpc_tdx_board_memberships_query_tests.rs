use super::super::register_tdx_boards_client;
use magic_market_core::{AssetClass, BoardCategory, BoardMembership, Exchange, ProviderId};
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, QueryResult, ServiceError,
};
use magic_tdx_rs::TdxBoardProvider;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Independent native packets, not the SDK's packet encoder or constants.
const SETUP_1: &[u8] = &[0x0c, 0x02, 0x18, 0x93, 0, 1, 3, 0, 3, 0, 0x0d, 0, 1];
const SETUP_2: &[u8] = &[0x0c, 0x02, 0x18, 0x94, 0, 1, 3, 0, 3, 0, 0x0d, 0, 2];
const SETUP_3: &[u8] = &[
    0x0c, 0x03, 0x18, 0x99, 0, 1, 0x20, 0, 0x20, 0, 0xdb, 0x0f, 0xd5, 0xd0, 0xc9, 0xcc, 0xd6, 0xa4,
    0xa8, 0xaf, 0, 0, 0, 0x8f, 0xc2, 0x25, 0x40, 0x13, 0, 0, 0xd5, 0, 0xc9, 0xcc, 0xbd, 0xf0, 0xd7,
    0xea, 0, 0, 0, 2,
];
const HANDSHAKE_RESPONSE: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 7];
const META_FG: &[u8] = &[
    0x0c, 0x39, 0x18, 0x69, 0x00, 0x01, 0x2a, 0x00, 0x2a, 0x00, 0xc5, 0x02, 0x62, 0x6c, 0x6f, 0x63,
    0x6b, 0x5f, 0x66, 0x67, 0x2e, 0x64, 0x61, 0x74, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];
const DATA_FG: &[u8] = &[
    0x0c, 0x37, 0x18, 0x6a, 0x00, 0x01, 0x6e, 0x00, 0x6e, 0x00, 0xb9, 0x06, 0x00, 0x00, 0x00, 0x00,
    0x7f, 0x0c, 0x00, 0x00, 0x62, 0x6c, 0x6f, 0x63, 0x6b, 0x5f, 0x66, 0x67, 0x2e, 0x64, 0x61, 0x74,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const META_GN: &[u8] = &[
    0x0c, 0x39, 0x18, 0x69, 0x00, 0x01, 0x2a, 0x00, 0x2a, 0x00, 0xc5, 0x02, 0x62, 0x6c, 0x6f, 0x63,
    0x6b, 0x5f, 0x67, 0x6e, 0x2e, 0x64, 0x61, 0x74, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];
const DATA_GN: &[u8] = &[
    0x0c, 0x37, 0x18, 0x6a, 0x00, 0x01, 0x6e, 0x00, 0x6e, 0x00, 0xb9, 0x06, 0x00, 0x00, 0x00, 0x00,
    0x7f, 0x0c, 0x00, 0x00, 0x62, 0x6c, 0x6f, 0x63, 0x6b, 0x5f, 0x67, 0x6e, 0x2e, 0x64, 0x61, 0x74,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const REQUEST_BODY: &[u8] =
    br#"{"instruments":[{"exchange":"Shanghai","code":"600000","asset_class":"Equity"}]}"#;
const EXPECTED_RECORDS: &str = r#"[
  {
    "instrument":{"exchange":"Shanghai","code":"600000","asset_class":"Equity"},
    "board_code":"tdx:industry:Energy","board_name":"Energy","category":"Industry",
    "evidence":{"provider":"Tdx","source_at":null,"observed_at":"O","batch_id":"B"}
  },
  {
    "instrument":{"exchange":"Shanghai","code":"600000","asset_class":"Equity"},
    "board_code":"tdx:concept:AI","board_name":"AI","category":"Concept",
    "evidence":{"provider":"Tdx","source_at":null,"observed_at":"O","batch_id":"B"}
  }
]"#;

// Literal .dat layout: header384/blockcount-u16/name9/stockcount-u16/type-u16/area2800.
// No BlockRecord is constructed and no SDK/parser/provider is substituted.
fn block_bytes(name: &[u8; 9]) -> Vec<u8> {
    let mut bytes = vec![0; 3199];
    bytes[384..386].copy_from_slice(&[1, 0]);
    bytes[386..395].copy_from_slice(name);
    bytes[395..399].copy_from_slice(&[2, 0, 2, 0]);
    bytes[399..406].copy_from_slice(b"600000\0");
    bytes[406..413].copy_from_slice(b"000001\0");
    bytes
}

fn meta_body() -> Vec<u8> {
    let mut bytes = vec![0; 38];
    bytes[..4].copy_from_slice(&[0x7f, 0x0c, 0, 0]);
    bytes[5..37].copy_from_slice(b"0123456789abcdef0123456789abcdef");
    bytes
}

fn data_body(bytes: Vec<u8>) -> Vec<u8> {
    let mut body = vec![0; 4];
    body.extend(bytes);
    body
}

#[derive(Debug)]
struct ConnectionTrace {
    peer: SocketAddr,
    requests: Vec<Vec<u8>>,
    trailing: Vec<u8>,
    peer_closed: bool,
}

struct WorkerReport {
    connections: Vec<ConnectionTrace>,
    result: io::Result<()>,
}

fn remaining(deadline: Instant, stop: &AtomicBool) -> io::Result<Duration> {
    if stop.load(Ordering::SeqCst) {
        return Err(io::Error::other("board fixture cancelled"));
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "board fixture deadline",
        ));
    }
    Ok(remaining.min(Duration::from_secs(2)))
}

fn read_bytes(
    stream: &mut TcpStream,
    bytes: &mut Vec<u8>,
    length: usize,
    deadline: Instant,
    stop: &AtomicBool,
) -> io::Result<()> {
    while bytes.len() < length {
        stream.set_read_timeout(Some(remaining(deadline, stop)?))?;
        let mut buffer = [0u8; 64];
        let cap = (length - bytes.len()).min(buffer.len());
        match stream.read(&mut buffer[..cap]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "short native request",
                ))
            }
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

// Capture observed full OR partial frame before length/equality/error guards.
// The source request header declares a payload after its first10 bytes.
// An excessive declaration records the header before rejecting, without allocation.
fn read_recorded(
    stream: &mut TcpStream,
    expected: &[u8],
    trace: &mut ConnectionTrace,
    deadline: Instant,
    stop: &AtomicBool,
) -> io::Result<()> {
    trace.requests.push(Vec::with_capacity(120));
    let bytes = trace
        .requests
        .last_mut()
        .ok_or_else(|| io::Error::other("missing request trace"))?;
    read_bytes(stream, bytes, 10, deadline, stop)?;
    let payload_length = usize::from(u16::from_le_bytes([bytes[6], bytes[7]]));
    if payload_length > 110 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native request exceeds fixture cap",
        ));
    }
    read_bytes(stream, bytes, 10 + payload_length, deadline, stop)?;
    if bytes.as_slice() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected native request bytes",
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
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "short board response",
                ))
            }
            Ok(count) => offset += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn write_response(
    stream: &mut TcpStream,
    body: &[u8],
    deadline: Instant,
    stop: &AtomicBool,
) -> io::Result<()> {
    let length =
        u16::try_from(body.len()).map_err(|_| io::Error::other("fixture response exceeds u16"))?;
    let mut header = [0u8; 16];
    header[12..14].copy_from_slice(&length.to_le_bytes());
    header[14..16].copy_from_slice(&length.to_le_bytes());
    write_bounded(stream, &header, deadline, stop)?;
    write_bounded(stream, body, deadline, stop)
}

// Real Direct drops each connection after the response. Capture any trailing
// command/junk (bounded) before rejecting instead of silently discarding it.
fn require_peer_close(
    stream: &mut TcpStream,
    trace: &mut ConnectionTrace,
    deadline: Instant,
    stop: &AtomicBool,
) -> io::Result<()> {
    loop {
        stream.set_read_timeout(Some(remaining(deadline, stop)?))?;
        let mut buffer = [0u8; 64];
        let cap = (121 - trace.trailing.len()).min(buffer.len());
        match stream.read(&mut buffer[..cap]) {
            Ok(0) => break,
            Ok(count) => {
                trace.trailing.extend_from_slice(&buffer[..count]);
                if trace.trailing.len() > 120 {
                    return Err(io::Error::other("trailing native bytes exceed fixture cap"));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    if !trace.trailing.is_empty() {
        return Err(io::Error::other("unexpected trailing native request"));
    }
    trace.peer_closed = true;
    Ok(())
}

fn serve(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    execute_finished: Arc<AtomicBool>,
    concept: Vec<u8>,
) -> WorkerReport {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut connections = Vec::new();
    let responses = [
        meta_body(),
        data_body(block_bytes(b"Energy\0\0\0")),
        meta_body(),
        data_body(concept),
    ];
    let expected = [META_FG, DATA_FG, META_GN, DATA_GN];
    let result = (|| {
        loop {
            let index = connections.len();
            let accept_deadline = if index < 4 {
                (Instant::now() + Duration::from_secs(3)).min(deadline)
            } else {
                deadline
            };
            let (mut stream, peer) = loop {
                remaining(accept_deadline, &stop)?;
                // Drain already-queued accepts before noticing Execute completion.
                match listener.accept() {
                    Ok(accepted) => break accepted,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if execute_finished.load(Ordering::SeqCst) {
                            if index == 4 {
                                return Ok(());
                            }
                            return Err(io::Error::other(
                                "Execute ended before all board requests",
                            ));
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            };
            // Accepted connection/peer is captured before any guard.
            connections.push(ConnectionTrace {
                peer,
                requests: Vec::new(),
                trailing: Vec::new(),
                peer_closed: false,
            });
            let trace = connections
                .last_mut()
                .ok_or_else(|| io::Error::other("missing trace"))?;
            if !peer.ip().is_loopback() {
                return Err(io::Error::other("non-loopback board fixture peer"));
            }
            stream.set_nonblocking(false)?; // Windows accept inherits listener state.
            if index >= 4 {
                read_recorded(&mut stream, SETUP_1, trace, deadline, &stop)?;
                return Err(io::Error::other("unexpected extra board connection"));
            }
            for setup in [SETUP_1, SETUP_2, SETUP_3] {
                read_recorded(&mut stream, setup, trace, deadline, &stop)?;
                write_bounded(&mut stream, HANDSHAKE_RESPONSE, deadline, &stop)?;
            }
            read_recorded(&mut stream, expected[index], trace, deadline, &stop)?;
            write_response(&mut stream, &responses[index], deadline, &stop)?;
            require_peer_close(&mut stream, trace, deadline, &stop)?;
            // Stream naturally drops here; every error path also drops it.
        }
    })();
    WorkerReport {
        connections,
        result,
    }
}

struct Fixture {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    execute_finished: Arc<AtomicBool>,
    worker: Option<JoinHandle<WorkerReport>>,
}

impl Fixture {
    fn start(concept: Vec<u8>) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let execute_finished = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_finished = Arc::clone(&execute_finished);
        let worker = thread::Builder::new()
            .name("tdx-board-memberships-wire-fixture".into())
            .spawn(move || serve(listener, worker_stop, worker_finished, concept))?;
        Ok(Self {
            address,
            stop,
            execute_finished,
            worker: Some(worker),
        })
    }

    fn finish(mut self) -> io::Result<WorkerReport> {
        self.execute_finished.store(true, Ordering::SeqCst);
        self.worker
            .take()
            .ok_or_else(|| io::Error::other("missing board worker"))?
            .join()
            .map_err(|_| io::Error::other("board worker panicked"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join(); // Bounded accept/IO observes cancellation; never detached.
        }
    }
}

fn run_case(
    concept: Vec<u8>,
    request_id: &str,
) -> (
    Result<QueryResult, ServiceError>,
    WorkerReport,
    u128,
    u128,
    QueryCommand,
) {
    let fixture = Fixture::start(concept).unwrap();
    let provider = TdxBoardProvider::new("127.0.0.1", fixture.address.port(), 2.0);
    let mut registry = OperationRegistry::all_unadmitted("isolated raw TDX board fixture");
    // Setup only: the original concrete closures, parser and provider remain real.
    register_tdx_boards_client(&mut registry, provider, 4096).unwrap();
    let command = QueryCommand::new(
        request_id,
        Operation::BoardMemberships,
        Some("Tdx".into()),
        CanonicalPayload::new(
            "magic.market.board_memberships.request",
            1,
            REQUEST_BODY.to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!command.allows_unadmitted());
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let result = registry.execute(command.clone());
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let report = fixture.finish().unwrap(); // Join/close BEFORE any outcome assertion.
    (result, report, before, after, command)
}

fn assert_trace(report: WorkerReport) {
    assert!(
        report.result.is_ok(),
        "fixture error: {:?}; trace: {:?}",
        report.result,
        report.connections
    );
    assert_eq!(
        report.connections.len(),
        4,
        "full trace: {:?}",
        report.connections
    );
    for (connection, query) in report
        .connections
        .iter()
        .zip([META_FG, DATA_FG, META_GN, DATA_GN])
    {
        assert!(connection.peer.ip().is_loopback());
        assert!(connection.peer_closed);
        assert!(connection.trailing.is_empty());
        assert_eq!(
            connection.requests,
            vec![
                SETUP_1.to_vec(),
                SETUP_2.to_vec(),
                SETUP_3.to_vec(),
                query.to_vec(),
            ]
        );
    }
}

#[test]
fn registered_tdx_memberships_reject_late_concept_duplicate_without_industry_prefix() {
    let valid_concept = block_bytes(b"AI\0\0\0\0\0\0\0");
    let (normal, normal_trace, before, after, command) =
        run_case(valid_concept.clone(), "tdx-board-normal");
    assert_trace(normal_trace);
    assert_eq!(command.payload().data(), REQUEST_BODY);
    assert!(!command.allows_unadmitted());
    let result = normal.unwrap();
    assert_eq!(result.provider, "Tdx");
    assert!(result.complete);
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at, None);
    assert_eq!(result.records.len(), 2);

    let millis = result.observed_at.strip_prefix("unix-ms:").unwrap();
    assert!(!millis.is_empty() && millis.bytes().all(|byte| byte.is_ascii_digit()));
    let observed = millis.parse::<u128>().unwrap();
    assert_eq!(observed.to_string(), millis);
    assert!(before <= observed && observed <= after);
    assert_eq!(
        result.batch_id,
        format!("tdx-block-files:memberships:{}", result.observed_at)
    );
    let mut expected: serde_json::Value = serde_json::from_str(EXPECTED_RECORDS).unwrap();
    for (index, payload) in result.records.iter().enumerate() {
        assert_eq!(payload.schema(), "magic.market.board_membership");
        assert_eq!(payload.schema_version(), 1);
        assert!(payload.data().len() <= 4096);
        let actual: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(actual.as_object().unwrap().len(), 5);
        assert_eq!(actual["instrument"].as_object().unwrap().len(), 3);
        assert_eq!(actual["evidence"].as_object().unwrap().len(), 4);
        expected[index]["evidence"]["observed_at"] = result.observed_at.clone().into();
        expected[index]["evidence"]["batch_id"] = result.batch_id.clone().into();
        assert_eq!(actual, expected[index]);
        let membership: BoardMembership = serde_json::from_slice(payload.data()).unwrap();
        assert_eq!(membership.instrument.exchange(), Exchange::Shanghai);
        assert_eq!(membership.instrument.code(), "600000");
        assert_eq!(membership.instrument.asset_class(), AssetClass::Equity);
        assert_eq!(
            membership.category,
            [BoardCategory::Industry, BoardCategory::Concept][index]
        );
        assert_eq!(
            membership.board_code.as_str(),
            ["tdx:industry:Energy", "tdx:concept:AI"][index]
        );
        assert_eq!(membership.board_name.as_str(), ["Energy", "AI"][index]);
        assert_eq!(membership.evidence.provider(), ProviderId::Tdx);
        assert_eq!(membership.evidence.source_at(), None);
        assert_eq!(membership.evidence.observed_at(), result.observed_at);
        assert_eq!(membership.evidence.batch_id(), result.batch_id);
        assert_eq!(serde_json::to_value(&membership).unwrap(), expected[index]);
    }

    let mut duplicate_concept = valid_concept.clone();
    duplicate_concept[406..413].copy_from_slice(b"600000\0");
    assert_eq!(&duplicate_concept[..406], &valid_concept[..406]);
    assert_eq!(&duplicate_concept[413..], &valid_concept[413..]);
    let (negative, negative_trace, _, _, rejected_command) =
        run_case(duplicate_concept, "tdx-board-late-duplicate");
    assert_trace(negative_trace);
    assert_eq!(rejected_command.payload().data(), REQUEST_BODY);
    assert!(!rejected_command.allows_unadmitted());
    assert_eq!(
        negative.unwrap_err(),
        ServiceError::FailedPrecondition(
            "Invalid data: duplicate TDX board/source pair \"AI\"/600000".into(),
        )
    );
}
