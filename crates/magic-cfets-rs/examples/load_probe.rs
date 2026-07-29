use magic_cfets_rs::CfetsClient;
use magic_market_core::{
    verify_serial_load, CurrencyCode, IsoDate, OfficialFxFixingIdentity, OfficialFxFixingRequest,
    PositiveU32, ProviderId, ReferenceRateIdentity, ReferenceRateKind, ReferenceRateRequest,
    ReferenceTenor,
};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    if !matches!(arguments.len(), 3 | 5) {
        return Err("usage: load_probe SHIBOR_FX_START SHIBOR_FX_END [LPR_START LPR_END]".into());
    }
    let start = IsoDate::new(&arguments[1])?;
    let end = IsoDate::new(&arguments[2])?;
    let (lpr_start, lpr_end) = lpr_range(&arguments, &end)?;
    let shibor = ReferenceRateRequest::new(
        vec![ReferenceRateIdentity::new(
            ProviderId::Cfets,
            ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
        )?],
        start.clone(),
        end.clone(),
        PositiveU32::new(20)?,
    )?;
    let lpr = ReferenceRateRequest::new(
        vec![ReferenceRateIdentity::new(
            ProviderId::Cfets,
            ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OneYear),
        )?],
        lpr_start,
        lpr_end,
        PositiveU32::new(20)?,
    )?;

    let fx = OfficialFxFixingRequest::new(
        vec![OfficialFxFixingIdentity::new(
            ProviderId::Cfets,
            CurrencyCode::new("USD")?,
            CurrencyCode::new("CNY")?,
        )?],
        start,
        end,
        PositiveU32::new(50)?,
    )?;

    let mut failures = Vec::new();
    if let Err(error) = probe_rates("Shibor", &shibor) {
        eprintln!("Shibor load probe failed: {error}");
        failures.push(format!("Shibor: {error}"));
    }
    if let Err(error) = probe_rates("LPR", &lpr) {
        eprintln!("LPR load probe failed: {error}");
        failures.push(format!("LPR: {error}"));
    }
    if let Err(error) = probe_fx(&fx) {
        eprintln!("official FX load probe failed: {error}");
        failures.push(format!("official FX: {error}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(
            format!("CFETS load probe failures after all families were attempted: {failures:?}")
                .into(),
        )
    }
}

fn probe_rates(
    label: &str,
    request: &ReferenceRateRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = CfetsClient::new(Duration::from_secs(10))?;
    for _ in 0..3 {
        let _ = client.probe_reference_rates(request)?;
    }
    let snapshot = client.load_probe_snapshot()?;
    if snapshot.request_starts() != 3 {
        return Err(format!(
            "{label} load probe expected exactly 3 transport starts, got {}",
            snapshot.request_starts()
        )
        .into());
    }
    let status = verify_serial_load(&snapshot, Duration::from_secs(1))?;
    println!("CFETS three-request {label} load probe: {status}");
    Ok(())
}

fn probe_fx(request: &OfficialFxFixingRequest) -> Result<(), Box<dyn std::error::Error>> {
    let client = CfetsClient::new(Duration::from_secs(10))?;
    for _ in 0..3 {
        let _ = client.probe_official_fx_fixings(request)?;
    }
    let snapshot = client.load_probe_snapshot()?;
    let status = verify_serial_load(&snapshot, Duration::from_secs(1))?;
    println!(
        "CFETS three-call official FX load probe: {status}; transport_starts={}",
        snapshot.request_starts()
    );
    Ok(())
}

fn lpr_range(
    arguments: &[String],
    shibor_end: &IsoDate,
) -> Result<(IsoDate, IsoDate), Box<dyn std::error::Error>> {
    if arguments.len() == 5 {
        return Ok((IsoDate::new(&arguments[3])?, IsoDate::new(&arguments[4])?));
    }
    let end = shibor_end.as_str();
    Ok((
        IsoDate::new(format!("{}-01", &end[..7]))?,
        shibor_end.clone(),
    ))
}
