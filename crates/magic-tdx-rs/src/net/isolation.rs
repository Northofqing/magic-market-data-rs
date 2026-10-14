//! Isolated test support: ONE Smart handshake and bounded quote-only query; no retry/probe/worker I/O.

use std::net::SocketAddrV4;
#[cfg(feature = "test-support")]
use std::net::{Ipv4Addr, Shutdown, TcpStream};
#[cfg(feature = "test-support")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "test-support")]
use std::sync::{Arc, MutexGuard};
use std::sync::{Mutex, TryLockError};
#[cfg(feature = "test-support")]
use std::time::{Duration, Instant};

#[cfg(feature = "test-support")]
use crate::error::Result;
use crate::error::TdxError;

#[derive(Clone, Copy, PartialEq, Eq)]
enum IsolationPhase {
    Open,
    #[cfg(feature = "test-support")]
    Quiescing,
    #[cfg(feature = "test-support")]
    Closed,
}

#[cfg(feature = "test-support")]
struct Pending {
    ticket: u64,
    epoch: u64,
    shutdown: Option<Arc<TcpStream>>,
}

struct IsolationState {
    phase: IsolationPhase,
    #[cfg(feature = "test-support")]
    epoch: u64,
    #[cfg(feature = "test-support")]
    next_ticket: u64,
    #[cfg(feature = "test-support")]
    pending: Option<Pending>,
    #[cfg(feature = "test-support")]
    owned: Option<Owned>,
    #[cfg(feature = "test-support")]
    query: Option<QueryState>,
    #[cfg(feature = "test-support")]
    draining: usize,
    #[cfg(feature = "test-support")]
    terminal_failure: Option<String>,
}

pub(super) struct IsolationScope {
    endpoints: Box<[SocketAddrV4]>,
    state: Mutex<IsolationState>,
    #[cfg(feature = "test-support")]
    stop: AtomicBool,
    #[cfg(feature = "test-support")]
    failed: AtomicBool,
    #[cfg(feature = "test-support")]
    closing: AtomicBool,
}

#[cfg(feature = "test-support")]
pub(super) fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|value| !value.is_zero())
        .ok_or(TdxError::ConnectionTimeout)
}

#[cfg(feature = "test-support")]
pub(super) fn lock_until<'a, T>(
    mutex: &'a Mutex<T>,
    deadline: Instant,
) -> Result<MutexGuard<'a, T>> {
    loop {
        remaining(deadline)?;
        match mutex.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(_)) => {
                return Err(TdxError::Connection("isolated TDX lock poisoned".into()));
            }
            Err(TryLockError::WouldBlock) => {
                std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(1)));
            }
        }
    }
}

#[cfg(feature = "test-support")]
pub(super) fn lock_cleanup_until<'a, T>(
    mutex: &'a Mutex<T>,
    deadline: Instant,
) -> Result<MutexGuard<'a, T>> {
    loop {
        // An expired operation may still settle immediately; only waiting uses
        // its original budget. This never authorizes further source I/O.
        match mutex.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(_)) => {
                return Err(TdxError::Connection(
                    "isolated TDX cleanup lock poisoned".into(),
                ));
            }
            Err(TryLockError::WouldBlock) => {
                std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(1)));
            }
        }
    }
}

#[cfg(feature = "test-support")]
pub(super) fn close_deadline() -> Result<Instant> {
    Instant::now()
        .checked_add(Duration::from_secs(5))
        .ok_or_else(|| TdxError::Connection("isolated close deadline overflow".into()))
}

impl IsolationScope {
    #[cfg(feature = "test-support")]
    pub(super) fn new(endpoints: &[SocketAddrV4]) -> Result<Arc<Self>> {
        if !(1..=2).contains(&endpoints.len()) {
            return Err(TdxError::InvalidData(
                "isolated TDX endpoints must contain 1 or 2 entries".into(),
            ));
        }
        for endpoint in endpoints {
            if endpoint.ip() != &Ipv4Addr::LOCALHOST {
                return Err(TdxError::InvalidData(
                    "isolated TDX endpoints must use 127.0.0.1".into(),
                ));
            }
            if endpoint.port() == 0 {
                return Err(TdxError::InvalidData(
                    "isolated TDX endpoint ports must be nonzero".into(),
                ));
            }
        }
        if endpoints.len() == 2 && endpoints[0] == endpoints[1] {
            return Err(TdxError::InvalidData(
                "isolated TDX endpoints must be distinct".into(),
            ));
        }
        Ok(Arc::new(Self {
            endpoints: endpoints.to_vec().into_boxed_slice(),
            state: Mutex::new(IsolationState {
                phase: IsolationPhase::Open,
                epoch: 0,
                next_ticket: 0,
                pending: None,
                owned: None,
                query: None,
                draining: 0,
                terminal_failure: None,
            }),
            stop: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            closing: AtomicBool::new(false),
        }))
    }

    pub(super) fn first_endpoint(&self) -> SocketAddrV4 {
        self.endpoints[0]
    }

    pub(super) fn runtime_error(&self) -> TdxError {
        #[cfg(feature = "test-support")]
        if self.stop.load(Ordering::SeqCst) {
            return TdxError::Disconnected;
        }
        match self.state.try_lock() {
            Ok(state) if state.phase == IsolationPhase::Open => {
                TdxError::Unsupported("isolated TDX runtime is not implemented".into())
            }
            Ok(_) => TdxError::Disconnected,
            Err(TryLockError::Poisoned(_)) => {
                TdxError::Connection("isolated TDX lifecycle lock poisoned".into())
            }
            Err(TryLockError::WouldBlock) => {
                TdxError::Connection("isolated TDX lifecycle busy".into())
            }
        }
    }

    #[cfg(feature = "test-support")]
    pub(super) fn fail_closed(&self) {
        self.failed.store(true, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);
    }

    #[cfg(feature = "test-support")]
    pub(super) fn record_failure(&self, reason: String) {
        self.fail_closed();
        // Diagnostic text is best-effort; the non-admitting flag is never optional.
        if let Ok(mut state) = self.state.try_lock() {
            state.terminal_failure.get_or_insert(reason);
        }
    }

    #[cfg(feature = "test-support")]
    pub(super) fn begin_attempt(self: &Arc<Self>, timeout: Option<f64>) -> Result<Attempt> {
        if self.stop.load(Ordering::SeqCst) {
            return Err(TdxError::Disconnected);
        }
        let seconds = timeout.unwrap_or(5.0);
        if !seconds.is_finite() || seconds <= 0.0 || seconds > 5.0 {
            return Err(TdxError::Connection(
                "isolated timeout must satisfy 0 < seconds <= 5".into(),
            ));
        }
        let duration = Duration::try_from_secs_f64(seconds).map_err(|_| {
            TdxError::Connection("isolated timeout must satisfy 0 < seconds <= 5".into())
        })?;
        if duration.is_zero() || duration > Duration::from_secs(5) {
            return Err(TdxError::Connection(
                "isolated timeout must satisfy 0 < seconds <= 5".into(),
            ));
        }
        let deadline = Instant::now()
            .checked_add(duration)
            .ok_or_else(|| TdxError::Connection("isolated operation deadline overflow".into()))?;
        if self.stop.load(Ordering::SeqCst) {
            return Err(TdxError::Disconnected);
        }
        let mut state = lock_until(&self.state, deadline)?;
        if state.phase != IsolationPhase::Open || self.stop.load(Ordering::SeqCst) {
            return Err(TdxError::Disconnected);
        }
        if state.pending.is_some()
            || state.owned.is_some()
            || state.query.is_some()
            || state.draining != 0
        {
            return Err(TdxError::Connection(
                "isolated connect already pending or connected".into(),
            ));
        }
        let ticket = state
            .next_ticket
            .checked_add(1)
            .ok_or_else(|| TdxError::Connection("isolated ticket overflow".into()))?;
        state.next_ticket = ticket;
        let epoch = state.epoch;
        state.pending = Some(Pending {
            ticket,
            epoch,
            shutdown: None,
        });
        Ok(Attempt {
            scope: Arc::clone(self),
            ticket,
            epoch,
            deadline,
            released: false,
        })
    }

    #[cfg(feature = "test-support")]
    pub(super) fn begin_close(&self, deadline: Instant) -> Result<Option<CloseOwner<'_>>> {
        // Admission closes before waiting for any lifecycle/pool/metadata mutex.
        self.stop.store(true, Ordering::SeqCst);
        loop {
            if self
                .closing
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(1)));
                continue;
            }
            let owner = CloseOwner { scope: self };
            let mut state = lock_until(&self.state, deadline)?;
            match state.phase {
                IsolationPhase::Closed => {
                    if self.failed.load(Ordering::SeqCst) {
                        return Err(TdxError::Connection(
                            "prior isolated cleanup failure".into(),
                        ));
                    }
                    return Ok(None);
                }
                IsolationPhase::Open => {
                    state.epoch = state
                        .epoch
                        .checked_add(1)
                        .ok_or_else(|| TdxError::Connection("isolated epoch overflow".into()))?;
                    state.phase = IsolationPhase::Quiescing;
                    return Ok(Some(owner));
                }
                IsolationPhase::Quiescing => {
                    // Retry cleanup after a prior failed close; failure remains sticky.
                    return Ok(Some(owner));
                }
            }
        }
    }

    #[cfg(feature = "test-support")]
    pub(super) fn shutdown_registered(&self, deadline: Instant) -> Result<()> {
        let stream = loop {
            let mut state = lock_until(&self.state, deadline)?;
            if state.phase != IsolationPhase::Quiescing {
                return Err(TdxError::Connection(
                    "isolated close ownership invalid".into(),
                ));
            }
            if state.draining != 0 {
                drop(state);
                std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(1)));
                continue;
            }
            let pending = state
                .pending
                .as_mut()
                .and_then(|pending| pending.shutdown.take());
            if pending.is_some() && state.owned.is_some() {
                self.fail_closed();
                // This invariant cannot arise through begin_attempt/commit.
                drop(state);
                drop(pending);
                return Err(TdxError::Connection(
                    "isolated duplicate stream ownership".into(),
                ));
            }
            let stream = pending.or_else(|| state.owned.take().map(|owned| owned.shutdown));
            state.draining = usize::from(stream.is_some());
            break stream;
        };
        let result = match stream {
            Some(stream) => {
                let result = stream.shutdown(Shutdown::Both).map_err(|error| {
                    TdxError::Connection(format!("isolated shutdown failed: {error}"))
                });
                drop(stream);
                result
            }
            None => Ok(()),
        };
        let mut state = lock_until(&self.state, deadline)?;
        state.draining = 0;
        if result.is_err() {
            self.fail_closed();
            if let Err(error) = &result {
                state.terminal_failure.get_or_insert(error.to_string());
            }
        }
        result
    }

    #[cfg(feature = "test-support")]
    pub(super) fn wait_pending(&self, deadline: Instant) -> Result<()> {
        loop {
            let state = lock_until(&self.state, deadline)?;
            if state.pending.is_none() && state.query.is_none() && state.draining == 0 {
                return Ok(());
            }
            drop(state);
            std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(1)));
        }
    }

    #[cfg(feature = "test-support")]
    pub(super) fn close_guard(&self, deadline: Instant) -> Result<ClosePermit<'_>> {
        let state = lock_until(&self.state, deadline)?;
        if state.phase != IsolationPhase::Quiescing
            || state.pending.is_some()
            || state.owned.is_some()
            || state.query.is_some()
            || state.draining != 0
        {
            return Err(TdxError::Connection(
                "isolated close has residual ownership".into(),
            ));
        }
        Ok(ClosePermit { state })
    }

    #[cfg(feature = "test-support")]
    pub(super) fn finish_drain(&self, deadline: Instant) -> Result<()> {
        let mut state = lock_until(&self.state, deadline)?;
        if state.draining != 1 {
            return Err(TdxError::Connection(
                "isolated drain accounting invalid".into(),
            ));
        }
        state.draining = 0;
        Ok(())
    }

    #[cfg(feature = "test-support")]
    pub(super) fn finish_close(&self, deadline: Instant) -> Result<()> {
        let mut state = lock_until(&self.state, deadline)?;
        if self.failed.load(Ordering::SeqCst)
            || state.phase != IsolationPhase::Quiescing
            || state.pending.is_some()
            || state.owned.is_some()
            || state.query.is_some()
            || state.draining != 0
        {
            return Err(TdxError::Connection(format!(
                "isolated close incomplete: {:?}",
                state.terminal_failure
            )));
        }
        state.phase = IsolationPhase::Closed;
        Ok(())
    }
}

#[cfg(feature = "test-support")]
pub(super) struct ClosePermit<'a> {
    state: MutexGuard<'a, IsolationState>,
}

#[cfg(feature = "test-support")]
pub(super) struct CloseOwner<'a> {
    scope: &'a IsolationScope,
}

#[cfg(feature = "test-support")]
impl Drop for CloseOwner<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.scope.fail_closed();
        }
        self.scope.closing.store(false, Ordering::SeqCst);
    }
}

#[cfg(feature = "test-support")]
impl ClosePermit<'_> {
    pub(super) fn track_pool_drain(&mut self) {
        // close_guard proves zero; caller transfers exactly one real pool stream.
        self.state.draining = 1;
    }
}

#[cfg(feature = "test-support")]
pub(super) struct Attempt {
    pub(super) scope: Arc<IsolationScope>,
    ticket: u64,
    epoch: u64,
    pub(super) deadline: Instant,
    released: bool,
}

#[cfg(feature = "test-support")]
impl Attempt {
    pub(super) fn endpoints(&self) -> &[SocketAddrV4] {
        &self.scope.endpoints
    }

    pub(super) fn remaining(&self) -> Result<Duration> {
        let state = lock_until(&self.scope.state, self.deadline)?;
        self.check(&state)?;
        remaining(self.deadline)
    }

    fn check(&self, state: &IsolationState) -> Result<()> {
        if self.scope.stop.load(Ordering::SeqCst)
            || state.phase != IsolationPhase::Open
            || state.epoch != self.epoch
            || !state
                .pending
                .as_ref()
                .is_some_and(|pending| pending.ticket == self.ticket && pending.epoch == self.epoch)
        {
            return Err(TdxError::Disconnected);
        }
        remaining(self.deadline)?;
        Ok(())
    }

    pub(super) fn register(&self, stream: Arc<TcpStream>) -> Result<()> {
        let mut stream = Some(stream);
        let result = (|| {
            let mut state = lock_until(&self.scope.state, self.deadline)?;
            self.check(&state)?;
            let pending = state.pending.as_mut().ok_or(TdxError::Disconnected)?;
            if pending.shutdown.is_some() {
                return Err(TdxError::Connection(
                    "isolated stream already registered".into(),
                ));
            }
            pending.shutdown = stream.take();
            Ok(())
        })();
        if let Some(stream) = stream {
            let cleanup = stream.shutdown(Shutdown::Both);
            drop(stream);
            if let Err(cleanup) = cleanup {
                self.scope.fail_closed();
                return Err(TdxError::Connection(format!(
                    "isolated registration failed: {result:?}; shutdown: {cleanup}"
                )));
            }
        }
        result
    }

    pub(super) fn io_key(&self) -> (u64, u64) {
        (self.ticket, self.epoch)
    }

    pub(super) fn publish_guard(&self) -> Result<PublishPermit<'_>> {
        let state = lock_until(&self.scope.state, self.deadline)?;
        self.check(&state)?;
        if state.owned.is_some()
            || state.draining != 0
            || !state
                .pending
                .as_ref()
                .is_some_and(|pending| pending.shutdown.is_some())
        {
            return Err(TdxError::Connection(
                "isolated publication ownership invalid".into(),
            ));
        }
        Ok(PublishPermit { state })
    }

    pub(super) fn released_after_commit(&mut self) {
        self.released = true;
    }

    pub(super) fn reset_stream(&self) -> Result<()> {
        let stream =
            {
                // The operation may already be timed out or cancelled. Cleanup
                // does not reopen admission or acquire a fresh operation budget.
                let mut state =
                    self.scope.state.try_lock().map_err(|_| {
                        TdxError::Connection("isolated reset lock unavailable".into())
                    })?;
                if !state.pending.as_ref().is_some_and(|pending| {
                    pending.ticket == self.ticket && pending.epoch == self.epoch
                }) {
                    return Err(TdxError::Connection("isolated reset ticket lost".into()));
                }
                state
                    .pending
                    .as_mut()
                    .and_then(|pending| pending.shutdown.take())
            };
        if let Some(stream) = stream {
            let result = stream.shutdown(Shutdown::Both).map_err(|error| {
                TdxError::Connection(format!("isolated attempt shutdown failed: {error}"))
            });
            drop(stream);
            if result.is_err() {
                self.scope.fail_closed();
            }
            result?;
        }
        Ok(())
    }
}

#[cfg(feature = "test-support")]
pub(super) struct PublishPermit<'a> {
    state: MutexGuard<'a, IsolationState>,
}

#[cfg(feature = "test-support")]
impl PublishPermit<'_> {
    pub(super) fn commit(&mut self) {
        // Caller holds all publication locks, checks deadline and stop once more.
        // All fallible allocations/checks precede these infallible field moves.
        if let Some(pending) = self.state.pending.take() {
            self.state.owned = pending.shutdown.map(|shutdown| Owned {
                ticket: pending.ticket,
                epoch: pending.epoch,
                shutdown,
            });
        }
    }
}

#[cfg(feature = "test-support")]
impl IsolationScope {
    pub(super) fn io_remaining(
        &self,
        ticket: u64,
        epoch: u64,
        deadline: Instant,
        query: Option<u64>,
    ) -> std::io::Result<Duration> {
        let result = (|| {
            let state = lock_until(&self.state, deadline)?;
            if self.stop.load(Ordering::SeqCst)
                || state.phase != IsolationPhase::Open
                || state.epoch != epoch
                || match query {
                    None => !state
                        .pending
                        .as_ref()
                        .is_some_and(|pending| pending.ticket == ticket && pending.epoch == epoch),
                    Some(id) => {
                        !state.query.as_ref().is_some_and(|active| {
                            active.id == id
                                && active.connection_ticket == ticket
                                && active.epoch == epoch
                                && active.deadline == deadline
                        }) || !state
                            .owned
                            .as_ref()
                            .is_some_and(|owned| owned.ticket == ticket && owned.epoch == epoch)
                    }
                }
            {
                return Err(TdxError::Disconnected);
            }
            remaining(deadline)
        })();
        result.map_err(|error| std::io::Error::other(error.to_string()))
    }

    pub(super) fn validate_commit(&self, deadline: Instant) -> Result<()> {
        // Called only while publication/settlement holds state; never re-lock recursively.
        if self.stop.load(Ordering::SeqCst) {
            return Err(TdxError::Disconnected);
        }
        remaining(deadline)?;
        Ok(())
    }
}

#[cfg(feature = "test-support")]
impl Drop for Attempt {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        // Caller must drop unpublished primary TcpConnection before this lease.
        // No panic/expect/blocking lock, and residual ownership cannot be Closed.
        let result = (|| {
            let stream = {
                let mut state = self.scope.state.try_lock().map_err(|_| {
                    TdxError::Connection("isolated pending cleanup lock unavailable".into())
                })?;
                let pending = state
                    .pending
                    .as_ref()
                    .filter(|pending| pending.ticket == self.ticket)
                    .ok_or_else(|| TdxError::Connection("isolated pending ticket lost".into()))?;
                let next_draining = if pending.shutdown.is_some() {
                    state
                        .draining
                        .checked_add(1)
                        .ok_or_else(|| TdxError::Connection("isolated drain overflow".into()))?
                } else {
                    state.draining
                };
                let stream = state
                    .pending
                    .as_mut()
                    .and_then(|pending| pending.shutdown.take());
                state.draining = next_draining;
                stream
            };
            let had_stream = stream.is_some();
            let cleanup = stream.map(|stream| {
                let result = stream.shutdown(Shutdown::Both);
                drop(stream);
                result
            });
            let mut state = self.scope.state.try_lock().map_err(|_| {
                TdxError::Connection("isolated pending settlement lock unavailable".into())
            })?;
            if !state
                .pending
                .as_ref()
                .is_some_and(|pending| pending.ticket == self.ticket)
            {
                return Err(TdxError::Connection(
                    "isolated pending settlement mismatch".into(),
                ));
            }
            state.pending = None;
            if had_stream {
                state.draining = state
                    .draining
                    .checked_sub(1)
                    .ok_or_else(|| TdxError::Connection("isolated drain underflow".into()))?;
            }
            if let Some(Err(error)) = cleanup {
                return Err(TdxError::Connection(format!(
                    "isolated pending shutdown failed: {error}"
                )));
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.scope.record_failure(error.to_string());
        }
        if std::thread::panicking() {
            self.scope.record_failure("isolated connect unwound".into());
        }
    }
}

#[cfg(feature = "test-support")]
struct Owned {
    ticket: u64,
    epoch: u64,
    shutdown: Arc<TcpStream>,
}

#[cfg(feature = "test-support")]
struct QueryState {
    id: u64,
    connection_ticket: u64,
    epoch: u64,
    deadline: Instant,
}

/// Private quote-only lease, never a public admission or new endpoint.
#[cfg(feature = "test-support")]
pub(super) struct QueryPermit {
    pub(super) scope: Arc<IsolationScope>,
    pub(super) deadline: Instant,
    id: u64,
    connection_ticket: u64,
    epoch: u64,
    released: bool,
}

#[cfg(feature = "test-support")]
impl IsolationScope {
    pub(super) fn begin_query(self: &Arc<Self>) -> Result<QueryPermit> {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(5))
            .ok_or_else(|| TdxError::Connection("isolated query deadline overflow".into()))?;
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return Err(TdxError::Disconnected);
            }
            let mut state = lock_until(&self.state, deadline)?;
            if state.phase != IsolationPhase::Open || self.stop.load(Ordering::SeqCst) {
                return Err(TdxError::Disconnected);
            }
            if state.pending.is_some() || state.draining != 0 {
                return Err(TdxError::Connection("isolated query ownership busy".into()));
            }
            let owned = state.owned.as_ref().ok_or_else(|| {
                TdxError::Unsupported("isolated TDX runtime is not implemented".into())
            })?;
            let (connection_ticket, epoch) = (owned.ticket, owned.epoch);
            if epoch != state.epoch {
                return Err(TdxError::Disconnected);
            }
            if state.query.is_some() {
                drop(state);
                std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(1)));
                continue;
            }
            let id = state
                .next_ticket
                .checked_add(1)
                .ok_or_else(|| TdxError::Connection("isolated query ticket overflow".into()))?;
            state.next_ticket = id;
            state.query = Some(QueryState {
                id,
                connection_ticket,
                epoch,
                deadline,
            });
            return Ok(QueryPermit {
                scope: Arc::clone(self),
                deadline,
                id,
                connection_ticket,
                epoch,
                released: false,
            });
        }
    }

    pub(super) fn finish_query_drain(&self, deadline: Instant) -> Result<()> {
        let mut state = lock_cleanup_until(&self.state, deadline)?;
        if state.draining != 1 {
            return Err(TdxError::Connection(
                "isolated query drain accounting invalid".into(),
            ));
        }
        state.draining = 0;
        Ok(())
    }
}

#[cfg(feature = "test-support")]
impl QueryPermit {
    fn matches(&self, state: &IsolationState) -> bool {
        state.query.as_ref().is_some_and(|query| {
            query.id == self.id
                && query.connection_ticket == self.connection_ticket
                && query.epoch == self.epoch
                && query.deadline == self.deadline
        })
    }

    pub(super) fn remaining(&self) -> Result<Duration> {
        let state = lock_until(&self.scope.state, self.deadline)?;
        if self.scope.stop.load(Ordering::SeqCst)
            || state.phase != IsolationPhase::Open
            || state.epoch != self.epoch
            || !self.matches(&state)
            || !state.owned.as_ref().is_some_and(|owned| {
                owned.ticket == self.connection_ticket && owned.epoch == self.epoch
            })
        {
            return Err(TdxError::Disconnected);
        }
        remaining(self.deadline)
    }

    pub(super) fn authorize(
        &self,
        scope: &Arc<IsolationScope>,
        ticket: u64,
        epoch: u64,
    ) -> Result<u64> {
        if !Arc::ptr_eq(scope, &self.scope)
            || ticket != self.connection_ticket
            || epoch != self.epoch
        {
            return Err(TdxError::Connection(
                "isolated query stream identity mismatch".into(),
            ));
        }
        self.remaining()?;
        Ok(self.id)
    }

    pub(super) fn allows_server(&self, server: &(String, u16)) -> bool {
        self.scope
            .endpoints
            .iter()
            .any(|endpoint| server.0 == endpoint.ip().to_string() && server.1 == endpoint.port())
    }

    pub(super) fn settlement(&self) -> Result<QuerySettlement<'_>> {
        // Cleanup does not acquire a fresh query budget or wait past the old one.
        let state = loop {
            let state = lock_cleanup_until(&self.scope.state, self.deadline)?;
            if !self.matches(&state) {
                return Err(TdxError::Connection(
                    "isolated query settlement identity invalid".into(),
                ));
            }
            if state.draining == 0 {
                break state;
            }
            // A close owner may be shutting down the registered clone to wake
            // this query. Let that exact drain settle before clearing the lease.
            drop(state);
            std::thread::sleep(remaining(self.deadline)?.min(Duration::from_millis(1)));
        };
        let reusable = state.phase == IsolationPhase::Open
            && !self.scope.stop.load(Ordering::SeqCst)
            && state.epoch == self.epoch
            && remaining(self.deadline).is_ok()
            && state.owned.as_ref().is_some_and(|owned| {
                owned.ticket == self.connection_ticket && owned.epoch == self.epoch
            });
        Ok(QuerySettlement { state, reusable })
    }

    pub(super) fn released(&mut self) {
        self.released = true;
    }
}

#[cfg(feature = "test-support")]
pub(super) struct QuerySettlement<'a> {
    state: MutexGuard<'a, IsolationState>,
    pub(super) reusable: bool,
}

#[cfg(feature = "test-support")]
impl QuerySettlement<'_> {
    pub(super) fn complete(&mut self, reusable: bool) -> Option<Arc<TcpStream>> {
        let owned = if reusable {
            None
        } else {
            self.state.owned.take().map(|owned| owned.shutdown)
        };
        self.state.query = None;
        self.state.draining = usize::from(owned.is_some());
        owned
    }
}

#[cfg(feature = "test-support")]
impl Drop for QueryPermit {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        let result = (|| {
            let mut state = self.scope.state.try_lock().map_err(|_| {
                TdxError::Connection("isolated query release lock unavailable".into())
            })?;
            if !self.matches(&state) {
                return Err(TdxError::Connection(
                    "isolated query release ticket lost".into(),
                ));
            }
            state.query = None;
            Ok(())
        })();
        if let Err(error) = result {
            self.scope.record_failure(error.to_string());
        }
        if std::thread::panicking() {
            self.scope.record_failure("isolated query unwound".into());
        }
    }
}
