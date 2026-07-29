use magic_cfets_rs::{CfetsClient, CfetsError};
use magic_market_core::{
    CurrencyCode, IsoDate, OfficialFxFixingIdentity, OfficialFxFixingProvider,
    OfficialFxFixingRequest, PositiveU32, ProviderId, ReferenceRateIdentity, ReferenceRateKind,
    ReferenceRateProvider, ReferenceRateRequest, ReferenceTenor,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct NoIo;

impl HttpTransport for NoIo {
    fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        panic!("unsupported DR007 request must not perform I/O");
    }
}

#[test]
fn dr007_is_explicitly_unsupported_before_io() {
    let request = ReferenceRateRequest::new(
        vec![ReferenceRateIdentity::new(ProviderId::Cfets, ReferenceRateKind::Dr007).unwrap()],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    let client = CfetsClient::with_transport(Arc::new(NoIo)).unwrap();
    assert!(matches!(
        client.reference_rates(&request),
        Err(CfetsError::Unsupported(_))
    ));
}

fn shibor_request(provider: ProviderId) -> ReferenceRateRequest {
    ReferenceRateRequest::new(
        vec![ReferenceRateIdentity::new(
            provider,
            ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
        )
        .unwrap()],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap()
}

#[test]
fn foreign_identities_fail_before_io() {
    let client = CfetsClient::with_transport(Arc::new(NoIo)).unwrap();
    assert!(matches!(
        client.reference_rates(&shibor_request(ProviderId::Fred)),
        Err(CfetsError::InvalidRequest(_))
    ));

    let foreign_fx = OfficialFxFixingRequest::new(
        vec![OfficialFxFixingIdentity::new(
            ProviderId::Fred,
            CurrencyCode::new("USD").unwrap(),
            CurrencyCode::new("CNY").unwrap(),
        )
        .unwrap()],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        client.official_fx_fixings(&foreign_fx),
        Err(CfetsError::InvalidRequest(_))
    ));
}

struct ShiborFixture {
    calls: Arc<AtomicUsize>,
}

impl HttpTransport for ShiborFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            include_bytes!("fixtures/shibor.json").to_vec(),
        ))
    }
}

#[test]
fn admitted_formal_rate_provider_performs_the_fetch() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = CfetsClient::with_transport(Arc::new(ShiborFixture {
        calls: Arc::clone(&calls),
    }))
    .unwrap();
    assert_eq!(
        client
            .reference_rates(&shibor_request(ProviderId::Cfets))
            .unwrap()
            .records()
            .len(),
        1
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

struct FxFixture {
    calls: Arc<AtomicUsize>,
}

impl HttpTransport for FxFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let body = if call == 0 {
            include_bytes!("fixtures/ccpr-page-1.json").to_vec()
        } else {
            include_bytes!("fixtures/ccpr-page-2.json").to_vec()
        };
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            body,
        ))
    }
}

#[test]
fn admitted_formal_fx_provider_fetches_and_validates_all_pages() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = CfetsClient::with_transport(Arc::new(FxFixture {
        calls: Arc::clone(&calls),
    }))
    .unwrap();
    let request = OfficialFxFixingRequest::new(
        [("USD", "CNY"), ("JPY", "CNY"), ("CNY", "KRW")]
            .into_iter()
            .map(|(base, quote)| {
                OfficialFxFixingIdentity::new(
                    ProviderId::Cfets,
                    CurrencyCode::new(base).unwrap(),
                    CurrencyCode::new(quote).unwrap(),
                )
                .unwrap()
            })
            .collect(),
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    assert_eq!(
        client
            .official_fx_fixings(&request)
            .unwrap()
            .records()
            .len(),
        6
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
