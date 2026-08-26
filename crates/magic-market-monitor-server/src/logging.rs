use std::fmt;
use std::io::{self, Write};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Clone, Copy)]
pub(crate) enum Level {
    Error,
}

impl Level {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
        }
    }
}

pub(crate) fn event(level: Level, target: &str, event: &str, message: fmt::Arguments<'_>) {
    let stderr = io::stderr();
    let mut writer = stderr.lock();
    let timestamp = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unavailable".to_owned());
    let _ = writeln!(
        writer,
        "ts={timestamp} level={} target={target} event={event} {message}",
        level.as_str()
    );
}
