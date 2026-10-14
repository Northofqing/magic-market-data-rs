use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::error::Result;
use crate::loge;
use crate::net::connection::TcpConnection;
use crate::net::isolation::IsolationScope;
use crate::protocol::constants::{CONNECT_TIMEOUT, DEFAULT_POOL_SIZE};

/// Setup callback executed after a new connection is established.
pub type HandshakeCallback = Box<dyn Fn(&mut TcpConnection) -> Result<()> + Send + Sync>;

/// 连接池中的单个连接
struct PooledConnection {
    conn: TcpConnection,
    server: (String, u16),
    generation: Arc<()>,
}

/// 连接池配置
pub struct PoolConfig {
    pub max_size: usize,
    pub connect_timeout: f64,
    /// 握手回调: 新建连接后执行 (setup commands)
    pub handshake_fn: Option<HandshakeCallback>,
}

impl PoolConfig {
    pub fn new() -> Self {
        Self {
            max_size: DEFAULT_POOL_SIZE,
            connect_timeout: CONNECT_TIMEOUT,
            handshake_fn: None,
        }
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// 线程安全的连接池
///
/// 管理多个 TCP 连接，支持:
/// - 单服务器连接池 (多个连接到同一服务器)
/// - 多服务器连接池 (连接到不同服务器)
/// - 连接借出/归还
pub struct ConnectionPool {
    inner: Mutex<PoolInner>,
    config: PoolConfig,
    isolation: Option<Arc<IsolationScope>>,
}

struct PoolInner {
    idle: VecDeque<PooledConnection>,
    active: usize,
    total: usize,
    generation: Arc<()>,
}

impl ConnectionPool {
    /// 创建连接池 (单服务器)
    pub fn new_single(_server: (String, u16), config: PoolConfig) -> Self {
        Self::new_single_with_isolation(_server, config, None)
    }

    pub(super) fn new_single_with_isolation(
        _server: (String, u16),
        config: PoolConfig,
        isolation: Option<Arc<IsolationScope>>,
    ) -> Self {
        Self {
            inner: Mutex::new(PoolInner {
                idle: VecDeque::new(),
                active: 0,
                total: 0,
                generation: Arc::new(()),
            }),
            config,
            isolation,
        }
    }

    fn reject_isolated_runtime(&self) -> Result<()> {
        match &self.isolation {
            Some(scope) => Err(scope.runtime_error()),
            None => Ok(()),
        }
    }

    #[cfg(feature = "test-support")]
    pub(super) fn check_isolated_idle(
        &self,
        scope: &Arc<IsolationScope>,
        deadline: std::time::Instant,
    ) -> Result<()> {
        let inner = crate::net::isolation::lock_until(&self.inner, deadline)?;
        if !self
            .isolation
            .as_ref()
            .is_some_and(|own| Arc::ptr_eq(own, scope))
            || !inner.idle.is_empty()
            || inner.active != 0
            || inner.total != 0
        {
            return Err(crate::TdxError::Connection(
                "isolated pool idle-close invariant failed".into(),
            ));
        }
        Ok(())
    }

    /// 将一个已握手的连接放入池中
    pub fn push(&self, conn: TcpConnection, server: (String, u16)) {
        if self.isolation.is_some() {
            // NOT IMPLEMENTED: refuse adoption; shut down and release the socket.
            let mut conn = conn;
            conn.close();
            drop(conn);
            return;
        }
        let mut inner = crate::sync::lock_recover(&self.inner, "connection pool");
        inner.total += 1;
        let generation = Arc::clone(&inner.generation);
        inner.idle.push_back(PooledConnection {
            conn,
            server,
            generation,
        });
    }

    /// 从池中借出一个连接
    ///
    /// 如果池中有空闲连接，返回一个；
    /// 如果未达上限，创建新连接；
    /// 如果已满，返回错误。
    pub fn borrow(&self, server: &(String, u16)) -> Result<PooledConnGuard<'_>> {
        self.reject_isolated_runtime()?;
        let mut inner = crate::sync::lock(&self.inner, "connection pool")?;

        // 尝试从空闲队列获取
        if let Some(conn) = inner.idle.pop_front() {
            inner.active += 1;
            return Ok(PooledConnGuard {
                pool: self,
                conn: Some(conn),
            });
        }

        // 如果未达到上限，创建新连接
        if inner.total < self.config.max_size {
            let server_clone = server.clone();
            let has_handshake = self.config.handshake_fn.is_some();
            let generation = Arc::clone(&inner.generation);
            inner.total += 1;
            inner.active += 1;

            // 释放锁后再创建连接 (避免持锁做 I/O)
            drop(inner);

            let mut conn = match TcpConnection::connect(
                &server_clone.0,
                server_clone.1,
                self.config.connect_timeout,
            ) {
                Ok(conn) => conn,
                Err(error) => {
                    self.release_reservation();
                    return Err(error);
                }
            };

            // 执行握手 (如果有)
            if has_handshake {
                if let Some(ref handshake_fn) = self.config.handshake_fn {
                    if let Err(error) = handshake_fn(&mut conn) {
                        conn.close();
                        self.release_reservation();
                        return Err(error);
                    }
                }
            }

            return Ok(PooledConnGuard {
                pool: self,
                conn: Some(PooledConnection {
                    conn,
                    server: server_clone,
                    generation,
                }),
            });
        }

        loge!(
            "pool",
            "exhausted (active={}, max={})",
            inner.active,
            self.config.max_size
        );
        Err(crate::error_codes::ErrorCode::POOL_EXHAUSTED.err(format!(
            "active={}, max={}",
            inner.active, self.config.max_size
        )))
    }

    /// 尝试借出连接 (非阻塞)
    pub fn try_borrow(&self, server: &(String, u16)) -> Result<Option<PooledConnGuard<'_>>> {
        self.reject_isolated_runtime()?;
        let mut inner = crate::sync::lock(&self.inner, "connection pool")?;

        if let Some(conn) = inner.idle.pop_front() {
            inner.active += 1;
            return Ok(Some(PooledConnGuard {
                pool: self,
                conn: Some(conn),
            }));
        }

        if inner.total < self.config.max_size {
            let server_clone = server.clone();
            let has_handshake = self.config.handshake_fn.is_some();
            let generation = Arc::clone(&inner.generation);
            inner.total += 1;
            inner.active += 1;
            drop(inner);

            let mut conn = match TcpConnection::connect(
                &server_clone.0,
                server_clone.1,
                self.config.connect_timeout,
            ) {
                Ok(conn) => conn,
                Err(error) => {
                    self.release_reservation();
                    return Err(error);
                }
            };

            if has_handshake {
                if let Some(ref handshake_fn) = self.config.handshake_fn {
                    if let Err(error) = handshake_fn(&mut conn) {
                        conn.close();
                        self.release_reservation();
                        return Err(error);
                    }
                }
            }

            return Ok(Some(PooledConnGuard {
                pool: self,
                conn: Some(PooledConnection {
                    conn,
                    server: server_clone,
                    generation,
                }),
            }));
        }

        Ok(None)
    }

    /// 归还连接到池中
    fn release_reservation(&self) {
        let mut inner = crate::sync::lock_recover(&self.inner, "connection pool");
        if inner.active == 0 || inner.total == 0 {
            loge!(
                "pool",
                "reservation accounting invariant failed (active={}, total={})",
                inner.active,
                inner.total
            );
            return;
        }
        inner.active -= 1;
        inner.total -= 1;
    }

    /// 归还连接到池中
    fn return_connection(&self, mut pooled: PooledConnection) {
        let mut inner = crate::sync::lock_recover(&self.inner, "connection pool");
        let reusable = Self::settle_return_state(
            &mut inner,
            &pooled.generation,
            pooled.conn.is_open(),
            self.config.max_size,
        );
        if reusable {
            inner.idle.push_back(pooled);
        } else {
            pooled.conn.close();
        }
    }

    fn discard_connection(&self, mut pooled: PooledConnection) {
        let mut inner = crate::sync::lock_recover(&self.inner, "connection pool");
        Self::settle_return_state(&mut inner, &pooled.generation, false, self.config.max_size);
        pooled.conn.close();
    }

    fn settle_return_state(
        inner: &mut PoolInner,
        returned_generation: &Arc<()>,
        connection_is_open: bool,
        max_size: usize,
    ) -> bool {
        if inner.active == 0 || inner.total == 0 {
            loge!(
                "pool",
                "return accounting invariant failed (active={}, total={})",
                inner.active,
                inner.total
            );
            return false;
        }
        inner.active -= 1;

        if Arc::ptr_eq(returned_generation, &inner.generation)
            && connection_is_open
            && inner.idle.len() < max_size
        {
            true
        } else {
            inner.total -= 1;
            false
        }
    }

    /// 关闭所有连接
    pub fn close_all(&self) {
        if self.isolation.is_some() {
            // NOT IMPLEMENTED for connected isolation: idle state is checked separately.
            return;
        }
        let mut inner = crate::sync::lock_recover(&self.inner, "connection pool");
        inner.generation = Arc::new(());
        while let Some(mut conn) = inner.idle.pop_front() {
            conn.conn.close();
            inner.total -= 1;
        }
    }

    /// 获取池状态
    pub fn stats(&self) -> PoolStats {
        let inner = crate::sync::lock_recover(&self.inner, "connection pool");
        PoolStats {
            idle: inner.idle.len(),
            active: inner.active,
            total: inner.total,
            max_size: self.config.max_size,
        }
    }
}

/// 连接池统计信息
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub idle: usize,
    pub active: usize,
    pub total: usize,
    pub max_size: usize,
}

/// 借出的连接守卫 (自动归还)
pub struct PooledConnGuard<'a> {
    pool: &'a ConnectionPool,
    conn: Option<PooledConnection>,
}

impl<'a> PooledConnGuard<'a> {
    /// 获取连接引用
    pub fn conn(&mut self) -> &mut TcpConnection {
        &mut self.conn.as_mut().unwrap().conn
    }

    /// 获取服务器信息
    pub fn server(&self) -> &(String, u16) {
        &self.conn.as_ref().unwrap().server
    }

    /// Prevents a failed transport from being returned to the idle pool.
    pub(crate) fn discard(&mut self) {
        if let Some(pooled) = self.conn.take() {
            self.pool.discard_connection(pooled);
        }
    }
}

impl<'a> Drop for PooledConnGuard<'a> {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.pool.return_connection(conn);
        }
    }
}

#[cfg(feature = "test-support")]
pub(super) struct IsolatedSlot<'a> {
    inner: std::sync::MutexGuard<'a, PoolInner>,
}

#[cfg(feature = "test-support")]
impl ConnectionPool {
    pub(super) fn isolated_slot(
        &self,
        scope: &Arc<IsolationScope>,
        deadline: std::time::Instant,
    ) -> Result<IsolatedSlot<'_>> {
        if !self
            .isolation
            .as_ref()
            .is_some_and(|own| Arc::ptr_eq(own, scope))
        {
            return Err(crate::TdxError::Connection(
                "isolated pool scope mismatch".into(),
            ));
        }
        let inner = crate::net::isolation::lock_until(&self.inner, deadline)?;
        if inner.active != 0 || inner.total != inner.idle.len() || inner.total > 1 {
            return Err(crate::TdxError::Connection(
                "isolated pool accounting invalid".into(),
            ));
        }
        Ok(IsolatedSlot { inner })
    }
}

#[cfg(feature = "test-support")]
impl IsolatedSlot<'_> {
    pub(super) fn prepare_publish(&mut self) -> Result<()> {
        if self.inner.total != 0 || !self.inner.idle.is_empty() {
            return Err(crate::TdxError::Connection(
                "isolated pool already connected".into(),
            ));
        }
        self.inner.idle.try_reserve(1).map_err(|error| {
            crate::TdxError::Connection(format!("isolated pool reserve failed: {error}"))
        })?;
        Ok(())
    }
    pub(super) fn install(&mut self, conn: TcpConnection, server: (String, u16)) {
        let generation = Arc::clone(&self.inner.generation);
        self.inner.idle.push_back(PooledConnection {
            conn,
            server,
            generation,
        });
        self.inner.total = 1;
    }
    pub(super) fn has_connection(&self) -> bool {
        !self.inner.idle.is_empty()
    }
    pub(super) fn take_one(&mut self) -> Option<TcpConnection> {
        let result = self.inner.idle.pop_front().map(|pooled| pooled.conn);
        self.inner.total = 0;
        result
    }
}

#[cfg(feature = "test-support")]
pub(super) struct IsolatedQueryLease<'a> {
    pool: &'a ConnectionPool,
    permit: crate::net::isolation::QueryPermit,
    conn: Option<PooledConnection>,
    settled: bool,
}

#[cfg(feature = "test-support")]
impl ConnectionPool {
    pub(super) fn borrow_isolated_quote(
        &self,
        permit: crate::net::isolation::QueryPermit,
    ) -> Result<IsolatedQueryLease<'_>> {
        permit.remaining()?;
        if !self
            .isolation
            .as_ref()
            .is_some_and(|scope| Arc::ptr_eq(scope, &permit.scope))
        {
            return Err(crate::TdxError::Connection(
                "isolated quote pool scope mismatch".into(),
            ));
        }
        let mut inner = crate::net::isolation::lock_until(&self.inner, permit.deadline)?;
        if inner.active != 0
            || inner.total != 1
            || inner.idle.len() != 1
            || !inner.idle.front().is_some_and(|pooled| {
                permit.allows_server(&pooled.server)
                    && Arc::ptr_eq(&pooled.generation, &inner.generation)
            })
        {
            return Err(crate::TdxError::Connection(
                "isolated quote pool has no exact committed connection".into(),
            ));
        }
        let conn = inner
            .idle
            .pop_front()
            .ok_or_else(|| crate::TdxError::Connection("isolated quote stream lost".into()))?;
        inner.active = 1;
        drop(inner);
        // Lease exists before any fallible stream reauthorization.
        let mut lease = IsolatedQueryLease {
            pool: self,
            permit,
            conn: Some(conn),
            settled: false,
        };
        lease
            .conn
            .as_mut()
            .ok_or_else(|| crate::TdxError::Connection("isolated quote lease missing".into()))?
            .conn
            .authorize_isolated_query(&lease.permit)?;
        Ok(lease)
    }
}

#[cfg(feature = "test-support")]
impl IsolatedQueryLease<'_> {
    pub(super) fn conn(&mut self) -> Result<&mut TcpConnection> {
        self.conn
            .as_mut()
            .map(|pooled| &mut pooled.conn)
            .ok_or_else(|| crate::TdxError::Connection("isolated quote lease settled".into()))
    }

    pub(super) fn remaining(&self) -> Result<std::time::Duration> {
        self.permit.remaining()
    }

    pub(super) fn finish(&mut self, allow_reuse: bool) -> Result<bool> {
        if self.settled {
            return Err(crate::TdxError::Connection(
                "isolated quote lease already settled".into(),
            ));
        }
        self.settled = true;
        let result = self.settle(allow_reuse);
        if let Err(error) = &result {
            self.permit.scope.record_failure(error.to_string());
            // The registered clone retains the single checked-shutdown duty;
            // dropping this handle must not shutdown the same socket twice.
            drop(self.conn.take());
        }
        result
    }

    fn settle(&mut self, allow_reuse: bool) -> Result<bool> {
        // Lifecycle -> pool, matching checked close; neither lock spans OS I/O.
        let mut lifecycle = self.permit.settlement()?;
        let mut inner =
            crate::net::isolation::lock_cleanup_until(&self.pool.inner, self.permit.deadline)?;
        if inner.active != 1
            || inner.total != 1
            || !inner.idle.is_empty()
            || !self.conn.as_ref().is_some_and(|pooled| {
                Arc::ptr_eq(&pooled.generation, &inner.generation)
                    && self.permit.allows_server(&pooled.server)
            })
        {
            return Err(crate::TdxError::Connection(
                "isolated quote return accounting invalid".into(),
            ));
        }
        // Pool-lock waiting must not preserve an earlier admission snapshot.
        // Lifecycle identity is still guarded; recheck stop/deadline without relocking it.
        let reusable = allow_reuse
            && lifecycle.reusable
            && self
                .permit
                .scope
                .validate_commit(self.permit.deadline)
                .is_ok();
        let pooled = self.conn.take().ok_or_else(|| {
            crate::TdxError::Connection("isolated quote return stream lost".into())
        })?;
        let shutdown = lifecycle.complete(reusable);
        inner.active = 0;
        let discard = if reusable {
            // The single-slot capacity was reserved during handshake publication.
            inner.idle.push_back(pooled);
            None
        } else {
            inner.total = 0;
            Some(pooled)
        };
        drop(inner);
        drop(lifecycle);
        self.permit.released();
        let mut errors = Vec::new();
        // Both handles refer to ONE socket. Only the transferred registered
        // clone (here or in close) performs checked shutdown; release this one.
        drop(discard);
        let had_shutdown = shutdown.is_some();
        if let Some(shutdown) = shutdown {
            if let Err(error) = shutdown.shutdown(std::net::Shutdown::Both) {
                // A close owner must see failure before the drain becomes zero.
                self.permit.scope.record_failure(error.to_string());
                errors.push(error.to_string());
            }
            drop(shutdown);
        }
        if had_shutdown {
            if let Err(error) = self.permit.scope.finish_query_drain(self.permit.deadline) {
                errors.push(error.to_string());
            }
        }
        if !errors.is_empty() {
            return Err(crate::TdxError::Connection(errors.join("; ")));
        }
        Ok(reusable)
    }
}

#[cfg(feature = "test-support")]
impl Drop for IsolatedQueryLease<'_> {
    fn drop(&mut self) {
        if !self.settled {
            // Unwind/error paths never return a possibly partial quote stream.
            let _ = self.finish(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_stats_initial() {
        let config = PoolConfig::new();
        let pool = ConnectionPool::new_single(("127.0.0.1".to_string(), 7709), config);
        let stats = pool.stats();
        assert_eq!(stats.idle, 0);
        assert_eq!(stats.active, 0);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.max_size, DEFAULT_POOL_SIZE);
    }

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_size, DEFAULT_POOL_SIZE);
        assert_eq!(config.connect_timeout, CONNECT_TIMEOUT);
    }

    #[test]
    fn test_pool_borrow_failure_no_server() {
        let mut config = PoolConfig::new();
        config.max_size = 2;
        config.connect_timeout = 0.1;
        let pool = ConnectionPool::new_single(("127.0.0.1".to_string(), 1), config);
        let server = ("127.0.0.1".to_string(), 1);
        let result = pool.borrow(&server);
        assert!(result.is_err());
        let stats = pool.stats();
        assert_eq!(stats.active, 0);
        assert_eq!(stats.total, 0);

        let result = pool.try_borrow(&server);
        assert!(result.is_err());
        let stats = pool.stats();
        assert_eq!(stats.active, 0);
        assert_eq!(stats.total, 0);
    }

    #[test]
    fn test_pool_close_all() {
        let config = PoolConfig::new();
        let pool = ConnectionPool::new_single(("127.0.0.1".to_string(), 7709), config);
        pool.close_all();
        let stats = pool.stats();
        assert_eq!(stats.total, 0);
    }

    #[test]
    fn close_all_keeps_active_reservations_until_stale_guards_return() {
        let mut config = PoolConfig::new();
        config.max_size = 1;
        let pool = ConnectionPool::new_single(("127.0.0.1".to_string(), 7709), config);
        let stale_generation = {
            let mut inner = pool.inner.lock().unwrap();
            inner.active = 1;
            inner.total = 1;
            Arc::clone(&inner.generation)
        };
        assert_eq!(pool.stats().active, 1);
        pool.close_all();
        let during_close = pool.stats();
        assert_eq!(during_close.idle, 0);
        assert_eq!(during_close.active, 1);
        assert_eq!(during_close.total, 1);

        let reusable = {
            let mut inner = pool.inner.lock().unwrap();
            ConnectionPool::settle_return_state(&mut inner, &stale_generation, true, 1)
        };
        assert!(!reusable);
        let after_return = pool.stats();
        assert_eq!(after_return.idle, 0);
        assert_eq!(after_return.active, 0);
        assert_eq!(after_return.total, 0);
    }
}
