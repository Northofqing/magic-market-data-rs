use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use magic_market_grpc_contracts::v1;
use magic_market_grpc_contracts::PROTOCOL_VERSION;
use prost::Message;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream, ReceiverStream};
use tokio_stream::Stream;
use tonic::{Request, Response, Status, Streaming};

type EventResult = Result<v1::MarketEventEnvelope, Status>;
type EventStream = Pin<Box<dyn Stream<Item = EventResult> + Send + 'static>>;
type CommandStream = ReceiverStream<Result<v1::AgentCommand, Status>>;

#[derive(Clone)]
pub(crate) struct EventHub {
    inner: Arc<Mutex<HubState>>,
    live: broadcast::Sender<v1::MarketEventEnvelope>,
    max_subscribers: usize,
    replay_max_events: usize,
    replay_max_bytes: usize,
    maximum_payload_bytes: usize,
    agent_command_capacity: usize,
}

struct HubState {
    generation: Option<String>,
    latest_sequence: u64,
    replay_bytes: usize,
    replay: VecDeque<v1::MarketEventEnvelope>,
    agent_session: Option<u64>,
    next_session: u64,
}

#[derive(Clone)]
struct EventFilter {
    instruments: Vec<String>,
    event_kinds: Vec<String>,
}

impl EventFilter {
    fn from_wire(filter: Option<v1::EventFilter>) -> Result<Self, Status> {
        let filter = filter.unwrap_or_default();
        if filter
            .instruments
            .iter()
            .any(|value| value.trim().is_empty())
            || filter
                .event_kinds
                .iter()
                .any(|value| value.trim().is_empty())
        {
            return Err(Status::invalid_argument(
                "event filter values must not be empty",
            ));
        }
        Ok(Self {
            instruments: filter.instruments,
            event_kinds: filter.event_kinds,
        })
    }

    fn matches(&self, event: &v1::MarketEventEnvelope) -> bool {
        (self.instruments.is_empty() || self.instruments.contains(&event.instrument))
            && (self.event_kinds.is_empty() || self.event_kinds.contains(&event.event_kind))
    }
}

struct SubscriberStream {
    inner: BroadcastStream<v1::MarketEventEnvelope>,
    filter: EventFilter,
    last_cursor: Option<v1::EventCursor>,
    terminated: bool,
}

impl Stream for SubscriberStream {
    type Item = EventResult;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.terminated {
            return Poll::Ready(None);
        }
        loop {
            match Pin::new(&mut self.inner).poll_next(context) {
                Poll::Ready(Some(Ok(event))) => {
                    if !self.filter.matches(&event) {
                        continue;
                    }
                    self.last_cursor = event.cursor.clone();
                    return Poll::Ready(Some(Ok(event)));
                }
                Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(skipped)))) => {
                    self.terminated = true;
                    let cursor = self
                        .last_cursor
                        .as_ref()
                        .map(|value| format!("{}:{}", value.generation, value.sequence))
                        .unwrap_or_else(|| "none".to_owned());
                    return Poll::Ready(Some(Err(Status::resource_exhausted(format!(
                        "subscriber queue lagged by {skipped} events; last_cursor={cursor}"
                    )))));
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl EventHub {
    pub(crate) fn new(
        max_subscribers: usize,
        subscriber_queue_capacity: usize,
        replay_max_events: usize,
        replay_max_bytes: usize,
        maximum_payload_bytes: usize,
        agent_command_capacity: usize,
    ) -> Result<Self, &'static str> {
        if max_subscribers == 0
            || subscriber_queue_capacity == 0
            || replay_max_events == 0
            || replay_max_bytes == 0
            || maximum_payload_bytes == 0
            || agent_command_capacity == 0
        {
            return Err("event hub limits must be positive");
        }
        let (live, _) = broadcast::channel(subscriber_queue_capacity);
        Ok(Self {
            inner: Arc::new(Mutex::new(HubState {
                generation: None,
                latest_sequence: 0,
                replay_bytes: 0,
                replay: VecDeque::new(),
                agent_session: None,
                next_session: 1,
            })),
            live,
            max_subscribers,
            replay_max_events,
            replay_max_bytes,
            maximum_payload_bytes,
            agent_command_capacity,
        })
    }

    async fn connect_agent(&self, hello: &v1::AgentHello) -> Result<(u64, u64), Status> {
        validate_hello(hello, self.maximum_payload_bytes)?;
        let mut state = self.inner.lock().await;
        if state.agent_session.is_some() {
            return Err(Status::already_exists(
                "a TDX agent stream is already active",
            ));
        }
        if state.generation.as_deref() != Some(&hello.terminal_generation) {
            state.generation = Some(hello.terminal_generation.clone());
            state.latest_sequence = 0;
            state.replay_bytes = 0;
            state.replay.clear();
        }
        let session = state.next_session;
        state.next_session = state
            .next_session
            .checked_add(1)
            .ok_or_else(|| Status::resource_exhausted("agent session identifier exhausted"))?;
        state.agent_session = Some(session);
        Ok((session, state.latest_sequence))
    }

    async fn disconnect_agent(&self, session: u64) {
        let mut state = self.inner.lock().await;
        if state.agent_session == Some(session) {
            state.agent_session = None;
        }
    }

    async fn heartbeat(
        &self,
        session: u64,
        generation: &str,
        last_sequence: u64,
    ) -> Result<u64, Status> {
        let state = self.inner.lock().await;
        if state.agent_session != Some(session) || state.generation.as_deref() != Some(generation) {
            return Err(Status::failed_precondition(
                "heartbeat does not match the active agent generation",
            ));
        }
        if last_sequence != state.latest_sequence {
            return Err(Status::failed_precondition(
                "heartbeat cursor does not equal the server cursor",
            ));
        }
        Ok(state.latest_sequence)
    }

    async fn publish(&self, session: u64, event: v1::MarketEventEnvelope) -> Result<u64, Status> {
        validate_event(&event, self.maximum_payload_bytes)?;
        let cursor = event
            .cursor
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("event cursor is required"))?;
        let sequence = cursor.sequence;
        let encoded_len = event.encoded_len();
        if encoded_len > self.replay_max_bytes {
            return Err(Status::resource_exhausted(
                "event exceeds the complete replay byte budget",
            ));
        }
        let mut state = self.inner.lock().await;
        if state.agent_session != Some(session) {
            return Err(Status::failed_precondition(
                "agent session is no longer active",
            ));
        }
        if state.generation.as_deref() != Some(cursor.generation.as_str()) {
            return Err(Status::failed_precondition(
                "event generation does not match agent hello",
            ));
        }
        let expected = state
            .latest_sequence
            .checked_add(1)
            .ok_or_else(|| Status::resource_exhausted("event sequence exhausted"))?;
        if cursor.sequence != expected {
            return Err(Status::failed_precondition(format!(
                "event sequence must be exactly {expected}"
            )));
        }
        state.latest_sequence = cursor.sequence;
        state.replay_bytes = state
            .replay_bytes
            .checked_add(encoded_len)
            .ok_or_else(|| Status::resource_exhausted("replay byte accounting overflow"))?;
        state.replay.push_back(event.clone());
        while state.replay.len() > self.replay_max_events
            || state.replay_bytes > self.replay_max_bytes
        {
            if let Some(removed) = state.replay.pop_front() {
                state.replay_bytes = state.replay_bytes.saturating_sub(removed.encoded_len());
            }
        }
        drop(state);
        let _ = self.live.send(event);
        Ok(sequence)
    }

    async fn subscribe_from(
        &self,
        after: Option<v1::EventCursor>,
        filter: EventFilter,
    ) -> Result<SubscriberStream, Status> {
        let state = self.inner.lock().await;
        if self.live.receiver_count() >= self.max_subscribers {
            return Err(Status::resource_exhausted(
                "maximum subscriber count reached",
            ));
        }
        validate_live_cursor(after.as_ref(), &state)?;
        let receiver = self.live.subscribe();
        drop(state);
        Ok(SubscriberStream {
            inner: BroadcastStream::new(receiver),
            filter,
            last_cursor: after,
            terminated: false,
        })
    }

    async fn replay_after(
        &self,
        after: Option<&v1::EventCursor>,
        filter: &EventFilter,
    ) -> Result<Vec<v1::MarketEventEnvelope>, Status> {
        let state = self.inner.lock().await;
        let generation = state
            .generation
            .as_deref()
            .ok_or_else(|| Status::failed_precondition("no TDX terminal generation is active"))?;
        let after_sequence = match after {
            Some(cursor) => {
                if cursor.generation != generation {
                    return Err(Status::failed_precondition(
                        "replay generation does not match",
                    ));
                }
                if cursor.sequence > state.latest_sequence {
                    return Err(Status::failed_precondition(
                        "replay cursor is ahead of the stream",
                    ));
                }
                if let Some(first) = state.replay.front().and_then(|event| event.cursor.as_ref()) {
                    if cursor.sequence.saturating_add(1) < first.sequence {
                        return Err(Status::out_of_range(
                            "replay cursor is older than retained data",
                        ));
                    }
                }
                cursor.sequence
            }
            None => 0,
        };
        Ok(state
            .replay
            .iter()
            .filter(|event| {
                event
                    .cursor
                    .as_ref()
                    .is_some_and(|cursor| cursor.sequence > after_sequence)
                    && filter.matches(event)
            })
            .cloned()
            .collect())
    }

    async fn listener_status(&self, request_id: String) -> v1::ListenerStatusResponse {
        let state = self.inner.lock().await;
        let generation = state.generation.clone().unwrap_or_default();
        let latest = state.generation.as_ref().map(|value| v1::EventCursor {
            generation: value.clone(),
            sequence: state.latest_sequence,
        });
        v1::ListenerStatusResponse {
            request_id,
            state: if state.agent_session.is_some() {
                "agent_connected_diagnostic".to_owned()
            } else if state.generation.is_some() {
                "agent_disconnected".to_owned()
            } else {
                "waiting_for_agent".to_owned()
            },
            terminal_generation: generation,
            latest,
            capabilities: Vec::new(),
        }
    }
}

fn validate_live_cursor(after: Option<&v1::EventCursor>, state: &HubState) -> Result<(), Status> {
    let Some(after) = after else {
        return Ok(());
    };
    let generation = state
        .generation
        .as_deref()
        .ok_or_else(|| Status::failed_precondition("no TDX terminal generation is active"))?;
    if after.generation != generation {
        return Err(Status::failed_precondition(
            "subscription generation does not match",
        ));
    }
    if after.sequence != state.latest_sequence {
        return Err(Status::failed_precondition(
            "subscription cursor must equal the current cursor; replay gaps first",
        ));
    }
    Ok(())
}

fn validate_hello(hello: &v1::AgentHello, maximum_payload_bytes: usize) -> Result<(), Status> {
    if hello.protocol_version != PROTOCOL_VERSION {
        return Err(Status::invalid_argument(
            "unsupported agent protocol version",
        ));
    }
    if hello.agent_id.trim().is_empty() || hello.terminal_generation.trim().is_empty() {
        return Err(Status::invalid_argument(
            "agent_id and terminal_generation are required",
        ));
    }
    let evidence = hello
        .terminal_evidence
        .as_ref()
        .ok_or_else(|| Status::invalid_argument("terminal evidence is required"))?;
    magic_market_grpc_contracts::validate_payload(evidence, maximum_payload_bytes)
        .map_err(|error| Status::invalid_argument(error.to_string()))
}

fn validate_event(
    event: &v1::MarketEventEnvelope,
    maximum_payload_bytes: usize,
) -> Result<(), Status> {
    if event.protocol_version != PROTOCOL_VERSION
        || event.event_id.trim().is_empty()
        || event.event_kind.trim().is_empty()
        || event.instrument.trim().is_empty()
        || event.observed_at.trim().is_empty()
    {
        return Err(Status::invalid_argument(
            "event identity fields are invalid",
        ));
    }
    if event.provider != "LocalTerminal" && event.provider != "LocalAnalysis" {
        return Err(Status::permission_denied(
            "TDX agent provider is outside its boundary",
        ));
    }
    if event.admission != v1::AdmissionState::Unadmitted as i32 {
        return Err(Status::failed_precondition(
            "TDX event families remain repository-unadmitted",
        ));
    }
    let cursor = event
        .cursor
        .as_ref()
        .ok_or_else(|| Status::invalid_argument("event cursor is required"))?;
    if cursor.generation.trim().is_empty() || cursor.sequence == 0 {
        return Err(Status::invalid_argument("event cursor is invalid"));
    }
    let payload = event
        .payload
        .as_ref()
        .ok_or_else(|| Status::invalid_argument("event payload is required"))?;
    magic_market_grpc_contracts::validate_payload(payload, maximum_payload_bytes)
        .map_err(|error| Status::invalid_argument(error.to_string()))
}

fn validate_context(context: Option<&v1::RequestContext>) -> Result<&v1::RequestContext, Status> {
    let context = context.ok_or_else(|| Status::invalid_argument("request context is required"))?;
    if context.protocol_version != PROTOCOL_VERSION || context.request_id.trim().is_empty() {
        return Err(Status::invalid_argument("request context is invalid"));
    }
    Ok(context)
}

#[tonic::async_trait]
impl v1::market_event_service_server::MarketEventService for EventHub {
    type SubscribeStream = EventStream;
    type ReplayStream = EventStream;

    async fn subscribe(
        &self,
        request: Request<v1::SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let request = request.into_inner();
        validate_context(request.context.as_ref())?;
        let filter = EventFilter::from_wire(request.filter)?;
        let stream = self.subscribe_from(request.after, filter).await?;
        Ok(Response::new(Box::pin(stream)))
    }

    async fn replay(
        &self,
        request: Request<v1::ReplayRequest>,
    ) -> Result<Response<Self::ReplayStream>, Status> {
        let request = request.into_inner();
        validate_context(request.context.as_ref())?;
        let filter = EventFilter::from_wire(request.filter)?;
        let events = self.replay_after(request.after.as_ref(), &filter).await?;
        Ok(Response::new(Box::pin(tokio_stream::iter(
            events.into_iter().map(Ok),
        ))))
    }

    async fn get_listener_status(
        &self,
        request: Request<v1::ListenerStatusRequest>,
    ) -> Result<Response<v1::ListenerStatusResponse>, Status> {
        let request = request.into_inner();
        let context = validate_context(request.context.as_ref())?;
        Ok(Response::new(
            self.listener_status(context.request_id.clone()).await,
        ))
    }
}

#[tonic::async_trait]
impl v1::tdx_agent_service_server::TdxAgentService for EventHub {
    type OpenStreamStream = CommandStream;

    async fn open_stream(
        &self,
        request: Request<Streaming<v1::AgentMessage>>,
    ) -> Result<Response<Self::OpenStreamStream>, Status> {
        let mut incoming = request.into_inner();
        let first = incoming
            .message()
            .await?
            .ok_or_else(|| Status::invalid_argument("agent hello is required"))?;
        let hello = match first.body {
            Some(v1::agent_message::Body::Hello(hello)) => hello,
            _ => {
                return Err(Status::invalid_argument(
                    "first agent message must be hello",
                ))
            }
        };
        let (session, accepted_sequence) = self.connect_agent(&hello).await?;
        let generation = hello.terminal_generation;
        let (commands, receiver) = mpsc::channel(self.agent_command_capacity);
        commands
            .try_send(Ok(v1::AgentCommand {
                body: Some(v1::agent_command::Body::Ack(v1::AgentAck {
                    terminal_generation: generation.clone(),
                    accepted_sequence,
                })),
            }))
            .map_err(|_| Status::resource_exhausted("agent command queue is unavailable"))?;
        let hub = self.clone();
        tokio::spawn(async move {
            while let Ok(Some(message)) = incoming.message().await {
                let outcome = match message.body {
                    Some(v1::agent_message::Body::Event(event)) => hub
                        .publish(session, event)
                        .await
                        .map(|sequence| v1::AgentCommand {
                            body: Some(v1::agent_command::Body::Ack(v1::AgentAck {
                                terminal_generation: generation.clone(),
                                accepted_sequence: sequence,
                            })),
                        }),
                    Some(v1::agent_message::Body::Heartbeat(heartbeat)) => hub
                        .heartbeat(
                            session,
                            &heartbeat.terminal_generation,
                            heartbeat.last_sequence,
                        )
                        .await
                        .map(|sequence| v1::AgentCommand {
                            body: Some(v1::agent_command::Body::Ack(v1::AgentAck {
                                terminal_generation: generation.clone(),
                                accepted_sequence: sequence,
                            })),
                        }),
                    _ => Err(Status::invalid_argument("agent stream message is invalid")),
                };
                match outcome {
                    Ok(command) => {
                        if commands.try_send(Ok(command)).is_err() {
                            break;
                        }
                    }
                    Err(status) => {
                        let _ = commands.try_send(Ok(v1::AgentCommand {
                            body: Some(v1::agent_command::Body::Stop(v1::AgentStop {
                                reason_code: status.message().to_owned(),
                            })),
                        }));
                        break;
                    }
                }
            }
            hub.disconnect_agent(session).await;
        });
        Ok(Response::new(ReceiverStream::new(receiver)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magic_market_grpc_contracts::CANONICAL_JSON_CONTENT_TYPE;
    use tokio_stream::wrappers::TcpListenerStream;
    use tokio_stream::StreamExt;

    fn hub(queue: usize, replay_events: usize) -> EventHub {
        EventHub::new(2, queue, replay_events, 32_768, 4096, 4).unwrap()
    }

    fn payload(schema: &str) -> v1::CanonicalPayload {
        v1::CanonicalPayload {
            schema: schema.to_owned(),
            schema_version: 1,
            content_type: CANONICAL_JSON_CONTENT_TYPE.to_owned(),
            data: b"{}".to_vec(),
        }
    }

    fn hello(generation: &str) -> v1::AgentHello {
        v1::AgentHello {
            protocol_version: PROTOCOL_VERSION,
            agent_id: "agent-1".to_owned(),
            terminal_generation: generation.to_owned(),
            terminal_evidence: Some(payload("magic.tdx.terminal_evidence")),
        }
    }

    fn event(generation: &str, sequence: u64) -> v1::MarketEventEnvelope {
        v1::MarketEventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            event_id: format!("event-{sequence}"),
            cursor: Some(v1::EventCursor {
                generation: generation.to_owned(),
                sequence,
            }),
            event_kind: "price_observation".to_owned(),
            provider: "LocalTerminal".to_owned(),
            instrument: "600396.SH".to_owned(),
            observed_at: "2026-08-13T00:00:00Z".to_owned(),
            source_at: String::new(),
            admission: v1::AdmissionState::Unadmitted as i32,
            payload: Some(payload("magic.tdx.observation")),
        }
    }

    #[tokio::test]
    async fn sequence_gap_is_rejected_without_advancing_replay() {
        let hub = hub(2, 4);
        let (session, _) = hub.connect_agent(&hello("generation-a")).await.unwrap();
        assert_eq!(
            hub.publish(session, event("generation-a", 1))
                .await
                .unwrap(),
            1
        );
        let status = hub
            .publish(session, event("generation-a", 3))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
        let replay = hub
            .replay_after(None, &EventFilter::from_wire(None).unwrap())
            .await
            .unwrap();
        assert_eq!(replay.len(), 1);
    }

    #[tokio::test]
    async fn replay_reports_an_expired_cursor() {
        let hub = hub(2, 2);
        let (session, _) = hub.connect_agent(&hello("generation-a")).await.unwrap();
        for sequence in 1..=3 {
            hub.publish(session, event("generation-a", sequence))
                .await
                .unwrap();
        }
        let status = hub
            .replay_after(
                Some(&v1::EventCursor {
                    generation: "generation-a".to_owned(),
                    sequence: 0,
                }),
                &EventFilter::from_wire(None).unwrap(),
            )
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::OutOfRange);
    }

    #[tokio::test]
    async fn slow_subscriber_gets_resource_exhausted_and_terminates() {
        let hub = hub(1, 4);
        let (session, _) = hub.connect_agent(&hello("generation-a")).await.unwrap();
        let mut stream = hub
            .subscribe_from(None, EventFilter::from_wire(None).unwrap())
            .await
            .unwrap();
        hub.publish(session, event("generation-a", 1))
            .await
            .unwrap();
        hub.publish(session, event("generation-a", 2))
            .await
            .unwrap();
        let status = stream.next().await.unwrap().unwrap_err();
        assert_eq!(status.code(), tonic::Code::ResourceExhausted);
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn admitted_tdx_event_is_rejected() {
        let hub = hub(2, 4);
        let (session, _) = hub.connect_agent(&hello("generation-a")).await.unwrap();
        let mut value = event("generation-a", 1);
        value.admission = v1::AdmissionState::Admitted as i32;
        let status = hub.publish(session, value).await.unwrap_err();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn grpc_transport_delivers_agent_event_to_subscriber() {
        let hub = hub(4, 4);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, shutdown_receiver) = tokio::sync::oneshot::channel();
        let server_hub = hub.clone();
        let server = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    v1::market_event_service_server::MarketEventServiceServer::new(
                        server_hub.clone(),
                    ),
                )
                .add_service(v1::tdx_agent_service_server::TdxAgentServiceServer::new(
                    server_hub,
                ))
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
                    let _ = shutdown_receiver.await;
                })
                .await
                .unwrap();
        });
        let endpoint = format!("http://{address}");
        let mut subscriber =
            v1::market_event_service_client::MarketEventServiceClient::connect(endpoint.clone())
                .await
                .unwrap();
        let mut events = subscriber
            .subscribe(v1::SubscribeRequest {
                context: Some(v1::RequestContext {
                    protocol_version: PROTOCOL_VERSION,
                    request_id: "subscribe-1".to_owned(),
                }),
                filter: None,
                after: None,
            })
            .await
            .unwrap()
            .into_inner();
        let mut agent = v1::tdx_agent_service_client::TdxAgentServiceClient::connect(endpoint)
            .await
            .unwrap();
        let (messages, receiver) = mpsc::channel(2);
        messages
            .send(v1::AgentMessage {
                body: Some(v1::agent_message::Body::Hello(hello("generation-a"))),
            })
            .await
            .unwrap();
        let mut commands = agent
            .open_stream(ReceiverStream::new(receiver))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            commands.message().await.unwrap().unwrap().body,
            Some(v1::agent_command::Body::Ack(v1::AgentAck {
                terminal_generation: "generation-a".to_owned(),
                accepted_sequence: 0,
            }))
        );
        messages
            .send(v1::AgentMessage {
                body: Some(v1::agent_message::Body::Event(event("generation-a", 1))),
            })
            .await
            .unwrap();
        let delivered = events.message().await.unwrap().unwrap();
        assert_eq!(delivered.event_id, "event-1");
        let acknowledgement = commands.message().await.unwrap().unwrap();
        assert!(matches!(
            acknowledgement.body,
            Some(v1::agent_command::Body::Ack(v1::AgentAck {
                accepted_sequence: 1,
                ..
            }))
        ));
        drop(messages);
        drop(events);
        drop(commands);
        drop(subscriber);
        drop(agent);
        let _ = shutdown.send(());
        tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
    }
}
