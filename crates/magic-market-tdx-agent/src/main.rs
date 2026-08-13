#![forbid(unsafe_code)]

mod client;
mod config;
mod monitor;

use std::time::{SystemTime, UNIX_EPOCH};

use client::AgentClient;
use config::AgentConfig;
use monitor::{read_frames, MonitorProcess};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(windows) {
        return Err("magic-market-tdx-agent is supported only on Windows".into());
    }
    let config = AgentConfig::parse(std::env::args())?;
    let generation = new_generation()?;
    let mut monitor = MonitorProcess::spawn_sibling()?;
    let stdout = monitor.take_stdout()?;
    let (frames, receiver) = tokio::sync::mpsc::channel(config.queue_capacity);
    let reader = tokio::spawn(read_frames(stdout, config.max_frame_bytes, frames));
    let client = AgentClient::new(&config, generation)?;
    let outcome: Result<(), Box<dyn std::error::Error>> = tokio::select! {
        result = client.forward(receiver) => result.map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        signal = tokio::signal::ctrl_c() => signal.map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
    };
    reader.abort();
    let shutdown = monitor.terminate(config.shutdown_timeout).await;
    outcome?;
    shutdown?;
    Ok(())
}

fn new_generation() -> Result<String, std::time::SystemTimeError> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ^ (u128::from(std::process::id()) << 96);
    Ok(format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        (nanos >> 96) as u32,
        ((nanos >> 80) & 0xffff) as u16,
        ((nanos >> 68) & 0x0fff) as u16,
        ((nanos >> 52) & 0x0fff) as u16,
        nanos & 0x0000_ffff_ffff_ffff
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_terminal_generation_is_canonical_uuid_shaped() {
        let value = new_generation().unwrap();
        assert_eq!(value.len(), 36);
        assert_eq!(&value[8..9], "-");
        assert_eq!(&value[13..14], "-");
        assert_eq!(&value[14..15], "4");
        assert_eq!(&value[18..19], "-");
        assert_eq!(&value[19..20], "8");
        assert_eq!(&value[23..24], "-");
    }
}
