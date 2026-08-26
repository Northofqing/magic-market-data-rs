//! 轻量级日志 — 按级别输出到 stderr, 模块名作为前缀
//!
//! 控制: 环境变量 `TDXRS_LOG` = off|error|warn|info|debug
//! 默认: debug 编译 → debug, release 编译 → warn

use std::fmt;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU8, Ordering};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const OFF: u8 = 0;
pub const ERROR: u8 = 1;
pub const WARN: u8 = 2;
pub const INFO: u8 = 3;
pub const DEBUG: u8 = 4;

static LEVEL: AtomicU8 = AtomicU8::new(WARN);

pub fn init() {
    let lvl = if let Ok(v) = std::env::var("TDXRS_LOG") {
        match v.to_lowercase().as_str() {
            "off" => OFF,
            "error" => ERROR,
            "warn" => WARN,
            "info" => INFO,
            "debug" => DEBUG,
            _ => WARN,
        }
    } else if cfg!(debug_assertions) {
        DEBUG
    } else {
        WARN
    };
    LEVEL.store(lvl, Ordering::Relaxed);
}

pub fn set_level(lvl: u8) {
    LEVEL.store(lvl.min(DEBUG), Ordering::Relaxed);
}

pub fn level_str(lvl: u8) -> &'static str {
    match lvl {
        ERROR => "E",
        WARN => "W",
        INFO => "I",
        DEBUG => "D",
        _ => "",
    }
}

pub fn emit(level: u8, module: &str, message: fmt::Arguments<'_>) {
    let stderr = io::stderr();
    let mut writer = stderr.lock();
    let _ = write_record(
        &mut writer,
        OffsetDateTime::now_utc(),
        level,
        module,
        message,
    );
}

fn write_record(
    writer: &mut impl Write,
    now: OffsetDateTime,
    level: u8,
    module: &str,
    message: fmt::Arguments<'_>,
) -> io::Result<()> {
    let timestamp = now
        .format(&Rfc3339)
        .unwrap_or_else(|_| format!("unix-ns:{}", now.unix_timestamp_nanos()));
    writeln!(
        writer,
        "ts={timestamp} [{}] {module}  {message}",
        level_str(level)
    )
}

/// 调用方的宏包装 — 编译期选择是否展开 format 参数
/// 用法: `logd!("mod", "msg {}", v);`  (debug)
///       `logi!("mod", "msg");`        (info)
#[macro_export]
macro_rules! logd {
    ($mod:expr, $($arg:tt)*) => {
        if $crate::logging::DEBUG <= $crate::logging::level() {
            $crate::logging::emit(
                $crate::logging::DEBUG,
                $mod,
                format_args!($($arg)*)
            );
        }
    };
}
#[macro_export]
macro_rules! logi {
    ($mod:expr, $($arg:tt)*) => {
        if $crate::logging::INFO <= $crate::logging::level() {
            $crate::logging::emit(
                $crate::logging::INFO,
                $mod,
                format_args!($($arg)*)
            );
        }
    };
}
#[macro_export]
macro_rules! logw {
    ($mod:expr, $($arg:tt)*) => {
        if $crate::logging::WARN <= $crate::logging::level() {
            $crate::logging::emit(
                $crate::logging::WARN,
                $mod,
                format_args!($($arg)*)
            );
        }
    };
}
#[macro_export]
macro_rules! loge {
    ($mod:expr, $($arg:tt)*) => {
        if $crate::logging::ERROR <= $crate::logging::level() {
            $crate::logging::emit(
                $crate::logging::ERROR,
                $mod,
                format_args!($($arg)*)
            );
        }
    };
}

#[inline]
pub fn level() -> u8 {
    LEVEL.load(Ordering::Relaxed)
}

// ================================================================
// 单元测试
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LEVEL_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_level() {
        let _guard = TEST_LEVEL_LOCK.lock().unwrap();
        init();
        // release build → WARN, debug build → DEBUG
        let lvl = level();
        assert!(lvl == WARN || lvl == DEBUG);
    }

    #[test]
    fn test_set_level() {
        let _guard = TEST_LEVEL_LOCK.lock().unwrap();
        set_level(OFF);
        assert_eq!(level(), OFF);
        set_level(ERROR);
        assert_eq!(level(), ERROR);
        set_level(INFO);
        assert_eq!(level(), INFO);
        set_level(DEBUG);
        assert_eq!(level(), DEBUG);
        // restore
        set_level(WARN);
    }

    #[test]
    fn test_level_str() {
        assert_eq!(level_str(ERROR), "E");
        assert_eq!(level_str(WARN), "W");
        assert_eq!(level_str(INFO), "I");
        assert_eq!(level_str(DEBUG), "D");
    }

    #[test]
    fn test_macros_dont_panic() {
        let _guard = TEST_LEVEL_LOCK.lock().unwrap();
        // 这些宏在运行时检查 level, 低 level 时不应输出
        set_level(OFF);
        logd!("test", "should not print");
        loge!("test", "should not print");
        logw!("test", "should not print");
        logi!("test", "should not print");
        set_level(DEBUG);
        logd!("test", "ok to print in test");
        set_level(WARN);
    }

    #[test]
    fn log_record_has_utc_rfc3339_timestamp() {
        let mut bytes = Vec::new();
        write_record(
            &mut bytes,
            OffsetDateTime::from_unix_timestamp(0).unwrap(),
            WARN,
            "hq",
            format_args!("retry={}", 1),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "ts=1970-01-01T00:00:00Z [W] hq  retry=1\n"
        );
    }
}
