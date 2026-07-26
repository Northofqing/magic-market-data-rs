use super::successful_probe_status;
use magic_market_core::ProbeStatus;

#[test]
fn successful_diagnostic_does_not_admit_a_false_capability() {
    assert_eq!(
        successful_probe_status(false),
        ProbeStatus::DiagnosticCompleteUnadmitted
    );
    assert_eq!(successful_probe_status(true), ProbeStatus::Admitted);
}
