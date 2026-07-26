use crate::{validate_timeout, WallstreetCnError};
use std::io::Read;
use std::time::Duration;

/// The one public WallstreetCN RSS endpoint permitted by this adapter.
pub const RSS_URL: &str = "https://dedicated.wallstreetcn.com/rss.xml";

pub(crate) const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

/// Immutable request passed to an injected transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    url: String,
    headers: Vec<(String, String)>,
}

impl HttpRequest {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }
}

/// Complete bounded response returned by a transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    final_url: String,
    content_type: Option<String>,
    body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(final_url: impl Into<String>, content_type: Option<String>, body: Vec<u8>) -> Self {
        Self {
            final_url: final_url.into(),
            content_type,
            body,
        }
    }

    pub fn final_url(&self) -> &str {
        &self.final_url
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

/// Bounded transport seam used by production and deterministic fixtures.
pub trait WallstreetCnTransport: Send + Sync {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, WallstreetCnError>;
}

#[derive(Clone)]
pub(crate) struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    pub(crate) fn new(timeout: Duration) -> Result<Self, WallstreetCnError> {
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
}

impl WallstreetCnTransport for HttpsTransport {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, WallstreetCnError> {
        ensure_official_url(request.url())?;
        let mut call = self.agent.get(request.url());
        for (name, value) in request.headers() {
            call = call.set(name, value);
        }
        let response = match call.call() {
            Ok(response) => response,
            Err(ureq::Error::Status(status, _)) => {
                return Err(WallstreetCnError::Protocol(format!(
                    "unexpected HTTP status {status}"
                )));
            }
            Err(error) => return Err(WallstreetCnError::Transport(error.to_string())),
        };
        ensure_success_status(response.status())?;
        let final_url = response.get_url().to_owned();
        ensure_official_final_url(&final_url)?;
        let content_type = response.header("Content-Type").map(str::to_owned);
        ensure_content_type(content_type.as_deref())?;
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|error| WallstreetCnError::Transport(error.to_string()))?;
        ensure_body_size(&body)?;
        Ok(HttpResponse::new(final_url, content_type, body))
    }
}

pub(crate) fn build_request() -> HttpRequest {
    HttpRequest {
        url: RSS_URL.to_owned(),
        headers: vec![
            (
                "Accept".into(),
                "application/rss+xml, application/xml;q=0.9, text/xml;q=0.8, text/html;q=0.1"
                    .into(),
            ),
            ("User-Agent".into(), "magic-wallstreetcn-rs/0.2".into()),
        ],
    }
}

fn ensure_official_final_url(url: &str) -> Result<(), WallstreetCnError> {
    ensure_official_url(url).map_err(|_| {
        WallstreetCnError::Protocol(format!(
            "response final URL is not the exact public WallstreetCN RSS endpoint: {url}"
        ))
    })
}

pub(crate) fn ensure_official_url(url: &str) -> Result<(), WallstreetCnError> {
    if url == RSS_URL && !url.chars().any(char::is_control) {
        Ok(())
    } else {
        Err(WallstreetCnError::InvalidRequest(
            "WallstreetCN transport permits only the exact public RSS endpoint".into(),
        ))
    }
}

pub(crate) fn ensure_success_status(status: u16) -> Result<(), WallstreetCnError> {
    if status == 200 {
        Ok(())
    } else {
        Err(WallstreetCnError::Protocol(format!(
            "unexpected HTTP status {status}"
        )))
    }
}

pub(crate) fn ensure_content_type(content_type: Option<&str>) -> Result<(), WallstreetCnError> {
    let accepted = content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .map(str::trim)
            .is_some_and(|media_type| {
                [
                    "application/rss+xml",
                    "application/xml",
                    "text/xml",
                    "text/html",
                ]
                .into_iter()
                .any(|allowed| media_type.eq_ignore_ascii_case(allowed))
            })
    });
    if accepted {
        Ok(())
    } else {
        Err(WallstreetCnError::Protocol(format!(
            "expected an RSS-compatible response, received content type {content_type:?}"
        )))
    }
}

pub(crate) fn ensure_body_size(body: &[u8]) -> Result<(), WallstreetCnError> {
    if body.is_empty() {
        Err(WallstreetCnError::Protocol(
            "WallstreetCN returned an empty RSS body".into(),
        ))
    } else if body.len() > MAX_RESPONSE_BYTES {
        Err(WallstreetCnError::Protocol(format!(
            "response exceeds {MAX_RESPONSE_BYTES} bytes"
        )))
    } else {
        Ok(())
    }
}

pub(crate) fn validate_response(response: &HttpResponse) -> Result<(), WallstreetCnError> {
    ensure_official_final_url(response.final_url())?;
    ensure_content_type(response.content_type())?;
    ensure_body_size(response.body())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_contract_is_exact() {
        assert_eq!(RSS_URL, "https://dedicated.wallstreetcn.com/rss.xml");
        let request = build_request();
        assert_eq!(request.url(), RSS_URL);
        assert_eq!(
            request.headers(),
            &[
                (
                    "Accept".into(),
                    "application/rss+xml, application/xml;q=0.9, text/xml;q=0.8, text/html;q=0.1"
                        .into(),
                ),
                ("User-Agent".into(), "magic-wallstreetcn-rs/0.2".into()),
            ]
        );
    }

    #[test]
    fn transport_endpoint_and_media_type_allowlists_are_closed() {
        assert!(ensure_official_url(RSS_URL).is_ok());
        for invalid in [
            "http://dedicated.wallstreetcn.com/rss.xml",
            "https://wallstreetcn.com/rss.xml",
            "https://dedicated.wallstreetcn.com:444/rss.xml",
            "https://user@dedicated.wallstreetcn.com/rss.xml",
            "https://dedicated.wallstreetcn.com//rss.xml",
            "https://dedicated.wallstreetcn.com/rss.xml/",
            "https://dedicated.wallstreetcn.com/rss.xml?x=1",
            "https://dedicated.wallstreetcn.com/rss.xml#x",
            "https://dedicated.wallstreetcn.com/rss.xml\n",
        ] {
            assert!(ensure_official_url(invalid).is_err(), "{invalid}");
        }

        for media_type in [
            "application/rss+xml",
            "application/xml; charset=utf-8",
            "TEXT/XML",
            "text/html; charset=UTF-8",
        ] {
            assert!(
                ensure_content_type(Some(media_type)).is_ok(),
                "{media_type}"
            );
        }
        for media_type in [None, Some("application/json"), Some("text/plain")] {
            assert!(ensure_content_type(media_type).is_err());
        }
    }

    #[test]
    fn transport_response_bounds_are_strict() {
        assert!(ensure_body_size(b"x").is_ok());
        assert!(ensure_body_size(&vec![b'x'; MAX_RESPONSE_BYTES]).is_ok());
        assert!(ensure_body_size(&[]).is_err());
        assert!(ensure_body_size(&vec![b'x'; MAX_RESPONSE_BYTES + 1]).is_err());
    }

    #[test]
    fn transport_status_failures_remain_typed() {
        assert!(ensure_success_status(200).is_ok());
        assert!(matches!(
            ensure_success_status(404),
            Err(WallstreetCnError::Protocol(_))
        ));
    }
}
