use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::Duration;

use crate::error::{Result, TdxError};
#[cfg(feature = "test-support")]
use crate::net::isolation::{Attempt, IsolationScope, QueryPermit};
#[cfg(feature = "test-support")]
use std::sync::Arc;
#[cfg(feature = "test-support")]
use std::time::Instant;

trait ConnectionStream: Read + Write + Send {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()>;
    fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()>;
    fn shutdown(&self, how: Shutdown) -> std::io::Result<()>;
    fn peer_addr(&self) -> std::io::Result<SocketAddr>;
    #[cfg(feature = "test-support")]
    fn authorize_query(&mut self, _permit: &QueryPermit) -> Result<()> {
        Err(TdxError::Unsupported(
            "unscoped stream cannot authorize isolated query".into(),
        ))
    }
}

impl ConnectionStream for TcpStream {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        TcpStream::set_read_timeout(self, timeout)
    }

    fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        TcpStream::set_write_timeout(self, timeout)
    }

    fn shutdown(&self, how: Shutdown) -> std::io::Result<()> {
        TcpStream::shutdown(self, how)
    }

    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        TcpStream::peer_addr(self)
    }
}

pub struct TcpConnection {
    stream: Box<dyn ConnectionStream>,
}

impl TcpConnection {
    pub fn connect(ip: &str, port: u16, timeout_secs: f64) -> Result<Self> {
        Self::connect_with(ip, port, timeout_secs, |address, timeout| {
            TcpStream::connect_timeout(&address, timeout)
                .map(|stream| Box::new(stream) as Box<dyn ConnectionStream>)
        })
    }

    fn connect_with<Connect>(
        ip: &str,
        port: u16,
        timeout_secs: f64,
        connect: Connect,
    ) -> Result<Self>
    where
        Connect: FnOnce(SocketAddr, Duration) -> std::io::Result<Box<dyn ConnectionStream>>,
    {
        if !timeout_secs.is_finite() || timeout_secs <= 0.0 {
            return Err(TdxError::Connection(
                "timeout must be a positive finite number of seconds".into(),
            ));
        }
        let addr = format!("{}:{}", ip, port);
        let socket_addr = addr.parse::<SocketAddr>().map_err(|error| {
            TdxError::Connection(format!("invalid TDX server address {addr}: {error}"))
        })?;
        let timeout = Duration::from_secs_f64(timeout_secs);
        let stream = connect(socket_addr, timeout)
            .map_err(|e| TdxError::Connection(format!("Failed to connect to {}: {}", addr, e)))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| TdxError::Connection(format!("set_read_timeout: {}", e)))?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|e| TdxError::Connection(format!("set_write_timeout: {}", e)))?;
        Ok(Self { stream })
    }

    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        self.stream
            .write_all(data)
            .map_err(|e| TdxError::Connection(format!("send failed: {}", e)))?;
        Ok(())
    }

    /// Read exactly `len` bytes, looping until all received or error.
    pub fn recv(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let mut total = 0;
        while total < len {
            let n = self
                .stream
                .read(&mut buf[total..])
                .map_err(|e| TdxError::Connection(format!("recv failed: {}", e)))?;
            if n == 0 {
                return Err(TdxError::Disconnected);
            }
            total += n;
        }
        Ok(buf)
    }

    pub fn close(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }

    pub fn is_open(&self) -> bool {
        self.stream.peer_addr().is_ok()
    }
}

/// Validates an ordered TDX server list for the failover constructors.
pub(crate) fn validate_server_list(servers: &[(String, u16)], timeout_secs: f64) -> Result<()> {
    if servers.is_empty() {
        return Err(TdxError::InvalidData(
            "TDX server list must not be empty".into(),
        ));
    }
    for (ip, port) in servers {
        if ip.trim().is_empty() {
            return Err(TdxError::InvalidData(
                "TDX server address must not be empty".into(),
            ));
        }
        if *port == 0 {
            return Err(TdxError::InvalidData(
                "TDX server port must be positive".into(),
            ));
        }
    }
    if !timeout_secs.is_finite() || timeout_secs <= 0.0 {
        return Err(TdxError::InvalidData(
            "TDX server timeout must be finite and positive".into(),
        ));
    }
    Ok(())
}

/// Runs `attempt` against each candidate server in order and returns the first
/// success. Only connection-class failures move to the next candidate;
/// protocol and data errors are returned unchanged. When every candidate
/// fails, the returned error lists each attempt so the caller sees the whole
/// failure set, not just the primary server's error.
pub(crate) fn with_failover<T>(
    servers: &[(String, u16)],
    mut attempt: impl FnMut(&str, u16) -> Result<T>,
) -> Result<T> {
    let mut tried: Vec<(String, u16)> = Vec::with_capacity(servers.len());
    let mut failures: Vec<String> = Vec::new();
    for (ip, port) in servers {
        let candidate = (ip.clone(), *port);
        if tried.contains(&candidate) {
            continue;
        }
        tried.push(candidate);
        match attempt(ip, *port) {
            Ok(value) => return Ok(value),
            Err(error) if error.is_connection_class() => {
                failures.push(format!("{ip}:{port} ({error})"));
            }
            Err(error) => return Err(error),
        }
    }
    if failures.is_empty() {
        return Err(TdxError::InvalidData(
            "TDX server list must not be empty".into(),
        ));
    }
    Err(TdxError::Connection(format!(
        "all {} TDX server candidates failed: {}",
        failures.len(),
        failures.join("; ")
    )))
}

#[cfg(feature = "test-support")]
impl TcpConnection {
    pub(super) fn connect_isolated(
        endpoint: std::net::SocketAddrV4,
        attempt: &Attempt,
    ) -> Result<Self> {
        let timeout = attempt.remaining()?;
        let scope = Arc::clone(&attempt.scope);
        let (ticket, epoch) = attempt.io_key();
        Self::connect_with(
            &endpoint.ip().to_string(),
            endpoint.port(),
            timeout.as_secs_f64(),
            |address, _| {
                if address != SocketAddr::V4(endpoint) {
                    return Err(std::io::Error::other("isolated address mismatch"));
                }
                let stream = Arc::new(TcpStream::connect_timeout(
                    &address,
                    attempt
                        .remaining()
                        .map_err(|error| std::io::Error::other(error.to_string()))?,
                )?);
                // One OS socket for I/O and cancellation. Duplicating a newly
                // connected Windows socket can produce an unconnected control handle.
                attempt
                    .register(Arc::clone(&stream))
                    .map_err(|error| std::io::Error::other(error.to_string()))?;
                Ok(Box::new(ScopedStream {
                    stream,
                    scope,
                    ticket,
                    epoch,
                    deadline: attempt.deadline,
                    query: None,
                }) as Box<dyn ConnectionStream>)
            },
        )
    }
}

#[cfg(feature = "test-support")]
struct ScopedStream {
    stream: Arc<TcpStream>,
    scope: Arc<IsolationScope>,
    ticket: u64,
    epoch: u64,
    deadline: Instant,
    query: Option<u64>,
}

#[cfg(feature = "test-support")]
impl Read for ScopedStream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        loop {
            // Shared-socket shutdown does not replace cancellation checks.
            // Keep the original operation deadline across every bounded read.
            let remaining =
                self.scope
                    .io_remaining(self.ticket, self.epoch, self.deadline, self.query)?;
            self.stream
                .set_read_timeout(Some(remaining.min(Duration::from_millis(50))))?;
            match (&*self.stream).read(buffer) {
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) => {}
                result => return result,
            }
        }
    }
}

#[cfg(feature = "test-support")]
impl Write for ScopedStream {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let timeout =
            self.scope
                .io_remaining(self.ticket, self.epoch, self.deadline, self.query)?;
        self.stream.set_write_timeout(Some(timeout))?;
        (&*self.stream).write(buffer)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.scope
            .io_remaining(self.ticket, self.epoch, self.deadline, self.query)?;
        (&*self.stream).flush()
    }
}

#[cfg(feature = "test-support")]
impl ConnectionStream for ScopedStream {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }
    fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.stream.set_write_timeout(timeout)
    }
    fn shutdown(&self, how: Shutdown) -> std::io::Result<()> {
        self.stream.shutdown(how)
    }
    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
    fn authorize_query(&mut self, permit: &QueryPermit) -> Result<()> {
        let query = permit.authorize(&self.scope, self.ticket, self.epoch)?;
        self.deadline = permit.deadline;
        self.query = Some(query);
        Ok(())
    }
}

#[cfg(feature = "test-support")]
impl TcpConnection {
    pub(super) fn authorize_isolated_query(&mut self, permit: &QueryPermit) -> Result<()> {
        self.stream.authorize_query(permit)
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_server_list, with_failover, ConnectionStream, TcpConnection};
    use std::io::{self, Read, Write};
    use std::net::{Shutdown, SocketAddr};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Debug, Default)]
    struct StreamState {
        read_timeout: Option<Duration>,
        write_timeout: Option<Duration>,
        written: Vec<u8>,
        shutdown: bool,
    }

    struct MemoryStream {
        state: Arc<Mutex<StreamState>>,
        response: &'static [u8],
        offset: usize,
    }

    impl Read for MemoryStream {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let remaining = &self.response[self.offset..];
            let size = remaining.len().min(buffer.len());
            buffer[..size].copy_from_slice(&remaining[..size]);
            self.offset += size;
            Ok(size)
        }
    }

    impl Write for MemoryStream {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.state.lock().unwrap().written.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl ConnectionStream for MemoryStream {
        fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
            self.state.lock().unwrap().read_timeout = timeout;
            Ok(())
        }

        fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
            self.state.lock().unwrap().write_timeout = timeout;
            Ok(())
        }

        fn shutdown(&self, _how: Shutdown) -> io::Result<()> {
            self.state.lock().unwrap().shutdown = true;
            Ok(())
        }

        fn peer_addr(&self) -> io::Result<SocketAddr> {
            Ok("127.0.0.1:7709".parse().unwrap())
        }
    }

    #[test]
    fn injected_transport_preserves_connection_error_and_timeout() {
        let result = TcpConnection::connect_with("127.0.0.1", 7709, 0.02, |address, timeout| {
            assert_eq!(address, "127.0.0.1:7709".parse::<SocketAddr>().unwrap());
            assert_eq!(timeout, Duration::from_millis(20));
            Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "deterministic refusal",
            ))
        });
        let error = match result {
            Ok(_) => panic!("connection unexpectedly succeeded"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("Failed to connect"));
        assert!(error.to_string().contains("deterministic refusal"));
    }

    #[test]
    fn failover_returns_first_success_without_touching_later_candidates() {
        let servers = vec![
            ("10.0.0.1".to_owned(), 1_u16),
            ("10.0.0.2".to_owned(), 2_u16),
        ];
        let mut calls = 0_u32;
        let value = with_failover(&servers, |_ip, _port| {
            calls += 1;
            Ok::<u32, crate::error::TdxError>(42)
        })
        .unwrap();
        assert_eq!(value, 42);
        assert_eq!(calls, 1);
    }

    #[test]
    fn failover_moves_on_only_for_connection_class_errors() {
        let servers = vec![
            ("10.0.0.1".to_owned(), 1_u16),
            ("10.0.0.2".to_owned(), 2_u16),
            ("10.0.0.3".to_owned(), 3_u16),
        ];
        let mut calls = Vec::new();
        let value = with_failover(&servers, |_ip, port| {
            calls.push(port);
            if port == 1 {
                Err(crate::error::TdxError::Connection("refused".into()))
            } else {
                Ok::<u32, crate::error::TdxError>(7)
            }
        })
        .unwrap();
        assert_eq!(value, 7);
        assert_eq!(calls, vec![1, 2]);

        let mut calls2 = Vec::new();
        let error = with_failover(&servers, |_ip, port| {
            calls2.push(port);
            if port == 1 {
                Err::<(), crate::error::TdxError>(crate::error::TdxError::ConnectionTimeout)
            } else {
                Err::<(), crate::error::TdxError>(crate::error::TdxError::InvalidData(
                    "must stop here".into(),
                ))
            }
        })
        .unwrap_err();
        assert!(matches!(error, crate::error::TdxError::InvalidData(_)));
        assert_eq!(calls2.len(), 2);
    }

    #[test]
    fn failover_lists_every_attempt_when_all_candidates_fail() {
        let servers = vec![
            ("10.0.0.1".to_owned(), 1_u16),
            ("10.0.0.2".to_owned(), 2_u16),
        ];
        let error = with_failover(&servers, |_ip, _port| {
            Err::<(), crate::error::TdxError>(crate::error::TdxError::Disconnected)
        })
        .unwrap_err();
        let text = error.to_string();
        assert!(text.contains("all 2 TDX server candidates failed"));
        assert!(text.contains("10.0.0.1:1"));
        assert!(text.contains("10.0.0.2:2"));
    }

    #[test]
    fn failover_skips_duplicate_candidates() {
        let servers = vec![
            ("10.0.0.1".to_owned(), 1_u16),
            ("10.0.0.1".to_owned(), 1_u16),
        ];
        let mut calls = 0_u32;
        let _ = with_failover(&servers, |_ip, _port| {
            calls += 1;
            Err::<(), crate::error::TdxError>(crate::error::TdxError::ConnectionTimeout)
        });
        assert_eq!(calls, 1);
    }

    #[test]
    fn server_list_validation_rejects_empty_empty_address_zero_port_and_bad_timeout() {
        assert!(validate_server_list(&[], 5.0).is_err());
        assert!(validate_server_list(&[("127.0.0.1".to_owned(), 0)], 5.0).is_err());
        assert!(validate_server_list(&[(" ".to_owned(), 1)], 5.0).is_err());
        assert!(validate_server_list(&[("127.0.0.1".to_owned(), 1)], 0.0).is_err());
        assert!(validate_server_list(&[("127.0.0.1".to_owned(), 1)], 5.0).is_ok());
    }

    #[test]
    fn injected_stream_uses_timeouts_and_public_io_contract() {
        let state = Arc::new(Mutex::new(StreamState::default()));
        let stream_state = Arc::clone(&state);
        let mut connection =
            TcpConnection::connect_with("127.0.0.1", 7709, 0.1, move |_address, _timeout| {
                Ok(Box::new(MemoryStream {
                    state: stream_state,
                    response: b"pong",
                    offset: 0,
                }))
            })
            .unwrap();

        assert!(connection.is_open());
        connection.send(b"ping").unwrap();
        assert_eq!(connection.recv(4).unwrap(), b"pong");
        connection.close();

        let state = state.lock().unwrap();
        assert_eq!(state.read_timeout, Some(Duration::from_millis(100)));
        assert_eq!(state.write_timeout, Some(Duration::from_millis(100)));
        assert_eq!(state.written, b"ping");
        assert!(state.shutdown);
    }
}
