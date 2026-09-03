use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::time::Duration;

use magic_tdx_local_rs::{SourceExchange, TqInstrument, TqLoopbackLimits};

use crate::analysis::{AmountRule, PriceRule, RuleLimits, VolumeRule};

#[derive(Clone, Debug)]
pub(crate) struct WatchInstrument {
    pub(crate) label: String,
    pub(crate) source: TqInstrument,
}

#[derive(Clone, Debug)]
pub(crate) struct Config {
    pub(crate) watchlist: Vec<WatchInstrument>,
    pub(crate) poll_interval: Duration,
    pub(crate) rediscover_interval: Duration,
    pub(crate) discovery_timeout: Duration,
    pub(crate) discovery_max_bytes: usize,
    pub(crate) loopback_limits: TqLoopbackLimits,
    pub(crate) rule_limits: RuleLimits,
    pub(crate) price_rule: PriceRule,
    pub(crate) amount_rule: AmountRule,
    pub(crate) volume_rule: VolumeRule,
    pub(crate) snapshot_cadence_poll_cycles: u64,
    pub(crate) identity_recheck_cycles: u64,
    pub(crate) restart_budget: u32,
    /// Zero keeps the service unbounded. A positive value stops after that
    /// many scheduler cycles; a Waiting cycle is one discovery attempt and a
    /// Running cycle is one complete watchlist poll pass.
    pub(crate) diagnostic_poll_cycles: u64,
    pub(crate) max_event_bytes: usize,
    pub(crate) output_queue_capacity: usize,
    pub(crate) output_shutdown_timeout: Duration,
    pub(crate) output_slow_consumer_policy: OutputSlowConsumerPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputSlowConsumerPolicy {
    Stop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ConfigError {
    OddArgumentCount,
    ExpectedSwitch(String),
    DuplicateSwitch(String),
    UnknownSwitch(String),
    MissingSwitch(&'static str),
    InvalidValue {
        switch: &'static str,
        reason: &'static str,
    },
    DuplicateInstrument(String),
    InvalidLoopbackLimits(String),
    InvalidRule(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OddArgumentCount => {
                formatter.write_str("every switch requires one explicit value")
            }
            Self::ExpectedSwitch(value) => {
                write!(formatter, "expected a --switch, received {value}")
            }
            Self::DuplicateSwitch(value) => write!(formatter, "duplicate switch {value}"),
            Self::UnknownSwitch(value) => write!(formatter, "unknown or forbidden switch {value}"),
            Self::MissingSwitch(value) => write!(formatter, "missing required switch {value}"),
            Self::InvalidValue { switch, reason } => {
                write!(formatter, "invalid {switch}: {reason}")
            }
            Self::DuplicateInstrument(value) => {
                write!(formatter, "duplicate watchlist instrument {value}")
            }
            Self::InvalidLoopbackLimits(value) => {
                write!(formatter, "invalid loopback limits: {value}")
            }
            Self::InvalidRule(value) => write!(formatter, "invalid monitor rule: {value}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    const SWITCHES: [&'static str; 38] = [
        "--watchlist",
        "--max-instruments",
        "--poll-interval-ms",
        "--rediscover-interval-ms",
        "--discovery-timeout-ms",
        "--discovery-max-bytes",
        "--connect-timeout-ms",
        "--read-timeout-ms",
        "--write-timeout-ms",
        "--max-request-bytes",
        "--max-response-bytes",
        "--window-capacity",
        "--price-rule-version",
        "--price-window-ms",
        "--price-boundary-tolerance-ms",
        "--price-trigger-ratio",
        "--price-rearm-ratio",
        "--price-cooldown-ms",
        "--amount-rule-version",
        "--amount-window-ms",
        "--amount-boundary-tolerance-ms",
        "--amount-trigger-cny",
        "--amount-rearm-cny",
        "--amount-cooldown-ms",
        "--snapshot-cadence-poll-cycles",
        "--identity-recheck-cycles",
        "--volume-rule-version",
        "--volume-window-ms",
        "--volume-boundary-tolerance-ms",
        "--volume-trigger-delta",
        "--volume-rearm-delta",
        "--volume-cooldown-ms",
        "--restart-budget",
        "--diagnostic-poll-cycles",
        "--max-event-bytes",
        "--output-queue-capacity",
        "--output-shutdown-timeout-ms",
        "--output-slow-consumer-policy",
    ];

    pub(crate) fn parse(arguments: &[String]) -> Result<Self, ConfigError> {
        if !arguments.len().is_multiple_of(2) {
            return Err(ConfigError::OddArgumentCount);
        }
        let mut values = BTreeMap::new();
        for pair in arguments.chunks_exact(2) {
            let switch = pair[0].as_str();
            if !switch.starts_with("--") {
                return Err(ConfigError::ExpectedSwitch(pair[0].clone()));
            }
            if !Self::SWITCHES.contains(&switch) {
                return Err(ConfigError::UnknownSwitch(switch.to_owned()));
            }
            if values.insert(switch.to_owned(), pair[1].clone()).is_some() {
                return Err(ConfigError::DuplicateSwitch(switch.to_owned()));
            }
        }

        let max_instruments = required_u16(&values, "--max-instruments", 1)?;
        let watchlist = parse_watchlist(required(&values, "--watchlist")?, max_instruments)?;
        let poll_interval = required_duration(&values, "--poll-interval-ms")?;
        let rediscover_interval = required_duration(&values, "--rediscover-interval-ms")?;
        let discovery_timeout = required_duration(&values, "--discovery-timeout-ms")?;
        let discovery_max_bytes = required_usize(&values, "--discovery-max-bytes", 1)?;
        let connect_timeout = required_duration(&values, "--connect-timeout-ms")?;
        let read_timeout = required_duration(&values, "--read-timeout-ms")?;
        let write_timeout = required_duration(&values, "--write-timeout-ms")?;
        let max_request_bytes = required_usize(&values, "--max-request-bytes", 1)?;
        let max_response_bytes = required_usize(&values, "--max-response-bytes", 1)?;
        let loopback_limits = TqLoopbackLimits::new(
            connect_timeout,
            read_timeout,
            write_timeout,
            max_request_bytes,
            max_response_bytes,
        )
        .map_err(|error| ConfigError::InvalidLoopbackLimits(error.to_string()))?;
        let window_capacity = required_u16(&values, "--window-capacity", 2)?;
        let rule_limits =
            RuleLimits::new(max_instruments, window_capacity).map_err(ConfigError::InvalidRule)?;
        let price_window_millis = required_u64(&values, "--price-window-ms", 1)?;
        let price_tolerance_millis = required_u64(&values, "--price-boundary-tolerance-ms", 0)?;
        let price_rule = PriceRule::new(
            required_u32(&values, "--price-rule-version", 1)?,
            price_window_millis,
            price_tolerance_millis,
            required_f64(&values, "--price-trigger-ratio")?,
            required_f64(&values, "--price-rearm-ratio")?,
            required_u64(&values, "--price-cooldown-ms", 0)?,
        )
        .map_err(ConfigError::InvalidRule)?;
        let amount_window_millis = required_u64(&values, "--amount-window-ms", 1)?;
        let amount_tolerance_millis = required_u64(&values, "--amount-boundary-tolerance-ms", 0)?;
        let amount_rule = AmountRule::new(
            required_u32(&values, "--amount-rule-version", 1)?,
            amount_window_millis,
            amount_tolerance_millis,
            required_f64(&values, "--amount-trigger-cny")?,
            required_f64(&values, "--amount-rearm-cny")?,
            required_u64(&values, "--amount-cooldown-ms", 0)?,
        )
        .map_err(ConfigError::InvalidRule)?;
        let volume_window_millis = required_u64(&values, "--volume-window-ms", 1)?;
        let volume_tolerance_millis = required_u64(&values, "--volume-boundary-tolerance-ms", 0)?;
        let volume_rule = VolumeRule::new(
            required_u32(&values, "--volume-rule-version", 1)?,
            volume_window_millis,
            volume_tolerance_millis,
            required_f64(&values, "--volume-trigger-delta")?,
            required_f64(&values, "--volume-rearm-delta")?,
            required_u64(&values, "--volume-cooldown-ms", 0)?,
        )
        .map_err(ConfigError::InvalidRule)?;
        let snapshot_cadence_poll_cycles =
            required_u64(&values, "--snapshot-cadence-poll-cycles", 1)?;
        let identity_recheck_cycles = required_u64(&values, "--identity-recheck-cycles", 1)?;
        validate_window_capacity(
            window_capacity,
            poll_interval,
            watchlist.len(),
            snapshot_cadence_poll_cycles,
            [price_window_millis, volume_window_millis],
            [price_tolerance_millis, volume_tolerance_millis],
            amount_window_millis,
            amount_tolerance_millis,
        )?;
        let restart_budget = required_u32(&values, "--restart-budget", 0)?;
        let diagnostic_poll_cycles = required_u64(&values, "--diagnostic-poll-cycles", 0)?;
        let max_event_bytes = required_usize(&values, "--max-event-bytes", 1)?;
        let output_queue_capacity = required_usize(&values, "--output-queue-capacity", 1)?;
        let output_shutdown_timeout = required_duration(&values, "--output-shutdown-timeout-ms")?;
        let output_slow_consumer_policy = match required(&values, "--output-slow-consumer-policy")?
        {
            "stop" => OutputSlowConsumerPolicy::Stop,
            _ => {
                return Err(ConfigError::InvalidValue {
                    switch: "--output-slow-consumer-policy",
                    reason: "the only fail-closed policy is stop",
                })
            }
        };
        Ok(Self {
            watchlist,
            poll_interval,
            rediscover_interval,
            discovery_timeout,
            discovery_max_bytes,
            loopback_limits,
            rule_limits,
            price_rule,
            amount_rule,
            volume_rule,
            snapshot_cadence_poll_cycles,
            identity_recheck_cycles,
            restart_budget,
            diagnostic_poll_cycles,
            max_event_bytes,
            output_queue_capacity,
            output_shutdown_timeout,
            output_slow_consumer_policy,
        })
    }

    pub(crate) const fn usage() -> &'static str {
        concat!(
            "required pairs: --watchlist EQUITY:SH:600000,EQUITY:SZ:000001 --max-instruments N ",
            "--poll-interval-ms N --rediscover-interval-ms N ",
            "--discovery-timeout-ms N --discovery-max-bytes N ",
            "--connect-timeout-ms N --read-timeout-ms N --write-timeout-ms N ",
            "--max-request-bytes N --max-response-bytes N --window-capacity N ",
            "--price-rule-version N --price-window-ms N --price-boundary-tolerance-ms N ",
            "--price-trigger-ratio R --price-rearm-ratio R --price-cooldown-ms N ",
            "--amount-rule-version N --amount-window-ms N --amount-boundary-tolerance-ms N ",
            "--amount-trigger-cny N --amount-rearm-cny N --amount-cooldown-ms N ",
            "--snapshot-cadence-poll-cycles N ",
            "--identity-recheck-cycles N ",
            "--volume-rule-version N --volume-window-ms N --volume-boundary-tolerance-ms N ",
            "--volume-trigger-delta N --volume-rearm-delta N --volume-cooldown-ms N ",
            "--restart-budget N(4294967295=unbounded) ",
            "--diagnostic-poll-cycles N(0=unbounded) --max-event-bytes N ",
            "--output-queue-capacity N --output-shutdown-timeout-ms N ",
            "--output-slow-consumer-policy stop"
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_window_capacity(
    capacity: u16,
    poll_interval: Duration,
    watchlist_len: usize,
    snapshot_cadence: u64,
    fast_windows: [u64; 2],
    fast_tolerances: [u64; 2],
    amount_window: u64,
    amount_tolerance: u64,
) -> Result<(), ConfigError> {
    let poll_millis =
        u64::try_from(poll_interval.as_millis()).map_err(|_| ConfigError::InvalidValue {
            switch: "--poll-interval-ms",
            reason: "exceeds supported capacity arithmetic",
        })?;
    let fast_span = fast_windows
        .into_iter()
        .zip(fast_tolerances)
        .map(|(window, tolerance)| window.saturating_add(tolerance))
        .max()
        .unwrap_or(0);
    let snapshot_period = poll_millis
        .checked_mul(snapshot_cadence)
        .and_then(|value| value.checked_mul(u64::try_from(watchlist_len).ok()?))
        .ok_or(ConfigError::InvalidValue {
            switch: "--snapshot-cadence-poll-cycles",
            reason: "overflows capacity arithmetic",
        })?;
    let required_fast = fast_span.div_ceil(poll_millis).saturating_add(2);
    let required_amount = amount_window
        .saturating_add(amount_tolerance)
        .div_ceil(snapshot_period)
        .saturating_add(2);
    let required = required_fast.max(required_amount);
    if u64::from(capacity) < required {
        return Err(ConfigError::InvalidValue {
            switch: "--window-capacity",
            reason: "cannot retain the configured poll/window/watchlist sampling span",
        });
    }
    Ok(())
}

fn required<'a>(
    values: &'a BTreeMap<String, String>,
    switch: &'static str,
) -> Result<&'a str, ConfigError> {
    values
        .get(switch)
        .map(String::as_str)
        .ok_or(ConfigError::MissingSwitch(switch))
}

fn required_u64(
    values: &BTreeMap<String, String>,
    switch: &'static str,
    minimum: u64,
) -> Result<u64, ConfigError> {
    let value = required(values, switch)?;
    let parsed = value
        .parse::<u64>()
        .map_err(|_| ConfigError::InvalidValue {
            switch,
            reason: "must be an unsigned integer",
        })?;
    if parsed < minimum {
        return Err(ConfigError::InvalidValue {
            switch,
            reason: "is below the allowed minimum",
        });
    }
    Ok(parsed)
}

fn required_u32(
    values: &BTreeMap<String, String>,
    switch: &'static str,
    minimum: u32,
) -> Result<u32, ConfigError> {
    let parsed = required_u64(values, switch, u64::from(minimum))?;
    u32::try_from(parsed).map_err(|_| ConfigError::InvalidValue {
        switch,
        reason: "exceeds u32",
    })
}

fn required_u16(
    values: &BTreeMap<String, String>,
    switch: &'static str,
    minimum: u16,
) -> Result<u16, ConfigError> {
    let parsed = required_u64(values, switch, u64::from(minimum))?;
    u16::try_from(parsed).map_err(|_| ConfigError::InvalidValue {
        switch,
        reason: "exceeds u16",
    })
}

fn required_usize(
    values: &BTreeMap<String, String>,
    switch: &'static str,
    minimum: usize,
) -> Result<usize, ConfigError> {
    let parsed = required_u64(values, switch, u64::try_from(minimum).unwrap_or(u64::MAX))?;
    let parsed = usize::try_from(parsed).map_err(|_| ConfigError::InvalidValue {
        switch,
        reason: "exceeds platform usize",
    })?;
    if parsed == usize::MAX {
        return Err(ConfigError::InvalidValue {
            switch,
            reason: "must leave room for a one-byte overflow probe",
        });
    }
    Ok(parsed)
}

fn required_duration(
    values: &BTreeMap<String, String>,
    switch: &'static str,
) -> Result<Duration, ConfigError> {
    Ok(Duration::from_millis(required_u64(values, switch, 1)?))
}

fn required_f64(
    values: &BTreeMap<String, String>,
    switch: &'static str,
) -> Result<f64, ConfigError> {
    let parsed =
        required(values, switch)?
            .parse::<f64>()
            .map_err(|_| ConfigError::InvalidValue {
                switch,
                reason: "must be a finite decimal number",
            })?;
    if !parsed.is_finite() {
        return Err(ConfigError::InvalidValue {
            switch,
            reason: "must be finite",
        });
    }
    Ok(parsed)
}

fn parse_watchlist(value: &str, maximum: u16) -> Result<Vec<WatchInstrument>, ConfigError> {
    if value.is_empty() || value.trim() != value {
        return Err(ConfigError::InvalidValue {
            switch: "--watchlist",
            reason: "must be non-empty and unpadded",
        });
    }
    let mut seen = HashSet::new();
    let mut instruments = Vec::new();
    for item in value.split(',') {
        let mut parts = item.split(':');
        let asset = parts.next();
        let exchange = parts.next();
        let code = parts.next();
        if parts.next().is_some() || asset != Some("EQUITY") || exchange.is_none() || code.is_none()
        {
            return Err(ConfigError::InvalidValue {
            switch: "--watchlist",
            reason: "entries must use explicit EQUITY:SH:123456, EQUITY:SZ:123456, or EQUITY:BJ:123456 form",
            });
        }
        let (Some(exchange), Some(code)) = (exchange, code) else {
            return Err(ConfigError::InvalidValue {
                switch: "--watchlist",
                reason: "exchange and code are required",
            });
        };
        let exchange = match exchange {
            "SH" => SourceExchange::Shanghai,
            "SZ" => SourceExchange::Shenzhen,
            "BJ" => SourceExchange::Beijing,
            _ => {
                return Err(ConfigError::InvalidValue {
                    switch: "--watchlist",
                    reason: "exchange must be SH, SZ, or BJ",
                })
            }
        };
        let label = item.to_owned();
        if !seen.insert(label.clone()) {
            return Err(ConfigError::DuplicateInstrument(label));
        }
        let source = TqInstrument::new(exchange, code).map_err(|_| ConfigError::InvalidValue {
            switch: "--watchlist",
            reason: "instrument code must be exactly six ASCII digits",
        })?;
        instruments.push(WatchInstrument { label, source });
        if instruments.len() > usize::from(maximum) {
            return Err(ConfigError::InvalidValue {
                switch: "--watchlist",
                reason: "contains more entries than --max-instruments",
            });
        }
    }
    Ok(instruments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_args() -> Vec<String> {
        [
            ("--watchlist", "EQUITY:SH:600000,EQUITY:SZ:000001"),
            ("--max-instruments", "2"),
            ("--poll-interval-ms", "100"),
            ("--rediscover-interval-ms", "200"),
            ("--discovery-timeout-ms", "300"),
            ("--discovery-max-bytes", "4096"),
            ("--connect-timeout-ms", "50"),
            ("--read-timeout-ms", "50"),
            ("--write-timeout-ms", "50"),
            ("--max-request-bytes", "1024"),
            ("--max-response-bytes", "4096"),
            ("--window-capacity", "16"),
            ("--price-rule-version", "1"),
            ("--price-window-ms", "1000"),
            ("--price-boundary-tolerance-ms", "100"),
            ("--price-trigger-ratio", "0.05"),
            ("--price-rearm-ratio", "0.01"),
            ("--price-cooldown-ms", "500"),
            ("--amount-rule-version", "1"),
            ("--amount-window-ms", "1000"),
            ("--amount-boundary-tolerance-ms", "100"),
            ("--amount-trigger-cny", "10000"),
            ("--amount-rearm-cny", "1000"),
            ("--amount-cooldown-ms", "500"),
            ("--snapshot-cadence-poll-cycles", "1"),
            ("--identity-recheck-cycles", "10"),
            ("--volume-rule-version", "1"),
            ("--volume-window-ms", "1000"),
            ("--volume-boundary-tolerance-ms", "100"),
            ("--volume-trigger-delta", "1000"),
            ("--volume-rearm-delta", "100"),
            ("--volume-cooldown-ms", "500"),
            ("--restart-budget", "3"),
            ("--diagnostic-poll-cycles", "0"),
            ("--max-event-bytes", "8192"),
            ("--output-queue-capacity", "16"),
            ("--output-shutdown-timeout-ms", "100"),
            ("--output-slow-consumer-policy", "stop"),
        ]
        .into_iter()
        .flat_map(|(key, value)| [key.to_owned(), value.to_owned()])
        .collect()
    }

    #[test]
    fn all_operating_limits_are_explicit_and_no_path_or_endpoint_is_accepted() {
        let config = Config::parse(&valid_args()).unwrap();
        assert_eq!(config.watchlist.len(), 2);
        assert_eq!(config.diagnostic_poll_cycles, 0);

        let mut missing = valid_args();
        let position = missing
            .iter()
            .position(|value| value == "--read-timeout-ms")
            .unwrap();
        missing.drain(position..=position + 1);
        assert_eq!(
            Config::parse(&missing).unwrap_err(),
            ConfigError::MissingSwitch("--read-timeout-ms")
        );

        let mut missing_diagnostic_bound = valid_args();
        let position = missing_diagnostic_bound
            .iter()
            .position(|value| value == "--diagnostic-poll-cycles")
            .unwrap();
        missing_diagnostic_bound.drain(position..=position + 1);
        assert_eq!(
            Config::parse(&missing_diagnostic_bound).unwrap_err(),
            ConfigError::MissingSwitch("--diagnostic-poll-cycles")
        );

        for forbidden in ["--tdx-path", "--bridge-path", "--endpoint"] {
            let mut values = valid_args();
            values.extend([forbidden.to_owned(), "x".to_owned()]);
            assert_eq!(
                Config::parse(&values).unwrap_err(),
                ConfigError::UnknownSwitch(forbidden.to_owned())
            );
        }
    }

    #[test]
    fn watchlist_requires_explicit_exchange_and_respects_bound() {
        let mut values = valid_args();
        let position = values
            .iter()
            .position(|value| value == "EQUITY:SH:600000,EQUITY:SZ:000001")
            .unwrap();
        values[position] = "600000".to_owned();
        assert!(matches!(
            Config::parse(&values),
            Err(ConfigError::InvalidValue {
                switch: "--watchlist",
                ..
            })
        ));

        values[position] = "EQUITY:SH:600000,EQUITY:SZ:000001,EQUITY:BJ:430001".to_owned();
        assert!(matches!(
            Config::parse(&values),
            Err(ConfigError::InvalidValue {
                switch: "--watchlist",
                ..
            })
        ));
    }

    #[test]
    fn watchlist_rejects_untyped_funds_and_indices_without_guessing_prefixes() {
        for invalid in ["SH:600000", "ETF:SH:510300", "INDEX:SH:000001"] {
            let mut values = valid_args();
            let position = values
                .iter()
                .position(|value| value == "EQUITY:SH:600000,EQUITY:SZ:000001")
                .unwrap();
            values[position] = invalid.to_owned();
            assert!(matches!(
                Config::parse(&values),
                Err(ConfigError::InvalidValue {
                    switch: "--watchlist",
                    ..
                })
            ));
        }
    }

    #[test]
    fn impossible_window_capacity_is_rejected() {
        let mut values = valid_args();
        let position = values.iter().position(|value| value == "16").unwrap();
        values[position] = "2".to_owned();
        assert!(matches!(
            Config::parse(&values),
            Err(ConfigError::InvalidValue {
                switch: "--window-capacity",
                ..
            })
        ));
    }
}
