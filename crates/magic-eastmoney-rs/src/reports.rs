use crate::mapping::{
    finite, non_empty, optional_f64, optional_string, required_string, validate_date_or_datetime,
};
use crate::{
    query_url, validate_instrument, validate_source_instrument, BatchContext, EastmoneyClient,
    EastmoneyError,
};
use magic_market_core::{
    EarningsEstimate, Exchange, HttpsUrl, NonEmptyText, PositiveU32, ReportScope, ResearchReport,
    ResearchReports, ResearchRequest,
};
use serde_json::Value;

const REPORT_ENDPOINT: &str = "https://reportapi.eastmoney.com/report/list";

impl ResearchReports for EastmoneyClient {
    type Error = EastmoneyError;

    fn research_reports(
        &self,
        request: &ResearchRequest,
    ) -> Result<magic_market_core::DataBatch<ResearchReport>, Self::Error> {
        let (query_type, code, industry_code) = match request.scope() {
            ReportScope::Instrument(instrument) => {
                validate_instrument(instrument)?;
                ("0", instrument.code().to_owned(), "*".to_owned())
            }
            ReportScope::Industry(industry) => ("1", "*".to_owned(), industry.as_str().to_owned()),
        };
        let url = query_url(
            REPORT_ENDPOINT,
            &[
                ("industryCode", industry_code),
                ("pageSize", request.page_size().get().to_string()),
                ("industry", "*".into()),
                ("rating", "*".into()),
                ("ratingChange", "*".into()),
                ("beginTime", "2000-01-01".into()),
                ("endTime", "2099-12-31".into()),
                ("pageNo", request.page().get().to_string()),
                ("fields", String::new()),
                ("qType", query_type.into()),
                ("orgCode", String::new()),
                ("code", code),
                ("rcode", String::new()),
            ],
        );
        let bytes = self.get(
            &url,
            &[
                ("Accept", "application/json"),
                ("Referer", "https://data.eastmoney.com/"),
            ],
        )?;
        parse_reports(&bytes, request.scope())
    }
}

fn parse_reports(
    bytes: &[u8],
    requested_scope: &ReportScope,
) -> Result<magic_market_core::DataBatch<ResearchReport>, EastmoneyError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    let rows = root
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| EastmoneyError::Protocol("report response data is not an array".into()))?;
    let source_at = rows
        .iter()
        .filter_map(|row| optional_string(row.get("publishDate")).ok().flatten())
        .max();
    let context = BatchContext::new("research-reports", source_at.as_deref())?;
    let records = rows
        .iter()
        .map(|row| map_report(row, requested_scope, &context))
        .collect::<Result<Vec<_>, _>>()?;
    context.finish(records)
}

fn map_report(
    row: &Value,
    requested_scope: &ReportScope,
    context: &BatchContext,
) -> Result<ResearchReport, EastmoneyError> {
    let report_id = required_string(row, "infoCode")?;
    if !report_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(EastmoneyError::Protocol(
            "report infoCode contains URL-unsafe characters".into(),
        ));
    }
    let published_at = required_string(row, "publishDate")?;
    validate_date_or_datetime(&published_at, "report publishDate")?;
    let organization = optional_string(row.get("orgSName"))?
        .or(optional_string(row.get("orgName"))?)
        .ok_or_else(|| EastmoneyError::Protocol("report organization is absent".into()))?;
    let author = optional_authors(row.get("researcher"))?.or(optional_authors(row.get("author"))?);
    let industry_code =
        optional_string(row.get("indvInduCode"))?.or(optional_string(row.get("industryCode"))?);
    let industry_name =
        optional_string(row.get("indvInduName"))?.or(optional_string(row.get("industryName"))?);
    let scope = match requested_scope {
        ReportScope::Instrument(instrument) => {
            // Instrument-report rows identify their source with the real
            // reportapi pair stockCode + market. The query's code filter is
            // not response evidence, so both fields are mandatory.
            let source_code = required_string(row, "stockCode")?;
            let source_market = required_string(row, "market")?;
            let source_exchange = report_exchange(&source_market)?;
            validate_source_instrument(instrument, &source_code, Some(source_exchange))?;
            ReportScope::Instrument(instrument.clone())
        }
        ReportScope::Industry(industry) => {
            let source_industry_code = industry_code.as_ref().ok_or_else(|| {
                EastmoneyError::Protocol("industry report source industryCode is absent".into())
            })?;
            if industry.as_str() != "*" && source_industry_code != industry.as_str() {
                return Err(EastmoneyError::Protocol(format!(
                    "Eastmoney report source industry {source_industry_code} does not match requested {}",
                    industry.as_str()
                )));
            }
            ReportScope::Industry(NonEmptyText::new(source_industry_code.clone())?)
        }
    };
    let pdf = format!("https://pdf.dfcfw.com/pdf/H3_{report_id}_1.pdf");
    let estimates = map_estimates(row, &published_at)?;
    Ok(ResearchReport {
        report_id: NonEmptyText::new(report_id)?,
        scope,
        title: NonEmptyText::new(required_string(row, "title")?)?,
        organization: NonEmptyText::new(organization)?,
        author: non_empty(author)?,
        rating: non_empty(optional_string(row.get("emRatingName"))?)?,
        industry_code: non_empty(industry_code)?,
        industry_name: non_empty(industry_name)?,
        published_at: NonEmptyText::new(published_at.clone())?,
        canonical_url: HttpsUrl::new(pdf.clone())?,
        pdf_url: Some(HttpsUrl::new(pdf)?),
        estimates,
        evidence: context.evidence_at(Some(&published_at))?,
    })
}

fn report_exchange(value: &str) -> Result<Exchange, EastmoneyError> {
    match value.to_ascii_uppercase().as_str() {
        "SHANGHAI" => Ok(Exchange::Shanghai),
        "SHENZHEN" => Ok(Exchange::Shenzhen),
        "BEIJING" => Ok(Exchange::Beijing),
        _ => Err(EastmoneyError::Protocol(format!(
            "unsupported Eastmoney report market {value:?}"
        ))),
    }
}

fn optional_authors(value: Option<&Value>) -> Result<Option<String>, EastmoneyError> {
    match value {
        Some(Value::Array(authors)) => {
            let authors = authors
                .iter()
                .map(|author| optional_string(Some(author)))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            if authors.is_empty() {
                Ok(None)
            } else {
                Ok(Some(authors.join(", ")))
            }
        }
        other => optional_string(other),
    }
}

fn map_estimates(row: &Value, published_at: &str) -> Result<Vec<EarningsEstimate>, EastmoneyError> {
    let year = published_at
        .get(..4)
        .ok_or_else(|| EastmoneyError::Protocol("report publishDate has no year".into()))?
        .parse::<u32>()
        .map_err(|error| EastmoneyError::Protocol(format!("invalid report year: {error}")))?;
    let fields = [
        "predictThisYearEps",
        "predictNextYearEps",
        "predictNextTwoYearEps",
    ];
    let mut estimates = Vec::new();
    for (offset, field) in fields.iter().enumerate() {
        let eps = optional_f64(row.get(*field))?;
        if eps.is_some() {
            estimates.push(EarningsEstimate::new(
                PositiveU32::new(year + offset as u32)?,
                finite(eps)?,
                None,
                None,
                None,
                None,
                None,
            )?);
        }
    }
    Ok(estimates)
}

#[cfg(test)]
#[path = "../tests/internal/reports_tests.rs"]
mod tests;
