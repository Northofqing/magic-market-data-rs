use super::declared_admission_status;
use magic_market_core::ProbeStatus;

#[test]
fn a_verified_batch_cannot_promote_an_unadvertised_cls_family() {
    assert_eq!(
        declared_admission_status(false, ProbeStatus::Admitted),
        ProbeStatus::DiagnosticCompleteUnadmitted
    );
    assert_eq!(
        declared_admission_status(true, ProbeStatus::Admitted),
        ProbeStatus::Admitted
    );
}
