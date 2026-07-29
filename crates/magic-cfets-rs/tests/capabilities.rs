use magic_cfets_rs::{
    CfetsClient, DR007_ADMITTED, LPR_ADMITTED, OFFICIAL_FX_ADMITTED, SHIBOR_ADMITTED,
};

#[test]
fn capabilities_are_family_specific() {
    let source = CfetsClient::capabilities();
    assert_eq!(source.shibor, SHIBOR_ADMITTED);
    assert_eq!(source.loan_prime_rate, LPR_ADMITTED);
    assert!(source.shibor);
    assert!(source.loan_prime_rate);
    assert!(!source.dr007);
    assert_eq!(source.official_fx_fixings, OFFICIAL_FX_ADMITTED);
    assert!(source.official_fx_fixings);
    let core = CfetsClient::reference_data_capabilities();
    assert_eq!(
        core.benchmark_rates,
        SHIBOR_ADMITTED || LPR_ADMITTED || DR007_ADMITTED
    );
    assert_eq!(core.official_fx_fixings, OFFICIAL_FX_ADMITTED);
}
