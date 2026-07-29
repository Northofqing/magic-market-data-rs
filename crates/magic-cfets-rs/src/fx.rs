use crate::{CfetsError, MAX_RESPONSE_BYTES};
use magic_market_core::{
    CurrencyCode, DataBatch, FiniteNumber, IsoDate, OfficialFxFixing, OfficialFxFixingIdentity,
    OfficialFxFixingRequest, PositiveU32, Provenance, ProviderId, SourceEvidence,
};
use serde::Deserialize;
use std::collections::HashMap;

const MAX_PAGES: usize = 20;
const MAX_ROWS: usize = 1_000;

const HEADINGS: [(&str, &str, &str, u32); 25] = [
    ("USD", "CNY", "USD/CNY", 1),
    ("EUR", "CNY", "EUR/CNY", 1),
    ("JPY", "CNY", "100JPY/CNY", 100),
    ("HKD", "CNY", "HKD/CNY", 1),
    ("GBP", "CNY", "GBP/CNY", 1),
    ("AUD", "CNY", "AUD/CNY", 1),
    ("NZD", "CNY", "NZD/CNY", 1),
    ("SGD", "CNY", "SGD/CNY", 1),
    ("CHF", "CNY", "CHF/CNY", 1),
    ("CAD", "CNY", "CAD/CNY", 1),
    ("CNY", "MOP", "CNY/MOP", 1),
    ("CNY", "MYR", "CNY/MYR", 1),
    ("CNY", "RUB", "CNY/RUB", 1),
    ("CNY", "ZAR", "CNY/ZAR", 1),
    ("CNY", "KRW", "CNY/KRW", 1),
    ("CNY", "AED", "CNY/AED", 1),
    ("CNY", "SAR", "CNY/SAR", 1),
    ("CNY", "HUF", "CNY/HUF", 1),
    ("CNY", "PLN", "CNY/PLN", 1),
    ("CNY", "DKK", "CNY/DKK", 1),
    ("CNY", "SEK", "CNY/SEK", 1),
    ("CNY", "NOK", "CNY/NOK", 1),
    ("CNY", "TRY", "CNY/TRY", 1),
    ("CNY", "MXN", "CNY/MXN", 1),
    ("CNY", "THB", "CNY/THB", 1),
];

#[derive(Deserialize)]
struct Envelope {
    data: Metadata,
    records: Vec<Record>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    head: Vec<String>,
    total: usize,
    page_total: usize,
    page_size: usize,
    page_num: usize,
    currency: String,
    searchlist: Vec<String>,
    start_date: String,
    end_date: String,
    flag_message: String,
}

#[derive(Deserialize)]
struct Record {
    date: String,
    values: Vec<String>,
}

pub fn source_heading(identity: &OfficialFxFixingIdentity) -> Result<&'static str, CfetsError> {
    if identity.provider() != ProviderId::Cfets {
        return Err(CfetsError::InvalidRequest(
            "official FX identity provider must be CFETS".into(),
        ));
    }
    HEADINGS
        .iter()
        .find(|(base, quote, _, _)| {
            identity.base().as_str() == *base && identity.quote().as_str() == *quote
        })
        .map(|(_, _, heading, _)| *heading)
        .ok_or_else(|| {
            CfetsError::Unsupported(format!(
                "CFETS central parity pair {}/{} is outside the audited closed table",
                identity.base(),
                identity.quote()
            ))
        })
}

pub(crate) fn page_total(body: &[u8]) -> Result<usize, CfetsError> {
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(CfetsError::Protocol("FX response exceeds 2 MiB".into()));
    }
    let envelope: Envelope =
        serde_json::from_slice(body).map_err(|error| CfetsError::Decode(error.to_string()))?;
    if !(1..=MAX_PAGES).contains(&envelope.data.page_total) {
        return Err(CfetsError::Protocol(
            "FX pageTotal is outside 1 through 20".into(),
        ));
    }
    Ok(envelope.data.page_total)
}

pub fn parse_central_parity_pages<B: AsRef<[u8]>>(
    pages: &[B],
    request: &OfficialFxFixingRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<DataBatch<OfficialFxFixing>, CfetsError> {
    if pages.is_empty() || pages.len() > MAX_PAGES {
        return Err(CfetsError::Protocol(
            "FX request must contain 1 through 20 pages".into(),
        ));
    }
    if request.provider() != ProviderId::Cfets {
        return Err(CfetsError::InvalidRequest(
            "official FX request provider must be CFETS".into(),
        ));
    }
    let selected = request
        .pairs()
        .iter()
        .map(source_heading)
        .collect::<Result<Vec<_>, _>>()?;
    let selected_string = selected.join(",");
    let expected_head = HEADINGS
        .iter()
        .map(|(_, _, heading, _)| (*heading).to_owned())
        .collect::<Vec<_>>();
    let mut stable_head: Option<Vec<String>> = None;
    let mut stable_total = None;
    let mut stable_page_total = None;
    let mut rows = Vec::new();
    for (index, page) in pages.iter().enumerate() {
        let bytes = page.as_ref();
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(CfetsError::Protocol("FX response exceeds 2 MiB".into()));
        }
        let envelope: Envelope =
            serde_json::from_slice(bytes).map_err(|error| CfetsError::Decode(error.to_string()))?;
        let data = &envelope.data;
        if !data.flag_message.trim().is_empty()
            || data.start_date != request.start().as_str()
            || data.end_date != request.end().as_str()
        {
            return Err(CfetsError::Protocol(
                "FX message or source bounds violate the request".into(),
            ));
        }
        if data.page_num != index + 1
            || !(1..=MAX_PAGES).contains(&data.page_total)
            || data.page_size == 0
            || data.page_size > 50
        {
            return Err(CfetsError::Protocol(
                "FX pagination metadata is invalid".into(),
            ));
        }
        if data.currency != selected_string
            || data
                .searchlist
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                != selected
        {
            return Err(CfetsError::Protocol(
                "FX currency/searchlist differs from requested selected order".into(),
            ));
        }
        if data.head != expected_head {
            return Err(CfetsError::Protocol(
                "FX complete heading catalog differs from the exact audited 25-heading sequence"
                    .into(),
            ));
        }
        if data.total == 0 {
            return Err(CfetsError::Protocol(
                "ordinary empty CFETS FX history is not a verified strict success".into(),
            ));
        }
        if stable_head.as_ref().is_some_and(|head| head != &data.head)
            || stable_total.is_some_and(|total| total != data.total)
            || stable_page_total.is_some_and(|total| total != data.page_total)
        {
            return Err(CfetsError::Protocol(
                "FX page metadata changed across pages".into(),
            ));
        }
        stable_head.get_or_insert_with(|| data.head.clone());
        stable_total.get_or_insert(data.total);
        stable_page_total.get_or_insert(data.page_total);
        for record in envelope.records {
            if record.values.len() != selected.len() {
                return Err(CfetsError::Protocol(
                    "FX positional value count differs from selected currencies".into(),
                ));
            }
            rows.push(record);
            if rows.len() > MAX_ROWS {
                return Err(CfetsError::Protocol("FX row ceiling exceeded".into()));
            }
        }
    }
    if stable_page_total != Some(pages.len()) || stable_total != Some(rows.len()) {
        return Err(CfetsError::Protocol(
            "FX pages do not cover the declared complete result".into(),
        ));
    }
    if rows.is_empty() {
        return Err(CfetsError::Protocol(
            "ordinary empty CFETS FX history is not a verified strict success".into(),
        ));
    }

    let evidence = SourceEvidence::new(ProviderId::Cfets, observed_at, batch_id)?;
    let mut seen: HashMap<(String, String), f64> = HashMap::new();
    let mut records = Vec::new();
    for row in rows {
        let date = IsoDate::new(row.date)?;
        if date < *request.start() || date > *request.end() {
            return Err(CfetsError::Protocol(
                "FX fixing date is outside requested bounds".into(),
            ));
        }
        for ((identity, heading), raw) in request.pairs().iter().zip(&selected).zip(row.values) {
            let value = raw
                .parse::<f64>()
                .map_err(|_| CfetsError::Protocol(format!("FX value {raw:?} is not numeric")))?;
            let value = FiniteNumber::new(value)?;
            if value.get() <= 0.0 {
                return Err(CfetsError::Protocol(
                    "FX fixing value must be positive".into(),
                ));
            }
            if seen
                .insert((date.as_str().into(), (*heading).into()), value.get())
                .is_some()
            {
                return Err(CfetsError::Protocol(
                    "duplicate FX date/currency identity".into(),
                ));
            }
            let (_, _, _, base) = HEADINGS
                .iter()
                .find(|item| item.2 == *heading)
                .ok_or_else(|| CfetsError::Protocol("validated heading disappeared".into()))?;
            records.push(OfficialFxFixing::new(
                CurrencyCode::new(identity.base().as_str())?,
                CurrencyCode::new(identity.quote().as_str())?,
                date.clone(),
                value,
                PositiveU32::new(*base)?,
                None,
                None,
                evidence.clone(),
            )?);
        }
    }
    if records
        .iter()
        .any(|record| request_pair_position(request, record).is_none())
    {
        return Err(CfetsError::Protocol(
            "parsed FX identity is absent from the request".into(),
        ));
    }
    records.sort_by(|left, right| {
        request_pair_position(request, left)
            .unwrap_or(usize::MAX)
            .cmp(&request_pair_position(request, right).unwrap_or(usize::MAX))
            .then_with(|| left.fixing_date().cmp(right.fixing_date()))
    });
    records.truncate(request.max_rows().get() as usize);
    let provenance =
        Provenance::new("CFETS central parity", observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn request_pair_position(
    request: &OfficialFxFixingRequest,
    record: &OfficialFxFixing,
) -> Option<usize> {
    request.pairs().iter().position(|identity| {
        identity.base().as_str() == record.base().as_str()
            && identity.quote().as_str() == record.quote().as_str()
    })
}
