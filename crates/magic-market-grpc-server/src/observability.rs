use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(crate) struct RuntimeObservability {
    process_started_at_unix_ms: u64,
    process_started: Instant,
    query_started: AtomicU64,
    query_succeeded: AtomicU64,
    query_failed: AtomicU64,
    query_cancelled: AtomicU64,
    query_in_flight: AtomicU64,
    query_rejected: AtomicU64,
    query_timed_out: AtomicU64,
    query_duration_micros_total: AtomicU64,
    query_duration_micros_max: AtomicU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueryOutcome {
    Succeeded,
    Failed,
    Rejected,
    TimedOut,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeSnapshot {
    pub(crate) process_started_at_unix_ms: u64,
    pub(crate) uptime_millis: u64,
    pub(crate) query_started: u64,
    pub(crate) query_succeeded: u64,
    pub(crate) query_failed: u64,
    pub(crate) query_cancelled: u64,
    pub(crate) query_in_flight: u64,
    pub(crate) query_rejected: u64,
    pub(crate) query_timed_out: u64,
    pub(crate) query_duration_micros_total: u64,
    pub(crate) query_duration_micros_max: u64,
}

pub(crate) struct QueryObservation<'a> {
    observability: &'a RuntimeObservability,
    started: Instant,
    finished: bool,
}

impl RuntimeObservability {
    pub(crate) fn new() -> Self {
        let process_started_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
            .unwrap_or_default();
        Self {
            process_started_at_unix_ms,
            process_started: Instant::now(),
            query_started: AtomicU64::new(0),
            query_succeeded: AtomicU64::new(0),
            query_failed: AtomicU64::new(0),
            query_cancelled: AtomicU64::new(0),
            query_in_flight: AtomicU64::new(0),
            query_rejected: AtomicU64::new(0),
            query_timed_out: AtomicU64::new(0),
            query_duration_micros_total: AtomicU64::new(0),
            query_duration_micros_max: AtomicU64::new(0),
        }
    }

    pub(crate) fn observe_query(&self) -> QueryObservation<'_> {
        saturating_increment(&self.query_started);
        saturating_increment(&self.query_in_flight);
        QueryObservation {
            observability: self,
            started: Instant::now(),
            finished: false,
        }
    }

    pub(crate) fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            process_started_at_unix_ms: self.process_started_at_unix_ms,
            uptime_millis: duration_millis(self.process_started.elapsed()),
            query_started: self.query_started.load(Ordering::Relaxed),
            query_succeeded: self.query_succeeded.load(Ordering::Relaxed),
            query_failed: self.query_failed.load(Ordering::Relaxed),
            query_cancelled: self.query_cancelled.load(Ordering::Relaxed),
            query_in_flight: self.query_in_flight.load(Ordering::Relaxed),
            query_rejected: self.query_rejected.load(Ordering::Relaxed),
            query_timed_out: self.query_timed_out.load(Ordering::Relaxed),
            query_duration_micros_total: self.query_duration_micros_total.load(Ordering::Relaxed),
            query_duration_micros_max: self.query_duration_micros_max.load(Ordering::Relaxed),
        }
    }

    fn finish_query(&self, started: Instant, outcome: Option<QueryOutcome>) {
        let previous = self.query_in_flight.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(previous > 0, "query in-flight telemetry underflow");
        let duration_micros = duration_micros(started.elapsed());
        saturating_add(&self.query_duration_micros_total, duration_micros);
        self.query_duration_micros_max
            .fetch_max(duration_micros, Ordering::Relaxed);
        match outcome {
            Some(QueryOutcome::Succeeded) => saturating_increment(&self.query_succeeded),
            Some(QueryOutcome::Rejected) => {
                saturating_increment(&self.query_failed);
                saturating_increment(&self.query_rejected);
            }
            Some(QueryOutcome::TimedOut) => {
                saturating_increment(&self.query_failed);
                saturating_increment(&self.query_timed_out);
            }
            Some(QueryOutcome::Failed) => saturating_increment(&self.query_failed),
            None => {
                saturating_increment(&self.query_failed);
                saturating_increment(&self.query_cancelled);
            }
        }
    }
}

impl QueryObservation<'_> {
    pub(crate) fn finish(mut self, outcome: QueryOutcome) {
        self.observability.finish_query(self.started, Some(outcome));
        self.finished = true;
    }
}

impl Drop for QueryObservation<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.observability.finish_query(self.started, None);
        }
    }
}

fn saturating_increment(counter: &AtomicU64) {
    saturating_add(counter, 1);
}

fn saturating_add(counter: &AtomicU64, value: u64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(value))
    });
}

fn duration_millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn duration_micros(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observations_track_success_failure_timeout_rejection_and_cancellation() {
        let telemetry = RuntimeObservability::new();
        telemetry.observe_query().finish(QueryOutcome::Succeeded);
        telemetry.observe_query().finish(QueryOutcome::Failed);
        telemetry.observe_query().finish(QueryOutcome::Rejected);
        telemetry.observe_query().finish(QueryOutcome::TimedOut);
        drop(telemetry.observe_query());

        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.query_started, 5);
        assert_eq!(snapshot.query_succeeded, 1);
        assert_eq!(snapshot.query_failed, 4);
        assert_eq!(snapshot.query_rejected, 1);
        assert_eq!(snapshot.query_timed_out, 1);
        assert_eq!(snapshot.query_cancelled, 1);
        assert_eq!(snapshot.query_in_flight, 0);
        assert!(snapshot.query_duration_micros_total >= snapshot.query_duration_micros_max);
    }

    #[test]
    fn in_flight_gauge_is_visible_before_completion() {
        let telemetry = RuntimeObservability::new();
        let observation = telemetry.observe_query();
        assert_eq!(telemetry.snapshot().query_in_flight, 1);
        observation.finish(QueryOutcome::Succeeded);
        assert_eq!(telemetry.snapshot().query_in_flight, 0);
    }
}
