use magic_market_core::{Capabilities, ProviderId};
#[test]
fn provider_capabilities_are_explicit() {
    assert_eq!(ProviderId::Tdx, ProviderId::Tdx);
    assert!(!Capabilities::new().quotes);
}
