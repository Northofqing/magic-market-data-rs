use magic_cfets_rs::CfetsClient;
use magic_market_core::{
    CurrencyCode, IsoDate, OfficialFxFixingIdentity, OfficialFxFixingRequest, PositiveU32,
    ProviderId, ReferenceRateIdentity, ReferenceRateKind, ReferenceRateRequest, ReferenceTenor,
};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    if !matches!(arguments.len(), 3 | 5) {
        return Err("usage: live_probe SHIBOR_FX_START SHIBOR_FX_END [LPR_START LPR_END]".into());
    }
    let start = IsoDate::new(&arguments[1])?;
    let end = IsoDate::new(&arguments[2])?;
    let (lpr_start, lpr_end) = lpr_range(&arguments, &end)?;
    let client = CfetsClient::new(Duration::from_secs(10))?;

    let shibor = ReferenceRateRequest::new(
        [
            ReferenceRateKind::Shibor(ReferenceTenor::Overnight),
            ReferenceRateKind::Shibor(ReferenceTenor::OneWeek),
        ]
        .into_iter()
        .map(|kind| ReferenceRateIdentity::new(ProviderId::Cfets, kind))
        .collect::<Result<Vec<_>, _>>()?,
        start.clone(),
        end.clone(),
        PositiveU32::new(20)?,
    )?;

    let lpr = ReferenceRateRequest::new(
        [
            ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OneYear),
            ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OverFiveYears),
        ]
        .into_iter()
        .map(|kind| ReferenceRateIdentity::new(ProviderId::Cfets, kind))
        .collect::<Result<Vec<_>, _>>()?,
        lpr_start,
        lpr_end,
        PositiveU32::new(20)?,
    )?;

    let fx = OfficialFxFixingRequest::new(
        [("USD", "CNY"), ("JPY", "CNY")]
            .into_iter()
            .map(|(base, quote)| {
                OfficialFxFixingIdentity::new(
                    ProviderId::Cfets,
                    CurrencyCode::new(base)?,
                    CurrencyCode::new(quote)?,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        start,
        end,
        PositiveU32::new(20)?,
    )?;

    let mut failures = Vec::new();
    if let Err(error) = print_rates(&client, "Shibor", &shibor) {
        eprintln!("Shibor probe failed: {error}");
        failures.push(format!("Shibor: {error}"));
    }
    if let Err(error) = print_rates(&client, "LPR", &lpr) {
        eprintln!("LPR probe failed: {error}");
        failures.push(format!("LPR: {error}"));
    }
    if let Err(error) = print_fx(&client, &fx) {
        eprintln!("official FX probe failed: {error}");
        failures.push(format!("official FX: {error}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(
            format!("CFETS live probe failures after all families were attempted: {failures:?}")
                .into(),
        )
    }
}

fn print_rates(
    client: &CfetsClient,
    label: &str,
    request: &ReferenceRateRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    for row in client.probe_reference_rates(request)?.records() {
        println!(
            "family={label} rate={:?} date={} value={} unit={:?} observed={} batch={}",
            row.identity().kind(),
            row.fixing_date(),
            row.rate().get(),
            row.unit(),
            row.evidence().observed_at(),
            row.evidence().batch_id()
        );
    }
    Ok(())
}

fn print_fx(
    client: &CfetsClient,
    request: &OfficialFxFixingRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    for row in client.probe_official_fx_fixings(request)?.records() {
        println!(
            "pair={}/{} date={} value={} base={} observed={} batch={}",
            row.base(),
            row.quote(),
            row.fixing_date(),
            row.value().get(),
            row.quotation_base().get(),
            row.evidence().observed_at(),
            row.evidence().batch_id()
        );
    }
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
