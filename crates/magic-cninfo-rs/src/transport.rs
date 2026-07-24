use crate::{CninfoError, MAX_RESPONSE_BYTES};
use std::io::Read;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub final_url: String,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub trait CninfoTransport: Send + Sync {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError>;
}

#[derive(Clone)]
pub(crate) struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    pub(crate) fn new(timeout: Duration) -> Result<Self, CninfoError> {
        if timeout.is_zero() || timeout > Duration::from_secs(60) {
            return Err(CninfoError::InvalidRequest(
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

    fn collect(response: ureq::Response) -> Result<HttpResponse, CninfoError> {
        let status = response.status();
        let final_url = response.get_url().to_owned();
        let content_type = response.header("Content-Type").map(str::to_owned);
        read_http_response(status, final_url, content_type, response.into_reader())
    }

    pub(crate) fn prepare(&self, request: &HttpRequest) -> ureq::Request {
        let mut wire = match request.method {
            HttpMethod::Get => self.agent.get(&request.url),
            HttpMethod::Post => self.agent.post(&request.url),
        };
        for (name, value) in &request.headers {
            wire = wire.set(name, value);
        }
        wire
    }
}

pub(crate) fn read_http_response(
    status: u16,
    final_url: String,
    content_type: Option<String>,
    reader: impl Read,
) -> Result<HttpResponse, CninfoError> {
    let mut body = Vec::new();
    reader
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|error| CninfoError::Transport(error.to_string()))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(CninfoError::Incomplete(format!(
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

pub(crate) fn collect_transport_result(
    result: Result<ureq::Response, ureq::Error>,
) -> Result<HttpResponse, CninfoError> {
    match result {
        Ok(response) => HttpsTransport::collect(response),
        Err(ureq::Error::Status(_, response)) => HttpsTransport::collect(response),
        Err(ureq::Error::Transport(error)) => Err(CninfoError::Transport(error.to_string())),
    }
}

impl CninfoTransport for HttpsTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, CninfoError> {
        let wire = self.prepare(request);
        let result = match request.method {
            HttpMethod::Get => wire.call(),
            HttpMethod::Post => wire.send_bytes(&request.body),
        };
        collect_transport_result(result)
    }
}
