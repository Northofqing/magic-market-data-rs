use crate::{CfetsError, MAX_RESPONSE_BYTES};
use magic_market_core::{
    DataBatch, FiniteNumber, IsoDate, Provenance, ProviderId, RatioUnit, ReferenceRateIdentity,
    ReferenceRateKind, ReferenceRateObservation, ReferenceRateRequest, ReferenceTenor,
    SourceEvidence,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

const SHIBOR_COLUMNS: [(&str, &str, ReferenceTenor); 8] = [
    ("O/N", "ON", ReferenceTenor::Overnight),
    ("1W", "1W", ReferenceTenor::OneWeek),
    ("2W", "2W", ReferenceTenor::TwoWeeks),
    ("1M", "1M", ReferenceTenor::OneMonth),
    ("3M", "3M", ReferenceTenor::ThreeMonths),
    ("6M", "6M", ReferenceTenor::SixMonths),
    ("9M", "9M", ReferenceTenor::NineMonths),
    ("1Y", "1Y", ReferenceTenor::OneYear),
];

#[derive(Deserialize)]
struct RateEnvelope {
    data: RateData,
    records: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateData {
    #[serde(default)]
    base_curve_cfg_list: Value,
    #[serde(rename = "startDateCN", alias = "strStartDate")]
    start_date_cn: String,
    #[serde(rename = "endDateCN", alias = "strEndDate")]
    end_date_cn: String,
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurveColumn {
    cfg_item: String,
    cfg_item_nm: String,
    sqnc_cd: usize,
}

pub fn parse_shibor_payload(
    body: &[u8],
    request: &ReferenceRateRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<DataBatch<ReferenceRateObservation>, CfetsError> {
    parse_rate_payload(body, request, observed_at, batch_id, RateFamily::Shibor)
}

pub fn parse_lpr_payload(
    body: &[u8],
    request: &ReferenceRateRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<DataBatch<ReferenceRateObservation>, CfetsError> {
    parse_rate_payload(body, request, observed_at, batch_id, RateFamily::Lpr)
}

#[derive(Clone, Copy)]
enum RateFamily {
    Shibor,
    Lpr,
}

fn parse_rate_payload(
    body: &[u8],
    request: &ReferenceRateRequest,
    observed_at: &str,
    batch_id: &str,
    family: RateFamily,
) -> Result<DataBatch<ReferenceRateObservation>, CfetsError> {
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(CfetsError::Protocol("rate response exceeds 2 MiB".into()));
    }
    if request.provider() != ProviderId::Cfets {
        return Err(CfetsError::InvalidRequest(
            "reference-rate provider must be CFETS".into(),
        ));
    }
    validate_requested_family(request, family)?;
    let envelope: RateEnvelope =
        serde_json::from_slice(body).map_err(|error| CfetsError::Decode(error.to_string()))?;
    if !envelope.data.message.trim().is_empty() {
        return Err(CfetsError::Protocol(format!(
            "CFETS message reported an error: {}",
            envelope.data.message
        )));
    }
    if envelope.data.start_date_cn != request.start().as_str()
        || envelope.data.end_date_cn != request.end().as_str()
    {
        return Err(CfetsError::Protocol(
            "rate source bounds differ from the request".into(),
        ));
    }
    if envelope.records.is_empty() {
        return Err(CfetsError::Protocol(
            "ordinary empty CFETS rate history is not a verified strict success".into(),
        ));
    }
    let columns: Vec<(&str, ReferenceRateKind)> = match family {
        RateFamily::Shibor => {
            let source_columns: Vec<CurveColumn> = serde_json::from_value(
                envelope.data.base_curve_cfg_list.clone(),
            )
            .map_err(|_| {
                CfetsError::Protocol(
                    "Shibor baseCurveCfgList must be the audited object array".into(),
                )
            })?;
            if source_columns.len() != SHIBOR_COLUMNS.len() {
                return Err(CfetsError::Protocol(
                    "Shibor must expose exactly eight tenors".into(),
                ));
            }
            for (index, (actual, expected)) in source_columns.iter().zip(SHIBOR_COLUMNS).enumerate()
            {
                if actual.cfg_item != expected.0
                    || actual.cfg_item_nm != expected.1
                    || actual.sqnc_cd != index + 1
                {
                    return Err(CfetsError::Protocol(
                        "Shibor tenor metadata differs from the audited order".into(),
                    ));
                }
            }
            SHIBOR_COLUMNS
                .iter()
                .map(|(_, field, tenor)| (*field, ReferenceRateKind::Shibor(*tenor)))
                .collect()
        }
        RateFamily::Lpr => {
            let headings: Vec<String> = serde_json::from_value(
                envelope.data.base_curve_cfg_list.clone(),
            )
            .map_err(|_| {
                CfetsError::Protocol("LPR baseCurveCfgList must be the audited string array".into())
            })?;
            if headings != ["1Y", "5Y"] {
                return Err(CfetsError::Protocol(
                    "LPR headings must be exactly 1Y and 5Y".into(),
                ));
            }
            vec![
                (
                    "1Y",
                    ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OneYear),
                ),
                (
                    "5Y",
                    ReferenceRateKind::LoanPrimeRate(ReferenceTenor::OverFiveYears),
                ),
            ]
        }
    };
    let requested: HashSet<&ReferenceRateKind> = request
        .rates()
        .iter()
        .map(ReferenceRateIdentity::kind)
        .collect();
    let evidence = SourceEvidence::new(ProviderId::Cfets, observed_at, batch_id)?;
    let mut dates = HashSet::new();
    let mut records = Vec::new();
    for raw in envelope.records {
        let object = raw
            .as_object()
            .ok_or_else(|| CfetsError::Protocol("rate record must be an object".into()))?;
        let date = object
            .get("showDateCN")
            .and_then(Value::as_str)
            .ok_or_else(|| CfetsError::Protocol("rate record date is missing".into()))?;
        let date = IsoDate::new(date)?;
        if date < *request.start() || date > *request.end() || !dates.insert(date.clone()) {
            return Err(CfetsError::Protocol(
                "rate date is duplicate or outside request bounds".into(),
            ));
        }
        for (field, kind) in &columns {
            let raw_value = object
                .get(*field)
                .and_then(Value::as_str)
                .ok_or_else(|| CfetsError::Protocol(format!("rate field {field} is missing")))?;
            if raw_value.trim().is_empty() {
                return Err(CfetsError::Protocol(format!("rate field {field} is blank")));
            }
            let value = raw_value
                .parse::<f64>()
                .map_err(|_| CfetsError::Protocol(format!("rate field {field} is not numeric")))?;
            let value = FiniteNumber::new(value)?;
            if requested.contains(kind) {
                records.push(ReferenceRateObservation::new(
                    ReferenceRateIdentity::new(ProviderId::Cfets, kind.clone())?,
                    date.clone(),
                    value,
                    RatioUnit::Percent,
                    None,
                    None,
                    evidence.clone(),
                )?);
            }
        }
    }
    if records.iter().any(|record| {
        request
            .rates()
            .iter()
            .position(|identity| identity.kind() == record.identity().kind())
            .is_none()
    }) {
        return Err(CfetsError::Protocol(
            "parsed rate identity is absent from the request".into(),
        ));
    }
    records.sort_by(|left, right| {
        let left_position = request
            .rates()
            .iter()
            .position(|identity| identity.kind() == left.identity().kind())
            .unwrap_or(usize::MAX);
        let right_position = request
            .rates()
            .iter()
            .position(|identity| identity.kind() == right.identity().kind())
            .unwrap_or(usize::MAX);
        left_position
            .cmp(&right_position)
            .then_with(|| left.fixing_date().cmp(right.fixing_date()))
    });
    records.truncate(request.max_rows().get() as usize);
    let source = match family {
        RateFamily::Shibor => "CFETS Shibor",
        RateFamily::Lpr => "CFETS Loan Prime Rate",
    };
    let provenance = Provenance::new(source, observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn validate_requested_family(
    request: &ReferenceRateRequest,
    family: RateFamily,
) -> Result<(), CfetsError> {
    for rate in request.rates() {
        let valid = matches!(
            (family, rate.kind()),
            (RateFamily::Shibor, ReferenceRateKind::Shibor(_))
                | (RateFamily::Lpr, ReferenceRateKind::LoanPrimeRate(_))
        );
        if !valid {
            return Err(CfetsError::InvalidRequest(
                "rate request family does not match the parser".into(),
            ));
        }
    }
    Ok(())
}
