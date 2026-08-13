#![cfg(feature = "fake-bridge-fixture")]
#![forbid(unsafe_code)]

use magic_tdx_local_rs::{
    BridgeCommand, BridgeErrorCode, BridgeMessage, BridgeRuntimeState, BridgeSequenceTracker,
    FrameCodec, ProtocolError, Shutdown, SupervisorAction, SupervisorEvent, SupervisorMachine,
    SupervisorState,
};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const FIXTURE_FRAME_LIMIT: usize = 16 * 1024;
const CHILD_WAIT_LIMIT: Duration = Duration::from_secs(5);

struct FixtureChild {
    child: Child,
}

impl FixtureChild {
    fn spawn(scenario: &str) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_magic-tdx-fake-bridge"))
            .args(["--scenario", scenario])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Cargo-built immutable fake bridge must start");
        Self { child }
    }

    fn wait_bounded(&mut self) -> std::process::ExitStatus {
        let started = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().expect("poll fake bridge") {
                return status;
            }
            assert!(
                started.elapsed() < CHILD_WAIT_LIMIT,
                "fake bridge did not exit within the deterministic test bound"
            );
            thread::yield_now();
        }
    }

    fn stderr(&mut self) -> String {
        let mut bytes = Vec::new();
        self.child
            .stderr
            .take()
            .expect("captured stderr")
            .read_to_end(&mut bytes)
            .expect("read fixture diagnostics");
        String::from_utf8(bytes).expect("fixture stderr is UTF-8")
    }
}

impl Drop for FixtureChild {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn codec() -> FrameCodec {
    FrameCodec::new(FIXTURE_FRAME_LIMIT).unwrap()
}

fn read_message(child: &mut FixtureChild) -> Result<BridgeMessage, ProtocolError> {
    codec().read_message(child.child.stdout.as_mut().expect("captured stdout"))
}

#[test]
fn child_hello_and_graceful_stop_use_only_framed_stdout() {
    let mut child = FixtureChild::spawn("normal");
    let mut sequences = BridgeSequenceTracker::new();
    let hello = read_message(&mut child).unwrap();
    assert!(matches!(hello, BridgeMessage::Hello(_)));
    sequences.observe(&hello).unwrap();
    let starting = read_message(&mut child).unwrap();
    assert!(matches!(
        starting,
        BridgeMessage::Status(ref status) if status.state == BridgeRuntimeState::Starting
    ));
    sequences.observe(&starting).unwrap();
    let ready = read_message(&mut child).unwrap();
    assert!(matches!(
        ready,
        BridgeMessage::Status(ref status) if status.state == BridgeRuntimeState::Ready
    ));
    sequences.observe(&ready).unwrap();
    let observation = read_message(&mut child).unwrap();
    assert!(matches!(observation, BridgeMessage::Observation(_)));
    sequences.observe(&observation).unwrap();

    codec()
        .write_command(
            child.child.stdin.as_mut().expect("captured stdin"),
            &BridgeCommand::Shutdown(Shutdown::current()),
        )
        .unwrap();
    let stopping = read_message(&mut child).unwrap();
    assert!(matches!(
        stopping,
        BridgeMessage::Status(ref status) if status.state == BridgeRuntimeState::Stopping
    ));
    sequences.observe(&stopping).unwrap();
    let stopped = read_message(&mut child).unwrap();
    assert_eq!(
        stopped,
        BridgeMessage::Stopped(magic_tdx_local_rs::Stopped::current(5))
    );
    sequences.observe(&stopped).unwrap();
    assert_eq!(sequences.last(), Some(5));
    let status = child.wait_bounded();
    assert!(status.success());

    let mut trailing_stdout = Vec::new();
    child
        .child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut trailing_stdout)
        .unwrap();
    assert!(
        trailing_stdout.is_empty(),
        "stdout must remain protocol-only"
    );
    assert!(child.stderr().contains("fake_bridge_scenario=normal"));
}

#[test]
fn wrong_protocol_and_schema_versions_fail_during_validated_decode() {
    let mut wrong_protocol = FixtureChild::spawn("wrong_protocol");
    assert!(matches!(
        read_message(&mut wrong_protocol),
        Err(ProtocolError::UnsupportedProtocolVersion { .. })
    ));
    assert!(wrong_protocol.wait_bounded().success());

    let mut wrong_schema = FixtureChild::spawn("wrong_schema");
    assert!(matches!(
        read_message(&mut wrong_schema),
        Err(ProtocolError::UnsupportedSchemaVersion { .. })
    ));
    assert!(wrong_schema.wait_bounded().success());
}

#[test]
fn partial_oversized_and_malformed_child_frames_fail_explicitly() {
    let mut partial = FixtureChild::spawn("partial");
    assert!(matches!(
        read_message(&mut partial),
        Err(ProtocolError::Io { .. })
    ));
    assert!(partial.wait_bounded().success());

    let mut oversized = FixtureChild::spawn("oversized");
    assert!(matches!(
        read_message(&mut oversized),
        Err(ProtocolError::FrameTooLarge {
            announced,
            maximum: FIXTURE_FRAME_LIMIT
        }) if announced == FIXTURE_FRAME_LIMIT + 1
    ));
    assert!(oversized.wait_bounded().success());

    let mut malformed = FixtureChild::spawn("malformed");
    assert!(matches!(
        read_message(&mut malformed),
        Err(ProtocolError::DecodeJson(_))
    ));
    assert!(malformed.wait_bounded().success());
}

#[test]
fn crash_is_observable_without_a_synthetic_protocol_message() {
    let mut child = FixtureChild::spawn("crash");
    let status = child.wait_bounded();
    assert_eq!(status.code(), Some(23));
    let mut stdout = Vec::new();
    child
        .child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    assert!(stdout.is_empty());
}

#[test]
fn typed_runtime_error_is_observable_without_text_parsing() {
    let mut child = FixtureChild::spawn("error");
    assert!(matches!(
        read_message(&mut child),
        Ok(BridgeMessage::Hello(_))
    ));
    assert!(matches!(
        read_message(&mut child),
        Ok(BridgeMessage::Error(error))
            if error.code == BridgeErrorCode::SourceReadFailed && error.retryable
    ));
    assert!(child.wait_bounded().success());
}

#[test]
fn bridge_local_sequence_gap_is_detected_without_claiming_source_completeness() {
    let mut child = FixtureChild::spawn("sequence_gap");
    let mut sequences = BridgeSequenceTracker::new();
    let hello = read_message(&mut child).unwrap();
    sequences.observe(&hello).unwrap();
    let ready = read_message(&mut child).unwrap();
    sequences.observe(&ready).unwrap();
    let observation = read_message(&mut child).unwrap();
    assert!(matches!(
        sequences.observe(&observation),
        Err(ProtocolError::BridgeSequenceGap {
            expected: 2,
            actual: 3
        })
    ));
    assert!(child.wait_bounded().success());
}

#[test]
fn hung_child_remains_bounded_by_parent_owned_termination() {
    let mut child = FixtureChild::spawn("hang");
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(25) {
        assert!(child.child.try_wait().unwrap().is_none());
        thread::yield_now();
    }
    child.child.kill().unwrap();
    let _status = child.wait_bounded();
}

#[test]
fn validated_child_hello_drives_only_explicit_supervisor_actions() {
    let mut child = FixtureChild::spawn("normal");
    let mut supervisor = SupervisorMachine::new(1);
    assert_eq!(
        supervisor
            .transition(SupervisorEvent::Enable)
            .unwrap()
            .action,
        SupervisorAction::RunDiscoveryProbe
    );
    supervisor
        .transition(SupervisorEvent::DiscoveryFound)
        .unwrap();
    supervisor
        .transition(SupervisorEvent::ValidationAccepted)
        .unwrap();
    supervisor
        .transition(SupervisorEvent::BridgeStarted)
        .unwrap();
    assert!(matches!(
        read_message(&mut child).unwrap(),
        BridgeMessage::Hello(_)
    ));
    assert_eq!(
        supervisor
            .transition(SupervisorEvent::HelloAccepted)
            .unwrap()
            .action,
        SupervisorAction::PublishRunning { generation: 1 }
    );
    assert_eq!(supervisor.state(), SupervisorState::Running);

    for expected_sequence in 1..=3 {
        assert_eq!(
            read_message(&mut child).unwrap().bridge_sequence(),
            Some(expected_sequence)
        );
    }

    codec()
        .write_command(
            child.child.stdin.as_mut().unwrap(),
            &BridgeCommand::Shutdown(Shutdown::current()),
        )
        .unwrap();
    assert!(matches!(
        read_message(&mut child),
        Ok(BridgeMessage::Status(_))
    ));
    assert!(matches!(
        read_message(&mut child),
        Ok(BridgeMessage::Stopped(_))
    ));
    assert!(child.wait_bounded().success());
}
