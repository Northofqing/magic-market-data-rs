use super::combined_probe_status;
use magic_market_core::ProbeStatus;

#[test]
fn cninfo_requires_both_advertised_families_to_pass() {
    assert_eq!(
        combined_probe_status(true, true, ProbeStatus::Admitted, ProbeStatus::Admitted),
        ProbeStatus::Admitted
    );
    assert_eq!(
        combined_probe_status(true, false, ProbeStatus::Admitted, ProbeStatus::Admitted),
        ProbeStatus::DiagnosticCompleteUnadmitted
    );
    assert_eq!(
        combined_probe_status(true, true, ProbeStatus::Admitted, ProbeStatus::Failed),
        ProbeStatus::Failed
    );
}
