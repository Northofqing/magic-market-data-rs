use std::path::Path;
use std::time::Duration;

use magic_market_grpc_contracts::v1;
use magic_market_grpc_contracts::{CANONICAL_JSON_CONTENT_TYPE, PROTOCOL_VERSION};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::metadata::MetadataValue;
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint, Identity};
use tonic::Request;

use crate::config::AgentConfig;

pub(crate) struct AgentClient {
    endpoint: Endpoint,
    authorization: MetadataValue<tonic::metadata::Ascii>,
    command_timeout: Duration,
    heartbeat_interval: Duration,
    reconnect_delay: Duration,
    queue_capacity: usize,
    generation_prefix: String,
    watchlist_revision: u64,
    watchlist_instruments: Vec<String>,
    maximum_watchlist_instruments: u32,
}

struct PendingFrame {
    raw: Vec<u8>,
    sequence: u64,
}

impl AgentClient {
    pub(crate) fn new(
        config: &AgentConfig,
        generation: String,
        watchlist_revision: u64,
        watchlist_instruments: Vec<String>,
        maximum_watchlist_instruments: u32,
    ) -> Result<Self, ClientError> {
        let maximum = usize::try_from(maximum_watchlist_instruments).map_err(|_| {
            ClientError::InvalidWatchlist("maximum is not representable".to_owned())
        })?;
        magic_market_grpc_contracts::validate_monitor_watchlist(&watchlist_instruments, maximum)
            .map_err(|error| ClientError::InvalidWatchlist(error.to_string()))?;
        let mut endpoint = Endpoint::from_shared(config.server_uri.clone())?
            .connect_timeout(config.connect_timeout);
        if let Some(tls) = &config.tls {
            let ca = read_bounded(&tls.ca)?;
            let certificate = read_bounded(&tls.certificate)?;
            let private_key = read_bounded(&tls.private_key)?;
            endpoint = endpoint.tls_config(
                ClientTlsConfig::new()
                    .domain_name(tls.domain.clone())
                    .ca_certificate(Certificate::from_pem(ca))
                    .identity(Identity::from_pem(certificate, private_key)),
            )?;
        }
        let authorization = format!("Bearer {}", config.auth_token)
            .parse()
            .map_err(|_| ClientError::InvalidAuthorization)?;
        Ok(Self {
            endpoint,
            authorization,
            command_timeout: config.command_timeout,
            heartbeat_interval: config.heartbeat_interval,
            reconnect_delay: config.reconnect_delay,
            queue_capacity: config.queue_capacity,
            generation_prefix: generation,
            watchlist_revision,
            watchlist_instruments,
            maximum_watchlist_instruments,
        })
    }

    async fn receive_initial_ack(
        &self,
        commands: &mut tonic::Streaming<v1::AgentCommand>,
        generation: &str,
    ) -> Result<(u64, Option<v1::AgentConfigureWatchlist>), ClientError> {
        let mut configuration = None;
        loop {
            let command = receive_command(commands, self.command_timeout).await?;
            match command.body {
                Some(v1::agent_command::Body::Ack(ack))
                    if ack.terminal_generation == generation =>
                {
                    return Ok((ack.accepted_sequence, configuration));
                }
                Some(v1::agent_command::Body::ConfigureWatchlist(value)) => {
                    self.absorb_configuration_value(value, &mut configuration)?;
                }
                Some(v1::agent_command::Body::Stop(stop)) => {
                    return Err(ClientError::Stopped(stop.reason_code));
                }
                _ => return Err(ClientError::InvalidAcknowledgement),
            }
        }
    }

    fn absorb_configuration(
        &self,
        command: v1::AgentCommand,
        pending: &mut Option<v1::AgentConfigureWatchlist>,
    ) -> Result<(), ClientError> {
        match command.body {
            Some(v1::agent_command::Body::ConfigureWatchlist(value)) => {
                self.absorb_configuration_value(value, pending)
            }
            Some(v1::agent_command::Body::Stop(stop)) => {
                Err(ClientError::Stopped(stop.reason_code))
            }
            _ => Err(ClientError::InvalidAcknowledgement),
        }
    }

    fn absorb_configuration_value(
        &self,
        value: v1::AgentConfigureWatchlist,
        pending: &mut Option<v1::AgentConfigureWatchlist>,
    ) -> Result<(), ClientError> {
        let maximum = usize::try_from(self.maximum_watchlist_instruments).map_err(|_| {
            ClientError::InvalidWatchlist("maximum is not representable".to_owned())
        })?;
        magic_market_grpc_contracts::validate_monitor_watchlist(&value.instruments, maximum)
            .map_err(|error| ClientError::InvalidWatchlist(error.to_string()))?;
        let (current_revision, current_instruments) = pending.as_ref().map_or(
            (
                self.watchlist_revision,
                self.watchlist_instruments.as_slice(),
            ),
            |configuration| (configuration.revision, configuration.instruments.as_slice()),
        );
        if value.revision < current_revision {
            return Err(ClientError::StaleWatchlistRevision {
                current: current_revision,
                received: value.revision,
            });
        }
        if value.revision == current_revision {
            if value.instruments != current_instruments {
                return Err(ClientError::WatchlistRevisionContradiction(value.revision));
            }
            return Ok(());
        }
        *pending = Some(value);
        Ok(())
    }

    pub(crate) async fn forward(
        &self,
        mut frames: mpsc::Receiver<Vec<u8>>,
    ) -> Result<ForwardOutcome, ClientError> {
        let mut accepted_sequence = 0_u64;
        let mut pending: Option<PendingFrame> = None;
        let mut monitor_generation = None;
        let mut generation = format!("{}:waiting", self.generation_prefix);
        loop {
            let result = self
                .session(
                    &generation,
                    &mut monitor_generation,
                    &mut frames,
                    &mut pending,
                    &mut accepted_sequence,
                )
                .await;
            match result {
                Ok(SessionEnd::FramesComplete) => return Ok(ForwardOutcome::FramesComplete),
                Ok(SessionEnd::Reconfigure(configuration)) => {
                    return Ok(ForwardOutcome::Reconfigure(configuration))
                }
                Ok(SessionEnd::GenerationChanged(value)) => {
                    monitor_generation = Some(value);
                    generation = format!("{}:{value}", self.generation_prefix);
                    accepted_sequence = 0;
                    tokio::time::sleep(self.reconnect_delay).await;
                }
                Ok(SessionEnd::Reconnect) | Err(ClientError::Transport(_)) => {
                    tokio::time::sleep(self.reconnect_delay).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn session(
        &self,
        generation: &str,
        monitor_generation: &mut Option<u64>,
        frames: &mut mpsc::Receiver<Vec<u8>>,
        pending: &mut Option<PendingFrame>,
        accepted_sequence: &mut u64,
    ) -> Result<SessionEnd, ClientError> {
        let channel = self.endpoint.connect().await?;
        let mut client = v1::tdx_agent_service_client::TdxAgentServiceClient::new(channel);
        let (outgoing, receiver) = mpsc::channel(self.queue_capacity);
        outgoing
            .send(v1::AgentMessage {
                body: Some(v1::agent_message::Body::Hello(self.hello(generation))),
            })
            .await
            .map_err(|_| ClientError::OutgoingStopped)?;
        let mut request = Request::new(ReceiverStream::new(receiver));
        request
            .metadata_mut()
            .insert("authorization", self.authorization.clone());
        let mut commands = client.open_stream(request).await?.into_inner();
        let (server_sequence, configuration) =
            self.receive_initial_ack(&mut commands, generation).await?;
        if let Some(frame) = pending.as_ref() {
            if server_sequence == frame.sequence {
                *accepted_sequence = frame.sequence;
                *pending = None;
            } else if server_sequence != *accepted_sequence {
                return Err(ClientError::SequenceContradiction {
                    local: *accepted_sequence,
                    server: server_sequence,
                });
            }
        } else if server_sequence != *accepted_sequence {
            return Err(ClientError::SequenceContradiction {
                local: *accepted_sequence,
                server: server_sequence,
            });
        }
        if let Some(configuration) = configuration {
            return Ok(SessionEnd::Reconfigure(configuration));
        }
        let mut heartbeat = tokio::time::interval(self.heartbeat_interval);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        heartbeat.tick().await;

        loop {
            if pending.is_none() {
                tokio::select! {
                    frame = frames.recv() => match frame {
                        Some(raw) => {
                            if let Some(value) = frame_generation(&raw)? {
                                if Some(value) != *monitor_generation {
                                    *pending = Some(PendingFrame { raw, sequence: 1 });
                                    return Ok(SessionEnd::GenerationChanged(value));
                                }
                            }
                            let sequence = accepted_sequence.checked_add(1).ok_or(ClientError::SequenceExhausted)?;
                            *pending = Some(PendingFrame { raw, sequence });
                        }
                        None => return Ok(SessionEnd::FramesComplete),
                    },
                    command = commands.message() => {
                        let Some(command) = command? else {
                            return Ok(SessionEnd::Reconnect);
                        };
                        let mut configuration = None;
                        self.absorb_configuration(command, &mut configuration)?;
                        if let Some(configuration) = configuration {
                            return Ok(SessionEnd::Reconfigure(configuration));
                        }
                        continue;
                    },
                    _ = heartbeat.tick() => {
                        outgoing
                            .send(v1::AgentMessage {
                                body: Some(v1::agent_message::Body::Heartbeat(v1::AgentHeartbeat {
                                    terminal_generation: generation.to_owned(),
                                    last_sequence: *accepted_sequence,
                                })),
                            })
                            .await
                            .map_err(|_| ClientError::OutgoingStopped)?;
                        let mut configuration = None;
                        let sequence = loop {
                            let command = receive_command(&mut commands, self.command_timeout).await?;
                            match command.body {
                                Some(v1::agent_command::Body::Ack(ack))
                                    if ack.terminal_generation == generation =>
                                {
                                    break ack.accepted_sequence;
                                }
                                Some(v1::agent_command::Body::ConfigureWatchlist(value)) => {
                                    self.absorb_configuration_value(value, &mut configuration)?;
                                }
                                Some(v1::agent_command::Body::Stop(stop)) => {
                                    return Err(ClientError::Stopped(stop.reason_code));
                                }
                                _ => return Err(ClientError::InvalidAcknowledgement),
                            }
                        };
                        if sequence != *accepted_sequence {
                            return Err(ClientError::SequenceContradiction {
                                local: *accepted_sequence,
                                server: sequence,
                            });
                        }
                        if let Some(configuration) = configuration {
                            return Ok(SessionEnd::Reconfigure(configuration));
                        }
                    }
                }
            }
            let frame = pending.as_ref().ok_or(ClientError::MissingPendingFrame)?;
            outgoing
                .send(v1::AgentMessage {
                    body: Some(v1::agent_message::Body::Event(event_from_frame(
                        generation,
                        frame.sequence,
                        &frame.raw,
                    )?)),
                })
                .await
                .map_err(|_| ClientError::OutgoingStopped)?;
            let mut configuration = None;
            let sequence = loop {
                let command = receive_command(&mut commands, self.command_timeout).await?;
                match command.body {
                    Some(v1::agent_command::Body::Ack(ack))
                        if ack.terminal_generation == generation =>
                    {
                        break ack.accepted_sequence;
                    }
                    Some(v1::agent_command::Body::ConfigureWatchlist(value)) => {
                        self.absorb_configuration_value(value, &mut configuration)?;
                    }
                    Some(v1::agent_command::Body::Stop(stop)) => {
                        return Err(ClientError::Stopped(stop.reason_code));
                    }
                    _ => return Err(ClientError::InvalidAcknowledgement),
                }
            };
            if sequence != frame.sequence {
                return Err(ClientError::SequenceContradiction {
                    local: frame.sequence,
                    server: sequence,
                });
            }
            *accepted_sequence = sequence;
            *pending = None;
            if let Some(configuration) = configuration {
                return Ok(SessionEnd::Reconfigure(configuration));
            }
        }
    }

    fn hello(&self, generation: &str) -> v1::AgentHello {
        v1::AgentHello {
            protocol_version: PROTOCOL_VERSION,
            agent_id: "magic-market-tdx-agent".to_owned(),
            terminal_generation: generation.to_owned(),
            terminal_evidence: Some(v1::CanonicalPayload {
                schema: "magic.market.tdx_agent_evidence".to_owned(),
                schema_version: 1,
                content_type: CANONICAL_JSON_CONTENT_TYPE.to_owned(),
                data: br#"{"monitor":"fixed_sibling","tdx_origin":"http://127.0.0.1:17709/","admitted":false}"#.to_vec(),
            }),
            watchlist_revision: self.watchlist_revision,
            watchlist_instruments: self.watchlist_instruments.clone(),
            maximum_watchlist_instruments: self.maximum_watchlist_instruments,
        }
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, ClientError> {
    let metadata = std::fs::metadata(path).map_err(|source| ClientError::TlsFile {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.len() == 0 || metadata.len() > 1_048_576 {
        return Err(ClientError::TlsFileSize(path.to_path_buf()));
    }
    std::fs::read(path).map_err(|source| ClientError::TlsFile {
        path: path.to_path_buf(),
        source,
    })
}

async fn receive_command(
    commands: &mut tonic::Streaming<v1::AgentCommand>,
    timeout: Duration,
) -> Result<v1::AgentCommand, ClientError> {
    tokio::time::timeout(timeout, commands.message())
        .await
        .map_err(|_| ClientError::CommandTimeout)?
        .map_err(ClientError::Status)?
        .ok_or(ClientError::CommandStreamClosed)
}

fn event_from_frame(
    generation: &str,
    sequence: u64,
    raw: &[u8],
) -> Result<v1::MarketEventEnvelope, ClientError> {
    let document: serde_json::Value =
        serde_json::from_slice(raw).map_err(ClientError::FrameJson)?;
    let event_kind = document
        .get("type")
        .and_then(serde_json::Value::as_str)
        .ok_or(ClientError::MissingEventType)?;
    let provider = if event_kind == "analysis" {
        "LocalAnalysis"
    } else {
        "LocalTerminal"
    };
    let instrument = document
        .get("instrument")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("TDX.LOCAL");
    let observed_at = document
        .get("observed_at_utc")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
        });
    let admission = event_admission(event_kind, &document)?;
    Ok(v1::MarketEventEnvelope {
        protocol_version: PROTOCOL_VERSION,
        event_id: format!("{generation}:{sequence}"),
        cursor: Some(v1::EventCursor {
            generation: generation.to_owned(),
            sequence,
        }),
        event_kind: event_kind.to_owned(),
        provider: provider.to_owned(),
        instrument: instrument.to_owned(),
        observed_at,
        source_at: String::new(),
        admission,
        payload: Some(v1::CanonicalPayload {
            schema: format!("magic.market.monitor.{event_kind}"),
            schema_version: 1,
            content_type: CANONICAL_JSON_CONTENT_TYPE.to_owned(),
            data: raw.to_vec(),
        }),
    })
}

fn event_admission(event_kind: &str, document: &serde_json::Value) -> Result<i32, ClientError> {
    let admitted = match event_kind {
        "observation" => {
            admitted_field(document, "price_admitted", "price")?
                || admitted_field(document, "volume_admitted", "cumulative_volume")?
        }
        "snapshot_observation" => admitted_field(document, "amount_admitted", "cumulative_amount")?,
        "analysis" => {
            if document
                .get("admitted")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
            {
                return Err(ClientError::UnadmittedAnalysisClaim);
            }
            false
        }
        _ => false,
    };
    Ok(if admitted {
        v1::AdmissionState::Admitted as i32
    } else {
        v1::AdmissionState::Unadmitted as i32
    })
}

fn admitted_field(
    document: &serde_json::Value,
    admission_field: &'static str,
    value_field: &'static str,
) -> Result<bool, ClientError> {
    let admitted = document
        .get(admission_field)
        .and_then(serde_json::Value::as_bool)
        .ok_or(ClientError::InvalidAdmissionMarker(admission_field))?;
    if !admitted {
        return Ok(false);
    }
    if document
        .get(value_field)
        .and_then(serde_json::Value::as_str)
        .is_none()
    {
        return Err(ClientError::AdmittedFieldMissing(value_field));
    }
    Ok(true)
}

fn frame_generation(raw: &[u8]) -> Result<Option<u64>, ClientError> {
    let document: serde_json::Value =
        serde_json::from_slice(raw).map_err(ClientError::FrameJson)?;
    match document.get("generation") {
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or(ClientError::InvalidMonitorGeneration),
        None => Ok(None),
    }
}

enum SessionEnd {
    FramesComplete,
    Reconnect,
    GenerationChanged(u64),
    Reconfigure(v1::AgentConfigureWatchlist),
}

pub(crate) enum ForwardOutcome {
    FramesComplete,
    Reconfigure(v1::AgentConfigureWatchlist),
}

#[derive(Debug, Error)]
pub(crate) enum ClientError {
    #[error("invalid gRPC endpoint: {0}")]
    InvalidEndpoint(#[from] tonic::transport::Error),
    #[error("gRPC transport failed: {0}")]
    Transport(#[from] tonic::Status),
    #[error("invalid authorization metadata")]
    InvalidAuthorization,
    #[error("unable to read TLS file {path}: {source}")]
    TlsFile {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("TLS file is empty or exceeds 1 MiB: {0}")]
    TlsFileSize(std::path::PathBuf),
    #[error("agent outgoing stream stopped")]
    OutgoingStopped,
    #[error("agent command deadline exceeded")]
    CommandTimeout,
    #[error("agent command stream failed: {0}")]
    Status(tonic::Status),
    #[error("agent command stream closed")]
    CommandStreamClosed,
    #[error("server stopped the agent: {0}")]
    Stopped(String),
    #[error("server returned an invalid acknowledgement")]
    InvalidAcknowledgement,
    #[error("invalid monitor watchlist: {0}")]
    InvalidWatchlist(String),
    #[error("stale watchlist revision: current={current}, received={received}")]
    StaleWatchlistRevision { current: u64, received: u64 },
    #[error("watchlist revision {0} has contradictory instruments")]
    WatchlistRevisionContradiction(u64),
    #[error("agent sequence exhausted")]
    SequenceExhausted,
    #[error("agent/server sequence contradiction: local={local}, server={server}")]
    SequenceContradiction { local: u64, server: u64 },
    #[error("pending frame state is missing")]
    MissingPendingFrame,
    #[error("monitor frame is not JSON: {0}")]
    FrameJson(serde_json::Error),
    #[error("monitor frame has no event type")]
    MissingEventType,
    #[error("monitor frame has an invalid or missing admission marker: {0}")]
    InvalidAdmissionMarker(&'static str),
    #[error("monitor frame admitted a field without a value: {0}")]
    AdmittedFieldMissing(&'static str),
    #[error("local analysis admission remains disabled")]
    UnadmittedAnalysisClaim,
    #[error("monitor frame generation is not an unsigned integer")]
    InvalidMonitorGeneration,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> AgentClient {
        AgentClient {
            endpoint: Endpoint::from_static("http://127.0.0.1:1"),
            authorization: "Bearer test".parse().unwrap(),
            command_timeout: Duration::from_secs(1),
            heartbeat_interval: Duration::from_millis(50),
            reconnect_delay: Duration::from_secs(1),
            queue_capacity: 1,
            generation_prefix: "00000000-0000-4000-8000-000000000001".to_owned(),
            watchlist_revision: 2,
            watchlist_instruments: vec!["EQUITY:SH:600396".to_owned()],
            maximum_watchlist_instruments: 2,
        }
    }

    #[test]
    fn raw_observation_uses_repository_admission_markers() {
        let raw = br#"{"type":"observation","instrument":"600396.SH","observed_at_utc":"2026-08-15T00:00:00Z","price":"17.18","cumulative_volume":"100","price_admitted":true,"volume_admitted":true}"#;
        let event = event_from_frame("00000000-0000-4000-8000-000000000001", 1, raw).unwrap();
        assert_eq!(event.provider, "LocalTerminal");
        assert_eq!(event.admission, v1::AdmissionState::Admitted as i32);
        assert_eq!(event.payload.unwrap().data, raw);
    }

    #[test]
    fn local_analysis_cannot_promote_its_own_admission() {
        let raw = br#"{"type":"analysis","instrument":"600396.SH","admitted":true}"#;
        let error = event_from_frame("00000000-0000-4000-8000-000000000001", 1, raw).unwrap_err();
        assert!(matches!(error, ClientError::UnadmittedAnalysisClaim));

        let raw = br#"{"type":"analysis","instrument":"600396.SH","admitted":false}"#;
        let event = event_from_frame("00000000-0000-4000-8000-000000000001", 1, raw).unwrap();
        assert_eq!(event.provider, "LocalAnalysis");
        assert_eq!(event.admission, v1::AdmissionState::Unadmitted as i32);
        assert_eq!(event.payload.unwrap().data, raw);
    }

    #[test]
    fn admitted_observation_requires_the_corresponding_value() {
        let raw = br#"{"type":"observation","instrument":"600396.SH","price":null,"cumulative_volume":null,"price_admitted":true,"volume_admitted":false}"#;
        let error = event_from_frame("00000000-0000-4000-8000-000000000001", 1, raw).unwrap_err();
        assert!(matches!(error, ClientError::AdmittedFieldMissing("price")));
    }

    #[test]
    fn monitor_generation_is_explicit_and_typed() {
        assert_eq!(frame_generation(br#"{"generation":2}"#).unwrap(), Some(2));
        assert_eq!(frame_generation(br#"{"type":"waiting"}"#).unwrap(), None);
        assert!(matches!(
            frame_generation(br#"{"generation":"2"}"#),
            Err(ClientError::InvalidMonitorGeneration)
        ));
    }

    #[test]
    fn watchlist_commands_are_bounded_monotonic_and_noncontradictory() {
        let client = client();
        let mut pending = None;
        client
            .absorb_configuration_value(
                v1::AgentConfigureWatchlist {
                    revision: 3,
                    instruments: vec!["EQUITY:SH:600519".to_owned(), "EQUITY:SZ:000001".to_owned()],
                },
                &mut pending,
            )
            .unwrap();
        assert_eq!(pending.as_ref().unwrap().revision, 3);

        let stale = client
            .absorb_configuration_value(
                v1::AgentConfigureWatchlist {
                    revision: 2,
                    instruments: vec!["EQUITY:SH:600396".to_owned()],
                },
                &mut pending,
            )
            .unwrap_err();
        assert!(matches!(stale, ClientError::StaleWatchlistRevision { .. }));

        let contradictory = client
            .absorb_configuration_value(
                v1::AgentConfigureWatchlist {
                    revision: 3,
                    instruments: vec!["EQUITY:SH:600396".to_owned()],
                },
                &mut pending,
            )
            .unwrap_err();
        assert!(matches!(
            contradictory,
            ClientError::WatchlistRevisionContradiction(3)
        ));
    }
}
