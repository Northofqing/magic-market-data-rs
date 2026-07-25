use magic_eastmoney_rs::EastmoneyClient;
use magic_market_core::{
    DragonTigerDiscovery, MarketDiscoveryCapabilities, NewsProvider, PostCloseFlows,
    ResearchDocuments,
};

fn assert_dragon_tiger_discovery<T: DragonTigerDiscovery>() {}
fn assert_new_contracts<T: ResearchDocuments + PostCloseFlows>() {}
fn assert_news_contract<T: NewsProvider>() {}

#[test]
fn advertises_only_proved_market_discovery() {
    assert_dragon_tiger_discovery::<EastmoneyClient>();
    assert_new_contracts::<EastmoneyClient>();
    assert_news_contract::<EastmoneyClient>();
    assert_eq!(
        EastmoneyClient::market_discovery_capabilities(),
        MarketDiscoveryCapabilities {
            dragon_tiger_discovery: true,
            board_directory: false,
            board_memberships: false,
            board_constituents: false,
        }
    );
    let research = EastmoneyClient::research_capabilities();
    assert!(research.pdf_download && research.document_body);
    assert!(EastmoneyClient::capital_capabilities().post_close_flow);
    let content = EastmoneyClient::content_capabilities();
    assert!(content.global_news);
    assert!(!content.instrument_news);
}
