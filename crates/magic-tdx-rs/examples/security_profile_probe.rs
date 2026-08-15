use std::{error::Error, time::Instant};

use magic_market_core::{
    AssetClass, Exchange, InstrumentId, ProviderId, SecurityProfiles, SourcedRecord,
};
use magic_tdx_rs::{TdxSecurityProfileProvider, SECURITY_PROFILES_ADMITTED};

const SERVER: &str = "180.153.18.170";
const PORT: u16 = 7709;
const TIMEOUT_SECONDS: f64 = 10.0;

fn main() -> Result<(), Box<dyn Error>> {
    let requests = std::env::var("MAGIC_TDX_SECURITY_PROFILE_REQUESTS")
        .unwrap_or_else(|_| "1".into())
        .parse::<u8>()?;
    if !(1..=3).contains(&requests) {
        return Err("MAGIC_TDX_SECURITY_PROFILE_REQUESTS must be in 1..=3".into());
    }
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)?;
    let provider = TdxSecurityProfileProvider::new(SERVER, PORT, TIMEOUT_SECONDS)?;
    let mut successes = 0_u8;
    let started = Instant::now();
    for attempt in 1..=requests {
        let batch = if SECURITY_PROFILES_ADMITTED {
            provider.security_profiles(std::slice::from_ref(&instrument))?
        } else {
            provider.diagnostic_security_profiles(std::slice::from_ref(&instrument))?
        };
        if !batch.quality().is_complete() || batch.records().len() != 1 {
            return Err(format!("attempt {attempt} returned an incomplete profile batch").into());
        }
        let profile = &batch.records()[0];
        if profile.instrument != instrument
            || profile.provider_id() != ProviderId::Tdx
            || profile.name.as_str().is_empty()
            || profile.facts.is_empty()
            || profile.evidence.source_at().is_some()
        {
            return Err(format!("attempt {attempt} violated the profile contract").into());
        }
        println!(
            "attempt={attempt} provider={:?} code={} name={} listed_on={:?} facts={} batch_id={} source_at={:?}",
            profile.provider_id(),
            profile.instrument.code(),
            profile.name,
            profile.listed_on.as_ref().map(|value| value.as_str()),
            profile.facts.len(),
            profile.evidence.batch_id(),
            profile.evidence.source_at()
        );
        successes += 1;
    }
    println!(
        "attempts={requests} successes={successes} elapsed_ms={} admission={} probe_status={}",
        started.elapsed().as_millis(),
        SECURITY_PROFILES_ADMITTED,
        if SECURITY_PROFILES_ADMITTED {
            "admitted"
        } else {
            "diagnostic_complete_unadmitted"
        }
    );
    Ok(())
}
