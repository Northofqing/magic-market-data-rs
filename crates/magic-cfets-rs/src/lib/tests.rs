use super::*;
use magic_market_core::{
    CurrencyCode, IsoDate, NonEmptyText, OfficialFxFixingIdentity, PositiveU32, ProviderId,
    ReferenceRateIdentity, ReferenceTenor,
};
use magic_market_transport::{HttpRequest, HttpResponse};

struct NoIo;
impl HttpTransport for NoIo {
    fn execute(&self, _: &HttpRequest) -> Result<HttpResponse, TransportError> {
        unreachable!()
    }
}

fn rate_request(kinds: Vec<ReferenceRateKind>) -> ReferenceRateRequest {
    ReferenceRateRequest::new(
        kinds
            .into_iter()
            .map(|kind| ReferenceRateIdentity::new(ProviderId::Cfets, kind).unwrap())
            .collect(),
        IsoDate::new("2026-07-01").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap()
}

#[test]
fn constructors_capabilities_snapshot_and_timestamp_are_covered() {
    assert!(CfetsClient::new(Duration::ZERO).is_err());
    assert!(CfetsClient::new(Duration::from_secs(1)).is_ok());
    let client = CfetsClient::with_transport(Arc::new(NoIo)).unwrap();
    assert_eq!(client.load_probe_snapshot().unwrap().request_starts(), 0);
    assert!(CfetsClient::capabilities().shibor);
    assert!(CfetsClient::reference_data_capabilities().benchmark_rates);
    assert!(now_timestamp().contains('T'));
}

#[test]
fn rate_and_fx_preflight_reject_mixed_and_unadmitted_families() {
    assert!(matches!(
        validate_rate_request(&rate_request(vec![
            ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
            ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OneYear),
        ])),
        Err(CfetsError::InvalidRequest(_))
    ));
    assert!(matches!(
        validate_rate_request(&rate_request(vec![ReferenceRateKind::Dr007])),
        Err(CfetsError::Unsupported(_))
    ));
    assert!(matches!(
        validate_rate_request(&rate_request(vec![ReferenceRateKind::SourceDefined(
            NonEmptyText::new("custom").unwrap(),
        )])),
        Err(CfetsError::Unsupported(_))
    ));
    let unsupported = OfficialFxFixingRequest::new(
        vec![OfficialFxFixingIdentity::new(
            ProviderId::Cfets,
            CurrencyCode::new("EUR").unwrap(),
            CurrencyCode::new("USD").unwrap(),
        )
        .unwrap()],
        IsoDate::new("2026-07-01").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        validate_fx_request(&unsupported),
        Err(CfetsError::Unsupported(_))
    ));
}

#[test]
fn preflight_rejects_foreign_rate_and_fx_providers() {
    let foreign_rate = ReferenceRateRequest::new(
        vec![ReferenceRateIdentity::new(
            ProviderId::Pbc,
            ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
        )
        .unwrap()],
        IsoDate::new("2026-07-01").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        validate_rate_request(&foreign_rate),
        Err(CfetsError::InvalidRequest(_))
    ));

    let foreign_fx = OfficialFxFixingRequest::new(
        vec![OfficialFxFixingIdentity::new(
            ProviderId::Pbc,
            CurrencyCode::new("USD").unwrap(),
            CurrencyCode::new("CNY").unwrap(),
        )
        .unwrap()],
        IsoDate::new("2026-07-01").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        validate_fx_request(&foreign_fx),
        Err(CfetsError::InvalidRequest(_))
    ));
}

#[test]
fn request_probe_reports_unbalanced_and_poisoned_state() {
    let client = CfetsClient::with_transport(Arc::new(NoIo)).unwrap();
    assert!(matches!(
        client.record_finish(),
        Err(CfetsError::Protocol(_))
    ));

    let poison = client.clone();
    assert!(std::thread::spawn(move || {
        let _guard = poison.request_probe.lock().unwrap();
        panic!("poison request probe for contract coverage");
    })
    .join()
    .is_err());
    assert!(matches!(
        client.load_probe_snapshot(),
        Err(CfetsError::Protocol(_))
    ));
    assert!(matches!(
        client.record_start(),
        Err(CfetsError::Protocol(_))
    ));
    assert!(matches!(
        client.record_finish(),
        Err(CfetsError::Protocol(_))
    ));
}

struct FamilyFixture;

impl HttpTransport for FamilyFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = if request.url().contains("/LprHis?") {
            include_bytes!("../../tests/fixtures/lpr.json").to_vec()
        } else {
            include_str!("../../tests/fixtures/ccpr-page-1.json")
                .replace(
                    "\"total\": 2, \"pageTotal\": 2",
                    "\"total\": 1, \"pageTotal\": 1",
                )
                .replace(
                    "\"currency\": \"USD/CNY,100JPY/CNY,CNY/KRW\"",
                    "\"currency\": \"USD/CNY\"",
                )
                .replace(
                    "\"searchlist\": [\"USD/CNY\", \"100JPY/CNY\", \"CNY/KRW\"]",
                    "\"searchlist\": [\"USD/CNY\"]",
                )
                .replace(
                    "\"values\":[\"6.7928\",\"4.5660\",\"193.72\"]",
                    "\"values\":[\"6.7928\"]",
                )
                .into_bytes()
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
fn diagnostic_probe_paths_cover_lpr_and_official_fx_families() {
    let client = CfetsClient::with_transport(Arc::new(FamilyFixture)).unwrap();
    let lpr = rate_request(vec![ReferenceRateKind::LoanPrimeRate(
        ReferenceTenor::OneYear,
    )]);
    assert_eq!(
        client.probe_reference_rates(&lpr).unwrap().records().len(),
        1
    );

    let fx = OfficialFxFixingRequest::new(
        vec![OfficialFxFixingIdentity::new(
            ProviderId::Cfets,
            CurrencyCode::new("USD").unwrap(),
            CurrencyCode::new("CNY").unwrap(),
        )
        .unwrap()],
        IsoDate::new("2026-07-28").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(10).unwrap(),
    )
    .unwrap();
    assert_eq!(
        client
            .probe_official_fx_fixings(&fx)
            .unwrap()
            .records()
            .len(),
        1
    );
}
