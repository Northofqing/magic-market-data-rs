use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

const SWITCHES: &[&str] = &[
    "--bind",
    "--auth-token-env",
    "--max-decoding-bytes",
    "--max-encoding-bytes",
    "--max-payload-bytes",
    "--unary-concurrency",
    "--blocking-concurrency",
    "--provider-timeout-ms",
    "--blocking-deadline-ms",
    "--max-subscribers",
    "--subscriber-queue-capacity",
    "--replay-max-events",
    "--replay-max-bytes",
    "--agent-command-capacity",
    "--agent-heartbeat-timeout-ms",
    "--shutdown-timeout-ms",
    "--reflection",
    "--tls-cert",
    "--tls-key",
    "--tls-client-ca",
];

#[derive(Clone, Debug)]
pub(crate) struct ServerConfig {
    pub(crate) bind: SocketAddr,
    pub(crate) auth_token: String,
    pub(crate) max_decoding_bytes: usize,
    pub(crate) max_encoding_bytes: usize,
    pub(crate) max_payload_bytes: usize,
    pub(crate) unary_concurrency: usize,
    pub(crate) blocking_concurrency: usize,
    pub(crate) provider_timeout: Duration,
    pub(crate) blocking_deadline: Duration,
    pub(crate) max_subscribers: usize,
    pub(crate) subscriber_queue_capacity: usize,
    pub(crate) replay_max_events: usize,
    pub(crate) replay_max_bytes: usize,
    pub(crate) agent_command_capacity: usize,
    pub(crate) agent_heartbeat_timeout: Duration,
    pub(crate) shutdown_timeout: Duration,
    pub(crate) reflection: bool,
    pub(crate) tls: Option<TlsFiles>,
}

#[derive(Clone, Debug)]
pub(crate) struct TlsFiles {
    pub(crate) certificate: PathBuf,
    pub(crate) private_key: PathBuf,
    pub(crate) client_ca: Option<PathBuf>,
}

impl ServerConfig {
    pub(crate) fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, ConfigError> {
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        let mut values = BTreeMap::new();
        while let Some(switch) = arguments.next() {
            if !SWITCHES.contains(&switch.as_str()) {
                return Err(ConfigError::UnknownSwitch(switch));
            }
            let value = arguments
                .next()
                .ok_or_else(|| ConfigError::MissingValue(switch.clone()))?;
            if values.insert(switch.clone(), value).is_some() {
                return Err(ConfigError::DuplicateSwitch(switch));
            }
        }
        for switch in SWITCHES {
            if !values.contains_key(*switch) {
                return Err(ConfigError::MissingSwitch((*switch).to_owned()));
            }
        }

        let bind = required(&values, "--bind")?
            .parse::<SocketAddr>()
            .map_err(|_| ConfigError::Invalid("--bind must be an IP socket address".to_owned()))?;
        let token_environment = required(&values, "--auth-token-env")?;
        if token_environment.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "--auth-token-env must not be empty".to_owned(),
            ));
        }
        let auth_token = std::env::var(token_environment)
            .map_err(|_| ConfigError::MissingSecretEnvironment(token_environment.to_owned()))?;
        if auth_token.is_empty() {
            return Err(ConfigError::Invalid(
                "authentication token environment must not be empty".to_owned(),
            ));
        }

        let certificate = required(&values, "--tls-cert")?;
        let private_key = required(&values, "--tls-key")?;
        let client_ca = required(&values, "--tls-client-ca")?;
        let tls = match (certificate, private_key, client_ca) {
            ("-", "-", "-") => None,
            ("-", "-", _) => {
                return Err(ConfigError::Invalid(
                    "--tls-client-ca requires --tls-cert and --tls-key".to_owned(),
                ));
            }
            ("-", _, _) | (_, "-", _) => {
                return Err(ConfigError::Invalid(
                    "--tls-cert and --tls-key must both be '-' or both be paths".to_owned(),
                ));
            }
            (certificate, private_key, client_ca) => Some(TlsFiles {
                certificate: PathBuf::from(certificate),
                private_key: PathBuf::from(private_key),
                client_ca: (client_ca != "-").then(|| PathBuf::from(client_ca)),
            }),
        };
        if !bind.ip().is_loopback() && tls.is_none() {
            return Err(ConfigError::Invalid(
                "non-loopback bind requires TLS certificate and key".to_owned(),
            ));
        }
        if !bind.ip().is_loopback() && tls.as_ref().is_some_and(|tls| tls.client_ca.is_none()) {
            return Err(ConfigError::Invalid(
                "non-loopback bind requires --tls-client-ca for mutual TLS".to_owned(),
            ));
        }

        let provider_timeout = positive_duration(&values, "--provider-timeout-ms")?;
        let blocking_deadline = positive_duration(&values, "--blocking-deadline-ms")?;
        if provider_timeout > blocking_deadline {
            return Err(ConfigError::Invalid(
                "--provider-timeout-ms must not exceed --blocking-deadline-ms".to_owned(),
            ));
        }

        Ok(Self {
            bind,
            auth_token,
            max_decoding_bytes: positive_usize(&values, "--max-decoding-bytes")?,
            max_encoding_bytes: positive_usize(&values, "--max-encoding-bytes")?,
            max_payload_bytes: positive_usize(&values, "--max-payload-bytes")?,
            unary_concurrency: positive_usize(&values, "--unary-concurrency")?,
            blocking_concurrency: positive_usize(&values, "--blocking-concurrency")?,
            provider_timeout,
            blocking_deadline,
            max_subscribers: positive_usize(&values, "--max-subscribers")?,
            subscriber_queue_capacity: positive_usize(&values, "--subscriber-queue-capacity")?,
            replay_max_events: positive_usize(&values, "--replay-max-events")?,
            replay_max_bytes: positive_usize(&values, "--replay-max-bytes")?,
            agent_command_capacity: positive_usize(&values, "--agent-command-capacity")?,
            agent_heartbeat_timeout: positive_duration(&values, "--agent-heartbeat-timeout-ms")?,
            shutdown_timeout: positive_duration(&values, "--shutdown-timeout-ms")?,
            reflection: boolean(&values, "--reflection")?,
            tls,
        })
    }
}

fn required<'a>(values: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, ConfigError> {
    values
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| ConfigError::MissingSwitch(key.to_owned()))
}

fn positive_usize(values: &BTreeMap<String, String>, key: &str) -> Result<usize, ConfigError> {
    let value = required(values, key)?
        .parse::<usize>()
        .map_err(|_| ConfigError::Invalid(format!("{key} must be a positive integer")))?;
    if value == 0 {
        return Err(ConfigError::Invalid(format!(
            "{key} must be a positive integer"
        )));
    }
    Ok(value)
}

fn positive_duration(
    values: &BTreeMap<String, String>,
    key: &str,
) -> Result<Duration, ConfigError> {
    let millis = required(values, key)?
        .parse::<u64>()
        .map_err(|_| ConfigError::Invalid(format!("{key} must be positive milliseconds")))?;
    if millis == 0 {
        return Err(ConfigError::Invalid(format!(
            "{key} must be positive milliseconds"
        )));
    }
    Ok(Duration::from_millis(millis))
}

fn boolean(values: &BTreeMap<String, String>, key: &str) -> Result<bool, ConfigError> {
    match required(values, key)? {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ConfigError::Invalid(format!("{key} must be true or false"))),
    }
}

#[derive(Debug, Error)]
pub(crate) enum ConfigError {
    #[error("unknown switch {0}")]
    UnknownSwitch(String),
    #[error("duplicate switch {0}")]
    DuplicateSwitch(String),
    #[error("missing value for {0}")]
    MissingValue(String),
    #[error("missing required switch {0}")]
    MissingSwitch(String),
    #[error("missing secret environment {0}")]
    MissingSecretEnvironment(String),
    #[error("invalid configuration: {0}")]
    Invalid(String),
}
