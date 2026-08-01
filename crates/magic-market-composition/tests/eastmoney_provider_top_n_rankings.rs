use magic_eastmoney_rs::EastmoneyClient;
use magic_market_composition::{
    EastmoneyProviderTopNRankingRouter, EastmoneyProviderTopNRouterError,
};
use magic_market_core::{
    IsoDate, MarketRankingCapabilities, MarketRankingKind, PositiveU32, ProviderId,
    SignalCapabilities,
};

fn router() -> EastmoneyProviderTopNRankingRouter {
    EastmoneyProviderTopNRankingRouter::new().unwrap()
}

fn request(date: &str) -> magic_market_core::ProviderTopNRankingRequest {
    EastmoneyClient::provider_top_n_a_share_request(
        MarketRankingKind::VolumeRatio,
        IsoDate::new(date).unwrap(),
        PositiveU32::new(20).unwrap(),
    )
    .unwrap()
}

#[test]
fn identity_capabilities_and_provider_are_owned_by_the_concrete_binding() {
    let router = router();
    assert_eq!(router.provider_ids(), vec![ProviderId::Eastmoney]);
    assert_eq!(router.expected_source().as_str(), "eastmoney-web");
    assert!(router.capabilities().volume_ratio);
    assert!(router.capabilities().main_net_inflow);
}

#[test]
fn public_router_debug_preserves_the_bound_production_identity() {
    let debug = format!("{:?}", router());
    assert!(debug.contains("EastmoneyProviderTopNRankingRouter"));
    assert!(debug.contains("Eastmoney"));
    assert!(debug.contains("eastmoney-web"));
    assert!(debug.contains("<injected>"));
}

#[test]
fn dedicated_policy_does_not_claim_realtime_source_time() {
    let router = router();
    assert!(router.policy().require_complete());
    assert!(!router.policy().require_source_at());
    assert!(!router.policy().accept_complete_empty());
}

#[test]
fn future_self_consistent_request_is_rejected_before_provider_io() {
    let router = router();
    let error = router.route(&request("2099-12-31")).unwrap_err();
    assert!(matches!(
        error,
        EastmoneyProviderTopNRouterError::RejectedRequest(_)
    ));
}

#[test]
fn concrete_router_has_exactly_one_non_replaceable_provider() {
    let router = router();
    assert_eq!(router.provider_ids(), [ProviderId::Eastmoney]);
}

#[test]
fn narrow_top_n_contract_does_not_enable_complete_market_rankings() {
    assert_eq!(
        EastmoneyClient::market_ranking_capabilities(),
        MarketRankingCapabilities {
            volume_ratio: false,
            main_net_inflow: false,
        }
    );
    assert!(!EastmoneyClient::signal_capabilities().market_rankings);
    assert!(!SignalCapabilities::default().market_rankings);
    assert_ne!(
        std::any::type_name::<EastmoneyProviderTopNRankingRouter>(),
        std::any::type_name::<magic_market_router::MarketRankingRouter>()
    );
}
