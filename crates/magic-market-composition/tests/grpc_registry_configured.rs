//! Public execution boundaries with isolated, non-secret configuration.
//! No valid provider request is issued and repository admission is unchanged.

use magic_market_composition::production_operation_registry;
use magic_market_service::{CanonicalPayload, Operation, QueryCommand, ServiceError};
use std::{process::Command, time::Duration};

const CHILD_MARKER: &str = "MAGIC_MARKET_CONFIGURED_REGISTRY_TEST_CHILD";
const CHILD_VALUE: &str = "fixed-public-fixtures-v1";
const FIXTURE_KEY: &str = "offline-registry-fixture-not-a-real-api-key";
const FIXTURE_USER_AGENT: &str = "MagicMarketOfflineRegression/1 fixtures@example.invalid";

#[test]
fn configured_registry_refuses_invalid_envelopes_before_io() {
    if std::env::var(CHILD_MARKER).as_deref() != Ok(CHILD_VALUE) {
        let executable = std::env::current_exe().unwrap();
        let absent_bridge = executable.with_extension("offline-absent-bridge");
        assert!(
            !absent_bridge.exists(),
            "fixture bridge path must be absent"
        );
        let mut child = Command::new(executable);
        child
            .args([
                "--exact",
                "configured_registry_refuses_invalid_envelopes_before_io",
                "--test-threads=1",
            ])
            .env_clear();
        // Keep native loader/temp settings and genuine coverage instrumentation,
        // never inherit a real provider secret, endpoint override or bridge.
        for name in [
            "SYSTEMROOT",
            "WINDIR",
            "PATH",
            "TEMP",
            "TMP",
            "TMPDIR",
            "LANG",
            "LC_ALL",
            "LLVM_PROFILE_FILE",
        ] {
            if let Some(value) = std::env::var_os(name) {
                child.env(name, value);
            }
        }
        for name in [
            "HITHINK_FINANCE_API_KEY",
            "IWENCAI_API_KEY",
            "FRED_API_KEY",
            "EASTMONEY_API_KEY",
        ] {
            child.env(name, FIXTURE_KEY);
        }
        let output = child
            .env("SEC_USER_AGENT", FIXTURE_USER_AGENT)
            .env("MAGIC_EMQUANT_BRIDGE", absent_bridge)
            .env(CHILD_MARKER, CHILD_VALUE)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated configured registry test failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return;
    }

    for name in [
        "HITHINK_FINANCE_API_KEY",
        "IWENCAI_API_KEY",
        "FRED_API_KEY",
        "EASTMONEY_API_KEY",
    ] {
        assert_eq!(std::env::var(name).unwrap(), FIXTURE_KEY);
    }
    assert_eq!(std::env::var("SEC_USER_AGENT").unwrap(), FIXTURE_USER_AGENT);
    let registry = production_operation_registry(Duration::from_secs(1), 16_384).unwrap();
    let capabilities = registry.capabilities();
    for (operation, provider) in [
        (Operation::HistoricalBars, "HithinkFinance"),
        (Operation::RealtimeQuotes, "HithinkFinance"),
        (Operation::MarketStatistics, "HithinkFinance"),
        (Operation::LimitPools, "HithinkFinance"),
        (Operation::Popularity, "HithinkFinance"),
        (Operation::FinancialStatements, "HithinkFinance"),
        (Operation::CorporateActions, "HithinkFinance"),
        (Operation::SecurityMetadata, "HithinkFinance"),
        (Operation::CurrentAuctionObservations, "HithinkFinance"),
        (Operation::EconomicSeries, "Fred"),
        (Operation::EconomicReleaseSchedule, "Fred"),
        (Operation::CompanyFilings, "SecEdgar"),
        (Operation::SemanticSearch, "Iwencai"),
        (Operation::MarketBreadth, "EastmoneyMiaoxiang"),
    ] {
        let capability = capabilities
            .iter()
            .find(|item| item.operation == operation && item.provider == provider)
            .unwrap_or_else(|| panic!("missing configured route {operation:?}/{provider}"));
        assert!(capability.repository_admitted, "{operation:?}/{provider}");
        assert!(capability.runtime_available, "{operation:?}/{provider}");
    }
    for provider in ["HithinkFinance", "EastmoneyMiaoxiang"] {
        let auction = capabilities
            .iter()
            .find(|item| item.operation == Operation::Auctions && item.provider == provider)
            .unwrap();
        assert!(!auction.repository_admitted);
        assert!(!auction.runtime_available);
        assert!(auction.diagnostic_available);
    }

    let mut checked = 0;
    for capability in capabilities {
        let callable = capability.repository_admitted && capability.runtime_available
            || capability.diagnostic_available;
        let payload = CanonicalPayload::new(
            "magic.market.offline.invalid_schema.request",
            99,
            b"{}".to_vec(),
            4096,
        )
        .unwrap();
        let command = QueryCommand::new(
            "configured-registry-invalid-envelope",
            capability.operation,
            Some(capability.provider.clone()),
            payload,
        )
        .unwrap()
        .with_unadmitted_access(true);
        let result = registry.execute(command);
        if callable {
            assert!(
                matches!(result, Err(ServiceError::InvalidRequest(_))),
                "invalid envelope reached a configured provider: {:?}/{}: {result:?}",
                capability.operation,
                capability.provider
            );
            checked += 1;
        } else if capability.repository_admitted {
            assert!(
                matches!(result, Err(ServiceError::Unavailable { operation, .. })
                    if operation == capability.operation),
                "admitted but unavailable route misclassified: {:?}/{}: {result:?}",
                capability.operation,
                capability.provider
            );
        } else {
            assert!(
                matches!(result, Err(ServiceError::Unsupported { operation, .. })
                    if operation == capability.operation),
                "unavailable route became executable: {:?}/{}: {result:?}",
                capability.operation,
                capability.provider
            );
        }
    }
    assert!(
        checked >= 64,
        "the configured public route matrix was empty"
    );
}
