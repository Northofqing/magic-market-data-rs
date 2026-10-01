use super::*;
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const URL: &str = "https://www.stats.gov.cn/sj/zxfb/202609/t20260930_1.html";
const LIST: &str = r#"<div class="list-content"><ul><li><a class="pc_1600" href="./202609/t20260930_1.html" title="原始标题">原始标题</a><span>2026-09-30</span></li><li><a class="pc_1600" href="./202609/t20260929_2.html" title="第二标题">第二标题</a><span>2026-09-29</span></li></ul></div>"#;
const ARTICLE: &str = r#"<meta name="ArticleTitle" content="原始标题"><meta name="PubDate" content="2026-09-30"><div class="detail-text-content"><div class="TRS_UEDITOR"><p>正文 0</p></div></div>"#;

struct Fixture(Arc<Mutex<Vec<Instant>>>);
impl HttpTransport for Fixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.lock().unwrap().push(Instant::now());
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("text/html".into()),
            if request.url() == URL { ARTICLE } else { LIST }
                .as_bytes()
                .to_vec(),
        ))
    }
}

fn registry() -> (OperationRegistry, Arc<Mutex<Vec<Instant>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let client =
        OfficialNewsClient::with_transport(OfficialSource::Nbs, Fixture(calls.clone())).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("fixture unavailable");
    register_client(&mut registry, OfficialSource::Nbs, client, 4096).unwrap();
    for op in [
        Operation::OfficialPublications,
        Operation::OfficialPublication,
    ] {
        registry
            .register_unavailable(capability(op, OfficialSource::Gacc, false))
            .unwrap();
        registry.set_default_provider(op, "Nbs").unwrap();
    }
    (registry, calls)
}

fn command(
    operation: Operation,
    schema: &str,
    version: u32,
    value: Value,
    provider: Option<&str>,
) -> QueryCommand {
    QueryCommand::new(
        "fixture",
        operation,
        provider.map(str::to_owned),
        CanonicalPayload::new(schema, version, serde_json::to_vec(&value).unwrap(), 4096).unwrap(),
    )
    .unwrap()
}

#[test]
fn service_preserves_full_native_evidence_and_shares_list_article_pacing() {
    let (registry, calls) = registry();
    let list = registry
        .execute(command(
            Operation::OfficialPublications,
            OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA,
            1,
            json!({"limit":1}),
            None,
        ))
        .unwrap();
    let article = registry
        .execute(command(
            Operation::OfficialPublication,
            OFFICIAL_PUBLICATION_REQUEST_SCHEMA,
            1,
            json!({"url":URL}),
            Some("Nbs"),
        ))
        .unwrap();
    for result in [&list, &article] {
        assert_eq!(result.provider, "Nbs");
        assert!(result.complete && result.repository_admitted);
        assert!(result.source_at.is_none());
        assert_eq!(result.records.len(), 1);
        let value: Value = serde_json::from_slice(result.records[0].data()).unwrap();
        assert_eq!(value["observed_at"], result.observed_at);
        assert_eq!(value["source"], "Nbs");
    }
    let listing: Value = serde_json::from_slice(list.records[0].data()).unwrap();
    assert_eq!(listing["source_rows"], 2);
    assert_eq!(listing["items"].as_array().unwrap().len(), 1);
    assert_eq!(listing["items"][0]["canonical_url"], URL);
    assert_eq!(
        listing["items"][0]["publication_label_origin"],
        "ListingHtml"
    );
    assert_eq!(
        listing["response_sha256"],
        format!("{:x}", Sha256::digest(LIST.as_bytes()))
    );
    let original: Value = serde_json::from_slice(article.records[0].data()).unwrap();
    assert_eq!(original["precision"], "Date");
    assert_eq!(original["publication_label_origin"], "ArticleMetadata");
    assert_eq!(original["publication_label"], "2026-09-30");
    assert_eq!(original["content"], "正文 0");
    assert_eq!(
        original["response_sha256"],
        format!("{:x}", Sha256::digest(ARTICLE.as_bytes()))
    );
    assert_ne!(list.batch_id, article.batch_id);
    let starts = calls.lock().unwrap();
    assert_eq!(starts.len(), 2);
    assert!(starts[1].duration_since(starts[0]) >= Duration::from_millis(990));
}

#[test]
fn service_request_and_admission_rejections_precede_transport() {
    let (registry, calls) = registry();
    for (schema, version, value) in [
        ("wrong", 1, json!({"limit":1})),
        (OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA, 2, json!({"limit":1})),
        (OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA, 1, json!({"limit":0})),
        (OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA, 1, json!({"limit":21})),
        (
            OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA,
            1,
            json!({"limit":1, "endpoint":"https://evil.test"}),
        ),
    ] {
        assert!(matches!(
            registry.execute(command(
                Operation::OfficialPublications,
                schema,
                version,
                value,
                None
            )),
            Err(ServiceError::InvalidRequest(_))
        ));
    }
    for url in [
        "http://www.stats.gov.cn/sj/zxfb/202609/t20260930_1.html",
        "https://evil.test/original.html",
    ] {
        assert!(registry
            .execute(command(
                Operation::OfficialPublication,
                OFFICIAL_PUBLICATION_REQUEST_SCHEMA,
                1,
                json!({"url":url}),
                Some("Nbs")
            ))
            .is_err());
    }
    for operation in [
        Operation::OfficialPublications,
        Operation::OfficialPublication,
    ] {
        let request = command(
            operation,
            OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA,
            1,
            json!({"limit":1}),
            Some("Gacc"),
        );
        assert!(matches!(
            registry.execute(request.with_unadmitted_access(true)),
            Err(ServiceError::Unsupported { .. })
        ));
    }
    assert!(matches!(
        registry.execute(command(
            Operation::OfficialPublications,
            OFFICIAL_PUBLICATIONS_REQUEST_SCHEMA,
            1,
            json!({"limit":1}),
            Some("Unknown")
        )),
        Err(ServiceError::Unsupported { .. })
    ));
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn service_source_failures_are_typed_and_do_not_leak_upstream_body() {
    for (error, expected) in [
        (
            OfficialNewsError::Transport(TransportError::HttpStatus { status: 429 }),
            ProviderFailureKind::RateLimited,
        ),
        (
            OfficialNewsError::Transport(TransportError::Network("private upstream body".into())),
            ProviderFailureKind::Unavailable,
        ),
        (
            OfficialNewsError::Protocol("private upstream body".into()),
            ProviderFailureKind::ResponseInvalid,
        ),
    ] {
        let failure = source_error(Operation::OfficialPublication, OfficialSource::Pbc, error);
        let ServiceError::ProviderFailure {
            kind,
            provider,
            provider_reason,
            ..
        } = failure
        else {
            panic!("typed failure required");
        };
        assert_eq!(kind, expected);
        assert_eq!(provider, "Pbc");
        assert!(!provider_reason.contains("private"));
    }
}

#[test]
fn official_registry_exposes_exactly_eight_admitted_sources_per_operation() {
    let registry = official_publication_registry(Duration::from_secs(1), 4096).unwrap();
    for operation in [
        Operation::OfficialPublications,
        Operation::OfficialPublication,
    ] {
        let caps = registry
            .capabilities()
            .into_iter()
            .filter(|cap| cap.operation == operation)
            .collect::<Vec<_>>();
        assert_eq!(caps.len(), 9);
        assert_eq!(
            caps.iter()
                .filter(|cap| cap.repository_admitted && cap.runtime_available)
                .count(),
            8
        );
        let gacc = caps.iter().find(|cap| cap.provider == "Gacc").unwrap();
        assert!(!gacc.repository_admitted && !gacc.runtime_available && !gacc.diagnostic_available);
        assert_eq!(registry.default_provider(operation), Some("Nbs"));
    }
    assert!(official_publication_registry(Duration::ZERO, 4096).is_err());
    assert!(official_publication_registry(Duration::from_secs(1), 0).is_err());
}
