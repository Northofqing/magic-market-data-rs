#![forbid(unsafe_code)]

mod app;
mod auth;
mod config;
mod events;

use std::sync::Arc;

use app::GrpcApplication;
use auth::BearerAuth;
use config::ServerConfig;
use events::EventHub;
use magic_market_grpc_contracts::v1;
use magic_market_service::OperationRegistry;
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::parse(std::env::args())?;
    let authentication = BearerAuth::new(&config.auth_token)?;
    let gateway = Arc::new(OperationRegistry::all_unadmitted(
        "production provider composition is not registered",
    ));
    let application = GrpcApplication::new(
        gateway,
        config.max_payload_bytes,
        config.unary_concurrency,
        config.blocking_concurrency,
        config.blocking_deadline,
    )?;
    let event_hub = EventHub::new(
        config.max_subscribers,
        config.subscriber_queue_capacity,
        config.replay_max_events,
        config.replay_max_bytes,
        config.max_payload_bytes,
        config.agent_command_capacity,
    )?;

    let system = v1::system_service_server::SystemServiceServer::new(application.clone())
        .max_decoding_message_size(config.max_decoding_bytes)
        .max_encoding_message_size(config.max_encoding_bytes);
    let system =
        tonic::service::interceptor::InterceptedService::new(system, authentication.clone());
    let market = v1::market_data_service_server::MarketDataServiceServer::new(application)
        .max_decoding_message_size(config.max_decoding_bytes)
        .max_encoding_message_size(config.max_encoding_bytes);
    let market = tonic::service::interceptor::InterceptedService::new(market, authentication);
    let events = v1::market_event_service_server::MarketEventServiceServer::new(event_hub.clone())
        .max_decoding_message_size(config.max_decoding_bytes)
        .max_encoding_message_size(config.max_encoding_bytes);
    let events = tonic::service::interceptor::InterceptedService::new(
        events,
        BearerAuth::new(&config.auth_token)?,
    );
    let agent = v1::tdx_agent_service_server::TdxAgentServiceServer::new(event_hub)
        .max_decoding_message_size(config.max_decoding_bytes)
        .max_encoding_message_size(config.max_encoding_bytes);
    let agent = tonic::service::interceptor::InterceptedService::new(
        agent,
        BearerAuth::new(&config.auth_token)?,
    );

    let (_health_reporter, health) = tonic_health::server::health_reporter();
    let reflection = if config.reflection {
        Some(
            tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(v1::FILE_DESCRIPTOR_SET)
                .build_v1()?,
        )
    } else {
        None
    };

    let mut builder = Server::builder();
    if let Some(tls) = config.tls {
        let certificate = std::fs::read(tls.certificate)?;
        let private_key = std::fs::read(tls.private_key)?;
        let mut tls_config =
            ServerTlsConfig::new().identity(Identity::from_pem(certificate, private_key));
        if let Some(client_ca) = tls.client_ca {
            tls_config =
                tls_config.client_ca_root(Certificate::from_pem(std::fs::read(client_ca)?));
        }
        builder = builder.tls_config(tls_config)?;
    }

    let shutdown_timeout = config.shutdown_timeout;
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();
    let serving = builder
        .add_service(health)
        .add_service(system)
        .add_service(market)
        .add_service(events)
        .add_service(agent)
        .add_optional_service(reflection)
        .serve_with_shutdown(config.bind, async move {
            let _ = shutdown_receiver.await;
        });
    tokio::pin!(serving);
    tokio::select! {
        result = &mut serving => result?,
        signal = tokio::signal::ctrl_c() => {
            signal?;
            let _ = shutdown_sender.send(());
            tokio::time::timeout(shutdown_timeout, &mut serving)
                .await
                .map_err(|_| std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "gRPC graceful shutdown deadline exceeded",
                ))??;
        }
    }
    Ok(())
}
