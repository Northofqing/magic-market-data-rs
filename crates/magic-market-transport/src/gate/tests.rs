use super::*;
use std::sync::Arc;

#[test]
fn zero_interval_and_poisoned_reservation_lock_are_typed() {
    assert!(matches!(
        RequestGate::new(Duration::ZERO),
        Err(TransportError::InvalidRequest(_))
    ));

    let gate = Arc::new(RequestGate::new(Duration::from_millis(1)).unwrap());
    let poison = Arc::clone(&gate);
    assert!(std::thread::spawn(move || {
        let _guard = poison.next_start.lock().unwrap();
        panic!("poison reservation lock");
    })
    .join()
    .is_err());
    assert!(matches!(
        gate.reserve(),
        Err(TransportError::Internal(message)) if message.contains("lock poisoned")
    ));
}

#[test]
fn poisoned_actual_start_lock_is_typed() {
    let gate = Arc::new(RequestGate::new(Duration::from_millis(1)).unwrap());
    let poison = Arc::clone(&gate);
    assert!(std::thread::spawn(move || {
        let _guard = poison.next_actual_start.lock().unwrap();
        panic!("poison actual-start lock");
    })
    .join()
    .is_err());
    assert!(matches!(
        gate.wait_for_turn(),
        Err(TransportError::Internal(message)) if message.contains("start lock poisoned")
    ));
}

#[test]
fn wait_until_accepts_past_and_current_instants() {
    wait_until(Instant::now() - Duration::from_millis(1));
    wait_until(Instant::now());
}
