use magic_market_monitor::{
    AmountProcessOutcome, AmountSpikeRule, DeterministicAmountMonitor, DeterministicPriceMonitor,
    DeterministicVolumeMonitor, InjectedObservation, MonitorError, MonitorLimits, PriceChangeRule,
    ProcessOutcome, VolumeProcessOutcome, VolumeSpikeRule, LOCAL_AMOUNT_CHANGE_ANOMALY_ADMITTED,
    LOCAL_PRICE_CHANGE_ANOMALY_ADMITTED, LOCAL_VOLUME_CHANGE_ANOMALY_ADMITTED,
};
use magic_tdx_local_rs::{
    SupervisorEvent, SupervisorMachine, SupervisorTransition, TransitionError,
    LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED, LOCAL_TERMINAL_CUMULATIVE_VOLUME_ADMITTED,
    LOCAL_TERMINAL_PRICE_ADMITTED,
};

/// One independently governed capability required by the initial production
/// local-terminal monitor composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalMonitorCapability {
    LocalTerminalPrice,
    LocalTerminalCumulativeAmount,
    LocalTerminalCumulativeVolume,
    LocalTerminalSourceRecordCount,
    LocalPriceChangeAnomaly,
    LocalAmountChangeAnomaly,
    LocalVolumeChangeAnomaly,
}

/// Typed construction failure for the production composition boundary.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LocalTerminalMonitorCompositionError {
    #[error("local-terminal monitor capabilities are not admitted: {capabilities:?}")]
    CapabilityUnadmitted {
        capabilities: Vec<LocalMonitorCapability>,
    },
    #[error("local-terminal production composition is not implemented")]
    ImplementationUnavailable,
}

/// Production local-terminal monitor composition.
///
/// Its constructor accepts no bridge, transport, process, runtime, or evidence
/// injection. Construction proves that the three raw inputs and three derived
/// event families are repository-admitted; the independently unavailable
/// source-record-count field is not a prerequisite. Construction performs no
/// I/O and starts no process, thread, subscription, network connection, or
/// listener.
pub struct LocalTerminalMonitorComposition {
    _private: (),
}

impl LocalTerminalMonitorComposition {
    pub fn new() -> Result<Self, LocalTerminalMonitorCompositionError> {
        let requirements = [
            (
                LocalMonitorCapability::LocalTerminalPrice,
                LOCAL_TERMINAL_PRICE_ADMITTED,
            ),
            (
                LocalMonitorCapability::LocalTerminalCumulativeAmount,
                LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED,
            ),
            (
                LocalMonitorCapability::LocalTerminalCumulativeVolume,
                LOCAL_TERMINAL_CUMULATIVE_VOLUME_ADMITTED,
            ),
            (
                LocalMonitorCapability::LocalPriceChangeAnomaly,
                LOCAL_PRICE_CHANGE_ANOMALY_ADMITTED,
            ),
            (
                LocalMonitorCapability::LocalAmountChangeAnomaly,
                LOCAL_AMOUNT_CHANGE_ANOMALY_ADMITTED,
            ),
            (
                LocalMonitorCapability::LocalVolumeChangeAnomaly,
                LOCAL_VOLUME_CHANGE_ANOMALY_ADMITTED,
            ),
        ];
        let capabilities = requirements
            .into_iter()
            .filter_map(|(capability, admitted)| (!admitted).then_some(capability))
            .collect::<Vec<_>>();
        if !capabilities.is_empty() {
            return Err(LocalTerminalMonitorCompositionError::CapabilityUnadmitted {
                capabilities,
            });
        }
        Ok(Self { _private: () })
    }
}

/// Explicitly diagnostic composition of two pure deterministic state machines.
///
/// This type is not a Provider, has no production constructor, creates no
/// `LocalTerminal` success evidence, and performs no external action. Supervisor
/// actions are descriptions for a separately governed executor; monitor
/// outcomes contain no production Provider identity or admission claim.
pub struct DiagnosticLocalTerminalMonitorComposition {
    supervisor: SupervisorMachine,
    price_monitor: DeterministicPriceMonitor,
    amount_monitor: DeterministicAmountMonitor,
    volume_monitor: DeterministicVolumeMonitor,
}

impl DiagnosticLocalTerminalMonitorComposition {
    pub fn new(
        max_restart_attempts: u32,
        monitor_limits: MonitorLimits,
        price_rule: PriceChangeRule,
        amount_rule: AmountSpikeRule,
        volume_rule: VolumeSpikeRule,
    ) -> Self {
        Self {
            supervisor: SupervisorMachine::new(max_restart_attempts),
            price_monitor: DeterministicPriceMonitor::new(monitor_limits, price_rule),
            amount_monitor: DeterministicAmountMonitor::new(monitor_limits, amount_rule),
            volume_monitor: DeterministicVolumeMonitor::new(monitor_limits, volume_rule),
        }
    }

    pub fn supervisor(&self) -> &SupervisorMachine {
        &self.supervisor
    }

    pub fn transition_supervisor(
        &mut self,
        event: SupervisorEvent,
    ) -> Result<SupervisorTransition, TransitionError> {
        self.supervisor.transition(event)
    }

    pub fn process_observation(
        &mut self,
        observation: InjectedObservation,
    ) -> Result<DiagnosticProcessOutcome, MonitorError> {
        let price = self.price_monitor.process(observation.clone())?;
        let amount = self.amount_monitor.process(observation.clone())?;
        let volume = self.volume_monitor.process(observation)?;
        Ok(DiagnosticProcessOutcome {
            price,
            amount,
            volume,
        })
    }
}

/// Independent results from one observation. One unavailable production
/// capability cannot silently promote another family.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticProcessOutcome {
    pub price: ProcessOutcome,
    pub amount: AmountProcessOutcome,
    pub volume: VolumeProcessOutcome,
}
