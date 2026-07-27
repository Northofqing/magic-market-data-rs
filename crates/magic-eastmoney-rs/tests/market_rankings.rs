use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError};
use magic_market_core::{MarketRankingCapabilities, MarketRankings, SignalCapabilities};

#[test]
fn client_implements_the_provider_contract_but_capability_waits_for_live_admission() {
    fn assert_provider<T: MarketRankings<Error = EastmoneyError>>() {}
    assert_provider::<EastmoneyClient>();
    assert_eq!(
        EastmoneyClient::market_ranking_capabilities(),
        MarketRankingCapabilities {
            volume_ratio: false,
            main_net_inflow: false,
        }
    );
    assert_eq!(
        EastmoneyClient::signal_capabilities(),
        SignalCapabilities {
            board_memberships: false,
            strong_stock_reasons: false,
            dragon_tiger: true,
            market_rankings: false,
            popularity: true,
            concept_hits: false,
        }
    );
}
