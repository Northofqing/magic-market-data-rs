use super::{completion_status, is_diagnostic_operation, select_operation, CompletionStatus};

#[test]
fn suite_rotates_every_advertised_operation() {
    let selected = (0..super::SUITE_ATTEMPTS)
        .map(|attempt| select_operation("suite", attempt).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), super::SUITE_ATTEMPTS as usize);
    assert_eq!(selected.first(), Some(&"research-instrument"));
    assert_eq!(selected.last(), Some(&"popularity"));
    assert!(!selected.contains(&"fund-flow"));
    assert!(!selected.contains(&"news"));
}

#[test]
fn unadmitted_operations_and_failure_statuses_are_explicit() {
    assert!(is_diagnostic_operation("fund-flow"));
    assert!(is_diagnostic_operation("news"));
    assert!(!is_diagnostic_operation("research-instrument"));
    assert_eq!(
        completion_status(true, 0, 1),
        CompletionStatus::DiagnosticFailed(1)
    );
    assert_eq!(
        completion_status(true, 0, 0),
        CompletionStatus::DiagnosticCompleteUnadmitted
    );
    assert_eq!(completion_status(false, 2, 0), CompletionStatus::Failed(2));
    assert_eq!(completion_status(false, 0, 0), CompletionStatus::Admitted);
}

#[test]
fn invalid_operation_message_lists_every_explicit_diagnostic() {
    let error = select_operation("unknown", 0).unwrap_err().to_string();
    assert!(error.contains("fund-flow"));
    assert!(error.contains("news"));
}
