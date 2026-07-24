use crate::ExchangeError;
use std::io::Read;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use url::Url;

pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 64 * 1024;

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
}

impl HttpsTransport {
    pub(crate) fn new(timeout: Duration) -> Result<Self, ExchangeError> {
        validate_timeout(timeout)?;
        Ok(Self {
            agent: ureq::AgentBuilder::new()
                .timeout_connect(timeout)
                .timeout_read(timeout)
                .timeout_write(timeout)
                .redirects(0)
                .build(),
        })
    }

    fn collect(response: ureq::Response) -> Result<HttpResponse, ExchangeError> {
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
            Err(ureq::Error::Transport(error)) => Err(ExchangeError::Transport(error.to_string())),
        }
    }
}

#[derive(Debug)]
pub(crate) struct RequestGate {
    minimum_interval: Duration,
    last_started: Mutex<Option<Instant>>,
}

impl RequestGate {
    pub(crate) fn new(minimum_interval: Duration) -> Self {
        Self {
            minimum_interval,
            last_started: Mutex::new(None),
        }
    }

    /// Keeps the mutex through the complete response so client clones cannot
    /// overlap requests even when a transport call is slow.
    pub(crate) fn execute<T>(
        &self,
        operation: impl FnOnce() -> Result<T, ExchangeError>,
    ) -> Result<T, ExchangeError> {
        let mut last_started = self
            .last_started
            .lock()
            .map_err(|_| ExchangeError::Transport("request limiter mutex poisoned".into()))?;
        if let Some(previous) = *last_started {
            let elapsed = previous.elapsed();
            if elapsed < self.minimum_interval {
                thread::sleep(self.minimum_interval - elapsed);
            }
        }
        *last_started = Some(Instant::now());
        operation()
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
) -> Result<(), ExchangeError> {
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
    Ok(())
}

pub(crate) fn validate_response(
    request: &HttpRequest,
    response: &HttpResponse,
    allowed_content_types: &[&str],
) -> Result<(), ExchangeError> {
    if response.body.len() > MAX_RESPONSE_BYTES {
        return Err(ExchangeError::Incomplete(format!(
            "response body exceeds {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    let expected = Url::parse(&request.url)
        .map_err(|error| ExchangeError::InvalidRequest(error.to_string()))?;
    let actual = Url::parse(&response.final_url)
        .map_err(|error| ExchangeError::Schema(error.to_string()))?;
    if expected != actual {
        return Err(ExchangeError::Schema(
            "redirected or final response URL does not match request".into(),
        ));
    }
    match response.status {
        200..=299 => {}
        401 | 403 => return Err(ExchangeError::Authentication(response.status)),
        429 => return Err(ExchangeError::RateLimited),
        status => return Err(ExchangeError::HttpStatus(status)),
    }
    let content_type = response
        .content_type
        .as_deref()
        .ok_or_else(|| ExchangeError::Schema("Content-Type is missing".into()))?
        .to_ascii_lowercase();
    let media_type = content_type
        .split(';')
        .next()
        .map(str::trim)
        .unwrap_or_default();
    let accepted = allowed_content_types.iter().any(|allowed| match *allowed {
        "json" => media_type == "application/json" || media_type.ends_with("+json"),
        "javascript" => matches!(
            media_type,
            "application/javascript" | "text/javascript" | "application/x-javascript"
        ),
        exact => media_type == exact,
    });
    if !accepted {
        return Err(ExchangeError::Schema(format!(
            "unexpected Content-Type {content_type:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

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
        assert!(validate_response(&request, &valid, &["json"]).is_ok());
        let mut redirected = valid.clone();
        redirected.final_url =
            "https://query.sse.com.cn/security/stock/queryCompanyBulletin.do?x=2".into();
        assert!(validate_response(&request, &redirected, &["json"]).is_err());
        let mut html = valid.clone();
        html.content_type = Some("text/html".into());
        assert!(validate_response(&request, &html, &["json"]).is_err());
        let mut fake_javascript = valid.clone();
        fake_javascript.content_type = Some("text/notjavascript".into());
        assert!(validate_response(&request, &fake_javascript, &["javascript"]).is_err());
        let mut oversized = valid;
        oversized.body = vec![0; MAX_RESPONSE_BYTES + 1];
        assert!(validate_response(&request, &oversized, &["json"]).is_err());
    }

    #[test]
    fn timeout_bounds_are_strict() {
        assert!(validate_timeout(Duration::ZERO).is_err());
        assert!(validate_timeout(Duration::from_secs(15)).is_ok());
        assert!(validate_timeout(Duration::from_secs(61)).is_err());
    }

    #[test]
    fn clone_shared_gate_serializes_and_spaces_start_times() {
        let gate = Arc::new(RequestGate::new(Duration::from_millis(30)));
        let starts = Arc::new(Mutex::new(Vec::new()));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let gate = Arc::clone(&gate);
            let starts = Arc::clone(&starts);
            threads.push(thread::spawn(move || {
                gate.execute(|| {
                    starts.lock().unwrap().push(Instant::now());
                    thread::sleep(Duration::from_millis(15));
                    Ok(())
                })
                .unwrap();
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        let mut starts = starts.lock().unwrap().clone();
        starts.sort_unstable();
        assert!(starts[1].duration_since(starts[0]) >= Duration::from_millis(25));
    }
}
