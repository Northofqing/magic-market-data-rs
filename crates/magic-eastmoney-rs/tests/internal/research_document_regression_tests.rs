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
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
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
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
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
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
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
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
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
        Ok(b"%PDF-1.7\nfixture\nstartxref\n9\n%%EOF\n".to_vec())
    }
}

#[test]
fn downloads_original_pdf_body_for_the_exact_report_identity() {
    let client = EastmoneyClient::with_transport(PdfFixture);
    let request = ResearchDocumentRequest {
        report_id: NonEmptyText::new("AP202607231827290069").unwrap(),
        pdf_url: HttpsUrl::new("https://pdf.dfcfw.com/pdf/H3_AP202607231827290069_1.pdf").unwrap(),
    };
    let batch = client.research_document(&request).unwrap();
    assert_eq!(
        batch.records()[0].body,
        b"%PDF-1.7\nfixture\nstartxref\n9\n%%EOF\n"
    );
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
