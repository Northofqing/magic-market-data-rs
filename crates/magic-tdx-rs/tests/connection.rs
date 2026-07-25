use magic_tdx_rs::net::connection::TcpConnection;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

fn connection_error(ip: &str, port: u16, timeout: f64) -> String {
    match TcpConnection::connect(ip, port, timeout) {
        Ok(mut connection) => {
            connection.close();
            panic!("connection unexpectedly succeeded")
        }
        Err(error) => error.to_string(),
    }
}

#[test]
fn connection_rejects_invalid_address_and_timeout_before_transport() {
    for timeout in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let error = connection_error("127.0.0.1", 7709, timeout);
        assert!(error.contains("positive finite"));
    }

    let error = connection_error("not-an-ip", 7709, 0.01);
    assert!(error.contains("invalid TDX server address"));
}

#[test]
fn connection_failure_is_explicit_and_bounded() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);

    let started = Instant::now();
    let error = connection_error(&address.ip().to_string(), address.port(), 0.02);
    assert!(error.contains("Failed to connect"));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "connect timeout exceeded the deterministic upper bound"
    );
}

#[test]
fn connection_applies_read_write_timeout_to_a_live_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4];
        stream.read_exact(&mut request).unwrap();
        assert_eq!(&request, b"ping");
        stream.write_all(b"pong").unwrap();
    });

    let mut connection =
        TcpConnection::connect(&address.ip().to_string(), address.port(), 0.1).unwrap();
    assert!(connection.is_open());
    connection.send(b"ping").unwrap();
    assert_eq!(connection.recv(4).unwrap(), b"pong");
    connection.close();
    server.join().unwrap();
}
