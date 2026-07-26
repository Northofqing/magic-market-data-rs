use super::*;

#[test]
fn load_probe_rotates_all_advertised_families() {
    let operations = (0..4).map(select_operation).collect::<Vec<_>>();
    assert_eq!(
        operations,
        vec![
            "consensus",
            "strong_stock_reasons",
            "upper_limit_pool",
            "popularity"
        ]
    );
}

#[test]
fn load_probe_bounds_require_one_request_per_family() {
    assert!(!(MIN_REQUESTS..=MAX_REQUESTS).contains(&(MIN_REQUESTS - 1)));
    assert!((MIN_REQUESTS..=MAX_REQUESTS).contains(&MIN_REQUESTS));
    assert!((MIN_REQUESTS..=MAX_REQUESTS).contains(&MAX_REQUESTS));
    assert!(!(MIN_REQUESTS..=MAX_REQUESTS).contains(&(MAX_REQUESTS + 1)));
}
