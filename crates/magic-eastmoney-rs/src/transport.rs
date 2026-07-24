use crate::EastmoneyError;
use magic_market_core::{LoadProbeSnapshot, ProbeRequestTracker};
use std::io::Read;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub(crate) const DEFAULT_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36";
const ALLOWED_HOSTS: &[&str] = &[
    "datacenter-web.eastmoney.com",
    "emappdata.eastmoney.com",
    "push2.eastmoney.com",
    "push2ex.eastmoney.com",
    "push2his.eastmoney.com",
    "reportapi.eastmoney.com",
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
        if max_bytes == 0 || max_bytes > DEFAULT_MAX_RESPONSE_BYTES {
            return Err(EastmoneyError::InvalidRequest(format!(
                "response limit must be in 1..={DEFAULT_MAX_RESPONSE_BYTES}"
            )));
        }
        if response.status() != 200 {
            return Err(EastmoneyError::Transport(format!(
                "unexpected HTTP status {}",
                response.status()
            )));
        }
        validate_content_type(response.header("Content-Type"))?;
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
    }

    fn get_request(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        validate_endpoint(url)?;
        let request_gate = self.acquire_slot()?;
        let result = (|| {
            let mut request = self.agent.get(url).set("User-Agent", USER_AGENT);
            for (name, value) in headers {
                request = request.set(name, value);
            }
            let response = request
                .call()
                .map_err(|error| EastmoneyError::Transport(error.to_string()))?;
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
        if body.len() > 64 * 1024 {
            return Err(EastmoneyError::InvalidRequest(
                "JSON request body exceeds 65536 bytes".into(),
            ));
        }
        let request_gate = self.acquire_slot()?;
        let result = (|| {
            let mut request = self
                .agent
                .post(url)
                .set("User-Agent", USER_AGENT)
                .set("Content-Type", "application/json");
            for (name, value) in headers {
                request = request.set(name, value);
            }
            let response = request
                .send_bytes(body)
                .map_err(|error| EastmoneyError::Transport(error.to_string()))?;
            Self::read_response(response, max_bytes)
        })();
        self.finish_request()?;
        drop(request_gate);
        result
    }
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
