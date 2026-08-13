use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::mpsc;

const ARGUMENT_FILE_NAME: &str = "magic-market-monitor-server.args.json";

pub(crate) struct MonitorProcess {
    child: Child,
    stdout: Option<ChildStdout>,
}

impl MonitorProcess {
    pub(crate) fn spawn_sibling() -> Result<Self, MonitorError> {
        let current = std::env::current_exe().map_err(MonitorError::CurrentExecutable)?;
        let directory = current
            .parent()
            .ok_or(MonitorError::MissingExecutableDirectory)?;
        let executable = directory.join(monitor_binary_name());
        let argument_file = directory.join(ARGUMENT_FILE_NAME);
        let arguments = read_arguments(&argument_file)?;
        let mut child = Command::new(&executable)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| MonitorError::Spawn { executable, source })?;
        let stdout = child.stdout.take().ok_or(MonitorError::MissingStdout)?;
        Ok(Self {
            child,
            stdout: Some(stdout),
        })
    }

    pub(crate) fn take_stdout(&mut self) -> Result<ChildStdout, MonitorError> {
        self.stdout.take().ok_or(MonitorError::MissingStdout)
    }

    pub(crate) async fn terminate(mut self, timeout: Duration) -> Result<(), MonitorError> {
        self.child.start_kill().map_err(MonitorError::Terminate)?;
        tokio::time::timeout(timeout, self.child.wait())
            .await
            .map_err(|_| MonitorError::ShutdownTimeout)?
            .map_err(MonitorError::Wait)?;
        Ok(())
    }
}

fn monitor_binary_name() -> &'static str {
    if cfg!(windows) {
        "magic-market-monitor-server.exe"
    } else {
        "magic-market-monitor-server"
    }
}

fn read_arguments(path: &Path) -> Result<Vec<String>, MonitorError> {
    let metadata = std::fs::metadata(path).map_err(|source| MonitorError::ArgumentFile {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.len() == 0 || metadata.len() > 65_536 {
        return Err(MonitorError::InvalidArguments(
            "monitor argument file must contain 1..=65536 bytes".to_owned(),
        ));
    }
    let bytes = std::fs::read(path).map_err(|source| MonitorError::ArgumentFile {
        path: path.to_path_buf(),
        source,
    })?;
    let arguments: Vec<String> =
        serde_json::from_slice(&bytes).map_err(MonitorError::ArgumentJson)?;
    if arguments.is_empty()
        || arguments
            .iter()
            .any(|value| value.is_empty() || value.contains('\0'))
    {
        return Err(MonitorError::InvalidArguments(
            "monitor arguments must be a non-empty array of non-empty strings".to_owned(),
        ));
    }
    Ok(arguments)
}

pub(crate) async fn read_frames<R: AsyncRead + Unpin>(
    mut reader: R,
    maximum_bytes: usize,
    frames: mpsc::Sender<Vec<u8>>,
) -> Result<(), MonitorError> {
    loop {
        let mut prefix = [0_u8; 4];
        match reader.read_exact(&mut prefix).await {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(MonitorError::Read(error)),
        }
        let length =
            usize::try_from(u32::from_be_bytes(prefix)).map_err(|_| MonitorError::FrameTooLarge)?;
        if length == 0 || length > maximum_bytes {
            return Err(MonitorError::FrameTooLarge);
        }
        let mut frame = vec![0_u8; length];
        reader
            .read_exact(&mut frame)
            .await
            .map_err(MonitorError::Read)?;
        serde_json::from_slice::<serde_json::Value>(&frame).map_err(MonitorError::FrameJson)?;
        frames
            .send(frame)
            .await
            .map_err(|_| MonitorError::FrameConsumerStopped)?;
    }
}

#[derive(Debug, Error)]
pub(crate) enum MonitorError {
    #[error("unable to locate current executable: {0}")]
    CurrentExecutable(#[source] std::io::Error),
    #[error("current executable has no parent directory")]
    MissingExecutableDirectory,
    #[error("unable to read monitor argument file {path}: {source}")]
    ArgumentFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid monitor argument JSON: {0}")]
    ArgumentJson(#[source] serde_json::Error),
    #[error("invalid monitor arguments: {0}")]
    InvalidArguments(String),
    #[error("unable to start fixed sibling monitor {executable}: {source}")]
    Spawn {
        executable: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("monitor stdout is unavailable")]
    MissingStdout,
    #[error("unable to read monitor frame: {0}")]
    Read(#[source] std::io::Error),
    #[error("monitor frame is empty or exceeds the configured bound")]
    FrameTooLarge,
    #[error("monitor frame is not JSON: {0}")]
    FrameJson(#[source] serde_json::Error),
    #[error("bounded monitor frame consumer stopped")]
    FrameConsumerStopped,
    #[error("unable to terminate monitor: {0}")]
    Terminate(#[source] std::io::Error),
    #[error("monitor shutdown exceeded its deadline")]
    ShutdownTimeout,
    #[error("unable to wait for monitor: {0}")]
    Wait(#[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn framed_reader_rejects_oversize_before_allocation() {
        let (mut writer, reader) = tokio::io::duplex(32);
        let (sender, _receiver) = mpsc::channel(1);
        writer.write_all(&100_u32.to_be_bytes()).await.unwrap();
        drop(writer);
        let error = read_frames(reader, 10, sender).await.unwrap_err();
        assert!(matches!(error, MonitorError::FrameTooLarge));
    }

    #[tokio::test]
    async fn framed_reader_preserves_exact_json_bytes() {
        let payload = br#"{"type":"waiting"}"#;
        let (mut writer, reader) = tokio::io::duplex(64);
        let (sender, mut receiver) = mpsc::channel(1);
        writer
            .write_all(&(payload.len() as u32).to_be_bytes())
            .await
            .unwrap();
        writer.write_all(payload).await.unwrap();
        drop(writer);
        read_frames(reader, 64, sender).await.unwrap();
        assert_eq!(receiver.recv().await.unwrap(), payload);
    }
}
