use crate::transport::{
    validate_minimum_interval, validate_response, validate_timeout, ExchangeTransport, HttpMethod,
    HttpRequest, HttpsTransport, RequestGate,
};
use crate::ExchangeError;
use magic_market_core::{
    CalendarCapabilities, DataBatch, FuturesDeliveryCalendar, FuturesDeliveryEvent,
    FuturesDeliveryMethod, FuturesDeliveryRequest, FuturesProduct, HttpsUrl, IsoDate, NonEmptyText,
    Provenance, ProviderId, SourceEvidence,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

const HOST: &str = "www.cffex.com.cn";
const LIST_ENDPOINT: &str = "https://www.cffex.com.cn/jystz/";
const DELIVERY_TITLE: &str = "股指期货和股指期权合约交割的通知";
const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; magic-exchange-rs/0.2; read-only CFFEX notice parser)";

#[derive(Debug, Clone)]
pub struct CffexConfig {
    pub list_endpoint: String,
    pub timeout: Duration,
    pub minimum_interval: Duration,
    pub max_pages: u32,
}

impl Default for CffexConfig {
    fn default() -> Self {
        Self {
            list_endpoint: LIST_ENDPOINT.into(),
            timeout: Duration::from_secs(20),
            minimum_interval: Duration::from_secs(1),
            max_pages: 120,
        }
    }
}

impl CffexConfig {
    fn validate(&self) -> Result<(), ExchangeError> {
        if self.list_endpoint != LIST_ENDPOINT {
            return Err(ExchangeError::InvalidRequest(
                "CFFEX list endpoint must be the exact official HTTPS notice path".into(),
            ));
        }
        validate_timeout(self.timeout)?;
        validate_minimum_interval(self.minimum_interval)?;
        if self.max_pages == 0 || self.max_pages > 120 {
            return Err(ExchangeError::InvalidRequest(
                "CFFEX max_pages must be between 1 and 120".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct CffexClient {
    config: CffexConfig,
    transport: Arc<dyn ExchangeTransport>,
    gate: Arc<RequestGate>,
}

impl std::fmt::Debug for CffexClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CffexClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl CffexClient {
    pub fn new() -> Result<Self, ExchangeError> {
        Self::with_config(CffexConfig::default())
    }

    pub fn with_config(config: CffexConfig) -> Result<Self, ExchangeError> {
        config.validate()?;
        let transport = HttpsTransport::new(config.timeout)?;
        Self::from_parts(config, Arc::new(transport))
    }

    pub fn with_transport(
        config: CffexConfig,
        transport: impl ExchangeTransport + 'static,
    ) -> Result<Self, ExchangeError> {
        config.validate()?;
        Self::from_parts(config, Arc::new(transport))
    }

    fn from_parts(
        config: CffexConfig,
        transport: Arc<dyn ExchangeTransport>,
    ) -> Result<Self, ExchangeError> {
        Ok(Self {
            gate: Arc::new(RequestGate::new(config.minimum_interval)),
            config,
            transport,
        })
    }

    pub const fn provider_id() -> ProviderId {
        ProviderId::Cffex
    }

    pub const fn calendar_capabilities() -> CalendarCapabilities {
        CalendarCapabilities {
            economic_releases: false,
            futures_delivery: true,
        }
    }

    fn get_html(&self, url: &str) -> Result<Vec<u8>, ExchangeError> {
        validate_cffex_url(url)?;
        let request = HttpRequest {
            method: HttpMethod::Get,
            url: url.to_owned(),
            headers: vec![
                ("User-Agent".into(), USER_AGENT.into()),
                ("Accept".into(), "text/html,application/xhtml+xml".into()),
                ("Referer".into(), "https://www.cffex.com.cn/".into()),
            ],
            body: Vec::new(),
        };
        let response = self.gate.execute(|| self.transport.execute(&request))?;
        validate_response(&request, &response, &["text/html", "application/xhtml+xml"])?;
        Ok(response.body)
    }

    fn find_notice(
        &self,
        request: &FuturesDeliveryRequest,
    ) -> Result<(String, Vec<u8>), ExchangeError> {
        let target_month = format!("{:04}-{:02}", request.year().get(), request.month().get());
        for page in 1..=self.config.max_pages {
            let page_url = if page == 1 {
                self.config.list_endpoint.clone()
            } else {
                format!("https://{HOST}/jystz/index_{page}.html")
            };
            let body = self.get_html(&page_url)?;
            let html = std::str::from_utf8(&body)
                .map_err(|error| ExchangeError::Decode(format!("CFFEX list UTF-8: {error}")))?;
            let links = parse_notice_links(html)?;
            if links.is_empty() {
                return Err(ExchangeError::Schema(format!(
                    "CFFEX notice page {page} contains no dated notice links"
                )));
            }
            for link in &links {
                if link.title.contains(DELIVERY_TITLE) && link.date.starts_with(&target_month) {
                    let url = official_notice_url(&link.href)?;
                    let detail = self.get_html(&url)?;
                    return Ok((url, detail));
                }
            }
            let oldest = links
                .iter()
                .map(|link| link.date.as_str())
                .min()
                .ok_or_else(|| ExchangeError::Schema("CFFEX list has no date".into()))?;
            if oldest
                .get(..7)
                .is_some_and(|month| month < target_month.as_str())
            {
                return Err(ExchangeError::Incomplete(format!(
                    "CFFEX has no official equity-index delivery notice for {target_month}"
                )));
            }
        }
        Err(ExchangeError::Incomplete(format!(
            "CFFEX notice search exceeded {} pages",
            self.config.max_pages
        )))
    }
}

impl FuturesDeliveryCalendar for CffexClient {
    type Error = ExchangeError;

    fn futures_delivery_calendar(
        &self,
        request: &FuturesDeliveryRequest,
    ) -> Result<DataBatch<FuturesDeliveryEvent>, Self::Error> {
        let (notice_url, detail) = self.find_notice(request)?;
        let observed_at = now()?;
        parse_delivery_notice(&detail, request, &notice_url, &observed_at)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NoticeLink {
    href: String,
    title: String,
    date: String,
}

fn parse_notice_links(html: &str) -> Result<Vec<NoticeLink>, ExchangeError> {
    let mut links = Vec::new();
    let mut remainder = html;
    while let Some(start) = remainder.find("<a") {
        remainder = &remainder[start + 2..];
        let Some(tag_end) = remainder.find('>') else {
            return Err(ExchangeError::Schema(
                "CFFEX list contains an unterminated anchor".into(),
            ));
        };
        let tag = &remainder[..tag_end];
        let after_tag = &remainder[tag_end + 1..];
        let Some(close) = after_tag.find("</a>") else {
            remainder = after_tag;
            continue;
        };
        let title = strip_html(&after_tag[..close]);
        let href = extract_attribute(tag, "href");
        let after_anchor = &after_tag[close + 4..];
        if let (Some(href), Some(date)) = (
            href,
            find_iso_date(&after_anchor[..after_anchor.len().min(240)]),
        ) {
            if href.contains("/jystz/") && href.ends_with(".html") {
                links.push(NoticeLink { href, title, date });
            }
        }
        remainder = after_anchor;
    }
    let mut identities = HashSet::with_capacity(links.len());
    for link in &links {
        if !identities.insert(link.href.as_str()) {
            return Err(ExchangeError::Schema(format!(
                "CFFEX list contains duplicate notice link {}",
                link.href
            )));
        }
    }
    Ok(links)
}

fn extract_attribute(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(start) = tag.find(&needle) {
            let value = &tag[start + needle.len()..];
            if let Some(end) = value.find(quote) {
                return Some(value[..end].trim().to_owned());
            }
        }
    }
    None
}

fn find_iso_date(value: &str) -> Option<String> {
    value.as_bytes().windows(10).find_map(|window| {
        let candidate = std::str::from_utf8(window).ok()?;
        IsoDate::new(candidate).ok().map(|_| candidate.to_owned())
    })
}

fn official_notice_url(value: &str) -> Result<String, ExchangeError> {
    let url = if value.starts_with("https://") {
        value.to_owned()
    } else {
        let path = if value.starts_with('/') {
            value.to_owned()
        } else {
            format!("/jystz/{value}")
        };
        format!("https://{HOST}{path}")
    };
    validate_cffex_url(&url)?;
    if !is_detail_path(
        Url::parse(&url)
            .map_err(|error| ExchangeError::Schema(error.to_string()))?
            .path(),
    ) {
        return Err(ExchangeError::Schema(
            "CFFEX delivery link is not a dated detail path".into(),
        ));
    }
    Ok(url)
}

fn validate_cffex_url(value: &str) -> Result<(), ExchangeError> {
    let url =
        Url::parse(value).map_err(|error| ExchangeError::InvalidRequest(error.to_string()))?;
    let path = url.path();
    let list_path = path == "/jystz/"
        || path
            .strip_prefix("/jystz/index_")
            .and_then(|value| value.strip_suffix(".html"))
            .is_some_and(|page| {
                page.parse::<u32>()
                    .is_ok_and(|page| (2..=120).contains(&page))
            });
    if url.scheme() != "https"
        || url.host_str() != Some(HOST)
        || url.port_or_known_default() != Some(443)
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (!list_path && !is_detail_path(path))
    {
        return Err(ExchangeError::InvalidRequest(
            "CFFEX request URL is outside the official bounded notice paths".into(),
        ));
    }
    Ok(())
}

fn is_detail_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/jystz/") else {
        return false;
    };
    let Some((date, file)) = rest.split_once('/') else {
        return false;
    };
    date.len() == 8
        && date.bytes().all(|byte| byte.is_ascii_digit())
        && file
            .strip_suffix(".html")
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
}

fn parse_delivery_notice(
    body: &[u8],
    request: &FuturesDeliveryRequest,
    notice_url: &str,
    observed_at: &str,
) -> Result<DataBatch<FuturesDeliveryEvent>, ExchangeError> {
    validate_cffex_url(notice_url)?;
    let html = std::str::from_utf8(body)
        .map_err(|error| ExchangeError::Decode(format!("CFFEX detail UTF-8: {error}")))?;
    let text = strip_html(html);
    if !text.contains(DELIVERY_TITLE) {
        return Err(ExchangeError::Schema(
            "CFFEX detail title is not the equity-index delivery notice".into(),
        ));
    }
    if !text.contains("交割结算价") {
        return Err(ExchangeError::Schema(
            "CFFEX detail does not prove a delivery settlement price".into(),
        ));
    }
    let suffix = format!(
        "{:02}{:02}",
        request.year().get() % 100,
        request.month().get()
    );
    let products = [
        (FuturesProduct::If, "IF"),
        (FuturesProduct::Ih, "IH"),
        (FuturesProduct::Ic, "IC"),
        (FuturesProduct::Im, "IM"),
    ];
    for (_, prefix) in products {
        let contract = format!("{prefix}{suffix}");
        if !text.contains(&contract) {
            return Err(ExchangeError::Incomplete(format!(
                "CFFEX notice does not contain required contract {contract}"
            )));
        }
    }
    let delivery_date = parse_delivery_date(&text)?;
    if delivery_date.as_str().get(..7)
        != Some(&format!(
            "{:04}-{:02}",
            request.year().get(),
            request.month().get()
        ))
    {
        return Err(ExchangeError::Schema(format!(
            "CFFEX delivery date {} does not match requested contract month",
            delivery_date.as_str()
        )));
    }
    let batch_id = format!("cffex:{}:{}:{observed_at}", suffix, delivery_date.as_str());
    let url = HttpsUrl::new(notice_url)?;
    let mut records = Vec::with_capacity(products.len());
    for (product, prefix) in products {
        let evidence = SourceEvidence::new(ProviderId::Cffex, observed_at, batch_id.clone())?
            .with_source_at(delivery_date.as_str())?;
        records.push(FuturesDeliveryEvent {
            product,
            contract_code: NonEmptyText::new(format!("{prefix}{suffix}"))?,
            last_trading_date: delivery_date.clone(),
            delivery_date: delivery_date.clone(),
            method: FuturesDeliveryMethod::NotProvided,
            notice_url: url.clone(),
            evidence,
        });
    }
    let provenance = Provenance::new("cffex-official-notice", observed_at)?
        .with_source_at(delivery_date.as_str())?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn parse_delivery_date(text: &str) -> Result<IsoDate, ExchangeError> {
    let marker = "进行交割";
    let marker_at = text
        .find(marker)
        .ok_or_else(|| ExchangeError::Schema("CFFEX delivery sentence is missing".into()))?;
    let before = &text[..marker_at];
    let year_end = before
        .rfind('年')
        .ok_or_else(|| ExchangeError::Schema("delivery year marker is missing".into()))?;
    let year_start = before[..year_end]
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_ascii_digit())
        .map_or(0, |(index, character)| index + character.len_utf8());
    let month_end_relative = before[year_end + '年'.len_utf8()..]
        .find('月')
        .ok_or_else(|| ExchangeError::Schema("delivery month marker is missing".into()))?;
    let month_end = year_end + '年'.len_utf8() + month_end_relative;
    let day_end_relative = before[month_end + '月'.len_utf8()..]
        .find('日')
        .ok_or_else(|| ExchangeError::Schema("delivery day marker is missing".into()))?;
    let day_end = month_end + '月'.len_utf8() + day_end_relative;
    let year = &before[year_start..year_end];
    let month = &before[year_end + '年'.len_utf8()..month_end];
    let day = &before[month_end + '月'.len_utf8()..day_end];
    if year.len() != 4
        || !year.bytes().all(|byte| byte.is_ascii_digit())
        || month.is_empty()
        || !month.bytes().all(|byte| byte.is_ascii_digit())
        || day.is_empty()
        || !day.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ExchangeError::Schema(
            "CFFEX delivery date contains invalid digits".into(),
        ));
    }
    let month = month
        .parse::<u32>()
        .map_err(|error| ExchangeError::Schema(error.to_string()))?;
    let day = day
        .parse::<u32>()
        .map_err(|error| ExchangeError::Schema(error.to_string()))?;
    Ok(IsoDate::new(format!("{year}-{month:02}-{day:02}"))?)
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
    output
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn now() -> Result<String, ExchangeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| ExchangeError::Transport(format!("system clock error: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::HttpResponse;
    use magic_market_core::PositiveU32;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct FixtureTransport {
        responses: Mutex<VecDeque<HttpResponse>>,
    }

    impl ExchangeTransport for FixtureTransport {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
            let mut response = self
                .responses
                .lock()
                .map_err(|_| ExchangeError::Transport("fixture lock poisoned".into()))?
                .pop_front()
                .ok_or_else(|| ExchangeError::Transport("no fixture response".into()))?;
            response.final_url = request.url.clone();
            Ok(response)
        }
    }

    fn response(body: &str) -> HttpResponse {
        HttpResponse {
            status: 200,
            final_url: String::new(),
            content_type: Some("text/html; charset=UTF-8".into()),
            body: body.as_bytes().to_vec(),
        }
    }

    fn request() -> FuturesDeliveryRequest {
        FuturesDeliveryRequest::new(
            PositiveU32::new(2026).unwrap(),
            PositiveU32::new(2).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn parses_holiday_adjusted_notice_without_inventing_delivery_method() {
        let list = r#"
          <ul><li><a href="/jystz/20260224/46999.html">
          关于股指期货和股指期权合约交割的通知</a><span>2026-02-24</span></li></ul>
        "#;
        let detail = r#"
          <html><h1>关于股指期货和股指期权合约交割的通知</h1>
          <p>IF2602等合约于2026年2月24日进行交割，各合约的交割结算价具体如下：</p>
          <p>沪深300股指期货IF2602合约交割结算价；</p>
          <p>中证500股指期货IC2602合约交割结算价；</p>
          <p>中证1000股指期货IM2602合约交割结算价；</p>
          <p>上证50股指期货IH2602合约交割结算价。</p></html>
        "#;
        let client = CffexClient::with_transport(
            CffexConfig {
                minimum_interval: Duration::from_secs(1),
                ..CffexConfig::default()
            },
            FixtureTransport {
                responses: Mutex::new(VecDeque::from([response(list), response(detail)])),
            },
        )
        .unwrap();
        let batch = client.futures_delivery_calendar(&request()).unwrap();
        assert_eq!(batch.records().len(), 4);
        assert_eq!(batch.records()[0].contract_code.as_str(), "IF2602");
        assert_eq!(batch.records()[0].delivery_date.as_str(), "2026-02-24");
        assert_eq!(
            batch.records()[0].method,
            FuturesDeliveryMethod::NotProvided
        );
    }

    #[test]
    fn rejects_formula_only_or_incomplete_notices() {
        let detail = r#"
          <h1>关于股指期货和股指期权合约交割的通知</h1>
          IF2602合约于2026年2月20日进行交割，交割结算价具体如下：
          IC2602 IM2602
        "#;
        assert!(parse_delivery_notice(
            detail.as_bytes(),
            &request(),
            "https://www.cffex.com.cn/jystz/20260220/1.html",
            "observed"
        )
        .is_err());
    }

    #[test]
    fn list_parser_requires_unique_official_dated_links() {
        let list = r#"
          <a href="/jystz/20260224/1.html">关于股指期货和股指期权合约交割的通知</a>2026-02-24
          <a href="/jystz/20260224/1.html">重复</a>2026-02-24
        "#;
        assert!(parse_notice_links(list).is_err());
        assert!(official_notice_url("https://example.com/x").is_err());
    }
}
