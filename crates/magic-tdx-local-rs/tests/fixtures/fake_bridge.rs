#![forbid(unsafe_code)]

use magic_tdx_local_rs::{
    ArtifactIdentity, BridgeCommand, BridgeErrorCode, BridgeErrorReport, BridgeMessage,
    BridgeRuntimeState, BridgeStatus, BridgeStatusReason, DecimalObservation, FrameCodec, Hello,
    ObservationUnit, SourceExchange, SourceInstrument, SourceObservation, Stopped, TerminalState,
    PROTOCOL_VERSION, SCHEMA_VERSION,
};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::process::ExitCode;

const FIXTURE_FRAME_LIMIT: usize = 16 * 1024;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("fake_bridge_error={message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let scenario = parse_scenario()?;
    eprintln!("fake_bridge_scenario={scenario}");
    match scenario.as_str() {
        "normal" => serve_normal(),
        "wrong_protocol" => write_unvalidated_hello(PROTOCOL_VERSION + 1, SCHEMA_VERSION),
        "wrong_schema" => write_unvalidated_hello(PROTOCOL_VERSION, SCHEMA_VERSION + 1),
        "partial" => write_raw_frame(32, br#"{}"#),
        "oversized" => write_raw_frame(
            u32::try_from(FIXTURE_FRAME_LIMIT + 1).map_err(|error| error.to_string())?,
            &[],
        ),
        "malformed" => write_raw_frame(1, b"{"),
        "hang" => {
            std::thread::park();
            Ok(())
        }
        "crash" => std::process::exit(23),
        "error" => serve_error(),
        "sequence_gap" => serve_sequence_gap(),
        _ => Err("unsupported fixed scenario".into()),
    }
}

fn parse_scenario() -> Result<String, String> {
    let mut arguments = std::env::args();
    let _program = arguments.next();
    if arguments.next().as_deref() != Some("--scenario") {
        return Err("expected --scenario followed by one fixed scenario".into());
    }
    let scenario = arguments
        .next()
        .ok_or_else(|| "missing fixed scenario".to_owned())?;
    if arguments.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    Ok(scenario)
}

fn artifact(filename: &str, hash_byte: char) -> ArtifactIdentity {
    ArtifactIdentity {
        filename: filename.into(),
        product_version: "fixture-product-1.0".into(),
        file_version: "fixture-file-1.0.0.1".into(),
        sha256: std::iter::repeat_n(hash_byte, 64).collect(),
    }
}

fn hello(protocol_version: u16, schema_version: u16) -> Hello {
    Hello {
        protocol_version,
        schema_version,
        peer: artifact("magic-tdx-fake-bridge", 'a'),
        peer_architecture: std::env::consts::ARCH.into(),
        terminal: artifact("fixture-tdx.exe", 'b'),
        transport_profile_id: "fixture-loopback-http-v1".into(),
        terminal_state: TerminalState::Ready,
        capabilities: BTreeMap::from([
            ("price".into(), true),
            ("cumulative_amount".into(), true),
            ("cumulative_volume".into(), true),
            ("source_record_count".into(), true),
        ]),
        entitlements: BTreeMap::from([
            ("price".into(), true),
            ("cumulative_amount".into(), true),
            ("cumulative_volume".into(), true),
            ("source_record_count".into(), true),
        ]),
    }
}

fn status(
    bridge_sequence: u64,
    state: BridgeRuntimeState,
    reason: BridgeStatusReason,
) -> BridgeMessage {
    BridgeMessage::Status(BridgeStatus {
        protocol_version: PROTOCOL_VERSION,
        schema_version: SCHEMA_VERSION,
        bridge_sequence,
        state,
        reason,
        detail: None,
    })
}

fn observation(bridge_sequence: u64) -> BridgeMessage {
    BridgeMessage::Observation(Box::new(SourceObservation {
        protocol_version: PROTOCOL_VERSION,
        schema_version: SCHEMA_VERSION,
        bridge_sequence,
        instrument: SourceInstrument {
            exchange: SourceExchange::Shanghai,
            code: "600000".into(),
        },
        observed_at_utc: "2026-08-13T01:02:03Z".into(),
        source_timestamp: Some("2026-08-13T09:02:03+08:00".into()),
        price: Some(DecimalObservation {
            value: "12.34".into(),
            unit: ObservationUnit::CnyPerShare,
        }),
        cumulative_amount: Some(DecimalObservation {
            value: "123456.78".into(),
            unit: ObservationUnit::Cny,
        }),
        cumulative_volume: Some(DecimalObservation {
            value: "10000".into(),
            unit: ObservationUnit::Lot,
        }),
        source_record_count: Some(42),
    }))
}

fn serve_normal() -> Result<(), String> {
    let codec = FrameCodec::new(FIXTURE_FRAME_LIMIT).map_err(|error| error.to_string())?;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    codec
        .write_message(
            &mut stdout,
            &BridgeMessage::Hello(Box::new(hello(PROTOCOL_VERSION, SCHEMA_VERSION))),
        )
        .map_err(|error| error.to_string())?;
    codec
        .write_message(
            &mut stdout,
            &status(
                1,
                BridgeRuntimeState::Starting,
                BridgeStatusReason::ProcessStarted,
            ),
        )
        .map_err(|error| error.to_string())?;
    codec
        .write_message(
            &mut stdout,
            &status(
                2,
                BridgeRuntimeState::Ready,
                BridgeStatusReason::SourceReady,
            ),
        )
        .map_err(|error| error.to_string())?;
    codec
        .write_message(&mut stdout, &observation(3))
        .map_err(|error| error.to_string())?;

    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let command = codec
        .read_command(&mut stdin)
        .map_err(|error| error.to_string())?;
    match command {
        BridgeCommand::Shutdown(_) => {
            codec
                .write_message(
                    &mut stdout,
                    &status(
                        4,
                        BridgeRuntimeState::Stopping,
                        BridgeStatusReason::ShutdownRequested,
                    ),
                )
                .map_err(|error| error.to_string())?;
            codec
                .write_message(&mut stdout, &BridgeMessage::Stopped(Stopped::current(5)))
                .map_err(|error| error.to_string())
        }
    }
}

fn serve_error() -> Result<(), String> {
    let codec = FrameCodec::new(FIXTURE_FRAME_LIMIT).map_err(|error| error.to_string())?;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    codec
        .write_message(
            &mut stdout,
            &BridgeMessage::Hello(Box::new(hello(PROTOCOL_VERSION, SCHEMA_VERSION))),
        )
        .map_err(|error| error.to_string())?;
    codec
        .write_message(
            &mut stdout,
            &BridgeMessage::Error(BridgeErrorReport {
                protocol_version: PROTOCOL_VERSION,
                schema_version: SCHEMA_VERSION,
                bridge_sequence: 1,
                code: BridgeErrorCode::SourceReadFailed,
                retryable: true,
                message: "deterministic fixture source failure".into(),
            }),
        )
        .map_err(|error| error.to_string())
}

fn serve_sequence_gap() -> Result<(), String> {
    let codec = FrameCodec::new(FIXTURE_FRAME_LIMIT).map_err(|error| error.to_string())?;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    codec
        .write_message(
            &mut stdout,
            &BridgeMessage::Hello(Box::new(hello(PROTOCOL_VERSION, SCHEMA_VERSION))),
        )
        .map_err(|error| error.to_string())?;
    codec
        .write_message(
            &mut stdout,
            &status(
                1,
                BridgeRuntimeState::Ready,
                BridgeStatusReason::SourceReady,
            ),
        )
        .map_err(|error| error.to_string())?;
    codec
        .write_message(&mut stdout, &observation(3))
        .map_err(|error| error.to_string())
}

fn write_unvalidated_hello(protocol_version: u16, schema_version: u16) -> Result<(), String> {
    FrameCodec::new(FIXTURE_FRAME_LIMIT)
        .map_err(|error| error.to_string())?
        .write_json(
            &mut io::stdout().lock(),
            &BridgeMessage::Hello(Box::new(hello(protocol_version, schema_version))),
        )
        .map_err(|error| error.to_string())
}

fn write_raw_frame(announced: u32, payload: &[u8]) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout
        .write_all(&announced.to_be_bytes())
        .and_then(|()| stdout.write_all(payload))
        .and_then(|()| stdout.flush())
        .map_err(|error| error.to_string())
}
