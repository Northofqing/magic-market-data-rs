use crate::SecEdgarError;
use magic_market_core::{
    CompanyFiling, CompanyFilingRequest, HttpsUrl, IsoDate, NonEmptyText, ProviderId,
    SecAccessionNumber, SecCompanyIdentity, SecPrimaryDocument, SourceEvidence,
};
use serde::Deserialize;
use std::collections::HashMap;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

pub(crate) const MAX_RECENT_FILINGS: usize = 2_000;
pub(crate) const MAX_OLDER_FILES: usize = 20;
pub(crate) const MAX_DECODED_FILINGS: usize = 20_000;
const MAX_COMPANY_NAME_CHARS: usize = 512;

#[derive(Debug)]
pub(crate) struct ParsedCompany {
    pub(crate) company: SecCompanyIdentity,
    pub(crate) company_name: String,
    pub(crate) records: Vec<CompanyFiling>,
    pub(crate) record_index: HashMap<FilingIdentity, usize>,
    pub(crate) signatures: HashMap<FilingIdentity, FilingSignature>,
    pub(crate) recent_decoded_count: usize,
    pub(crate) older_files: Vec<OlderFileDescriptor>,
}

#[derive(Debug, Clone)]
pub(crate) struct OlderFileDescriptor {
    pub(crate) name: String,
    pub(crate) filing_count: usize,
    pub(crate) filing_from: IsoDate,
    pub(crate) filing_to: IsoDate,
}

#[derive(Debug)]
pub(crate) struct ParsedOlder {
    pub(crate) records: Vec<CompanyFiling>,
    pub(crate) signatures: HashMap<FilingIdentity, FilingSignature>,
    pub(crate) decoded_count: usize,
    pub(crate) min_date: IsoDate,
    pub(crate) max_date: IsoDate,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CikWire {
    Text(String),
    Number(u64),
}

impl CikWire {
    fn into_string(self) -> String {
        match self {
            Self::Text(value) => value,
            Self::Number(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ParentWire {
    cik: CikWire,
    name: String,
    #[serde(default)]
    tickers: Vec<String>,
    filings: ParentFilingsWire,
}

#[derive(Debug, Deserialize)]
struct ParentFilingsWire {
    recent: FilingArrays,
    #[serde(default)]
    files: Vec<OlderFileWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OlderFileWire {
    name: String,
    filing_count: usize,
    filing_from: String,
    filing_to: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilingArrays {
    accession_number: Vec<String>,
    filing_date: Vec<String>,
    report_date: Vec<String>,
    acceptance_date_time: Vec<String>,
    act: Vec<String>,
    form: Vec<String>,
    file_number: Vec<String>,
    film_number: Vec<String>,
    items: Vec<String>,
    size: Vec<u64>,
    #[serde(rename = "isXBRL")]
    is_xbrl: Vec<u8>,
    #[serde(rename = "isInlineXBRL")]
    is_inline_xbrl: Vec<u8>,
    primary_document: Vec<String>,
    primary_doc_description: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OlderWire {
    #[serde(flatten)]
    rows: FilingArrays,
}

#[derive(Debug)]
struct CheckedRow {
    record: CompanyFiling,
    accepted_sort: Option<(i64, u32)>,
    signature: FilingSignature,
}

struct ParsedAcceptance {
    text: Option<NonEmptyText>,
    sort_key: Option<(i64, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilingSignature {
    accession_number: String,
    filing_date: String,
    report_date: String,
    acceptance_date_time: String,
    act: String,
    form: String,
    file_number: String,
    film_number: String,
    items: String,
    size: u64,
    is_xbrl: u8,
    is_inline_xbrl: u8,
    primary_document: String,
    primary_doc_description: String,
}

pub(crate) type FilingIdentity = (String, String);

struct ParsedRows {
    records: Vec<CompanyFiling>,
    record_index: HashMap<FilingIdentity, usize>,
    signatures: HashMap<FilingIdentity, FilingSignature>,
}

pub(crate) fn parse_parent(
    body: &[u8],
    requested: &SecCompanyIdentity,
    request: &CompanyFilingRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<ParsedCompany, SecEdgarError> {
    let wire: ParentWire =
        serde_json::from_slice(body).map_err(|error| SecEdgarError::Decode(error.to_string()))?;
    let response_company = SecCompanyIdentity::new(wire.cik.into_string(), None::<String>)?;
    if response_company.cik() != requested.cik() {
        return Err(SecEdgarError::Protocol(
            "response CIK does not match the requested normalized CIK".into(),
        ));
    }
    validate_company_name(&wire.name)?;
    let company = checked_response_company(requested, &wire.tickers)?;
    let recent_dates = wire.filings.recent.checked_filing_dates()?;
    let recent_decoded_count = wire.filings.recent.len();
    let rows = parse_arrays(
        wire.filings.recent,
        &company,
        &wire.name,
        request,
        observed_at,
        batch_id,
        MAX_RECENT_FILINGS,
    )?;
    let older_files = parse_older_descriptors(wire.filings.files, requested.cik())?;
    validate_recent_catalog_boundary(&recent_dates, &older_files)?;
    Ok(ParsedCompany {
        company,
        company_name: wire.name,
        records: rows.records,
        record_index: rows.record_index,
        signatures: rows.signatures,
        recent_decoded_count,
        older_files,
    })
}

pub(crate) fn parse_older(
    body: &[u8],
    company: &SecCompanyIdentity,
    company_name: &str,
    request: &CompanyFilingRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<ParsedOlder, SecEdgarError> {
    let wire: OlderWire =
        serde_json::from_slice(body).map_err(|error| SecEdgarError::Decode(error.to_string()))?;
    let decoded_count = wire.rows.len();
    if decoded_count == 0 {
        return Err(SecEdgarError::Protocol(
            "older submissions file must contain at least one filing".into(),
        ));
    }
    let dates = wire.rows.checked_filing_dates()?;
    let min_date =
        dates.iter().min().cloned().ok_or_else(|| {
            SecEdgarError::Protocol("older filing minimum date is missing".into())
        })?;
    let max_date =
        dates.iter().max().cloned().ok_or_else(|| {
            SecEdgarError::Protocol("older filing maximum date is missing".into())
        })?;
    let rows = parse_arrays(
        wire.rows,
        company,
        company_name,
        request,
        observed_at,
        batch_id,
        MAX_DECODED_FILINGS,
    )?;
    Ok(ParsedOlder {
        records: rows.records,
        signatures: rows.signatures,
        decoded_count,
        min_date,
        max_date,
    })
}

fn parse_arrays(
    rows: FilingArrays,
    company: &SecCompanyIdentity,
    company_name: &str,
    request: &CompanyFilingRequest,
    observed_at: &str,
    batch_id: &str,
    limit: usize,
) -> Result<ParsedRows, SecEdgarError> {
    let len = rows.validate_lengths(limit)?;
    let mut checked = Vec::with_capacity(len);
    for index in 0..len {
        let accession = SecAccessionNumber::new(rows.accession_number[index].clone())?;
        if &accession.as_str()[..10] != company.cik() {
            return Err(SecEdgarError::Protocol(
                "filing accession CIK prefix contradicts the response CIK".into(),
            ));
        }
        let filing_date = IsoDate::new(rows.filing_date[index].clone())?;
        let report_period = match rows.report_date[index].trim() {
            "" => None,
            value => Some(IsoDate::new(value)?),
        };
        let acceptance = parse_acceptance(rows.acceptance_date_time[index].trim())?;
        let primary_document = SecPrimaryDocument::new(rows.primary_document[index].clone())?;
        let (filing_index_url, primary_document_url) =
            canonical_urls(company.cik(), &accession, &primary_document)?;
        let mut evidence = SourceEvidence::new(ProviderId::SecEdgar, observed_at, batch_id)?;
        if let Some(source_at) = acceptance.text.as_ref() {
            evidence = evidence.with_source_at(source_at.as_str())?;
        }
        let record = CompanyFiling::new(
            company.clone(),
            company_name,
            rows.form[index].clone(),
            filing_date,
            report_period,
            accession,
            primary_document,
            filing_index_url,
            primary_document_url,
            acceptance.text,
            evidence,
        )?;
        checked.push(CheckedRow {
            record,
            accepted_sort: acceptance.sort_key,
            signature: rows.signature_at(index),
        });
    }

    checked.sort_by(|left, right| {
        right
            .record
            .filing_date()
            .cmp(left.record.filing_date())
            .then_with(|| right.accepted_sort.cmp(&left.accepted_sort))
            .then_with(|| {
                left.record
                    .accession()
                    .as_str()
                    .cmp(right.record.accession().as_str())
            })
    });
    let mut unique: HashMap<FilingIdentity, (CompanyFiling, FilingSignature)> = HashMap::new();
    let mut ordered = Vec::with_capacity(checked.len());
    for row in checked {
        let identity = (
            row.record.company().cik().to_owned(),
            row.record.accession().as_str().to_owned(),
        );
        if let Some((previous, signature)) = unique.get(&identity) {
            if previous != &row.record || signature != &row.signature {
                return Err(SecEdgarError::Protocol(
                    "conflicting duplicate filing identity".into(),
                ));
            }
            continue;
        }
        unique.insert(identity, (row.record.clone(), row.signature));
        ordered.push(row.record);
    }

    let records: Vec<CompanyFiling> = ordered
        .into_iter()
        .filter(|record| matches_filters(record, request))
        .collect();
    let record_index = records
        .iter()
        .enumerate()
        .map(|(index, record)| (filing_identity(record), index))
        .collect();
    let signatures = unique
        .into_iter()
        .map(|(identity, (_, signature))| (identity, signature))
        .collect();
    Ok(ParsedRows {
        records,
        record_index,
        signatures,
    })
}

impl FilingArrays {
    fn len(&self) -> usize {
        self.accession_number.len()
    }

    fn validate_lengths(&self, limit: usize) -> Result<usize, SecEdgarError> {
        let expected = self.len();
        let lengths = [
            self.filing_date.len(),
            self.report_date.len(),
            self.acceptance_date_time.len(),
            self.act.len(),
            self.form.len(),
            self.file_number.len(),
            self.film_number.len(),
            self.items.len(),
            self.size.len(),
            self.is_xbrl.len(),
            self.is_inline_xbrl.len(),
            self.primary_document.len(),
            self.primary_doc_description.len(),
        ];
        if expected > limit || lengths.into_iter().any(|length| length != expected) {
            return Err(SecEdgarError::Protocol(
                "SEC filing parallel arrays have contradictory lengths or exceed limits".into(),
            ));
        }
        Ok(expected)
    }

    fn checked_filing_dates(&self) -> Result<Vec<IsoDate>, SecEdgarError> {
        self.validate_lengths(MAX_DECODED_FILINGS)?;
        self.filing_date
            .iter()
            .map(|date| IsoDate::new(date.clone()).map_err(SecEdgarError::from))
            .collect()
    }

    fn signature_at(&self, index: usize) -> FilingSignature {
        FilingSignature {
            accession_number: self.accession_number[index].clone(),
            filing_date: self.filing_date[index].clone(),
            report_date: self.report_date[index].clone(),
            acceptance_date_time: self.acceptance_date_time[index].clone(),
            act: self.act[index].clone(),
            form: self.form[index].clone(),
            file_number: self.file_number[index].clone(),
            film_number: self.film_number[index].clone(),
            items: self.items[index].clone(),
            size: self.size[index],
            is_xbrl: self.is_xbrl[index],
            is_inline_xbrl: self.is_inline_xbrl[index],
            primary_document: self.primary_document[index].clone(),
            primary_doc_description: self.primary_doc_description[index].clone(),
        }
    }
}

fn checked_response_company(
    requested: &SecCompanyIdentity,
    response_tickers: &[String],
) -> Result<SecCompanyIdentity, SecEdgarError> {
    let ticker = match requested.ticker() {
        Some(requested_ticker) => response_tickers
            .iter()
            .find(|ticker| ticker.eq_ignore_ascii_case(requested_ticker))
            .cloned()
            .ok_or_else(|| {
                SecEdgarError::Protocol(
                    "requested ticker is not present in the SEC company response".into(),
                )
            })?,
        None => match response_tickers.first() {
            Some(ticker) => ticker.clone(),
            None => return Ok(SecCompanyIdentity::new(requested.cik(), None::<String>)?),
        },
    };
    Ok(SecCompanyIdentity::new(requested.cik(), Some(ticker))?)
}

fn validate_company_name(name: &str) -> Result<(), SecEdgarError> {
    if name.trim().is_empty()
        || name.chars().count() > MAX_COMPANY_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(SecEdgarError::Protocol(
            "SEC company name is empty, unsafe, or exceeds the limit".into(),
        ));
    }
    Ok(())
}

fn parse_acceptance(value: &str) -> Result<ParsedAcceptance, SecEdgarError> {
    if value.is_empty() {
        return Ok(ParsedAcceptance {
            text: None,
            sort_key: None,
        });
    }
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| SecEdgarError::Protocol("invalid SEC acceptance timestamp".into()))?
        .to_offset(UtcOffset::UTC);
    let normalized = parsed.format(&Rfc3339).map_err(|_| {
        SecEdgarError::Protocol("SEC acceptance timestamp formatting failed".into())
    })?;
    Ok(ParsedAcceptance {
        text: Some(NonEmptyText::new(normalized)?),
        sort_key: Some((parsed.unix_timestamp(), parsed.nanosecond())),
    })
}

fn canonical_urls(
    cik: &str,
    accession: &SecAccessionNumber,
    primary_document: &SecPrimaryDocument,
) -> Result<(HttpsUrl, HttpsUrl), SecEdgarError> {
    let archive_cik = cik.trim_start_matches('0');
    let archive_cik = if archive_cik.is_empty() {
        "0"
    } else {
        archive_cik
    };
    let compact = accession.without_hyphens();
    let base = format!("https://www.sec.gov/Archives/edgar/data/{archive_cik}/{compact}/");
    let index = format!("{base}{}-index.html", accession.as_str());
    let primary = format!("{base}{}", primary_document.as_str());
    validate_canonical_url(&index, &base)?;
    validate_canonical_url(&primary, &base)?;
    Ok((HttpsUrl::new(index)?, HttpsUrl::new(primary)?))
}

fn validate_canonical_url(url: &str, expected_base: &str) -> Result<(), SecEdgarError> {
    if !url.starts_with(expected_base)
        || !expected_base.starts_with("https://www.sec.gov/Archives/edgar/data/")
        || url.contains('?')
        || url.contains('#')
        || url["https://".len()..].contains('@')
        || url.chars().any(char::is_control)
    {
        return Err(SecEdgarError::Protocol(
            "generated SEC archive URL violates the exact host/path contract".into(),
        ));
    }
    Ok(())
}

fn matches_filters(record: &CompanyFiling, request: &CompanyFilingRequest) -> bool {
    let form_matches = request.forms().is_empty()
        || request
            .forms()
            .iter()
            .any(|form| form.as_str() == record.form());
    let date_matches = match (request.start(), request.end()) {
        (Some(start), Some(end)) => record.filing_date() >= start && record.filing_date() <= end,
        _ => true,
    };
    form_matches && date_matches
}

fn parse_older_descriptors(
    files: Vec<OlderFileWire>,
    cik: &str,
) -> Result<Vec<OlderFileDescriptor>, SecEdgarError> {
    if files.len() > MAX_OLDER_FILES {
        return Err(SecEdgarError::Protocol(
            "SEC response references too many older submission files".into(),
        ));
    }
    let mut seen_names = std::collections::HashSet::with_capacity(files.len());
    let mut descriptors: Vec<OlderFileDescriptor> = files
        .into_iter()
        .map(|file| {
            validate_older_filename(&file.name, cik)?;
            if !seen_names.insert(file.name.clone()) {
                return Err(SecEdgarError::Protocol(
                    "SEC response repeats an older submissions filename".into(),
                ));
            }
            if file.filing_count > MAX_DECODED_FILINGS {
                return Err(SecEdgarError::Protocol(
                    "older submissions descriptor exceeds the filing limit".into(),
                ));
            }
            let filing_from = IsoDate::new(file.filing_from)?;
            let filing_to = IsoDate::new(file.filing_to)?;
            if filing_from > filing_to {
                return Err(SecEdgarError::Protocol(
                    "older submissions descriptor has a reversed date range".into(),
                ));
            }
            Ok(OlderFileDescriptor {
                name: file.name,
                filing_count: file.filing_count,
                filing_from,
                filing_to,
            })
        })
        .collect::<Result<_, _>>()?;
    descriptors.sort_by(|left, right| {
        right
            .filing_to
            .cmp(&left.filing_to)
            .then_with(|| right.filing_from.cmp(&left.filing_from))
            .then_with(|| left.name.cmp(&right.name))
    });
    if descriptors.windows(2).any(|pair| {
        let newer = &pair[0];
        let older = &pair[1];
        newer.filing_from <= older.filing_to
    }) {
        return Err(SecEdgarError::Protocol(
            "older submissions catalog ranges overlap or are ambiguous".into(),
        ));
    }
    Ok(descriptors)
}

fn validate_recent_catalog_boundary(
    recent_dates: &[IsoDate],
    descriptors: &[OlderFileDescriptor],
) -> Result<(), SecEdgarError> {
    let Some(oldest_recent) = recent_dates.iter().min() else {
        return Ok(());
    };
    if descriptors
        .iter()
        .any(|descriptor| descriptor.filing_to >= *oldest_recent)
    {
        return Err(SecEdgarError::Protocol(
            "older submissions catalog is not strictly older than recent filings".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_older_filename(name: &str, cik: &str) -> Result<(), SecEdgarError> {
    let expected_prefix = format!("CIK{cik}-submissions-");
    let Some(suffix) = name.strip_prefix(&expected_prefix) else {
        return Err(SecEdgarError::Protocol(
            "older submissions filename has the wrong CIK or prefix".into(),
        ));
    };
    let Some(sequence) = suffix.strip_suffix(".json") else {
        return Err(SecEdgarError::Protocol(
            "older submissions filename has the wrong suffix".into(),
        ));
    };
    if sequence.len() != 3 || !sequence.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SecEdgarError::Protocol(
            "older submissions filename must end in a three-digit sequence".into(),
        ));
    }
    Ok(())
}

pub(crate) fn merge_records(
    destination: &mut Vec<CompanyFiling>,
    destination_index: &mut HashMap<FilingIdentity, usize>,
    destination_signatures: &mut HashMap<FilingIdentity, FilingSignature>,
    incoming: Vec<CompanyFiling>,
    incoming_signatures: HashMap<FilingIdentity, FilingSignature>,
) -> Result<(), SecEdgarError> {
    for (identity, signature) in incoming_signatures {
        if let Some(previous) = destination_signatures.get(&identity) {
            if previous != &signature {
                return Err(SecEdgarError::Protocol(
                    "recent and older files contain a conflicting filing signature".into(),
                ));
            }
        } else {
            destination_signatures.insert(identity, signature);
        }
    }
    for record in incoming {
        let identity = filing_identity(&record);
        if let Some(index) = destination_index.get(&identity) {
            let previous = destination.get(*index).ok_or_else(|| {
                SecEdgarError::Protocol("filing identity index is inconsistent".into())
            })?;
            if previous != &record {
                return Err(SecEdgarError::Protocol(
                    "recent and older files contain a conflicting filing identity".into(),
                ));
            }
        } else {
            destination_index.insert(identity, destination.len());
            destination.push(record);
        }
    }
    Ok(())
}

pub(crate) fn sort_records(destination: &mut [CompanyFiling]) {
    destination.sort_by(|left, right| {
        right
            .filing_date()
            .cmp(left.filing_date())
            .then_with(|| right.accepted_at().cmp(&left.accepted_at()))
            .then_with(|| left.accession().as_str().cmp(right.accession().as_str()))
    });
}

fn filing_identity(record: &CompanyFiling) -> FilingIdentity {
    (
        record.company().cik().to_owned(),
        record.accession().as_str().to_owned(),
    )
}
