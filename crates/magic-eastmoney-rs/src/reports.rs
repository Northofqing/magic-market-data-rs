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
mod tests {
    use super::parse_reports;
    use crate::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
    use magic_market_core::{
        AssetClass, Exchange, HttpsUrl, InstrumentId, NonEmptyText, ReportScope,
        ResearchDocumentRequest, ResearchDocuments,
    };

    #[test]
    fn maps_every_available_report_contract_field() {
        let fixture = r#"{
          "TotalPage":1,
          "data":[{
            "infoCode":"AP202607231714427688",
            "title":"电力行业跟踪",
            "publishDate":"2026-07-23 09:30:00",
            "orgSName":"中信",
            "researcher":null,
            "author":["研究员甲","研究员乙"],
            "emRatingName":"增持",
            "stockCode":"600396",
            "market":"SHANGHAI",
            "stockName":"华电辽能",
            "industryCode":"BK0428",
            "industryName":"电力行业",
            "predictThisYearEps":"0.42",
            "predictNextYearEps":0.53,
            "predictNextTwoYearEps":"-"
          }]
        }"#
        .as_bytes();
        let instrument =
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
        let batch = parse_reports(fixture, &ReportScope::Instrument(instrument.clone())).unwrap();
        let report = &batch.records()[0];
        assert_eq!(report.report_id.as_str(), "AP202607231714427688");
        assert_eq!(report.scope, ReportScope::Instrument(instrument));
        assert_eq!(report.title.as_str(), "电力行业跟踪");
        assert_eq!(report.organization.as_str(), "中信");
        assert_eq!(
            report.author.as_ref().unwrap().as_str(),
            "研究员甲, 研究员乙"
        );
        assert_eq!(report.rating.as_ref().unwrap().as_str(), "增持");
        assert_eq!(report.industry_code.as_ref().unwrap().as_str(), "BK0428");
        assert_eq!(report.industry_name.as_ref().unwrap().as_str(), "电力行业");
        assert_eq!(report.published_at.as_str(), "2026-07-23 09:30:00");
        assert_eq!(report.estimates.len(), 2);
        assert_eq!(report.estimates[0].fiscal_year().get(), 2026);
        assert_eq!(report.estimates[0].eps().unwrap().get(), 0.42);
        assert_eq!(report.estimates[1].fiscal_year().get(), 2027);
        assert!(report
            .canonical_url
            .as_str()
            .starts_with("https://pdf.dfcfw.com/"));
        assert_eq!(
            report.pdf_url.as_ref().unwrap().as_str(),
            report.canonical_url.as_str()
        );
        assert_eq!(report.evidence.source_at(), Some("2026-07-23 09:30:00"));
        assert_eq!(batch.provenance().source(), "eastmoney-web");
    }

    #[test]
    fn malformed_report_shapes_fail_explicitly() {
        let scope = ReportScope::Industry(magic_market_core::NonEmptyText::new("BK0428").unwrap());
        assert!(parse_reports(br#"{"data":{}}"#, &scope).is_err());
        assert!(parse_reports(br#"{"data":[{"title":"missing id"}]}"#, &scope).is_err());
    }

    #[test]
    fn instrument_report_source_code_must_match_the_request() {
        let instrument =
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
        let fixture = br#"{"data":[{
          "infoCode":"AP1",
          "title":"x",
          "publishDate":"2026-07-23",
          "orgName":"x",
          "stockCode":"002475",
          "market":"SHENZHEN"
        }]}"#;
        assert!(parse_reports(fixture, &ReportScope::Instrument(instrument)).is_err());
    }

    #[test]
    fn instrument_report_requires_matching_stock_code_and_real_market_pair() {
        let instrument =
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
        for fixture in [
            br#"{"data":[{
              "infoCode":"AP1","title":"x","publishDate":"2026-07-23",
              "orgName":"x","stockCode":"600396"
            }]}"#
                .as_slice(),
            br#"{"data":[{
              "infoCode":"AP1","title":"x","publishDate":"2026-07-23",
              "orgName":"x","market":"SHANGHAI"
            }]}"#
                .as_slice(),
            br#"{"data":[{
              "infoCode":"AP1","title":"x","publishDate":"2026-07-23",
              "orgName":"x","stockCode":"600396","market":"SHENZHEN"
            }]}"#
                .as_slice(),
            br#"{"data":[{
              "infoCode":"AP1","title":"x","publishDate":"2026-07-23",
              "orgName":"x","stockCode":"600396","market":"UNKNOWN"
            }]}"#
                .as_slice(),
        ] {
            assert!(parse_reports(fixture, &ReportScope::Instrument(instrument.clone())).is_err());
        }
    }

    #[test]
    fn industry_report_requires_and_matches_the_real_industry_code() {
        let requested = magic_market_core::NonEmptyText::new("481").unwrap();
        let valid = br#"{"data":[{
          "infoCode":"AP1","title":"x","publishDate":"2026-07-23 00:00:00.000",
          "orgName":"x","industryCode":"481"
        }]}"#;
        let batch = parse_reports(valid, &ReportScope::Industry(requested.clone())).unwrap();
        assert_eq!(
            batch.records()[0].scope,
            ReportScope::Industry(requested.clone())
        );
        for invalid in [
            br#"{"data":[{
              "infoCode":"AP1","title":"x","publishDate":"2026-07-23",
              "orgName":"x"
            }]}"#
                .as_slice(),
            br#"{"data":[{
              "infoCode":"AP1","title":"x","publishDate":"2026-07-23",
              "orgName":"x","industryCode":"482"
            }]}"#
                .as_slice(),
        ] {
            assert!(parse_reports(invalid, &ReportScope::Industry(requested.clone())).is_err());
        }
    }

    #[test]
    fn report_publish_date_must_be_a_real_date_and_time() {
        let instrument =
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
        for published_at in [
            "2026-02-30",
            "2026-07-23T09:30:00",
            "2026-07-23 24:00:00",
            "2026-07-23 09:60:00",
            "2026-07-23 09:30:60",
            "2026-07-23 09:30:00.bad",
        ] {
            let fixture = format!(
                r#"{{"data":[{{
                  "infoCode":"AP1","title":"x","publishDate":"{published_at}",
                  "orgName":"x","stockCode":"600396","market":"SHANGHAI"
                }}]}}"#
            );
            assert!(
                parse_reports(
                    fixture.as_bytes(),
                    &ReportScope::Instrument(instrument.clone())
                )
                .is_err(),
                "{published_at}"
            );
        }
    }

    struct PdfFixture;

    impl EastmoneyTransport for PdfFixture {
        fn get(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            Err(EastmoneyError::Transport("unexpected JSON request".into()))
        }

        fn post_json(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _body: &[u8],
            _max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            Err(EastmoneyError::Transport("unexpected POST request".into()))
        }

        fn get_pdf(
            &self,
            url: &str,
            _headers: &[(&str, &str)],
            max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            assert_eq!(
                url,
                "https://pdf.dfcfw.com/pdf/H3_AP202607231827290069_1.pdf"
            );
            assert_eq!(max_bytes, 32 * 1024 * 1024);
            Ok(b"%PDF-1.7\nfixture".to_vec())
        }
    }

    #[test]
    fn downloads_original_pdf_body_for_the_exact_report_identity() {
        let client = EastmoneyClient::with_transport(PdfFixture);
        let request = ResearchDocumentRequest {
            report_id: NonEmptyText::new("AP202607231827290069").unwrap(),
            pdf_url: HttpsUrl::new("https://pdf.dfcfw.com/pdf/H3_AP202607231827290069_1.pdf")
                .unwrap(),
        };
        let batch = client.research_document(&request).unwrap();
        assert_eq!(batch.records()[0].body, b"%PDF-1.7\nfixture");
        assert_eq!(batch.records()[0].content_type.as_str(), "application/pdf");
    }

    #[test]
    fn report_document_rejects_identity_url_disagreement() {
        let client = EastmoneyClient::with_transport(PdfFixture);
        let request = ResearchDocumentRequest {
            report_id: NonEmptyText::new("AP1").unwrap(),
            pdf_url: HttpsUrl::new("https://pdf.dfcfw.com/pdf/H3_AP2_1.pdf").unwrap(),
        };
        assert!(matches!(
            client.research_document(&request),
            Err(EastmoneyError::InvalidRequest(_))
        ));
    }
}
