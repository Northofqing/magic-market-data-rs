use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use magic_market_monitor::{
    LOCAL_AMOUNT_CHANGE_ANOMALY_ADMITTED, LOCAL_PRICE_CHANGE_ANOMALY_ADMITTED,
    LOCAL_VOLUME_CHANGE_ANOMALY_ADMITTED,
};
use magic_tdx_local_rs::{
    SourceObservation, TqInstrument, TqLoopbackClient, TqLoopbackError, TqLoopbackErrorCategory,
    LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED, LOCAL_TERMINAL_CUMULATIVE_VOLUME_ADMITTED,
    LOCAL_TERMINAL_PRICE_ADMITTED,
};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::analysis::{AmountAnalysisInput, AnalysisFamily, AnalysisInput, Analyzers};
use crate::config::{Config, OutputSlowConsumerPolicy, WatchInstrument};
use crate::discovery::{CandidateEvidence, DiscoverTerminal, DiscoveryOutcome, SiblingDiscovery};
use crate::output::{
    AmountUnit, BoundedEventWriter, EventSink, FailureDisposition, FieldAvailability,
    LoopbackOperation, OutputError, ResetReason, ServiceEvent, VolumeUnit,
};

pub(crate) struct ProductionService {
    runtime: Runtime<SiblingDiscovery, TqPoller, SnapshotWorker, BoundedEventWriter, SystemClock>,
    poll_interval: Duration,
    rediscover_interval: Duration,
}

impl ProductionService {
    pub(crate) fn new(config: Config) -> Result<Self, ServiceError> {
        let discovery =
            SiblingDiscovery::production(config.discovery_timeout, config.discovery_max_bytes)?;
        let poller = TqPoller {
            client: TqLoopbackClient::new(config.loopback_limits),
        };
        let snapshot = SnapshotWorker::production(config.loopback_limits)?;
        let output = match config.output_slow_consumer_policy {
            OutputSlowConsumerPolicy::Stop => BoundedEventWriter::stdout(
                config.max_event_bytes,
                config.output_queue_capacity,
                config.output_shutdown_timeout,
            )?,
        };
        let poll_interval = config.poll_interval;
        let rediscover_interval = config.rediscover_interval;
        Ok(Self {
            runtime: Runtime::new(
                config,
                discovery,
                poller,
                snapshot,
                output,
                SystemClock::new(),
            ),
            poll_interval,
            rediscover_interval,
        })
    }

    pub(crate) fn run(&mut self) -> Result<(), ServiceError> {
        let run_result = loop {
            match self.runtime.step() {
                Err(error) => break Err(error),
                Ok(Step::PollAgain) => thread::sleep(self.poll_interval),
                Ok(Step::Rediscover) => thread::sleep(self.rediscover_interval),
                Ok(Step::Stop) => break Ok(()),
            }
        };
        let shutdown_result = self.runtime.shutdown_snapshot();
        let output_result = self.runtime.shutdown_output();
        run_result.and(shutdown_result).and(output_result)
    }
}

pub(crate) trait PollPriceVolume {
    fn validate_watchlist(
        &mut self,
        request_id: u64,
        instruments: &[TqInstrument],
    ) -> Result<usize, PollFailure>;

    fn poll(
        &mut self,
        request_id: u64,
        sequence: u64,
        instrument: &TqInstrument,
        observed_at_utc: &str,
    ) -> Result<SourceObservation, PollFailure>;
}

struct TqPoller {
    client: TqLoopbackClient,
}

impl PollPriceVolume for TqPoller {
    fn validate_watchlist(
        &mut self,
        request_id: u64,
        instruments: &[TqInstrument],
    ) -> Result<usize, PollFailure> {
        self.client
            .validate_equity_watchlist(request_id, instruments)
            .map(|evidence| evidence.instrument_count())
            .map_err(PollFailure::from_loopback)
    }

    fn poll(
        &mut self,
        request_id: u64,
        sequence: u64,
        instrument: &TqInstrument,
        observed_at_utc: &str,
    ) -> Result<SourceObservation, PollFailure> {
        self.client
            .poll_price_volume(request_id, sequence, instrument, observed_at_utc)
            .map_err(PollFailure::from_loopback)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PollFailure {
    pub(crate) category: TqLoopbackErrorCategory,
    pub(crate) disposition: FailureDisposition,
    pub(crate) message: String,
}

impl PollFailure {
    fn from_loopback(error: TqLoopbackError) -> Self {
        let category = error.category();
        Self {
            category,
            disposition: failure_disposition(category),
            message: error.to_string(),
        }
    }
}

const fn failure_disposition(category: TqLoopbackErrorCategory) -> FailureDisposition {
    match category {
        TqLoopbackErrorCategory::Connect
        | TqLoopbackErrorCategory::Timeout
        | TqLoopbackErrorCategory::Transport
        | TqLoopbackErrorCategory::HttpStatus
        | TqLoopbackErrorCategory::Read
        | TqLoopbackErrorCategory::Synchronization => FailureDisposition::Transient,
        TqLoopbackErrorCategory::InvalidLimits
        | TqLoopbackErrorCategory::InvalidRequest
        | TqLoopbackErrorCategory::RequestEncoding
        | TqLoopbackErrorCategory::RequestTooLarge
        | TqLoopbackErrorCategory::MissingContentType
        | TqLoopbackErrorCategory::InvalidContentType
        | TqLoopbackErrorCategory::ResponseTooLarge
        | TqLoopbackErrorCategory::InvalidJson
        | TqLoopbackErrorCategory::Rpc
        | TqLoopbackErrorCategory::CorrelationMismatch
        | TqLoopbackErrorCategory::InstrumentIdentity
        | TqLoopbackErrorCategory::Schema => FailureDisposition::Permanent,
    }
}

#[derive(Clone, Debug)]
struct SnapshotJob {
    request_id: u64,
    sequence: u64,
    generation: u64,
    watched: WatchInstrument,
    reading: ClockReading,
    last_fast_price: String,
    last_fast_volume: String,
}

#[derive(Clone, Debug)]
struct SnapshotResult {
    job: SnapshotJob,
    observation: Result<SourceObservation, PollFailure>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SnapshotSubmit {
    Submitted,
    Busy,
}

trait SnapshotSource {
    fn try_submit(&mut self, job: SnapshotJob) -> Result<SnapshotSubmit, ServiceError>;
    fn try_receive(&mut self) -> Result<Option<SnapshotResult>, ServiceError>;
    fn in_flight(&self) -> bool;
    fn shutdown(&mut self) -> Result<(), ServiceError>;
}

enum SnapshotCommand {
    Poll(SnapshotJob),
    Shutdown,
}

struct SnapshotWorker {
    commands: Option<SyncSender<SnapshotCommand>>,
    results: Receiver<SnapshotResult>,
    handle: Option<thread::JoinHandle<()>>,
    in_flight: bool,
}

impl SnapshotWorker {
    fn production(limits: magic_tdx_local_rs::TqLoopbackLimits) -> Result<Self, ServiceError> {
        let client = TqLoopbackClient::new(limits);
        Self::spawn_with(move |job| {
            client
                .poll_market_snapshot(
                    job.request_id,
                    job.sequence,
                    &job.watched.source,
                    &job.reading.observed_at_utc,
                )
                .map_err(PollFailure::from_loopback)
        })
    }

    fn spawn_with<F>(mut poll: F) -> Result<Self, ServiceError>
    where
        F: FnMut(&SnapshotJob) -> Result<SourceObservation, PollFailure> + Send + 'static,
    {
        // At most one job and one result can exist. The public submit invariant
        // additionally forbids a queued job while another request is active.
        let (command_tx, command_rx) = mpsc::sync_channel(1);
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        let handle = thread::Builder::new()
            .name("tdx-market-snapshot".to_owned())
            .spawn(move || {
                while let Ok(command) = command_rx.recv() {
                    match command {
                        SnapshotCommand::Poll(job) => {
                            let observation = poll(&job);
                            if result_tx.send(SnapshotResult { job, observation }).is_err() {
                                break;
                            }
                        }
                        SnapshotCommand::Shutdown => break,
                    }
                }
            })
            .map_err(ServiceError::SnapshotSpawn)?;
        Ok(Self {
            commands: Some(command_tx),
            results: result_rx,
            handle: Some(handle),
            in_flight: false,
        })
    }
}

impl SnapshotSource for SnapshotWorker {
    fn try_submit(&mut self, job: SnapshotJob) -> Result<SnapshotSubmit, ServiceError> {
        if self.in_flight {
            return Ok(SnapshotSubmit::Busy);
        }
        let Some(commands) = &self.commands else {
            return Err(ServiceError::SnapshotWorkerStopped);
        };
        match commands.try_send(SnapshotCommand::Poll(job)) {
            Ok(()) => {
                self.in_flight = true;
                Ok(SnapshotSubmit::Submitted)
            }
            Err(TrySendError::Full(_)) => Ok(SnapshotSubmit::Busy),
            Err(TrySendError::Disconnected(_)) => Err(ServiceError::SnapshotWorkerStopped),
        }
    }

    fn try_receive(&mut self) -> Result<Option<SnapshotResult>, ServiceError> {
        match self.results.try_recv() {
            Ok(result) => {
                self.in_flight = false;
                Ok(Some(result))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) if !self.in_flight => Ok(None),
            Err(TryRecvError::Disconnected) => Err(ServiceError::SnapshotWorkerStopped),
        }
    }

    fn in_flight(&self) -> bool {
        self.in_flight
    }

    fn shutdown(&mut self) -> Result<(), ServiceError> {
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(SnapshotCommand::Shutdown);
        }
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| ServiceError::SnapshotWorkerPanicked)?;
        }
        self.in_flight = false;
        Ok(())
    }
}

impl Drop for SnapshotWorker {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

trait Clock {
    fn now(&mut self) -> Result<ClockReading, ServiceError>;
}

struct SystemClock {
    started: Instant,
}

impl SystemClock {
    fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&mut self) -> Result<ClockReading, ServiceError> {
        let arrival_millis = u64::try_from(self.started.elapsed().as_millis())
            .map_err(|_| ServiceError::ClockOverflow)?;
        let observed_at_utc = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(ServiceError::FormatClock)?;
        Ok(ClockReading {
            arrival_millis,
            observed_at_utc,
        })
    }
}

#[derive(Clone, Debug)]
struct ClockReading {
    arrival_millis: u64,
    observed_at_utc: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SessionMarker {
    shanghai_date: time::Date,
    period: SessionPeriod,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionPeriod {
    Morning,
    Break,
    Afternoon,
    Outside,
}

fn session_marker(observed_at_utc: &str) -> Result<SessionMarker, ServiceError> {
    let utc = OffsetDateTime::parse(observed_at_utc, &Rfc3339)
        .map_err(ServiceError::ParseObservationTime)?;
    let offset = time::UtcOffset::from_hms(8, 0, 0).map_err(|_| ServiceError::ShanghaiOffset)?;
    let local = utc.to_offset(offset);
    let minute = u16::from(local.hour()) * 60 + u16::from(local.minute());
    let period = match minute {
        570..=690 => SessionPeriod::Morning,
        691..=779 => SessionPeriod::Break,
        780..=900 => SessionPeriod::Afternoon,
        _ => SessionPeriod::Outside,
    };
    Ok(SessionMarker {
        shanghai_date: local.date(),
        period,
    })
}

#[derive(Clone, Debug)]
enum Lifecycle {
    Waiting,
    Candidate(CandidateEvidence),
    Running(CandidateEvidence),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    PollAgain,
    Rediscover,
    Stop,
}

struct Runtime<D, P, S, O, C> {
    discovery: D,
    poller: P,
    snapshot: S,
    output: O,
    clock: C,
    watchlist: Vec<WatchInstrument>,
    analyzers: Analyzers,
    lifecycle: Lifecycle,
    generation: u64,
    next_request_id: u64,
    next_sequence: u64,
    next_snapshot_sequence: u64,
    snapshot_cadence_poll_cycles: u64,
    identity_recheck_cycles: u64,
    running_poll_cycles: u64,
    next_snapshot_watch_index: usize,
    consecutive_failures: u32,
    restart_budget: u32,
    diagnostic_poll_cycles: u64,
    completed_cycles: u64,
    last_arrival_millis: Option<u64>,
    last_session_marker: Option<SessionMarker>,
    watchlist_validated_generation: Option<u64>,
}

impl<D, P, S, O, C> Runtime<D, P, S, O, C>
where
    D: DiscoverTerminal,
    P: PollPriceVolume,
    S: SnapshotSource,
    O: EventSink,
    C: Clock,
{
    fn new(config: Config, discovery: D, poller: P, snapshot: S, output: O, clock: C) -> Self {
        let analyzers = Analyzers::new(
            config.rule_limits,
            config.price_rule,
            config.amount_rule,
            config.volume_rule,
        );
        Self {
            discovery,
            poller,
            snapshot,
            output,
            clock,
            watchlist: config.watchlist,
            analyzers,
            lifecycle: Lifecycle::Waiting,
            generation: 0,
            next_request_id: 1,
            next_sequence: 1,
            next_snapshot_sequence: 1,
            snapshot_cadence_poll_cycles: config.snapshot_cadence_poll_cycles,
            identity_recheck_cycles: config.identity_recheck_cycles,
            running_poll_cycles: 0,
            next_snapshot_watch_index: 0,
            consecutive_failures: 0,
            restart_budget: config.restart_budget,
            diagnostic_poll_cycles: config.diagnostic_poll_cycles,
            completed_cycles: 0,
            last_arrival_millis: None,
            last_session_marker: None,
            watchlist_validated_generation: None,
        }
    }

    fn step(&mut self) -> Result<Step, ServiceError> {
        let action = self.step_inner()?;
        if action == Step::Stop {
            return Ok(action);
        }
        self.completed_cycles = self
            .completed_cycles
            .checked_add(1)
            .ok_or(ServiceError::DiagnosticCycleExhausted)?;
        if self.diagnostic_poll_cycles != 0 && self.completed_cycles >= self.diagnostic_poll_cycles
        {
            self.output.emit(&ServiceEvent::DiagnosticCompleted {
                completed_cycles: self.completed_cycles,
                configured_cycles: self.diagnostic_poll_cycles,
            })?;
            return Ok(Step::Stop);
        }
        Ok(action)
    }

    fn step_inner(&mut self) -> Result<Step, ServiceError> {
        self.drain_snapshot_result()?;
        if let Lifecycle::Running(candidate) = self.lifecycle.clone() {
            if self.running_poll_cycles != 0
                && self
                    .running_poll_cycles
                    .is_multiple_of(self.identity_recheck_cycles)
            {
                return self.recheck_identity(candidate);
            }
        }
        match self.lifecycle.clone() {
            Lifecycle::Running(candidate) | Lifecycle::Candidate(candidate) => {
                return self.poll_candidate(candidate);
            }
            Lifecycle::Waiting => {}
        }
        let discovery = match self.discovery.discover() {
            Ok(outcome) => outcome,
            Err(error) => {
                eprintln!("discovery failed: {error}");
                return self.fail_to_waiting(ResetReason::DiscoveryFailed, error.to_string());
            }
        };
        match discovery {
            DiscoveryOutcome::None { reason } => {
                self.reset_to_waiting(ResetReason::TerminalNotRunning)?;
                self.output.emit(&ServiceEvent::Waiting {
                    reason,
                    listener_started: false,
                })?;
                Ok(Step::Rediscover)
            }
            DiscoveryOutcome::Rejected { reason } => {
                self.reset_to_waiting(ResetReason::DiscoveryRejected)?;
                self.output.emit(&ServiceEvent::Waiting {
                    reason,
                    listener_started: false,
                })?;
                Ok(Step::Rediscover)
            }
            DiscoveryOutcome::Candidate(candidate) => self.poll_candidate(candidate),
        }
    }

    fn recheck_identity(&mut self, current: CandidateEvidence) -> Result<Step, ServiceError> {
        match self.discovery.discover() {
            Ok(DiscoveryOutcome::Candidate(candidate)) => {
                let same = candidate.process_id == current.process_id
                    && candidate.session_id == current.session_id
                    && candidate.process_creation_time_100ns_since_1601
                        == current.process_creation_time_100ns_since_1601;
                if same {
                    self.poll_candidate(current)
                } else {
                    self.reset_windows(ResetReason::TerminalCandidateChanged)?;
                    self.lifecycle = Lifecycle::Waiting;
                    self.poll_candidate(candidate)
                }
            }
            Ok(DiscoveryOutcome::None { reason }) => {
                self.fail_to_waiting(ResetReason::TerminalNotRunning, reason)
            }
            Ok(DiscoveryOutcome::Rejected { reason }) => {
                self.fail_to_waiting(ResetReason::DiscoveryRejected, reason)
            }
            Err(error) => {
                eprintln!("identity recheck failed: {error}");
                self.fail_to_waiting(ResetReason::DiscoveryFailed, error.to_string())
            }
        }
    }

    fn poll_candidate(&mut self, candidate: CandidateEvidence) -> Result<Step, ServiceError> {
        let health_pending = !matches!(self.lifecycle, Lifecycle::Running(_));
        let same_candidate = match &self.lifecycle {
            Lifecycle::Candidate(previous) | Lifecycle::Running(previous) => {
                previous.process_id == candidate.process_id
                    && previous.session_id == candidate.session_id
                    && previous.process_creation_time_100ns_since_1601
                        == candidate.process_creation_time_100ns_since_1601
            }
            Lifecycle::Waiting => false,
        };
        if !same_candidate {
            if !matches!(self.lifecycle, Lifecycle::Waiting) {
                self.reset_windows(ResetReason::TerminalCandidateChanged)?;
            }
            self.generation = self
                .generation
                .checked_add(1)
                .ok_or(ServiceError::GenerationExhausted)?;
            self.next_sequence = 1;
            self.next_snapshot_sequence = 1;
            self.running_poll_cycles = 0;
            self.next_snapshot_watch_index = 0;
            self.last_arrival_millis = None;
            self.last_session_marker = None;
            self.watchlist_validated_generation = None;
            self.lifecycle = Lifecycle::Candidate(candidate.clone());
            self.output.emit(&ServiceEvent::DiscoveryCandidate {
                generation: self.generation,
                candidate: candidate.clone(),
                data_family_admissions_unchanged: true,
            })?;
        }

        if self.watchlist_validated_generation != Some(self.generation) {
            let request_id = self.take_request_id()?;
            let instruments = self
                .watchlist
                .iter()
                .map(|watched| watched.source.clone())
                .collect::<Vec<_>>();
            match self.poller.validate_watchlist(request_id, &instruments) {
                Ok(universe_instrument_count) => {
                    self.watchlist_validated_generation = Some(self.generation);
                    self.output.emit(&ServiceEvent::EquityWatchlistValidated {
                        generation: self.generation,
                        configured_instrument_count: instruments.len(),
                        universe_instrument_count,
                        admission_unchanged: true,
                    })?;
                }
                Err(error) => {
                    self.output.emit(&ServiceEvent::LoopbackFailure {
                        generation: self.generation,
                        instrument: "EQUITY_WATCHLIST".to_owned(),
                        operation: LoopbackOperation::EquityUniverse,
                        category: error.category,
                        disposition: error.disposition,
                        message: error.message.clone(),
                        amount_window_cleared: false,
                    })?;
                    return self.fail_to_waiting(ResetReason::LoopbackPollFailed, error.message);
                }
            }
        }

        let mut first = true;
        let mut fast_samples = Vec::with_capacity(self.watchlist.len());
        let watchlist = self.watchlist.clone();
        for watched in watchlist {
            let reading = self.monotonic_reading()?;
            let request_id = self.take_request_id()?;
            let sequence = self.take_sequence()?;
            let observation = match self.poller.poll(
                request_id,
                sequence,
                &watched.source,
                &reading.observed_at_utc,
            ) {
                Ok(observation) => observation,
                Err(error) => {
                    eprintln!(
                        "loopback poll failed for {}: {}",
                        watched.label, error.message
                    );
                    self.output.emit(&ServiceEvent::LoopbackFailure {
                        generation: self.generation,
                        instrument: watched.label.clone(),
                        operation: LoopbackOperation::PriceVolume,
                        category: error.category,
                        disposition: error.disposition,
                        message: error.message.clone(),
                        amount_window_cleared: false,
                    })?;
                    let reason = if error.category == TqLoopbackErrorCategory::Connect {
                        ResetReason::LoopbackConnectFailed
                    } else {
                        ResetReason::LoopbackPollFailed
                    };
                    return self.fail_to_waiting(reason, error.message);
                }
            };
            let values = match validate_observation(
                &observation,
                &watched,
                sequence,
                &reading.observed_at_utc,
            ) {
                Ok(values) => values,
                Err(error) => {
                    let message = error.to_string();
                    eprintln!(
                        "loopback schema validation failed for {}: {message}",
                        watched.label
                    );
                    self.output.emit(&ServiceEvent::FamilyUnavailable {
                        generation: self.generation,
                        instrument: watched.label.clone(),
                        family: AnalysisFamily::Price,
                        reason: message.clone(),
                        skipped_without_advancing: true,
                    })?;
                    self.output.emit(&ServiceEvent::FamilyUnavailable {
                        generation: self.generation,
                        instrument: watched.label.clone(),
                        family: AnalysisFamily::Volume,
                        reason: message,
                        skipped_without_advancing: true,
                    })?;
                    continue;
                }
            };
            if values.price.is_none() {
                self.output.emit(&ServiceEvent::FamilyUnavailable {
                    generation: self.generation,
                    instrument: watched.label.clone(),
                    family: AnalysisFamily::Price,
                    reason: "Now is zero, missing, or not representable as a positive price"
                        .to_owned(),
                    skipped_without_advancing: true,
                })?;
            }
            if values.volume.is_none() {
                self.output.emit(&ServiceEvent::FamilyUnavailable {
                    generation: self.generation,
                    instrument: watched.label.clone(),
                    family: AnalysisFamily::Volume,
                    reason: "Volume is missing or not representable".to_owned(),
                    skipped_without_advancing: true,
                })?;
            }
            if first && health_pending {
                self.output.emit(&ServiceEvent::LoopbackHealth {
                    generation: self.generation,
                    instrument: watched.label.clone(),
                    schema_valid: true,
                    protocol_version: observation.protocol_version,
                    schema_version: observation.schema_version,
                })?;
                if !matches!(self.lifecycle, Lifecycle::Running(_)) {
                    self.lifecycle = Lifecycle::Running(candidate.clone());
                    self.output.emit(&ServiceEvent::Running {
                        generation: self.generation,
                    })?;
                }
                first = false;
            }
            self.output.emit(&ServiceEvent::Observation {
                generation: self.generation,
                instrument: watched.label.clone(),
                observed_at_utc: observation.observed_at_utc.clone(),
                source_observation: observation,
                price: values.price_text.clone(),
                cumulative_volume: values.volume_text.clone(),
                cumulative_volume_unit: VolumeUnit::Lot,
                price_admitted: LOCAL_TERMINAL_PRICE_ADMITTED,
                volume_admitted: LOCAL_TERMINAL_CUMULATIVE_VOLUME_ADMITTED,
                amount_admitted: LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED,
                amount: FieldAvailability::Unavailable,
                source_record_count: FieldAvailability::Unavailable,
            })?;
            for update in self
                .analyzers
                .process(AnalysisInput {
                    instrument_label: &watched.label,
                    exchange: watched.source.source_instrument().exchange,
                    code: &watched.source.source_instrument().code,
                    observed_at_utc: &reading.observed_at_utc,
                    arrival_millis: reading.arrival_millis,
                    generation: self.generation,
                    sequence,
                    price: values.price,
                    cumulative_volume_lots: values.volume,
                })
                .map_err(ServiceError::Analysis)?
            {
                let admitted = match update.family {
                    crate::analysis::AnalysisFamily::Price => {
                        LOCAL_TERMINAL_PRICE_ADMITTED && LOCAL_PRICE_CHANGE_ANOMALY_ADMITTED
                    }
                    crate::analysis::AnalysisFamily::Amount => {
                        LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED
                            && LOCAL_AMOUNT_CHANGE_ANOMALY_ADMITTED
                    }
                    crate::analysis::AnalysisFamily::Volume => {
                        LOCAL_TERMINAL_CUMULATIVE_VOLUME_ADMITTED
                            && LOCAL_VOLUME_CHANGE_ANOMALY_ADMITTED
                    }
                };
                self.output.emit(&ServiceEvent::Analysis {
                    generation: self.generation,
                    admitted,
                    update,
                })?;
            }
            fast_samples.push(FastSample {
                watched,
                price: values.price_text.unwrap_or_default(),
                volume: values.volume_text.unwrap_or_default(),
            });
        }
        self.running_poll_cycles = self
            .running_poll_cycles
            .checked_add(1)
            .ok_or(ServiceError::DiagnosticCycleExhausted)?;
        if !fast_samples.is_empty()
            && self
                .running_poll_cycles
                .is_multiple_of(self.snapshot_cadence_poll_cycles)
        {
            self.schedule_snapshot(&fast_samples)?;
        }
        self.consecutive_failures = 0;
        Ok(Step::PollAgain)
    }

    fn schedule_snapshot(&mut self, samples: &[FastSample]) -> Result<(), ServiceError> {
        let index = self.next_snapshot_watch_index % samples.len();
        let sample = &samples[index];
        if self.snapshot.in_flight() {
            self.output.emit(&ServiceEvent::SnapshotBusy {
                generation: self.generation,
                instrument: sample.watched.label.clone(),
                bounded_capacity: 1,
            })?;
            return Ok(());
        }
        let reading = self.monotonic_reading()?;
        let job = SnapshotJob {
            request_id: self.take_request_id()?,
            sequence: self.take_snapshot_sequence()?,
            generation: self.generation,
            watched: sample.watched.clone(),
            reading,
            last_fast_price: sample.price.clone(),
            last_fast_volume: sample.volume.clone(),
        };
        match self.snapshot.try_submit(job)? {
            SnapshotSubmit::Submitted => {
                self.next_snapshot_watch_index = (index + 1) % samples.len();
            }
            SnapshotSubmit::Busy => {
                self.output.emit(&ServiceEvent::SnapshotBusy {
                    generation: self.generation,
                    instrument: sample.watched.label.clone(),
                    bounded_capacity: 1,
                })?;
            }
        }
        Ok(())
    }

    fn drain_snapshot_result(&mut self) -> Result<(), ServiceError> {
        let Some(result) = self.snapshot.try_receive()? else {
            return Ok(());
        };
        if result.job.generation != self.generation
            || !matches!(
                self.lifecycle,
                Lifecycle::Candidate(_) | Lifecycle::Running(_)
            )
        {
            self.output.emit(&ServiceEvent::FamilyUnavailable {
                generation: result.job.generation,
                instrument: result.job.watched.label,
                family: AnalysisFamily::Amount,
                reason: "stale snapshot result from a previous lifecycle generation".to_owned(),
                skipped_without_advancing: true,
            })?;
            return Ok(());
        }
        let observation = match result.observation {
            Ok(observation) => observation,
            Err(error) => {
                let cleared = self.analyzers.reset_amount();
                self.output.emit(&ServiceEvent::LoopbackFailure {
                    generation: self.generation,
                    instrument: result.job.watched.label,
                    operation: LoopbackOperation::MarketSnapshot,
                    category: error.category,
                    disposition: error.disposition,
                    message: error.message,
                    amount_window_cleared: cleared,
                })?;
                return Ok(());
            }
        };
        let values = match validate_snapshot(&observation, &result.job) {
            Ok(values) => values,
            Err(message) => {
                let cleared = self.analyzers.reset_amount();
                self.output.emit(&ServiceEvent::LoopbackFailure {
                    generation: self.generation,
                    instrument: result.job.watched.label,
                    operation: LoopbackOperation::MarketSnapshot,
                    category: TqLoopbackErrorCategory::Schema,
                    disposition: FailureDisposition::Permanent,
                    message,
                    amount_window_cleared: cleared,
                })?;
                return Ok(());
            }
        };
        self.output.emit(&ServiceEvent::SnapshotObservation {
            generation: self.generation,
            instrument: result.job.watched.label.clone(),
            observed_at_utc: observation.observed_at_utc.clone(),
            cumulative_amount: values.amount_text.clone(),
            cumulative_amount_unit: AmountUnit::Cny,
            snapshot_price: values.price_text.clone(),
            snapshot_volume: values.volume_text.clone(),
            snapshot_volume_unit: VolumeUnit::Lot,
            price_matches_last_fast_sample: values.price_text == result.job.last_fast_price,
            volume_matches_last_fast_sample: values.volume_text == result.job.last_fast_volume,
            amount_admitted: LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED,
        })?;
        for update in self
            .analyzers
            .process_amount(AmountAnalysisInput {
                instrument_label: &result.job.watched.label,
                exchange: result.job.watched.source.source_instrument().exchange,
                code: &result.job.watched.source.source_instrument().code,
                observed_at_utc: &result.job.reading.observed_at_utc,
                arrival_millis: result.job.reading.arrival_millis,
                generation: result.job.generation,
                sequence: result.job.sequence,
                cumulative_amount_cny: values.amount,
            })
            .map_err(ServiceError::Analysis)?
        {
            self.output.emit(&ServiceEvent::Analysis {
                generation: self.generation,
                admitted: LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED
                    && LOCAL_AMOUNT_CHANGE_ANOMALY_ADMITTED,
                update,
            })?;
        }
        Ok(())
    }

    fn shutdown_snapshot(&mut self) -> Result<(), ServiceError> {
        let in_flight = self.snapshot.in_flight();
        self.snapshot.shutdown()?;
        self.output.emit(&ServiceEvent::SnapshotWorkerStopped {
            in_flight_at_shutdown: in_flight,
            joined: true,
        })?;
        Ok(())
    }

    fn shutdown_output(&mut self) -> Result<(), ServiceError> {
        self.output.shutdown().map_err(ServiceError::Output)
    }

    fn monotonic_reading(&mut self) -> Result<ClockReading, ServiceError> {
        let mut reading = self.clock.now()?;
        if let Some(previous) = self.last_arrival_millis {
            reading.arrival_millis = reading
                .arrival_millis
                .max(previous.checked_add(1).ok_or(ServiceError::ClockOverflow)?);
        }
        self.last_arrival_millis = Some(reading.arrival_millis);
        self.apply_conservative_session_reset(&reading.observed_at_utc)?;
        Ok(reading)
    }

    fn apply_conservative_session_reset(
        &mut self,
        observed_at_utc: &str,
    ) -> Result<(), ServiceError> {
        let marker = session_marker(observed_at_utc)?;
        let reason = self.last_session_marker.and_then(|previous| {
            if previous.shanghai_date != marker.shanghai_date {
                Some(ResetReason::TradingDateChanged)
            } else if matches!(
                previous.period,
                SessionPeriod::Morning | SessionPeriod::Break
            ) && marker.period == SessionPeriod::Afternoon
            {
                Some(ResetReason::MiddayBreak)
            } else if previous.period == SessionPeriod::Outside
                && matches!(
                    marker.period,
                    SessionPeriod::Morning | SessionPeriod::Afternoon
                )
            {
                Some(ResetReason::SessionOpened)
            } else {
                None
            }
        });
        self.last_session_marker = Some(marker);
        if let Some(reason) = reason {
            self.reset_windows(reason)?;
        }
        Ok(())
    }

    fn take_request_id(&mut self) -> Result<u64, ServiceError> {
        let value = self.next_request_id;
        self.next_request_id = value
            .checked_add(1)
            .ok_or(ServiceError::RequestIdExhausted)?;
        Ok(value)
    }

    fn take_sequence(&mut self) -> Result<u64, ServiceError> {
        let value = self.next_sequence;
        self.next_sequence = value
            .checked_add(1)
            .ok_or(ServiceError::SequenceExhausted)?;
        Ok(value)
    }

    fn take_snapshot_sequence(&mut self) -> Result<u64, ServiceError> {
        let value = self.next_snapshot_sequence;
        self.next_snapshot_sequence = value
            .checked_add(1)
            .ok_or(ServiceError::SequenceExhausted)?;
        Ok(value)
    }

    fn fail_to_waiting(
        &mut self,
        reason: ResetReason,
        message: String,
    ) -> Result<Step, ServiceError> {
        self.reset_to_waiting(reason)?;
        self.output.emit(&ServiceEvent::Waiting {
            reason: message,
            listener_started: false,
        })?;
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures > self.restart_budget {
            self.output.emit(&ServiceEvent::RestartBudgetExhausted {
                used: self.consecutive_failures,
                budget: self.restart_budget,
            })?;
            return Ok(Step::Stop);
        }
        Ok(Step::Rediscover)
    }

    fn reset_to_waiting(&mut self, reason: ResetReason) -> Result<(), ServiceError> {
        if !matches!(self.lifecycle, Lifecycle::Waiting) {
            self.reset_windows(reason)?;
        }
        self.lifecycle = Lifecycle::Waiting;
        self.next_sequence = 1;
        self.next_snapshot_sequence = 1;
        self.running_poll_cycles = 0;
        self.last_arrival_millis = None;
        self.last_session_marker = None;
        self.watchlist_validated_generation = None;
        Ok(())
    }

    fn reset_windows(&mut self, reason: ResetReason) -> Result<(), ServiceError> {
        let windows_cleared = self.analyzers.reset();
        self.output.emit(&ServiceEvent::Reset {
            generation: self.generation,
            reason,
            windows_cleared,
        })?;
        Ok(())
    }
}

struct ObservationValues {
    price_text: Option<String>,
    volume_text: Option<String>,
    price: Option<f64>,
    volume: Option<f64>,
}

fn validate_observation(
    observation: &SourceObservation,
    watched: &WatchInstrument,
    expected_sequence: u64,
    expected_observed_at_utc: &str,
) -> Result<ObservationValues, ServiceError> {
    observation
        .validate()
        .map_err(|error| ServiceError::Observation(error.to_string()))?;
    if &observation.instrument != watched.source.source_instrument() {
        return Err(ServiceError::Observation(
            "loopback instrument correlation mismatch".to_owned(),
        ));
    }
    if observation.bridge_sequence != expected_sequence {
        return Err(ServiceError::Observation(
            "loopback sequence correlation mismatch".to_owned(),
        ));
    }
    if observation.observed_at_utc != expected_observed_at_utc {
        return Err(ServiceError::Observation(
            "loopback observation-time correlation mismatch".to_owned(),
        ));
    }
    if observation.cumulative_amount.is_some()
        || observation.source_record_count.is_some()
        || observation.source_timestamp.is_some()
    {
        return Err(ServiceError::Observation(
            "price-volume method returned fields outside the initial fixed schema".to_owned(),
        ));
    }
    let price = observation.price.as_ref();
    let volume = observation.cumulative_volume.as_ref();
    let price_value = price
        .and_then(|value| value.value.parse::<f64>().ok())
        .filter(|value| *value > 0.0);
    let volume_value = volume.and_then(|value| value.value.parse::<f64>().ok());
    Ok(ObservationValues {
        price_text: price
            .zip(price_value)
            .map(|(decimal, _)| decimal.value.clone()),
        volume_text: volume
            .zip(volume_value)
            .map(|(decimal, _)| decimal.value.clone()),
        price: price_value,
        volume: volume_value,
    })
}

struct FastSample {
    watched: WatchInstrument,
    price: String,
    volume: String,
}

struct SnapshotValues {
    amount_text: String,
    price_text: String,
    volume_text: String,
    amount: f64,
}

fn validate_snapshot(
    observation: &SourceObservation,
    job: &SnapshotJob,
) -> Result<SnapshotValues, String> {
    observation.validate().map_err(|error| error.to_string())?;
    if &observation.instrument != job.watched.source.source_instrument() {
        return Err("snapshot instrument correlation mismatch".to_owned());
    }
    if observation.bridge_sequence != job.sequence {
        return Err("snapshot sequence correlation mismatch".to_owned());
    }
    if observation.observed_at_utc != job.reading.observed_at_utc {
        return Err("snapshot observation-time correlation mismatch".to_owned());
    }
    if observation.source_timestamp.is_some() || observation.source_record_count.is_some() {
        return Err("snapshot returned fields outside the fixed schema".to_owned());
    }
    let amount = observation
        .cumulative_amount
        .as_ref()
        .filter(|value| value.unit == magic_tdx_local_rs::ObservationUnit::Cny)
        .ok_or_else(|| "snapshot amount CNY is unavailable".to_owned())?;
    let price = observation
        .price
        .as_ref()
        .filter(|value| value.unit == magic_tdx_local_rs::ObservationUnit::CnyPerShare)
        .ok_or_else(|| "snapshot cross-check price is unavailable".to_owned())?;
    let volume = observation
        .cumulative_volume
        .as_ref()
        .filter(|value| value.unit == magic_tdx_local_rs::ObservationUnit::Lot)
        .ok_or_else(|| "snapshot cross-check volume lots are unavailable".to_owned())?;
    let amount_value = amount
        .value
        .parse::<f64>()
        .map_err(|_| "snapshot amount cannot be represented for analysis".to_owned())?;
    Ok(SnapshotValues {
        amount_text: amount.value.clone(),
        price_text: price.value.clone(),
        volume_text: volume.value.clone(),
        amount: amount_value,
    })
}

#[derive(Debug, Error)]
pub(crate) enum ServiceError {
    #[error(transparent)]
    Discovery(#[from] crate::discovery::DiscoveryError),
    #[error(transparent)]
    Output(#[from] OutputError),
    #[error("system clock cannot be represented as milliseconds")]
    ClockOverflow,
    #[error("diagnostic cycle counter exhausted")]
    DiagnosticCycleExhausted,
    #[error("unable to format UTC observation time: {0}")]
    FormatClock(#[source] time::error::Format),
    #[error("unable to parse UTC observation time: {0}")]
    ParseObservationTime(#[source] time::error::Parse),
    #[error("fixed Shanghai UTC offset is invalid")]
    ShanghaiOffset,
    #[error("service generation exhausted")]
    GenerationExhausted,
    #[error("loopback request identifier exhausted")]
    RequestIdExhausted,
    #[error("generation-local sequence exhausted")]
    SequenceExhausted,
    #[error("invalid loopback observation: {0}")]
    Observation(String),
    #[error("monitor analysis failed: {0}")]
    Analysis(String),
    #[error("unable to spawn the bounded snapshot worker: {0}")]
    SnapshotSpawn(#[source] std::io::Error),
    #[error("snapshot worker stopped unexpectedly")]
    SnapshotWorkerStopped,
    #[error("snapshot worker panicked")]
    SnapshotWorkerPanicked,
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use magic_tdx_local_rs::{
        DecimalObservation, ObservationUnit, SourceExchange, SourceInstrument, PROTOCOL_VERSION,
        SCHEMA_VERSION,
    };

    use super::*;

    struct FakeDiscovery {
        values: VecDeque<DiscoveryOutcome>,
    }

    impl DiscoverTerminal for FakeDiscovery {
        fn discover(&mut self) -> Result<DiscoveryOutcome, crate::discovery::DiscoveryError> {
            Ok(self.values.pop_front().expect("scripted discovery"))
        }
    }

    struct FakePoller {
        values: VecDeque<Result<SourceObservation, PollFailure>>,
        calls: usize,
    }

    #[derive(Default)]
    struct FakeSnapshot {
        submitted: Vec<SnapshotJob>,
        results: VecDeque<SnapshotResult>,
        in_flight: bool,
        shutdown: bool,
    }

    impl SnapshotSource for FakeSnapshot {
        fn try_submit(&mut self, job: SnapshotJob) -> Result<SnapshotSubmit, ServiceError> {
            if self.in_flight {
                return Ok(SnapshotSubmit::Busy);
            }
            self.submitted.push(job);
            self.in_flight = true;
            Ok(SnapshotSubmit::Submitted)
        }

        fn try_receive(&mut self) -> Result<Option<SnapshotResult>, ServiceError> {
            let result = self.results.pop_front();
            if result.is_some() {
                self.in_flight = false;
            }
            Ok(result)
        }

        fn in_flight(&self) -> bool {
            self.in_flight
        }

        fn shutdown(&mut self) -> Result<(), ServiceError> {
            self.shutdown = true;
            self.in_flight = false;
            Ok(())
        }
    }

    impl PollPriceVolume for FakePoller {
        fn validate_watchlist(
            &mut self,
            _request_id: u64,
            instruments: &[TqInstrument],
        ) -> Result<usize, PollFailure> {
            assert!(!instruments.is_empty());
            if instruments
                .iter()
                .any(|instrument| instrument.source_instrument().code == "999999")
            {
                return Err(PollFailure {
                    category: TqLoopbackErrorCategory::InstrumentIdentity,
                    disposition: FailureDisposition::Permanent,
                    message: "scripted missing equity identity".to_owned(),
                });
            }
            Ok(5_552)
        }

        fn poll(
            &mut self,
            _request_id: u64,
            _sequence: u64,
            _instrument: &TqInstrument,
            _observed_at_utc: &str,
        ) -> Result<SourceObservation, PollFailure> {
            self.calls += 1;
            self.values.pop_front().expect("scripted poll")
        }
    }

    #[derive(Default)]
    struct FakeOutput(Vec<ServiceEvent>);

    impl EventSink for FakeOutput {
        fn emit(&mut self, event: &ServiceEvent) -> Result<(), OutputError> {
            self.0.push(event.clone());
            Ok(())
        }
    }

    struct FakeClock {
        next: u64,
    }

    impl Clock for FakeClock {
        fn now(&mut self) -> Result<ClockReading, ServiceError> {
            let next = self.next;
            self.next += 100;
            Ok(ClockReading {
                arrival_millis: next,
                observed_at_utc: "2026-08-13T01:02:03Z".to_owned(),
            })
        }
    }

    fn candidate(process_id: u32) -> CandidateEvidence {
        CandidateEvidence {
            discovery_schema_version: 1,
            process_id,
            session_id: 7,
            process_creation_time_100ns_since_1601: 1234,
            executable_architecture: Some("x86_64".to_owned()),
            executable_sha256: Some("abc".to_owned()),
            executable_file_version: None,
            executable_product_version: None,
            executable_version_source: None,
            executable_version_failure: None,
        }
    }

    fn observation(sequence: u64, price: &str, volume: &str) -> SourceObservation {
        SourceObservation {
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
            bridge_sequence: sequence,
            instrument: SourceInstrument {
                exchange: SourceExchange::Shanghai,
                code: "600000".to_owned(),
            },
            observed_at_utc: "2026-08-13T01:02:03Z".to_owned(),
            source_timestamp: None,
            price: Some(DecimalObservation {
                value: price.to_owned(),
                unit: ObservationUnit::CnyPerShare,
            }),
            cumulative_amount: None,
            cumulative_volume: Some(DecimalObservation {
                value: volume.to_owned(),
                unit: ObservationUnit::Lot,
            }),
            source_record_count: None,
        }
    }

    fn snapshot(job: &SnapshotJob, amount: &str) -> SourceObservation {
        SourceObservation {
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
            bridge_sequence: job.sequence,
            instrument: job.watched.source.source_instrument().clone(),
            observed_at_utc: job.reading.observed_at_utc.clone(),
            source_timestamp: None,
            price: Some(DecimalObservation {
                value: job.last_fast_price.clone(),
                unit: ObservationUnit::CnyPerShare,
            }),
            cumulative_amount: Some(DecimalObservation {
                value: amount.to_owned(),
                unit: ObservationUnit::Cny,
            }),
            cumulative_volume: Some(DecimalObservation {
                value: job.last_fast_volume.clone(),
                unit: ObservationUnit::Lot,
            }),
            source_record_count: None,
        }
    }

    fn config(restart_budget: u32) -> Config {
        Config {
            watchlist: vec![WatchInstrument {
                label: "EQUITY:SH:600000".to_owned(),
                source: TqInstrument::new(SourceExchange::Shanghai, "600000").unwrap(),
            }],
            poll_interval: Duration::from_millis(1),
            rediscover_interval: Duration::from_millis(1),
            discovery_timeout: Duration::from_millis(1),
            discovery_max_bytes: 4096,
            loopback_limits: magic_tdx_local_rs::TqLoopbackLimits::new(
                Duration::from_millis(1),
                Duration::from_millis(1),
                Duration::from_millis(1),
                1024,
                4096,
            )
            .unwrap(),
            rule_limits: crate::analysis::RuleLimits::new(1, 8).unwrap(),
            price_rule: crate::analysis::PriceRule::new(1, 100, 0, 0.1, 0.02, 10).unwrap(),
            amount_rule: crate::analysis::AmountRule::new(1, 200, 0, 50.0, 10.0, 10).unwrap(),
            volume_rule: crate::analysis::VolumeRule::new(1, 100, 0, 50.0, 10.0, 10).unwrap(),
            snapshot_cadence_poll_cycles: 1,
            identity_recheck_cycles: 100,
            restart_budget,
            diagnostic_poll_cycles: 0,
            max_event_bytes: 8192,
            output_queue_capacity: 16,
            output_shutdown_timeout: Duration::from_millis(100),
            output_slow_consumer_policy: OutputSlowConsumerPolicy::Stop,
        }
    }

    fn runtime(
        discovery: Vec<DiscoveryOutcome>,
        polls: Vec<Result<SourceObservation, PollFailure>>,
        restart_budget: u32,
    ) -> Runtime<FakeDiscovery, FakePoller, FakeSnapshot, FakeOutput, FakeClock> {
        Runtime::new(
            config(restart_budget),
            FakeDiscovery {
                values: discovery.into(),
            },
            FakePoller {
                values: polls.into(),
                calls: 0,
            },
            FakeSnapshot::default(),
            FakeOutput::default(),
            FakeClock { next: 1 },
        )
    }

    #[test]
    fn missing_terminal_waits_without_polling_or_starting_listener() {
        let mut runtime = runtime(
            vec![DiscoveryOutcome::None {
                reason: "terminal_not_running".to_owned(),
            }],
            vec![],
            1,
        );
        assert_eq!(runtime.step().unwrap(), Step::Rediscover);
        assert_eq!(runtime.poller.calls, 0);
        assert!(matches!(
            runtime.output.0.as_slice(),
            [ServiceEvent::Waiting {
                listener_started: false,
                ..
            }]
        ));
    }

    #[test]
    fn candidate_requires_loopback_schema_health_before_running_and_amount_stays_unavailable() {
        let mut runtime = runtime(
            vec![DiscoveryOutcome::Candidate(candidate(42))],
            vec![Ok(observation(1, "10.0", "100"))],
            1,
        );
        assert_eq!(runtime.step().unwrap(), Step::PollAgain);
        let health = runtime
            .output
            .0
            .iter()
            .position(|event| matches!(event, ServiceEvent::LoopbackHealth { .. }))
            .unwrap();
        let watchlist_validated = runtime
            .output
            .0
            .iter()
            .position(|event| matches!(event, ServiceEvent::EquityWatchlistValidated { .. }))
            .unwrap();
        let running = runtime
            .output
            .0
            .iter()
            .position(|event| matches!(event, ServiceEvent::Running { .. }))
            .unwrap();
        assert!(watchlist_validated < health);
        assert!(health < running);
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::Observation {
                amount: FieldAvailability::Unavailable,
                price_admitted: true,
                volume_admitted: true,
                ..
            }
        )));
    }

    #[test]
    fn invalid_equity_identity_fails_before_any_market_poll_or_health_event() {
        let mut cfg = config(0);
        cfg.watchlist = vec![WatchInstrument {
            label: "EQUITY:SH:999999".to_owned(),
            source: TqInstrument::new(SourceExchange::Shanghai, "999999").unwrap(),
        }];
        let mut runtime = Runtime::new(
            cfg,
            FakeDiscovery {
                values: vec![DiscoveryOutcome::Candidate(candidate(42))].into(),
            },
            FakePoller {
                values: VecDeque::new(),
                calls: 0,
            },
            FakeSnapshot::default(),
            FakeOutput::default(),
            FakeClock { next: 1 },
        );
        assert_eq!(runtime.step().unwrap(), Step::Stop);
        assert_eq!(runtime.poller.calls, 0);
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::LoopbackFailure {
                operation: LoopbackOperation::EquityUniverse,
                category: TqLoopbackErrorCategory::InstrumentIdentity,
                disposition: FailureDisposition::Permanent,
                ..
            }
        )));
        assert!(!runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::LoopbackHealth { .. }
                | ServiceEvent::Running { .. }
                | ServiceEvent::Observation { .. }
                | ServiceEvent::Analysis { .. }
        )));
    }

    #[test]
    fn connect_failure_resets_windows_and_returns_to_waiting() {
        let mut runtime = runtime(
            vec![
                DiscoveryOutcome::Candidate(candidate(42)),
                DiscoveryOutcome::Candidate(candidate(42)),
            ],
            vec![
                Ok(observation(1, "10.0", "100")),
                Err(PollFailure {
                    category: TqLoopbackErrorCategory::Connect,
                    disposition: FailureDisposition::Transient,
                    message: "connect".to_owned(),
                }),
            ],
            1,
        );
        assert_eq!(runtime.step().unwrap(), Step::PollAgain);
        assert_eq!(runtime.step().unwrap(), Step::Rediscover);
        assert!(matches!(runtime.lifecycle, Lifecycle::Waiting));
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::Reset {
                reason: ResetReason::LoopbackConnectFailed,
                windows_cleared: true,
                ..
            }
        )));
    }

    #[test]
    fn running_uses_cached_candidate_and_loopback_failure_returns_to_waiting() {
        let mut runtime = runtime(
            vec![
                DiscoveryOutcome::Candidate(candidate(42)),
                DiscoveryOutcome::None {
                    reason: "terminal_not_running".to_owned(),
                },
            ],
            vec![
                Ok(observation(1, "10.0", "100")),
                Err(PollFailure {
                    category: TqLoopbackErrorCategory::Connect,
                    disposition: FailureDisposition::Transient,
                    message: "terminal exited".to_owned(),
                }),
            ],
            1,
        );
        assert_eq!(runtime.step().unwrap(), Step::PollAgain);
        assert_eq!(runtime.step().unwrap(), Step::Rediscover);
        assert_eq!(runtime.poller.calls, 2);
        assert_eq!(runtime.discovery.values.len(), 1);
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::Reset {
                reason: ResetReason::LoopbackConnectFailed,
                ..
            }
        )));
    }

    #[test]
    fn loopback_health_is_emitted_once_for_multiple_running_cycles() {
        let mut runtime = runtime(
            vec![DiscoveryOutcome::Candidate(candidate(42))],
            vec![
                Ok(observation(1, "10.0", "100")),
                Ok(observation(2, "10.1", "120")),
            ],
            1,
        );
        assert_eq!(runtime.step().unwrap(), Step::PollAgain);
        assert_eq!(runtime.step().unwrap(), Step::PollAgain);
        assert_eq!(runtime.discovery.values.len(), 0);
        assert_eq!(
            runtime
                .output
                .0
                .iter()
                .filter(|event| matches!(event, ServiceEvent::LoopbackHealth { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn diagnostic_cycle_bound_stops_without_external_process_kill() {
        let mut config = config(1);
        config.diagnostic_poll_cycles = 1;
        let mut runtime = Runtime::new(
            config,
            FakeDiscovery {
                values: vec![DiscoveryOutcome::Candidate(candidate(42))].into(),
            },
            FakePoller {
                values: vec![Ok(observation(1, "10.0", "100"))].into(),
                calls: 0,
            },
            FakeSnapshot::default(),
            FakeOutput::default(),
            FakeClock { next: 1 },
        );
        assert_eq!(runtime.step().unwrap(), Step::Stop);
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::DiagnosticCompleted {
                completed_cycles: 1,
                configured_cycles: 1
            }
        )));
    }

    #[test]
    fn diagnostic_cycle_bound_also_stops_a_waiting_only_run() {
        let mut config = config(1);
        config.diagnostic_poll_cycles = 1;
        let mut runtime = Runtime::new(
            config,
            FakeDiscovery {
                values: vec![DiscoveryOutcome::None {
                    reason: "terminal_not_running".to_owned(),
                }]
                .into(),
            },
            FakePoller {
                values: VecDeque::new(),
                calls: 0,
            },
            FakeSnapshot::default(),
            FakeOutput::default(),
            FakeClock { next: 1 },
        );
        assert_eq!(runtime.step().unwrap(), Step::Stop);
        assert_eq!(runtime.poller.calls, 0);
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::DiagnosticCompleted {
                completed_cycles: 1,
                configured_cycles: 1
            }
        )));
    }

    #[test]
    fn zero_restart_budget_stops_after_first_loopback_failure() {
        let mut runtime = runtime(
            vec![DiscoveryOutcome::Candidate(candidate(42))],
            vec![Err(PollFailure {
                category: TqLoopbackErrorCategory::Schema,
                disposition: FailureDisposition::Permanent,
                message: "schema".to_owned(),
            })],
            0,
        );
        assert_eq!(runtime.step().unwrap(), Step::Stop);
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::RestartBudgetExhausted { used: 1, budget: 0 }
        )));
    }

    #[test]
    fn slow_snapshot_does_not_block_fast_poll_and_queue_stays_single_in_flight() {
        let mut runtime = runtime(
            vec![DiscoveryOutcome::Candidate(candidate(42))],
            vec![
                Ok(observation(1, "10.0", "100")),
                Ok(observation(2, "10.1", "110")),
            ],
            1,
        );
        assert_eq!(runtime.step().unwrap(), Step::PollAgain);
        assert!(runtime.snapshot.in_flight);
        assert_eq!(runtime.snapshot.submitted.len(), 1);
        assert_eq!(runtime.step().unwrap(), Step::PollAgain);
        assert_eq!(runtime.poller.calls, 2);
        assert_eq!(runtime.snapshot.submitted.len(), 1);
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::SnapshotBusy {
                bounded_capacity: 1,
                ..
            }
        )));
    }

    #[test]
    fn snapshot_amount_warms_and_triggers_without_replaying_fast_families() {
        let mut runtime = runtime(
            vec![DiscoveryOutcome::Candidate(candidate(42))],
            vec![
                Ok(observation(1, "10.0", "100")),
                Ok(observation(2, "10.0", "100")),
                Ok(observation(3, "10.0", "100")),
                Ok(observation(4, "10.0", "100")),
            ],
            1,
        );
        runtime.step().unwrap();
        let job1 = runtime.snapshot.submitted[0].clone();
        runtime.snapshot.results.push_back(SnapshotResult {
            job: job1.clone(),
            observation: Ok(snapshot(&job1, "100")),
        });
        runtime.step().unwrap();
        let job2 = runtime.snapshot.submitted[1].clone();
        runtime.snapshot.results.push_back(SnapshotResult {
            job: job2.clone(),
            observation: Ok(snapshot(&job2, "160")),
        });
        runtime.step().unwrap();
        let job3 = runtime.snapshot.submitted[2].clone();
        runtime.snapshot.results.push_back(SnapshotResult {
            job: job3.clone(),
            observation: Ok(snapshot(&job3, "230")),
        });
        runtime.step().unwrap();
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::Analysis {
                admitted: false,
                update: crate::analysis::AnalysisUpdate {
                    family: AnalysisFamily::Amount,
                    transition: crate::analysis::AnalysisTransition::Triggered { value: 70.0 },
                    value_unit: "cny",
                    ..
                },
                ..
            }
        )));
        assert_eq!(runtime.poller.calls, 4);
    }

    #[test]
    fn snapshot_failure_is_typed_and_resets_only_amount_window() {
        let mut runtime = runtime(
            vec![DiscoveryOutcome::Candidate(candidate(42))],
            vec![
                Ok(observation(1, "10.0", "100")),
                Ok(observation(2, "10.1", "110")),
                Ok(observation(3, "10.2", "120")),
            ],
            1,
        );
        runtime.step().unwrap();
        let first = runtime.snapshot.submitted[0].clone();
        runtime.snapshot.results.push_back(SnapshotResult {
            job: first.clone(),
            observation: Ok(snapshot(&first, "100")),
        });
        runtime.step().unwrap();
        let second = runtime.snapshot.submitted[1].clone();
        runtime.snapshot.results.push_back(SnapshotResult {
            job: second,
            observation: Err(PollFailure {
                category: TqLoopbackErrorCategory::Timeout,
                disposition: FailureDisposition::Transient,
                message: "timed out".to_owned(),
            }),
        });
        runtime.step().unwrap();
        assert!(matches!(runtime.lifecycle, Lifecycle::Running(_)));
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::LoopbackFailure {
                operation: LoopbackOperation::MarketSnapshot,
                category: TqLoopbackErrorCategory::Timeout,
                disposition: FailureDisposition::Transient,
                amount_window_cleared: true,
                ..
            }
        )));
    }

    #[test]
    fn shutdown_is_typed_and_calls_snapshot_shutdown() {
        let mut runtime = runtime(vec![], vec![], 1);
        runtime.shutdown_snapshot().unwrap();
        assert!(runtime.snapshot.shutdown);
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::SnapshotWorkerStopped { joined: true, .. }
        )));
    }

    #[test]
    fn zero_price_skips_only_price_family_and_continues_watchlist() {
        let mut cfg = config(1);
        cfg.watchlist.push(WatchInstrument {
            label: "EQUITY:SZ:000001".to_owned(),
            source: TqInstrument::new(SourceExchange::Shenzhen, "000001").unwrap(),
        });
        cfg.rule_limits = crate::analysis::RuleLimits::new(2, 8).unwrap();
        let mut second = observation(2, "11.0", "200");
        second.instrument = SourceInstrument {
            exchange: SourceExchange::Shenzhen,
            code: "000001".to_owned(),
        };
        let mut runtime = Runtime::new(
            cfg,
            FakeDiscovery {
                values: vec![DiscoveryOutcome::Candidate(candidate(42))].into(),
            },
            FakePoller {
                values: vec![Ok(observation(1, "0", "100")), Ok(second)].into(),
                calls: 0,
            },
            FakeSnapshot::default(),
            FakeOutput::default(),
            FakeClock { next: 1 },
        );
        assert_eq!(runtime.step().unwrap(), Step::PollAgain);
        assert_eq!(runtime.poller.calls, 2);
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::FamilyUnavailable {
                family: AnalysisFamily::Price,
                skipped_without_advancing: true,
                ..
            }
        )));
    }

    #[test]
    fn identity_recheck_keeps_or_replaces_the_generation_by_process_identity() {
        let mut cfg = config(1);
        cfg.identity_recheck_cycles = 1;
        let mut runtime = Runtime::new(
            cfg,
            FakeDiscovery {
                values: vec![
                    DiscoveryOutcome::Candidate(candidate(42)),
                    DiscoveryOutcome::Candidate(candidate(43)),
                ]
                .into(),
            },
            FakePoller {
                values: vec![
                    Ok(observation(1, "10", "100")),
                    Ok(observation(1, "11", "110")),
                ]
                .into(),
                calls: 0,
            },
            FakeSnapshot::default(),
            FakeOutput::default(),
            FakeClock { next: 1 },
        );
        runtime.step().unwrap();
        runtime.step().unwrap();
        assert_eq!(runtime.generation, 2);
        assert!(runtime.output.0.iter().any(|event| matches!(
            event,
            ServiceEvent::Reset {
                reason: ResetReason::TerminalCandidateChanged,
                ..
            }
        )));
    }

    #[test]
    fn shanghai_session_marker_is_fixed_offset_and_conservatively_classified() {
        assert_eq!(
            session_marker("2026-08-13T01:30:00Z").unwrap().period,
            SessionPeriod::Morning
        );
        assert_eq!(
            session_marker("2026-08-13T03:31:00Z").unwrap().period,
            SessionPeriod::Break
        );
        assert_eq!(
            session_marker("2026-08-13T05:00:00Z").unwrap().period,
            SessionPeriod::Afternoon
        );
    }

    #[test]
    fn conservative_session_boundaries_clear_active_windows() {
        let mut runtime = runtime(
            vec![DiscoveryOutcome::Candidate(candidate(42))],
            vec![Ok(observation(1, "10", "100"))],
            1,
        );
        runtime.step().unwrap();
        runtime
            .apply_conservative_session_reset("2026-08-13T01:30:00Z")
            .unwrap();
        runtime
            .analyzers
            .process(AnalysisInput {
                instrument_label: "EQUITY:SH:600000",
                exchange: SourceExchange::Shanghai,
                code: "600000",
                observed_at_utc: "2026-08-13T02:00:00Z",
                arrival_millis: 1000,
                generation: 1,
                sequence: 50,
                price: Some(10.1),
                cumulative_volume_lots: Some(110.0),
            })
            .unwrap();
        runtime
            .apply_conservative_session_reset("2026-08-13T03:31:00Z")
            .unwrap();
        runtime
            .apply_conservative_session_reset("2026-08-13T05:00:00Z")
            .unwrap();
        runtime
            .analyzers
            .process(AnalysisInput {
                instrument_label: "EQUITY:SH:600000",
                exchange: SourceExchange::Shanghai,
                code: "600000",
                observed_at_utc: "2026-08-13T05:01:00Z",
                arrival_millis: 2000,
                generation: 1,
                sequence: 51,
                price: Some(10.2),
                cumulative_volume_lots: Some(120.0),
            })
            .unwrap();
        runtime
            .apply_conservative_session_reset("2026-08-14T01:30:00Z")
            .unwrap();
        for reason in [
            ResetReason::SessionOpened,
            ResetReason::MiddayBreak,
            ResetReason::TradingDateChanged,
        ] {
            assert!(runtime.output.0.iter().any(|event| matches!(
                event,
                ServiceEvent::Reset {
                    reason: actual,
                    windows_cleared: true,
                    ..
                } if *actual == reason
            )));
        }
    }
}
