use super::*;

#[test]
fn load_probe_rotates_all_advertised_families() {
    assert_eq!(select_operation(0), "announcements");
    assert_eq!(select_operation(1), "investor_questions");
    assert_eq!(select_operation(2), "announcements");
}

#[test]
fn load_probe_bounds_require_one_request_per_family() {
    assert!(!(2..=MAX_REQUESTS).contains(&1));
    assert!((2..=MAX_REQUESTS).contains(&2));
    assert!((2..=MAX_REQUESTS).contains(&MAX_REQUESTS));
    assert!(!(2..=MAX_REQUESTS).contains(&(MAX_REQUESTS + 1)));
}
