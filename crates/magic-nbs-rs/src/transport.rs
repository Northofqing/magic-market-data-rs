use crate::NbsError;
use magic_market_transport::{HttpMethod, HttpRequest, HttpResponse, HttpTransport, RequestGate};

pub(crate) const LANDING_URL: &str = "https://www.stats.gov.cn/";
pub(crate) const API_BASE: &str = "https://data.stats.gov.cn/dg/website/publicrelease/web/external";

pub(crate) fn execute(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    method: HttpMethod,
    url: String,
    body: Vec<u8>,
) -> Result<HttpResponse, NbsError> {
    gate.wait_for_turn()?;
    let mut headers = vec![("Accept".into(), "application/json".into())];
    if method == HttpMethod::Post {
        headers.push(("Content-Type".into(), "application/json".into()));
    }
    let request = HttpRequest::new(method, url, headers, body)?;
    Ok(transport.execute(&request)?)
}

pub(crate) fn probe_landing_page(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
) -> Result<usize, NbsError> {
    gate.wait_for_turn()?;
    let request = HttpRequest::new(
        HttpMethod::Get,
        LANDING_URL,
        vec![("Accept".into(), "text/html".into())],
        Vec::new(),
    )?;
    Ok(transport.execute(&request)?.body().len())
}

#[cfg(test)]
mod tests;
