use magic_market_composition::EastmoneyProviderTopNRankingRouter;
use magic_market_core::ProviderId;

#[test]
fn production_router_constructs_without_caller_owned_transport() {
    let router = EastmoneyProviderTopNRankingRouter::new().unwrap();
    assert_eq!(router.provider_ids(), [ProviderId::Eastmoney]);
    assert_eq!(router.expected_source().as_str(), "eastmoney-web");
}
