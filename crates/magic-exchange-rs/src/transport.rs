use crate::ExchangeError;
use magic_market_transport::{
    EndpointPolicy, HttpMethod as SharedHttpMethod, HttpRequest as SharedHttpRequest, MediaType,
    RequestGate, TransportError,
};
use std::io::Read;
use std::str::FromStr;
#[cfg(feature = "native-tls")]
use std::sync::Arc;
use std::time::Duration;
use url::Url;

pub(crate) use magic_market_transport::MediaType as SharedMediaType;
pub(crate) use magic_market_transport::RequestGate as SharedRequestGate;

pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 64 * 1024;

/// Explicit TLS backend for official exchange HTTPS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsBackend {
    Rustls,
    NativeTls,
}

impl TlsBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rustls => "rustls",
            Self::NativeTls => "native-tls",
        }
    }
}

impl FromStr for TlsBackend {
    type Err = ExchangeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "rustls" => Ok(Self::Rustls),
            "native-tls" | "native_tls" => Ok(Self::NativeTls),
            other => Err(ExchangeError::InvalidRequest(format!(
                "unsupported TLS backend {other:?}; expected rustls or native-tls"
            ))),
        }
    }
}

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

pub trait ExchangeTransport: Send + Sync {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError>;
}

#[derive(Clone)]
pub(crate) struct HttpsTransport {
    agent: ureq::Agent,
    tls_backend: TlsBackend,
}

impl HttpsTransport {
    pub(crate) fn new(timeout: Duration) -> Result<Self, ExchangeError> {
        Self::with_tls_backend(timeout, TlsBackend::Rustls)
    }

    pub(crate) fn with_tls_backend(
        timeout: Duration,
        tls_backend: TlsBackend,
    ) -> Result<Self, ExchangeError> {
        validate_timeout(timeout)?;
        let builder = ureq::AgentBuilder::new()
            .timeout_connect(timeout)
            .timeout_read(timeout)
            .timeout_write(timeout)
            .redirects(0);
        let agent = match tls_backend {
            TlsBackend::Rustls => builder.build(),
            TlsBackend::NativeTls => {
                #[cfg(feature = "native-tls")]
                {
                    let connector = ureq::native_tls::TlsConnector::new().map_err(|error| {
                        ExchangeError::Tls {
                            backend: tls_backend,
                            message: error.to_string(),
                        }
                    })?;
                    builder.tls_connector(Arc::new(connector)).build()
                }
                #[cfg(not(feature = "native-tls"))]
                {
                    return Err(ExchangeError::Unsupported(
                        "native-tls backend is not compiled; enable magic-exchange-rs feature native-tls"
                            .into(),
                    ));
                }
            }
        };
        Ok(Self { agent, tls_backend })
    }

    pub(crate) fn collect(response: ureq::Response) -> Result<HttpResponse, ExchangeError> {
        let status = response.status();
        let final_url = response.get_url().to_owned();
        let content_type = response.header("Content-Type").map(str::to_owned);
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|error| ExchangeError::Transport(error.to_string()))?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(ExchangeError::Incomplete(format!(
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
}

impl ExchangeTransport for HttpsTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        let mut wire = match request.method {
            HttpMethod::Get => self.agent.get(&request.url),
            HttpMethod::Post => self.agent.post(&request.url),
        };
        for (name, value) in &request.headers {
            wire = wire.set(name, value);
        }
        let result = match request.method {
            HttpMethod::Get => wire.call(),
            HttpMethod::Post => wire.send_bytes(&request.body),
        };
        match result {
            Ok(response) => Self::collect(response),
            Err(ureq::Error::Status(_, response)) => Self::collect(response),
            Err(ureq::Error::Transport(error)) => {
                Err(classify_transport_error(self.tls_backend, error))
            }
        }
    }
}

fn classify_transport_error(backend: TlsBackend, error: ureq::Transport) -> ExchangeError {
    let message = error.to_string();
    let lowercase = message.to_ascii_lowercase();
    if ["tls", "ssl", "handshake", "certificate"]
        .iter()
        .any(|marker| lowercase.contains(marker))
    {
        ExchangeError::Tls { backend, message }
    } else {
        ExchangeError::Transport(message)
    }
}

pub(crate) fn new_request_gate(interval: Duration) -> Result<RequestGate, ExchangeError> {
    RequestGate::new(interval).map_err(map_shared_error)
}

pub(crate) fn wait_for_request_start(gate: &RequestGate) -> Result<(), ExchangeError> {
    gate.wait_for_turn().map_err(map_shared_error)
}

pub(crate) fn map_shared_error(error: TransportError) -> ExchangeError {
    match error {
        TransportError::InvalidRequest(message) => ExchangeError::InvalidRequest(message),
        TransportError::Authentication(message) => ExchangeError::InvalidRequest(message),
        TransportError::Network(message) | TransportError::Internal(message) => {
            ExchangeError::Transport(message)
        }
        TransportError::HttpStatus { status } if matches!(status, 401 | 403) => {
            ExchangeError::Authentication(status)
        }
        TransportError::HttpStatus { status: 429 } => ExchangeError::RateLimited,
        TransportError::HttpStatus { status } => ExchangeError::HttpStatus(status),
        TransportError::Redirect(message) | TransportError::MediaType(message) => {
            ExchangeError::Schema(message)
        }
        TransportError::ResourceLimit(message) => ExchangeError::Incomplete(message),
    }
}

pub(crate) fn validate_timeout(timeout: Duration) -> Result<(), ExchangeError> {
    if timeout.is_zero() || timeout > Duration::from_secs(60) {
        return Err(ExchangeError::InvalidRequest(
            "timeout must be positive and at most 60 seconds".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_minimum_interval(interval: Duration) -> Result<(), ExchangeError> {
    if interval < Duration::from_secs(1) {
        return Err(ExchangeError::InvalidRequest(
            "minimum request interval must be at least one second".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_endpoint(
    value: &str,
    expected_host: &str,
    expected_path: &str,
) -> Result<(), ExchangeError> {
    let policy = EndpointPolicy::new(
        expected_host,
        vec![expected_path.to_owned()],
        Vec::new(),
        vec![MediaType::Json],
        MAX_RESPONSE_BYTES,
        Duration::from_secs(15),
    )
    .map_err(map_shared_error)?;
    let shared_request =
        SharedHttpRequest::new(SharedHttpMethod::Get, value, Vec::new(), Vec::new())
            .map_err(map_shared_error)?;
    policy
        .validate_request(&shared_request)
        .map_err(map_shared_error)?;
    let parsed =
        Url::parse(value).map_err(|error| ExchangeError::InvalidRequest(error.to_string()))?;
    if parsed.scheme() != "https"
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.port_or_known_default() != Some(443)
        || parsed.host_str() != Some(expected_host)
        || parsed.path() != expected_path
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(ExchangeError::InvalidRequest(format!(
            "endpoint must be exact credential-free HTTPS https://{expected_host}{expected_path}"
        )));
    }
    Ok(())
}

pub(crate) fn validate_request(
    request: &HttpRequest,
    method: HttpMethod,
    expected_host: &str,
    expected_path: &str,
    allowed_query_keys: &[&str],
    allowed_media_types: &[MediaType],
    timeout: Duration,
) -> Result<EndpointPolicy, ExchangeError> {
    let policy = EndpointPolicy::new(
        expected_host,
        vec![expected_path.to_owned()],
        allowed_query_keys
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        allowed_media_types.to_vec(),
        MAX_RESPONSE_BYTES,
        timeout,
    )
    .map_err(map_shared_error)?;
    let shared_method = match request.method {
        HttpMethod::Get => SharedHttpMethod::Get,
        HttpMethod::Post => SharedHttpMethod::Post,
    };
    let shared_request = SharedHttpRequest::new(
        shared_method,
        request.url.clone(),
        request.headers.clone(),
        request.body.clone(),
    )
    .map_err(map_shared_error)?;
    policy
        .validate_request(&shared_request)
        .map_err(map_shared_error)?;
    let parsed = Url::parse(&request.url)
        .map_err(|error| ExchangeError::InvalidRequest(error.to_string()))?;
    if request.method != method
        || parsed.scheme() != "https"
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.port_or_known_default() != Some(443)
        || parsed.host_str() != Some(expected_host)
        || parsed.path() != expected_path
        || parsed.fragment().is_some()
    {
        return Err(ExchangeError::InvalidRequest(format!(
            "request must use the allowlisted {method:?} endpoint {expected_host}{expected_path}"
        )));
    }
    if request.body.len() > MAX_REQUEST_BYTES {
        return Err(ExchangeError::InvalidRequest(format!(
            "request body exceeds {MAX_REQUEST_BYTES} bytes"
        )));
    }
    Ok(policy)
}

pub(crate) fn validate_response(
    policy: &EndpointPolicy,
    request: &HttpRequest,
    response: &HttpResponse,
) -> Result<(), ExchangeError> {
    let shared_method = match request.method {
        HttpMethod::Get => SharedHttpMethod::Get,
        HttpMethod::Post => SharedHttpMethod::Post,
    };
    let shared_request = SharedHttpRequest::new(
        shared_method,
        request.url.clone(),
        request.headers.clone(),
        request.body.clone(),
    )
    .map_err(map_shared_error)?;
    let shared_response = magic_market_transport::HttpResponse::new(
        response.status,
        response.final_url.clone(),
        response.content_type.clone(),
        response.body.clone(),
    );
    policy
        .validate_response_for(&shared_request, shared_response)
        .map_err(map_shared_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use std::time::Instant;

    #[test]
    fn endpoint_allowlist_is_https_host_and_path_exact() {
        assert!(validate_endpoint(
            "https://query.sse.com.cn/security/stock/queryCompanyBulletin.do",
            "query.sse.com.cn",
            "/security/stock/queryCompanyBulletin.do"
        )
        .is_ok());
        for invalid in [
            "http://query.sse.com.cn/security/stock/queryCompanyBulletin.do",
            "https://query.sse.com.cn.evil.invalid/security/stock/queryCompanyBulletin.do",
            "https://query.sse.com.cn/security/stock/other.do",
            "https://user@query.sse.com.cn/security/stock/queryCompanyBulletin.do",
            "https://query.sse.com.cn:444/security/stock/queryCompanyBulletin.do",
            "https://query.sse.com.cn/security/stock/queryCompanyBulletin.do?x=1",
        ] {
            assert!(validate_endpoint(
                invalid,
                "query.sse.com.cn",
                "/security/stock/queryCompanyBulletin.do"
            )
            .is_err());
        }
    }

    #[test]
    fn response_rejects_redirect_content_type_and_size() {
        let request = HttpRequest {
            method: HttpMethod::Get,
            url: "https://query.sse.com.cn/security/stock/queryCompanyBulletin.do?x=1".into(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let valid = HttpResponse {
            status: 200,
            final_url: request.url.clone(),
            content_type: Some("application/json;charset=UTF-8".into()),
            body: b"cb({})".to_vec(),
        };
        let policy = validate_request(
            &request,
            HttpMethod::Get,
            "query.sse.com.cn",
            "/security/stock/queryCompanyBulletin.do",
            &["x"],
            &[SharedMediaType::Json],
            Duration::from_secs(15),
        )
        .unwrap();
        assert!(validate_response(&policy, &request, &valid).is_ok());
        let mut redirected = valid.clone();
        redirected.final_url =
            "https://query.sse.com.cn/security/stock/queryCompanyBulletin.do?x=2".into();
        assert!(validate_response(&policy, &request, &redirected).is_err());
        let mut html = valid.clone();
        html.content_type = Some("text/html".into());
        assert!(validate_response(&policy, &request, &html).is_err());
        let mut fake_javascript = valid.clone();
        fake_javascript.content_type = Some("text/notjavascript".into());
        assert!(validate_response(&policy, &request, &fake_javascript).is_err());
        let mut oversized = valid;
        oversized.body = vec![0; MAX_RESPONSE_BYTES + 1];
        assert!(validate_response(&policy, &request, &oversized).is_err());
    }

    #[test]
    fn shared_request_contract_rejects_credentials_before_transport() {
        let request = HttpRequest {
            method: HttpMethod::Get,
            url: "https://query.sse.com.cn/security/stock/queryCompanyBulletin.do".into(),
            headers: vec![("Cookie".into(), "session=secret".into())],
            body: Vec::new(),
        };
        assert!(validate_request(
            &request,
            HttpMethod::Get,
            "query.sse.com.cn",
            "/security/stock/queryCompanyBulletin.do",
            &[],
            &[SharedMediaType::Json],
            Duration::from_secs(15),
        )
        .is_err());
    }

    #[test]
    fn shared_request_contract_rejects_unknown_query_key_before_transport() {
        let request = HttpRequest {
            method: HttpMethod::Get,
            url: "https://query.sse.com.cn/security/stock/queryCompanyBulletin.do?unexpected=1"
                .into(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        assert!(validate_request(
            &request,
            HttpMethod::Get,
            "query.sse.com.cn",
            "/security/stock/queryCompanyBulletin.do",
            &["productId"],
            &[SharedMediaType::Json],
            Duration::from_secs(15),
        )
        .is_err());
    }

    #[test]
    fn timeout_bounds_are_strict() {
        assert!(validate_timeout(Duration::ZERO).is_err());
        assert!(validate_timeout(Duration::from_secs(15)).is_ok());
        assert!(validate_timeout(Duration::from_secs(61)).is_err());
    }

    #[test]
    fn backend_names_are_stable_for_operator_evidence() {
        assert_eq!(TlsBackend::Rustls.as_str(), "rustls");
        assert_eq!(TlsBackend::NativeTls.as_str(), "native-tls");
        assert_eq!("rustls".parse::<TlsBackend>().unwrap(), TlsBackend::Rustls);
        assert_eq!(
            "native-tls".parse::<TlsBackend>().unwrap(),
            TlsBackend::NativeTls
        );
        assert!("automatic".parse::<TlsBackend>().is_err());
    }

    #[test]
    fn clone_shared_gate_spaces_starts_without_serializing_io() {
        let gate = Arc::new(new_request_gate(Duration::from_millis(30)).unwrap());
        let starts = Arc::new(Mutex::new(Vec::new()));
        let barrier = Arc::new(Barrier::new(3));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum_active = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let gate = Arc::clone(&gate);
            let starts = Arc::clone(&starts);
            let barrier = Arc::clone(&barrier);
            let active = Arc::clone(&active);
            let maximum_active = Arc::clone(&maximum_active);
            threads.push(thread::spawn(move || {
                barrier.wait();
                wait_for_request_start(&gate).unwrap();
                starts.lock().unwrap().push(Instant::now());
                let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum_active.fetch_max(now_active, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(80));
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        barrier.wait();
        for thread in threads {
            thread.join().unwrap();
        }
        let mut starts = starts.lock().unwrap().clone();
        starts.sort_unstable();
        assert!(starts[1].duration_since(starts[0]) >= Duration::from_millis(25));
        assert_eq!(maximum_active.load(Ordering::SeqCst), 2);
    }
}
