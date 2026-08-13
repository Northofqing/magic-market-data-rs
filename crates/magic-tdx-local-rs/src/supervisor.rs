//! Deterministic supervisor lifecycle state transitions.
//!
//! This module describes required external actions but performs none of them.

use thiserror::Error;

/// Destination selected before external bridge cleanup begins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopDestination {
    WaitingForTerminal,
    Stopped,
}

/// Complete lifecycle state of the optional native bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorState {
    Disabled,
    Discovering,
    WaitingForTerminal,
    ValidatingVersion,
    Starting,
    Handshaking,
    Running,
    BackingOff,
    CircuitOpen,
    Stopping { destination: StopDestination },
    Stopped,
}

/// External fact supplied to the pure transition function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorEvent {
    Enable,
    Rediscover,
    DiscoveryFound,
    DiscoveryAbsent,
    DiscoveryRejected,
    DiscoveryFailed,
    ValidationAccepted,
    ValidationRejected,
    BridgeStarted,
    BridgeStartFailed,
    HelloAccepted,
    HandshakeFailed,
    RuntimeFailed,
    StabilityProved,
    BackoffElapsed,
    TerminalLost,
    StopRequested,
    StopCompleted,
    CircuitReset,
}

/// Side effect a runtime executor may perform after a successful transition.
/// The state machine itself never performs the action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorAction {
    None,
    RunDiscoveryProbe,
    WaitForTerminal,
    ValidateVersion,
    StartBridge,
    AwaitHello,
    PublishRunning { generation: u64 },
    ScheduleBackoff { consecutive_failures: u32 },
    OpenCircuit { consecutive_failures: u32 },
    StopBridge { destination: StopDestination },
    PublishStopped,
}

/// Result of one accepted, deterministic transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisorTransition {
    pub previous: SupervisorState,
    pub current: SupervisorState,
    pub action: SupervisorAction,
}

/// Pure lifecycle machine. Timeouts and jitter are executor inputs represented
/// by events; no wall clock or random source is consulted here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupervisorMachine {
    state: SupervisorState,
    max_restart_attempts: u32,
    consecutive_failures: u32,
    generation: u64,
}

impl SupervisorMachine {
    /// Creates a disabled lifecycle machine. A zero restart budget opens the
    /// circuit on the first restartable failure.
    pub fn new(max_restart_attempts: u32) -> Self {
        Self {
            state: SupervisorState::Disabled,
            max_restart_attempts,
            consecutive_failures: 0,
            generation: 0,
        }
    }

    pub fn state(&self) -> SupervisorState {
        self.state
    }

    pub fn max_restart_attempts(&self) -> u32 {
        self.max_restart_attempts
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Applies one external fact and returns the next required action.
    pub fn transition(
        &mut self,
        event: SupervisorEvent,
    ) -> Result<SupervisorTransition, TransitionError> {
        let previous = self.state;
        let (current, action) = match (previous, event) {
            (SupervisorState::Disabled | SupervisorState::Stopped, SupervisorEvent::Enable) => (
                SupervisorState::Discovering,
                SupervisorAction::RunDiscoveryProbe,
            ),
            (SupervisorState::WaitingForTerminal, SupervisorEvent::Rediscover) => (
                SupervisorState::Discovering,
                SupervisorAction::RunDiscoveryProbe,
            ),
            (SupervisorState::Discovering, SupervisorEvent::DiscoveryFound) => (
                SupervisorState::ValidatingVersion,
                SupervisorAction::ValidateVersion,
            ),
            (
                SupervisorState::Discovering,
                SupervisorEvent::DiscoveryAbsent | SupervisorEvent::DiscoveryRejected,
            ) => (
                SupervisorState::WaitingForTerminal,
                SupervisorAction::WaitForTerminal,
            ),
            (SupervisorState::Discovering, SupervisorEvent::DiscoveryFailed)
            | (SupervisorState::Starting, SupervisorEvent::BridgeStartFailed)
            | (SupervisorState::Handshaking, SupervisorEvent::HandshakeFailed)
            | (SupervisorState::Running, SupervisorEvent::RuntimeFailed) => {
                return self.restartable_failure(previous);
            }
            (SupervisorState::ValidatingVersion, SupervisorEvent::ValidationAccepted) => {
                (SupervisorState::Starting, SupervisorAction::StartBridge)
            }
            (SupervisorState::ValidatingVersion, SupervisorEvent::ValidationRejected) => (
                SupervisorState::WaitingForTerminal,
                SupervisorAction::WaitForTerminal,
            ),
            (SupervisorState::Starting, SupervisorEvent::BridgeStarted) => {
                (SupervisorState::Handshaking, SupervisorAction::AwaitHello)
            }
            (SupervisorState::Handshaking, SupervisorEvent::HelloAccepted) => {
                self.generation = self
                    .generation
                    .checked_add(1)
                    .ok_or(TransitionError::GenerationExhausted)?;
                (
                    SupervisorState::Running,
                    SupervisorAction::PublishRunning {
                        generation: self.generation,
                    },
                )
            }
            (SupervisorState::Running, SupervisorEvent::StabilityProved) => {
                self.consecutive_failures = 0;
                (SupervisorState::Running, SupervisorAction::None)
            }
            (SupervisorState::BackingOff, SupervisorEvent::BackoffElapsed) => (
                SupervisorState::Discovering,
                SupervisorAction::RunDiscoveryProbe,
            ),
            (SupervisorState::CircuitOpen, SupervisorEvent::CircuitReset) => {
                self.consecutive_failures = 0;
                (
                    SupervisorState::Discovering,
                    SupervisorAction::RunDiscoveryProbe,
                )
            }
            (
                SupervisorState::Running
                | SupervisorState::Starting
                | SupervisorState::Handshaking
                | SupervisorState::ValidatingVersion,
                SupervisorEvent::TerminalLost,
            ) => {
                let destination = StopDestination::WaitingForTerminal;
                (
                    SupervisorState::Stopping { destination },
                    SupervisorAction::StopBridge { destination },
                )
            }
            (
                SupervisorState::Discovering
                | SupervisorState::WaitingForTerminal
                | SupervisorState::ValidatingVersion
                | SupervisorState::Starting
                | SupervisorState::Handshaking
                | SupervisorState::Running
                | SupervisorState::BackingOff
                | SupervisorState::CircuitOpen,
                SupervisorEvent::StopRequested,
            ) => {
                let destination = StopDestination::Stopped;
                (
                    SupervisorState::Stopping { destination },
                    SupervisorAction::StopBridge { destination },
                )
            }
            (SupervisorState::Stopping { destination }, SupervisorEvent::StopCompleted) => {
                let current = match destination {
                    StopDestination::WaitingForTerminal => SupervisorState::WaitingForTerminal,
                    StopDestination::Stopped => SupervisorState::Stopped,
                };
                let action = match destination {
                    StopDestination::WaitingForTerminal => SupervisorAction::WaitForTerminal,
                    StopDestination::Stopped => SupervisorAction::PublishStopped,
                };
                (current, action)
            }
            (SupervisorState::Disabled, SupervisorEvent::StopRequested) => {
                (SupervisorState::Stopped, SupervisorAction::PublishStopped)
            }
            (SupervisorState::Stopped, SupervisorEvent::StopRequested) => {
                (SupervisorState::Stopped, SupervisorAction::None)
            }
            _ => {
                return Err(TransitionError::InvalidTransition {
                    state: previous,
                    event,
                });
            }
        };
        self.state = current;
        Ok(SupervisorTransition {
            previous,
            current,
            action,
        })
    }

    fn restartable_failure(
        &mut self,
        previous: SupervisorState,
    ) -> Result<SupervisorTransition, TransitionError> {
        let Some(next_failure_count) = self.consecutive_failures.checked_add(1) else {
            self.state = SupervisorState::CircuitOpen;
            return Ok(SupervisorTransition {
                previous,
                current: self.state,
                action: SupervisorAction::OpenCircuit {
                    consecutive_failures: self.consecutive_failures,
                },
            });
        };
        self.consecutive_failures = next_failure_count;
        let exhausted = self.consecutive_failures > self.max_restart_attempts;
        let (current, action) = if exhausted {
            (
                SupervisorState::CircuitOpen,
                SupervisorAction::OpenCircuit {
                    consecutive_failures: self.consecutive_failures,
                },
            )
        } else {
            (
                SupervisorState::BackingOff,
                SupervisorAction::ScheduleBackoff {
                    consecutive_failures: self.consecutive_failures,
                },
            )
        };
        self.state = current;
        Ok(SupervisorTransition {
            previous,
            current,
            action,
        })
    }
}

/// A rejected state/event pair or exhausted stream generation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TransitionError {
    #[error("event {event:?} is invalid while supervisor is {state:?}")]
    InvalidTransition {
        state: SupervisorState,
        event: SupervisorEvent,
    },
    #[error("bridge stream generation exhausted")]
    GenerationExhausted,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transition(machine: &mut SupervisorMachine, event: SupervisorEvent) -> SupervisorAction {
        machine.transition(event).unwrap().action
    }

    fn reach_handshake(machine: &mut SupervisorMachine) {
        transition(machine, SupervisorEvent::Enable);
        transition(machine, SupervisorEvent::DiscoveryFound);
        transition(machine, SupervisorEvent::ValidationAccepted);
        transition(machine, SupervisorEvent::BridgeStarted);
    }

    #[test]
    fn absent_terminal_waits_without_requesting_bridge_start() {
        let mut machine = SupervisorMachine::new(3);
        assert_eq!(
            transition(&mut machine, SupervisorEvent::Enable),
            SupervisorAction::RunDiscoveryProbe
        );
        assert_eq!(
            transition(&mut machine, SupervisorEvent::DiscoveryAbsent),
            SupervisorAction::WaitForTerminal
        );
        assert_eq!(machine.state(), SupervisorState::WaitingForTerminal);
        assert_eq!(machine.consecutive_failures(), 0);
    }

    #[test]
    fn admitted_terminal_reaches_versioned_running_generation() {
        let mut machine = SupervisorMachine::new(3);
        reach_handshake(&mut machine);
        assert_eq!(machine.state(), SupervisorState::Handshaking);
        assert_eq!(
            transition(&mut machine, SupervisorEvent::HelloAccepted),
            SupervisorAction::PublishRunning { generation: 1 }
        );
        assert_eq!(machine.state(), SupervisorState::Running);
        assert_eq!(machine.generation(), 1);
    }

    #[test]
    fn rejected_version_fails_closed_to_waiting() {
        let mut machine = SupervisorMachine::new(3);
        transition(&mut machine, SupervisorEvent::Enable);
        transition(&mut machine, SupervisorEvent::DiscoveryFound);
        assert_eq!(
            transition(&mut machine, SupervisorEvent::ValidationRejected),
            SupervisorAction::WaitForTerminal
        );
        assert_eq!(machine.state(), SupervisorState::WaitingForTerminal);
    }

    #[test]
    fn restart_budget_backs_off_then_opens_the_circuit() {
        let mut machine = SupervisorMachine::new(2);
        transition(&mut machine, SupervisorEvent::Enable);
        assert_eq!(
            transition(&mut machine, SupervisorEvent::DiscoveryFailed),
            SupervisorAction::ScheduleBackoff {
                consecutive_failures: 1
            }
        );
        transition(&mut machine, SupervisorEvent::BackoffElapsed);
        assert_eq!(
            transition(&mut machine, SupervisorEvent::DiscoveryFailed),
            SupervisorAction::ScheduleBackoff {
                consecutive_failures: 2
            }
        );
        transition(&mut machine, SupervisorEvent::BackoffElapsed);
        assert_eq!(
            transition(&mut machine, SupervisorEvent::DiscoveryFailed),
            SupervisorAction::OpenCircuit {
                consecutive_failures: 3
            }
        );
        assert_eq!(machine.state(), SupervisorState::CircuitOpen);
    }

    #[test]
    fn zero_restart_budget_opens_on_first_failure() {
        let mut machine = SupervisorMachine::new(0);
        transition(&mut machine, SupervisorEvent::Enable);
        transition(&mut machine, SupervisorEvent::DiscoveryFailed);
        assert_eq!(machine.state(), SupervisorState::CircuitOpen);
    }

    #[test]
    fn stability_evidence_resets_consecutive_failure_budget() {
        let mut machine = SupervisorMachine::new(2);
        reach_handshake(&mut machine);
        transition(&mut machine, SupervisorEvent::HandshakeFailed);
        transition(&mut machine, SupervisorEvent::BackoffElapsed);
        transition(&mut machine, SupervisorEvent::DiscoveryFound);
        transition(&mut machine, SupervisorEvent::ValidationAccepted);
        transition(&mut machine, SupervisorEvent::BridgeStarted);
        transition(&mut machine, SupervisorEvent::HelloAccepted);
        assert_eq!(machine.consecutive_failures(), 1);
        transition(&mut machine, SupervisorEvent::StabilityProved);
        assert_eq!(machine.consecutive_failures(), 0);
    }

    #[test]
    fn terminal_loss_requires_cleanup_before_waiting() {
        let mut machine = SupervisorMachine::new(3);
        reach_handshake(&mut machine);
        transition(&mut machine, SupervisorEvent::HelloAccepted);
        let destination = StopDestination::WaitingForTerminal;
        assert_eq!(
            transition(&mut machine, SupervisorEvent::TerminalLost),
            SupervisorAction::StopBridge { destination }
        );
        assert_eq!(machine.state(), SupervisorState::Stopping { destination });
        assert_eq!(
            transition(&mut machine, SupervisorEvent::StopCompleted),
            SupervisorAction::WaitForTerminal
        );
        assert_eq!(machine.state(), SupervisorState::WaitingForTerminal);
    }

    #[test]
    fn explicit_stop_reaches_stopped_and_can_be_enabled_again() {
        let mut machine = SupervisorMachine::new(3);
        transition(&mut machine, SupervisorEvent::Enable);
        transition(&mut machine, SupervisorEvent::DiscoveryAbsent);
        transition(&mut machine, SupervisorEvent::StopRequested);
        assert_eq!(
            transition(&mut machine, SupervisorEvent::StopCompleted),
            SupervisorAction::PublishStopped
        );
        assert_eq!(machine.state(), SupervisorState::Stopped);
        transition(&mut machine, SupervisorEvent::Enable);
        assert_eq!(machine.state(), SupervisorState::Discovering);
    }

    #[test]
    fn invalid_transition_is_typed_and_does_not_mutate_state() {
        let mut machine = SupervisorMachine::new(3);
        let error = machine
            .transition(SupervisorEvent::HelloAccepted)
            .unwrap_err();
        assert_eq!(
            error,
            TransitionError::InvalidTransition {
                state: SupervisorState::Disabled,
                event: SupervisorEvent::HelloAccepted
            }
        );
        assert_eq!(machine.state(), SupervisorState::Disabled);
        assert_eq!(machine.generation(), 0);
    }

    #[test]
    fn circuit_reset_starts_a_fresh_bounded_retry_audit() {
        let mut machine = SupervisorMachine::new(0);
        transition(&mut machine, SupervisorEvent::Enable);
        transition(&mut machine, SupervisorEvent::DiscoveryFailed);
        assert_eq!(machine.consecutive_failures(), 1);
        assert_eq!(
            transition(&mut machine, SupervisorEvent::CircuitReset),
            SupervisorAction::RunDiscoveryProbe
        );
        assert_eq!(machine.consecutive_failures(), 0);
    }
}
