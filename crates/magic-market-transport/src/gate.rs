use crate::TransportError;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Clone-shareable request-start pacing primitive.
///
/// The mutex protects only reservation arithmetic. It is released before the
/// caller sleeps, and this type never performs network I/O.
#[derive(Debug)]
pub struct RequestGate {
    interval: Duration,
    next_start: Mutex<Instant>,
}

impl RequestGate {
    pub fn new(interval: Duration) -> Result<Self, TransportError> {
        if interval.is_zero() {
            return Err(TransportError::InvalidRequest(
                "request interval must be positive".into(),
            ));
        }
        Ok(Self {
            interval,
            next_start: Mutex::new(Instant::now()),
        })
    }

    pub fn reserve(&self) -> Result<Instant, TransportError> {
        let now = Instant::now();
        let mut next = self
            .next_start
            .lock()
            .map_err(|_| TransportError::Internal("request gate lock poisoned".into()))?;
        let reserved = (*next).max(now);
        *next = reserved
            .checked_add(self.interval)
            .ok_or_else(|| TransportError::ResourceLimit("request gate instant overflow".into()))?;
        Ok(reserved)
    }

    pub fn wait_for_turn(&self) -> Result<(), TransportError> {
        let reserved = self.reserve()?;
        if let Some(wait) = reserved.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        Ok(())
    }
}
