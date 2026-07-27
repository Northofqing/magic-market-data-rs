use crate::EastmoneyError;
use magic_market_core::{LoadProbeSnapshot, ProbeRequestTracker};
use std::io::Read;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub(crate) const DEFAULT_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_HTML_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_PDF_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36";
const MAX_REDIRECT_LOCATION_CHARS: usize = 512;
const ALLOWED_HOSTS: &[&str] = &[
    "datacenter-web.eastmoney.com",
    "emappdata.eastmoney.com",
    "push2.eastmoney.com",
    "push2delay.eastmoney.com",
    "push2ex.eastmoney.com",
    "push2his.eastmoney.com",
    "reportapi.eastmoney.com",
    "pdf.dfcfw.com",
    "roll.eastmoney.com",
];

/// Injected bounded transport used by deterministic fixtures and the live client.
pub trait EastmoneyTransport: Send + Sync {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError>;

    fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError>;

    fn load_probe_snapshot(&self) -> Option<LoadProbeSnapshot> {
        None
    }

    fn get_pdf(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.get(url, headers, max_bytes)
    }

    fn get_html(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.get(url, headers, max_bytes)
    }
}

#[derive(Clone)]
pub(crate) struct HttpsTransport {
    agent: ureq::Agent,
    minimum_interval: Duration,
    last_request: Arc<Mutex<Option<Instant>>>,
    request_probe: Arc<Mutex<ProbeRequestTracker>>,
}

impl HttpsTransport {
    pub(crate) fn new(timeout: Duration) -> Result<Self, EastmoneyError> {
        if timeout.is_zero() {
            return Err(EastmoneyError::InvalidRequest(
                "timeout must be greater than zero".into(),
            ));
        }
        Ok(Self {
            agent: ureq::AgentBuilder::new()
                .timeout_connect(timeout)
                .timeout_read(timeout)
                .timeout_write(timeout)
                .redirects(0)
                .build(),
            minimum_interval: Duration::from_secs(1),
            last_request: Arc::new(Mutex::new(None)),
            request_probe: Arc::new(Mutex::new(ProbeRequestTracker::default())),
        })
    }

    fn acquire_slot(&self) -> Result<MutexGuard<'_, Option<Instant>>, EastmoneyError> {
        let mut last_request = self
            .last_request
            .lock()
            .map_err(|_| EastmoneyError::Transport("request limiter lock poisoned".into()))?;
        if let Some(previous) = *last_request {
            let elapsed = previous.elapsed();
            if elapsed < self.minimum_interval {
                std::thread::sleep(self.minimum_interval - elapsed);
            }
        }
        *last_request = Some(Instant::now());
        self.request_probe
            .lock()
            .map_err(|_| EastmoneyError::Transport("request probe lock poisoned".into()))?
            .request_started();
        Ok(last_request)
    }

    fn finish_request(&self) -> Result<(), EastmoneyError> {
        self.request_probe
            .lock()
            .map_err(|_| EastmoneyError::Transport("request probe lock poisoned".into()))?
            .request_finished()
            .map_err(|error| EastmoneyError::Transport(error.to_string()))
    }

    fn read_response(
        response: ureq::Response,
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        let status = response.status();
        let content_type = response.header("Content-Type").map(str::to_owned);
        read_http_response(
            status,
            content_type.as_deref(),
            response.into_reader(),
            max_bytes,
        )
    }

    fn prepare_get(&self, url: &str, headers: &[(&str, &str)]) -> ureq::Request {
        let mut request = self.agent.get(url).set("User-Agent", USER_AGENT);
        for (name, value) in headers {
            request = request.set(name, value);
        }
        request
    }

    fn prepare_post(&self, url: &str, headers: &[(&str, &str)]) -> ureq::Request {
        let mut request = self
            .agent
            .post(url)
            .set("User-Agent", USER_AGENT)
            .set("Content-Type", "application/json");
        for (name, value) in headers {
            request = request.set(name, value);
        }
        request
    }

    fn get_request(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        validate_endpoint(url)?;
        validate_response_limit(max_bytes)?;
        let request_gate = self.acquire_slot()?;
        let result = (|| {
            let request = self.prepare_get(url, headers);
            let response = request.call().map_err(map_ureq_error)?;
            Self::read_response(response, max_bytes)
        })();
        self.finish_request()?;
        drop(request_gate);
        result
    }

    fn post_request(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        validate_endpoint(url)?;
        validate_response_limit(max_bytes)?;
        if body.len() > 64 * 1024 {
            return Err(EastmoneyError::InvalidRequest(
                "JSON request body exceeds 65536 bytes".into(),
            ));
        }
        let request_gate = self.acquire_slot()?;
        let result = (|| {
            let request = self.prepare_post(url, headers);
            let response = request.send_bytes(body).map_err(map_ureq_error)?;
            Self::read_response(response, max_bytes)
        })();
        self.finish_request()?;
        drop(request_gate);
        result
    }

    fn get_pdf_request(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        validate_endpoint(url)?;
        if max_bytes == 0 || max_bytes > MAX_PDF_RESPONSE_BYTES {
            return Err(EastmoneyError::InvalidRequest(format!(
                "PDF response limit must be in 1..={MAX_PDF_RESPONSE_BYTES}"
            )));
        }
        let request_gate = self.acquire_slot()?;
        let result = (|| {
            let mut request = self.agent.get(url).set("User-Agent", USER_AGENT);
            for (name, value) in headers {
                request = request.set(name, value);
            }
            let response = request.call().map_err(map_ureq_error)?;
            validate_pdf_content_type(response.header("Content-Type"))?;
            let mut body = Vec::new();
            response
                .into_reader()
                .take((max_bytes + 1) as u64)
                .read_to_end(&mut body)
                .map_err(|error| EastmoneyError::Transport(error.to_string()))?;
            if body.len() > max_bytes {
                return Err(EastmoneyError::ResponseTooLarge { limit: max_bytes });
            }
            Ok(body)
        })();
        self.finish_request()?;
        drop(request_gate);
        result
    }

    fn get_html_request(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        validate_news_page_endpoint(url)?;
        if max_bytes == 0 || max_bytes > MAX_HTML_RESPONSE_BYTES {
            return Err(EastmoneyError::InvalidRequest(format!(
                "HTML response limit must be in 1..={MAX_HTML_RESPONSE_BYTES}"
            )));
        }
        let request_gate = self.acquire_slot()?;
        let result = (|| {
            let mut request = self.agent.get(url).set("User-Agent", USER_AGENT);
            for (name, value) in headers {
                request = request.set(name, value);
            }
            let response = request.call().map_err(map_ureq_error)?;
            validate_html_content_type(response.header("Content-Type"))?;
            let mut body = Vec::new();
            response
                .into_reader()
                .take((max_bytes + 1) as u64)
                .read_to_end(&mut body)
                .map_err(|error| EastmoneyError::Transport(error.to_string()))?;
            if body.len() > max_bytes {
                return Err(EastmoneyError::ResponseTooLarge { limit: max_bytes });
            }
            Ok(body)
        })();
        self.finish_request()?;
        drop(request_gate);
        result
    }
}

fn map_ureq_error(error: ureq::Error) -> EastmoneyError {
    match error {
        ureq::Error::Status(status, response) => {
            let mut message = format!("unexpected HTTP status {status}");
            if (300..400).contains(&status) {
                match response.header("Location") {
                    Some(location) => {
                        let bounded = location
                            .chars()
                            .take(MAX_REDIRECT_LOCATION_CHARS)
                            .collect::<String>();
                        message
                            .push_str(&format!("; redirects are disabled; Location={bounded:?}"));
                    }
                    None => message.push_str("; redirects are disabled; Location missing"),
                }
            }
            EastmoneyError::Transport(message)
        }
        ureq::Error::Transport(error) => EastmoneyError::Transport(error.to_string()),
    }
}

fn validate_response_limit(max_bytes: usize) -> Result<(), EastmoneyError> {
    if max_bytes == 0 || max_bytes > DEFAULT_MAX_RESPONSE_BYTES {
        return Err(EastmoneyError::InvalidRequest(format!(
            "response limit must be in 1..={DEFAULT_MAX_RESPONSE_BYTES}"
        )));
    }
    Ok(())
}

fn read_http_response(
    status: u16,
    content_type: Option<&str>,
    reader: impl Read,
    max_bytes: usize,
) -> Result<Vec<u8>, EastmoneyError> {
    validate_response_limit(max_bytes)?;
    if status != 200 {
        return Err(EastmoneyError::Transport(format!(
            "unexpected HTTP status {status}"
        )));
    }
    validate_content_type(content_type)?;
    let mut body = Vec::new();
    reader
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|error| EastmoneyError::Transport(error.to_string()))?;
    if body.len() > max_bytes {
        return Err(EastmoneyError::ResponseTooLarge { limit: max_bytes });
    }
    Ok(body)
}

fn validate_content_type(content_type: Option<&str>) -> Result<(), EastmoneyError> {
    let content_type = content_type.ok_or_else(|| {
        EastmoneyError::Protocol("Eastmoney response has no Content-Type header".into())
    })?;
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if matches!(
        media_type.as_str(),
        "application/json"
            | "application/javascript"
            | "text/javascript"
            | "text/plain"
            | "application/x-javascript"
    ) {
        Ok(())
    } else {
        Err(EastmoneyError::Protocol(format!(
            "unexpected Eastmoney response Content-Type {content_type:?}"
        )))
    }
}

fn validate_pdf_content_type(content_type: Option<&str>) -> Result<(), EastmoneyError> {
    let media_type = content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if media_type.is_some_and(|value| value.eq_ignore_ascii_case("application/pdf")) {
        Ok(())
    } else {
        Err(EastmoneyError::Protocol(format!(
            "expected PDF Content-Type, received {content_type:?}"
        )))
    }
}

fn validate_html_content_type(content_type: Option<&str>) -> Result<(), EastmoneyError> {
    let content_type = content_type.ok_or_else(|| {
        EastmoneyError::Protocol("Eastmoney HTML response has no Content-Type header".into())
    })?;
    let mut parts = content_type.split(';').map(str::trim);
    let media_type = parts.next().unwrap_or_default();
    let utf8 = parts.any(|part| {
        part.split_once('=').is_some_and(|(name, value)| {
            name.trim().eq_ignore_ascii_case("charset")
                && value.trim().trim_matches('"').eq_ignore_ascii_case("utf-8")
        })
    });
    if media_type.eq_ignore_ascii_case("text/html") && utf8 {
        Ok(())
    } else {
        Err(EastmoneyError::Protocol(format!(
            "expected UTF-8 HTML Content-Type, received {content_type:?}"
        )))
    }
}

impl EastmoneyTransport for HttpsTransport {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.get_request(url, headers, max_bytes)
    }

    fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.post_request(url, headers, body, max_bytes)
    }

    fn load_probe_snapshot(&self) -> Option<LoadProbeSnapshot> {
        self.request_probe.lock().ok().map(|probe| probe.snapshot())
    }

    fn get_pdf(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.get_pdf_request(url, headers, max_bytes)
    }

    fn get_html(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.get_html_request(url, headers, max_bytes)
    }
}

pub(crate) fn validate_news_page_endpoint(url: &str) -> Result<(), EastmoneyError> {
    if url == "https://roll.eastmoney.com/finance.html" {
        Ok(())
    } else {
        Err(EastmoneyError::InvalidRequest(
            "latest news must use the exact Eastmoney finance rolling page".into(),
        ))
    }
}

pub(crate) fn validate_endpoint(url: &str) -> Result<(), EastmoneyError> {
    let remainder = url
        .strip_prefix("https://")
        .ok_or_else(|| EastmoneyError::InvalidRequest("endpoint must use HTTPS".into()))?;
    let authority = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return Err(EastmoneyError::InvalidRequest(
            "endpoint authority is invalid".into(),
        ));
    }
    let host = match authority.rsplit_once(':') {
        Some((host, "443")) => host,
        Some(_) => {
            return Err(EastmoneyError::InvalidRequest(
                "endpoint may only use the default HTTPS port".into(),
            ))
        }
        None => authority,
    };
    if !ALLOWED_HOSTS.contains(&host) {
        return Err(EastmoneyError::InvalidRequest(format!(
            "host {host} is not an allowed Eastmoney public host"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/internal/transport_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/internal/document_and_news_regression_tests.rs"]
mod document_and_news_regression_tests;
