#![forbid(unsafe_code)]
//! Bounded read-only adapter for the official China Government policy library.

use magic_market_core::{
    DataBatch, HttpsUrl, IsoDate, NonEmptyText, PolicyCapabilities, PolicyDocument,
    PolicyDocuments, PolicyRequest, Provenance, ProviderId, SourceEvidence,
};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use url::Url;

const ENDPOINT: &str = "https://sousuo.www.gov.cn/search-gov/data";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const POLICY_CATEGORIES: [&str; 2] = ["gongwen", "bumenfile"];

#[derive(Debug, Error)]
pub enum GovError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("government policy response decoding failed: {0}")]
    Decode(String),
    #[error("government policy protocol error: {0}")]
    Protocol(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

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

pub trait GovTransport: Send + Sync {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, GovError>;
}

#[derive(Clone)]
struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    fn new(timeout: Duration) -> Result<Self, GovError> {
        if timeout.is_zero() || timeout > Duration::from_secs(60) {
            return Err(GovError::InvalidRequest(
                "timeout must be positive and at most 60 seconds".into(),
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
}

impl GovTransport for HttpsTransport {
    fn get(&self, request: &HttpRequest) -> Result<Vec<u8>, GovError> {
        ensure_search_url(request.url())?;
        let mut call = self.agent.get(request.url());
        for (name, value) in request.headers() {
            call = call.set(name, value);
        }
        let response = call
            .call()
            .map_err(|error| GovError::Transport(error.to_string()))?;
        if response.status() != 200 {
            return Err(GovError::Transport(format!(
                "unexpected HTTP status {}",
                response.status()
            )));
        }
        let content_type = response.header("Content-Type");
        if !content_type.is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
        }) {
            return Err(GovError::Protocol(format!(
                "expected JSON content type, received {content_type:?}"
            )));
        }
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|error| GovError::Transport(error.to_string()))?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(GovError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        Ok(body)
    }
}

#[derive(Clone)]
pub struct GovClient {
    transport: Arc<dyn GovTransport>,
    minimum_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
}

impl std::fmt::Debug for GovClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("GovClient").finish_non_exhaustive()
    }
}

impl GovClient {
    pub fn new() -> Result<Self, GovError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, GovError> {
        Ok(Self::from_parts(
            Arc::new(HttpsTransport::new(timeout)?),
            MINIMUM_REQUEST_INTERVAL,
        ))
    }

    pub fn with_transport(transport: impl GovTransport + 'static) -> Self {
        Self::from_parts(Arc::new(transport), MINIMUM_REQUEST_INTERVAL)
    }

    fn from_parts(transport: Arc<dyn GovTransport>, minimum_interval: Duration) -> Self {
        Self {
            transport,
            minimum_interval,
            request_gate: Arc::new(Mutex::new(None)),
        }
    }

    pub const fn policy_capabilities() -> PolicyCapabilities {
        PolicyCapabilities {
            official_documents: true,
        }
    }

    fn execute(&self, request: &HttpRequest) -> Result<Vec<u8>, GovError> {
        let mut last_started = self
            .request_gate
            .lock()
            .map_err(|_| GovError::Transport("request gate lock poisoned".into()))?;
        if let Some(previous) = *last_started {
            let elapsed = previous.elapsed();
            if elapsed < self.minimum_interval {
                thread::sleep(self.minimum_interval - elapsed);
            }
        }
        *last_started = Some(Instant::now());
        let result = self.transport.get(request);
        drop(last_started);
        result
    }
}

impl PolicyDocuments for GovClient {
    type Error = GovError;

    fn policy_documents(
        &self,
        request: &PolicyRequest,
    ) -> Result<DataBatch<PolicyDocument>, Self::Error> {
        let wire_request = build_request(request)?;
        let body = self.execute(&wire_request)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(GovError::Protocol(format!(
                "response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        let observed_at = now()?;
        parse_response(&body, request, &observed_at)
    }
}

fn build_request(request: &PolicyRequest) -> Result<HttpRequest, GovError> {
    let mut url =
        Url::parse(ENDPOINT).map_err(|error| GovError::InvalidRequest(error.to_string()))?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("t", "zhengcelibrary")
            .append_pair(
                "q",
                request
                    .query()
                    .map(NonEmptyText::as_str)
                    .unwrap_or_default(),
            )
            .append_pair("sort", "pubtime")
            .append_pair("sortType", "1")
            .append_pair("searchfield", "title:content:summary")
            .append_pair("p", &request.page().get().to_string())
            .append_pair("n", &request.page_size().get().to_string());
        if let (Some(start), Some(end)) = (request.start(), request.end()) {
            query
                .append_pair("timetype", "timezd")
                .append_pair("mintime", start.as_str())
                .append_pair("maxtime", end.as_str());
        }
    }
    let request = HttpRequest {
        url: url.into(),
        headers: vec![
            ("Accept".into(), "application/json".into()),
            ("Referer".into(), "https://sousuo.www.gov.cn/".into()),
            ("User-Agent".into(), "magic-gov-rs/0.2".into()),
        ],
    };
    ensure_search_url(request.url())?;
    Ok(request)
}

fn ensure_search_url(value: &str) -> Result<(), GovError> {
    let url = Url::parse(value).map_err(|error| GovError::InvalidRequest(error.to_string()))?;
    if url.scheme() != "https"
        || url.host_str() != Some("sousuo.www.gov.cn")
        || url.port_or_known_default() != Some(443)
        || url.username() != ""
        || url.password().is_some()
        || url.path() != "/search-gov/data"
    {
        return Err(GovError::InvalidRequest(
            "policy search must use the official credential-free HTTPS endpoint".into(),
        ));
    }
    Ok(())
}

fn parse_response(
    body: &[u8],
    request: &PolicyRequest,
    observed_at: &str,
) -> Result<DataBatch<PolicyDocument>, GovError> {
    let root: Value =
        serde_json::from_slice(body).map_err(|error| GovError::Decode(error.to_string()))?;
    let object = root
        .as_object()
        .ok_or_else(|| GovError::Protocol("response root must be an object".into()))?;
    if object.get("code").and_then(Value::as_i64) != Some(200) {
        return Err(GovError::Protocol(format!(
            "government search returned code {:?}",
            object.get("code")
        )));
    }
    let categories = object
        .get("searchVO")
        .and_then(Value::as_object)
        .and_then(|value| value.get("catMap"))
        .and_then(Value::as_object)
        .ok_or_else(|| GovError::Protocol("searchVO.catMap is missing".into()))?;
    let batch_id = format!(
        "gov-policy:{observed_at}:{}:{}",
        request.page().get(),
        request.page_size().get()
    );
    let mut identities = HashSet::new();
    let mut records = Vec::new();
    for category in POLICY_CATEGORIES {
        let family = categories
            .get(category)
            .and_then(Value::as_object)
            .ok_or_else(|| GovError::Protocol(format!("category {category} is missing")))?;
        let source_category = family
            .get("catName")
            .and_then(Value::as_str)
            .ok_or_else(|| GovError::Protocol(format!("{category}.catName is missing")))?;
        if source_category != category {
            return Err(GovError::Protocol(format!(
                "category identity {source_category:?} does not match {category}"
            )));
        }
        let rows = family
            .get("listVO")
            .and_then(Value::as_array)
            .ok_or_else(|| GovError::Protocol(format!("{category}.listVO is missing")))?;
        if rows.len() > request.page_size().get() as usize {
            return Err(GovError::Protocol(format!(
                "{category} returned more than the requested page size"
            )));
        }
        for row in rows {
            let row = row
                .as_object()
                .ok_or_else(|| GovError::Protocol("policy row must be an object".into()))?;
            let record = map_document(row, category, observed_at, &batch_id)?;
            if !identities.insert(record.document_id.as_str().to_owned()) {
                return Err(GovError::Protocol(format!(
                    "duplicate policy document {}",
                    record.document_id.as_str()
                )));
            }
            if request
                .start()
                .is_some_and(|start| &record.published_date < start)
                || request
                    .end()
                    .is_some_and(|end| &record.published_date > end)
            {
                return Err(GovError::Protocol(format!(
                    "policy {} falls outside requested date range",
                    record.document_id.as_str()
                )));
            }
            records.push(record);
        }
    }
    if records.is_empty() {
        return Err(GovError::Protocol(
            "official policy search returned no documents".into(),
        ));
    }
    records.sort_by(|left, right| {
        right
            .published_date
            .cmp(&left.published_date)
            .then_with(|| right.document_id.as_str().cmp(left.document_id.as_str()))
    });
    records.truncate(request.page_size().get() as usize);
    let source_at = records
        .first()
        .map(|record| record.published_date.as_str())
        .ok_or_else(|| GovError::Protocol("policy source date is missing".into()))?;
    let provenance = Provenance::new("gov-cn-policy-library", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn map_document(
    row: &Map<String, Value>,
    family: &str,
    observed_at: &str,
    batch_id: &str,
) -> Result<PolicyDocument, GovError> {
    let document_id = required_text(row, "id")?;
    let published_date = parse_date(&required_text(row, "pubtimeStr")?)?;
    let canonical_url = official_document_url(&required_text(row, "url")?)?;
    let organization = required_text(row, "puborg")?;
    let category = optional_text(row, "childtype").unwrap_or_else(|| family.to_owned());
    let evidence = SourceEvidence::new(ProviderId::StateCouncil, observed_at, batch_id)?
        .with_source_at(published_date.as_str())?;
    Ok(PolicyDocument {
        document_id: NonEmptyText::new(document_id)?,
        title: NonEmptyText::new(strip_html(&required_text(row, "title")?))?,
        summary: optional_text(row, "summary")
            .map(|value| NonEmptyText::new(strip_html(&value)))
            .transpose()?,
        organization: NonEmptyText::new(organization)?,
        document_number: optional_text(row, "pcode")
            .map(NonEmptyText::new)
            .transpose()?,
        category: Some(NonEmptyText::new(category)?),
        published_date,
        canonical_url,
        evidence,
    })
}

fn official_document_url(value: &str) -> Result<HttpsUrl, GovError> {
    let url = Url::parse(value)
        .map_err(|error| GovError::Protocol(format!("invalid document URL: {error}")))?;
    if url.scheme() != "https"
        || url.host_str() != Some("www.gov.cn")
        || url.port_or_known_default() != Some(443)
        || url.username() != ""
        || url.password().is_some()
    {
        return Err(GovError::Protocol(
            "policy document URL is not an official credential-free www.gov.cn URL".into(),
        ));
    }
    Ok(HttpsUrl::new(value)?)
}

fn required_text(row: &Map<String, Value>, field: &str) -> Result<String, GovError> {
    optional_text(row, field)
        .ok_or_else(|| GovError::Protocol(format!("policy {field} is missing")))
}

fn optional_text(row: &Map<String, Value>, field: &str) -> Option<String> {
    row.get(field)
        .and_then(Value::as_str)
        .map(normalize_text)
        .filter(|value| !value.is_empty())
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    normalize_text(
        &output
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'"),
    )
}

fn parse_date(value: &str) -> Result<IsoDate, GovError> {
    let bytes = value.as_bytes();
    if value.len() != 10 || bytes[4] != b'.' || bytes[7] != b'.' {
        return Err(GovError::Protocol("pubtimeStr must use YYYY.MM.DD".into()));
    }
    IsoDate::new(format!("{}-{}-{}", &value[..4], &value[5..7], &value[8..])).map_err(Into::into)
}

fn now() -> Result<String, GovError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| GovError::Protocol(format!("system clock precedes UNIX epoch: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use magic_market_core::PositiveU32;

    struct FixtureTransport(Vec<u8>);

    impl GovTransport for FixtureTransport {
        fn get(&self, _request: &HttpRequest) -> Result<Vec<u8>, GovError> {
            Ok(self.0.clone())
        }
    }

    const FIXTURE: &str = r#"{
      "code": 200,
      "msg": "操作成功",
      "searchVO": {
        "catMap": {
          "gongwen": {
            "catName": "gongwen",
            "listVO": [{
              "id": "26391409",
              "pcode": "国函〔2026〕80号",
              "title": "国务院关于同意将山东省潍坊市列为国家历史文化名城的批复",
              "summary": "国务院关于同意将潍坊市列为国家历史文化名城的批复",
              "pubtimeStr": "2026.07.24",
              "url": "https://www.gov.cn/zhengce/zhengceku/202607/content_7076509.htm",
              "childtype": "文化、广电、新闻出版\\文化",
              "puborg": "国务院"
            }]
          },
          "bumenfile": {
            "catName": "bumenfile",
            "listVO": [{
              "id": "26392212",
              "pcode": "交运发〔2026〕86号",
              "title": "综合运输服务发展规划",
              "summary": "",
              "pubtimeStr": "2026.07.24",
              "url": "https://www.gov.cn/zhengce/zhengceku/202607/content_7076565.htm",
              "childtype": "工业、交通\\其他",
              "puborg": "交通运输部 国家铁路局"
            }]
          }
        }
      }
    }"#;

    #[test]
    fn parses_only_official_policy_families_and_urls() {
        let request =
            PolicyRequest::new(PositiveU32::new(1).unwrap(), PositiveU32::new(2).unwrap())
                .unwrap()
                .with_range(
                    IsoDate::new("2026-07-24").unwrap(),
                    IsoDate::new("2026-07-24").unwrap(),
                )
                .unwrap();
        let client = GovClient::from_parts(
            Arc::new(FixtureTransport(FIXTURE.as_bytes().to_vec())),
            Duration::ZERO,
        );
        let batch = client.policy_documents(&request).unwrap();
        assert_eq!(batch.records().len(), 2);
        assert!(batch.records()[0]
            .canonical_url
            .as_str()
            .starts_with("https://www.gov.cn/"));
        assert_eq!(batch.provenance().source_at(), Some("2026-07-24"));
    }

    #[test]
    fn rejects_non_official_document_links_and_builds_exact_search_contract() {
        assert!(official_document_url("https://example.com/policy").is_err());
        let request =
            PolicyRequest::new(PositiveU32::new(2).unwrap(), PositiveU32::new(5).unwrap())
                .unwrap()
                .with_query("金融")
                .unwrap();
        let wire = build_request(&request).unwrap();
        assert!(wire.url().contains("t=zhengcelibrary"));
        assert!(wire.url().contains("sort=pubtime"));
        assert!(wire.url().contains("p=2"));
        assert!(wire.url().contains("n=5"));
        assert!(ensure_search_url(wire.url()).is_ok());
    }
}
