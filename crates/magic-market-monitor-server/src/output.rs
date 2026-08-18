use std::io::Write;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread;
use std::time::Duration;

use magic_tdx_local_rs::TqLoopbackErrorCategory;
use serde::Serialize;
use thiserror::Error;

pub(crate) trait EventSink {
    fn emit(&mut self, event: &ServiceEvent) -> Result<(), OutputError>;

    fn shutdown(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) struct LengthPrefixedJson<W> {
    output: W,
    maximum_bytes: usize,
}

#[cfg(test)]
impl<W: Write> LengthPrefixedJson<W> {
    pub(crate) fn new(output: W, maximum_bytes: usize) -> Result<Self, OutputError> {
        if maximum_bytes == 0 || u32::try_from(maximum_bytes).is_err() {
            return Err(OutputError::InvalidMaximum(maximum_bytes));
        }
        Ok(Self {
            output,
            maximum_bytes,
        })
    }
}

#[cfg(test)]
impl<W: Write> EventSink for LengthPrefixedJson<W> {
    fn emit(&mut self, event: &ServiceEvent) -> Result<(), OutputError> {
        let encoded = serde_json::to_vec(event).map_err(OutputError::Json)?;
        if encoded.is_empty() || encoded.len() > self.maximum_bytes {
            return Err(OutputError::EventTooLarge {
                actual: encoded.len(),
                maximum: self.maximum_bytes,
            });
        }
        let length = u32::try_from(encoded.len()).map_err(|_| OutputError::EventTooLarge {
            actual: encoded.len(),
            maximum: self.maximum_bytes,
        })?;
        self.output
            .write_all(&length.to_be_bytes())
            .map_err(OutputError::Write)?;
        self.output
            .write_all(&encoded)
            .map_err(OutputError::Write)?;
        self.output.flush().map_err(OutputError::Write)
    }
}

enum WriterCommand {
    Frame(Vec<u8>),
    Shutdown,
}

pub(crate) struct BoundedEventWriter {
    commands: Option<SyncSender<WriterCommand>>,
    completion: Receiver<Result<(), String>>,
    handle: Option<thread::JoinHandle<()>>,
    maximum_bytes: usize,
    shutdown_timeout: Duration,
}

impl BoundedEventWriter {
    pub(crate) fn stdout(
        maximum_bytes: usize,
        queue_capacity: usize,
        shutdown_timeout: Duration,
    ) -> Result<Self, OutputError> {
        Self::with_writer(
            std::io::stdout(),
            maximum_bytes,
            queue_capacity,
            shutdown_timeout,
        )
    }

    fn with_writer<W: Write + Send + 'static>(
        mut writer: W,
        maximum_bytes: usize,
        queue_capacity: usize,
        shutdown_timeout: Duration,
    ) -> Result<Self, OutputError> {
        if maximum_bytes == 0 || u32::try_from(maximum_bytes).is_err() {
            return Err(OutputError::InvalidMaximum(maximum_bytes));
        }
        if queue_capacity == 0 {
            return Err(OutputError::InvalidQueueCapacity);
        }
        let (commands, command_rx) = mpsc::sync_channel(queue_capacity);
        let (completion_tx, completion) = mpsc::sync_channel(1);
        let handle = thread::Builder::new()
            .name("market-monitor-stdout".to_owned())
            .spawn(move || {
                let result =
                    writer_loop(&mut writer, command_rx).map_err(|error| error.to_string());
                let _ = completion_tx.send(result);
            })
            .map_err(OutputError::Spawn)?;
        Ok(Self {
            commands: Some(commands),
            completion,
            handle: Some(handle),
            maximum_bytes,
            shutdown_timeout,
        })
    }
}

impl EventSink for BoundedEventWriter {
    fn emit(&mut self, event: &ServiceEvent) -> Result<(), OutputError> {
        let encoded = serde_json::to_vec(event).map_err(OutputError::Json)?;
        if encoded.is_empty() || encoded.len() > self.maximum_bytes {
            return Err(OutputError::EventTooLarge {
                actual: encoded.len(),
                maximum: self.maximum_bytes,
            });
        }
        let Some(commands) = &self.commands else {
            return Err(OutputError::WriterStopped);
        };
        match commands.try_send(WriterCommand::Frame(encoded)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(OutputError::SlowConsumerStop),
            Err(TrySendError::Disconnected(_)) => Err(OutputError::WriterStopped),
        }
    }

    fn shutdown(&mut self) -> Result<(), OutputError> {
        if let Some(commands) = self.commands.take() {
            match commands.try_send(WriterCommand::Shutdown) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => return Err(OutputError::SlowConsumerStop),
                Err(TrySendError::Disconnected(_)) => {}
            }
        }
        match self.completion.recv_timeout(self.shutdown_timeout) {
            Ok(Ok(())) => {
                if let Some(handle) = self.handle.take() {
                    handle.join().map_err(|_| OutputError::WriterPanicked)?;
                }
                Ok(())
            }
            Ok(Err(message)) => Err(OutputError::WriterFailure(message)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(OutputError::ShutdownTimeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(OutputError::WriterStopped),
        }
    }
}

fn writer_loop(writer: &mut impl Write, commands: Receiver<WriterCommand>) -> std::io::Result<()> {
    while let Ok(command) = commands.recv() {
        match command {
            WriterCommand::Frame(encoded) => {
                let length = u32::try_from(encoded.len()).map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "frame exceeds u32")
                })?;
                writer.write_all(&length.to_be_bytes())?;
                writer.write_all(&encoded)?;
                writer.flush()?;
            }
            WriterCommand::Shutdown => break,
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ServiceEvent {
    Waiting {
        reason: String,
        listener_started: bool,
    },
    DiscoveryCandidate {
        generation: u64,
        candidate: crate::discovery::CandidateEvidence,
        data_family_admissions_unchanged: bool,
    },
    LoopbackHealth {
        generation: u64,
        instrument: String,
        schema_valid: bool,
        protocol_version: u16,
        schema_version: u16,
    },
    EquityWatchlistValidated {
        generation: u64,
        configured_instrument_count: usize,
        universe_instrument_count: usize,
        admission_unchanged: bool,
    },
    Running {
        generation: u64,
    },
    Observation {
        generation: u64,
        instrument: String,
        observed_at_utc: String,
        source_observation: magic_tdx_local_rs::SourceObservation,
        price: Option<String>,
        cumulative_volume: Option<String>,
        cumulative_volume_unit: VolumeUnit,
        price_admitted: bool,
        volume_admitted: bool,
        amount_admitted: bool,
        amount: FieldAvailability,
        source_record_count: FieldAvailability,
    },
    Analysis {
        generation: u64,
        admitted: bool,
        instrument: String,
        observed_at_utc: String,
        time_basis: magic_market_core::ObservationTimeBasis,
        update: Box<crate::analysis::AnalysisUpdate>,
    },
    SnapshotObservation {
        generation: u64,
        instrument: String,
        observed_at_utc: String,
        cumulative_amount: String,
        cumulative_amount_unit: AmountUnit,
        snapshot_price: String,
        previous_close: String,
        previous_close_unit: PriceUnit,
        previous_close_admitted: bool,
        open: String,
        high: String,
        low: String,
        ohlc_unit: PriceUnit,
        ohlc_admitted: bool,
        snapshot_volume: String,
        snapshot_volume_unit: VolumeUnit,
        price_matches_last_fast_sample: bool,
        volume_matches_last_fast_sample: bool,
        amount_admitted: bool,
    },
    FamilyUnavailable {
        generation: u64,
        instrument: String,
        family: crate::analysis::AnalysisFamily,
        reason: String,
        skipped_without_advancing: bool,
    },
    LoopbackFailure {
        generation: u64,
        instrument: String,
        operation: LoopbackOperation,
        category: TqLoopbackErrorCategory,
        disposition: FailureDisposition,
        message: String,
        amount_window_cleared: bool,
    },
    SnapshotBusy {
        generation: u64,
        instrument: String,
        bounded_capacity: usize,
    },
    SnapshotWorkerStopped {
        in_flight_at_shutdown: bool,
        joined: bool,
    },
    Reset {
        generation: u64,
        reason: ResetReason,
        windows_cleared: bool,
    },
    RestartBudgetExhausted {
        used: u32,
        budget: u32,
    },
    DiagnosticCompleted {
        completed_cycles: u64,
        configured_cycles: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum FieldAvailability {
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VolumeUnit {
    Lot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AmountUnit {
    Cny,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PriceUnit {
    CnyPerShare,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LoopbackOperation {
    EquityUniverse,
    PriceVolume,
    MarketSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailureDisposition {
    Transient,
    Permanent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResetReason {
    TerminalNotRunning,
    DiscoveryRejected,
    DiscoveryFailed,
    LoopbackConnectFailed,
    LoopbackPollFailed,
    TerminalCandidateChanged,
    TradingDateChanged,
    MiddayBreak,
    SessionOpened,
}

#[derive(Debug, Error)]
pub(crate) enum OutputError {
    #[error("invalid event maximum {0}")]
    InvalidMaximum(usize),
    #[error("unable to encode service event: {0}")]
    Json(#[source] serde_json::Error),
    #[error("event size {actual} exceeds maximum {maximum}")]
    EventTooLarge { actual: usize, maximum: usize },
    #[error("unable to write service event: {0}")]
    #[cfg(test)]
    Write(#[source] std::io::Error),
    #[error("output queue capacity must be positive")]
    InvalidQueueCapacity,
    #[error("unable to spawn stdout writer: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("stdout consumer is slower than the bounded queue; stop policy applied")]
    SlowConsumerStop,
    #[error("stdout writer stopped unexpectedly")]
    WriterStopped,
    #[error("stdout writer failed: {0}")]
    WriterFailure(String),
    #[error("stdout writer panicked")]
    WriterPanicked,
    #[error("stdout writer did not stop within the explicit timeout")]
    ShutdownTimeout,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Condvar, Mutex};

    use super::*;

    #[test]
    fn writes_one_big_endian_bounded_json_frame() {
        let mut bytes = Vec::new();
        let mut output = LengthPrefixedJson::new(&mut bytes, 1024).unwrap();
        output
            .emit(&ServiceEvent::Waiting {
                reason: "terminal_not_running".to_owned(),
                listener_started: false,
            })
            .unwrap();
        let announced = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert_eq!(announced, bytes.len() - 4);
        let value: serde_json::Value = serde_json::from_slice(&bytes[4..]).unwrap();
        assert_eq!(value["listener_started"], false);
    }

    #[test]
    fn refuses_oversize_before_writing_any_bytes() {
        let mut bytes = Vec::new();
        let mut output = LengthPrefixedJson::new(&mut bytes, 4).unwrap();
        assert!(matches!(
            output.emit(&ServiceEvent::Waiting {
                reason: "none".to_owned(),
                listener_started: false
            }),
            Err(OutputError::EventTooLarge { .. })
        ));
        assert!(bytes.is_empty());
    }

    #[derive(Clone)]
    struct GateWriter {
        gate: Arc<(Mutex<bool>, Condvar)>,
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for GateWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let (gate, wake) = &*self.gate;
            let mut open = gate.lock().map_err(|_| std::io::Error::other("poison"))?;
            while !*open {
                open = wake
                    .wait(open)
                    .map_err(|_| std::io::Error::other("poison"))?;
            }
            self.bytes
                .lock()
                .map_err(|_| std::io::Error::other("poison"))?
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn waiting(index: usize) -> ServiceEvent {
        ServiceEvent::Waiting {
            reason: format!("waiting-{index}"),
            listener_started: false,
        }
    }

    #[test]
    fn bounded_writer_applies_stop_policy_without_blocking_producer() {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let writer = GateWriter {
            gate: gate.clone(),
            bytes,
        };
        let mut output =
            BoundedEventWriter::with_writer(writer, 1024, 1, Duration::from_millis(10)).unwrap();
        output.emit(&waiting(1)).unwrap();
        let mut stopped = false;
        for index in 2..100 {
            if matches!(
                output.emit(&waiting(index)),
                Err(OutputError::SlowConsumerStop)
            ) {
                stopped = true;
                break;
            }
        }
        assert!(stopped);
        assert!(matches!(
            output.shutdown(),
            Err(OutputError::SlowConsumerStop | OutputError::ShutdownTimeout)
        ));
        let (open, wake) = &*gate;
        *open.lock().unwrap() = true;
        wake.notify_all();
    }

    #[test]
    fn bounded_writer_preserves_big_endian_json_frames_and_joins() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let gate = Arc::new((Mutex::new(true), Condvar::new()));
        let writer = GateWriter {
            gate,
            bytes: bytes.clone(),
        };
        let mut output =
            BoundedEventWriter::with_writer(writer, 1024, 4, Duration::from_secs(1)).unwrap();
        output.emit(&waiting(1)).unwrap();
        output.shutdown().unwrap();
        let bytes = bytes.lock().unwrap();
        let announced = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert_eq!(announced, bytes.len() - 4);
        let value: serde_json::Value = serde_json::from_slice(&bytes[4..]).unwrap();
        assert_eq!(value["reason"], "waiting-1");
    }
}
