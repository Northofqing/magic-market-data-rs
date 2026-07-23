use magic_market_core::{ConsensusData, LimitPools, PopularityData, StrongStockReasons};
use magic_ths_rs::{ThsClient, ThsError};

fn assert_consensus_provider<T: ConsensusData<Error = ThsError>>() {}
fn assert_strong_stock_provider<T: StrongStockReasons<Error = ThsError>>() {}
fn assert_limit_pool_provider<T: LimitPools<Error = ThsError>>() {}
fn assert_popularity_provider<T: PopularityData<Error = ThsError>>() {}

#[test]
fn public_traits_and_capabilities_match_the_implementation() {
    assert_consensus_provider::<ThsClient>();
    assert_strong_stock_provider::<ThsClient>();
    assert_limit_pool_provider::<ThsClient>();
    assert_popularity_provider::<ThsClient>();

    let capabilities = ThsClient::capabilities();
    assert!(capabilities.research.consensus);
    assert!(!capabilities.research.reports);
    assert!(!capabilities.research.semantic_search);
    assert!(!capabilities.research.pdf_download);
    assert!(capabilities.signals.strong_stock_reasons);
    assert!(capabilities.signals.popularity);
    assert!(!capabilities.signals.board_memberships);
    assert!(!capabilities.signals.dragon_tiger);
    assert!(!capabilities.signals.market_rankings);
    assert!(!capabilities.signals.concept_hits);
    assert!(capabilities.limit_pools.upper);
    assert!(capabilities.limit_pools.reasons);
    assert!(!capabilities.limit_pools.broken);
    assert!(!capabilities.limit_pools.lower);
    assert!(!capabilities.limit_pools.previous_upper);
}
