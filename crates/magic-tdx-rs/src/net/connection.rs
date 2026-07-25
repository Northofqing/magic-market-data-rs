use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use crate::error::{Result, TdxError};

pub struct TcpConnection {
    stream: TcpStream,
}

impl TcpConnection {
    pub fn connect(ip: &str, port: u16, timeout_secs: f64) -> Result<Self> {
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
        let stream = TcpStream::connect_timeout(&socket_addr, timeout)
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
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }

    pub fn is_open(&self) -> bool {
        self.stream.peer_addr().is_ok()
    }
}
