use super::{parse_reports, parse_target_price_pages, target_page_metadata, target_price_url};
use crate::test_support::ScriptedTransport;
use crate::{EastmoneyClient, EastmoneyError};
use magic_market_core::{
    AssetClass, Exchange, HttpsUrl, InstrumentId, NonEmptyText, PositiveU32, ReportScope,
    ResearchDocumentRequest, ResearchDocuments, ResearchReports, ResearchRequest, TargetPriceData,
    TargetPriceRequest,
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
        "orgCode":"8001",
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
        ,"indvAimPriceT":25.5,
        "indvAimPriceL":23.0
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
    assert_eq!(report.organization_id.as_ref().unwrap().as_str(), "8001");
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
    assert_eq!(report.source_indv_aim_price_t.unwrap().get(), 25.5);
    assert_eq!(report.source_indv_aim_price_l.unwrap().get(), 23.0);
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

#[test]
fn public_research_contract_routes_instrument_and_industry_scopes() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
    let instrument_fixture = r#"{"data":[{
      "infoCode":"AP1","title":"x","publishDate":"2026-07-23",
      "orgName":"x","stockCode":"600396","market":"SHANGHAI"
    }]}"#;
    let industry_fixture = r#"{"data":[{
      "infoCode":"AP2","title":"y","publishDate":"2026-07-23",
      "orgName":"x","industryCode":"481"
    }]}"#;
    for (scope, fixture, query_marker) in [
        (
            ReportScope::Instrument(instrument),
            instrument_fixture,
            "qType=0",
        ),
        (
            ReportScope::Industry(NonEmptyText::new("481").unwrap()),
            industry_fixture,
            "qType=1",
        ),
    ] {
        let transport = ScriptedTransport::from_bodies([fixture.as_bytes()]);
        let requests = transport.requests();
        let client = EastmoneyClient::with_transport(transport);
        let request = ResearchRequest::new(
            scope,
            PositiveU32::new(2).unwrap(),
            PositiveU32::new(20).unwrap(),
        )
        .unwrap();
        let batch = client.research_reports(&request).unwrap();
        assert_eq!(batch.records().len(), 1);
        let source_request = requests.lock().unwrap()[0].clone();
        assert!(source_request.contains(query_marker), "{source_request}");
        assert!(source_request.contains("pageNo=2"), "{source_request}");
        assert!(source_request.contains("pageSize=20"), "{source_request}");
    }
}

#[test]
fn report_ids_and_json_decode_fail_before_url_construction() {
    let scope = ReportScope::Industry(NonEmptyText::new("*").unwrap());
    assert!(matches!(
        parse_reports(b"{", &scope),
        Err(EastmoneyError::Decode(_))
    ));
    assert!(matches!(
        parse_reports(
            br#"{"data":[{
              "infoCode":"unsafe/id","title":"x","publishDate":"2026-07-23",
              "orgName":"x","industryCode":"481"
            }]}"#,
            &scope
        ),
        Err(EastmoneyError::Protocol(_))
    ));
}

#[test]
fn target_price_pagination_requires_every_declared_page_and_a_full_nonfinal_page() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let request = TargetPriceRequest::new(
        instrument,
        magic_market_core::IsoDate::new("2026-01-01").unwrap(),
        magic_market_core::IsoDate::new("2026-07-27").unwrap(),
    )
    .unwrap();
    let row = |id: &str| {
        format!(
            r#"{{"infoCode":"{id}","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
              "orgCode":"O{id}","orgSName":"机构{id}","publishDate":"2026-07-20",
              "indvAimPriceT":1430,"indvAimPriceL":1430}}"#
        )
    };
    let page = |total: u32, rows: String| {
        let size = if rows.is_empty() { 0 } else { 1 };
        format!(r#"{{"hits":{total},"size":{size},"TotalPage":{total},"data":[{rows}]}}"#)
            .into_bytes()
    };

    assert!(parse_target_price_pages(&[page(2, row("1"))], &request, 1).is_err());
    assert!(
        parse_target_price_pages(&[page(2, row("1")), page(3, row("2"))], &request, 1).is_err()
    );
    assert!(
        parse_target_price_pages(&[page(2, row("1")), page(2, String::new())], &request, 1)
            .is_err()
    );

    let batch =
        parse_target_price_pages(&[page(2, row("1")), page(2, row("2"))], &request, 1).unwrap();
    assert_eq!(batch.records()[0].sample_count().get(), 2);
}

#[test]
fn target_price_url_and_page_metadata_preserve_the_exact_source_contract() {
    let request = TargetPriceRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        magic_market_core::IsoDate::new("2026-01-01").unwrap(),
        magic_market_core::IsoDate::new("2026-07-27").unwrap(),
    )
    .unwrap();
    let url = target_price_url(&request, 7);
    for marker in [
        "code=600519",
        "beginTime=2026-01-01",
        "endTime=2026-07-27",
        "pageNo=7",
        "pageSize=100",
        "qType=0",
    ] {
        assert!(url.contains(marker), "missing {marker} in {url}");
    }

    assert_eq!(
        target_page_metadata(br#"{"hits":2,"size":1,"TotalPage":2,"data":[{}]}"#).unwrap(),
        (2, 2, 1)
    );
    for fixture in [
        br#"{"#.as_slice(),
        br#"{"hits":1,"size":1,"data":[{}]}"#.as_slice(),
        br#"{"size":1,"TotalPage":1,"data":[{}]}"#.as_slice(),
        br#"{"hits":1,"TotalPage":1,"data":[{}]}"#.as_slice(),
        br#"{"hits":1,"size":1,"TotalPage":1}"#.as_slice(),
        br#"{"hits":"bad","size":1,"TotalPage":1,"data":[{}]}"#.as_slice(),
        br#"{"hits":1,"size":2,"TotalPage":1,"data":[{}]}"#.as_slice(),
    ] {
        assert!(target_page_metadata(fixture).is_err(), "{fixture:?}");
    }
}

#[test]
fn public_target_price_state_machine_fetches_complete_pages_and_rejects_page_drift() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let request = TargetPriceRequest::new(
        instrument,
        magic_market_core::IsoDate::new("2026-01-01").unwrap(),
        magic_market_core::IsoDate::new("2026-07-27").unwrap(),
    )
    .unwrap();
    let row = |index: u32| {
        format!(
            r#"{{"infoCode":"R{index}","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
                "orgCode":"O{index}","orgName":"机构{index}","publishDate":"2026-07-20",
                "indvAimPriceT":1430,"indvAimPriceL":1400}}"#
        )
    };
    let first_rows = (0..100).map(&row).collect::<Vec<_>>().join(",");
    let page = |declared_pages: u32, hits: u32, rows: &str| {
        let size = rows.matches("\"infoCode\"").count();
        format!(r#"{{"hits":{hits},"size":{size},"TotalPage":{declared_pages},"data":[{rows}]}}"#)
            .into_bytes()
    };

    let transport = ScriptedTransport::from_results([
        Ok(page(2, 101, &first_rows)),
        Ok(page(2, 101, &row(100))),
    ]);
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);
    let batch = client.target_price_consensus(&request).unwrap();
    assert_eq!(batch.records()[0].sample_count().get(), 101);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].contains("pageNo=1"));
    assert!(requests[1].contains("pageNo=2"));
    assert!(requests
        .iter()
        .all(|request| request.contains("pageSize=100")));
    drop(requests);

    let transport = ScriptedTransport::from_results([
        Ok(page(2, 101, &first_rows)),
        Ok(page(3, 101, &row(100))),
    ]);
    let client = EastmoneyClient::with_transport(transport);
    assert!(matches!(
        client.target_price_consensus(&request),
        Err(EastmoneyError::Protocol(message))
            if message.contains("TotalPage changed from 2 to 3")
    ));

    let client = EastmoneyClient::with_transport(ScriptedTransport::from_results([Ok(page(
        101,
        1,
        &row(0),
    ))]));
    assert!(matches!(
        client.target_price_consensus(&request),
        Err(EastmoneyError::Protocol(message))
            if message.contains("pagination shape")
    ));
}

#[test]
fn target_price_parser_rejects_missing_metadata_invalid_rows_and_range_violations() {
    let request = TargetPriceRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        magic_market_core::IsoDate::new("2026-01-01").unwrap(),
        magic_market_core::IsoDate::new("2026-07-27").unwrap(),
    )
    .unwrap();
    assert!(matches!(
        parse_target_price_pages(&[], &request, 100),
        Err(EastmoneyError::InvalidRequest(_))
    ));
    assert!(matches!(
        parse_target_price_pages(
            &[br#"{"hits":1,"size":1,"TotalPage":1,"data":[{}]}"#.to_vec()],
            &request,
            0,
        ),
        Err(EastmoneyError::InvalidRequest(_))
    ));

    for fixture in [
        br#"{"#.as_slice(),
        br#"{"hits":1,"size":1,"data":[{}]}"#.as_slice(),
        br#"{"size":1,"TotalPage":1,"data":[{}]}"#.as_slice(),
        br#"{"hits":1,"size":1,"TotalPage":1}"#.as_slice(),
        br#"{"hits":1,"TotalPage":1,"data":[{}]}"#.as_slice(),
        br#"{"hits":1,"size":2,"TotalPage":1,"data":[{}]}"#.as_slice(),
        r#"{"hits":1,"size":1,"TotalPage":1,"data":[{
          "infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
          "orgCode":"O1","orgName":"机构一","publishDate":"2026-07-20",
          "indvAimPriceT":null,"indvAimPriceL":null
        }]}"#
            .as_bytes(),
        r#"{"hits":1,"size":1,"TotalPage":1,"data":[{
          "infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"UNKNOWN",
          "orgCode":"O1","orgName":"机构一","publishDate":"2026-07-20",
          "indvAimPriceT":1430,"indvAimPriceL":1400
        }]}"#
            .as_bytes(),
        r#"{"hits":1,"size":1,"TotalPage":1,"data":[{
          "infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
          "orgCode":"O1","orgName":"机构一","publishDate":"2025-12-31",
          "indvAimPriceT":1430,"indvAimPriceL":1400
        }]}"#
            .as_bytes(),
        r#"{"hits":1,"size":1,"TotalPage":1,"data":[{
          "infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
          "orgCode":"O1","publishDate":"2026-07-20",
          "indvAimPriceT":1430,"indvAimPriceL":1400
        }]}"#
            .as_bytes(),
        r#"{"hits":1,"size":1,"TotalPage":1,"data":[{
          "infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
          "orgCode":"O1","orgName":"机构一","publishDate":"2026-07-20",
          "indvAimPriceT":0,"indvAimPriceL":0
        }]}"#
            .as_bytes(),
    ] {
        assert!(
            parse_target_price_pages(&[fixture.to_vec()], &request, 100).is_err(),
            "{fixture:?}"
        );
    }

    let rows = r#"{"infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
        "orgCode":"O1","orgName":"机构一","publishDate":"2026-07-20",
        "indvAimPriceT":1430,"indvAimPriceL":1400},
      {"infoCode":"R2","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
        "orgCode":"O2","orgName":"机构二","publishDate":"2026-07-21",
        "indvAimPriceT":null,"indvAimPriceL":null}"#;
    let fixture = format!(r#"{{"hits":2,"size":2,"TotalPage":1,"data":[{rows}]}}"#).into_bytes();
    let batch = parse_target_price_pages(&[fixture], &request, 100).unwrap();
    assert_eq!(batch.records()[0].sample_count().get(), 1);
}

#[test]
fn report_parser_maps_wildcard_industries_author_fallbacks_and_beijing_identity() {
    let wildcard = ReportScope::Industry(NonEmptyText::new("*").unwrap());
    let fixture = r#"{"data":[{
      "infoCode":"AP1","title":"行业报告","publishDate":"2026-07-23",
      "orgName":"机构一","researcher":[],"author":"研究员甲",
      "industryCode":"481","predictThisYearEps":0.1,
      "predictNextYearEps":0.2,"predictNextTwoYearEps":0.3
    }]}"#
        .as_bytes();
    let batch = parse_reports(fixture, &wildcard).unwrap();
    let report = &batch.records()[0];
    assert_eq!(
        report.scope,
        ReportScope::Industry(NonEmptyText::new("481").unwrap())
    );
    assert_eq!(report.author.as_ref().unwrap().as_str(), "研究员甲");
    assert_eq!(report.estimates.len(), 3);

    let beijing = InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap();
    let fixture = r#"{"data":[{
      "infoCode":"AP2","title":"北证报告","publishDate":"2026-07-23",
      "orgName":"机构二","researcher":[],"author":[],
      "stockCode":"920118","market":"beijing"
    }]}"#
        .as_bytes();
    let batch = parse_reports(fixture, &ReportScope::Instrument(beijing)).unwrap();
    assert!(batch.records()[0].author.is_none());

    assert!(parse_reports(br#"{"data":[]}"#, &wildcard).is_err());
}

/// 空 data 数组 = 源对该精确请求确认无报告 (业务态), 必须分类为 VerifiedEmpty
/// 而非 Protocol 缺陷 — 服务器据此映射业务态而非 invalid_evidence
/// (2026-08-30: 605178/300128 空响应曾被误判 "no usable records")。
#[test]
fn empty_report_response_is_verified_empty_not_protocol_defect() {
    let wildcard = ReportScope::Industry(NonEmptyText::new("*").unwrap());
    let err = parse_reports(br#"{"data":[]}"#, &wildcard)
        .expect_err("empty data array must be an error variant");
    match err {
        EastmoneyError::VerifiedEmpty(empty) => {
            assert_eq!(empty.family(), "research_reports");
            assert_eq!(empty.request_identity(), "industry:*");
            assert!(empty.reason().contains("data=[]"));
        }
        other => panic!("expected VerifiedEmpty, got {other:?}"),
    }
}

#[test]
fn public_report_and_target_price_facades_reject_invalid_instruments_before_transport() {
    let invalid = InstrumentId::new(Exchange::Shanghai, "510050", AssetClass::Fund).unwrap();
    let transport = ScriptedTransport::from_bodies([]);
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);
    let report_request = ResearchRequest::new(
        ReportScope::Instrument(invalid.clone()),
        PositiveU32::new(1).unwrap(),
        PositiveU32::new(20).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        client.research_reports(&report_request),
        Err(EastmoneyError::Unsupported(_))
    ));
    let target_request = TargetPriceRequest::new(
        invalid,
        magic_market_core::IsoDate::new("2026-01-01").unwrap(),
        magic_market_core::IsoDate::new("2026-07-27").unwrap(),
    )
    .unwrap();
    assert!(matches!(
        client.target_price_consensus(&target_request),
        Err(EastmoneyError::Unsupported(_))
    ));
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn target_price_aggregation_rejects_cross_page_and_cardinality_contradictions() {
    let request = TargetPriceRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        magic_market_core::IsoDate::new("2026-01-01").unwrap(),
        magic_market_core::IsoDate::new("2026-07-27").unwrap(),
    )
    .unwrap();
    let row = |id: &str, low: &str| {
        format!(
            r#"{{"infoCode":"{id}","stockCode":"600519","stockName":"贵州茅台",
              "market":"SHANGHAI","orgCode":"O{id}","orgName":"机构{id}",
              "publishDate":"2026-07-20","indvAimPriceT":1430,"indvAimPriceL":{low}}}"#
        )
    };
    let page = |total_pages: u32, hits: u32, rows: &[String]| {
        format!(
            r#"{{"hits":{hits},"size":{},"TotalPage":{total_pages},"data":[{}]}}"#,
            rows.len(),
            rows.join(",")
        )
        .into_bytes()
    };

    let inconsistent_hits = [
        page(2, 2, &[row("1", "1400")]),
        page(2, 3, &[row("2", "1400")]),
    ];
    assert!(matches!(
        parse_target_price_pages(&inconsistent_hits, &request, 1),
        Err(EastmoneyError::Protocol(message)) if message.contains("hits changed")
    ));

    let short_nonfinal = [
        page(2, 2, &[row("1", "1400")]),
        page(2, 2, &[row("2", "1400")]),
    ];
    assert!(matches!(
        parse_target_price_pages(&short_nonfinal, &request, 2),
        Err(EastmoneyError::Protocol(message)) if message.contains("before final page")
    ));

    assert!(matches!(
        parse_target_price_pages(&[page(1, 2, &[row("1", "1400")])], &request, 100),
        Err(EastmoneyError::Protocol(message)) if message.contains("source hits")
    ));
    assert!(matches!(
        parse_target_price_pages(
            &[page(1, 2, &[row("1", "1400"), row("2", "1400")])],
            &request,
            1,
        ),
        Err(EastmoneyError::Protocol(message)) if message.contains("imply 2 pages")
    ));

    assert!(matches!(
        parse_target_price_pages(
            &[page(1, 2, &[row("1", "1400"), row("2", "null")])],
            &request,
            100,
        ),
        Err(EastmoneyError::Protocol(message)) if message.contains("only one")
    ));
}

#[test]
fn research_document_rejects_url_unsafe_report_identity_before_transport() {
    let transport = ScriptedTransport::from_bodies([]);
    let requests = transport.requests();
    let client = EastmoneyClient::with_transport(transport);
    let request = ResearchDocumentRequest {
        report_id: NonEmptyText::new("unsafe/id").unwrap(),
        pdf_url: HttpsUrl::new("https://pdf.dfcfw.com/pdf/H3_unsafe/id_1.pdf").unwrap(),
    };

    assert!(matches!(
        client.research_document(&request),
        Err(EastmoneyError::InvalidRequest(message))
            if message.contains("URL-unsafe")
    ));
    assert!(requests.lock().unwrap().is_empty());
}
