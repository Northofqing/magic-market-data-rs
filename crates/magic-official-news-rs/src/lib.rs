#![forbid(unsafe_code)]
//! Bounded original publications with explicit, source-native date precision.
//! These records are not projections into the instant-based GlobalNews schema.

mod dom;
mod parse;
mod profiles;

use magic_market_core::{HttpsUrl, IsoDate};
use magic_market_transport::{
    EndpointPolicy, HttpMethod, HttpRequest, HttpTransport, ReqwestTransport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub const NBS_PUBLICATIONS_ADMITTED: bool = true;
pub const PBC_PUBLICATIONS_ADMITTED: bool = true;
pub const NDRC_PUBLICATIONS_ADMITTED: bool = true;
pub const MOF_PUBLICATIONS_ADMITTED: bool = true;
pub const MIIT_PUBLICATIONS_ADMITTED: bool = true;
pub const MOFCOM_PUBLICATIONS_ADMITTED: bool = true;
pub const GACC_PUBLICATIONS_ADMITTED: bool = false;
pub const NEA_PUBLICATIONS_ADMITTED: bool = true;
pub const CSRC_PUBLICATIONS_ADMITTED: bool = true;

const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MINIMUM_INTERVAL: Duration = Duration::from_secs(1);

/// Publisher identity of the exact original website, not an inferred topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OfficialSource {
    Nbs,
    Pbc,
    Ndrc,
    Mof,
    Miit,
    Mofcom,
    Gacc,
    Nea,
    Csrc,
}

impl OfficialSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nbs => "Nbs",
            Self::Pbc => "Pbc",
            Self::Ndrc => "Ndrc",
            Self::Mof => "Mof",
            Self::Miit => "Miit",
            Self::Mofcom => "Mofcom",
            Self::Gacc => "Gacc",
            Self::Nea => "Nea",
            Self::Csrc => "Csrc",
        }
    }

    pub const ALL: [Self; 9] = [
        Self::Nbs,
        Self::Pbc,
        Self::Ndrc,
        Self::Mof,
        Self::Miit,
        Self::Mofcom,
        Self::Gacc,
        Self::Nea,
        Self::Csrc,
    ];

    pub const fn admitted(self) -> bool {
        match self {
            Self::Nbs => NBS_PUBLICATIONS_ADMITTED,
            Self::Pbc => PBC_PUBLICATIONS_ADMITTED,
            Self::Ndrc => NDRC_PUBLICATIONS_ADMITTED,
            Self::Mof => MOF_PUBLICATIONS_ADMITTED,
            Self::Miit => MIIT_PUBLICATIONS_ADMITTED,
            Self::Mofcom => MOFCOM_PUBLICATIONS_ADMITTED,
            Self::Gacc => GACC_PUBLICATIONS_ADMITTED,
            Self::Nea => NEA_PUBLICATIONS_ADMITTED,
            Self::Csrc => CSRC_PUBLICATIONS_ADMITTED,
        }
    }

    pub fn listing_url(self) -> &'static str {
        profiles::profile(self).listing
    }
}

#[derive(Debug, Error)]
pub enum OfficialNewsError {
    #[error("invalid official publication request: {0}")]
    InvalidRequest(String),
    #[error("official publication source is unadmitted: {0:?}")]
    Unadmitted(OfficialSource),
    #[error("unsupported official publication template: {0}")]
    Unsupported(String),
    #[error("official publication protocol failure: {0}")]
    Protocol(String),
    #[error(transparent)]
    Transport(#[from] magic_market_transport::TransportError),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}

/// Precision actually provided by the original publication label. No zone is inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PublicationPrecision {
    Date,
    Minute,
    Second,
}

/// Exact source location supplying the label; precision does not imply a time zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PublicationLabelOrigin {
    ListingHtml,
    ListingApi,
    ArticleMetadata,
    VisibleArticleDate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicationReference {
    pub source: OfficialSource,
    pub title: String,
    pub canonical_url: HttpsUrl,
    pub published_date: IsoDate,
    /// Exact source row label, including source punctuation; no instant is inferred.
    pub publication_label: String,
    pub precision: PublicationPrecision,
    pub publication_label_origin: PublicationLabelOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicationListing {
    pub source: OfficialSource,
    pub listing_url: HttpsUrl,
    /// Exact fetched URL whose response bytes are hashed, including a fixed list API.
    pub response_url: HttpsUrl,
    pub observed_at: String,
    pub response_sha256: String,
    /// Number of validated entries on this one page, before caller limit.
    pub source_rows: usize,
    pub items: Vec<PublicationReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OfficialPublication {
    pub source: OfficialSource,
    pub canonical_url: HttpsUrl,
    pub title: String,
    pub published_date: IsoDate,
    /// Original publication label at the declared origin, never page generation time.
    pub publication_label: String,
    pub precision: PublicationPrecision,
    pub publication_label_origin: PublicationLabelOrigin,
    pub content: String,
    pub observed_at: String,
    pub response_sha256: String,
}

/// Reusable source-specific client. Clones share transport, pacing and serialization.
#[derive(Clone)]
pub struct OfficialNewsClient {
    source: OfficialSource,
    policy: EndpointPolicy,
    transport: Arc<dyn HttpTransport>,
    api_transport: Option<Arc<dyn HttpTransport>>,
    gate: Arc<Mutex<Option<Instant>>>,
    timeout: Duration,
}

impl OfficialNewsClient {
    pub fn new(source: OfficialSource) -> Result<Self, OfficialNewsError> {
        Self::with_timeout(source, Duration::from_secs(15))
    }

    /// Caller timeout of at least one second, capped at the admitted 15-second maximum.
    pub fn with_timeout(
        source: OfficialSource,
        timeout: Duration,
    ) -> Result<Self, OfficialNewsError> {
        let timeout = bounded_timeout(timeout)?;
        let policy = profiles::policy(source, timeout)?;
        let transport = Arc::new(ReqwestTransport::new(policy.clone())?);
        let api_transport = if profiles::profile(source).api.is_some() {
            Some(Arc::new(ReqwestTransport::new(profiles::api_policy(
                source, timeout,
            )?)?) as Arc<dyn HttpTransport>)
        } else {
            None
        };
        let mut client = Self::from_parts(source, policy, transport);
        client.api_transport = api_transport;
        client.timeout = timeout;
        Ok(client)
    }

    pub fn with_transport(
        source: OfficialSource,
        transport: impl HttpTransport + 'static,
    ) -> Result<Self, OfficialNewsError> {
        Ok(Self::from_parts(
            source,
            profiles::policy(source, Duration::from_secs(15))?,
            Arc::new(transport),
        ))
    }

    fn from_parts(
        source: OfficialSource,
        policy: EndpointPolicy,
        transport: Arc<dyn HttpTransport>,
    ) -> Self {
        Self {
            source,
            policy,
            api_transport: profiles::profile(source).api.map(|_| transport.clone()),
            transport,
            gate: Arc::new(Mutex::new(None)),
            timeout: Duration::from_secs(15),
        }
    }

    pub fn latest(&self, limit: u32) -> Result<PublicationListing, OfficialNewsError> {
        self.ensure_admitted()?;
        self.probe_latest(limit)
    }

    pub fn article(&self, url: &str) -> Result<OfficialPublication, OfficialNewsError> {
        self.ensure_admitted()?;
        self.probe_article(url)
    }

    /// Explicit diagnostic access does not promote a source or normalize news instants.
    pub fn probe_latest(&self, limit: u32) -> Result<PublicationListing, OfficialNewsError> {
        if !(1..=20).contains(&limit) {
            return Err(OfficialNewsError::InvalidRequest(
                "limit must be 1 through 20".into(),
            ));
        }
        let profile = profiles::profile(self.source);
        if profile.rows.is_empty() {
            return Err(OfficialNewsError::Unsupported(
                "current dynamic listing requires a separately verified profile".into(),
            ));
        }
        let (body, observed_at, hash) = self.get(profile.api.unwrap_or(profile.listing))?;
        match self.source {
            OfficialSource::Mofcom | OfficialSource::Miit => {
                let value: serde_json::Value = serde_json::from_str(&body)
                    .map_err(|_| protocol("official unit response is not JSON"))?;
                if value.get("code").and_then(serde_json::Value::as_str) != Some("200")
                    || value.get("success").and_then(serde_json::Value::as_bool) != Some(true)
                {
                    return Err(protocol("official unit response is not successful"));
                }
                let html = value
                    .pointer("/data/html")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| protocol("official unit HTML is missing"))?;
                parse::listing(self.source, html, limit, observed_at, hash)
            }
            OfficialSource::Csrc => parse::csrc_listing(&body, limit, observed_at, hash),
            _ => parse::listing(self.source, &body, limit, observed_at, hash),
        }
    }

    /// Fetch one exact allowlisted original HTML page; attachments/scripts are never followed.
    pub fn probe_article(&self, value: &str) -> Result<OfficialPublication, OfficialNewsError> {
        profiles::validate_article(self.source, value)?;
        let (body, observed_at, hash) = self.get(value)?;
        parse::article(self.source, value, &body, observed_at, hash)
    }

    fn ensure_admitted(&self) -> Result<(), OfficialNewsError> {
        if self.source.admitted() {
            Ok(())
        } else {
            Err(OfficialNewsError::Unadmitted(self.source))
        }
    }

    fn get(&self, value: &str) -> Result<(String, String, String), OfficialNewsError> {
        let is_api = profiles::profile(self.source).api == Some(value);
        let policy = if is_api {
            profiles::api_policy(self.source, self.timeout)?
        } else {
            self.policy.clone()
        };
        let request = HttpRequest::new(
            HttpMethod::Get,
            value,
            vec![
                (
                    "Accept".into(),
                    if is_api {
                        "application/json"
                    } else {
                        "text/html"
                    }
                    .into(),
                ),
                ("Accept-Encoding".into(), "identity".into()),
                ("User-Agent".into(), "magic-official-news-rs/0.2".into()),
            ],
            Vec::new(),
        )?;
        policy.validate_request(&request)?;
        let mut last = self
            .gate
            .lock()
            .map_err(|_| OfficialNewsError::Protocol("request gate is poisoned".into()))?;
        if let Some(previous) = *last {
            let elapsed = previous.elapsed();
            if elapsed < MINIMUM_INTERVAL {
                thread::sleep(MINIMUM_INTERVAL - elapsed);
            }
        }
        *last = Some(Instant::now());
        let transport = if is_api {
            self.api_transport
                .as_ref()
                .ok_or_else(|| protocol("source list API transport is missing"))?
        } else {
            &self.transport
        };
        let response = transport.execute(&request)?;
        let response = policy.validate_response_for(&request, response)?;
        let hash = format!("{:x}", Sha256::digest(response.body()));
        let text = std::str::from_utf8(response.body())
            .map_err(|_| OfficialNewsError::Protocol("HTML must be valid UTF-8".into()))?
            .to_owned();
        let observed_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|error| OfficialNewsError::Protocol(error.to_string()))?;
        drop(last);
        Ok((text, observed_at, hash))
    }
}

fn bounded_timeout(timeout: Duration) -> Result<Duration, OfficialNewsError> {
    if timeout < Duration::from_secs(1) {
        return Err(OfficialNewsError::InvalidRequest(
            "timeout must be at least one second".into(),
        ));
    }
    Ok(timeout.min(Duration::from_secs(15)))
}

fn protocol(message: impl Into<String>) -> OfficialNewsError {
    OfficialNewsError::Protocol(message.into())
}

#[cfg(test)]
mod tests;
