use std::fmt;
use std::io::{self, Write};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Clone, Copy)]
pub(crate) enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

pub(crate) fn event(level: Level, target: &str, event: &str, message: fmt::Arguments<'_>) {
    let stderr = io::stderr();
    let mut writer = stderr.lock();
    let _ = write_record(
        &mut writer,
        OffsetDateTime::now_utc(),
        level,
        target,
        event,
        message,
    );
}

fn write_record(
    writer: &mut impl Write,
    now: OffsetDateTime,
    level: Level,
    target: &str,
    event: &str,
    message: fmt::Arguments<'_>,
) -> io::Result<()> {
    let timestamp = now
        .format(&Rfc3339)
        .unwrap_or_else(|_| format!("unix-ns:{}", now.unix_timestamp_nanos()));
    writeln!(
        writer,
        "ts={timestamp} level={} target={target} event={event} {message}",
        level.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operational_record_has_rfc3339_timestamp_and_stable_fields() {
        let mut bytes = Vec::new();
        write_record(
            &mut bytes,
            OffsetDateTime::from_unix_timestamp(0).unwrap(),
            Level::Warn,
            "grpc_server",
            "test_event",
            format_args!("count={}", 2),
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "ts=1970-01-01T00:00:00Z level=WARN target=grpc_server event=test_event count=2\n"
        );
    }
}
