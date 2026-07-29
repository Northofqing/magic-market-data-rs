use crate::CfetsError;
use magic_market_core::{OfficialFxFixingRequest, ReferenceRateRequest};
use magic_market_transport::{HttpMethod, HttpRequest, HttpResponse, HttpTransport};

const BASE: &str = "https://www.chinamoney.com.cn/ags/ms/";

pub(crate) fn fetch_shibor(
    transport: &dyn HttpTransport,
    request: &ReferenceRateRequest,
) -> Result<HttpResponse, CfetsError> {
    let url = format!(
        "{BASE}cm-u-bk-shibor/ShiborHis?lang=cn&startDate={}&endDate={}",
        request.start(),
        request.end()
    );
    execute_json(transport, HttpMethod::Post, url)
}

pub(crate) fn fetch_lpr(
    transport: &dyn HttpTransport,
    request: &ReferenceRateRequest,
) -> Result<HttpResponse, CfetsError> {
    let url = format!(
        "{BASE}cm-u-bk-currency/LprHis?lang=CN&strStartDate={}&strEndDate={}",
        request.start(),
        request.end()
    );
    execute_json(transport, HttpMethod::Post, url)
}

pub(crate) fn fetch_fx_page(
    transport: &dyn HttpTransport,
    request: &OfficialFxFixingRequest,
    headings: &[&str],
    page: usize,
) -> Result<HttpResponse, CfetsError> {
    let currency = headings.join(",").replace('/', "%2F").replace(',', "%2C");
    let url = format!(
        "{BASE}cm-u-bk-ccpr/CcprHisNew?startDate={}&endDate={}&currency={currency}&pageNum={page}&pageSize=50",
        request.start(),
        request.end()
    );
    execute_json(transport, HttpMethod::Get, url)
}

fn execute_json(
    transport: &dyn HttpTransport,
    method: HttpMethod,
    url: String,
) -> Result<HttpResponse, CfetsError> {
    let request = HttpRequest::new(
        method,
        url,
        vec![("Accept".into(), "application/json".into())],
        Vec::new(),
    )?;
    transport.execute(&request).map_err(CfetsError::from)
}
