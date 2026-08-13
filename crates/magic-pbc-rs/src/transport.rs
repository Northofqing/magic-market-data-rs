use crate::{PbcError, PbcTableDescriptor, REGIONAL_SOCIAL_FINANCING_URL};
use magic_market_transport::{HttpMethod, HttpRequest, HttpResponse, HttpTransport};

pub(crate) fn fetch_table(
    transport: &dyn HttpTransport,
    descriptor: &PbcTableDescriptor,
) -> Result<HttpResponse, PbcError> {
    let request = HttpRequest::new(
        HttpMethod::Get,
        descriptor.canonical_url(),
        vec![("Accept".into(), "text/html".into())],
        Vec::new(),
    )?;
    transport.execute(&request).map_err(PbcError::from)
}

pub(crate) fn fetch_regional_workbook(
    transport: &dyn HttpTransport,
) -> Result<HttpResponse, PbcError> {
    let request = HttpRequest::new(
        HttpMethod::Get,
        REGIONAL_SOCIAL_FINANCING_URL,
        vec![(
            "Accept".into(),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(),
        )],
        Vec::new(),
    )?;
    transport.execute(&request).map_err(PbcError::from)
}
