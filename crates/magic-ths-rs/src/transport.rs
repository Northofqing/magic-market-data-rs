use crate::{ThsError, MAX_RESPONSE_BYTES};
use std::io::Read;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub final_url: String,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub trait ThsTransport: Send + Sync {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ThsError>;
}

#[derive(Clone)]
pub(crate) struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    pub(crate) fn new(timeout: Duration) -> Result<Self, ThsError> {
        if timeout.is_zero() || timeout > Duration::from_secs(60) {
            return Err(ThsError::InvalidRequest(
                "timeout must be between 1ns and 60s".into(),
            ));
        }
        Ok(Self {
            agent: ureq::AgentBuilder::new()
                .timeout_connect(timeout)
                .timeout_read(timeout)
                .timeout_write(timeout)
                .redirects(0)
                .build(),
        })
    }

    fn collect(response: ureq::Response) -> Result<HttpResponse, ThsError> {
        let status = response.status();
        let final_url = response.get_url().to_owned();
        let content_type = response.header("Content-Type").map(str::to_owned);
        read_http_response(status, final_url, content_type, response.into_reader())
    }
}

pub(crate) fn collect_transport_result(
    result: Result<ureq::Response, ureq::Error>,
) -> Result<HttpResponse, ThsError> {
    match result {
        Ok(response) => HttpsTransport::collect(response),
        Err(ureq::Error::Status(_, response)) => HttpsTransport::collect(response),
        Err(ureq::Error::Transport(error)) => Err(ThsError::Transport(error.to_string())),
    }
}

pub(crate) fn read_http_response(
    status: u16,
    final_url: String,
    content_type: Option<String>,
    reader: impl Read,
) -> Result<HttpResponse, ThsError> {
    let mut body = Vec::new();
    reader
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|error| ThsError::Transport(error.to_string()))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(ThsError::Incomplete(format!(
            "response body exceeds {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    Ok(HttpResponse {
        status,
        final_url,
        content_type,
        body,
    })
}

impl ThsTransport for HttpsTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ThsError> {
        let mut wire = match request.method {
            HttpMethod::Get => self.agent.get(&request.url),
        };
        for (name, value) in &request.headers {
            wire = wire.set(name, value);
        }
        collect_transport_result(wire.call())
    }
}
