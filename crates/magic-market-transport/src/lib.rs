#![forbid(unsafe_code)]

mod gate;
mod http;

pub use gate::RequestGate;
pub use http::{
    EndpointPolicy, HttpMethod, HttpRequest, HttpResponse, HttpTransport, MediaType,
    ReqwestTransport,
};

use thiserror::Error;

/// Failures raised while validating or executing a bounded HTTP request.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("invalid transport request: {0}")]
    InvalidRequest(String),
    #[error("authentication failed: {0}")]
    Authentication(String),
    #[error("HTTP transport failed: {0}")]
    Network(String),
    #[error("HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("redirect or final URL rejected: {0}")]
    Redirect(String),
    #[error("response media type rejected: {0}")]
    MediaType(String),
    #[error("transport resource limit: {0}")]
    ResourceLimit(String),
    #[error("transport internal failure: {0}")]
    Internal(String),
}
