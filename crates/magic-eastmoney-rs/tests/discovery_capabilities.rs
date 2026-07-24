use magic_eastmoney_rs::EastmoneyClient;
use magic_market_core::{DragonTigerDiscovery, MarketDiscoveryCapabilities};

fn assert_dragon_tiger_discovery<T: DragonTigerDiscovery>() {}

#[test]
fn advertises_only_proved_market_discovery() {
    assert_dragon_tiger_discovery::<EastmoneyClient>();
    assert_eq!(
        EastmoneyClient::market_discovery_capabilities(),
        MarketDiscoveryCapabilities {
            dragon_tiger_discovery: true,
            board_directory: false,
            board_memberships: false,
            board_constituents: false,
        }
    );
}
