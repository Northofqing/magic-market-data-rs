use magic_imf_rs::{ImfClient, ECONOMIC_SERIES_ADMITTED};

#[test]
fn capability_matches_live_admission_flag() {
    assert_eq!(
        ImfClient::economic_data_capabilities().economic_series,
        ECONOMIC_SERIES_ADMITTED
    );
    assert!(!std::hint::black_box(ECONOMIC_SERIES_ADMITTED));
}
