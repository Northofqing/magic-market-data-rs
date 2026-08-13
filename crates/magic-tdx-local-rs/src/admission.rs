//! Repository-governed admission gates for the local terminal source.
//!
//! Runtime bridge facts never modify these constants. Each capability remains
//! false until its own deterministic contracts, bounded live probes and
//! registry evidence have passed the repository's admission gates.

/// Local-terminal price observations have not completed admission.
pub const LOCAL_TERMINAL_PRICE_ADMITTED: bool = false;

/// Local-terminal cumulative amount observations have not completed admission.
pub const LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED: bool = false;

/// Local-terminal cumulative volume observations have not completed admission.
pub const LOCAL_TERMINAL_CUMULATIVE_VOLUME_ADMITTED: bool = false;

/// Local-terminal source-record-count evidence has not completed admission.
pub const LOCAL_TERMINAL_SOURCE_RECORD_COUNT_ADMITTED: bool = false;

const PRICE_CAPABILITY: &str = "price";
const CUMULATIVE_AMOUNT_CAPABILITY: &str = "cumulative_amount";
const CUMULATIVE_VOLUME_CAPABILITY: &str = "cumulative_volume";
const SOURCE_RECORD_COUNT_CAPABILITY: &str = "source_record_count";

/// Repository-owned capability state. Runtime evidence cannot change it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryAdmission {
    Admitted,
    Unadmitted,
}

impl RepositoryAdmission {
    const fn from_bool(admitted: bool) -> Self {
        if admitted {
            Self::Admitted
        } else {
            Self::Unadmitted
        }
    }
}

/// Runtime evidence for one independently admitted source capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAvailability {
    Available,
    TerminalNotReady,
    CapabilityNotReported,
    CapabilityUnavailable,
    EntitlementNotReported,
    EntitlementUnavailable,
}

/// The two gates for one capability. Effective availability is deliberately
/// computed as a strict AND rather than cached or inferred from bridge claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityAvailability {
    pub repository: RepositoryAdmission,
    pub runtime: RuntimeAvailability,
}

impl CapabilityAvailability {
    pub const fn is_effectively_available(self) -> bool {
        matches!(self.repository, RepositoryAdmission::Admitted)
            && matches!(self.runtime, RuntimeAvailability::Available)
    }
}

/// Runtime evidence separated by source field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalTerminalRuntimeAvailability {
    pub price: RuntimeAvailability,
    pub cumulative_amount: RuntimeAvailability,
    pub cumulative_volume: RuntimeAvailability,
    pub source_record_count: RuntimeAvailability,
}

impl LocalTerminalRuntimeAvailability {
    /// Derives runtime facts from a validated hello. Missing capability or
    /// entitlement keys fail closed and remain distinguishable from `false`.
    pub fn from_hello(hello: &crate::protocol::Hello) -> Result<Self, crate::ProtocolError> {
        hello.validate()?;
        Ok(Self {
            price: runtime_availability(hello, PRICE_CAPABILITY),
            cumulative_amount: runtime_availability(hello, CUMULATIVE_AMOUNT_CAPABILITY),
            cumulative_volume: runtime_availability(hello, CUMULATIVE_VOLUME_CAPABILITY),
            source_record_count: runtime_availability(hello, SOURCE_RECORD_COUNT_CAPABILITY),
        })
    }
}

/// Effective state for all local-terminal observation fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalTerminalCapabilityAvailability {
    pub price: CapabilityAvailability,
    pub cumulative_amount: CapabilityAvailability,
    pub cumulative_volume: CapabilityAvailability,
    pub source_record_count: CapabilityAvailability,
}

/// Immutable repository admission view. This is intentionally separate from
/// the source-reported capabilities and entitlements carried by
/// [`Hello`](crate::protocol::Hello).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalTerminalAdmission {
    pub price: bool,
    pub cumulative_amount: bool,
    pub cumulative_volume: bool,
    pub source_record_count: bool,
}

impl LocalTerminalAdmission {
    /// Returns the compile-time repository admission state.
    pub const fn current() -> Self {
        Self {
            price: LOCAL_TERMINAL_PRICE_ADMITTED,
            cumulative_amount: LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED,
            cumulative_volume: LOCAL_TERMINAL_CUMULATIVE_VOLUME_ADMITTED,
            source_record_count: LOCAL_TERMINAL_SOURCE_RECORD_COUNT_ADMITTED,
        }
    }

    /// Combines immutable repository admission and runtime evidence without
    /// allowing either side to promote the other.
    pub const fn combine(
        self,
        runtime: LocalTerminalRuntimeAvailability,
    ) -> LocalTerminalCapabilityAvailability {
        LocalTerminalCapabilityAvailability {
            price: CapabilityAvailability {
                repository: RepositoryAdmission::from_bool(self.price),
                runtime: runtime.price,
            },
            cumulative_amount: CapabilityAvailability {
                repository: RepositoryAdmission::from_bool(self.cumulative_amount),
                runtime: runtime.cumulative_amount,
            },
            cumulative_volume: CapabilityAvailability {
                repository: RepositoryAdmission::from_bool(self.cumulative_volume),
                runtime: runtime.cumulative_volume,
            },
            source_record_count: CapabilityAvailability {
                repository: RepositoryAdmission::from_bool(self.source_record_count),
                runtime: runtime.source_record_count,
            },
        }
    }
}

fn runtime_availability(hello: &crate::protocol::Hello, name: &str) -> RuntimeAvailability {
    if hello.terminal_state != crate::protocol::TerminalState::Ready {
        return RuntimeAvailability::TerminalNotReady;
    }
    match hello.capabilities.get(name) {
        None => return RuntimeAvailability::CapabilityNotReported,
        Some(false) => return RuntimeAvailability::CapabilityUnavailable,
        Some(true) => {}
    }
    match hello.entitlements.get(name) {
        None => RuntimeAvailability::EntitlementNotReported,
        Some(false) => RuntimeAvailability::EntitlementUnavailable,
        Some(true) => RuntimeAvailability::Available,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        ArtifactIdentity, Hello, TerminalState, PROTOCOL_VERSION, SCHEMA_VERSION,
    };
    use std::collections::BTreeMap;

    fn artifact(filename: &str, hash_byte: char) -> ArtifactIdentity {
        ArtifactIdentity {
            filename: filename.into(),
            product_version: "1.0.0".into(),
            file_version: "1.0.0.1".into(),
            sha256: std::iter::repeat_n(hash_byte, 64).collect(),
        }
    }

    fn maximally_claiming_hello() -> Hello {
        Hello {
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
            peer: artifact("magic-tdx-fake-peer.exe", 'a'),
            peer_architecture: "x86_64-pc-windows-msvc".into(),
            terminal: artifact("TdxW.exe", 'b'),
            transport_profile_id: "official-tq-local-http-v1".into(),
            terminal_state: TerminalState::Ready,
            capabilities: BTreeMap::from([
                ("price".into(), true),
                ("cumulative_amount".into(), true),
                ("cumulative_volume".into(), true),
                ("source_record_count".into(), true),
            ]),
            entitlements: BTreeMap::from([
                ("price".into(), true),
                ("cumulative_amount".into(), true),
                ("cumulative_volume".into(), true),
                ("source_record_count".into(), true),
            ]),
        }
    }

    #[test]
    fn every_local_terminal_capability_is_false_by_default() {
        const {
            assert!(!LOCAL_TERMINAL_PRICE_ADMITTED);
            assert!(!LOCAL_TERMINAL_CUMULATIVE_AMOUNT_ADMITTED);
            assert!(!LOCAL_TERMINAL_CUMULATIVE_VOLUME_ADMITTED);
            assert!(!LOCAL_TERMINAL_SOURCE_RECORD_COUNT_ADMITTED);
        }
        assert_eq!(
            LocalTerminalAdmission::current(),
            LocalTerminalAdmission {
                price: false,
                cumulative_amount: false,
                cumulative_volume: false,
                source_record_count: false,
            }
        );
    }

    #[test]
    fn runtime_capability_and_entitlement_claims_do_not_modify_repository_admission() {
        let hello = maximally_claiming_hello();
        hello.validate().unwrap();
        assert!(hello.capabilities.values().all(|value| *value));
        assert!(hello.entitlements.values().all(|value| *value));
        assert_eq!(
            LocalTerminalAdmission::current(),
            LocalTerminalAdmission {
                price: false,
                cumulative_amount: false,
                cumulative_volume: false,
                source_record_count: false,
            }
        );

        let runtime = LocalTerminalRuntimeAvailability::from_hello(&hello).unwrap();
        let combined = LocalTerminalAdmission::current().combine(runtime);
        assert_eq!(combined.price.runtime, RuntimeAvailability::Available);
        assert_eq!(combined.price.repository, RepositoryAdmission::Unadmitted);
        assert!(!combined.price.is_effectively_available());
        assert!(!combined.cumulative_amount.is_effectively_available());
        assert!(!combined.cumulative_volume.is_effectively_available());
        assert!(!combined.source_record_count.is_effectively_available());
    }

    #[test]
    fn runtime_availability_is_typed_and_fail_closed_per_capability() {
        let mut hello = maximally_claiming_hello();
        hello.capabilities.remove(CUMULATIVE_AMOUNT_CAPABILITY);
        hello
            .capabilities
            .insert(CUMULATIVE_VOLUME_CAPABILITY.into(), false);
        hello.entitlements.remove(SOURCE_RECORD_COUNT_CAPABILITY);
        hello.entitlements.insert(PRICE_CAPABILITY.into(), false);

        let runtime = LocalTerminalRuntimeAvailability::from_hello(&hello).unwrap();
        assert_eq!(runtime.price, RuntimeAvailability::EntitlementUnavailable);
        assert_eq!(
            runtime.cumulative_amount,
            RuntimeAvailability::CapabilityNotReported
        );
        assert_eq!(
            runtime.cumulative_volume,
            RuntimeAvailability::CapabilityUnavailable
        );
        assert_eq!(
            runtime.source_record_count,
            RuntimeAvailability::EntitlementNotReported
        );

        hello.terminal_state = TerminalState::NotLoggedIn;
        let runtime = LocalTerminalRuntimeAvailability::from_hello(&hello).unwrap();
        assert_eq!(runtime.price, RuntimeAvailability::TerminalNotReady);
        assert_eq!(
            runtime.cumulative_amount,
            RuntimeAvailability::TerminalNotReady
        );
    }

    #[test]
    fn effective_availability_requires_both_repository_and_runtime_gates() {
        let admitted = CapabilityAvailability {
            repository: RepositoryAdmission::Admitted,
            runtime: RuntimeAvailability::Available,
        };
        assert!(admitted.is_effectively_available());
        assert!(!CapabilityAvailability {
            repository: RepositoryAdmission::Unadmitted,
            runtime: RuntimeAvailability::Available,
        }
        .is_effectively_available());
        assert!(!CapabilityAvailability {
            repository: RepositoryAdmission::Admitted,
            runtime: RuntimeAvailability::CapabilityUnavailable,
        }
        .is_effectively_available());
    }
}
