use crate::{PbcError, PbcTableDescriptor};
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
