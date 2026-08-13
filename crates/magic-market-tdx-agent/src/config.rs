use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

const SWITCHES: &[&str] = &[
    "--server-uri",
    "--auth-token-env",
    "--max-frame-bytes",
    "--queue-capacity",
    "--connect-timeout-ms",
    "--command-timeout-ms",
    "--reconnect-delay-ms",
    "--shutdown-timeout-ms",
    "--tls-domain",
    "--tls-ca",
    "--tls-cert",
    "--tls-key",
];

#[derive(Clone, Debug)]
pub(crate) struct AgentConfig {
    pub(crate) server_uri: String,
    pub(crate) auth_token: String,
    pub(crate) max_frame_bytes: usize,
    pub(crate) queue_capacity: usize,
    pub(crate) connect_timeout: Duration,
    pub(crate) command_timeout: Duration,
    pub(crate) reconnect_delay: Duration,
    pub(crate) shutdown_timeout: Duration,
    pub(crate) tls: Option<ClientTlsFiles>,
}

#[derive(Clone, Debug)]
pub(crate) struct ClientTlsFiles {
    pub(crate) domain: String,
    pub(crate) ca: PathBuf,
    pub(crate) certificate: PathBuf,
    pub(crate) private_key: PathBuf,
}

impl AgentConfig {
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
        let server_uri = required(&values, "--server-uri")?.to_owned();
        let is_loopback_http =
            server_uri.starts_with("http://127.0.0.1:") || server_uri.starts_with("http://[::1]:");
        if !server_uri.starts_with("https://") && !is_loopback_http {
            return Err(ConfigError::Invalid(
                "server URI must use HTTPS, except for an exact loopback HTTP address".to_owned(),
            ));
        }
        let token_environment = required(&values, "--auth-token-env")?;
        let auth_token = std::env::var(token_environment)
            .map_err(|_| ConfigError::MissingSecretEnvironment(token_environment.to_owned()))?;
        if auth_token.is_empty() || auth_token.chars().any(char::is_whitespace) {
            return Err(ConfigError::Invalid(
                "authentication token must be non-empty and contain no whitespace".to_owned(),
            ));
        }
        let tls_domain = required(&values, "--tls-domain")?;
        let tls_ca = required(&values, "--tls-ca")?;
        let tls_cert = required(&values, "--tls-cert")?;
        let tls_key = required(&values, "--tls-key")?;
        let tls = match (tls_domain, tls_ca, tls_cert, tls_key) {
            ("-", "-", "-", "-") if is_loopback_http => None,
            (domain, ca, certificate, private_key)
                if !domain.is_empty()
                    && domain != "-"
                    && ca != "-"
                    && certificate != "-"
                    && private_key != "-" =>
            {
                Some(ClientTlsFiles {
                    domain: domain.to_owned(),
                    ca: PathBuf::from(ca),
                    certificate: PathBuf::from(certificate),
                    private_key: PathBuf::from(private_key),
                })
            }
            _ => {
                return Err(ConfigError::Invalid(
                    "HTTPS requires --tls-domain, --tls-ca, --tls-cert and --tls-key; loopback HTTP requires '-' for all four".to_owned(),
                ));
            }
        };
        Ok(Self {
            server_uri,
            auth_token,
            max_frame_bytes: positive_usize(&values, "--max-frame-bytes")?,
            queue_capacity: positive_usize(&values, "--queue-capacity")?,
            connect_timeout: positive_duration(&values, "--connect-timeout-ms")?,
            command_timeout: positive_duration(&values, "--command-timeout-ms")?,
            reconnect_delay: positive_duration(&values, "--reconnect-delay-ms")?,
            shutdown_timeout: positive_duration(&values, "--shutdown-timeout-ms")?,
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
    if value == 0 || u32::try_from(value).is_err() {
        return Err(ConfigError::Invalid(format!(
            "{key} must be positive and no larger than u32::MAX"
        )));
    }
    Ok(value)
}

fn positive_duration(
    values: &BTreeMap<String, String>,
    key: &str,
) -> Result<Duration, ConfigError> {
    let value = required(values, key)?
        .parse::<u64>()
        .map_err(|_| ConfigError::Invalid(format!("{key} must be positive milliseconds")))?;
    if value == 0 {
        return Err(ConfigError::Invalid(format!(
            "{key} must be positive milliseconds"
        )));
    }
    Ok(Duration::from_millis(value))
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
