use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError};
use magic_market_core::{
    IsoDate, MarketRankingKind, NonEmptyText, PositiveU32, ProviderTopNRankingRequest,
    ProviderTopNRankings,
};

#[test]
fn public_provider_contract_exposes_the_exact_production_identity() {
    let request = EastmoneyClient::provider_top_n_a_share_request(
        MarketRankingKind::VolumeRatio,
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(20).unwrap(),
    )
    .unwrap();

    assert_eq!(request.kind(), &MarketRankingKind::VolumeRatio);
    assert_eq!(request.limit().get(), 20);
    assert_eq!(
        EastmoneyClient::provider_top_n_source_identity()
            .unwrap()
            .as_str(),
        "eastmoney-web"
    );
    assert!(EastmoneyClient::provider_top_n_ranking_capabilities().volume_ratio);
    assert!(EastmoneyClient::provider_top_n_ranking_capabilities().main_net_inflow);
}

#[test]
fn public_provider_trait_rejects_an_unadmitted_filter_before_network_io() {
    let request = ProviderTopNRankingRequest::new(
        MarketRankingKind::VolumeRatio,
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(20).unwrap(),
        NonEmptyText::new("not-the-admitted-a-share-filter").unwrap(),
    )
    .unwrap();
    let client = EastmoneyClient::new().unwrap();

    assert!(matches!(
        ProviderTopNRankings::provider_top_n_rankings(&client, &request),
        Err(EastmoneyError::InvalidRequest(message))
            if message.contains("filter identity")
    ));
}
