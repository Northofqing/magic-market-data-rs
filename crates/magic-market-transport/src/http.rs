use crate::TransportError;
use std::collections::HashSet;
use std::fmt;
use std::io::Read;
use std::time::Duration;
use url::Url;

const MAX_CONFIGURED_BODY_BYTES: usize = 16_777_216;
const MAX_TIMEOUT: Duration = Duration::from_secs(60);

/// HTTP verbs supported by the shared transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// Closed response media-type families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaType {
    Json,
    Html,
    Javascript,
    Xml,
    PlainText,
}

impl MediaType {
    fn matches(self, value: &str) -> bool {
        match self {
            Self::Json => value.eq_ignore_ascii_case("application/json"),
            Self::Html => {
                value.eq_ignore_ascii_case("text/html")
                    || value.eq_ignore_ascii_case("application/xhtml+xml")
            }
            Self::Javascript => matches!(
                value.to_ascii_lowercase().as_str(),
                "application/javascript" | "text/javascript" | "application/x-javascript"
            ),
            Self::Xml => {
                value.eq_ignore_ascii_case("application/xml")
                    || value.eq_ignore_ascii_case("text/xml")
            }
            Self::PlainText => value.eq_ignore_ascii_case("text/plain"),
        }
    }
}

/// Immutable request passed through an injected or production transport.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpRequest {
    method: HttpMethod,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpRequest {
    pub fn new(
        method: HttpMethod,
        url: impl Into<String>,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Result<Self, TransportError> {
        let url = url.into();
        let mut seen = HashSet::with_capacity(headers.len());
        for (name, value) in &headers {
            if !valid_header_name(name) {
                return Err(TransportError::InvalidRequest(
                    "request contains an invalid HTTP header name".into(),
                ));
            }
            let normalized = name.to_ascii_lowercase();
            if !seen.insert(normalized.clone()) {
                return Err(TransportError::InvalidRequest(
                    "request contains duplicate HTTP header names".into(),
                ));
            }
            if is_credential_header(&normalized) {
                return Err(TransportError::Authentication(
                    "credential-bearing HTTP headers are forbidden".into(),
                ));
            }
            if is_authority_or_framing_header(&normalized) {
                return Err(TransportError::InvalidRequest(
                    "authority, framing and hop-by-hop HTTP headers are forbidden".into(),
                ));
            }
            if value.chars().any(char::is_control) {
                return Err(TransportError::InvalidRequest(
                    "request contains a control character in an HTTP header value".into(),
                ));
            }
        }
        Ok(Self {
            method,
            url,
            headers,
            body,
        })
    }

    pub fn method(&self) -> HttpMethod {
        self.method
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let safe_url = redacted_url(&self.url);
        let headers: Vec<(&str, &str)> = self
            .headers
            .iter()
            .map(|(name, _)| (name.as_str(), "[REDACTED]"))
            .collect();
        formatter
            .debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &safe_url)
            .field("headers", &headers)
            .field(
                "body",
                &format_args!("[REDACTED; {} bytes]", self.body.len()),
            )
            .finish()
    }
}

/// Complete, bounded response returned by a transport.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpResponse {
    status: u16,
    final_url: String,
    content_type: Option<String>,
    content_encoding: Option<String>,
    body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(
        status: u16,
        final_url: impl Into<String>,
        content_type: Option<String>,
        body: Vec<u8>,
    ) -> Self {
        Self {
            status,
            final_url: final_url.into(),
            content_type,
            content_encoding: None,
            body,
        }
    }

    /// Attaches the source Content-Encoding for injected response validation.
    pub fn with_content_encoding(mut self, content_encoding: Option<String>) -> Self {
        self.content_encoding = content_encoding;
        self
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn final_url(&self) -> &str {
        &self.final_url
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    pub fn content_encoding(&self) -> Option<&str> {
        self.content_encoding.as_deref()
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("final_url", &redacted_url(&self.final_url))
            .field("content_type", &self.content_type)
            .field("content_encoding", &self.content_encoding)
            .field(
                "body",
                &format_args!("[REDACTED; {} bytes]", self.body.len()),
            )
            .finish()
    }
}

/// Transport seam used by production clients and deterministic fixtures.
pub trait HttpTransport: Send + Sync {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError>;
}

/// Closed endpoint contract for one production transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointPolicy {
    hostname: String,
    path_prefixes: Vec<String>,
    query_keys: HashSet<String>,
    media_types: HashSet<MediaType>,
    max_body_bytes: usize,
    timeout: Duration,
}

impl EndpointPolicy {
    pub fn new(
        hostname: impl Into<String>,
        path_prefixes: Vec<String>,
        query_keys: Vec<String>,
        media_types: Vec<MediaType>,
        max_body_bytes: usize,
        timeout: Duration,
    ) -> Result<Self, TransportError> {
        let hostname = hostname.into();
        if !valid_ascii_hostname(&hostname) {
            return Err(TransportError::InvalidRequest(
                "endpoint hostname must be an exact lowercase ASCII hostname".into(),
            ));
        }
        if path_prefixes.is_empty()
            || path_prefixes
                .iter()
                .any(|prefix| !valid_path_prefix(prefix))
        {
            return Err(TransportError::InvalidRequest(
                "endpoint requires non-empty absolute path prefixes".into(),
            ));
        }
        let unique_paths: HashSet<&str> = path_prefixes.iter().map(String::as_str).collect();
        if unique_paths.len() != path_prefixes.len() {
            return Err(TransportError::InvalidRequest(
                "endpoint path prefixes must be unique".into(),
            ));
        }
        let mut unique_queries = HashSet::with_capacity(query_keys.len());
        for key in query_keys {
            if !valid_query_key(&key) || !unique_queries.insert(key) {
                return Err(TransportError::InvalidRequest(
                    "endpoint query keys must be valid and unique".into(),
                ));
            }
        }
        let unique_media: HashSet<MediaType> = media_types.iter().copied().collect();
        if media_types.is_empty() || unique_media.len() != media_types.len() {
            return Err(TransportError::InvalidRequest(
                "endpoint media types must be non-empty and unique".into(),
            ));
        }
        if !(1..=MAX_CONFIGURED_BODY_BYTES).contains(&max_body_bytes) {
            return Err(TransportError::InvalidRequest(
                "endpoint body ceiling must be between 1 and 16777216 bytes".into(),
            ));
        }
        if timeout < Duration::from_secs(1) || timeout > MAX_TIMEOUT {
            return Err(TransportError::InvalidRequest(
                "endpoint timeout must be between 1 and 60 seconds".into(),
            ));
        }
        Ok(Self {
            hostname,
            path_prefixes,
            query_keys: unique_queries,
            media_types: unique_media,
            max_body_bytes,
            timeout,
        })
    }

    pub fn validate_request(&self, request: &HttpRequest) -> Result<(), TransportError> {
        let parsed = self.parse_and_validate_url(request.url())?;
        if request.method() == HttpMethod::Get && !request.body().is_empty() {
            return Err(TransportError::InvalidRequest(
                "GET requests cannot contain a body".into(),
            ));
        }
        if let Some((_, value)) = request
            .headers()
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("Accept-Encoding"))
        {
            if !value.trim().eq_ignore_ascii_case("identity") {
                return Err(TransportError::InvalidRequest(
                    "request Accept-Encoding must be identity".into(),
                ));
            }
        }
        let mut seen = HashSet::new();
        for (key, _) in parsed.query_pairs() {
            if !self.query_keys.contains(key.as_ref()) {
                return Err(TransportError::InvalidRequest(format!(
                    "request query key {key:?} is not allowlisted"
                )));
            }
            if !seen.insert(key.into_owned()) {
                return Err(TransportError::InvalidRequest(
                    "request contains duplicate query keys".into(),
                ));
            }
        }
        Ok(())
    }

    /// Validates a standalone injected response against the endpoint contract.
    ///
    /// Production transports use [`Self::validate_response_for`] so the final
    /// URL is additionally bound to the exact request, including query values.
    pub fn validate_response(
        &self,
        response: HttpResponse,
    ) -> Result<HttpResponse, TransportError> {
        self.validate_response_fields(&response)?;
        self.parse_and_validate_url(response.final_url())
            .map_err(|_| {
                TransportError::Redirect(
                    "response final URL violates the configured endpoint policy".into(),
                )
            })?;
        Ok(response)
    }

    /// Validates a response and binds its final URL to the exact request URL.
    pub fn validate_response_for(
        &self,
        request: &HttpRequest,
        response: HttpResponse,
    ) -> Result<HttpResponse, TransportError> {
        let request_url = self.parse_and_validate_url(request.url())?;
        self.validate_request(request)?;
        self.validate_response_fields(&response)?;
        let response_url = self
            .parse_and_validate_url(response.final_url())
            .map_err(|_| {
                TransportError::Redirect(
                    "response final URL violates the configured endpoint policy".into(),
                )
            })?;
        if response_url != request_url {
            return Err(TransportError::Redirect(
                "response final URL differs from the validated request URL".into(),
            ));
        }
        Ok(response)
    }

    fn validate_response_fields(&self, response: &HttpResponse) -> Result<(), TransportError> {
        match response.status() {
            200 => {}
            300..=399 => {
                return Err(TransportError::Redirect(
                    "redirect responses are forbidden".into(),
                ));
            }
            status => return Err(TransportError::HttpStatus { status }),
        }
        if response.body().len() > self.max_body_bytes {
            return Err(TransportError::ResourceLimit(format!(
                "response exceeds {} bytes",
                self.max_body_bytes
            )));
        }
        if response
            .content_encoding()
            .is_some_and(|value| !value.trim().eq_ignore_ascii_case("identity"))
        {
            return Err(TransportError::MediaType(
                "compressed response content encoding is forbidden".into(),
            ));
        }
        let content_type = response
            .content_type()
            .ok_or_else(|| TransportError::MediaType("response Content-Type is required".into()))?;
        let mime = content_type
            .split(';')
            .next()
            .map(str::trim)
            .unwrap_or_default();
        if mime.is_empty() || !self.media_types.iter().any(|kind| kind.matches(mime)) {
            return Err(TransportError::MediaType(
                "response Content-Type is outside the configured allowlist".into(),
            ));
        }
        Ok(())
    }

    fn parse_and_validate_url(&self, input: &str) -> Result<Url, TransportError> {
        let parsed = Url::parse(input).map_err(|_| {
            TransportError::InvalidRequest("request URL is not a valid absolute URL".into())
        })?;
        if parsed.scheme() != "https" {
            return Err(TransportError::InvalidRequest(
                "request URL must use HTTPS".into(),
            ));
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(TransportError::Authentication(
                "URL credentials are forbidden".into(),
            ));
        }
        if parsed.port().is_some_and(|port| port != 443) {
            return Err(TransportError::InvalidRequest(
                "request URL uses a forbidden port".into(),
            ));
        }
        if parsed.host_str() != Some(self.hostname.as_str()) {
            return Err(TransportError::InvalidRequest(
                "request URL host is not allowlisted".into(),
            ));
        }
        if parsed.fragment().is_some() {
            return Err(TransportError::InvalidRequest(
                "request URL fragments are forbidden".into(),
            ));
        }
        if !self
            .path_prefixes
            .iter()
            .any(|prefix| path_matches_prefix(parsed.path(), prefix))
        {
            return Err(TransportError::InvalidRequest(
                "request URL path is not allowlisted".into(),
            ));
        }
        Ok(parsed)
    }
}

/// Bounded synchronous HTTPS implementation backed by `reqwest`.
///
/// The exact pinned client path was selected because its default logging does
/// not include complete request URIs. Raw connection logging is never enabled,
/// so query credentials such as a FRED API key do not enter dependency logs.
#[derive(Clone)]
pub struct ReqwestTransport {
    policy: EndpointPolicy,
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new(policy: EndpointPolicy) -> Result<Self, TransportError> {
        ensure_rustls_provider()?;
        let timeout = policy.timeout;
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(timeout)
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_zstd()
            .no_deflate()
            .connection_verbose(false)
            .tls_sslkeylogfile(false)
            .build()
            .map_err(|_| {
                TransportError::Internal("bounded HTTPS client initialization failed".into())
            })?;
        Ok(Self { policy, client })
    }

    pub fn policy(&self) -> &EndpointPolicy {
        &self.policy
    }
}

impl fmt::Debug for ReqwestTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReqwestTransport")
            .field("policy", &self.policy)
            .field("client", &"[REDACTED]")
            .finish()
    }
}

impl HttpTransport for ReqwestTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.policy.validate_request(request)?;

        let mut headers = reqwest::header::HeaderMap::with_capacity(request.headers().len() + 1);
        for (name, value) in request.headers() {
            let name = reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                TransportError::Internal("validated HTTP header name was rejected".into())
            })?;
            let value = reqwest::header::HeaderValue::from_str(value).map_err(|_| {
                TransportError::Internal("validated HTTP header value was rejected".into())
            })?;
            headers.insert(name, value);
        }
        headers.insert(
            reqwest::header::ACCEPT_ENCODING,
            reqwest::header::HeaderValue::from_static("identity"),
        );

        let builder = match request.method() {
            HttpMethod::Get => self.client.get(request.url()),
            HttpMethod::Post => self
                .client
                .post(request.url())
                .body(request.body().to_vec()),
        };
        let response = builder.headers(headers).send().map_err(|_| {
            TransportError::Network("request failed before a valid HTTP response".into())
        })?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(rejected_status(status));
        }
        let final_url = response.url().as_str().to_owned();
        let content_type = response_header(&response, reqwest::header::CONTENT_TYPE)?;
        let content_encoding = response_header(&response, reqwest::header::CONTENT_ENCODING)?;
        if content_encoding
            .as_deref()
            .is_some_and(|value| !value.trim().eq_ignore_ascii_case("identity"))
        {
            return Err(TransportError::MediaType(
                "compressed response content encoding is forbidden".into(),
            ));
        }
        let mut body = Vec::with_capacity(self.policy.max_body_bytes.min(64 * 1024));
        let read_limit =
            self.policy.max_body_bytes.checked_add(1).ok_or_else(|| {
                TransportError::ResourceLimit("response read limit overflow".into())
            })?;
        response
            .take(read_limit as u64)
            .read_to_end(&mut body)
            .map_err(|_| TransportError::Network("response body read failed".into()))?;
        let response = HttpResponse {
            status,
            final_url,
            content_type,
            content_encoding,
            body,
        };
        // This request-bound path is mandatory for production execution.
        self.policy.validate_response_for(request, response)
    }
}

fn ensure_rustls_provider() -> Result<(), TransportError> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        return Err(TransportError::Internal(
            "no Rustls crypto provider is available".into(),
        ));
    }
    Ok(())
}

fn response_header(
    response: &reqwest::blocking::Response,
    name: reqwest::header::HeaderName,
) -> Result<Option<String>, TransportError> {
    response
        .headers()
        .get(name)
        .map(|value| {
            value.to_str().map(str::to_owned).map_err(|_| {
                TransportError::MediaType("response contains an invalid HTTP header".into())
            })
        })
        .transpose()
}

fn valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

fn is_credential_header(name: &str) -> bool {
    matches!(name, "cookie" | "authorization" | "proxy-authorization")
}

fn is_authority_or_framing_header(name: &str) -> bool {
    matches!(
        name,
        "host"
            | "content-length"
            | "transfer-encoding"
            | "connection"
            | "proxy-connection"
            | "keep-alive"
            | "te"
            | "trailer"
            | "upgrade"
    )
}

fn valid_ascii_hostname(hostname: &str) -> bool {
    !hostname.is_empty()
        && hostname.len() <= 253
        && hostname.is_ascii()
        && hostname.bytes().all(|byte| !byte.is_ascii_uppercase())
        && !hostname.starts_with('.')
        && !hostname.ends_with('.')
        && hostname.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
        })
}

fn valid_path_prefix(prefix: &str) -> bool {
    prefix.starts_with('/')
        && !prefix.is_empty()
        && !prefix.contains('?')
        && !prefix.contains('#')
        && !prefix.chars().any(char::is_control)
}

fn valid_query_key(key: &str) -> bool {
    !key.is_empty()
        && key.is_ascii()
        && !key
            .bytes()
            .any(|byte| byte.is_ascii_control() || matches!(byte, b'&' | b'=' | b'#'))
}

fn path_matches_prefix(path: &str, prefix: &str) -> bool {
    if path == prefix || prefix.ends_with('/') {
        path.starts_with(prefix)
    } else {
        path.strip_prefix(prefix)
            .is_some_and(|remainder| remainder.starts_with('/'))
    }
}

fn rejected_status(status: u16) -> TransportError {
    if (300..=399).contains(&status) {
        TransportError::Redirect("redirect responses are forbidden".into())
    } else {
        TransportError::HttpStatus { status }
    }
}

fn redacted_url(input: &str) -> String {
    let Ok(parsed) = Url::parse(input) else {
        return "[INVALID URL; REDACTED]".into();
    };
    let Some(host) = parsed.host_str() else {
        return "[INVALID URL; REDACTED]".into();
    };
    let mut safe = format!("{}://{}{}", parsed.scheme(), host, parsed.path());
    let keys: Vec<String> = parsed
        .query_pairs()
        .map(|(key, _)| format!("{key}=[REDACTED]"))
        .collect();
    if !keys.is_empty() {
        safe.push('?');
        safe.push_str(&keys.join("&"));
    }
    if parsed.fragment().is_some() {
        safe.push_str("#[REDACTED]");
    }
    safe
}

#[cfg(test)]
mod tests;
