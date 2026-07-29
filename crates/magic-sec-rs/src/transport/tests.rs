use super::*;

#[test]
fn submissions_url_allowlist_is_exact() {
    for valid in [
        "https://data.sec.gov/submissions/CIK0000320193.json",
        "https://data.sec.gov/submissions/CIK0000320193-submissions-001.json",
    ] {
        assert!(validate_exact_submissions_url(valid).is_ok(), "{valid}");
    }
    for invalid in [
        "http://data.sec.gov/submissions/CIK0000320193.json",
        "https://www.sec.gov/submissions/CIK0000320193.json",
        "https://data.sec.gov/submissions/CIK320193.json",
        "https://data.sec.gov/submissions/CIK0000320193-submissions-01.json",
        "https://data.sec.gov/submissions/../Archives/file.json",
        "https://data.sec.gov/submissions/CIK0000320193.json?x=1",
    ] {
        assert!(
            validate_exact_submissions_url(invalid).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn status_403_is_authentication_and_429_remains_transport() {
    assert!(matches!(
        map_transport_error(TransportError::HttpStatus { status: 403 }),
        SecEdgarError::Authentication(_)
    ));
    assert!(matches!(
        map_transport_error(TransportError::HttpStatus { status: 429 }),
        SecEdgarError::Transport(TransportError::HttpStatus { status: 429 })
    ));
}
