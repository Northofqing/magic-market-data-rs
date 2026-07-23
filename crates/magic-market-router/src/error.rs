use thiserror::Error;

/// Stable source-failure categories used by routing policy and traces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    InvalidRequest,
    Unsupported,
    Transport,
    Timeout,
    RateLimited,
    NoData,
    Protocol,
    Quality,
    Evidence,
    Provider,
}

/// Whether a source failure terminates the route or permits the next source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureAction {
    Stop,
    TryNext,
}

/// Explicitly classified failure returned by one registered source.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{kind:?}: {message}")]
pub struct SourceError {
    kind: FailureKind,
    action: FailureAction,
    message: String,
}

impl SourceError {
    pub fn new(kind: FailureKind, action: FailureAction, message: impl Into<String>) -> Self {
        Self {
            kind,
            action,
            message: message.into(),
        }
    }

    pub fn stop(kind: FailureKind, message: impl Into<String>) -> Self {
        Self::new(kind, FailureAction::Stop, message)
    }

    pub fn try_next(kind: FailureKind, message: impl Into<String>) -> Self {
        Self::new(kind, FailureAction::TryNext, message)
    }

    pub fn kind(&self) -> FailureKind {
        self.kind
    }

    pub fn action(&self) -> FailureAction {
        self.action
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
