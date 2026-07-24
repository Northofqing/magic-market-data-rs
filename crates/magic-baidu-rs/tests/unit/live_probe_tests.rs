use super::probe_status;
use magic_market_core::ProbeStatus;

#[test]
fn unproved_baidu_continuity_gates_cannot_be_admitted() {
    assert_eq!(
        probe_status(true, false, true, true, true),
        ProbeStatus::DiagnosticCompleteUnadmitted
    );
    assert_eq!(
        probe_status(true, true, false, true, true),
        ProbeStatus::DiagnosticCompleteUnadmitted
    );
    assert_eq!(
        probe_status(true, true, true, false, true),
        ProbeStatus::DiagnosticCompleteUnadmitted
    );
    assert_eq!(
        probe_status(true, true, true, true, false),
        ProbeStatus::DiagnosticCompleteUnadmitted
    );
}

#[test]
fn admission_requires_both_a_declared_capability_and_every_gate() {
    assert_eq!(
        probe_status(true, true, true, true, true),
        ProbeStatus::Admitted
    );
    assert_eq!(
        probe_status(false, true, true, true, true),
        ProbeStatus::DiagnosticCompleteUnadmitted
    );
}
