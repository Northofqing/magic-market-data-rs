use super::*;
use magic_eastmoney_rs::EastmoneyTransport;

struct EmptyLimitPoolTransport {
    calls: Arc<AtomicUsize>,
}

impl EastmoneyTransport for EmptyLimitPoolTransport {
    fn get(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        assert!(url.starts_with("https://push2ex.eastmoney.com/"));
        assert!(url.contains("date=20260723") && url.contains("pagesize=10"));
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(br#"{"rc":0,"data":{"tc":0,"qdate":20260723,"pool":[]}}"#.to_vec())
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        panic!("limit-pool public query must not POST");
    }
}

#[test]
fn registered_upper_limit_review_preserves_source_verified_empty_for_all_four_families() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = EastmoneyClient::with_transport(EmptyLimitPoolTransport {
        calls: calls.clone(),
    });
    let mut registry = OperationRegistry::all_unadmitted("offline external source fixture only");
    registry
        .register_handler(
            Capability {
                operation: Operation::UpperLimitPoolReview,
                repository_admitted: true,
                runtime_available: true,
                provider: "Eastmoney".into(),
                exact_scope: "existing atomic four-family review from exact source-empty responses"
                    .into(),
                blocker: None,
                diagnostic_available: false,
            },
            move |command| execute_upper_limit_pool_review(&client, command, 32_768),
        )
        .unwrap();
    let command = QueryCommand::new(
        "source-empty-limit-pool-review",
        Operation::UpperLimitPoolReview,
        Some("Eastmoney".into()),
        CanonicalPayload::new(
            "magic.market.upper_limit_pool_review.request",
            1,
            br#"{"trading_date":"2026-07-23","per_pool_limit":10}"#.to_vec(),
            4096,
        )
        .unwrap(),
    )
    .unwrap();
    let result = registry.execute(command).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert!(result.complete && result.repository_admitted);
    assert_eq!(result.provider, "Eastmoney");
    assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.records.len(), 1);
    assert_eq!(
        result.records[0].schema(),
        "magic.market.upper_limit_pool_review"
    );
    assert_eq!(result.records[0].schema_version(), 1);
    let record: UpperLimitPoolReviewRecord =
        serde_json::from_slice(result.records[0].data()).unwrap();
    assert!(record.upper().is_empty());
    assert!(record.broken().is_empty());
    assert!(record.lower().is_empty());
    assert!(record.previous_upper().is_empty());
    assert_eq!(record.maximum_streak(), None);
    assert_eq!(record.input_evidence().len(), 4);
    for evidence in record.input_evidence() {
        assert_eq!(evidence.provider(), ProviderId::Eastmoney);
        assert_eq!(evidence.source_at(), Some("2026-07-23"));
        assert!(!evidence.batch_id().is_empty());
        assert!(EvidenceTimestamp::parse_instant(evidence.observed_at()).is_ok());
    }
    assert_eq!(result.observed_at, record.input_evidence()[3].observed_at());
    assert_eq!(record.input_digest_sha256().len(), 64);
}
