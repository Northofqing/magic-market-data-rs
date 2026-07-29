use crate::NbsError;
use magic_market_transport::{HttpMethod, HttpRequest, HttpTransport, RequestGate};

const LANDING_URL: &str = "https://www.stats.gov.cn/";

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
