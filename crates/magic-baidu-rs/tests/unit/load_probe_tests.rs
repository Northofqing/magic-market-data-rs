use super::*;

#[test]
fn load_probe_is_client_paced_and_hard_bounded() {
    assert_eq!(MIN_INTERVAL, Duration::from_secs(1));
    assert!(validate_load(1).is_ok());
    assert!(validate_load(3).is_ok());
    assert!(validate_load(0).is_err());
    assert!(validate_load(4).is_err());
    assert_eq!(percentile(&[1, 2, 3, 4, 5], 50), 3);
    assert_eq!(percentile(&[1, 2, 3, 4, 5], 95), 5);
    assert_eq!(percentile(&[1, 2, 3, 4, 5], 99), 5);
}
