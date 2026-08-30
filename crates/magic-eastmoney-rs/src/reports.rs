use crate::mapping::{
    finite, non_empty, optional_f64, optional_string, required_string, validate_date_or_datetime,
};
use crate::{
    query_url, validate_instrument, validate_source_instrument, BatchContext, EastmoneyClient,
    EastmoneyError,
};
use magic_market_core::{
    EarningsEstimate, Exchange, HttpsUrl, NonEmptyText, PositiveU32, ReportScope, ResearchDocument,
    ResearchDocumentRequest, ResearchDocuments, ResearchReport, ResearchReports, ResearchRequest,
    TargetPriceConsensus, TargetPriceData, TargetPriceObservation, TargetPriceRequest,
    VerifiedEmpty,
};
use serde_json::Value;

const REPORT_ENDPOINT: &str = "https://reportapi.eastmoney.com/report/list";
const TARGET_PRICE_PAGE_SIZE: u32 = 100;
const MAX_TARGET_PRICE_PAGES: u32 = 100;

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

impl ResearchDocuments for EastmoneyClient {
    type Error = EastmoneyError;

    fn research_document(
        &self,
        request: &ResearchDocumentRequest,
    ) -> Result<magic_market_core::DataBatch<ResearchDocument>, Self::Error> {
        let report_id = request.report_id.as_str();
        if !report_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(EastmoneyError::InvalidRequest(
                "report ID contains URL-unsafe characters".into(),
            ));
        }
        let expected = format!("https://pdf.dfcfw.com/pdf/H3_{report_id}_1.pdf");
        if request.pdf_url.as_str() != expected {
            return Err(EastmoneyError::InvalidRequest(format!(
                "report PDF URL must exactly match {expected}"
            )));
        }
        let body = self.get_pdf(
            request.pdf_url.as_str(),
            &[
                ("Accept", "application/pdf"),
                ("Referer", "https://data.eastmoney.com/"),
            ],
        )?;
        let context = BatchContext::new("research-document", None)?;
        let document = ResearchDocument::new(
            request.report_id.clone(),
            request.pdf_url.clone(),
            body,
            context.evidence()?,
        )?;
        context.finish(vec![document])
    }
}

impl TargetPriceData for EastmoneyClient {
    type Error = EastmoneyError;

    fn target_price_consensus(
        &self,
        request: &TargetPriceRequest,
    ) -> Result<magic_market_core::DataBatch<TargetPriceConsensus>, Self::Error> {
        validate_instrument(request.instrument())?;
        let mut pages = Vec::new();
        let mut page = 1_u32;
        let mut total_pages = None;
        loop {
            let url = target_price_url(request, page);
            let bytes = self.get(
                &url,
                &[
                    ("Accept", "application/json"),
                    ("Referer", "https://data.eastmoney.com/"),
                ],
            )?;
            let (declared_pages, hits, row_count) = target_page_metadata(&bytes)?;
            if declared_pages == 0 && hits == 0 && row_count == 0 {
                if page != 1 || !pages.is_empty() {
                    return Err(EastmoneyError::Protocol(
                        "target-price pagination changed from non-empty to exact zero".into(),
                    ));
                }
                let context = BatchContext::new("target-price", None)?;
                let evidence = context.evidence()?;
                let batch = context.finish_allow_empty::<TargetPriceConsensus>(Vec::new())?;
                let request_identity = format!(
                    "{:?}:{}:{}..{}",
                    request.instrument().exchange(),
                    request.instrument().code(),
                    request.from(),
                    request.through()
                );
                let empty = VerifiedEmpty::new(
                    "target_price_consensus",
                    request_identity,
                    "source returned hits=0,size=0,TotalPage=0,data=[] for the exact request",
                    evidence,
                    batch.provenance().clone(),
                )
                .map_err(|error| {
                    EastmoneyError::Protocol(format!(
                        "target-price verified-empty evidence is invalid: {error}"
                    ))
                })?;
                return Err(EastmoneyError::VerifiedEmpty(Box::new(empty)));
            }
            if declared_pages == 0
                || hits == 0
                || row_count == 0
                || declared_pages > MAX_TARGET_PRICE_PAGES
            {
                return Err(EastmoneyError::Protocol(format!(
                    "target-price pagination shape TotalPage={declared_pages}, hits={hits}, rows={row_count} is contradictory"
                )));
            }
            match total_pages {
                Some(expected) if expected != declared_pages => {
                    return Err(EastmoneyError::Protocol(format!(
                        "target-price TotalPage changed from {expected} to {declared_pages}"
                    )))
                }
                None => total_pages = Some(declared_pages),
                _ => {}
            }
            pages.push(bytes);
            if page == declared_pages {
                break;
            }
            page = page
                .checked_add(1)
                .ok_or_else(|| EastmoneyError::Protocol("target-price page overflow".into()))?;
        }
        parse_target_price_pages(&pages, request, TARGET_PRICE_PAGE_SIZE)
    }
}

fn target_price_url(request: &TargetPriceRequest, page: u32) -> String {
    query_url(
        REPORT_ENDPOINT,
        &[
            ("industryCode", "*".into()),
            ("pageSize", TARGET_PRICE_PAGE_SIZE.to_string()),
            ("industry", "*".into()),
            ("rating", "*".into()),
            ("ratingChange", "*".into()),
            ("beginTime", request.from().as_str().into()),
            ("endTime", request.through().as_str().into()),
            ("pageNo", page.to_string()),
            ("fields", String::new()),
            ("qType", "0".into()),
            ("orgCode", String::new()),
            ("code", request.instrument().code().into()),
            ("rcode", String::new()),
        ],
    )
}

fn target_page_metadata(bytes: &[u8]) -> Result<(u32, u32, usize), EastmoneyError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    let total_pages = crate::mapping::optional_u32(root.get("TotalPage"))?
        .ok_or_else(|| EastmoneyError::Protocol("target-price TotalPage is absent".into()))?;
    let hits = crate::mapping::optional_u32(root.get("hits"))?
        .ok_or_else(|| EastmoneyError::Protocol("target-price hits is absent".into()))?;
    let size = crate::mapping::optional_u32(root.get("size"))?
        .ok_or_else(|| EastmoneyError::Protocol("target-price size is absent".into()))?;
    let rows = root
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| EastmoneyError::Protocol("target-price data is not an array".into()))?;
    if size as usize != rows.len() {
        return Err(EastmoneyError::Protocol(format!(
            "target-price size {size} contradicts {} response rows",
            rows.len()
        )));
    }
    Ok((total_pages, hits, rows.len()))
}

fn parse_target_price_pages(
    pages: &[Vec<u8>],
    request: &TargetPriceRequest,
    page_size: u32,
) -> Result<magic_market_core::DataBatch<TargetPriceConsensus>, EastmoneyError> {
    if pages.is_empty() || page_size == 0 {
        return Err(EastmoneyError::InvalidRequest(
            "target-price aggregation requires bounded source pages".into(),
        ));
    }
    let expected_pages = u32::try_from(pages.len())
        .map_err(|_| EastmoneyError::Protocol("target-price page count overflow".into()))?;
    let mut expected_hits = None;
    let mut rows = Vec::<Value>::new();
    for (index, bytes) in pages.iter().enumerate() {
        let root: Value = serde_json::from_slice(bytes)
            .map_err(|error| EastmoneyError::Decode(error.to_string()))?;
        let declared_pages = crate::mapping::optional_u32(root.get("TotalPage"))?
            .ok_or_else(|| EastmoneyError::Protocol("target-price TotalPage is absent".into()))?;
        let hits = crate::mapping::optional_u32(root.get("hits"))?
            .ok_or_else(|| EastmoneyError::Protocol("target-price hits is absent".into()))?;
        if declared_pages != expected_pages {
            return Err(EastmoneyError::Protocol(format!(
                "target-price page {} declares TotalPage {declared_pages}, fetched {expected_pages}",
                index + 1
            )));
        }
        match expected_hits {
            Some(expected) if expected != hits => {
                return Err(EastmoneyError::Protocol(format!(
                    "target-price hits changed from {expected} to {hits} on page {}",
                    index + 1
                )))
            }
            None => expected_hits = Some(hits),
            _ => {}
        }
        let page_rows = root
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| EastmoneyError::Protocol("target-price data is not an array".into()))?;
        let size = crate::mapping::optional_u32(root.get("size"))?
            .ok_or_else(|| EastmoneyError::Protocol("target-price size is absent".into()))?;
        if size as usize != page_rows.len() {
            return Err(EastmoneyError::Protocol(format!(
                "target-price page {} size {size} contradicts {} rows",
                index + 1,
                page_rows.len()
            )));
        }
        if index + 1 < pages.len() && page_rows.len() != page_size as usize {
            return Err(EastmoneyError::Protocol(format!(
                "target-price page {} returned {} rows before final page; expected {page_size}",
                index + 1,
                page_rows.len()
            )));
        }
        if index + 1 == pages.len() && page_rows.is_empty() {
            return Err(EastmoneyError::Protocol(
                "target-price final page is empty".into(),
            ));
        }
        rows.extend(page_rows.iter().cloned());
    }
    let hits = expected_hits
        .ok_or_else(|| EastmoneyError::Protocol("target-price hits is absent".into()))?;
    if hits == 0 || rows.len() != hits as usize {
        return Err(EastmoneyError::Protocol(format!(
            "target-price fetched {} rows for source hits {hits}",
            rows.len()
        )));
    }
    let calculated_pages = hits.div_ceil(page_size);
    if calculated_pages != expected_pages {
        return Err(EastmoneyError::Protocol(format!(
            "target-price hits {hits} and page size {page_size} imply {calculated_pages} pages, source declared {expected_pages}"
        )));
    }

    let latest_source_at = rows
        .iter()
        .filter(|row| {
            optional_target_price(row, "indvAimPriceT")
                .ok()
                .flatten()
                .is_some()
                && optional_target_price(row, "indvAimPriceL")
                    .ok()
                    .flatten()
                    .is_some()
        })
        .filter_map(|row| optional_string(row.get("publishDate")).ok().flatten())
        .max()
        .ok_or_else(|| EastmoneyError::Protocol("target-price source date is absent".into()))?;
    let context = BatchContext::new("target-price", Some(&latest_source_at))?;
    let mut observations = Vec::new();
    for row in &rows {
        let source_t = optional_target_price(row, "indvAimPriceT")?;
        let source_l = optional_target_price(row, "indvAimPriceL")?;
        let (source_t, source_l) = match (source_t, source_l) {
            (Some(source_t), Some(source_l)) => (source_t, source_l),
            (None, None) => continue,
            _ => {
                return Err(EastmoneyError::Protocol(
                    "target-price report contains only one of indvAimPriceT/L".into(),
                ))
            }
        };
        let source_code = required_string(row, "stockCode")?;
        let instrument_name = NonEmptyText::new(required_string(row, "stockName")?)?;
        let source_market = required_string(row, "market")?;
        let source_exchange = report_exchange(&source_market)?;
        validate_source_instrument(request.instrument(), &source_code, Some(source_exchange))?;
        let published_at = required_string(row, "publishDate")?;
        validate_date_or_datetime(&published_at, "target-price publishDate")?;
        let published_on = crate::mapping::iso_date(&published_at)?;
        if &published_on < request.from() || &published_on > request.through() {
            return Err(EastmoneyError::Protocol(format!(
                "target-price report date {published_on} is outside requested range"
            )));
        }
        let report_id = NonEmptyText::new(required_string(row, "infoCode")?)?;
        let institution_id = NonEmptyText::new(required_string(row, "orgCode")?)?;
        let institution_name = optional_string(row.get("orgSName"))?
            .or(optional_string(row.get("orgName"))?)
            .ok_or_else(|| {
                EastmoneyError::Protocol("target-price institution name is absent".into())
            })?;
        observations.push(TargetPriceObservation::new(
            request.instrument().clone(),
            instrument_name,
            report_id,
            institution_id,
            NonEmptyText::new(institution_name)?,
            published_on,
            source_t,
            source_l,
            context.evidence_at(Some(&published_at))?,
        )?);
    }
    if observations.is_empty() {
        return Err(EastmoneyError::Protocol(
            "complete report pagination contains no reports with both indvAimPriceT/L".into(),
        ));
    }
    let aggregate = TargetPriceConsensus::new(request, observations, context.evidence()?)?;
    context.finish(vec![aggregate])
}

fn optional_target_price(
    row: &Value,
    field: &'static str,
) -> Result<Option<magic_market_core::Price>, EastmoneyError> {
    optional_f64(row.get(field))?
        .map(magic_market_core::Price::new)
        .transpose()
        .map_err(Into::into)
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
    if rows.is_empty() {
        // 空 data 数组 = 源对该精确请求确认无报告 (业务态), 非响应缺陷。
        // 与 target-price 先例对齐: finish_allow_empty + VerifiedEmpty, 服务器
        // 据此分类为业务态而非 invalid_evidence (2026-08-30: 605178/300128
        // 空响应曾被误判 Protocol("no usable records") → invalid_evidence)。
        let context = BatchContext::new("research-reports", None)?;
        let evidence = context.evidence()?;
        let batch = context.finish_allow_empty::<ResearchReport>(Vec::new())?;
        let request_identity = match requested_scope {
            ReportScope::Instrument(instrument) => format!(
                "{:?}:{}:research-report",
                instrument.exchange(),
                instrument.code()
            ),
            ReportScope::Industry(industry) => format!("industry:{}", industry.as_str()),
        };
        let empty = VerifiedEmpty::new(
            "research_reports",
            request_identity,
            "source returned data=[] for the exact request",
            evidence,
            batch.provenance().clone(),
        )
        .map_err(|error| {
            EastmoneyError::Protocol(format!(
                "research-reports verified-empty evidence is invalid: {error}"
            ))
        })?;
        return Err(EastmoneyError::VerifiedEmpty(Box::new(empty)));
    }
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
        organization_id: non_empty(optional_string(row.get("orgCode"))?)?,
        author: non_empty(author)?,
        rating: non_empty(optional_string(row.get("emRatingName"))?)?,
        industry_code: non_empty(industry_code)?,
        industry_name: non_empty(industry_name)?,
        published_at: NonEmptyText::new(published_at.clone())?,
        canonical_url: HttpsUrl::new(pdf.clone())?,
        pdf_url: Some(HttpsUrl::new(pdf)?),
        estimates,
        source_indv_aim_price_t: optional_target_price(row, "indvAimPriceT")?,
        source_indv_aim_price_l: optional_target_price(row, "indvAimPriceL")?,
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

#[cfg(test)]
#[path = "../tests/internal/research_document_regression_tests.rs"]
mod research_document_regression_tests;
