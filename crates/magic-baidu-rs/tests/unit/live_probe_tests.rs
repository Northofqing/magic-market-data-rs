use super::probe_status;
use magic_market_core::ProbeStatus;

#[test]
fn technical_bar_probe_follows_the_repository_admission() {
    assert_eq!(probe_status(true), ProbeStatus::Admitted);
    assert_eq!(
        probe_status(false),
        ProbeStatus::DiagnosticCompleteUnadmitted
    );
}
