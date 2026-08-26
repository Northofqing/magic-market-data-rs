#![forbid(unsafe_code)]

mod client;
mod config;
mod logging;
mod monitor;

use std::time::{SystemTime, UNIX_EPOCH};

use client::{AgentClient, ForwardOutcome};
use config::AgentConfig;
use logging::Level;
use monitor::{read_frames, MonitorTemplate};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(windows) {
        return Err("magic-market-tdx-agent is supported only on Windows".into());
    }
    let config = AgentConfig::parse(std::env::args())?;
    let template = MonitorTemplate::load_sibling()?;
    let maximum_watchlist_instruments = template.maximum_watchlist_instruments();
    let mut watchlist_revision = 0_u64;
    let mut watchlist = template.initial_watchlist();
    logging::event(
        Level::Info,
        "tdx_agent",
        "agent_started",
        format_args!(
            "queue_capacity={} heartbeat_interval_ms={} watchlist_limit={} initial_instrument_count={}",
            config.queue_capacity,
            config.heartbeat_interval.as_millis(),
            maximum_watchlist_instruments,
            watchlist.len()
        ),
    );
    loop {
        let generation = new_generation()?;
        let mut monitor = template.spawn(&watchlist)?;
        logging::event(
            Level::Info,
            "tdx_agent",
            "monitor_started",
            format_args!(
                "generation={generation} watchlist_revision={watchlist_revision} instrument_count={}",
                watchlist.len()
            ),
        );
        let stdout = monitor.take_stdout()?;
        let (frames, receiver) = tokio::sync::mpsc::channel(config.queue_capacity);
        let reader = tokio::spawn(read_frames(stdout, config.max_frame_bytes, frames));
        let client = AgentClient::new(
            &config,
            generation.clone(),
            watchlist_revision,
            watchlist.clone(),
            maximum_watchlist_instruments,
        )?;
        let outcome = tokio::select! {
            result = client.forward(receiver) => Some(result),
            signal = tokio::signal::ctrl_c() => {
                signal?;
                None
            },
        };
        reader.abort();
        monitor.terminate(config.shutdown_timeout).await?;
        match outcome {
            None => {
                logging::event(
                    Level::Info,
                    "tdx_agent",
                    "agent_stopped",
                    format_args!("reason=ctrl_c"),
                );
                return Ok(());
            }
            Some(Ok(ForwardOutcome::FramesComplete)) => {
                logging::event(
                    Level::Warn,
                    "tdx_agent",
                    "monitor_restarting",
                    format_args!("reason=output_closed generation={generation}"),
                );
                tokio::time::sleep(config.reconnect_delay).await;
            }
            Some(Ok(ForwardOutcome::Reconfigure(configuration))) => {
                watchlist_revision = configuration.revision;
                watchlist = configuration.instruments;
                logging::event(
                    Level::Info,
                    "tdx_agent",
                    "watchlist_reconfigured",
                    format_args!(
                        "revision={watchlist_revision} instrument_count={}",
                        watchlist.len()
                    ),
                );
            }
            Some(Ok(ForwardOutcome::RestartMonitor(reason))) => {
                logging::event(
                    Level::Warn,
                    "tdx_agent",
                    "monitor_restarting",
                    format_args!("reason=frame_rejected generation={generation} detail={reason}"),
                );
                tokio::time::sleep(config.reconnect_delay).await;
            }
            Some(Err(error)) => {
                logging::event(
                    Level::Error,
                    "tdx_agent",
                    "agent_failed",
                    format_args!("generation={generation} detail={error}"),
                );
                return Err(Box::new(error) as Box<dyn std::error::Error>);
            }
        }
    }
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
