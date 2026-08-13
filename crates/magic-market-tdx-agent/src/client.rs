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
    reconnect_delay: Duration,
    queue_capacity: usize,
    generation_prefix: String,
}

struct PendingFrame {
    raw: Vec<u8>,
    sequence: u64,
}

impl AgentClient {
    pub(crate) fn new(config: &AgentConfig, generation: String) -> Result<Self, ClientError> {
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
            reconnect_delay: config.reconnect_delay,
            queue_capacity: config.queue_capacity,
            generation_prefix: generation,
        })
    }

    pub(crate) async fn forward(
        &self,
        mut frames: mpsc::Receiver<Vec<u8>>,
    ) -> Result<(), ClientError> {
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
                Ok(SessionEnd::FramesComplete) => return Ok(()),
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
        let initial = receive_command(&mut commands, self.command_timeout).await?;
        let server_sequence = ack_sequence(initial, generation)?;
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
                        return match command? {
                            Some(command) => Err(ClientError::UnexpectedCommand(command)),
                            None => Ok(SessionEnd::Reconnect),
                        };
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
            let command = receive_command(&mut commands, self.command_timeout).await?;
            let sequence = ack_sequence(command, generation)?;
            if sequence != frame.sequence {
                return Err(ClientError::SequenceContradiction {
                    local: frame.sequence,
                    server: sequence,
                });
            }
            *accepted_sequence = sequence;
            *pending = None;
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

fn ack_sequence(command: v1::AgentCommand, generation: &str) -> Result<u64, ClientError> {
    match command.body {
        Some(v1::agent_command::Body::Ack(ack)) if ack.terminal_generation == generation => {
            Ok(ack.accepted_sequence)
        }
        Some(v1::agent_command::Body::Stop(stop)) => Err(ClientError::Stopped(stop.reason_code)),
        _ => Err(ClientError::InvalidAcknowledgement),
    }
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
        admission: v1::AdmissionState::Unadmitted as i32,
        payload: Some(v1::CanonicalPayload {
            schema: format!("magic.market.monitor.{event_kind}"),
            schema_version: 1,
            content_type: CANONICAL_JSON_CONTENT_TYPE.to_owned(),
            data: raw.to_vec(),
        }),
    })
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
    #[error("server returned an unexpected command: {0:?}")]
    UnexpectedCommand(v1::AgentCommand),
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
    #[error("monitor frame generation is not an unsigned integer")]
    InvalidMonitorGeneration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_mapping_preserves_diagnostic_admission_and_raw_payload() {
        let raw = br#"{"type":"analysis","instrument":"600396.SH","admitted":true}"#;
        let event = event_from_frame("00000000-0000-4000-8000-000000000001", 1, raw).unwrap();
        assert_eq!(event.provider, "LocalAnalysis");
        assert_eq!(event.admission, v1::AdmissionState::Unadmitted as i32);
        assert_eq!(event.payload.unwrap().data, raw);
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
}
