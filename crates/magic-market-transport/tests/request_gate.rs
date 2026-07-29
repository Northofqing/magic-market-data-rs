use magic_market_transport::RequestGate;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

#[test]
fn reservation_lock_is_not_held_during_wait_or_io() {
    let gate = Arc::new(RequestGate::new(Duration::from_millis(40)).unwrap());
    gate.wait_for_turn().unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let second = {
        let gate = Arc::clone(&gate);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            let started = Instant::now();
            gate.wait_for_turn().unwrap();
            started.elapsed()
        })
    };
    barrier.wait();
    let reservation_started = Instant::now();
    let _third_reservation = gate.reserve().unwrap();
    assert!(reservation_started.elapsed() < Duration::from_millis(20));
    assert!(second.join().unwrap() >= Duration::from_millis(35));
}
